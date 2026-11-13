//! `importation-apercu` sur la boite aux lettres `import/`.
//! RIEN N'EST ECRIT.
//!
//! 1. Le tri (`import::tri`) : chaque fichier recoit sa sorte, par le contenu.
//! 2. Les DSN et AEM trouvees, ou qu'elles soient (sous-dossier, zip), sont copiees dans un dossier de travail
//!    (temporaire, efface avant de rendre) et passent par `crate::import::apercu_lu`, qui lit leurs
//!    sous-dossiers `DSN/` et `AEM_EDI/` -- celles d'une autre entreprise exceptees : devant le Siren de
//!    la compagnie (`importation::siren_de_reference`, etape 3), elles prennent le sort `incoherent` et rien n'en est lu.
//! 3. Les bulletins (`bulletin_lu`), page par page, completent cet apercu : les fiches qu'ils creent (salaries par NIR,
//!    spectacles par numero d'objet avec leur titre, l'entreprise) et leurs ecarts avec les fiches existantes (signales,
//!    jamais appliques) ; un contrat atteste par bulletin (`atteste_par: "bulletin"`), que le registre de l'import retient s'il
//!    n'est pas deja atteste (`import::attestes`) ; la paie, rapprochee des paies versees des engagements importes et des
//!    DSN par (NIR, debut, fin).
//!
//! LE BULLETIN FAIT FOI : une paie deja declaree est COMPLETEE par le bulletin (cotisations par
//! part, cout employeur, lignes, dates, titre...) et, sur le brut, le net imposable, le net a payer et le PAS, c'est
//! le montant du bulletin qui s'ecrit ; l'ecart est note dans le compte rendu. Une paie qu'aucune DSN ne declare est
//! nouvelle : `contrat` = le numero que le bulletin imprime (`CleDePaieVersee::Bulletin`, CoolPaie), sinon sa copie (et
//! la page, s'il en a plusieurs : `CleDePaieVersee::Copie`), `mois` = le mois de la fin de periode du bulletin.
//!
//! CE QUI EST SAISI ICI N'EST PAS REPRIS : un bulletin d'un salarie qu'un engagement saisi (non importe) paie dans le
//! mois du bulletin est « deja saisi », rien d'autre (ni fiche, ni contrat, ni paie) -- sauf s'il a une paie declaree a
//! completer (une DSN de l'apercu ou une paie importee, meme salarie, meme mois) : il la complete, quel que soit
//! l'ordre de la saisie et de l'import.
//!
//! RIEN NE SE DEVINE (regle 1). Une page n'est pas reprise quand : elle n'a pas de texte, le rang d'en-tetes de son
//! tableau ne se trouve pas, elle se lit de deux facons, ce qui l'identifie ne se lit pas (le lecteur), son NIR ne se lit pas ou contredit sa cle, son
//! Siret n'est pas celui de la compagnie (« bulletin d'une autre entreprise »), plusieurs paies declarees pourraient
//! etre la sienne, ou un autre bulletin donne d'autres montants pour la meme paie. LE BULLETIN FAIT FOI : une
//! page qui se contredit elle-meme (`incohérent : ...` du lecteur : le brut n'est pas la somme des lignes, le net ne
//! se retrouve pas -- un centime sur le net avant impot chez CoolPaie) est REPRISE avec les montants qu'elle imprime,
//! et l'incoherence est un avertissement du compte rendu. Un libelle de ligne inconnu de la table n'empeche pas la
//! reprise : la ligne est gardee telle quelle, sans compteur, et le sort le dit.
//!
//! LES NOMS DE FICHIER NE PORTENT RIEN DE PERSONNEL. Ce que l'import ecrit nomme chaque fichier par sa COPIE dans
//! `historique/bulletins/` : `<16 premiers caracteres de l'empreinte SHA-256>.<pdf|dsn|txt>` (`nom_copie`) -- le
//! `fichier` d'une paie ou d'un contrat, le `bulletin.fichier`, le `contrat` d'une paie sans numero de bulletin. Le
//! meme nom dans l'apercu (lu dans `import/`) et dans l'import (relu dans la copie) : un import se rejoue, un apercu
//! d'apres l'import ne voit rien de neuf. Le compte rendu (`tri`) garde le chemin d'origine, pour l'ecran, et dit la
//! `copie` de chaque fichier repris.
//!
//! LE COMPTE RENDU (`tri`) donne a chaque fichier un sort, et un seul, avec sa raison en une phrase (sans nom ni NIR :
//! la phrase peut partir chez l'editeur) ; un PDF a aussi le sort de chacune de ses pages. Les sorts : `reponse::Sort`.

use droit::fiche::Lue as _;
use socle::echec::Classe;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::RoundingStrategy;
use serde::Serialize;

use import::bulletin_lu::{self, BulletinLu, Compteurs, Table};
use declarations::champs::{Saisie, Valeur};
use droit::contexte::Contexte;
use droit::natures::NatureDeContrat;
use droit::fiche::{CaseDeFiche, CaseSalarie, CaseSpectacle, Nir, NomDeCase, NomDeFiche, NumeroObjetSpectacle};
use engagement::importe::EmploiImprime;
use socle::date::Jour;
use socle::echec::R;
use socle::fichiers::NomDeFichier;
use socle::mois::Mois;
use socle::echec::erreur;
use crate::importation::completer as completer;
use crate::fiches::FicheNommee;
use import::proposee;
use import::tri::Sorte;
use socle::num::Decimal;
use socle::texte::euros;
use import::attestes::{Atteste, Registre, Reste};
use engagement::importe::{Attestation, CleDePaieVersee, ContratAtteste, ContratLu, PaieLue, PaieVersee};
use engagement::numeros::NumeroContrat;
use crate::import::{self as ri, ACreer, Apercu, SpectacleDesPieces};
use droit::contexte::IdSalarie;
use import::reponse::{
    BlocBulletins, BlocPieces, DitPar, Ecart, EcartPaie, EtatContrat, EtatFiche, EtatPaie, FicheImportee, FichierVu, Importation, InfosPage, NonRepris, NonReprisVu,
    Origine, PageVu, ParSort, Sort, Source, TriVu,
};

/// Ce que l'encart dit d'un import qui laisse des documents : il n'est pas bloquant, et se rejoue.
pub const REESSAYER: &str = "Le logiciel s'améliore sans cesse : réessayez ces documents plus tard ; ce qui sera repris s'ajoutera à votre historique.";

/// Ce qu'un bulletin compte du contrat qu'il atteste : ses heures, ses cachets, ses jours, son brut.
struct Compte {
    heures: i64,
    cachets: i64,
    jours: i64,
    brut: Decimal,
}


/// Le compte rendu d'une page de PDF.
struct PageCr {
    page: usize,
    sort: Sort,
    raison: String,
    infos: InfosPage,
}

/// Un bulletin lu qui peut etre repris.
struct Lu {
    /// Le rang du fichier dans le tri.
    fichier: usize,
    page: usize,
    b: BulletinLu,
    /// Les 13 caracteres du NIR (sans la cle), comme dans les DSN, les AEM et les fiches.
    nir: Nir,
    /// La cle du NIR, quand le bulletin l'imprime.
    cle_nir: Option<droit::fiche::CleNir>,
    /// Le mois de la fin de periode.
    mois: Mois,
    /// Le mois a des engagements saisis (`mois_saisis`).
    mois_saisi: bool,
    compteurs: R<Compteurs>,
    /// Ce que la page dit d'elle-meme qui ne se retrouve pas (`incohérent : ...`) : un avertissement, le bulletin fait foi.
    incoherences: Vec<String>,
}

/// Ce que la lecture d'un dossier rend : l'apercu, le tri (les fichiers sur le disque, a lire avant d'effacer le dossier
/// de travail), et le nom de la copie de chaque fichier repris (par rang dans le tri).
pub(crate) struct DossierLu {
    pub(crate) ap: Apercu,
    pub(crate) tri: import::tri::Tri,
    pub(crate) copies: BTreeMap<usize, NomDeFichier>,
}

/// `importation-apercu` avec `{"dossier": ...}`.
pub(crate) fn apercu_dossier(ctx: &Contexte, dossier: &Path) -> R<Importation> {
    let mut r = lire_dossier(ctx, dossier, &HashMap::new(), |lu| Ok(lu.ap))?.reponse;
    r.dire_l_apercu();
    Ok(r)
}

/// Lit le dossier, et passe ce qui a ete lu a `suite` AVANT d'effacer le dossier de travail (les zip ouverts) : la
/// copie de l'import et le texte des pages refusees s'y lisent encore. `deposes` : le nom que la personne a donne a un
/// fichier du dossier, par son chemin dans le dossier, quand le dossier lu est fait de copies (`import::ecrite`) ; tout
/// ce que l'import dit d'un fichier (« lu dans… », le compte rendu) le nomme ainsi.
pub(crate) fn lire_dossier<T>(ctx: &Contexte, dossier: &Path, deposes: &HashMap<String, String>, suite: impl FnOnce(DossierLu) -> R<T>) -> R<T> {
    let travail = ri::dossier_de_travail();
    match trier_et_lire(ctx, dossier, &travail, deposes).and_then(suite) {
        Ok(t) => socle::fichiers::effacer(&travail).map(|()| t),
        Err(e) => Err(socle::fichiers::effacer_apres(e, &travail)),
    }
}

/// Le nom de la copie d'un fichier dans `historique/bulletins/` : l'empreinte, rien de personnel. `None` pour ce qui ne
/// se reprend pas.
pub(crate) fn nom_copie(f: &import::tri::Fichier) -> R<Option<NomDeFichier>> {
    let ext = match f.sorte {
        Sorte::Dsn => "dsn",
        // `importation::lire_aem` ne lit que les `*.txt`.
        Sorte::Aem => "txt",
        Sorte::PdfTexte => "pdf",
        _ => return Ok(None),
    };
    f.empreinte.as_ref().map(|e| NomDeFichier::lire(&format!("{}.{}", e.courte(), ext))).transpose()
}

/// LE NOM DE LA COPIE D'UNE PIECE LUE : une piece lue a ete hachee au tri, elle a donc sa copie ; sans elle, c'est un
/// defaut du logiciel.
fn copie_lue(f: &import::tri::Fichier) -> R<NomDeFichier> {
    nom_copie(f)?.ok_or_else(|| erreur(Classe::DefautDuLogiciel, format!("Défaut du logiciel : la pièce lue {} n'a pas de copie.", f.chemin)))
}

/// Un texte compare sans casse, sans accents, sans espaces ni ponctuation de bord (`bulletin_lu::jetons`).
pub(crate) fn compare(t: &str) -> String {
    bulletin_lu::jetons(t).concat()
}

/// Le NIR imprime : 13 caracteres, ou 15 avec la cle (alors verifiee).
fn nir_lu(b: &BulletinLu) -> R<Option<(Nir, Option<droit::fiche::CleNir>)>> {
    let Some(brut) = &b.salarie.nir else { return Ok(None) };
    let n: String = brut.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_uppercase();
    match Nir::et_sa_cle(&n) {
        Some((nir, None)) => Ok(Some((Nir::lire(nir)?, None))),
        Some((nir, Some(cle))) => {
            let nir = Nir::lire(nir)?;
            match droit::fiche::CleNir::du_nir(&nir).filter(|c| **c == *cle) {
                Some(c) => Ok(Some((nir, Some(c)))),
                None => Err(socle::echec::refus("la clé du NIR imprimé ne correspond pas à son numéro")),
            }
        }
        None => Err(socle::echec::refus("le NIR imprimé n'a ni 13 ni 15 caractères")),
    }
}

/// Le Siret imprime, ses chiffres ; des chiffres qui ne font pas un Siret, le bulletin ne le dit pas.
fn siret_lu(b: &BulletinLu) -> Option<droit::fiche::Siret> {
    let s: String = b.entreprise.siret.as_deref()?.chars().filter(char::is_ascii_digit).collect();
    droit::fiche::Siret::lire(&s).ok()
}

/// Une adresse imprimee : les rangs avant le code postal, puis « 99999 Commune ». Sans rang de code postal, rien
/// (on ne devine pas ce qui est la rue et ce qui est la ville).
fn adresse_lue(rangs: &[String]) -> Option<(String, String, String)> {
    // Un rang « 99999 Commune » : cinq chiffres, un blanc, la commune.
    let i = rangs.iter().position(|r| {
        r.char_indices().nth(5).is_some_and(|(i, _)| {
            let (cp, reste) = r.split_at(i);
            cp.len() == 5 && cp.chars().all(|c| c.is_ascii_digit()) && reste.starts_with(' ') && !reste.trim().is_empty()
        })
    })?;
    let rue = rangs[..i].join(", ");
    let (cp, commune) = rangs[i].split_at(5);
    Some((rue, cp.to_string(), commune.trim().to_string()))
}

/// LE COMPTE URSSAF imprime apres « Urssaf : » : ses chiffres, sans espaces. C'est la case que le moteur
/// imprime de `urssaf_compte` sous la meme etiquette (`bulletin.rs`) ; sur les bulletins CoolPaie du poste, la valeur
/// imprimee a cette place est celle du `urssaf_compte` de la fiche (releve du 14/10/2026 sur trois bulletins de 2022 a
/// 2026, valeurs non recopiees) -- une inference pour un autre logiciel. Ce qui n'est pas que des chiffres ne se reprend pas.
fn urssaf_lu(b: &BulletinLu) -> Option<String> {
    let t: String = b.entreprise.urssaf.as_deref()?.chars().filter(|c| !c.is_whitespace()).collect();
    (!t.is_empty() && t.chars().all(|c| c.is_ascii_digit())).then_some(t)
}

/// L'IDCC d'une convention imprimee (« IDCC n° 3090 - ... ») ; rien si le texte ne le dit pas.
fn idcc_lu(texte: &str) -> Option<droit::fiche::Idcc> {
    let haut = texte.to_uppercase();
    let i = haut.find("IDCC")?;
    let apres: String = haut[i + 4..].chars().skip_while(|c| !c.is_ascii_digit()).take_while(char::is_ascii_digit).collect();
    let saut = haut[i + 4..].chars().take_while(|c| !c.is_ascii_digit()).count();
    if saut > 6 {
        return None;
    }
    droit::fiche::Idcc::lire(&apres).ok()
}

fn trier_et_lire(ctx: &Contexte, dossier: &Path, travail: &Path, deposes: &HashMap<String, String>) -> R<DossierLu> {
    let mut tri = import::tri::trier(dossier, travail)?;
    for f in &mut tri.fichiers {
        if let Some(nom) = deposes.get(&f.chemin) {
            f.chemin = nom.clone();
        }
    }

    // 1. Les DSN et AEM : ce que chacune dit de son employeur. Elles ne sont copiees la ou l'import existant
    // les lit qu'apres l'etape 3, et seulement celles de la compagnie.
    let pieces: Vec<(usize, bool, import::dsn_aem::Employeur)> = tri
        .fichiers
        .iter()
        .enumerate()
        .filter_map(|(i, f)| match f.sorte {
            Sorte::Dsn => Some(import::dsn_aem::employeur_dsn(&f.disque).map(|e| (i, false, e))),
            Sorte::Aem => Some(import::dsn_aem::employeur_aem(&f.disque).map(|e| (i, true, e))),
            _ => None,
        })
        .collect::<R<_>>()?;

    // 2. Les bulletins, page par page.
    let table = Table::livree().map_err(|m| m.reformuler(|p| format!("La table de lecture des bulletins ne se lit pas : {}", p)).reclasser(Classe::Refuse))?;
    let saisis: HashSet<Mois> = engagement::stock::mois_saisis(&ctx.racine).map_err(|m| m.reclasser(Classe::Refuse))?.into_iter().collect();
    let mut pages: BTreeMap<usize, Vec<PageCr>> = BTreeMap::new();
    let mut illisibles: BTreeMap<usize, String> = BTreeMap::new();
    let mut lus: Vec<Lu> = Vec::new();
    let mut certificats: Vec<Attestee> = Vec::new();
    let mut par_logiciel: BTreeMap<String, usize> = BTreeMap::new();
    let (mut du, mut au): (Option<Jour>, Option<Jour>) = (None, None);
    let mut n_pages = 0usize;
    for (i, f) in tri.fichiers.iter().enumerate() {
        if f.sorte != Sorte::PdfTexte {
            continue;
        }
        let lectures = match bulletin_lu::lire_avec(&f.disque, &table) {
            Ok(l) => l,
            Err(m) => {
                illisibles.insert(i, m.phrase.replace(&*f.disque.to_string_lossy(), &f.chemin));
                continue;
            }
        };
        // LES CERTIFICATS D'EMPLOI CONGES SPECTACLES du fichier : ils se lisent a part (`certificat_lu`), pas comme des
        // bulletins ; l'AER Cinéma Spectacle par ses champs, les AEM par leurs rubriques. Un fichier dont une de ces
        // lectures ne se fait pas n'est pas repris, et le dit.
        let octets = std::fs::read(&f.disque).map_err(|e| socle::echec::du_disque(&f.disque, &e).reclasser(Classe::Disque))?;
        let lues = (import::certificat_lu::lire(&f.disque), import::aem_lu::lire_aer(&octets), import::aem_lu::lire(&f.disque));
        let (certificats_du_fichier, aer, aem) = match lues {
            (Ok(c), Ok(r), Ok(a)) => (c, r, a),
            (Err(m), _, _) | (_, Err(m), _) | (_, _, Err(m)) => {
                illisibles.insert(i, m.phrase.replace(&*f.disque.to_string_lossy(), &f.chemin));
                continue;
            }
        };
        let cr = pages.entry(i).or_default();
        let mut pages_certificats: HashSet<usize> = certificats_du_fichier.iter().map(|c| c.page).collect();
        for pc in certificats_du_fichier {
            match pc.resultat {
                Ok(c) => {
                    certificats.push(Attestee { fichier: i, c, aem: None });
                }
                Err((raison, _)) => {
                    cr.push(PageCr { page: pc.page, sort: Sort::Incomplet, raison, infos: InfosPage::default() });
                }
            }
        }
        // L'AER CINEMA SPECTACLE (`aem_lu::lire_aer`) : un formulaire entier, chaque periode attestee comme une AEM.
        if let Some(colonnes) = aer {
            pages_certificats.extend(1..=f.pages.len());
            for pa in colonnes {
                match pa.lecture {
                    import::aem_lu::Lecture::Lue(a) => {
                        let c = a.commun();
                        certificats.push(Attestee { fichier: i, c, aem: Some(a) });
                    }
                    import::aem_lu::Lecture::Refusee { sort, raison, .. } => {
                        cr.push(PageCr { page: pa.page, sort, raison, infos: InfosPage::default() });
                    }
                    import::aem_lu::Lecture::Notice => {}
                }
            }
        }
        // LES AEM EN PDF du fichier (`aem_lu`), et la notice du formulaire.
        for pa in aem {
            if !pages_certificats.insert(pa.page) {
                continue;
            }
            match pa.lecture {
                import::aem_lu::Lecture::Lue(a) => {
                    let c = a.commun();
                    certificats.push(Attestee { fichier: i, c, aem: Some(a) });
                }
                import::aem_lu::Lecture::Refusee { sort, raison, .. } => {
                    cr.push(PageCr { page: pa.page, sort, raison, infos: InfosPage::default() });
                }
                import::aem_lu::Lecture::Notice => {
                    cr.push(PageCr { page: pa.page, sort: Sort::Annexe, raison: "la notice de l'AEM : rien à reprendre".into(), infos: InfosPage::default() });
                }
            }
        }
        for l in lectures {
            if pages_certificats.contains(&l.page) {
                continue;
            }
            n_pages += 1;
            let b = match l.resultat {
                Ok(b) => b,
                Err(r) => {
                    cr.push(PageCr { page: l.page, sort: r.code(), raison: r.raison(), infos: InfosPage::default() });
                    continue;
                }
            };
            let logiciel = match &b.logiciel {
                Some(l) => l.clone(),
                None => "autre".to_string(),
            };
            *par_logiciel.entry(logiciel).or_default() += 1;
            du = du.into_iter().chain([b.debut]).min();
            au = au.into_iter().chain([b.fin]).max();
            let mois = b.fin.mois_civil();
            let infos = InfosPage {
                logiciel: b.logiciel.clone(),
                numero: b.numero.clone(),
                debut: Some(b.debut),
                fin: Some(b.fin),
                mois: Some(mois),
                ..InfosPage::default()
            };
            let refus = |sort: Sort, raison: String, infos: InfosPage| PageCr { page: l.page, sort, raison, infos };
            let mois_saisi = saisis.contains(&mois);
            // LE BULLETIN FAIT FOI : une page qui se contredit (un centime sur le net) est reprise telle qu'imprimee.
            let incoherences: Vec<String> = b.avertissements.iter().filter(|a| a.starts_with("incohérent")).cloned().collect();
            let (nir, cle_nir) = match nir_lu(&b) {
                Ok(Some(n)) => n,
                Ok(None) => {
                    cr.push(refus(Sort::Incomplet, "le NIR ne se lit pas : la paie ne se rattache à aucune fiche".into(), infos));
                    continue;
                }
                Err(m) => {
                    cr.push(refus(Sort::Incoherent, m.phrase, infos));
                    continue;
                }
            };
            // Les compteurs se font plus bas, quand tous les bulletins du salarie sont lus (`compter`).
            lus.push(Lu { fichier: i, page: l.page, b, nir, cle_nir, mois, mois_saisi, compteurs: Ok(Compteurs::default()), incoherences });
        }
    }

    // 3. La compagnie : son Siren (`importation::siren_de_reference`, le meme pour l'export) -- la fiche, sinon les DSN
    // si elles n'en portent qu'un, sinon les bulletins et les AEM s'ils n'en portent qu'un. Une DSN, une AEM ou un
    // bulletin d'une autre entreprise n'est pas repris ; d'une DSN ou d'une AEM ecartee, RIEN n'est lu.
    let siren_fiche = ctx.entreprise.siren.portee().map(|s| s.trim().to_string());
    let sirens_dsn: std::collections::BTreeSet<droit::fiche::Siren> =
        pieces.iter().filter(|(_, aem, _)| !aem).flat_map(|(_, _, e)| e.sirens.iter().cloned()).collect();
    let mut sirens_autres: std::collections::BTreeSet<droit::fiche::Siren> =
        pieces.iter().filter(|(_, aem, _)| *aem).flat_map(|(_, _, e)| e.sirens.iter().cloned()).collect();
    sirens_autres.extend(lus.iter().filter_map(|l| siret_lu(&l.b)).map(|s| s.siren()));
    sirens_autres.extend(certificats.iter().filter_map(|c| c.c.siret.as_ref()).map(droit::fiche::Siret::siren));
    let siren = import::dsn_aem::siren_de_reference(&ctx.entreprise, &sirens_dsn, &sirens_autres);
    let mut gardes_cert: Vec<Attestee> = Vec::new();
    for c in certificats {
        let raison = match (&c.c.siret, &siren) {
            (Some(s), Some(r)) if s.siren() != *r => Some(format!("{} d'une autre entreprise : son Siret n'est pas celui de la compagnie, non repris", c.nom())),
            (Some(_), None) => Some(import::dsn_aem::PLUSIEURS_ENTREPRISES.to_string()),
            _ => None,
        };
        match raison {
            Some(raison) => pages.entry(c.fichier).or_default().push(PageCr { page: c.c.page, sort: Sort::Incoherent, raison, infos: InfosPage::default() }),
            None => gardes_cert.push(c),
        }
    }
    let mut certificats = gardes_cert;
    certificats.sort_by(|a, b| (a.c.fin, a.c.debut, a.fichier, a.c.page).cmp(&(b.c.fin, b.c.debut, b.fichier, b.c.page)));
    let mut gardes: Vec<Lu> = Vec::new();
    for l in lus {
        let ecart = match (siret_lu(&l.b), &siren) {
            (Some(s), Some(r)) if s.siren() != *r => {
                Some("bulletin d'une autre entreprise : son Siret n'est pas celui de la compagnie, non repris".to_string())
            }
            (Some(_), None) => Some(import::dsn_aem::PLUSIEURS_ENTREPRISES.to_string()),
            _ => None,
        };
        match ecart {
            Some(raison) => {
                let infos = infos_de(&l, None);
                pages.entry(l.fichier).or_default().push(PageCr { page: l.page, sort: Sort::Incoherent, raison, infos });
            }
            None => gardes.push(l),
        }
    }
    let mut lus = gardes;
    lus.sort_by(|a, b| (a.b.fin, a.b.debut, a.fichier, a.page).cmp(&(b.b.fin, b.b.debut, b.fichier, b.page)));
    compter(&mut lus, &ctx.racine);

    // Les DSN et AEM de la compagnie, copiees la ou `apercu_lu` les lit, sous le nom de leur copie (`nom_copie`).
    // Un nom deja pris recoit un rang devant.
    let export = travail.join("export");
    let mut noms: BTreeMap<usize, String> = BTreeMap::new();
    let (mut dsn_dir, mut aem_dir): (Option<PathBuf>, Option<PathBuf>) = (None, None);
    let mut pris: HashSet<String> = HashSet::new();
    let mut ecartes: BTreeMap<usize, String> = BTreeMap::new();
    let mut avertis: BTreeMap<usize, String> = BTreeMap::new();
    for (i, aem, e) in &pieces {
        let f = &tri.fichiers[*i];
        match import::dsn_aem::verdict_entreprise(*aem, e, siren.as_ref()) {
            import::dsn_aem::Verdict::Ecartee(raison) => {
                ecartes.insert(*i, raison.to_string());
                continue;
            }
            import::dsn_aem::Verdict::Avertie(a) => {
                avertis.insert(*i, a.to_string());
            }
            import::dsn_aem::Verdict::Reprise => {}
        }
        let sous = if *aem { "AEM_EDI" } else { "DSN" };
        let d = export.join(sous);
        std::fs::create_dir_all(&d).map_err(|e| socle::echec::du_disque(&d, &e).reclasser(Classe::Disque))?;
        // Le nom de sa copie ; une piece sans empreinte (une entree d'archive qui ne s'est pas extraite) n'en a pas, elle
        // se copie sous le nom de sa sorte.
        let base = match nom_copie(f)? {
            Some(n) => n.to_string(),
            None => if *aem { "fichier.txt".to_string() } else { "fichier.dsn".to_string() },
        };
        let mut nom = base.clone();
        let mut k = 2;
        while !pris.insert(format!("{}/{}", sous, nom)) {
            nom = format!("{}-{}", k, base);
            k += 1;
        }
        std::fs::copy(&f.disque, d.join(&nom)).map_err(|e| erreur(Classe::Disque, format!("{} : {}", f.chemin, e)))?;
        noms.insert(*i, nom);
        if *aem {
            aem_dir = Some(d);
        } else {
            dsn_dir = Some(d);
        }
    }

    // 4. Les DSN et AEM par l'import existant ; les NIR des bulletins y comptent comme des fiches a venir.
    let a_venir: HashSet<Nir> = lus.iter().map(|l| l.nir.clone()).chain(certificats.iter().filter_map(|c| c.c.nir.clone())).collect();
    let mut ap = ri::apercu_lu(ctx, &dossier.display().to_string(), dsn_dir.as_deref(), aem_dir.as_deref(), ctx.aujourdhui, &a_venir)?;

    let mut par_nir: HashMap<Nir, IdSalarie> = HashMap::new();
    for (cle, s) in &ctx.salaries {
        if let Some(n) = s.nir.dite() {
            par_nir.insert(n.clone(), cle.clone());
        }
    }
    // La cle de la fiche du NIR ; `None` : sa fiche est a creer.
    let cle_de = |nir: &Nir| par_nir.get(nir).cloned();
    // LES CONTRATS ATTESTES : l'historique, puis ce que chaque piece de cet import retient (`import::attestes`).
    let nir_de_cle: HashMap<IdSalarie, Nir> = par_nir.iter().map(|(n, c)| (c.clone(), n.clone())).collect();
    let mut registre = Registre::de_l_historique(ctx, &nir_de_cle)?;
    let historique_paies: Vec<PaieLue> = ri::paies_historique(ctx)?.into_iter().map(engagement::importe::PaieRelue::a_l_apercu).collect();
    let hist_par_cle: HashMap<(Mois, Option<IdSalarie>, CleDePaieVersee), PaieLue> = historique_paies.iter().map(|x| (x.cle(), x.clone())).collect();

    // Ce qui est saisi ici : (salarie, mois) que paie un engagement saisi. Un bulletin qui en est, sans paie declaree
    // du meme salarie et du meme mois a completer (DSN de cet apercu, paie importee), est « deja saisi ».
    let mut saisis_ici: HashSet<(Option<IdSalarie>, Mois)> = HashSet::new();
    if lus.iter().any(|l| l.mois_saisi) {
        let objets = engagement::stock::objets(&ctx.racine).map_err(|m| m.reclasser(Classe::Refuse))?;
        for o in objets.iter().filter(|o| !o.importe_seul()) {
            let Some(cle) = &o.brut.saisie.carte.salarie else { continue };
            for l in lus.iter().filter(|l| l.mois_saisi) {
                if o.couvre(l.mois).map_err(|m| m.reclasser(Classe::Refuse))? {
                    saisis_ici.insert((Some(cle.clone()), l.mois));
                }
            }
        }
    }
    let declarees: HashSet<(Nir, Mois)> = ap
        .paies
        .iter()
        .chain(historique_paies.iter())
        .filter_map(|x| Some((x.paie.nir.clone()?, x.mois)))
        .collect();
    let mut gardes: Vec<Lu> = Vec::new();
    for l in lus {
        if saisis_ici.contains(&(cle_de(&l.nir), l.mois)) && !declarees.contains(&(l.nir.clone(), l.mois)) {
            let raison = format!("le mois {} de ce salarié est saisi ici, sans paie déclarée à compléter : non repris", l.mois);
            pages.entry(l.fichier).or_default().push(PageCr { page: l.page, sort: Sort::DejaSaisi, raison, infos: infos_de(&l, None) });
            continue;
        }
        gardes.push(l);
    }
    let lus = gardes;
    let mut nirs_dsn: HashSet<Nir> = HashSet::new();
    for a in ap.a_creer.iter().filter(|a| a.nom == NomDeFiche::Salarie) {
        nirs_dsn.extend(a.nir()?);
    }

    // LA CONVENTION COLLECTIVE DE L'IMPORT : celle de la fiche Entreprise, sinon celle que les DSN declarent
    // (S21.G00.11.022, S21.G00.40.017), sinon celle que les bulletins impriment, sinon celle des AEM (`EMP_IDCC`, « N°
    // IDCC »). C'est sous elle que se resout le metier d'un salarie qu'une fiche cree (`statut_et_metier`) ; un bulletin
    // qui imprime la sienne se resout sous la sienne. Aucune : la fiche la demandera (`idcc`), rien ne se suppose : le
    // champ d'une convention tient a des faits que les pieces ne portent pas (financement, label, gouvernance : CCN 3090
    // art. 1er, CCNEAC art. I.1, `atelier/reference/sources/conventions-champ-d-application/`).
    let idcc_lus: Vec<Option<droit::fiche::Idcc>> = lus.iter().map(|l| idcc_du_bulletin(&l.b)).collect::<R<_>>()?;
    let convention: Option<droit::fiche::Idcc> = ctx
        .entreprise
        .idcc
        .clone()
        .or_else(|| ap.convention.clone())
        .or_else(|| idcc_lus.iter().rev().flatten().next().cloned())
        .or_else(|| certificats.iter().rev().find_map(|c| c.aem.as_ref().and_then(|a| a.idcc.clone())));
    // La convention sous laquelle se resolvent les pieces d'un salarie : celle que son bulletin le plus recent imprime,
    // sinon celle de l'import.
    let convention_des = |rangs: &[usize]| rangs.iter().rev().find_map(|&k| idcc_lus[k].clone()).or_else(|| convention.clone());

    // Le sort des DSN et des AEM, avant que les bulletins ne completent les paies.
    let mut sorts_export: BTreeMap<usize, (Sort, String)> = BTreeMap::new();
    for (i, nom) in &noms {
        let f = &tri.fichiers[*i];
        if f.sorte == Sorte::Dsn {
            let mut n = 0usize;
            let mut nouveaux = 0usize;
            for x in ap.paies.iter().filter(|x| x.paie.fichier.as_ref().map(NomDeFichier::texte) == Some(nom.as_str())) {
                n += 1;
                if !hist_par_cle.contains_key(&x.cle()) {
                    nouveaux += 1;
                }
            }
            // Une DSN triee porte toujours son mois (`tri`) ; elle se nomme par lui.
            let dsn: String = match &f.mois {
                Some(m) => format!("DSN de {}", m),
                None => "DSN".to_string(),
            };
            sorts_export.insert(*i, match (n, nouveaux) {
                (0, _) => (Sort::Repris, format!("{} : aucun versement à reprendre (DSN sans versement, ou remplacée par une DSN du même mois déposée après)", dsn)),
                (n, 0) => (Sort::DejaRepris, format!("{} : ses {} versement(s) sont déjà dans l'historique", dsn, n)),
                (n, k) => (Sort::Repris, format!("{} : {} versement(s), dont {} à reprendre", dsn, n, k)),
            });
        } else {
            let (mut n, mut nouveaux, mut sans) = (0usize, 0usize, 0usize);
            for c in ap.contrats.iter().filter(|c| c.fichier.texte() == nom) {
                n += 1;
                if !par_nir.contains_key(&c.nir) && !nirs_dsn.contains(&c.nir) && !a_venir.contains(&c.nir) {
                    sans += 1;
                    continue;
                }
                if registre.pareils(&c.nir, c.debut, c.fin, "AEM").is_empty() {
                    registre.ajouter(&c.nir, import::dsn_aem::ligne_du_contrat(c, cle_de(&c.nir)), "AEM", "declarations");
                    nouveaux += 1;
                }
            }
            let sans_txt = if sans > 0 { format!(" ; {} sans fiche (NIR qu'aucune DSN ni aucun bulletin ne déclare)", sans) } else { String::new() };
            // `importation::lire_aem` garde un contrat une fois : un fichier qui ne rend rien a ete lu ailleurs, sauf
            // s'il est dans ses illisibles.
            let illisible =
                ap.reponse.avertissements.iter().any(|x| x.strip_prefix("AEM illisible, ignoree : ").is_some_and(|r| r.split(" : ").next() == Some(nom.as_str())));
            sorts_export.insert(*i, match (n, nouveaux) {
                (0, _) if illisible => (Sort::NonReconnu, "AEM dont les enregistrements ne se lisent pas".to_string()),
                (0, _) => (Sort::DejaRepris, "AEM : ses contrats attestés sont déjà dans un autre fichier du dossier".to_string()),
                (n, 0) if sans == 0 => (Sort::DejaRepris, format!("AEM : ses {} contrat(s) attesté(s) sont déjà dans l'historique", n)),
                (n, k) => (Sort::Repris, format!("AEM : {} contrat(s) attesté(s), dont {} nouveau(x){}", n, k, sans_txt)),
            });
        }
    }
    // Les DSN et AEM d'une autre entreprise : rien n'en a ete lu. Une AEM dont le Siret ne se lit pas passe, et le dit.
    for (i, raison) in ecartes {
        sorts_export.insert(i, (Sort::Incoherent, raison));
    }
    let mut avertissements_aem: Vec<String> = Vec::new();
    for (i, a) in &avertis {
        if let Some((_, raison)) = sorts_export.get_mut(i) {
            *raison = format!("{} ; {}", raison, a);
        }
        // Une AEM qui passe avec un avertissement est copiee : elle a son nom de copie.
        let nom: String = noms.get(i).iter().map(|n| format!(" : {}", n)).collect();
        avertissements_aem.push(format!("AEM reprise, mais {}{}", a, nom));
    }
    ap.reponse.avertissements.extend(avertissements_aem);

    let mut reponse = ap.reponse;
    let seuil = Mois::nouveau(ctx.aujourdhui.an - 1, ctx.aujourdhui.mois)?;
    let mut nouvelles_fiches = 0usize;
    let chemin_de = |l: &Lu| tri.fichiers[l.fichier].chemin.clone();

    // 5. Les salaries, par NIR : champ par champ, le bulletin le plus recent qui l'imprime.
    let mut par_salarie: BTreeMap<Nir, Vec<usize>> = BTreeMap::new();
    for (k, l) in lus.iter().enumerate() {
        par_salarie.entry(l.nir.clone()).or_default().push(k);
    }
    for (nir, rangs) in &par_salarie {
        let siens: Vec<&Lu> = rangs.iter().map(|&k| &lus[k]).collect();
        let proposees = fiche_salarie(nir, &siens, convention_des(rangs).as_ref())?;
        if let Some(cle) = par_nir.get(nir) {
            let fiche = &ctx.salaries[cle];
            let vue = proposee::de(NomDeFiche::Salarie, &fiche.champs)?;
            let ec = ecarts_salarie(&vue, &siens, &chemin_de);
            // Ce qui y est vide, de ce que les bulletins impriment (la fiche qu'ils creeraient) ; ce que la DSN complete
            // deja garde la valeur de la DSN (`completer::ajouter`).
            let champs = completer::champs_a_completer(NomDeFiche::Salarie, &vue, &proposees)?;
            let entree = fiche_existante(&mut reponse.salaries, &FicheNommee::Salarie(cle.clone()), || libelle_salarie(fiche));
            entree.ajouter_ecarts(ec);
            noter_par_champ(entree, &champs, &siens, &chemin_de)?;
            completer::ajouter(&mut ap.a_completer, FicheNommee::Salarie(cle.clone()), &champs);
            continue;
        }
        if nirs_dsn.contains(nir) {
            // LA DSN LA CREE, LE BULLETIN LA COMPLETE : ce que la DSN ne porte pas (le n° Congés Spectacles,
            // une adresse qu'elle n'a pas) vient du bulletin ; ce que la DSN donne reste.
            completer_a_creer(&mut ap.a_creer, &mut reponse, NomDeFiche::Salarie, Some(nir), &proposees)?;
            continue;
        }
        let anomalies = crate::importation::fiches::verifier_comme_a_l_ecriture(NomDeFiche::Salarie, &proposees)?;
        let libelle = libelle_propose(&proposees);
        reponse.salaries.push(FicheImportee::new(NomDeFiche::Salarie, None, &libelle, EtatFiche::ACreer, Vec::new(), anomalies, &proposees));
        ap.a_creer.push(ACreer { nom: NomDeFiche::Salarie, rang: Some(reponse.salaries.len() - 1), donnees: proposees });
        nouvelles_fiches += 1;
    }

    // 5b. Les salaries des certificats Congés Spectacles et des AEM en PDF : ils completent la fiche qui existe ou que les
    // DSN et les bulletins creent (ce qui y est vide), et creent celle d'un intermittent qu'aucune autre piece ne porte.
    let chemin_cert = |c: &Attestee| tri.fichiers[c.fichier].chemin.clone();
    let origine_cert = |c: &Attestee| Origine::piece(c.piece(), chemin_cert(c), c.c.page, c.mois());
    let mut par_salarie_cert: BTreeMap<Nir, Vec<usize>> = BTreeMap::new();
    for (k, c) in certificats.iter().enumerate() {
        if let Some(n) = &c.c.nir {
            par_salarie_cert.entry(n.clone()).or_default().push(k);
        }
    }
    // Les fiches que les pieces creent, et parmi elles celles des salaries, par sorte de piece (`Attestee::source`).
    let mut fiches_certificats: BTreeMap<&str, usize> = BTreeMap::new();
    let mut salaries_certificats: BTreeMap<&str, usize> = BTreeMap::new();
    for (nir, rangs) in &par_salarie_cert {
        let siens: Vec<&Attestee> = rangs.iter().map(|&k| &certificats[k]).collect();
        let dernier = siens[siens.len() - 1];
        // La convention de ses pieces : celle de son AEM la plus recente qui l'imprime, sinon celle de l'import.
        let convention_cert = siens.iter().rev().find_map(|c| c.aem.as_ref().and_then(|a| a.idcc.clone())).or_else(|| convention.clone());
        let proposees = fiche_de_certificats(nir, &siens, convention_cert.as_ref())?;
        if let Some(cle) = par_nir.get(nir) {
            let fiche = &ctx.salaries[cle];
            let champs = completer::champs_a_completer(NomDeFiche::Salarie, &proposee::de(NomDeFiche::Salarie, &fiche.champs)?, &proposees)?;
            let entree = fiche_existante(&mut reponse.salaries, &FicheNommee::Salarie(cle.clone()), || libelle_salarie(fiche));
            entree.noter(&champs, &origine_cert(dernier));
            completer::ajouter(&mut ap.a_completer, FicheNommee::Salarie(cle.clone()), &champs);
            continue;
        }
        if rang_a_creer(&ap.a_creer, NomDeFiche::Salarie, Some(nir))?.is_some() {
            completer_a_creer(&mut ap.a_creer, &mut reponse, NomDeFiche::Salarie, Some(nir), &proposees)?;
            continue;
        }
        let anomalies = crate::importation::fiches::verifier_comme_a_l_ecriture(NomDeFiche::Salarie, &proposees)?;
        let libelle = libelle_propose(&proposees);
        let mut entree = FicheImportee::new(NomDeFiche::Salarie, None, &libelle, EtatFiche::ACreer, Vec::new(), anomalies, &proposees);
        entree.lu_dans = Some(origine_cert(dernier));
        reponse.salaries.push(entree);
        ap.a_creer.push(ACreer { nom: NomDeFiche::Salarie, rang: Some(reponse.salaries.len() - 1), donnees: proposees });
        nouvelles_fiches += 1;
        *fiches_certificats.entry(dernier.source()).or_default() += 1;
        *salaries_certificats.entry(dernier.source()).or_default() += 1;
    }

    // 6. L'entreprise : creee des bulletins si ni la fiche ni une DSN ne la donne ; sinon ses ecarts.
    if ap.licences.is_empty() {
        ap.licences = licences_lues(&lus);
    }
    if let Some(dernier) = lus.iter().rev().find(|l| siret_lu(&l.b).is_some()).or(lus.last()) {
        let proposees = fiche_entreprise(&lus, dernier)?;
        if siren_fiche.is_some() {
            let vue = proposee::de(NomDeFiche::Entreprise, &ctx.entreprise)?;
            let ec = ecarts_entreprise(&vue, &proposees, dernier, &chemin_de(dernier));
            let entree = entreprise_existante(&mut reponse, ctx);
            entree.ajouter_ecarts(ec);
            let champs = completer::champs_a_completer(NomDeFiche::Entreprise, &vue, &proposees)?;
            entree.noter(&champs, &Origine::bulletin(Some(chemin_de(dernier)), Some(dernier.page)));
            completer::ajouter(&mut ap.a_completer, FicheNommee::Entreprise, &champs);
        } else if reponse.entreprise.as_ref().is_some_and(|e| e.etat == EtatFiche::ACreer) {
            // La fiche que les DSN creent, completee de ce que le bulletin imprime et qu'elles ne portent pas (3d).
            completer_a_creer(&mut ap.a_creer, &mut reponse, NomDeFiche::Entreprise, None, &proposees)?;
        } else if reponse.entreprise.is_none() {
            creer_l_entreprise(ctx, &mut ap.a_creer, &mut reponse, proposees, None)?;
            nouvelles_fiches += 1;
        }
    }

    // 6b. L'entreprise des certificats et des AEM : son numero d'adherent a la Caisse (« COMPTE »), que ni le bulletin ni la DSN
    // n'imprime toujours, son Siret et sa raison sociale ; comme pour un bulletin, la fiche qui existe garde ce qu'elle a.
    if let Some(dernier) = certificats.iter().rev().find(|c| c.c.siret.is_some()).or(certificats.last()) {
        let proposees = entreprise_du_certificat(dernier)?;
        if siren_fiche.is_some() {
            let mut ec = Vec::new();
            if let Some(compte) = &dernier.c.compte {
                let a = ctx.entreprise.congesspectacles_adherent.portee().map(documents::conges_spectacles::forme_adherent);
                if let Some(a) = a.filter(|a| a != compte) {
                    let (fichier, page, lu) = (chemin_cert(dernier), dernier.c.page, compte.clone());
                    let par = if dernier.aem.is_some() { DitPar::Aem { aem: lu, fichier, page } } else { DitPar::Certificat { certificat: lu, fichier, page } };
                    ec.push(Ecart::new(documents::conges_spectacles::CLE_ADHERENT, Valeur::Texte(a), par));
                }
            }
            let entree = entreprise_existante(&mut reponse, ctx);
            entree.ajouter_ecarts(ec);
            let champs = completer::champs_a_completer(NomDeFiche::Entreprise, &proposee::de(NomDeFiche::Entreprise, &ctx.entreprise)?, &proposees)?;
            entree.noter(&champs, &origine_cert(dernier));
            completer::ajouter(&mut ap.a_completer, FicheNommee::Entreprise, &champs);
        } else if ap.a_creer.iter().any(|a| a.nom == NomDeFiche::Entreprise) {
            completer_a_creer(&mut ap.a_creer, &mut reponse, NomDeFiche::Entreprise, None, &proposees)?;
        } else if reponse.entreprise.is_none() {
            creer_l_entreprise(ctx, &mut ap.a_creer, &mut reponse, proposees, Some(origine_cert(dernier)))?;
            nouvelles_fiches += 1;
            *fiches_certificats.entry(dernier.source()).or_default() += 1;
        }
    }

    // 7. Les spectacles, par numero d'objet, avec leur titre quand le bulletin le donne.
    let mut connus: HashMap<NumeroObjetSpectacle, droit::contexte::IdSpectacle> = HashMap::new();
    for (cle, sp) in &ctx.spectacles {
        if let Some(numero) = sp.numero_objet.dite() {
            connus.insert(numero.en_capitales(), cle.clone());
        }
    }
    let mut par_objet: BTreeMap<NumeroObjetSpectacle, Vec<usize>> = BTreeMap::new();
    for (k, l) in lus.iter().enumerate() {
        if let Some(o) = objet_imprime(&l.b) {
            par_objet.entry(o.en_capitales()).or_default().push(k);
        }
    }
    let mut objets: Vec<(NumeroObjetSpectacle, Vec<usize>)> = par_objet.into_iter().collect();
    objets.sort_by(|a, b| (lus[a.1[0]].b.fin, &a.0).cmp(&(lus[b.1[0]].b.fin, &b.0)));
    for (numero, rangs) in &objets {
        let titre = rangs.iter().rev().find_map(|&k| lus[k].b.spectacle.clone().filter(|t| !t.trim().is_empty()));
        let porteur = rangs.iter().rev().find(|&&k| lus[k].b.spectacle.as_ref().is_some_and(|t| !t.trim().is_empty())).map(|&k| &lus[k]);
        let analytique = rangs.iter().rev().find_map(|&k| lus[k].b.analytique.clone().filter(|t| !t.trim().is_empty()));
        // LE TITRE QUE L'IMPORT PROPOSE : celui de la ligne « Spectacle » du bulletin ; a defaut, la case « Analytique »,
        // ou un logiciel (sPAIEctacle) imprime le titre du spectacle. Il se propose, il ne s'ecrit pas : la fiche garde
        // « A COMPLETER » et « Pour démarrer » demande de le valider ou de le corriger (`declarations::demarrage`). La case
        // « Analytique » qui porte le titre n'est pas un code analytique.
        let propose: Option<(String, &'static str)> = match (&titre, &analytique) {
            (Some(t), _) => Some((t.clone(), "Spectacle")),
            (None, Some(a)) => Some((a.clone(), "Analytique")),
            (None, None) => None,
        };
        let analytique = analytique.filter(|_| titre.is_some());
        let proposition = |s: &mut Saisie| {
            if let Some((t, lu_dans)) = &propose {
                s.poser_note(declarations::demarrage::TITRE_PROPOSE, t.clone());
                s.poser_note(declarations::demarrage::TITRE_LU_DANS, lu_dans.to_string());
            }
        };
        #[derive(Serialize)]
        struct CodeAnalytique {
            code_analytique: String,
        }
        let code = |x: &String| proposee::de(NomDeFiche::Spectacle, &CodeAnalytique { code_analytique: x.clone() });
        if let Some(cle) = connus.get(numero) {
            let fiche = proposee::de(NomDeFiche::Spectacle, &ctx.spectacles[cle])?;
            let mut ec = Vec::new();
            // UN TITRE VIDE OU « A COMPLETER » (celui que l'import des DSN donne, la DSN ne portant pas le titre) recoit
            // le titre propose ; un vrai titre saisi reste, et l'ecart avec la ligne « Spectacle » est seulement signale.
            if let (Some(t), Some(p), Some(a)) = (&titre, porteur, fiche.mot(CaseSpectacle::Titre)) {
                if !completer::vide(fiche.case(CaseSpectacle::Titre)) && compare(&a) != compare(t) {
                    ec.push(Ecart::new(CaseSpectacle::Titre, Valeur::Texte(a), DitPar::Bulletin { bulletin: t.clone(), fichier: chemin_de(p), page: p.page }));
                }
            }
            // Ce que les bulletins proposent a la fiche : le code analytique qu'ils impriment, s'ils en impriment un.
            let mut proposees = Saisie::default();
            if let Some(x) = &analytique {
                proposees = code(x)?;
            }
            let mut champs = completer::champs_a_completer(NomDeFiche::Spectacle, &fiche, &proposees)?;
            // Le titre se propose a une fiche sans titre qui n'en a pas deja un a valider : comme toute case, la
            // proposition ne remplit qu'une case vide.
            if completer::vide(fiche.case(CaseSpectacle::Titre)) && fiche.note(declarations::demarrage::TITRE_PROPOSE).is_none_or(|t| t.trim().is_empty()) {
                proposition(&mut champs);
            }
            let origine = Origine::bulletin(porteur.map(chemin_de), porteur.map(|p| p.page));
            let titre_de_la_fiche = match fiche.mot(CaseSpectacle::Titre) {
                Some(t) => t,
                None => "Spectacle sans titre".to_string(),
            };
            let entree = fiche_existante(&mut reponse.spectacles, &FicheNommee::Spectacle(cle.clone()), || format!("{} ({})", titre_de_la_fiche, numero));
            entree.ajouter_ecarts(ec);
            entree.noter(&champs, &origine);
            completer::ajouter(&mut ap.a_completer, FicheNommee::Spectacle(cle.clone()), &champs);
            continue;
        }
        let deja_la = spectacle_du_numero(&reponse.spectacles, numero, true)?;
        if let Some(rang) = deja_la {
            // Un objet que les DSN ou les AEM creent : le bulletin lui propose son titre.
            let Some(a) = ap.a_creer.iter_mut().find(|a| a.nom == NomDeFiche::Spectacle && a.rang == Some(rang)) else { continue };
            proposition(&mut a.donnees);
            if let Some(x) = &analytique {
                proposee::completer_de(&mut a.donnees, &code(x)?);
            }
            let anomalies = crate::importation::fiches::verifier_comme_a_l_ecriture(NomDeFiche::Spectacle, &a.donnees)?;
            reponse.spectacles[rang].poser(&a.donnees, anomalies);
            continue;
        }
        // Un numero d'objet est lu sur un bulletin au moins : il a son premier et son dernier mois.
        let (Some(premier), Some(dernier)) = (rangs.iter().map(|&k| lus[k].mois).min(), rangs.iter().map(|&k| lus[k].mois).max()) else { continue };
        let sp = SpectacleDesPieces {
            titre: proposee::A_COMPLETER,
            numero_objet: numero.clone(),
            alias: Vec::new(),
            actif: dernier >= seuil,
            note: format!("bulletins de {} a {} ({} bulletin(s))", premier, dernier, rangs.len()),
            code_analytique: analytique.clone(),
            titre_propose: propose.as_ref().map(|p| p.0.clone()),
            titre_lu_dans: propose.as_ref().map(|p| p.1),
        };
        let donnees = proposee::de(NomDeFiche::Spectacle, &sp)?;
        let anomalies = crate::importation::fiches::verifier_comme_a_l_ecriture(NomDeFiche::Spectacle, &donnees)?;
        let libelle = match &propose {
            Some((t, _)) => format!("numéro d'objet {}, titre proposé « {} » (bulletins de {} à {})", numero, t, premier, dernier),
            None => format!("numéro d'objet {}, sans titre (bulletins de {} à {})", numero, premier, dernier),
        };
        reponse.spectacles.push(FicheImportee::new(NomDeFiche::Spectacle, None, &libelle, EtatFiche::ACreer, Vec::new(), anomalies, &donnees));
        ap.a_creer.push(ACreer { nom: NomDeFiche::Spectacle, rang: Some(reponse.spectacles.len() - 1), donnees });
        nouvelles_fiches += 1;
    }
    // 7b. Les numeros d'objet des AEM en PDF qu'aucune fiche, aucune DSN ni aucun bulletin ne connait : un spectacle a
    // creer, sans titre (l'AEM ne l'imprime pas), comme ceux des AEM deposees.
    let mut objets_aem: BTreeMap<NumeroObjetSpectacle, Vec<Mois>> = BTreeMap::new();
    for a in certificats.iter().filter_map(|c| c.aem.as_ref()) {
        if let (Some(o), Some(m)) = (&a.numero_objet, a.mois) {
            objets_aem.entry(o.en_capitales()).or_default().push(m);
        }
    }
    for (numero, mois) in objets_aem {
        let deja_la = spectacle_du_numero(&reponse.spectacles, &numero, false)?.is_some();
        let mut a_creer = false;
        for a in ap.a_creer.iter().filter(|a| a.nom == NomDeFiche::Spectacle) {
            if a.donnees.mot(CaseSpectacle::NumeroObjet).map(|n| NumeroObjetSpectacle::lire(&n)).transpose()?.is_some_and(|n| n.meme(&numero)) {
                a_creer = true;
                break;
            }
        }
        if connus.contains_key(&numero) || deja_la || a_creer {
            continue;
        }
        let (Some(premier), Some(dernier)) = (mois.iter().min().cloned(), mois.iter().max().cloned()) else { continue };
        let sp = SpectacleDesPieces {
            titre: proposee::A_COMPLETER,
            numero_objet: numero.clone(),
            alias: Vec::new(),
            actif: dernier >= seuil,
            note: format!("AEM de {} a {} ({} contrat(s))", premier, dernier, mois.len()),
            code_analytique: None,
            titre_propose: None,
            titre_lu_dans: None,
        };
        let donnees = proposee::de(NomDeFiche::Spectacle, &sp)?;
        let anomalies = crate::importation::fiches::verifier_comme_a_l_ecriture(NomDeFiche::Spectacle, &donnees)?;
        let libelle = format!("numéro d'objet {}, sans titre (AEM de {} à {})", numero, premier, dernier);
        reponse.spectacles.push(FicheImportee::new(NomDeFiche::Spectacle, None, &libelle, EtatFiche::ACreer, Vec::new(), anomalies, &donnees));
        ap.a_creer.push(ACreer { nom: NomDeFiche::Spectacle, rang: Some(reponse.spectacles.len() - 1), donnees });
        nouvelles_fiches += 1;
    }

    // 8. Les paies et les contrats, bulletin par bulletin, dans l'ordre du temps.
    // Les paies deja connues, par (NIR, debut, fin) : celles des DSN de cet apercu, puis celles de l'historique.
    #[derive(Clone, PartialEq)]
    enum Cible {
        Dsn(usize),
        Historique((Mois, Option<IdSalarie>, CleDePaieVersee)),
    }
    let mut index: HashMap<(Nir, Jour, Jour), Vec<(Mois, CleDePaieVersee, Cible)>> = HashMap::new();
    // LA PAIE D'UN CONTRAT QUI COURT AU-DELA DU MOIS (un permanent) : la DSN la date des bornes du contrat
    // (S21.G00.40.001 et .010, un CDI sans fin), le bulletin de la periode payee ; les dates ne la rapprochent donc pas.
    // Le numero du contrat (S21.G00.40.009, que le versement rappelle en S21.G00.51.010) et le mois, si : un bulletin qui
    // imprime son numero se rattache a la paie que la DSN du mois declare sous ce numero.
    let mut par_numero: HashMap<(Nir, Mois, NumeroContrat), Vec<(Mois, CleDePaieVersee, Cible)>> = HashMap::new();
    // Sans numero imprime, la regle de l'ecriture (`import::ecrite::rapprocher`, « un contrat a cheval sur deux mois ») :
    // le bulletin complete la paie declaree du meme mois dont le contrat contient sa periode, quand il n'y en a qu'une.
    // (debut, fin, contrat, cible) ; une fin absente : le contrat court encore.
    let mut par_mois: HashMap<(Nir, Mois), Vec<(Jour, Option<Jour>, CleDePaieVersee, Cible)>> = HashMap::new();
    let mut vus: HashSet<(Mois, Nir, CleDePaieVersee)> = HashSet::new();
    // (mois, NIR, contrat, debut, fin) d'une paie. Une paie sans NIR ne se rattache a aucun bulletin.
    let reperes = |x: &PaieLue| (x.mois, x.paie.nir.clone(), x.paie.contrat.clone(), x.paie.debut, x.paie.fin);
    for (k, x) in ap.paies.iter().enumerate() {
        let (mois, Some(nir), contrat, debut, fin) = reperes(x) else { continue };
        vus.insert((mois.clone(), nir.clone(), contrat.clone()));
        if let (Some(d), Some(f)) = (&debut, &fin) {
            index.entry((nir.clone(), *d, *f)).or_default().push((mois.clone(), contrat.clone(), Cible::Dsn(k)));
        }
        if let Some(n) = contrat.numero_declare() {
            par_numero.entry((nir.clone(), mois.clone(), n.clone())).or_default().push((mois.clone(), contrat.clone(), Cible::Dsn(k)));
        }
        if let Some(d) = debut {
            par_mois.entry((nir, mois)).or_default().push((d, fin, contrat, Cible::Dsn(k)));
        }
    }
    // Une paie que l'historique tient d'un bulletin seul s'efface devant la DSN qui la declare (la DSN est lue apres le
    // bulletin : l'ordre inverse) : le bulletin complete la ligne DSN, et l'ecriture retire la ligne « bulletin »
    // (`import::ecrite::rapprocher`).
    let mut de_bulletin: HashSet<(Mois, Option<IdSalarie>, CleDePaieVersee)> = HashSet::new();
    for x in &historique_paies {
        let (mois, Some(nir), contrat, debut, fin) = reperes(x) else { continue };
        if !vus.insert((mois.clone(), nir.clone(), contrat.clone())) {
            continue;
        }
        let cle = x.cle();
        if x.paie.contrat.d_un_bulletin() {
            de_bulletin.insert(cle.clone());
        }
        if let (Some(d), Some(f)) = (debut, fin) {
            index.entry((nir, d, f)).or_default().push((mois, contrat, Cible::Historique(cle)));
        }
    }
    let mut prises: HashMap<String, (String, usize, Decimal, Decimal)> = HashMap::new();
    let (mut paies_nouvelles, mut paies_completees) = (0usize, 0usize);
    let mut paies_liste: Vec<PaieLue> = Vec::new();
    let mut contrats_liste: Vec<ContratLu> = Vec::new();
    let mut pages_reprises = 0usize;
    let mut pages_incoherentes = 0usize;
    // LA CONVENTION DE REFERENCE, a laquelle chaque bulletin se compare : celle de la fiche, sinon celle de l'import.
    let mut bulletins_d_une_autre_convention = 0usize;
    for (rang, l) in lus.iter().enumerate() {
        let chemin = chemin_de(l);
        // Ce qui s'ecrit nomme la copie, pas le fichier d'origine (rien de personnel, le meme a l'apercu et a l'import). Un
        // PDF trie a son empreinte, donc sa copie.
        let copie = copie_lue(&tri.fichiers[l.fichier])?;
        let nb_pages = tri.fichiers[l.fichier].pages.len();
        let salarie = cle_de(&l.nir);
        let mut infos = infos_de(l, Some(&salarie));
        // UNE PAIE IMPORTEE DONT LA CONVENTION DIFFERE DE LA FICHE (ou de celle de l'import) : le bulletin le dit.
        if let (Some(lue), Some(reference)) = (&idcc_lus[rang], &convention) {
            if lue != reference {
                bulletins_d_une_autre_convention += 1;
                infos.avertissements.push(format!("le bulletin imprime la convention {}, la compagnie relève de la {}", lue, reference));
            }
        }
        let (debut, fin) = (l.b.debut, l.b.fin);
        let mut candidats: Vec<(Mois, CleDePaieVersee, Cible)> = index.get(&(l.nir.clone(), debut, fin)).into_iter().flatten().cloned().collect();
        if candidats.is_empty() {
            if let Some(n) = l.b.contrat.as_deref().and_then(NumeroContrat::d_une_piece) {
                candidats = par_numero.get(&(l.nir.clone(), l.mois, n)).into_iter().flatten().cloned().collect();
            }
        }
        if candidats.is_empty() {
            let contient: Vec<(Mois, CleDePaieVersee, Cible)> = par_mois
                .get(&(l.nir.clone(), l.mois))
                .into_iter()
                .flatten()
                .filter(|(d, f, _, _)| *d <= debut && f.as_ref().is_none_or(|f| fin <= *f))
                .map(|(_, _, contrat, cible)| (l.mois, contrat.clone(), cible.clone()))
                .collect();
            if contient.len() == 1 {
                candidats = contient;
            }
        }
        if candidats.iter().any(|c| matches!(c.2, Cible::Dsn(_))) {
            candidats.retain(|c| !matches!(&c.2, Cible::Historique(k) if de_bulletin.contains(k)));
        }
        if candidats.len() > 1 {
            let memes: Vec<(Mois, CleDePaieVersee, Cible)> = candidats.iter().filter(|c| c.0 == l.mois).cloned().collect();
            if memes.len() == 1 {
                candidats = memes;
            } else {
                let mois: Vec<String> = candidats.iter().map(|c| c.0.to_string()).collect();
                let raison = format!("plusieurs paies déclarées pour les mêmes dates ({}) : le bulletin ne se rattache à aucune", mois.join(", "));
                pages.entry(l.fichier).or_default().push(PageCr { page: l.page, sort: Sort::Incoherent, raison, infos });
                continue;
            }
        }
        let cible = candidats.into_iter().next();
        let ident = match &cible {
            Some((mois, contrat, _)) => format!("paie {} {} {}", mois, l.nir, contrat.dite()),
            None => format!("bulletin {} {} {}", l.nir, debut.iso(), fin.iso()),
        };
        if let Some((autre, page, brut, net)) = prises.get(&ident) {
            let (sort, raison) = if *brut == l.b.brut && *net == l.b.net_a_payer {
                (Sort::DejaRepris, format!("le même bulletin est déjà lu : {} page {}", autre, page))
            } else {
                (Sort::Incoherent, format!("un autre bulletin ({} page {}) donne d'autres montants pour la même paie : aucun des deux ne l'emporte", autre, page))
            };
            pages.entry(l.fichier).or_default().push(PageCr { page: l.page, sort, raison, infos });
            continue;
        }
        prises.insert(ident, (chemin.clone(), l.page, l.b.brut, l.b.net_a_payer));

        // La paie.
        let (mut ligne, source, avant): (PaieLue, &str, Option<PaieLue>) = match &cible {
            Some((_, _, Cible::Dsn(k))) => {
                let x = ap.paies[*k].clone();
                let avant = hist_par_cle.get(&x.cle()).cloned();
                (x, "dsn", avant)
            }
            Some((_, _, Cible::Historique(c))) => {
                let x = hist_par_cle[c].clone();
                (x.clone(), "historique", Some(x))
            }
            None => {
                let contrat = match &l.b.numero {
                    Some(n) => CleDePaieVersee::Bulletin(n.clone()),
                    None => CleDePaieVersee::Copie { fichier: copie.clone(), page: (nb_pages > 1).then_some(l.page as u64) },
                };
                let mut paie = PaieVersee::vide(contrat);
                (paie.nir, paie.debut, paie.fin, paie.fichier) = (Some(l.nir.clone()), Some(l.b.debut), Some(l.b.fin), Some(copie.clone()));
                let x = PaieLue { mois: l.mois, salarie: salarie.clone(), paie };
                let avant = hist_par_cle.get(&x.cle()).cloned();
                (x, "bulletin", avant)
            }
        };
        let ecarts = completer(&mut ligne.paie, &l.b, &copie, l.page, convention_des(&[rang]).as_ref())?;
        // Le contrat que le bulletin atteste ; celui d'un permanent ne compte que son mois : ses heures, ses cachets et
        // ses jours sont ceux de la paie de ce mois (le contrat entier n'en garde pas : `import::ecrite`).
        let atteste = l.compteurs.as_ref().map_err(|m| m.clone().reformuler(|p| format!("ses compteurs ne se lisent pas ({})", p))).and_then(|c| engagement(l, c, &salarie, &copie));
        if let (Ok((compte, _)), false) = (&atteste, marque_d_intermittent(&l.b)) {
            (ligne.paie.heures, ligne.paie.cachets, ligne.paie.jours) = (Some(compte.heures), Some(compte.cachets), Some(compte.jours));
        }
        let etat_paie = if avant.as_ref().map(PaieLue::sans_date_d_import) == Some(ligne.sans_date_d_import()) {
            EtatPaie::Deja
        } else if source == "bulletin" {
            EtatPaie::Nouvelle
        } else {
            EtatPaie::Completee
        };
        match &cible {
            Some((_, _, Cible::Dsn(k))) => ap.paies[*k] = ligne.clone(),
            _ if etat_paie != EtatPaie::Deja => ap.paies.push(ligne.clone()),
            _ => {}
        }
        if etat_paie != EtatPaie::Deja {
            paies_liste.push(ligne.clone());
            if etat_paie == EtatPaie::Nouvelle {
                paies_nouvelles += 1;
            } else {
                paies_completees += 1;
            }
        }
        let mut phrases: Vec<String> = vec![match (etat_paie, source) {
            (EtatPaie::Deja, _) => "paie déjà dans l'historique".to_string(),
            (EtatPaie::Nouvelle, _) => "paie nouvelle : aucune DSN ne la déclare".to_string(),
            (_, "dsn") => format!("complète la paie déclarée en DSN ({})", ligne.mois),
            _ => format!("complète la paie de l'historique ({})", ligne.mois),
        }];
        if !ecarts.is_empty() {
            let gagnants: Vec<&str> = ecarts.iter().filter(|e| e.2).map(|e| e.0).collect();
            let notes: Vec<&str> = ecarts.iter().filter(|e| !e.2).map(|e| e.0).collect();
            if !gagnants.is_empty() {
                phrases.push(format!("le bulletin l'emporte sur : {}", gagnants.iter().map(|c| libelle_champ(c)).collect::<Vec<_>>().join(", ")));
            }
            if !notes.is_empty() {
                phrases.push(format!("écart noté, non appliqué : {}", notes.iter().map(|c| libelle_champ(c)).collect::<Vec<_>>().join(", ")));
            }
            infos.ecarts = ecarts.into_iter().map(|(champ, avant, gagne, apres)| EcartPaie::new(champ, source, avant, gagne, apres)).collect();
        }
        infos.paie = Some(etat_paie);

        // Le contrat atteste.
        let etat_contrat = match atteste {
            Err(m) => {
                phrases.push(format!("contrat non repris : {}", m.phrase));
                EtatContrat::Inconnu
            }
            Ok((compte, ligne_contrat)) => {
                // LE BULLETIN EST LA PIECE DE PAIE : un contrat d'AEM aux memes dates qui compte autre chose
                // s'efface devant le sien (`import::attestes`) ; un autre contrat atteste reste, et l'ecart se dit.
                let pareils = registre.pareils(&l.nir, l.b.debut, l.b.fin, "bulletin");
                let le_sien = (Some(compte.heures), Some(compte.cachets), Some(compte.jours), Some(compte.brut));
                let en_mots = import::attestes::en_mots(le_sien.0, le_sien.1, le_sien.2, le_sien.3);
                if pareils.iter().any(|a| a.compte() == le_sien) {
                    phrases.push("contrat déjà attesté".into());
                    EtatContrat::Deja
                } else if let Some(a) = pareils.iter().find(|a| a.par != Attestation::Aem) {
                    phrases.push(format!(
                        "un contrat attesté aux mêmes dates ({}) porte d'autres compteurs : il est gardé, celui du bulletin n'est pas ajouté (le contrat attesté dit {}, le bulletin {})",
                        a.d_ou(),
                        a.en_mots(),
                        en_mots
                    ));
                    infos.contrat_bulletin = Some(ligne_contrat);
                    EtatContrat::Ecart
                } else {
                    phrases.push("contrat attesté ajouté".into());
                    contrats_liste.push(ligne_contrat.clone());
                    phrases.extend(registre.ajouter(&l.nir, ligne_contrat, "bulletin", "bulletins"));
                    EtatContrat::Nouveau
                }
            }
        };
        infos.contrat = Some(etat_contrat);
        // Les avertissements des compteurs lus ; des compteurs qui ne se lisent pas n'en ont pas : leur refus, le contrat
        // atteste le dit (`atteste`, plus haut).
        infos.compteurs = l.compteurs.iter().flat_map(|c| c.avertissements.iter().cloned()).collect();
        let inconnus: Vec<String> = l
            .b
            .avertissements
            .iter()
            .filter_map(|a| a.strip_prefix("libellé inconnu : ").map(String::from))
            .collect();
        let sort = if etat_paie == EtatPaie::Deja && etat_contrat != EtatContrat::Nouveau {
            Sort::DejaRepris
        } else if !inconnus.is_empty() {
            phrases.push(format!("ligne(s) sans rubrique du catalogue, gardée(s) telle(s) quelle(s) : {}", inconnus.join(", ")));
            Sort::LibelleInconnu
        } else {
            Sort::Repris
        };
        if sort != Sort::DejaRepris {
            pages_reprises += 1;
            if !l.incoherences.is_empty() {
                pages_incoherentes += 1;
            }
        }
        if !l.incoherences.is_empty() {
            phrases.push(format!("le bulletin fait foi, ses montants imprimés sont repris malgré : {}", l.incoherences.join(" ; ")));
            infos.avertissements.extend(l.incoherences.iter().cloned());
        }
        pages.entry(l.fichier).or_default().push(PageCr { page: l.page, sort, raison: phrases.join(" ; "), infos });
    }

    // 8b. Les contrats des certificats Congés Spectacles et des AEM en PDF, les AEM d'abord (elles disent les heures, les
    // cachets et les jours a part, et le brut au centime). Le registre dit si le contrat est deja atteste
    // (`import::attestes`) : la piece le confirme alors, n'ajoute rien, et l'ecart de brut, de jours ou de cachets se
    // note. Une paie declaree aux memes dates, sans contrat atteste (une DSN seule), recoit le sien ; une paie declaree
    // a d'autres dates qui les chevauchent ne se rattacherait a aucun contrat : celui de la piece n'est pas ajoute.
    let declarees_dates: Vec<(Nir, Option<Jour>, Option<Jour>)> =
        ap.paies.iter().chain(historique_paies.iter()).filter_map(|x| Some((x.paie.nir.clone()?, x.paie.debut, x.paie.fin))).collect();
    let chevauche = import::attestes::chevauche;
    // Un compte ou une date d'une paie, dans une phrase : dite telle quand la piece ne la dit pas.
    let dit = |x: Option<String>| match x {
        Some(t) => t,
        None => "(non dit)".to_string(),
    };
    let (mut certificats_confirmes, mut aem_confirmees) = (0usize, 0usize);
    let ordre: Vec<&Attestee> = certificats.iter().filter(|c| c.aem.is_some()).chain(certificats.iter().filter(|c| c.aem.is_none())).collect();
    for cl in ordre {
        let c = &cl.c;
        let est_aem = cl.aem.is_some();
        let (Some(nir), Some(debut), Some(fin), Some(brut)) = (&c.nir, c.debut, c.fin, c.brut) else { continue };
        let nom = cl.nom();
        let du_au = format!("du {} au {}", debut.jj_mm_aaaa(), fin.jj_mm_aaaa());
        let mut infos = InfosPage { debut: Some(debut), fin: Some(fin), mois: cl.mois(), ..InfosPage::default() };
        if est_aem {
            infos.aem = c.numero.clone();
        } else {
            infos.certificat = c.numero.clone();
        }
        let pareils = registre.pareils(nir, debut, fin, cl.source());
        // UN CERTIFICAT DONT D'AUTRES PIECES N'ATTESTENT QUE DES MOIS (un bulletin, une AEM, chacun le sien) atteste seul
        // ses autres mois (`attestes::reste_du_certificat`) ; un certificat deja repris ne se reprend pas deux fois.
        let mut reste: Option<ContratLu> = None;
        let mut sans_reste: Option<String> = None;
        // Le nom de la copie de la piece ; un PDF trie a son empreinte, donc sa copie.
        let copie = copie_lue(&tri.fichiers[cl.fichier])?;
        if !est_aem && !pareils.is_empty() && !pareils.iter().any(Atteste::de_certificat) {
            let statut = statut_de(ctx, &par_nir, &ap.a_creer, nir, c, convention.as_ref())?;
            match contrat_du_certificat(c, statut.as_deref(), cle_de(nir), &copie) {
                Ok((ligne, _)) => match registre.reste_du_certificat(nir, &ligne, &pareils) {
                    Reste::Rien => {}
                    Reste::Seul(l) => reste = Some(l),
                    Reste::NeSeComptePas(m) => sans_reste = Some(m),
                },
                Err(m) => sans_reste = Some(m.phrase),
            }
        }
        if !pareils.is_empty() && reste.is_none() {
            if est_aem {
                aem_confirmees += 1;
            } else {
                certificats_confirmes += 1;
            }
            let mut sources: Vec<String> = pareils.iter().map(Atteste::d_ou).collect();
            sources.dedup();
            let mut phrases = vec![format!("{} {} : le contrat est déjà attesté ({}), rien n'est ajouté", nom, du_au, sources.join(", "))];
            phrases.extend(sans_reste);
            // L'ecart, quand les contrats confirmes tiennent dans les dates de la piece (un certificat coupe au 31 mars
            // n'en porte qu'une part).
            if pareils.iter().all(|a| a.tient_dans(debut, fin)) {
                let somme: Option<Decimal> = pareils.iter().map(|a| a.brut).sum();
                let (cachets, jours): (Option<i64>, Option<i64>) = (pareils.iter().map(|a| a.cachets).sum(), pareils.iter().map(|a| a.jours).sum());
                let n = |x: Option<i64>| dit(x.map(|v| v.to_string()));
                match &cl.aem {
                    Some(a) => {
                        if let Some(s) = somme.filter(|s| *s != brut) {
                            phrases.push(format!("écart noté : l'{} dit {} de brut, les contrats {}", cl.nom(), euros(brut), euros(s)));
                        }
                        // Les jours, l'AEM ne les dit que si sa colonne se lit : seuls ceux qu'elle dit se comparent.
                        if Some(a.cachets()) != cachets || a.jours.is_some_and(|j| Some(j) != jours) {
                            phrases.push(format!(
                                "écart noté : l'{} dit {} cachets et {} jours, les contrats {} cachets et {} jours",
                                cl.nom(),
                                a.cachets(),
                                n(a.jours),
                                n(cachets),
                                n(jours)
                            ));
                        }
                    }
                    None => {
                        if let Some(s) = somme {
                            if Decimal::from(documents::conges_spectacles::arrondi_euro(s)?) != brut {
                                phrases.push(format!("écart noté : le certificat dit {} de brut, les contrats {}", euros(brut), euros(s)));
                            }
                        }
                        if let Some(nombre) = c.nombre.filter(|x| Some(*x) != cachets && Some(*x) != jours) {
                            phrases.push(format!("écart noté : le certificat dit {} jours ou cachets, les contrats {} cachets et {} jours", nombre, n(cachets), n(jours)));
                        }
                    }
                }
            }
            infos.contrat = Some(EtatContrat::Deja);
            pages.entry(cl.fichier).or_default().push(PageCr { page: c.page, sort: Sort::DejaRepris, raison: phrases.join(" ; "), infos });
            continue;
        }
        // Le reste d'un certificat : ses dates et ses compteurs sont ceux du reste.
        // Le reste d'un certificat porte toujours ses dates (`attestes::reste_du_certificat`).
        let (d, f, du_au) = match reste.as_ref().map(|l| (l.debut, l.fin)) {
            Some((Some(rd), Some(rf))) => {
                let mut sources: Vec<String> = pareils.iter().map(Atteste::d_ou).collect();
                sources.dedup();
                let du_au = format!("{} (attesté ailleurs pour ses autres mois : {}) : le reste, du {} au {},", du_au, sources.join(", "), rd.jj_mm_aaaa(), rf.jj_mm_aaaa());
                (rd, rf, du_au)
            }
            _ => (debut, fin, du_au),
        };
        let autres: Vec<String> = declarees_dates
            .iter()
            .filter(|p| &p.0 == nir && chevauche(p.1, p.2, d, f) && !(p.1 == Some(d) && p.2 == Some(f)))
            .map(|p| format!("du {} au {}", dit(p.1.map(|j| j.iso())), dit(p.2.map(|j| j.iso()))))
            .collect();
        if !autres.is_empty() {
            let raison = format!(
                "{} {} : une paie déclarée de ce salarié couvre d'autres dates ({}) ; le contrat de la pièce ne se rattacherait à aucune, il n'est pas ajouté",
                nom,
                du_au,
                autres.join(", ")
            );
            infos.contrat = Some(EtatContrat::Ecart);
            pages.entry(cl.fichier).or_default().push(PageCr { page: c.page, sort: Sort::DejaRepris, raison, infos });
            continue;
        }
        let est_reste = reste.is_some();
        let lu = match (&cl.aem, reste) {
            (Some(a), _) => contrat_de_l_aem(a, cle_de(nir), &copie),
            (None, Some(l)) => {
                let quoi = match (l.contrat.cachets, l.contrat.jours) {
                    (Some(n), _) if n > 0 => format!("{} cachet(s)", n),
                    (_, Some(n)) => format!("{} jour(s)", n),
                    _ => "ni cachet ni jour".to_string(),
                };
                let brut_reste: String = l.contrat.brut.map(|b| format!(", {} de brut", euros(b))).into_iter().collect();
                Ok((l, format!("{}{} : ce que dit le certificat moins ce que disent les autres pièces ; son brut, {}, est arrondi à l'euro", quoi, brut_reste, euros(brut))))
            }
            (None, None) => {
                let statut = statut_de(ctx, &par_nir, &ap.a_creer, nir, c, convention.as_ref())?;
                contrat_du_certificat(c, statut.as_deref(), cle_de(nir), &copie)
            }
        };
        match lu {
            Err(raison) => {
                infos.contrat = Some(EtatContrat::Inconnu);
                pages.entry(cl.fichier).or_default().push(PageCr { page: c.page, sort: Sort::Incomplet, raison: format!("{} {} : {}", nom, du_au, raison.phrase), infos });
            }
            Ok((ligne, quoi)) => {
                infos.contrat = Some(EtatContrat::Nouveau);
                let raison = if est_aem {
                    format!("{} {} : contrat attesté ajouté ({}, {} de brut)", nom, du_au, quoi, euros(brut))
                } else if est_reste {
                    format!("certificat Congés Spectacles {} contrat attesté ajouté ({}) ; les heures, que le certificat ne porte pas, n'y sont pas", du_au, quoi)
                } else {
                    format!("certificat Congés Spectacles {} : contrat attesté ajouté ({}, {} de brut arrondi à l'euro) ; les heures, que le certificat ne porte pas, n'y sont pas", du_au, quoi, euros(brut))
                };
                let bloc = if est_aem { "aem_pdf" } else { "certificats" };
                let raison = std::iter::once(raison).chain(registre.ajouter(nir, ligne, cl.source(), bloc)).collect::<Vec<_>>().join(" ; ");
                pages.entry(cl.fichier).or_default().push(PageCr { page: c.page, sort: Sort::Repris, raison, infos });
            }
        }
    }

    // 9. Le compte rendu, fichier par fichier.
    let mut fichiers: Vec<FichierVu> = Vec::new();
    let mut copies: BTreeMap<usize, NomDeFichier> = BTreeMap::new();
    let mut comptes = ParSort::a_zero();
    for (i, f) in tri.fichiers.iter().enumerate() {
        // Ce que le tri dit du fichier ; ce qu'il en dit sans raison, sa sorte.
        let tri_raison = match &f.raison {
            Some(r) => r.clone(),
            None => format!("fichier {}", f.sorte.nom()),
        };
        let mut ps: Vec<PageVu> = Vec::new();
        let (sort, raison): (Sort, String) = match f.sorte {
            // Chaque DSN et chaque AEM du dossier a son sort (`sorts_export`).
            Sorte::Dsn | Sorte::Aem => match sorts_export.get(&i) {
                Some(s) => s.clone(),
                None => (Sort::NonReconnu, format!("{} que l'import n'a pas lue", f.sorte.nom())),
            },
            Sorte::PdfSansTexte => (Sort::SansTexte, tri_raison),
            Sorte::Archive => (Sort::Archive, tri_raison),
            Sorte::Doublon => (Sort::Doublon, tri_raison),
            Sorte::Illisible | Sorte::Autre => (Sort::NonReconnu, tri_raison),
            Sorte::PdfTexte => match illisibles.get(&i) {
                Some(m) => (Sort::NonReconnu, m.clone()),
                None => {
                    let mut lues: Vec<PageCr> = pages.remove(&i).into_iter().flatten().collect();
                    lues.sort_by_key(|p| p.page);
                    // Un PDF sans page lue n'est pas reconnu.
                    let sort = match Sort::GRAVITE.iter().find(|g| lues.iter().any(|p| p.sort == **g)) {
                        Some(g) => *g,
                        None => Sort::NonReconnu,
                    };
                    let raison = match lues.as_slice() {
                        [] => "PDF sans page".to_string(),
                        [p] => p.raison.clone(),
                        _ => {
                            let par: Vec<String> = Sort::GRAVITE
                                .iter()
                                .filter_map(|g| Some(lues.iter().filter(|p| p.sort == *g).count()).filter(|n| *n > 0).map(|n| format!("{} {}", n, g.en_mots().1)))
                                .collect();
                            format!("{} pages : {}", lues.len(), par.join(", "))
                        }
                    };
                    ps = lues.into_iter().map(|p| PageVu::new(p.page, p.sort, p.raison, p.infos)).collect();
                    (sort, raison)
                }
            },
        };
        // Ce que l'import copie dans `historique/bulletins/` : une DSN ou une AEM lue, un PDF dont une page au moins est
        // reprise (ou l'a deja ete).
        let repris = match f.sorte {
            Sorte::Dsn | Sorte::Aem => matches!(sort, Sort::Repris | Sort::DejaRepris),
            Sorte::PdfTexte => ps.iter().any(|p| matches!(p.sort, Sort::Repris | Sort::LibelleInconnu | Sort::DejaRepris)),
            Sorte::PdfSansTexte | Sorte::Archive | Sorte::Doublon | Sorte::Illisible | Sorte::Autre => false,
        };
        let copie = nom_copie(f)?.filter(|_| repris);
        if let Some(c) = &copie {
            copies.insert(i, c.clone());
        }
        *comptes.de(sort) += 1;
        fichiers.push(FichierVu {
            chemin: f.chemin.clone(),
            sorte: f.sorte.nom(),
            pages: ps,
            sort,
            raison,
            copie,
            doublon_de: f.doublon_de.clone(),
            archive: f.archive.clone(),
            mois: f.mois.clone(),
            de_dit: None,
            pages_dit: String::new(),
        });
    }
    // CE QUI N'EST PAS REPRIS, pour l'encart : chaque fichier et sa raison ; un import qui en laisse se rejoue.
    let non_repris: Vec<NonReprisVu> =
        fichiers.iter().filter(|f| f.sort.non_repris()).map(|f| NonReprisVu { chemin: f.chemin.clone(), sort: f.sort, raison: f.raison.clone() }).collect();
    reponse.tri = TriVu {
        dossier: tri.dossier.clone(),
        fichiers,
        comptes,
        total: tri.fichiers.len(),
        non_repris: (!non_repris.is_empty()).then_some(NonRepris { fichiers: non_repris, phrase: REESSAYER }),
        dit: String::new(),
    };
    reponse.bulletins = BlocBulletins {
        pages: n_pages,
        lus: par_logiciel.values().sum(),
        repris: pages_reprises,
        par_logiciel,
        du,
        au,
        fiches_a_creer: nouvelles_fiches,
        contrats_nouveaux: contrats_liste.len(),
        contrats: contrats_liste,
        paies_nouvelles,
        paies_completees,
        paies: paies_liste,
        dit: None,
    };
    // LES CERTIFICATS ET LES AEM EN PDF, chacun son bloc : combien, de quand a quand, ce qu'ils ajoutent ; et, pour une
    // fiche qu'ils creent, ce qu'aucune de ces pieces ne porte et qui reste a saisir (la fiche dit elle-meme ses cases
    // obligatoires vides). L'AEM dit la date de naissance ; le certificat, non.
    let certificat_tait = vec![
        "le compte bancaire",
        "le taux de prélèvement à la source",
        "la date et la commune de naissance (le NIR ne dit que l'année, le mois et le département)",
    ];
    let aem_tait = vec!["le compte bancaire", "le taux de prélèvement à la source", "la commune de naissance"];
    for (bloc, source, ajoutes, confirmes, tait) in [
        ("certificats", "certificat", registre.lignes("certificats"), certificats_confirmes, certificat_tait),
        ("aem_pdf", "AEM", registre.lignes("aem_pdf"), aem_confirmees, aem_tait),
    ] {
        let siennes: Vec<&Attestee> = certificats.iter().filter(|c| c.source() == source).collect();
        if siennes.is_empty() {
            continue;
        }
        // Ce que la piece tait reste a saisir dans chaque fiche qu'elle a creee ; aucune fiche creee, rien.
        let reste: Vec<&'static str> = if salaries_certificats.get(source).is_some_and(|n| *n > 0) { tait } else { Vec::new() };
        let lu = BlocPieces {
            lus: siennes.len(),
            du: siennes.iter().filter_map(|c| c.c.debut).min(),
            au: siennes.iter().filter_map(|c| c.c.fin).max(),
            contrats_nouveaux: ajoutes.len(),
            contrats: ajoutes,
            confirmes,
            fiches_a_creer: fiches_certificats.iter().filter(|(s, _)| **s == source).map(|(_, n)| *n).sum(),
            reste_a_saisir: reste,
            dit: String::new(),
        };
        if bloc == "certificats" {
            reponse.certificats = Some(lu);
        } else {
            reponse.aem_pdf = Some(lu);
        }
    }
    if bulletins_d_une_autre_convention > 0 {
        let reference: String = convention.iter().map(|c| format!(" ({})", c)).collect();
        reponse.avertissements.push(format!(
            "{} bulletin(s) impriment une autre convention collective que celle de la compagnie{} : la fiche Entreprise n'en porte qu'une, à vérifier ; le compte rendu dit lesquels, page par page.",
            bulletins_d_une_autre_convention, reference
        ));
    }
    if pages_incoherentes > 0 {
        reponse.avertissements.push(format!(
            "{} bulletin(s) ne retombent pas sur leurs propres totaux (brut, net) : le bulletin fait foi, ses montants imprimés sont repris tels quels ; le compte rendu dit l'écart, page par page.",
            pages_incoherentes
        ));
    }
    // UN CONTRAT QUE LA DSN DECLARE SANS SES NETS (`routes::import::SansNets`) : son bulletin, lu ici ou deja
    // repris, les donne ; sinon l'apercu le dit, contrat par contrat.
    for s in &ap.sans_nets {
        let ligne = ap.paies.iter().find(|x| x.mois == s.mois && x.paie.nir == s.nir && x.paie.contrat.numero_declare() == Some(&s.contrat));
        let dates = ligne.map(|x| (x.paie.debut, x.paie.fin));
        let complete = ligne.is_some_and(|x| x.paie.bulletin.is_some())
            || historique_paies.iter().any(|h| {
                h.mois == s.mois
                    && h.paie.nir == s.nir
                    && h.paie.bulletin.is_some()
                    && (h.paie.contrat.numero_declare() == Some(&s.contrat) || (h.paie.contrat.d_un_bulletin() && Some((h.paie.debut, h.paie.fin)) == dates))
            });
        if !complete {
            reponse.avertissements.push(s.phrase());
        }
    }
    reponse.contrats_nouveaux = registre.nombre("declarations");
    reponse.rien_a_faire = reponse.rien_a_faire
        && nouvelles_fiches == 0
        && ap.a_completer.is_empty()
        && registre.retenus.is_empty()
        && paies_nouvelles == 0
        && paies_completees == 0;
    reponse.avertissements.extend(registre.muets.iter().cloned());
    ap.contrats_attestes = registre.retenus;
    ap.contrats_changes = registre.changes;
    ap.reponse = reponse;
    Ok(DossierLu { ap, tri, copies })
}

/// Ce que la page dit d'elle-meme dans le compte rendu.
fn infos_de(l: &Lu, salarie: Option<&Option<IdSalarie>>) -> InfosPage {
    InfosPage {
        logiciel: l.b.logiciel.clone(),
        numero: l.b.numero.clone(),
        salarie: salarie.cloned(),
        debut: Some(l.b.debut),
        fin: Some(l.b.fin),
        mois: Some(l.mois),
        ..InfosPage::default()
    }
}

/// Le rang dans `a_creer` de la fiche `nom` que l'import cree pour le NIR `nir` ; sans NIR, la premiere de ce nom.
fn rang_a_creer(a_creer: &[ACreer], nom: NomDeFiche, nir: Option<&Nir>) -> R<Option<usize>> {
    for (i, a) in a_creer.iter().enumerate().filter(|(_, a)| a.nom == nom) {
        match nir {
            None => return Ok(Some(i)),
            Some(n) => {
                if a.nir()?.as_ref() == Some(n) {
                    return Ok(Some(i));
                }
            }
        }
    }
    Ok(None)
}

/// Une fiche que l'import cree des DSN (`a_creer` ; un salarie par son NIR, ou l'entreprise), completee de ce que les
/// bulletins impriment et qu'elle n'a pas ; ses anomalies se refont, et la reponse montre la fiche completee.
fn completer_a_creer(a_creer: &mut [ACreer], reponse: &mut Importation, nom: NomDeFiche, nir: Option<&Nir>, proposees: &Saisie) -> R<()> {
    let Some(i) = rang_a_creer(a_creer, nom, nir)? else {
        return Ok(());
    };
    let a = &mut a_creer[i];
    let mut champs = completer::champs_a_completer(nom, &a.donnees, proposees)?;
    proposee::fondre(&mut champs, &lisibles_de_la_piece_sur(nom, &a.donnees, proposees)?);
    if !champs.est_dite() {
        return Ok(());
    }
    proposee::fondre(&mut a.donnees, &champs);
    let anomalies = crate::importation::fiches::verifier_comme_a_l_ecriture(nom, &a.donnees)?;
    let entree = match (nom, a.rang) {
        (NomDeFiche::Entreprise, _) => reponse.entreprise.as_mut(),
        (_, Some(i)) => reponse.salaries.get_mut(i),
        _ => None,
    };
    if let Some(e) = entree {
        e.poser(&a.donnees, anomalies);
    }
    Ok(())
}

/// L'entreprise que l'import cree des pieces (`origine` : la piece qui la donne, quand ce n'est ni une DSN ni un
/// bulletin), sous le nom que la personne lui a donne.
fn creer_l_entreprise(ctx: &Contexte, a_creer: &mut Vec<ACreer>, reponse: &mut Importation, mut e: Saisie, origine: Option<Origine>) -> R<()> {
    ri::garder_le_nom(ctx, &mut e)?;
    let anomalies = crate::importation::fiches::verifier_comme_a_l_ecriture(NomDeFiche::Entreprise, &e)?;
    let libelle = match e.mot(droit::fiche::CaseEntreprise::RaisonSociale) {
        Some(r) => r,
        None => "L'entreprise, sans raison sociale".to_string(),
    };
    let mut entree = FicheImportee::new(NomDeFiche::Entreprise, None, &libelle, EtatFiche::ACreer, Vec::new(), anomalies, &e);
    entree.lu_dans = origine;
    reponse.entreprise = Some(entree);
    a_creer.push(ACreer { nom: NomDeFiche::Entreprise, rang: None, donnees: e });
    Ok(())
}

/// Le libelle d'un salarie qu'une fiche proposee nomme : son nom, puis son prenom en capitales initiales.
fn libelle_propose(f: &Saisie) -> String {
    [f.mot(CaseSalarie::Nom), ri::titre_de(f.mot(CaseSalarie::Prenom).as_deref())].into_iter().flatten().collect::<Vec<_>>().join(" ")
}

/// La fiche existante `cle` de la liste, ajoutee (`libelle`) si la reponse ne la porte pas encore.
fn fiche_existante<'a>(liste: &'a mut Vec<FicheImportee>, fiche: &FicheNommee, libelle: impl FnOnce() -> String) -> &'a mut FicheImportee {
    let rang = match liste.iter().position(|e| e.cle.as_deref() == fiche.cle()) {
        Some(i) => i,
        None => {
            liste.push(FicheImportee::new(fiche.nom(), fiche.cle(), &libelle(), EtatFiche::Existante, Vec::new(), Vec::new(), &Saisie::default()));
            liste.len() - 1
        }
    };
    &mut liste[rang]
}

/// La fiche Entreprise existante de la reponse, ajoutee si elle ne la porte pas encore, sous sa raison sociale.
fn entreprise_existante<'a>(reponse: &'a mut Importation, ctx: &Contexte) -> &'a mut FicheImportee {
    reponse.entreprise.get_or_insert_with(|| {
        let libelle = match ctx.entreprise.raison_sociale.portee() {
            Some(r) => r,
            None => "L'entreprise, sans raison sociale",
        };
        FicheImportee::new(NomDeFiche::Entreprise, None, libelle, EtatFiche::Existante, Vec::new(), Vec::new(), &Saisie::default())
    })
}

/// Le libelle d'une fiche salarie existante : son nom, puis son prenom en capitales initiales, ce que la fiche dit.
fn libelle_salarie(fiche: &droit::fiche::ChampsSalarie) -> String {
    [fiche.nom.en_texte(), fiche.prenom.portee().map(declarations::champs::titre)].into_iter().flatten().collect::<Vec<_>>().join(" ")
}

/// Note dans l'entree d'un salarie ce que ses bulletins completent, chaque case avec le bulletin le plus recent qui
/// l'imprime (`identite`) ; l'adresse, le statut, l'anciennete et la cle du NIR, avec le plus recent.
fn noter_par_champ(entree: &mut FicheImportee, champs: &Saisie, ls: &[&Lu], chemin: &dyn Fn(&Lu) -> String) -> R<()> {
    let Some(dernier) = ls.last() else { return Ok(()) };
    let par: HashMap<CaseDeFiche, &Lu> = identite(ls).into_iter().map(|(cle, _, l)| (cle.into(), l)).collect();
    let table = declarations::champs::fiche(NomDeFiche::Salarie)?;
    for (k, v) in champs.cases() {
        // Une forme lisible vient du bulletin de sa forme declaree.
        let base = match completer::forme_declaree_de(&table, *k) {
            Some(b) => b,
            None => *k,
        };
        let l = match par.get(&base) {
            Some(l) => *l,
            None => *dernier,
        };
        let mut un = Saisie::default();
        un.poser(*k, v.clone());
        entree.noter(&un, &Origine::bulletin(Some(chemin(l)), Some(l.page)));
    }
    Ok(())
}

/// Ce que les bulletins d'un salarie impriment de lui, champ par champ : le plus recent qui l'imprime (`ls` dans
/// l'ordre du temps). (cle de la fiche, valeur, bulletin d'ou elle vient.)
fn identite<'a>(ls: &[&'a Lu]) -> Vec<(CaseSalarie, String, &'a Lu)> {
    let mut out: Vec<(CaseSalarie, String, &'a Lu)> = Vec::new();
    for l in ls {
        let s = &l.b.salarie;
        for (cle, v) in [
            (CaseSalarie::Nom, s.nom.clone()),
            (CaseSalarie::Prenom, s.prenom.clone()),
            (CaseSalarie::DateNaissance, s.naissance.map(|d| d.iso())),
            (CaseSalarie::NumCongesSpectacles, s.conges_spectacles.clone()),
            (CaseSalarie::Emploi, s.emploi.clone()),
            (CaseSalarie::CodeSalarie, s.code.clone()),
        ] {
            let Some(v) = v.map(|x| x.trim().to_string()).filter(|x| !x.is_empty()) else { continue };
            out.retain(|x| x.0 != cle);
            out.push((cle, v, l));
        }
    }
    out
}

/// Les champs d'identite que les bulletins disent autrement que la fiche (compares sans casse ni accents). L'emploi
/// change d'un contrat a l'autre, le code salarie d'un logiciel a l'autre : ils ne sont pas compares.
fn ecarts_salarie(fiche: &Saisie, ls: &[&Lu], chemin: &dyn Fn(&Lu) -> String) -> Vec<Ecart> {
    let mut out = Vec::new();
    for (cle, lu, l) in identite(ls) {
        if !matches!(cle, CaseSalarie::Nom | CaseSalarie::Prenom | CaseSalarie::DateNaissance | CaseSalarie::NumCongesSpectacles) {
            continue;
        }
        // Une case vide ou « A COMPLETER » n'est pas un ecart : l'import la complete.
        let Some(a) = fiche.mot(cle).filter(|_| !completer::vide(fiche.case(cle))) else { continue };
        if compare(&a) == compare(&lu) {
            continue;
        }
        out.push(Ecart::new(cle, Valeur::Texte(a), DitPar::Bulletin { bulletin: lu, fichier: chemin(l), page: l.page }));
    }
    out
}

/// Ce que le bulletin dit de l'entreprise autrement que la fiche (compare sans casse ni accents) : le Siret, le NAF, et
/// la raison sociale, l'adresse, l'IDCC et le compte Urssaf -- ce que `fiche_entreprise` en lit (`lue`).
fn ecarts_entreprise(fiche: &Saisie, lue: &Saisie, l: &Lu, chemin: &str) -> Vec<Ecart> {
    let mut out = Vec::new();
    use droit::fiche::CaseEntreprise as E;
    let t = |k: E| fiche.mot(k);
    let siret_fiche = match (t(E::Siret), t(E::Siren), t(E::Nic)) {
        (Some(s), _, _) => Some(s),
        (None, Some(s), Some(n)) => Some(format!("{}{}", s, n)),
        _ => None,
    };
    let de = |k: E| lue.mot(k);
    for (cle, a, lu) in [
        (E::Siret, siret_fiche, siret_lu(&l.b).map(|s| s.to_string())),
        (E::Naf, t(E::Naf), l.b.entreprise.naf.clone()),
        (E::RaisonSociale, t(E::RaisonSociale), de(E::RaisonSociale)),
        (E::Adresse, t(E::Adresse), de(E::Adresse)),
        (E::CodePostal, t(E::CodePostal), de(E::CodePostal)),
        (E::Commune, t(E::Commune), de(E::Commune)),
        (E::Idcc, t(E::Idcc), de(E::Idcc)),
        (E::UrssafCompte, t(E::UrssafCompte), de(E::UrssafCompte)),
    ] {
        let (Some(a), Some(lu)) = (a, lu) else { continue };
        if completer::vide(Some(&Valeur::Texte(a.clone()))) || compare(&a) == compare(&lu) {
            continue;
        }
        out.push(Ecart::new(cle, Valeur::Texte(a), DitPar::Bulletin { bulletin: lu, fichier: chemin.to_string(), page: l.page }));
    }
    out
}

/// CE QUE LE BULLETIN IMPRIME EN MINUSCULES EST DEJA LA FORME LISIBLE : « La Fanfare des Toits », « 3 quai de
/// la Fosse » vont tels quels dans la case qu'on lit (`<cle>_affichage`), la forme declaree s'en derive a l'ecriture.
/// Un texte tout en capitales (CoolPaie imprime ainsi noms et adresses) n'en dit pas la casse : sa forme lisible se
/// fait comme ailleurs (`champs::combler_affichage`). Sans cela, « La Fanfare des Toits » devenait « La Fanfare Des
/// Toits ».
fn lisibles_de_la_piece(nom: NomDeFiche, f: &mut Saisie) -> R<()> {
    for c in declarations::champs::fiche(nom)? {
        let Some(source) = c.derive_de else { continue };
        if f.valeur(source).is_some_and(Valeur::dite) {
            continue;
        }
        let Some(v) = f.valeur(c.cle).and_then(Valeur::texte).map(str::to_string) else { continue };
        if v.chars().any(char::is_lowercase) {
            f.poser(source, Valeur::Texte(v));
        }
    }
    Ok(())
}

/// LA FORME LISIBLE QUE LA PIECE IMPRIME, DANS UNE FICHE QUE L'IMPORT CREE D'UNE DSN : la DSN ecrit sans accent ni
/// minuscule (« CHAMPMESLE », norme DSN, caracteres autorises), la forme lisible qu'on en tire n'est qu'une
/// reconstitution (« Champmesle »). Quand le bulletin imprime le meme texte (compare sans casse ni accents) avec ce que
/// la DSN a perdu (« CHAMPMESLÉ », « Séraphine »), sa forme lisible (`lisibles_de_la_piece`, `champs::combler_affichage`)
/// remplace la reconstitution ; la forme declaree s'en derive a l'ecriture, au seul endroit qui la derive. Une forme
/// lisible que la fiche ne tient pas de la forme declaree (le metier du catalogue) reste la sienne.
fn lisibles_de_la_piece_sur(nom: NomDeFiche, fiche: &Saisie, proposees: &Saisie) -> R<Saisie> {
    let mut out = Saisie::default();
    for c in declarations::champs::fiche(nom)? {
        let Some(source) = c.derive_de else { continue };
        let texte = |s: &'_ Saisie, k: CaseDeFiche| s.valeur(k).and_then(Valeur::texte).map(str::to_string);
        let (Some(officiel), Some(lisible)) = (texte(fiche, c.cle), texte(fiche, source)) else { continue };
        let (Some(officiel_piece), Some(lisible_piece)) = (texte(proposees, c.cle), texte(proposees, source)) else { continue };
        if lisible_piece == lisible || compare(&officiel_piece) != compare(&officiel) {
            continue;
        }
        let mut seul = Saisie::default();
        seul.poser(c.cle, Valeur::Texte(officiel.to_string()));
        let reconstitue = seul.comblee(nom)?;
        if texte(&reconstitue, source) == Some(lisible) {
            out.poser(source, Valeur::Texte(lisible_piece));
        }
    }
    Ok(out)
}

/// La fiche d'un salarie qu'aucune fiche ni aucune DSN ne porte : ce que ses bulletins impriment, champ par champ le
/// plus recent (`ls` dans l'ordre du temps).
fn fiche_salarie(nir: &Nir, ls: &[&Lu], idcc: Option<&droit::fiche::Idcc>) -> R<Saisie> {
    let mut s = SalarieLu::de(nir);
    for (cle, v, _) in identite(ls) {
        if let Some(case) = s.case(cle) {
            *case = Some(v);
        }
    }
    s.date_naissance = ls.iter().rev().find_map(|l| l.b.salarie.naissance);
    if let Some((rue, cp, commune)) = ls.iter().rev().find_map(|l| adresse_lue(&l.b.salarie.adresse)) {
        s.adresse = Some(rue).filter(|r| !r.is_empty());
        (s.code_postal, s.commune) = (Some(cp), Some(commune));
    }
    s.nir_cle = ls.iter().rev().find_map(|l| l.cle_nir.clone()).or_else(|| droit::fiche::CleNir::du_nir(nir));
    s.date_anciennete = ls.iter().map(|l| l.b.debut).min();
    let statut = ls.iter().rev().find_map(|l| l.b.salarie.statut.as_deref());
    // Une salariee recoit le metier au feminin, comme dans sa fiche (`routes::mois::emploi`) : le sexe que porte son NIR.
    let femme = nir.sexe() == Some(droit::fiche::Sexe::Feminin);
    let metier = statut_et_metier(statut, s.emploi.as_deref(), femme, idcc)?;
    salarie_propose(s, &metier)
}

/// CE QU'UNE PIECE IMPRIME D'UN SALARIE, sous les cases de sa fiche.
#[derive(Serialize)]
struct SalarieLu {
    nir: Nir,
    #[serde(skip_serializing_if = "Option::is_none")]
    nir_cle: Option<droit::fiche::CleNir>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nom: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prenom: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    date_naissance: Option<Jour>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_conges_spectacles: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    emploi: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    code_salarie: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    adresse: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    code_postal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    commune: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    date_anciennete: Option<Jour>,
}

impl SalarieLu {
    /// Un salarie a son NIR, dont aucune case n'est encore lue.
    fn de(nir: &Nir) -> SalarieLu {
        SalarieLu {
            nir: nir.clone(),
            nir_cle: None,
            nom: None,
            prenom: None,
            date_naissance: None,
            num_conges_spectacles: None,
            emploi: None,
            code_salarie: None,
            adresse: None,
            code_postal: None,
            commune: None,
            date_anciennete: None,
        }
    }

    /// La case de texte d'identite `cle` (`identite`) ; la date de naissance, un jour, se pose a part.
    fn case(&mut self, cle: CaseSalarie) -> Option<&mut Option<String>> {
        match cle {
            CaseSalarie::Nom => Some(&mut self.nom),
            CaseSalarie::Prenom => Some(&mut self.prenom),
            CaseSalarie::DateNaissance => None,
            CaseSalarie::NumCongesSpectacles => Some(&mut self.num_conges_spectacles),
            CaseSalarie::Emploi => Some(&mut self.emploi),
            CaseSalarie::CodeSalarie => Some(&mut self.code_salarie),
            autre => unreachable!("`identite` ne nomme pas la case {}", NomDeCase::from(autre)),
        }
    }
}

/// La fiche d'un salarie que proposent ses pieces : ce qu'elles impriment de lui, puis ce que son statut et son metier
/// donnent (`statut_et_metier` : l'emploi du catalogue remplace l'intitule imprime), ses formes lisibles, et active.
fn salarie_propose(s: SalarieLu, metier: &EmploiImprime) -> R<Saisie> {
    #[derive(Serialize)]
    struct Actif {
        actif: bool,
    }
    let mut f = proposee::de(NomDeFiche::Salarie, &s)?;
    proposee::fondre(&mut f, &proposee::de(NomDeFiche::Salarie, metier)?);
    lisibles_de_la_piece(NomDeFiche::Salarie, &mut f)?;
    let mut f = f.comblee(NomDeFiche::Salarie)?;
    proposee::fondre(&mut f, &proposee::de(NomDeFiche::Salarie, &Actif { actif: true })?);
    Ok(f)
}

/// Ce que le statut et l'emploi qu'un bulletin imprime disent du salarie, en champs de sa fiche.
///
/// LE STATUT, EN TOUTES LETTRES, et rien de plus : « Artiste » ou « Technicien » font la case « Artiste ou
/// technicien » ; « Cadre » ou « Non cadre », la case « Cadre ou non cadre » (S21.G00.40.003 : 01 cadre, 04 non cadre,
/// cahier technique 2026.1 p. 183) ; « artiste cadre » dit les deux. « Non cadre » ne dit pas artiste ou technicien :
/// cette case-la reste a remplir, et l'apercu le dit.
///
/// L'EMPLOI, QUAND IL NOMME UN METIER DU CATALOGUE ET UN SEUL : « MUSICIEN » est le metier MUS010 ; la fiche
/// prend alors ce que le choix de ce metier donne (`emplois::resoudre` : le code PCS-ESE, le code France Travail, le
/// statut conventionnel), comme si on l'avait choisi dans la liste. Le statut et la qualite de cadre imprimes priment
/// sur ceux du catalogue. Un intitule qui ne nomme aucun metier, ou plusieurs, reste tel quel : le metier se choisit
/// dans la fiche apres l'import, et l'apercu le dit.
///
/// LE METIER SE RESOUT SOUS LA CONVENTION COLLECTIVE QUE LES PIECES DECLARENT (`idcc`) : un metier peut etre cadre dans
/// l'une et pas dans l'autre (`emplois::Metier::cadre_par_defaut`). Aucune : celle du catalogue par defaut.
pub(crate) fn statut_et_metier(statut: Option<&str>, emploi: Option<&str>, femme: bool, idcc: Option<&droit::fiche::Idcc>) -> R<EmploiImprime> {
    let mut f = EmploiImprime { emploi: emploi.map(str::trim).filter(|i| !i.is_empty()).map(str::to_string), ..EmploiImprime::default() };
    let mots: Vec<String> = statut.into_iter().flat_map(bulletin_lu::jetons).collect();
    f.statut = mots.iter().find_map(|m| match m.as_str() {
        "artiste" => Some(droit::fiche::Statut::Artiste),
        "technicien" => Some(droit::fiche::Statut::Technicien),
        _ => None,
    });
    if let Some(i) = mots.iter().position(|m| m == "cadre" || m == "noncadre") {
        let non = mots[i] == "noncadre" || (i > 0 && mots[i - 1] == "non");
        f.statut_categoriel_rc = Some(if non { droit::fiche::StatutCategorielRc::NonCadre } else { droit::fiche::StatutCategorielRc::Cadre });
    }
    let Some(intitule) = f.emploi.clone() else { return Ok(f) };
    if let Some((code, feminin)) = droit::emplois::par_intitule(&intitule, compare).map_err(|m| m.reclasser(Classe::Refuse))? {
        let cadre = f.statut_categoriel_rc.map(|s| s == droit::fiche::StatutCategorielRc::Cadre);
        let r = droit::emplois::resoudre(&code, cadre, feminin || femme, idcc).map_err(|m| m.reclasser(Classe::Refuse))?;
        // Ce que le choix du metier donne ; le statut imprime prime.
        f.emploi = Some(r.emploi);
        f.emploi_affichage = Some(r.emploi_affichage);
        f.pcs_ese = r.pcs_ese.or(f.pcs_ese);
        f.code_emploi_ft = r.code_emploi_ft.or(f.code_emploi_ft);
        f.statut.get_or_insert(r.statut);
        f.statut_conventionnel = r.statut_conventionnel.or(f.statut_conventionnel);
        f.statut_categoriel_rc = Some(r.statut_categoriel_rc);
    }
    Ok(f)
}

/// Le statut et l'emploi imprimes disent-ils quelque chose ?
fn dit_quelque_chose(e: &EmploiImprime) -> bool {
    *e != EmploiImprime::default()
}

/// La suite des licences que les bulletins impriment (mois, numero), la plus ancienne d'abord, comme les DSN.
fn licences_lues(lus: &[Lu]) -> Vec<(Mois, String)> {
    let mut licences: Vec<(Mois, String)> = Vec::new();
    for x in lus {
        if let Some(n) = x.b.entreprise.licence.as_deref().filter(|n| !n.is_empty()) {
            if licences.last().is_none_or(|p| p.1 != n) {
                licences.push((x.mois, n.to_string()));
            }
        }
    }
    licences
}

/// La fiche de l'entreprise, des bulletins : le plus recent pour l'identite, la premiere licence (`poser_licence`).
fn fiche_entreprise(lus: &[Lu], l: &Lu) -> R<Saisie> {
    let e = &l.b.entreprise;
    let siret = siret_lu(&l.b);
    let adresse = adresse_lue(&e.adresse);
    let f = EntrepriseLue {
        raison_sociale: e.nom.as_deref().map(str::trim).filter(|x| !x.is_empty()).map(str::to_string),
        siren: siret.as_ref().map(droit::fiche::Siret::siren),
        nic: siret.as_ref().map(droit::fiche::Siret::nic),
        siret: siret.clone(),
        naf: e.naf.clone().filter(|x| !x.is_empty()),
        adresse: adresse.as_ref().map(|a| a.0.clone()).filter(|r| !r.is_empty()),
        code_postal: adresse.as_ref().map(|a| a.1.clone()),
        commune: adresse.as_ref().map(|a| a.2.clone()),
        idcc: idcc_du_bulletin(&l.b)?,
        urssaf_compte: urssaf_lu(&l.b),
        licence_spectacle: licences_lues(lus).first().map(|x| x.1.clone()),
        ..EntrepriseLue::default()
    };
    entreprise_proposee(f)
}

/// LA CONVENTION COLLECTIVE QU'UN BULLETIN IMPRIME : son IDCC (« IDCC n° 3090 »), sinon la convention que son nom
/// designe, et elle seule (`droit::clauses::idcc_nomme`, les conventions du catalogue) ; rien s'il ne la dit pas.
fn idcc_du_bulletin(b: &BulletinLu) -> R<Option<droit::fiche::Idcc>> {
    let Some(c) = b.entreprise.convention.as_deref() else { return Ok(None) };
    match idcc_lu(c) {
        Some(i) => Ok(Some(i)),
        None => droit::clauses::idcc_nomme(c).map_err(|m| m.reclasser(Classe::Refuse)),
    }
}

/// CE QU'UNE PIECE IMPRIME DE L'ENTREPRISE, sous les cases de sa fiche.
#[derive(Serialize, Default)]
struct EntrepriseLue {
    #[serde(skip_serializing_if = "Option::is_none")]
    raison_sociale: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    siren: Option<droit::fiche::Siren>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nic: Option<droit::fiche::Nic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    siret: Option<droit::fiche::Siret>,
    #[serde(skip_serializing_if = "Option::is_none")]
    naf: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    adresse: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    code_postal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    commune: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    idcc: Option<droit::fiche::Idcc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    urssaf_compte: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    licence_spectacle: Option<String>,
    #[serde(rename = "congesspectacles_adherent", skip_serializing_if = "Option::is_none")]
    adherent: Option<String>,
}

/// La fiche de l'entreprise que proposent ses pieces : ce qu'elles impriment, ses formes lisibles (celles que la piece
/// imprime en minuscules, `lisibles_de_la_piece`), et son nom en clair (`routes::import::entreprise_comblee`).
fn entreprise_proposee(e: EntrepriseLue) -> R<Saisie> {
    let mut f = proposee::de(NomDeFiche::Entreprise, &e)?;
    lisibles_de_la_piece(NomDeFiche::Entreprise, &mut f)?;
    ri::entreprise_comblee(&f)
}

/// Complete une paie par le bulletin. Sur le brut, le net imposable, le net a payer et le PAS, le bulletin l'emporte ;
/// ailleurs il remplit ce qui est vide et note ce qui differe. Rend les ecarts : (champ, valeur d'avant, le bulletin
/// l'emporte, valeur du bulletin).
fn completer(ligne: &mut PaieVersee, b: &BulletinLu, copie: &NomDeFichier, page: usize, idcc: Option<&droit::fiche::Idcc>) -> R<Vec<(&'static str, String, bool, String)>> {
    use engagement::importe::{BulletinSource, CotisationImprimee, DatesTravaillees, LigneImprimee};
    let mut ecarts = Vec::new();
    let pas = b.pas.as_ref().and_then(|p| p.montant);
    for (champ, place, v) in [
        ("brut", &mut ligne.brut, Some(b.brut)),
        ("net_imposable", &mut ligne.net_imposable, b.net_imposable),
        ("net_a_payer", &mut ligne.net_a_payer, Some(b.net_a_payer)),
        ("pas", &mut ligne.pas, pas),
    ] {
        let Some(v) = v else { continue };
        if let Some(avant) = place.filter(|a| *a != v) {
            ecarts.push((champ, avant.to_string(), true, v.to_string()));
        }
        *place = Some(v);
    }
    let plein = |x: &Option<String>| x.clone().filter(|s| !s.trim().is_empty());
    match (&ligne.numero_objet, objet_imprime(b)) {
        (None, o) => ligne.numero_objet = o,
        (Some(avant), Some(o)) if !avant.meme(&o) => ecarts.push(("numero_objet", avant.to_string(), false, o.to_string())),
        _ => {}
    }
    // Le taux du prelevement que le bulletin imprime, quand la DSN ne l'a pas declare.
    if ligne.taux_pas.is_none() {
        ligne.taux_pas = b.pas.as_ref().and_then(|p| p.taux);
    }
    if ligne.verse_le.is_none() {
        ligne.verse_le = b.regle_le;
    }
    for (place, x) in [(&mut ligne.spectacle, &b.spectacle), (&mut ligne.analytique, &b.analytique)] {
        if let Some(x) = plein(x) {
            *place = Some(x);
        }
    }
    for (place, x) in [
        (&mut ligne.cotisations_salariales, b.total_salarie),
        (&mut ligne.cotisations_patronales, b.total_employeur),
        (&mut ligne.cout_employeur, b.cout_employeur),
        (&mut ligne.net_social, b.net_social),
        (&mut ligne.net_avant_impot, b.net_avant_impot),
    ] {
        if x.is_some() {
            *place = x;
        }
    }
    ligne.lignes = b
        .lignes
        .iter()
        .map(|l| LigneImprimee { quantite: l.quantite, libelle: l.libelle.clone(), taux: l.taux, montant: l.montant, soumis: l.soumis, rubrique: l.rubrique.clone(), heures: l.heures })
        .collect();
    ligne.dates = b.dates_travaillees.iter().map(|(debut, fin)| DatesTravaillees { debut: *debut, fin: *fin }).collect();
    ligne.cotisations = Some(
        b.cotisations
            .iter()
            .map(|c| CotisationImprimee {
                famille: c.famille.clone(),
                libelle: c.libelle.clone(),
                cotisation: c.cotisation.clone(),
                base: c.base,
                taux_employeur: c.taux_employeur,
                montant_employeur: c.montant_employeur,
                taux_salarie: c.taux_salarie,
                montant_salarie: c.montant_salarie,
                hors_total: c.hors_total,
            })
            .collect(),
    );
    // Le statut et l'emploi que CE bulletin imprime : un salarie artiste un mois peut etre technicien, ou non cadre au
    // regime general, un autre ; la comparaison des calculs lit celui de la paie (`engagement::ecarts`), pas la fiche.
    let imprime = statut_et_metier(b.salarie.statut.as_deref(), b.salarie.emploi.as_deref(), false, idcc)?;
    if dit_quelque_chose(&imprime) {
        ligne.emploi_imprime = Some(imprime);
    }
    ligne.bulletin = Some(BulletinSource { fichier: copie.clone(), page: page as u64, logiciel: b.logiciel.clone(), numero: b.numero.clone(), contrat: b.contrat.clone() });
    Ok(ecarts)
}

/// Le contrat qu'un bulletin atteste, tel qu'un engagement importe le porte (`importe.contrat`), et sa cle `_cle_engagement`.
/// Les heures se comptent en heures entieres comme celles d'une saisie (`cumul::engagement_de_paie` : arrondi au
/// pair) ; des cachets ou des jours qui ne sont pas entiers, ou un compteur inconnu, ne font pas de contrat.
fn engagement(l: &Lu, c: &Compteurs, salarie: &Option<IdSalarie>, copie: &NomDeFichier) -> R<(Compte, ContratLu)> {
    use socle::echec::refus;
    // LES JOURS TRAVAILLES sont ceux qu'atteste l'attestation employeur mensuelle de l'intermittent (annexes 8 et 10) ;
    // un permanent paye au mois n'en a pas, et son bulletin n'imprime ni dates travaillees ni cumul de jours : son
    // contrat ne compte pas de jours (0), il ne les « ignore » pas.
    let jours = if c.jours.is_none() && !marque_d_intermittent(&l.b) { Some(Decimal::ZERO) } else { c.jours };
    let (Some(cachets), Some(heures), Some(jours)) = (c.cachets, c.heures, jours) else {
        let manque: Vec<&str> =
            [("cachets", c.cachets), ("heures", c.heures), ("jours", c.jours)].iter().filter(|x| x.1.is_none()).map(|x| x.0).collect();
        return Err(refus(format!("{} inconnu(s), on ne les devine pas", manque.join(", "))));
    };
    let entier = |x: Decimal, quoi: &str| -> R<i64> {
        if !x.fract().is_zero() {
            return Err(refus(format!("{} non entiers ({})", quoi, x.normalize())));
        }
        x.to_i64().ok_or_else(|| refus(format!("{} hors limites", quoi)))
    };
    let (cachets, jours) = (entier(cachets, "cachets")?, entier(jours, "jours")?);
    let heures = heures.round_dp_with_strategy(0, RoundingStrategy::MidpointNearestEven).to_i64().ok_or_else(|| refus("heures hors limites"))?;
    let enseignement = c
        .heures_enseignement
        .map(|h| h.round_dp_with_strategy(0, RoundingStrategy::MidpointNearestEven).to_i64().ok_or_else(|| refus("heures d'enseignement hors limites")))
        .transpose()?;
    let mut brut = l.b.brut;
    brut.rescale(2);
    let (debut, fin) = (l.b.debut, l.b.fin);
    let compte = Compte { heures, cachets, jours, brut };
    let plein = |x: &Option<String>| x.clone().filter(|s| !s.trim().is_empty());
    // LA NATURE DU CONTRAT : aucun des bulletins lus ne l'imprime en mention (`bulletins-lecture-base.json`), et rien sur la
    // page ne la fixe a coup sur. Deux marques disent un engagement du spectacle : la cotisation Congés Spectacles
    // (due pour l'artiste ou le technicien qui n'est pas employe douze mois de suite, D. 7121-41 du code du travail,
    // `atelier/reference/sources/caisse-conges-affiliation/LISEZ-MOI.md`) et le numero d'objet (interdit hors des
    // natures 02 et 10, CCH-11 : regle 6). Avec l'une d'elles, le bulletin atteste un CDDU.
    // Sans aucune, c'est le contrat d'un permanent -- CDI ou CDD de droit commun, que le bulletin ne dit pas et qu'on ne
    // devine pas : sa nature reste a dire, se demande apres l'import (`import::nature`), et le contrat ne se compare ni ne
    // se reprend en attendant (`Importe::nature_rangee` refuse de ranger ce qui a ete verse tant qu'elle reste a dire).
    let nature = marque_d_intermittent(&l.b).then_some(NatureDeContrat::CdduSpectacle);
    let mut a = ContratAtteste::vide(Attestation::Bulletin, nature);
    a.numero_objet = objet_imprime(&l.b);
    (a.heures, a.cachets, a.jours, a.brut) = (Some(heures), Some(cachets), Some(jours), Some(brut));
    a.heures_enseignement = enseignement.filter(|h| *h > 0);
    a.numero_bulletin = plein(&l.b.numero);
    // Le numero du contrat, quand le bulletin l'imprime : il se lit, il ne se tire pas (regle 4).
    a.numero_contrat = l.b.contrat.as_deref().and_then(engagement::numeros::NumeroContrat::d_une_piece);
    (a.fichier, a.page) = (Some(copie.clone()), Some(l.page as u64));
    Ok((compte, ContratLu { salarie: salarie.clone(), debut: Some(debut), fin: Some(fin), numero: None, contrat: a }))
}

/// Le bulletin porte-t-il une marque d'engagement du spectacle : un numero d'objet, ou une cotisation Congés
/// Spectacles (sous sa famille, ou une ligne qui la nomme : CoolPaie la range sous les autres contributions) ?
fn marque_d_intermittent(b: &BulletinLu) -> bool {
    objet_imprime(b).is_some()
        || b.cotisations.iter().any(|c| {
            (c.famille.as_deref() == Some("conges_spectacles") || compare(&c.libelle).contains("congesspectacles"))
                && c.montant_employeur.is_some_and(|m| !m.is_zero())
        })
}

/// LE NUMERO D'OBJET QU'UN BULLETIN IMPRIME, la seule lecture de sa case : a blanc ou hors de sa forme, aucun
/// (`NumeroObjetSpectacle::d_une_piece`).
fn objet_imprime(b: &BulletinLu) -> Option<NumeroObjetSpectacle> {
    b.numero_objet.as_deref().and_then(NumeroObjetSpectacle::d_une_piece)
}

/// Le rang de la fiche de spectacle de la reponse qui porte ce numero d'objet (sans regarder la casse) ; `a_creer` : parmi
/// celles a creer seulement.
fn spectacle_du_numero(spectacles: &[FicheImportee], numero: &NumeroObjetSpectacle, a_creer: bool) -> R<Option<usize>> {
    for (rang, e) in spectacles.iter().enumerate() {
        if (!a_creer || e.etat == EtatFiche::ACreer) && e.numero_objet()?.is_some_and(|n| n.meme(numero)) {
            return Ok(Some(rang));
        }
    }
    Ok(None)
}

/// Le nom d'un champ de paie dans une phrase du compte rendu.
fn libelle_champ(champ: &str) -> &str {
    match champ {
        "net_imposable" => "net imposable",
        "net_a_payer" => "net à payer",
        "pas" => "prélèvement à la source",
        "numero_objet" => "numéro d'objet",
        autre => autre,
    }
}

// --------------------------------------------------------------------------------------------- les certificats et les AEM en PDF

/// Un certificat d'emploi Congés Spectacles ou une AEM en PDF, lus, et le rang de son fichier dans le tri. `c` : ce que
/// la piece dit de l'employeur, du salarie et du contrat, sous la forme d'un certificat (`aem_lu::Aem::commun`) ;
/// `aem` : ce que l'AEM dit de plus (les heures, les cachets, les jours, le numero d'objet, le statut, la naissance).
struct Attestee {
    fichier: usize,
    c: import::certificat_lu::Certificat,
    aem: Option<import::aem_lu::Aem>,
}

impl Attestee {
    /// Son nom dans le compte rendu.
    fn nom(&self) -> &'static str {
        match &self.aem {
            Some(a) if a.aer => "AER Cinéma Spectacle",
            Some(_) => "AEM",
            None => "certificat Congés Spectacles",
        }
    }

    /// Sa `source`, dans ce que l'import dit d'ou vient une valeur (et son `atteste_par`).
    fn source(&self) -> &'static str {
        if self.aem.is_some() { "AEM" } else { "certificat" }
    }

    /// La piece qu'elle est, dans la reponse.
    fn piece(&self) -> Source {
        if self.aem.is_some() { Source::Aem } else { Source::Certificat }
    }

    /// Le mois dont elle parle : celui de l'AEM, celui de la fin du certificat.
    fn mois(&self) -> Option<Mois> {
        self.aem.as_ref().and_then(|a| a.mois).or_else(|| self.c.fin.map(|f| f.mois_civil()))
    }
}

/// La fiche d'un salarie que ses certificats donnent (`ls` dans l'ordre du temps : le plus recent l'emporte, champ par
/// champ) : son nom de naissance, son prenom, son NIR et sa cle, son adresse, son emploi et ce que le metier du
/// catalogue qu'il nomme donne (`statut_et_metier`), cadre ou non, et le debut de son premier contrat.
fn fiche_de_certificats(nir: &Nir, ls: &[&Attestee], idcc: Option<&droit::fiche::Idcc>) -> R<Saisie> {
    let dernier = |g: &dyn Fn(&import::certificat_lu::Certificat) -> Option<String>| ls.iter().rev().find_map(|l| g(&l.c).filter(|x| !x.trim().is_empty()));
    let adresse = ls.iter().rev().find_map(|l| adresse_lue(&l.c.adresse));
    let s = SalarieLu {
        nir_cle: ls.iter().rev().find_map(|l| l.c.cle_nir.clone()).or_else(|| droit::fiche::CleNir::du_nir(nir)),
        nom: dernier(&|c| c.nom.clone()),
        prenom: dernier(&|c| c.prenom.clone()),
        // L'AEM dit la date de naissance, et artiste ou technicien par sa case ; le certificat, ni l'un ni l'autre.
        date_naissance: ls.iter().rev().find_map(|l| l.aem.as_ref().and_then(|a| a.naissance)),
        adresse: adresse.as_ref().map(|a| a.0.clone()).filter(|r| !r.is_empty()),
        code_postal: adresse.as_ref().map(|a| a.1.clone()),
        commune: adresse.as_ref().map(|a| a.2.clone()),
        date_anciennete: ls.iter().filter_map(|l| l.c.debut).min(),
        ..SalarieLu::de(nir)
    };
    let cadre = ls.iter().rev().find_map(|l| l.c.cadre).map(|c| if c { "cadre" } else { "non cadre" });
    let statut = ls.iter().rev().find_map(|l| l.aem.as_ref().and_then(|a| a.statut));
    let statut = match (statut, cadre) {
        (Some(s), Some(c)) => Some(format!("{} {}", s, c)),
        (s, c) => s.or(c).map(String::from),
    };
    let emploi = dernier(&|c| c.emploi.clone());
    let femme = nir.sexe() == Some(droit::fiche::Sexe::Feminin);
    let metier = statut_et_metier(statut.as_deref(), emploi.as_deref(), femme, idcc)?;
    salarie_propose(s, &metier)
}

/// La fiche de l'entreprise qu'un certificat ou une AEM donne : son Siret, sa raison sociale, son numero d'adherent a la
/// Caisse des Congés Spectacles (la case « COMPTE » du certificat, « N° d'affiliation à la caisse de congés spectacles »
/// de l'AEM : `documents::conges_spectacles::CLE_ADHERENT`) ; l'AEM dit aussi le code APE, l'IDCC, le code postal et la
/// commune.
fn entreprise_du_certificat(p: &Attestee) -> R<Saisie> {
    let c = &p.c;
    let aem = p.aem.as_ref();
    let f = EntrepriseLue {
        naf: aem.and_then(|a| a.ape.clone()),
        idcc: aem.and_then(|a| a.idcc.clone()),
        code_postal: aem.and_then(|a| a.employeur_code_postal.clone()),
        commune: aem.and_then(|a| a.employeur_commune.clone()),
        raison_sociale: c.employeur.as_deref().map(str::trim).filter(|x| !x.is_empty()).map(str::to_string),
        siren: c.siret.as_ref().map(droit::fiche::Siret::siren),
        nic: c.siret.as_ref().map(droit::fiche::Siret::nic),
        siret: c.siret.clone(),
        adherent: c.compte.clone(),
        ..EntrepriseLue::default()
    };
    entreprise_proposee(f)
}

/// Artiste ou technicien : la fiche qui existe, sinon celle que l'import cree (DSN, bulletin), sinon le metier du
/// catalogue que nomme l'emploi du certificat, sous la convention de l'import (`idcc`). `None` : rien ne le dit.
fn statut_de(ctx: &Contexte, par_nir: &HashMap<Nir, IdSalarie>, a_creer: &[ACreer], nir: &Nir, c: &import::certificat_lu::Certificat, idcc: Option<&droit::fiche::Idcc>) -> R<Option<String>> {
    if let Some(s) = par_nir.get(nir).and_then(|k| ctx.salaries.get(k)).and_then(|f| f.statut.dite()) {
        return Ok(Some(s.as_str().to_string()));
    }
    for a in a_creer.iter().filter(|a| a.nom == NomDeFiche::Salarie) {
        if let (Some(s), true) = (a.donnees.mot(CaseSalarie::Statut), a.nir()?.as_ref() == Some(nir)) {
            return Ok(Some(s));
        }
    }
    let femme = nir.sexe() == Some(droit::fiche::Sexe::Feminin);
    Ok(statut_et_metier(None, c.emploi.as_deref(), femme, idcc)?.statut.map(|s| s.as_str().to_string()))
}

/// Le contrat qu'une AEM en PDF atteste, tel qu'un engagement importe le porte (la forme du contrat d'une AEM deposee,
/// `import::ecrite`), et ce qu'il compte en mots : la periode du mois (`aem_lu::Aem::periode`), ses heures, ses
/// cachets isoles et groupes, ses jours, son brut et son numero d'objet, tels qu'imprimes.
fn contrat_de_l_aem(a: &import::aem_lu::Aem, salarie: Option<IdSalarie>, copie: &NomDeFichier) -> R<(ContratLu, String)> {
    let (Some((debut, fin)), Some(brut)) = (a.periode(), a.brut) else {
        return Err(socle::echec::refus("l'AEM ne dit pas tout ce qu'un contrat demande"));
    };
    // Ce que l'AEM dit, et seulement cela : une colonne qu'elle ne porte pas reste vide (les cachets sont leur somme).
    let (heures, cachets, jours) = (a.heures, Some(a.cachets()), a.jours);
    let mut brut = brut.round_dp(2);
    brut.rescale(2);
    let pluriel = |n: i64, mot: &str| format!("{} {}{}", n, mot, if n > 1 { "s" } else { "" });
    let quoi: Vec<String> = [(heures, "heure"), (cachets, "cachet"), (jours, "jour")].iter().filter_map(|x| x.0.filter(|n| *n > 0).map(|n| pluriel(n, x.1))).collect();
    let plein = |x: &Option<String>| x.clone().filter(|s| !s.trim().is_empty());
    // L'AEM est l'attestation des annexes 8 et 10, celle d'un contrat d'usage du spectacle (Unédic, circulaire 2016-25,
    // `atelier/reference/sources/unedic-circulaire-2016-25-annexes-8-10.txt`).
    let mut c = ContratAtteste::vide(Attestation::Aem, Some(NatureDeContrat::CdduSpectacle));
    c.numero_objet.clone_from(&a.numero_objet);
    (c.heures, c.cachets, c.jours, c.brut) = (heures, cachets, jours, Some(brut));
    c.numero_attestation = plein(&a.numero);
    (c.fichier, c.page) = (Some(copie.clone()), Some(a.page as u64));
    let ligne = ContratLu { salarie, debut: Some(debut), fin: Some(fin), numero: None, contrat: c };
    Ok((ligne, if quoi.is_empty() { "aucune heure, aucun cachet, aucun jour".into() } else { quoi.join(", ") }))
}

/// Le contrat qu'un certificat atteste, tel qu'un engagement importe le porte, et ce qu'il compte en mots.
///
/// LE « NB JOURS OU CACHETS » : l'attestation des annexes 8 et 10 porte « le nombre d'heures de travail, ou le nombre
/// de cachets pour les artistes » (Unédic, circulaire 2016-25, `atelier/reference/sources/unedic-circulaire-2016-25-
/// annexes-8-10.txt`, l. 5802) ; le certificat n'a qu'une case pour les deux. Inference : pour un artiste, elle compte
/// des cachets ; pour un technicien, des jours. Sans statut, rien ne le dit : pas de contrat. Les heures, que le
/// certificat ne porte pas, ne s'inventent pas (0) ; le brut est celui du certificat, « arrondi à l'euro le plus proche ».
fn contrat_du_certificat(c: &import::certificat_lu::Certificat, statut: Option<&str>, salarie: Option<IdSalarie>, copie: &NomDeFichier) -> R<(ContratLu, String)> {
    let (Some(debut), Some(fin), Some(nombre), Some(brut)) = (c.debut, c.fin, c.nombre, c.brut) else {
        return Err(socle::echec::refus("le certificat ne dit pas tout ce qu'un contrat demande"));
    };
    let pluriel = |n: i64, mot: &str| format!("{} {}{}", n, mot, if n > 1 { "s" } else { "" });
    let (cachets, jours, quoi) = match statut {
        Some("artiste") => (nombre, 0, pluriel(nombre, "cachet")),
        Some(s) if !s.trim().is_empty() => (0, nombre, pluriel(nombre, "jour")),
        _ => {
            let emploi = match &c.emploi {
                Some(e) => format!("l'emploi « {} » ne nomme aucun métier du catalogue", e),
                None => "le certificat ne dit pas l'emploi".to_string(),
            };
            return Err(socle::refus!(
                "{} : le nombre de jours ou cachets ne dit pas si ce sont des cachets (artiste) ou des jours (technicien), le contrat n'est pas repris",
                emploi
            ));
        }
    };
    let mut brut = brut;
    brut.rescale(2);
    // Le certificat Congés Spectacles atteste un engagement du spectacle (D. 7121-41 du code du travail,
    // `atelier/reference/sources/caisse-conges-affiliation/LISEZ-MOI.md`) : un contrat d'usage.
    let mut a = ContratAtteste::vide(Attestation::Certificat, Some(NatureDeContrat::CdduSpectacle));
    (a.heures, a.cachets, a.jours, a.brut) = (Some(0), Some(cachets), Some(jours), Some(brut));
    a.numero_certificat = c.numero.clone();
    a.base_conge = c.base_conge;
    a.atteste_le = c.signe_le;
    (a.fichier, a.page) = (Some(copie.clone()), Some(c.page as u64));
    Ok((ContratLu { salarie, debut: Some(debut), fin: Some(fin), numero: None, contrat: a }, quoi))
}

/// Les compteurs de chaque bulletin lu (`bulletin_lu::compteurs`), confrontes a ses cumuls avec ses freres : les autres
/// bulletins du meme salarie, du meme logiciel, dont la periode finit le meme mois (un meme bulletin lu deux fois, meme
/// numero et memes dates, n'en est pas un).
fn compter(lus: &mut [Lu], racine: &Path) {
    let comptes: Vec<R<Compteurs>> = lus
        .iter()
        .map(|l| {
            let freres: Vec<&BulletinLu> = lus
                .iter()
                .filter(|s| s.nir == l.nir && s.mois == l.mois && s.b.logiciel == l.b.logiciel)
                .filter(|s| (&s.b.numero, s.b.debut, s.b.fin) != (&l.b.numero, l.b.debut, l.b.fin))
                .map(|s| &s.b)
                .collect();
            bulletin_lu::compteurs(&l.b, &freres, racine)
        })
        .collect();
    for (l, c) in lus.iter_mut().zip(comptes) {
        l.compteurs = c;
    }
}
