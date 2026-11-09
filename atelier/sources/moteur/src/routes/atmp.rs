//! LE CODE RISQUE ET LE TAUX AT/MP DE LA COMPAGNIE (`taux-atmp`), dans sa surcouche du bareme
//! (`config/baremes/<annee>.json`, `taux_atmp` : `valeur`, `code_risque`).
//!
//! Ce que les textes disent (`atelier/reference/sources/tarification-atmp/LISEZ-MOI.md`) : la CARSAT notifie le code risque
//! et le taux ; l'employeur ne declare qu'un code notifie, et 999ZZ tant qu'il n'a pas de notification, sans taux (norme DSN,
//! S21.G00.40.040 et .043, `norme-dsn/texte/bloc-S21.G00.40.txt` l. 1224-1315). Le taux d'un risque « TC » de l'annexe 1 de
//! l'arrete de l'annee est collectif quel que soit l'effectif (D. 242-6-14, I ; art. 5 de l'arrete) ; celui d'un autre risque
//! ne l'est que sous 20 salaries (D. 242-6-2).
//!
//! A LA CREATION d'une compagnie, le risque par defaut du socle de l'annee (`risques_atmp.par_defaut`, 92.3AD : « tout
//! intermittent du spectacle ») et son taux de l'arrete se posent d'eux-memes (`poser_a_la_creation`) ; l'ecran les dit
//! une fois, avec le libelle de l'arrete, l'explication, ou les lire et ou les changer (`lire`). Une annee que la compagnie
//! n'a pas dite reprend son code, et le taux de l'arrete de l'annee pour un risque TC (`Contexte::taux_atmp_reporte`). L'avance de l'ecran propose
//! les codes que le socle connait, ou un autre code notifie.

use droit::fiche::Lue as _;
use std::path::Path;

use serde::{Deserialize, Serialize};
// La surcouche du bareme de l'annee ne s'ecrit pas en sa forme (`ecrire`) : ses cles se gardent en JSON.
use serde_json::{json, Value};

use crate::routes::enveloppe::Dite;
use droit::bareme::{self, RisqueAtmp};
use droit::contexte::Contexte;
use socle::echec::{erreur, Classe, R};
use socle::ecriture::ecrire_json;
use crate::valeur::vrai;
use socle::num::Decimal;
use crate::valeur::nom_type;

fn surcouche(racine: &Path, an: i64) -> std::path::PathBuf {
    racine.join(droit::contexte::BAREMES).join(format!("{}.json", an))
}

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

/// Moins de 20 salaries (D. 242-6-2, 1°), d'apres l'effectif que le moteur connait ; `None` s'il ne le sait pas.
fn moins_de_20(ctx: &Contexte, an: i64) -> Option<bool> {
    ctx.effectif().ok()?.atteint(20, an as i32).ok().map(|atteint| !atteint)
}

/// Ecrit `taux_atmp` dans la surcouche de l'annee ; le reste de la surcouche est garde.
///
/// La surcouche (`droit::forme_bareme::Surcouche`) se lit en sa forme mais ne s'ecrit pas : ses autres cles sont gardees
/// telles qu'elles sont ecrites, et seul `taux_atmp` se pose, en sa forme (`TauxAEcrire`).
fn ecrire(racine: &Path, an: i64, taux: &TauxAEcrire) -> R<()> {
    let p = surcouche(racine, an);
    let lu = if p.exists() { crate::routes::mois::lire_saisie(&p)? } else { json!({}) };
    let mut data = if vrai(&lu) { lu } else { json!({"annee": an, "socle": an}) };
    let Value::Object(d) = &mut data else {
        return Err(erreur(Classe::Illisible, socle::fichiers::attendu("un objet", nom_type(&data))));
    };
    d.insert("taux_atmp".into(), socle::ecriture::en_valeur(taux));
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| erreur(Classe::Disque, socle::echec::du_disque(&parent, &e)))?;
    }
    if p.exists() {
        crate::routes::mois::sauver_avant(&p)?;
    }
    ecrire_json(&p, &data, 2).map_err(|m| erreur(Classe::Disque, m))
}

/// LE TAUX AT/MP TEL QU'IL S'ECRIT dans la surcouche de l'annee (`droit::forme_bareme::TauxAtmpEcrit` le lit) : le taux
/// (rien avec 999ZZ), le code, d'ou il vient, et s'il a ete pose a la creation ou lu dans une notification.
#[derive(Debug, Serialize)]
struct TauxAEcrire {
    #[serde(skip_serializing_if = "Option::is_none")]
    valeur: Option<ValeurAEcrire>,
    code_risque: String,
    note: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pose_a_la_creation: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    notification: Option<NotificationDite>,
}

/// Le taux de l'annee, ou ses valeurs datees.
#[derive(Debug, Serialize)]
#[serde(untagged)]
enum ValeurAEcrire {
    Seule(f64),
    Datee(Vec<ValeurDatee>),
}

/// Une valeur du taux, a sa date d'effet.
#[derive(Debug, Serialize)]
struct ValeurDatee {
    depuis: String,
    valeur: f64,
}

/// A LA CREATION D'UNE COMPAGNIE : le risque par defaut du socle de l'annee et le taux que l'arrete lui fixe. Une annee dont
/// le socle n'a pas de table des risques (son arrete n'est pas depose) ne pose rien : « Pour demarrer » le demande.
pub(crate) fn poser_a_la_creation(racine: &Path, an: i64) -> R<()> {
    let Some(socle) = socle_de(an)? else { return Ok(()) };
    let Some(r) = bareme::risque_par_defaut(&socle) else { return Ok(()) };
    let Some(t) = bareme::taux_de_l_arrete(&r, None, false) else { return Ok(()) };
    ecrire(
        racine,
        an,
        &TauxAEcrire {
            valeur: Some(ValeurAEcrire::Seule(socle::num::flottant(t))),
            code_risque: r.code,
            note: format!("Pose a la creation de la compagnie : le risque par defaut du socle {} et son taux collectif (atelier/reference/sources/tarification-atmp/).", an),
            pose_a_la_creation: Some(true),
            notification: None,
        },
    )
}

/// POSE LE CODE RISQUE ET LE TAUX DE L'ANNEE `an`. `valeur` vide : le taux de l'arrete s'il s'applique au code (un risque
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
        match valeur.or_else(|| connu.as_ref().and_then(|r| bareme::taux_de_l_arrete(r, moins_de_20(ctx, an), en_alsace_moselle(ctx)))) {
            Some(v) if v > Decimal::ZERO && v < Decimal::from(20) => Some(v),
            Some(v) => {
                return Err(erreur(
                    Classe::Refuse,
                    format!("Taux AT/MP de {} % : il se donne en pourcentage, par exemple 1,50 pour 1,50 %.", v.normalize()),
                ))
            }
            None => {
                return Err(erreur(
                    Classe::Refuse,
                    format!("Code risque {} : son taux est celui que la CARSAT vous a notifié (compte AT/MP sur net-entreprises) ; saisissez-le.", code),
                ))
            }
        }
    };
    ecrire(
        &ctx.racine,
        an,
        &TauxAEcrire { valeur: valeur.map(|v| ValeurAEcrire::Seule(socle::num::flottant(v))), code_risque: code, note: source.to_string(), pose_a_la_creation: None, notification: None },
    )
}

/// LE CODE ET LE TAUX QUE LA CARSAT NOTIFIE (retour DSN de nature 34, `envois::retours::taux_atmp_d_octets`) se posent
/// d'eux-memes dans la surcouche de l'annee de leur date d'effet : la compagnie n'a rien a recopier. Effet au 1er janvier :
/// le taux de l'annee. Effet en cours d'annee : `valeur` devient la liste datee `[{depuis, valeur}]`, le taux d'avant
/// (dit pour l'annee, ou repris) jusqu'a la date d'effet (`droit::bareme::taux_atmp_au`, et le bareme a la date). `notification`
/// garde le jour de la notification et la date d'effet ; une notification plus ancienne que celle deja posee ne remplace
/// rien. `Ok(Some(phrase))` : rien n'est pose, et la phrase dit pourquoi et quoi faire (un changement de code en cours
/// d'annee, que la configuration ne date pas).
pub(crate) fn poser_notifie(ctx: &Contexte, n: &envois::retours::TauxAtmpNotifie, le: &str, source: &str) -> R<Option<String>> {
    let an = n.date_effet.an as i64;
    let code = n.code_risque.clone();
    if !code_risque_valide(&code) || code == bareme::SANS_NOTIFICATION || n.taux <= Decimal::ZERO || n.taux >= Decimal::from(20) {
        return Ok(Some(format!("{} : code risque {} et taux {} illisibles ; rien n'est posé. Le code et le taux se lisent sur la notification (compte AT/MP sur net-entreprises) et se saisissent dans Configuration › Entreprise, « Accidents du travail (AT/MP) ».", source, code, n.taux)));
    }
    // Le taux que l'annee dit (ou celui qu'elle reprend), et la notification deja posee pour elle.
    let avant = match ctx.taux_atmp_de(an as i32) {
        Some(Ok(t)) => Some(t),
        Some(Err(m)) => return Err(erreur(Classe::Illisible, m)),
        None => None,
    };
    let calcul = n.date_calcul.map(|j| j.iso());
    let deja = avant.as_ref().filter(|t| t.reporte_de.is_none()).and_then(|t| t.notification.as_ref()).and_then(|x| x.calcul.as_deref());
    if let (Some(deja), Some(calcul)) = (deja, &calcul) {
        if deja > calcul.as_str() {
            return Ok(None);
        }
    }
    let premier_janvier = n.date_effet.mois == 1 && n.date_effet.jour == 1;
    let valeur = if premier_janvier {
        ValeurAEcrire::Seule(socle::num::flottant(n.taux))
    } else {
        // Le taux d'avant la date d'effet : celui que l'annee dit, sinon celui qu'elle reprend.
        let code_avant = avant.as_ref().and_then(|t| t.code_risque.clone()).filter(|c| !c.is_empty());
        if let Some(c) = code_avant.filter(|c| *c != code) {
            return Ok(Some(format!(
                "{} : le code risque passe de {} à {} au {} ; rien n'est posé, car la configuration ne date pas le code risque. Saisissez le code et le taux notifiés dans Configuration › Entreprise, « Accidents du travail (AT/MP) ».",
                source, c, code, n.date_effet.jj_mm_aaaa()
            )));
        }
        let effet = n.date_effet.iso();
        // Une annee qui ne dit aucun taux d'avant : la liste commence a la date d'effet.
        let mut liste: Vec<ValeurDatee> = avant
            .as_ref()
            .and_then(|t| t.valeur.as_ref())
            .into_iter()
            .flat_map(|v| match v {
                droit::forme_bareme::ValeurAtmp::Datee(_) => v
                    .valeurs()
                    .into_iter()
                    .filter_map(|(d, x)| d.filter(|d| *d < effet.as_str()).map(|d| ValeurDatee { depuis: d.to_string(), valeur: socle::num::flottant(x) }))
                    .collect::<Vec<_>>(),
                droit::forme_bareme::ValeurAtmp::Seule(x) => vec![ValeurDatee { depuis: format!("{}-01-01", an), valeur: socle::num::flottant(x.0) }],
            })
            .collect();
        liste.push(ValeurDatee { depuis: effet, valeur: socle::num::flottant(n.taux) });
        ValeurAEcrire::Datee(liste)
    };
    ecrire(
        &ctx.racine,
        an,
        &TauxAEcrire {
            valeur: Some(valeur),
            code_risque: code,
            note: source.to_string(),
            pose_a_la_creation: None,
            notification: Some(NotificationDite { le: le.to_string(), calcul, effet: n.date_effet.iso() }),
        },
    )?;
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

/// La notification d'ou viennent le code et le taux, telle qu'elle est posee.
#[derive(Debug, Serialize)]
struct NotificationDite {
    le: String,
    calcul: Option<String>,
    effet: String,
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

/// Un jour ISO en mots (« 02/11/2026 », `routes::quand`).
fn jour_dit(iso: &str) -> String {
    crate::routes::quand(iso)
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
    // Le taux en vigueur aujourd'hui, ramene dans l'annee (une liste datee : celui de sa date d'effet).
    let jour = match ctx.aujourdhui.an as i64 {
        a if a < an => socle::date::Jour::nouveau(an as i32, 1, 1),
        a if a > an => socle::date::Jour::nouveau(an as i32, 12, 31),
        _ => Some(ctx.aujourdhui),
    };
    let valeur = pose.as_ref().and_then(|t| t.valeur.as_ref()).zip(jour).and_then(|(v, j)| v.au(&j));
    let notification = pose.as_ref().and_then(|t| t.notification.as_ref()).map(|n| NotificationDite { le: n.le.clone(), calcul: n.calcul.clone(), effet: n.effet.clone() });
    let a_la_creation = pose.as_ref().is_some_and(|t| t.pose_a_la_creation == Some(true));
    let socle = socle_de(an)?;
    let risques = match &socle {
        Some(s) => bareme::risques_de(s),
        None => Vec::new(),
    };
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
    let avec_le_taux = match &taux_dit {
        Some(t) => format!(", taux {}", t),
        None => String::new(),
    };
    let explication = match (connu.and_then(|r| r.alsace_moselle.map(|g| (r, g))), en_alsace_moselle(ctx), &notification, &nom) {
        (_, _, Some(n), Some(nom)) => {
            let notifiee = match &n.calcul {
                Some(c) => c,
                None => &n.le,
            };
            format!(
                "Code risque {}{}, notifiés par la CARSAT (notification du {}) et en vigueur depuis le {} : le logiciel les a lus dans la notification reçue avec les retours de la DSN, et posés de lui-même.",
                nom,
                avec_le_taux,
                jour_dit(notifiee),
                jour_dit(&n.effet)
            )
        }
        (Some((r, (groupe, t))), true, None, _) => format!(
            "L'établissement est en Alsace-Moselle : l'annexe 3 de {} classe {} au groupe {}, à {} en {}. Le logiciel ne pose pas ce taux de lui-même (l'article D. 242-35, qui dit à quelles compagnies il s'applique, n'est pas relevé) : celui de la paie est celui que vous saisissez, tel que la notification de la CARSAT le dit.{}",
            l_arrete(&arrete, an),
            nom_de(r),
            groupe,
            en_pourcent(t),
            an,
            match &taux_dit {
                Some(t) => format!(" Taux saisi : {}.", t),
                None => String::new(),
            }
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
        notification,
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
    let risques = match socle {
        Some(s) => bareme::risques_de(s),
        None => Vec::new(),
    };
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
