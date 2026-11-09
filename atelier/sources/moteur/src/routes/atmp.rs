//! LE CODE RISQUE ET LE TAUX AT/MP DE LA COMPAGNIE : une case de la configuration, comme les autres. Le siege l'a dans la
//! fiche entreprise (`taux_atmp` : code risque, taux, annee, et la notification d'ou ils viennent), chaque autre
//! etablissement sur sa ligne (`routes::etablissements`) ; datee comme toute case.
//!
//! Quatre sources l'ecrivent, toutes dans cette case :
//! - l'import de la compagnie : le code et le taux que declarent ses DSN reprises (S21.G00.40.040, .043), pour leur annee
//!   (`import::parametrage`) ;
//! - la notification recue avec les retours de la DSN (`recue`) : differente de la case, elle s'y inscrit a sa date
//!   d'effet ; la meme, il ne se passe rien ;
//! - la saisie, par le chemin ordinaire des fiches (`enregistrer-fiche`) : toujours possible ; si elle change un taux recu
//!   par la DSN, l'avertissement le dit (`droit::bareme::taux_atmp_a_confirmer`), et confirmee, elle vaut ;
//! - le taux collectif de l'arrete de l'annee pour un risque « TC » (annexe 1, D. 242-6-14) : une annee que la case ne
//!   dit pas, le bareme le prend, avec sa provenance (`droit::bareme::taux_atmp_de_l_annee`).
//!
//! Sans aucune d'elles, le taux de l'annee manque : « Pour demarrer » et le calcul le demandent, comme toute case vide.
//! Ce que les textes disent (`atelier/reference/sources/tarification-atmp/LISEZ-MOI.md`) : la CARSAT notifie le code
//! risque et le taux, pour une annee (D. 242-6-15) ; l'employeur declare 999ZZ tant qu'il n'a pas de notification, sans
//! taux (norme DSN, S21.G00.40.040 et .043, `norme-dsn/texte/bloc-S21.G00.40.txt` l. 1224-1315).
//!
//! La route `taux-atmp` dit ce que la case donne pour une annee, d'ou cela vient, le risque tel que l'arrete l'ecrit,
//! les codes que le socle connait (ce que l'ecran propose) et ce qui vaut pour les autres etablissements.

use droit::fiche::Lue as _;

use serde::{Deserialize, Serialize};

use crate::routes::enveloppe::Dite;
use droit::bareme::{self, RisqueAtmp};
use droit::contexte::Contexte;
use droit::fiche::{Montant, NotificationAtmp, TauxAtmp};
use socle::date::Jour;
use socle::echec::{erreur, Classe, R};
use socle::num::Decimal;

/// Le socle livre de l'annee, s'il existe, lu par sa forme (`droit::contexte::socle_livre_lu`) ; un socle livre qui ne se
/// lit pas se refuse.
fn socle_de(an: i64) -> R<Option<droit::forme_bareme::Pile>> {
    droit::contexte::socle_livre_lu(an as i32).map_err(|m| erreur(Classe::Illisible, m))
}

fn en_pourcent(t: Decimal) -> String {
    let mut t = t.round_dp(2);
    t.rescale(2);
    format!("{} %", t.to_string().replace('.', ","))
}

/// LE CODE ET LE TAUX QUE LA CARSAT NOTIFIE (retour DSN de nature 34, `envois::retours::taux_atmp_d_octets`) : s'ils
/// different de ce que la case du siege dit a leur date d'effet (code, taux ou annee), ils s'y inscrivent a cette date,
/// pour l'annee de cette date, avec la notification (`le` : le jour ou elle a paru) ; les memes, rien ne s'ecrit.
/// `Ok(Some(phrase))` : rien n'est inscrit, et la phrase dit pourquoi et quoi faire.
pub(crate) fn recue(ctx: &Contexte, n: &envois::retours::TauxAtmpNotifie, le: Option<Jour>, source: &str) -> R<Option<String>> {
    let t = TauxAtmp {
        code_risque: Some(n.code_risque.clone()),
        valeur: Some(Montant::Flottant(socle::num::flottant(n.taux))),
        annee: Some(i64::from(n.date_effet.an)),
        notification: Some(NotificationAtmp { le, calcul: n.date_calcul, effet: Some(n.date_effet) }),
    };
    if n.code_risque == bareme::SANS_NOTIFICATION {
        return Ok(Some(format!("{} : code risque {} ; rien n'est inscrit. Le code et le taux se lisent sur la notification (compte AT/MP sur net-entreprises) et s'écrivent dans Configuration › Entreprise, « Accidents du travail (AT/MP) ».", source, n.code_risque)));
    }
    if let Some(m) = bareme::taux_atmp_refuse(&t).map_err(|m| erreur(Classe::Illisible, m))? {
        return Ok(Some(format!("{} : {} Rien n'est inscrit ; le code et le taux de la notification (compte AT/MP sur net-entreprises) s'écrivent dans Configuration › Entreprise, « Accidents du travail (AT/MP) ».", source, m)));
    }
    let deja = ctx.entreprise_au(&n.date_effet).taux_atmp;
    if deja.is_some_and(|d| bareme::meme_taux_atmp(&d, &t)) {
        return Ok(None);
    }
    crate::routes::fiches::inscrire_taux_atmp(ctx, t, n.date_effet, source)?;
    Ok(None)
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

/// Le code et le taux d'un autre etablissement, tels que l'ecran les dit.
#[derive(Debug, Serialize)]
struct EtablissementDit {
    nic: Option<String>,
    libelle: Option<String>,
    resume: String,
    risque: Option<RisqueDit>,
    explication: String,
}

/// CE QUE REND `taux-atmp` : le code et le taux de l'annee (la phrase les dit), le risque tel que l'arrete l'ecrit, d'ou
/// ils viennent ou ce qui manque, ou les lire, les codes que le socle connait, et ceux des autres etablissements.
#[derive(Debug, Serialize)]
pub(crate) struct TauxAtmpDit {
    phrase: String,
    annee: i64,
    code_risque: Option<String>,
    taux: Option<f64>,
    risque: Option<RisqueDit>,
    explication: String,
    ou_le_lire: &'static str,
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

/// La charge de `taux-atmp` : l'annee.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeTaux {
    annee: i64,
}

/// `taux-atmp` : ce que la case du siege donne pour l'annee (`Contexte::taux_atmp_de`), et d'ou cela vient.
pub(crate) fn route(ctx: &Contexte, c: ChargeTaux) -> R<TauxAtmpDit> {
    let an = c.annee;
    let lu = ctx.taux_atmp_de(an as i32);
    let (pose, manque) = match lu {
        Some(Ok(t)) => (Some(t), None),
        Some(Err(m)) => (None, Some(m)),
        None => (None, None),
    };
    let code = pose.as_ref().and_then(|t| t.code_risque.clone()).filter(|c| !c.is_empty());
    let valeur = pose.as_ref().and_then(|t| t.valeur);
    let socle = socle_de(an)?;
    // Les codes que le socle de l'annee connait ; une annee sans socle n'en connait aucun.
    let risques: Vec<RisqueAtmp> = socle.iter().flat_map(bareme::risques_de).collect();
    let arrete = socle.as_ref().and_then(bareme::arrete_atmp);
    let connu = code.as_ref().and_then(|c| risques.iter().find(|r| r.code == *c));
    let nom = match connu {
        Some(r) => r.code_arrete.clone().or_else(|| Some(r.code.clone())),
        None => code.clone(),
    };
    let explication = match (&pose, valeur) {
        (Some(t), Some(v)) => match (&t.notification, &t.provenance) {
            (Some(n), _) => {
                // Le jour de son calcul, sinon celui ou elle a paru.
                let notifiee: String = n.calcul.or(n.le).iter().map(|j| format!(" (notification du {})", j.jj_mm_aaaa())).collect();
                let effet: String = n.effet.iter().map(|j| format!(", en vigueur depuis le {}", j.jj_mm_aaaa())).collect();
                format!("Notifiés par la CARSAT{}{} : le logiciel les a lus dans les retours de la DSN.", notifiee, effet)
            }
            (None, Some(p)) => format!(
                "{} Il est le même pour toutes les compagnies qui y sont classées, quel que soit leur effectif ({}) : {} en {}. Si votre notification en dit un autre, écrivez-le avec son année.",
                p,
                l_arrete(&arrete, an),
                en_pourcent(v),
                an
            ),
            (None, None) => format!("Taux de {} en {}, tel que la configuration le dit.", en_pourcent(v), an),
        },
        (Some(_), None) => format!(
            "999ZZ : la compagnie n'a pas encore reçu de notification de la CARSAT. La DSN le déclare ainsi, sans taux. Aucun texte ne dit quel taux le bulletin cotise en attendant : le logiciel ne calcule pas la paie de {} tant que le code et le taux notifiés ne sont pas écrits.",
            an
        ),
        (None, _) => match manque {
            Some(m) => m,
            None => format!("Le code risque et le taux AT/MP de {} ne sont pas dits : ils sont sur la notification de la CARSAT, et s'écrivent ci-dessus avec leur année.", an),
        },
    };
    let phrase = match (&nom, valeur) {
        (Some(n), Some(v)) => format!("Taux AT/MP {} : {}, code risque {}.", an, en_pourcent(v), n),
        (Some(n), None) => format!("Taux AT/MP {} : à écrire, code risque {}.", an, n),
        (None, _) => format!("Taux AT/MP {} : pas encore dit.", an),
    };
    Ok(TauxAtmpDit {
        phrase,
        annee: an,
        code_risque: code,
        taux: valeur.map(socle::num::flottant),
        risque: connu.map(risque_dit),
        explication,
        ou_le_lire: "La CARSAT notifie à la compagnie son code risque et son taux : la notification se lit dans le compte AT/MP de net-entreprises (compte entreprise › Accidents du travail et maladies professionnelles).",
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
