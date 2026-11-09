//! LE CODE RISQUE ET LE TAUX AT/MP DE LA COMPAGNIE (`taux-atmp`), dans la configuration : la fiche entreprise
//! (`taux_atmp` : `code_risque`, `valeur`, `annee`, `notification`), datee comme toute la configuration et comme ceux des
//! autres etablissements (`routes::etablissements`). Un taux saisi pour une annee vaut a partir de son 1er janvier ; un
//! taux recu (une notification) a partir de sa date d'effet, et c'est lui qui vaut.
//!
//! Ce que les textes disent (`atelier/reference/sources/tarification-atmp/LISEZ-MOI.md`) : la CARSAT notifie le code risque
//! et le taux ; l'employeur ne declare qu'un code notifie, et 999ZZ tant qu'il n'a pas de notification, sans taux (norme DSN,
//! S21.G00.40.040 et .043, `norme-dsn/texte/bloc-S21.G00.40.txt` l. 1224-1315). Le taux d'un risque « TC » de l'annexe 1 de
//! l'arrete de l'annee est collectif quel que soit l'effectif (D. 242-6-14, I ; art. 5 de l'arrete) ; celui d'un autre risque
//! ne l'est que sous 20 salaries (D. 242-6-2).
//!
//! A LA CREATION d'une compagnie, le risque par defaut du socle de l'annee (`risques_atmp.par_defaut`, 92.3AD : « tout
//! intermittent du spectacle ») se pose de lui-meme, le code seul, son taux celui de l'arrete de l'annee
//! (`a_la_creation`) ; l'ecran les dit une fois, avec le libelle de l'arrete, l'explication, ou les lire et ou les changer
//! (`lire`). Une annee que la compagnie n'a pas dite reprend son code, et le taux de l'arrete de l'annee pour un risque TC
//! (`Contexte::taux_atmp_de`). L'avance de l'ecran propose les codes que le socle connait, ou un autre code notifie.

use droit::fiche::Lue as _;

use serde::{Deserialize, Serialize};

use crate::routes::enveloppe::Dite;
use droit::bareme::{self, RisqueAtmp};
use droit::contexte::Contexte;
use droit::fiche::{Datee, Montant, NotificationAtmp, TauxAtmp};
use socle::date::Jour;
use socle::echec::{erreur, Classe, R};
use socle::num::Decimal;

/// Le socle livre de l'annee, s'il existe, lu par sa forme (`droit::contexte::socle_livre_lu`) ; un socle livre qui ne se
/// lit pas se refuse.
fn socle_de(an: i64) -> R<Option<droit::forme_bareme::Pile>> {
    droit::contexte::socle_livre_lu(an as i32).map_err(|m| erreur(Classe::Illisible, m))
}


fn aujourdhui_jj_mm_aaaa() -> String {
    let iso = socle::paris::iso_secondes(socle::temps::secondes());
    format!("{}/{}/{}", &iso[8..10], &iso[5..7], &iso[..4])
}

fn en_pourcent(t: Decimal) -> String {
    let mut t = t.round_dp(2);
    t.rescale(2);
    format!("{} %", t.to_string().replace('.', ","))
}

/// LA FORME D'UN CODE RISQUE (S21.G00.40.040, `X [5,6]`) : trois chiffres et deux caracteres, suivis du « B » du taux bureau
/// s'il est sur la notification (`bloc-S21.G00.40.txt` l. 1229-1240) ; 999ZZ sans notification.
fn code_risque_valide(code: &str) -> bool {
    let c: Vec<char> = code.chars().collect();
    (c.len() == 5 || (c.len() == 6 && c[5] == 'B'))
        && c[..3].iter().all(|x| x.is_ascii_digit())
        && c[3..5].iter().all(|x| x.is_ascii_digit() || x.is_ascii_uppercase())
}

/// L'EFFECTIF AU REGARD DU SEUIL DE 20 SALARIES (D. 242-6-2, 1°) pour payer en `an` (`droit::contexte::Effectif::atteint`) :
/// sous le seuil, au seuil, ou inconnu (et ce qui manque pour le dire).
enum SeuilDeVingt {
    Sous,
    Atteint,
    Inconnu(String),
}

fn seuil_de_vingt(ctx: &Contexte, an: i64) -> R<SeuilDeVingt> {
    Ok(match ctx.effectif().map_err(|m| erreur(Classe::Illisible, m))?.atteint(20, an as i32) {
        Ok(true) => SeuilDeVingt::Atteint,
        Ok(false) => SeuilDeVingt::Sous,
        Err(manque) => SeuilDeVingt::Inconnu(manque),
    })
}

/// LE CODE ET LE TAUX DE L'ANNEE `an` S'ECRIVENT DANS LA FICHE ENTREPRISE, a partir de `depuis`
/// (`routes::fiches::dater_taux_atmp`) : le taux (rien avec 999ZZ, ou quand il est a saisir), son annee, et la
/// notification d'ou il vient. `note` dit d'ou il vient.
fn ecrire(ctx: &Contexte, an: i64, code: String, valeur: Option<Decimal>, notification: Option<NotificationAtmp>, depuis: &Jour, note: &str) -> R<()> {
    let taux = TauxAtmp { code_risque: Some(code), valeur: valeur.map(|v| Montant::Flottant(socle::num::flottant(v))), annee: Some(an), notification };
    crate::routes::fiches::dater_taux_atmp(ctx, taux, depuis, note)
}

/// Le 1er janvier de l'annee `an`, d'ou vaut le taux saisi pour elle.
fn premier_janvier(an: i64) -> R<Jour> {
    let a = i32::try_from(an).map_err(|e| erreur(Classe::Refuse, format!("Année {} hors du calendrier ({}).", an, e)))?;
    Jour::nouveau(a, 1, 1).ok_or_else(|| erreur(Classe::Refuse, format!("Année {} hors du calendrier.", an)))
}

/// LE CODE RISQUE SEUL, SANS TAUX : celui que l'import lit dans une DSN reprise quand le taux de l'annee n'est pas celui
/// de l'arrete (`import::parametrage`) ; « Pour demarrer » demande alors le taux notifie. `note` dit d'ou vient le code.
pub(crate) fn poser_code_sans_taux(ctx: &Contexte, an: i64, code: &str, note: String) -> R<()> {
    ecrire(ctx, an, code.to_string(), None, None, &premier_janvier(an)?, &note)
}

/// A LA CREATION D'UNE COMPAGNIE : le risque par defaut du socle de l'annee, le code seul, de tout temps ; son taux est
/// celui que l'arrete de chaque annee lui fixe (`Contexte::taux_atmp_de`). Un risque sans taux collectif, ou une annee dont
/// le socle n'a pas de table des risques (son arrete n'est pas depose), ne pose rien : « Pour demarrer » le demande.
pub(crate) fn a_la_creation(an: i64) -> R<Option<Vec<Datee<TauxAtmp>>>> {
    let Some(socle) = socle_de(an)? else { return Ok(None) };
    let Some(r) = bareme::risque_par_defaut(&socle) else { return Ok(None) };
    if bareme::taux_de_l_arrete(&r, None, false).is_none() {
        return Ok(None);
    }
    Ok(Some(vec![Datee {
        depuis: None,
        valeur: Some(TauxAtmp { code_risque: Some(r.code), ..TauxAtmp::default() }),
        note: Some(format!("Pose a la creation de la compagnie : le risque par defaut du socle {}, son taux celui de l'arrete de l'annee (atelier/reference/sources/tarification-atmp/).", an)),
    }]))
}

/// POSE LE CODE RISQUE ET LE TAUX DE L'ANNEE `an`, a partir de son 1er janvier. `valeur` vide : le taux de l'arrete s'il s'applique au code (un risque
/// TC, ou un autre sous 20 salaries) ; sinon il se demande. 999ZZ n'a pas de taux (SIG-11). `source` dit d'ou vient la
/// valeur.
pub(crate) fn poser(ctx: &Contexte, an: i64, code: &str, valeur: Option<Decimal>, source: &str) -> R<()> {
    let code = code.trim_matches(socle::texte::blanc).replace(['.', ' '], "").to_uppercase();
    if !code_risque_valide(&code) {
        return Err(erreur(
            Classe::Refuse,
            format!(
                "Code risque « {} » : trois chiffres et deux lettres ou chiffres, suivis du « B » du taux bureau s'il est sur la notification (923AD, 602MDB) ; 999ZZ sans notification.",
                code
            ),
        ));
    }
    let socle = socle_de(an)?;
    let connu = socle.as_ref().and_then(|s| bareme::risques_de(s).into_iter().find(|r| r.code == code));
    let valeur = if code == bareme::SANS_NOTIFICATION {
        if valeur.is_some() {
            return Err(erreur(
                Classe::Refuse,
                "Avec 999ZZ (pas encore de notification), la DSN ne déclare aucun taux (norme DSN, S21.G00.40.043) : laissez le taux vide.".to_string(),
            ));
        }
        None
    } else {
        let seuil = seuil_de_vingt(ctx, an)?;
        let moins_de_20 = match &seuil {
            SeuilDeVingt::Sous => Some(true),
            SeuilDeVingt::Atteint => Some(false),
            SeuilDeVingt::Inconnu(_) => None,
        };
        let arrete = connu.as_ref().and_then(|r| bareme::taux_de_l_arrete(r, moins_de_20, en_alsace_moselle(ctx)));
        match valeur.or(arrete) {
            Some(v) if v > Decimal::ZERO && v < Decimal::from(20) => Some(v),
            Some(v) => {
                return Err(erreur(
                    Classe::Refuse,
                    format!("Taux AT/MP de {} % : il se donne en pourcentage, par exemple 1,50 pour 1,50 %.", v.normalize()),
                ))
            }
            None => {
                // Un risque de l'arrete qui n'est pas TC : son taux collectif ne vaut que sous 20 salaries, que l'effectif dit.
                let effectif = match (&seuil, connu.as_ref().is_some_and(|r| !r.tc)) {
                    (SeuilDeVingt::Inconnu(manque), true) => format!(" Le taux de l'arrêté ne vaut pour ce code que sous 20 salariés, et l'effectif ne le dit pas : {}", manque),
                    _ => String::new(),
                };
                return Err(erreur(
                    Classe::Refuse,
                    format!("Code risque {} : son taux est celui que la CARSAT vous a notifié (compte AT/MP sur net-entreprises) ; saisissez-le.{}", code, effectif),
                ));
            }
        }
    };
    ecrire(ctx, an, code, valeur, None, &premier_janvier(an)?, source)
}

/// LE CODE ET LE TAUX QUE LA CARSAT NOTIFIE (retour DSN de nature 34, `envois::retours::taux_atmp_d_octets`) se posent
/// d'eux-memes dans la fiche entreprise, a leur date d'effet, pour l'annee de cette date : la compagnie n'a rien a
/// recopier, et c'est le taux recu qui vaut a partir de la (ce qui precede garde le sien, le code compris). `notification`
/// garde le jour ou elle a paru (`le`), celui de son calcul et la date d'effet ; une notification calculee avant celle qui
/// vaut deja a cette date ne remplace rien. `Ok(Some(phrase))` : rien n'est pose, et la phrase dit pourquoi et quoi faire.
pub(crate) fn poser_notifie(ctx: &Contexte, n: &envois::retours::TauxAtmpNotifie, le: Option<Jour>, source: &str) -> R<Option<String>> {
    let an = i64::from(n.date_effet.an);
    let code = n.code_risque.clone();
    if !code_risque_valide(&code) || code == bareme::SANS_NOTIFICATION || n.taux <= Decimal::ZERO || n.taux >= Decimal::from(20) {
        return Ok(Some(format!("{} : code risque {} et taux {} illisibles ; rien n'est posé. Le code et le taux se lisent sur la notification (compte AT/MP sur net-entreprises) et se saisissent dans Configuration › Entreprise, « Accidents du travail (AT/MP) ».", source, code, n.taux)));
    }
    // La notification qui vaut deja a la date d'effet, telle que la fiche l'ecrit a cet instant.
    let fichier = ctx.racine.join(droit::contexte::ENTREPRISE);
    let fiche: droit::fiche::FicheEntrepriseDatee = droit::fiche::lire_fichier(&fichier, droit::contexte::ENTREPRISE).map_err(|m| erreur(Classe::Illisible, m))?;
    let deja = fiche.taux_atmp.as_deref().and_then(|l| droit::fiche::valeur_au(l, &n.date_effet)).and_then(|t| t.notification).and_then(|x| x.calcul);
    if let (Some(deja), Some(calcul)) = (deja, n.date_calcul) {
        if deja > calcul {
            return Ok(None);
        }
    }
    let notification = NotificationAtmp { le, calcul: n.date_calcul, effet: Some(n.date_effet) };
    ecrire(ctx, an, code, Some(n.taux), Some(notification), &n.date_effet, source)?;
    Ok(None)
}

/// Un taux saisi : `1,5`, `1.50`, `1,50 %` ; vide : rien.
pub(crate) fn taux_saisi(texte: &str) -> R<Option<Decimal>> {
    let brut = texte.replace(',', ".").replace('%', "");
    let brut = brut.trim();
    if brut.is_empty() {
        return Ok(None);
    }
    socle::num::decimal_lu(brut).map(Some).ok_or_else(|| erreur(Classe::Refuse, format!("Taux AT/MP illisible : {}.", socle::fichiers::repr(texte))))
}

/// Un risque du socle, tel que l'ecran le montre.
#[derive(Debug, Serialize)]
struct RisqueDit {
    code: String,
    code_arrete: Option<String>,
    libelle: String,
    precisions: Option<String>,
    taux: Option<f64>,
    tc: bool,
}

fn risque_dit(r: &RisqueAtmp) -> RisqueDit {
    RisqueDit { code: r.code.clone(), code_arrete: r.code_arrete.clone(), libelle: r.libelle.clone(), precisions: r.precisions.clone(), taux: r.taux.map(socle::num::flottant), tc: r.tc }
}

/// Le code et le taux poses pour l'annee.
#[derive(Debug, Serialize)]
struct TauxPose {
    valeur: Option<f64>,
    code_risque: Option<String>,
}

/// La notification d'ou viennent le code et le taux, telle qu'elle est posee (jours ISO).
#[derive(Debug, Serialize)]
struct NotificationDite {
    le: Option<String>,
    calcul: Option<String>,
    effet: Option<String>,
}

/// Le code et le taux d'un autre etablissement, tels que l'ecran les dit.
#[derive(Debug, Serialize)]
struct EtablissementDit {
    nic: Option<String>,
    libelle: Option<String>,
    resume: String,
    risque: Option<RisqueDit>,
    explication: String,
}

/// CE QUE REND `taux-atmp` : le code et le taux de l'annee, le risque tel que l'arrete l'ecrit, la phrase qui l'explique,
/// ou le lire, ou le changer, les codes que l'avance propose, et ceux des autres etablissements.
#[derive(Debug, Serialize)]
pub(crate) struct TauxAtmpDit {
    phrase: String,
    annee: i64,
    taux: Option<TauxPose>,
    risque: Option<RisqueDit>,
    nom: Option<String>,
    resume: Option<String>,
    pose_a_la_creation: bool,
    reporte_de: Option<i64>,
    notification: Option<NotificationDite>,
    explication: String,
    ou_le_lire: &'static str,
    ou_le_changer: &'static str,
    aide: &'static str,
    codes: Vec<RisqueDit>,
    etablissements: Vec<EtablissementDit>,
}

impl Dite for TauxAtmpDit {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

/// « l'arrete de tarification de l'annee » quand le socle ne nomme pas son arrete.
fn l_arrete(arrete: &Option<String>, an: i64) -> String {
    match arrete {
        Some(a) => a.clone(),
        None => format!("l'arrêté de tarification de {}", an),
    }
}

/// CE QUE L'ECRAN DIT du code et du taux de l'annee : ce qui est pose, le risque tel que l'arrete l'ecrit, la phrase qui
/// l'explique, ou le lire, ou le changer, et les codes que l'avance propose. `phrase` : ce que le geste a fait, sinon ce
/// que la lecture dit.
pub(crate) fn lire(ctx: &Contexte, an: i64, phrase: Option<&str>) -> R<TauxAtmpDit> {
    // Ce que l'annee dit, ou, l'annee que la compagnie n'a pas dite, le taux qui se reprend d'une annee d'avant, ou
    // pourquoi il ne se reprend pas (`Contexte::taux_atmp_de`).
    let (pose, reporte, manque) = match ctx.taux_atmp_de(an as i32) {
        Some(Ok(t)) => {
            let de = t.reporte_de.map(i64::from);
            (Some(t), de, None)
        }
        Some(Err(m)) => (None, None, Some(m)),
        None => (None, None, None),
    };
    let code = pose.as_ref().and_then(|t| t.code_risque.clone()).filter(|c| !c.is_empty());
    // Le taux en vigueur aujourd'hui, ramene dans l'annee (`Contexte::taux_atmp_de`).
    let valeur = pose.as_ref().and_then(|t| t.valeur);
    let notification = pose.as_ref().and_then(|t| t.notification.as_ref());
    let notification_dite = notification.map(|n| NotificationDite { le: n.le.map(|j| j.iso()), calcul: n.calcul.map(|j| j.iso()), effet: n.effet.map(|j| j.iso()) });
    let a_la_creation = pose.as_ref().is_some_and(|t| t.pose_a_la_creation == Some(true));
    let socle = socle_de(an)?;
    // Les codes que le socle de l'annee connait ; une annee sans socle n'en connait aucun.
    let risques: Vec<RisqueAtmp> = socle.iter().flat_map(bareme::risques_de).collect();
    let arrete = socle.as_ref().and_then(bareme::arrete_atmp);
    let connu = code.as_ref().and_then(|c| risques.iter().find(|r| r.code == *c));
    let nom_de = |r: &RisqueAtmp| match &r.code_arrete {
        Some(c) => c.clone(),
        None => r.code.clone(),
    };
    let nom = match connu {
        Some(r) => Some(nom_de(r)),
        None => code.clone(),
    };
    let taux_dit = valeur.map(en_pourcent);
    let taux_de_l_arrete = |r: &RisqueAtmp| r.taux.map(en_pourcent);
    let explication = match (connu, &taux_dit) {
        (Some(r), _) if r.code == bareme::SANS_NOTIFICATION => format!(
            "999ZZ : la compagnie n'a pas encore reçu de notification de la CARSAT. La DSN le déclare ainsi, sans taux. Aucun texte ne dit quel taux le bulletin cotise en attendant : le logiciel ne calcule pas la paie de {} tant que le code et le taux notifiés ne sont pas saisis ici.",
            an
        ),
        (Some(r), Some(t)) if r.tc && taux_de_l_arrete(r).as_deref() != Some(t.as_str()) => match taux_de_l_arrete(r) {
            Some(collectif) => format!(
                "Le taux de ce risque est collectif, le même pour toutes les compagnies qui y sont classées quel que soit leur effectif (code de la sécurité sociale, art. D. 242-6-14, et {}) : {} en {}. Celui que la compagnie a saisi, {}, est celui que la paie applique ; s'il n'est pas sur la notification, corrigez-le dans « Avancé ».",
                l_arrete(&arrete, an), collectif, an, t
            ),
            None => format!("Taux de {} en {}, tel que la compagnie l'a saisi.", t, an),
        },
        (Some(r), Some(t)) if r.tc => format!(
            "Le taux de ce risque est collectif : le même pour toutes les compagnies qui y sont classées, quel que soit leur effectif (code de la sécurité sociale, art. D. 242-6-14, et {}) : {} en {}.",
            l_arrete(&arrete, an), t, an
        ),
        (Some(r), Some(t)) if taux_de_l_arrete(r).as_deref() == Some(t.as_str()) => format!(
            "Ce risque a un taux collectif sous 20 salariés (code de la sécurité sociale, art. D. 242-6-2) : {} en {} ({}). À partir de 20 salariés, le taux est celui que la CARSAT notifie.",
            t, an, l_arrete(&arrete, an)
        ),
        (_, Some(t)) => format!("Taux de {} en {}, tel que la notification de la CARSAT le dit.", t, an),
        (_, None) => match (&code, manque) {
            (Some(c), _) => format!("Code risque {} : le taux de {} manque ; il est sur la notification de la CARSAT.", c, an),
            (None, Some(m)) => m,
            (None, None) => format!("Le code risque et le taux AT/MP de {} ne sont pas encore dits.", an),
        },
    };
    let explication = match reporte {
        Some(de) => format!("Le code risque est celui que la compagnie a dit pour {}. {}", de, explication),
        None => explication,
    };
    // Le taux dit s'ajoute a la phrase ; sans taux, elle s'arrete au code.
    let avec_le_taux: String = taux_dit.iter().map(|t| format!(", taux {}", t)).collect();
    let explication = match (connu.and_then(|r| r.alsace_moselle.map(|g| (r, g))), en_alsace_moselle(ctx), &notification, &nom) {
        (_, _, Some(n), Some(nom)) => {
            // Le jour de son calcul, sinon celui ou elle a paru.
            let notifiee: String = n.calcul.or(n.le).iter().map(|j| format!(" (notification du {})", j.jj_mm_aaaa())).collect();
            let effet: String = n.effet.iter().map(|j| format!(" et en vigueur depuis le {}", j.jj_mm_aaaa())).collect();
            format!(
                "Code risque {}{}, notifiés par la CARSAT{}{} : le logiciel les a lus dans la notification reçue avec les retours de la DSN, et posés de lui-même.",
                nom, avec_le_taux, notifiee, effet
            )
        }
        (Some((r, (groupe, t))), true, None, _) => format!(
            "L'établissement est en Alsace-Moselle : l'annexe 3 de {} classe {} au groupe {}, à {} en {}. Le logiciel ne pose pas ce taux de lui-même (l'article D. 242-35, qui dit à quelles compagnies il s'applique, n'est pas relevé) : celui de la paie est celui que vous saisissez, tel que la notification de la CARSAT le dit.{}",
            l_arrete(&arrete, an),
            nom_de(r),
            groupe,
            en_pourcent(t),
            an,
            taux_dit.iter().map(|t| format!(" Taux saisi : {}.", t)).collect::<String>()
        ),
        _ => explication,
    };
    let phrase = match phrase {
        Some(p) => p.to_string(),
        None => match (&pose, &code, valeur) {
            (Some(_), Some(c), Some(v)) => format!("Taux AT/MP {} : {}, code risque {}.", an, en_pourcent(v), c),
            (Some(_), Some(c), None) => format!("Taux AT/MP {} : pas de taux, code risque {}.", an, c),
            _ => format!("Aucun taux AT/MP pour {}.", an),
        },
    };
    Ok(TauxAtmpDit {
        phrase,
        annee: an,
        taux: pose.as_ref().map(|_| TauxPose { valeur: valeur.map(socle::num::flottant), code_risque: code.clone() }),
        risque: connu.map(risque_dit),
        resume: nom.as_ref().map(|n| format!("Code risque {}{}", n, avec_le_taux)),
        nom,
        pose_a_la_creation: a_la_creation,
        reporte_de: reporte,
        notification: notification_dite,
        explication,
        ou_le_lire: "La CARSAT notifie à la compagnie son code risque et son taux : la notification se lit dans le compte AT/MP de net-entreprises (compte entreprise › Accidents du travail et maladies professionnelles).",
        ou_le_changer: "Si la notification dit un autre code, ou un autre taux, ou si la compagnie n'en a pas encore reçu (999ZZ), changez-les dans « Avancé » ci-dessous.",
        aide: "Le taux vide : celui de l'arrêté de l'année, quand il s'applique au code choisi. Avec 999ZZ, pas de taux.",
        codes: risques.iter().map(risque_dit).collect(),
        etablissements: ctx.etablissements_dits().iter().map(|e| d_un_etablissement(e, an, socle.as_ref(), &arrete)).collect::<R<Vec<_>>>()?,
    })
}

/// CE QUE L'ECRAN DIT du code et du taux d'un autre etablissement pour l'annee `an` : le taux est determine par
/// etablissement, le classement suivant l'activite qui y est exercee (D. 242-6-1, `code-de-la-securite-sociale/xml/D242-6-1.xml` ;
/// arrete du 17/10/1995, art. 1, I, 1°) ; un code « TC » a le taux de l'arrete de l'annee quel que soit l'effectif
/// (D. 242-6-14, I), lu au meme endroit que pour la compagnie (`bareme::taux_atmp_applique`). Un etablissement qui ne dit
/// pas son code n'en a pas : l'ecran le dit.
fn d_un_etablissement(e: &droit::fiche::Etablissement, an: i64, socle: Option<&droit::forme_bareme::Pile>, arrete: &Option<String>) -> R<EtablissementDit> {
    let nic = e.nic.portee().map(String::from);
    let libelle = e.libelle.portee().map(String::from);
    let Some(t) = bareme::taux_atmp_d_un_etablissement(e, an as i32)? else {
        return Ok(EtablissementDit { nic, libelle, resume: "Code risque non dit".to_string(), risque: None, explication: "Le code risque de cet établissement n'est pas dit : saisissez celui que la CARSAT lui a notifié.".to_string() });
    };
    let code = t.code_risque.clone().filter(|c| !c.is_empty());
    let risques: Vec<RisqueAtmp> = socle.into_iter().flat_map(bareme::risques_de).collect();
    let r = code.as_ref().and_then(|c| risques.iter().find(|r| r.code == *c));
    let nom_code = match (r.and_then(|r| r.code_arrete.clone()), &code) {
        (Some(n), _) => n,
        (None, Some(c)) => c.clone(),
        (None, None) => "non dit".to_string(),
    };
    let taux = socle.and_then(|_| bareme::taux_atmp_applique(&risques, &t, bareme::en_alsace_moselle(e.code_postal.portee())));
    let arrete = l_arrete(arrete, an);
    let explication = match (&taux, t.valeur) {
        (Some(_), Some(_)) => "Le taux est celui que vous avez saisi, tel que la notification de la CARSAT le dit.".to_string(),
        (Some(x), None) if r.is_some_and(|r| r.tc) => format!(
            "Le taux de ce risque est collectif, le même quel que soit l'effectif (code de la sécurité sociale, art. D. 242-6-14, et {}) : {} en {}.",
            arrete, en_pourcent(*x), an
        ),
        (Some(x), None) => format!(
            "Un établissement nouvellement créé a le taux collectif de son risque l'année de sa création et les deux années suivantes, quel que soit l'effectif (code de la sécurité sociale, art. D. 242-6-17, et {}) : {} en {}. Si la CARSAT notifie un autre taux à cet établissement, saisissez-le.",
            arrete, en_pourcent(*x), an
        ),
        (None, _) if code.as_deref() == Some(bareme::SANS_NOTIFICATION) => "999ZZ : pas encore de notification ; la paie de cet établissement attend le code et le taux notifiés.".to_string(),
        (None, _) => format!("Son taux est celui que la CARSAT notifie à cet établissement (compte AT/MP sur net-entreprises) : saisissez-le avec le code {}.", nom_code),
    };
    let resume = match taux {
        Some(x) => format!("Code risque {}, taux {}", nom_code, en_pourcent(x)),
        None => format!("Code risque {}", nom_code),
    };
    Ok(EtablissementDit { nic, libelle, resume, risque: r.map(risque_dit), explication })
}

/// La charge de `taux-atmp` : l'annee ; pour poser, le code risque et le taux.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeTaux {
    annee: i64,
    code_risque: Option<String>,
    valeur: Option<TauxSaisi>,
}

/// Un taux saisi : un nombre, ou un texte qui en dit un (« 1,50 % »).
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum TauxSaisi {
    Nombre(f64),
    Texte(String),
}

/// `taux-atmp` : l'annee seule lit ; avec un code ou un taux, pose d'abord (`poser`), puis lit. Un taux donne seul garde
/// le code deja pose pour l'annee ; sans code pose, il se refuse.
pub(crate) fn route(ctx: &Contexte, c: ChargeTaux) -> R<TauxAtmpDit> {
    let an = c.annee;
    let valeur = match &c.valeur {
        Some(TauxSaisi::Nombre(x)) => Some(socle::num::dec_f64(*x).map_err(|m| erreur(Classe::Refuse, format!("Taux AT/MP illisible : {}.", m)))?),
        Some(TauxSaisi::Texte(t)) => taux_saisi(t)?,
        None => None,
    };
    let code = match (c.code_risque.filter(|c| !c.trim().is_empty()), valeur) {
        (Some(code), _) => Some(code),
        (None, Some(_)) => match ctx.taux_atmp_de(an as i32) {
            Some(Ok(t)) if t.reporte_de.is_none() => t.code_risque.filter(|c| !c.is_empty()),
            _ => None,
        }
        .map(Some)
        .ok_or_else(|| erreur(Classe::Manque, "Le code risque manque : il se dit avec le taux, tel que la notification de la CARSAT les écrit."))?,
        (None, None) => None,
    };
    let phrase = match code {
        Some(code) => {
            let source = format!("Saisi dans PuppetPaie le {} (notification AT/MP).", aujourdhui_jj_mm_aaaa());
            poser(ctx, an, &code, valeur, &source)?;
            Some("Enregistré.")
        }
        None if c.valeur.is_some() => Some("Ni code ni taux : rien n'est écrit."),
        None => None,
    };
    lire(ctx, an, phrase)
}

/// L'etablissement de la fiche est-il en Alsace-Moselle (`droit::bareme::en_alsace_moselle`, par son code postal) ?
fn en_alsace_moselle(ctx: &Contexte) -> bool {
    bareme::en_alsace_moselle(ctx.entreprise.code_postal.portee())
}
