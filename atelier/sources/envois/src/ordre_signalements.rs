//! Le numero d'ordre (S20.G00.05.004) du prochain signalement, LA PLATE-FORME FAISANT FOI.
//!
//! POURQUOI. Le journal d'un dossier (`journal/depots.jsonl`) ne connait que ce que CE dossier a depose ; la plate-forme se
//! souvient de tout, par SIRET, essais compris. Cahier technique DSN 2026.1, §1.4.3.2, p. 14 : « Quand un signalement
//! d'evenement de type normal a ete accepte pour un SIRET employeur et un numero d'ordre de la declaration donne, il n'est
//! pas possible d'emettre un deuxieme signalement d'evenement de type normal avec le meme SIRET employeur et le meme
//! numero d'ordre » ; p. 136 : pour un signalement, l'ordre « ne doit pas etre remis a zero afin qu'il soit unique quel
//! que soit le mois d'emission ». Un dossier au journal vide peut donc proposer un ordre deja pris : la plate-forme le
//! refuse alors (B1-208-10).
//!
//! CE QUE LA PLATE-FORME DIT. Seul le bilan de nature 11 (certificat ou bilan d'anomalies) porte l'ordre, avec la nature,
//! le type, le SIREN ; ni salarie, ni contrat (`retours::declarations_du_bilan`). Les documents se purgent a TROIS MOIS
//! (guide d'implementation de l'API DSN v6.8, §8.2, p. 93, `atelier/reference/sources/gip-mds-api-dsn/`) : on releve donc
//! les trois derniers mois, et ce qui est plus ancien n'est plus que dans nos archives (`retours/`) et le journal.
//!
//! LE PLUS GRAND DES DEUX, ESSAIS ET REELS ENSEMBLE. Un signalement s'ecrit une fois et part en test puis en reel
//! (la repetition, `depot_acte::exiger_repetition`) au meme ordre ; la base d'essai et la base reelle sont distinctes.
//! L'ordre doit donc etre libre dans les deux : on prend le plus grand de tous. Qu'une sequence reelle
//! saute des numeros, aucun texte ne l'interdit : p. 15, « Il ne sera pas possible d'integrer une annule et remplace de
//! rang inferieur au precedent envoi », et le rang se compare (B1-208-11, « inferieur (ou egal) au dernier ordre
//! accepte ») ; trop grand ne coute rien, trop petit est refuse (INFERENCE de ces deux textes). Tous SIRET de l'entreprise
//! confondus : trop grand ne coute rien.
//!
//! SANS RESEAU, OU SANS MOT DE PASSE, le journal et les bilans deja archives decident, et l'avertissement le dit : jamais
//! en silence. Une compagnie fictive n'a pas de plate-forme : ses envois sont joues, son journal est la seule
//! memoire, rien n'est a dire.

use droit::fiche::Lue as _;
use std::path::Path;

use droit::contexte::Contexte;
use droit::nature_declaration::NatureDeLaDeclaration;
use socle::echec::{erreur, Classe};
use crate::branchement::Branchement;
use crate::releve_retours::{self, Client};
use crate::suites::Suites;
use socle::echec::Echec;
use socle::mois::Mois;

/// Trois mois de releve : la retention des documents de la plate-forme (guide API v6.8, §8.2). 92 jours couvrent trois
/// mois civils, quels qu'ils soient.
pub const JOURS_DE_RELEVE: i64 = 92;

/// Ce que le moteur sait du dernier ordre pris, et ce qu'il doit en dire.
#[derive(Debug, Clone)]
pub struct OrdreConnu {
    /// Le plus grand ordre de signalement connu, 0 si aucun : le prochain signalement part au suivant.
    pub dernier: i64,
    /// Ce que l'appelant affiche avec le fichier ecrit : la plate-forme qui n'a pas repondu, ou un ordre qu'elle
    /// connaissait et que le journal ignorait.
    pub avertissements: Vec<String>,
}

/// Le SIREN de l'entreprise : `siren`, sinon les neuf premiers chiffres du `siret` ; sans l'un ni l'autre, le refus
/// nomme `siren`.
fn siren(ent: &droit::fiche::ChampsEntreprise) -> Result<String, Echec> {
    match (ent.siren.portee(), ent.siret.portee()) {
        (Some(s), _) => Ok(s.to_string()),
        (None, Some(siret)) => Ok(siret.chars().take(9).collect()),
        (None, None) => Err(socle::fichiers::absent("siren")),
    }
}

/// Le plus grand ordre des signalements (natures 04, 05, 07, 08) que les bilans de nature 11 archives dans `retours/`
/// declarent pour ce SIREN, essais et reels, acceptes ou non ; et le flux qui le porte. Le flux se lit dans le nom de
/// l'archive (`AAAAMMJJ-hhmmss_nature11_<flux>.<ext>`, `releve_retours::archiver_retour`).
pub fn dernier_ordre_des_bilans(racine: &Path, siren: &str) -> Result<Option<(i64, String)>, Echec> {
    let mut dernier: Option<(i64, String)> = None;
    for (nom, p) in bilans_archives(racine)? {
        let octets = std::fs::read(&p).map_err(|e| socle::echec::du_disque(&p, &e).reclasser(Classe::Disque))?;
        // Le flux suit la nature dans le nom, avant l'extension.
        let Some(apres) = nom.splitn(3, '_').nth(2) else { continue };
        let flux = match apres.rsplit_once('.') {
            Some((f, _)) => f,
            None => apres,
        };
        for d in crate::retours::declarations_du_bilan(&octets)? {
            let signalement = d.nature_declaration.is_some_and(NatureDeLaDeclaration::signalement_envoye);
            if !signalement || d.siren.as_deref() != Some(siren) {
                continue;
            }
            let Some(o) = ordre_lu(d.ordre.as_deref(), &p)? else { continue };
            if dernier.as_ref().is_none_or(|(m, _)| o > *m) {
                dernier = Some((o, flux.to_string()));
            }
        }
    }
    Ok(dernier)
}

/// Les bilans (nature 11) archives dans `retours/`, par nom (`releve_retours::archiver_retour`) ; aucun sans dossier des retours.
fn bilans_archives(racine: &Path) -> Result<Vec<(String, std::path::PathBuf)>, Echec> {
    let dossier = racine.join(crate::retours::RETOURS);
    let lecture = match std::fs::read_dir(&dossier) {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(socle::echec::du_disque(&dossier, &e).reclasser(Classe::Disque)),
    };
    let mut noms = Vec::new();
    for e in lecture {
        let nom = e.map_err(|x| socle::echec::du_disque(&dossier, &x).reclasser(Classe::Disque))?.file_name().to_string_lossy().into_owned();
        let p = dossier.join(&nom);
        if crate::retours::archive_de_nature(&nom, "11") && p.is_file() {
            noms.push((nom, p));
        }
    }
    noms.sort();
    Ok(noms)
}

/// Un numero d'ordre lu (S20.G00.05.004, dans un bilan ou au journal) ; aucun s'il n'est pas dit, un refus s'il ne se
/// lit pas (son fichier a part).
fn ordre_lu(ordre: Option<&str>, ou: &Path) -> Result<Option<i64>, Echec> {
    ordre
        .map(|o| o.trim().parse::<i64>().map_err(|_| socle::echec::du_disque(&ou, &format!("Un numéro d'ordre de déclaration ne se lit pas : {}.", o)).reclasser(Classe::Illisible)))
        .transpose()
}

/// Le plus grand ordre des signalements d'ESSAI que ce dossier a deposes (`detail.ordre` des lignes `dsn` en test) :
/// la base d'essai les garde, et le meme fichier part ensuite en reel. Une ligne qui ne dit pas son ordre n'en donne pas.
fn dernier_ordre_des_essais(racine: &Path) -> Result<i64, Echec> {
    let lignes = crate::journal::lire(racine, Some(crate::journal::Acte::Dsn), None, false).map_err(|m| m.reclasser(Classe::Refuse))?;
    let mut dernier = 0;
    for e in lignes.iter().filter_map(crate::journal::Ligne::dsn).filter(|e| e.mode == crate::journal::Mode::Test) {
        if !e.detail.nature_declaration.is_some_and(NatureDeLaDeclaration::signalement_envoye) {
            continue;
        }
        if let Some(o) = ordre_lu(e.detail.ordre.as_deref(), &crate::journal::chemin(racine))? {
            dernier = dernier.max(o);
        }
    }
    Ok(dernier)
}

/// Le plus grand ordre de signalement connu, la plate-forme faisant foi : la releve des retours des trois derniers mois
/// (`releve_retours::relever_retours`, qui archive avant toute lecture), puis le plus grand de ce que disent les bilans
/// archives et le journal (reels, `journal::dernier_ordre_des_signalements`, qui refuse une ligne reelle sans son
/// ordre ; essais). `client` : le client net-entreprises, avec le mot de passe de consultation que `moteur` a pris (un test
/// le pointe sur un faux serveur) ; `None` : aucun mot de passe, la plate-forme ne se lit pas et cela se dit.
pub fn dernier_ordre_connu(ctx: &Contexte, branchement: Branchement, client: Option<Client>, suites: &mut Suites) -> Result<OrdreConnu, Echec> {
    let reels = crate::journal::dernier_ordre_des_signalements(&ctx.racine).map_err(|m| m.reclasser(Classe::Refuse))?;
    let journal = reels.max(dernier_ordre_des_essais(&ctx.racine)?);
    let mut avertissements = Vec::new();

    // LA PLATE-FORME : une compagnie fictive n'en a pas ; ailleurs, ce qui empeche de la lire se dit.
    let joue = client.is_none() && branchement.joue();
    // Ce qui empeche de la lire, en une phrase ; `None` : elle est lue.
    let pas_lue: Option<String> = if joue {
        None
    } else {
        let fin = socle::temps::secondes();
        match fin.plus(-JOURS_DE_RELEVE * 86_400).ok_or_else(|| socle::echec::refus(socle::date::HORS_CALENDRIER)).and_then(|debut| releve_retours::relever_retours(ctx, branchement, debut, fin, false, client, releve_retours::CANAL_DSN, suites)) {
            Ok(r) if r.incidents.is_empty() => None,
            Ok(r) => Some(releve_retours::incidents_en_phrase(&r)),
            Err(e) => Some(e.phrase),
        }
    };

    let bilans = dernier_ordre_des_bilans(&ctx.racine, &siren(&ctx.entreprise)?)?;
    let dernier = match &bilans {
        Some((plateforme, _)) => journal.max(*plateforme),
        None => journal,
    };
    if let Some(pourquoi) = pas_lue {
        avertissements.push(format!(
            "La plate-forme n'a pas pu etre consultee ({}) : l'ordre {} suit le journal de ce dossier et les retours deja archives, qui ne savent que ce que ce dossier a depose et releve. Un signalement depose d'ailleurs sous le meme SIREN (un autre poste, une copie, un essai) peut avoir pris cet ordre : la plate-forme le refuserait (B1-208-10). Pour en etre sur : relever les retours, puis refaire le signalement.",
            pourquoi.trim_end_matches('.'),
            dernier + 1
        ));
    }
    if let Some((plateforme, flux)) = bilans.filter(|(p, _)| *p > journal) {
        avertissements.push(format!(
            "Ordre {} : la plate-forme connait un signalement a l'ordre {} (flux {}) que le journal de ce dossier ne connait pas (il s'arretait a {}). Elle fait foi : le numero d'ordre d'un signalement ne se prend pas deux fois (cahier technique DSN 2026.1, §1.4.3.2, p. 14).",
            dernier + 1,
            plateforme,
            flux,
            journal
        ));
    }
    Ok(OrdreConnu { dernier, avertissements })
}

/// La conduite a tenir devant un signalement refuse B1-208-10, ou `None` si ce bilan n'en est pas un. Le bilan ne dit
/// pas QUEL signalement tient deja cet ordre : la phrase donne les deux cas.
pub fn conduite_b1_208_10(declaration: &crate::retours::DeclarationIdentifiee, flux: Option<&str>) -> Option<String> {
    let nature = declaration.nature_declaration.filter(|n| n.signalement_envoye())?;
    if !declaration.codes.iter().any(|c| c == "B1-208-10") {
        return None;
    }
    let essai = if declaration.essai_reel.as_deref() == Some(crate::retours::ESSAI_TEST) { "essai" } else { "reel" };
    // Ce que le bilan ne dit pas se dit tel.
    let dit = |x: Option<&str>| match x {
        Some(t) => t.to_string(),
        None => "non dit".to_string(),
    };
    let (flux, ordre) = (dit(flux), dit(declaration.ordre.as_deref()));
    Some(format!(
        "Signalement {} de nature {} (flux {}, ordre {}) refuse B1-208-10, << il existe deja une declaration normale >> : pour un signalement, la plate-forme n'en accepte qu'un normal par SIRET et par numero d'ordre (cahier technique DSN 2026.1, §1.4.3.2, p. 14), et l'ordre {} etait deja pris -- par un depot que ce dossier ne connaissait peut-etre pas (un autre poste, une copie, un essai ; la base d'essai a sa propre memoire). Le bilan ne dit pas lequel. Si l'evenement n'est pas encore declare : refaire le signalement sans `--ordre`, il prend le suivant du plus grand ordre que la plate-forme et le journal connaissent. S'il l'est deja (un signalement accepte pour le meme salarie et le meme contrat) : il n'y a rien a renvoyer ; pour le corriger, c'est un annule-et-remplace (`--remplace`, S20.G00.05.006 = date de constitution et ordre du signalement accepte), a un ordre superieur (§1.4.3.4, p. 15).",
        essai, nature, flux, ordre, ordre
    ))
}

// ------------------------------------------------------------------ et l'ordre des mensuelles

/// Ce que les bilans archives disent des DSN MENSUELLES d'une base (essai ou reel), lu en silence : le compteur d'ordre
/// lit tous les retours, sans rien montrer, y compris ceux des tests faits ailleurs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MensuellesVues {
    /// Le plus grand ordre de mensuelle vu pour ce mois de constitution ; aucun si aucune n'est vue.
    pub ordre: Option<i64>,
    /// Une mensuelle du mois declare a ete ACCEPTEE dans cette base : une normale de plus y serait refusee, le depot
    /// suivant part en annule-et-remplace (cahier technique DSN, §1.4.3.2).
    pub du_mois_acceptee: bool,
}

/// Le rang de la premiere mensuelle d'un mois civil : le numero d'ordre « est remis a zero a chaque premier jour de mois
/// civil » (cahier technique DSN 2026.1, p. 136, `atelier/reference/norme-dsn/texte/bloc-S20.G00.05.txt`), la premiere
/// declaration prend le rang qui suit.
pub const PREMIER_RANG: i64 = 1;

impl MensuellesVues {
    /// Le rang de la mensuelle suivante : celui qui suit le plus grand vu, le premier si aucune n'est vue.
    pub fn suivant(&self) -> i64 {
        match self.ordre {
            Some(o) => o + 1,
            None => PREMIER_RANG,
        }
    }
}

/// L'ordre d'une mensuelle « au cours du mois courant », lu sur la plate-forme. Cahier technique DSN 2026.1, p. 136
/// (`atelier/reference/norme-dsn/texte/bloc-S20.G00.05.txt`) : le numero d'ordre « permet d'identifier le rang de
/// constitution d'une declaration au cours du mois courant. Pour une Mensuelle, ce numero est remis a zero a chaque
/// premier jour de mois civil » ; et B1-208-11 (`dsn::generer`) compare le rang « AAAAMM + ordre » au
/// « dernier ordre accepte ». La plate-forme se souvient de tout ce qui est parti sous le SIREN, d'ou qu'il vienne
/// (l'atelier, un autre poste) ; le journal du dossier ne sait que ce qu'il a envoye.
///
/// Deux lectures, sur les bilans de nature 11 archives dans `retours/` pour ce SIREN et cette base (`essai_reel`) :
/// - l'ordre de chaque mensuelle (nature 01) recue le mois `constitution`. Le mois de reception du bilan
///   (`date_reception`) tient lieu de mois de constitution : la date de constitution est le jour meme ou le fichier
///   s'ecrit (`dsn::generer`), et un fichier se depose ce jour-la ou plus tard ; la lecture ne peut donc que surestimer
///   l'ordre (un fichier ecrit le 31 et depose le 1er compte pour le mois suivant), jamais le sous-estimer, et trop
///   grand ne coute rien ;
/// - le rang que la plate-forme cite dans un refus (« d'ordre 20261021 inferieur (ou egal) au dernier ordre accepte
///   20261030 ») : AAAAMM + ordre, exact, quel que soit le jour du bilan.
///
/// `mois` : le mois declare (S20.G00.05.005) dont on cherche une mensuelle acceptee.
pub fn dernier_ordre_des_mensuelles(racine: &Path, siren: &str, essai: bool, constitution: Mois, mois: Mois) -> Result<MensuellesVues, Echec> {
    let base = if essai { crate::retours::ESSAI_TEST } else { crate::retours::ESSAI_REEL };
    let aaaamm = format!("{:04}{:02}", constitution.annee(), constitution.numero());
    let cite = regex::Regex::new(r"ordre (?:accept\S* )?(\d{7,})").map_err(|e| erreur(Classe::Illisible, e.to_string()))?;
    // Avant le premier bilan lu : aucune mensuelle vue.
    let mut vues = MensuellesVues { ordre: None, du_mois_acceptee: false };
    for (_, p) in bilans_archives(racine)? {
        let octets = std::fs::read(&p).map_err(|e| socle::echec::du_disque(&p, &e).reclasser(Classe::Disque))?;
        let Some(bilan) = crate::retours::bilan_si_xml(&octets)? else { continue };
        if bilan.essai_reel.as_deref() != Some(base) {
            continue;
        }
        let du_mois_recu = bilan.recu_le.as_ref().is_some_and(|r| r.jour().mois_civil() == constitution);
        let du_mois_declare = bilan.mois == Some(mois);
        let mut du_siren = false;
        for d in crate::retours::declarations_du_bilan(&octets)? {
            if d.nature_declaration != Some(crate::depot::MENSUELLE) || d.siren.as_deref() != Some(siren) {
                continue;
            }
            du_siren = true;
            if let Some(o) = ordre_lu(d.ordre.as_deref(), &p)?.filter(|_| du_mois_recu) {
                vues.ordre = vues.ordre.max(Some(o));
            }
            if du_mois_declare && d.etat.as_deref().is_some_and(|e| e.eq_ignore_ascii_case("OK")) {
                vues.du_mois_acceptee = true;
            }
        }
        if !du_siren {
            continue;
        }
        for c in cite.captures_iter(&String::from_utf8_lossy(&octets)) {
            if let Some(reste) = c[1].strip_prefix(aaaamm.as_str()) {
                if let Some(o) = ordre_lu(Some(reste), &p)? {
                    vues.ordre = vues.ordre.max(Some(o));
                }
            }
        }
    }
    Ok(vues)
}

/// Le SIREN de l'entreprise du dossier, comme le lit `dernier_ordre_connu`.
pub fn siren_de(ctx: &Contexte) -> Result<String, Echec> {
    siren(&ctx.entreprise)
}
