//! `S21.G00.30.013` (codification UE) : quatre valeurs (`CodeNationalite`), et la table qui les donne depuis le pays de
//! nationalite.
//!
//! LA NORME (cahier technique DSN 2026.1, p. 173, `atelier/reference/norme-dsn/texte/bloc-S21.G00.30.txt`, l. 267-277) :
//! « Classification de l'origine du salarié au vu des frontières françaises et des limites de l'Union Européenne. Il
//! faut renseigner dans cette rubrique le code le plus précis applicable à l'individu » ; « 01 - France / 02 - UE /
//! 03 - EEE / 04 - Reste du Monde ». « Le plus precis » : la France est dans l'Union, l'Union dans l'EEE ; 03 ne vaut
//! donc que pour un Etat de l'EEE hors de l'Union. Lire « origine » comme la nationalite est une INFERENCE (la
//! norme ne dit pas « nationalite »).
//!
//! LES ETATS (`atelier/reference/sources/union-europeenne/LISEZ-MOI.md`) : l'Union, traite sur l'Union europeenne,
//! art. 52 § 1, version consolidee (JO C 202 du 07/06/2016), moins le Royaume-Uni, sorti par l'accord de retrait
//! (JO L 29 du 31/01/2020 ; fin de la periode de transition le 31/12/2020, art. 126) ; l'EEE hors Union : Islande,
//! Liechtenstein, Norvege (synthese EUR-Lex de l'accord EEE, qui dit aussi que la Suisse l'a rejete). Le Royaume-Uni
//! et la Suisse ne sont ni dans l'une ni dans l'autre liste : 04 (INFERENCE par absence, la norme ne les nomme pas).
//! Les iles Feroe, dont le pays de rattachement est le Danemark, sont hors de l'Union par le texte (TFUE art. 355 § 5 a) : 04.
//!
//! LA CODIFICATION UE d'un pays de la liste (`socle::pays`) se lit sur son code, ou sur celui de son pays de
//! rattachement pour un territoire (Groenland : Danemark ; Aruba, Curacao : Pays-Bas ; Gibraltar : Royaume-Uni ; Hong
//! Kong : Chine). Ce rattachement vaut nationalite : c'est une INFERENCE (le COG dit le rattachement, pas la nationalite
//! de ceux qui y naissent). Les Feroe font exception (`TERRITOIRES_HORS_UNION`).
//!
//! UN 03 SANS PAYS DE NATIONALITE ne dit pas si le salarie est de l'EEE ou du reste du monde : le moteur recode depuis
//! le pays quand la fiche le porte (`pays_nationalite`), il refuse sinon.
//!
//! UN PAYS HORS DE LA LISTE : la fiche l'ecrit en `pays_nationalite_hors_liste` (le nom, en texte), et la codification
//! est celle que l'administrateur choisit en `code_nationalite_dsn`, sans pays de la liste a cote. La DSN ne declare
//! pas le pays de nationalite, seulement `30.013` : rien a inventer. Un pays de la liste ecrit la ne compte pas comme
//! hors liste (il se choisit dans la liste, la codification s'en deduit) ; la France non plus (01 refuse).

use serde::Serialize;
use socle::echec::{refus, R};
use socle::pays::{CodePays, Pays};

use crate::fiche::CodeNationalite;

/// Les Etats membres de l'Union (TUE art. 52 § 1, sans le Royaume-Uni), en codes ISO a deux lettres, la France a part.
pub const UNION: [&str; 26] = [
    "BE", "BG", "CZ", "DK", "DE", "EE", "IE", "GR", "ES", "HR", "IT", "CY", "LV", "LT", "LU", "HU", "MT", "NL", "AT",
    "PL", "PT", "RO", "SI", "SK", "FI", "SE",
];

/// Les territoires que les traites excluent de l'Union alors que leur pays de rattachement en est : les Feroe (TFUE art. 355
/// § 5 a), « les traites ne s'appliquent pas aux iles Feroe » ; protocole n° 2 de l'acte d'adhesion de 1972, art. 4 : les
/// ressortissants danois qui y resident ne sont pas des ressortissants d'un Etat membre ; hors de l'EEE par l'art. 126 § 1
/// de l'accord : `atelier/reference/sources/union-europeenne/LISEZ-MOI.md`, « Les iles Feroe »).
pub const TERRITOIRES_HORS_UNION: [&str; 1] = ["FO"];

/// Les Etats parties a l'accord EEE qui ne sont pas dans l'Union.
pub const EEE_HORS_UNION: [&str; 3] = ["IS", "LI", "NO"];

/// Le code de `S21.G00.30.013` d'un pays (son code ISO).
pub fn code_du_pays(iso: &CodePays) -> CodeNationalite {
    if *iso == CodePays::france() {
        CodeNationalite::Francaise
    } else if UNION.contains(&&**iso) {
        CodeNationalite::UnionEuropeenne
    } else if EEE_HORS_UNION.contains(&&**iso) {
        CodeNationalite::HorsUnionEuropeenne
    } else {
        CodeNationalite::Inconnue
    }
}

/// La codification UE (`S21.G00.30.013`) d'un pays de la liste : celle de son code, ou de son pays de rattachement.
pub fn codification(p: &Pays) -> CodeNationalite {
    if TERRITOIRES_HORS_UNION.contains(&&*p.code) {
        return code_du_pays(&p.code);
    }
    match &p.rattachement {
        Some(r) => code_du_pays(r),
        None => code_du_pays(&p.code),
    }
}

/// Un pays de nationalite tel que la page le recoit : son code, son nom, la codification UE que le moteur en deduit, et
/// ce que la case de nationalite dit quand elle s'en deduit.
#[derive(Debug, Clone, Serialize)]
pub struct PaysDeNationalite {
    pub code: CodePays,
    pub nom: String,
    pub codification_ue: CodeNationalite,
    pub deduit_dit: String,
}

/// Ce que la page recoit (`etat.pays`) : la liste triee par nom, chaque pays avec la codification UE que le moteur
/// en deduit. La page l'affiche ; elle ne decide rien.
pub fn servir() -> R<Vec<PaysDeNationalite>> {
    let mut l = (*socle::pays::liste()?).clone();
    l.sort_by_cached_key(|p| socle::pays::cle_de_nom(&p.nom));
    Ok(l.iter()
        .map(|p| PaysDeNationalite { code: p.code.clone(), nom: p.nom.clone(), codification_ue: codification(p), deduit_dit: format!("déduite de la nationalité ({})", p.nom) })
        .collect())
}

/// CE QU'UNE FICHE DIT DE LA NATIONALITE : le code choisi (`code_nationalite_dsn`), le pays de la liste
/// (`pays_nationalite`), le pays ecrit hors de la liste (`pays_nationalite_hors_liste`, sans blancs), chacun `None`
/// quand elle ne le dit pas ; et qui elle nomme, pour les refus.
#[derive(Debug, Clone)]
pub struct Nationalite {
    pub code: Option<CodeNationalite>,
    pub pays: Option<CodePays>,
    pub hors: Option<String>,
    pub qui: String,
}

impl Nationalite {
    pub fn nouvelle(code: Option<CodeNationalite>, pays: Option<CodePays>, hors: Option<&str>, qui: Option<String>) -> Nationalite {
        let hors = hors.map(str::trim).filter(|t| !t.is_empty()).map(str::to_string);
        let qui = match qui {
            Some(nomme) => nomme,
            None => "ce salarié".to_string(),
        };
        Nationalite { code, pays, hors, qui }
    }
}

/// Ce qui ne va pas dans le pays de nationalite ecrit hors de la liste (`pays_nationalite_hors_liste`), avant toute
/// codification : un pays de la liste a cote, ou un pays que la liste connait (son nom ou son code). `None` : rien a
/// dire (pas de pays hors liste, ou un pays que la liste ne connait pas).
pub fn faute_hors_liste(fiche: &Nationalite) -> R<Option<String>> {
    let Some(hors) = fiche.hors.clone() else { return Ok(None) };
    if let Some(pays) = fiche.pays.clone() {
        return Ok(Some(format!(
            "La fiche de {} dit deux pays de nationalité : {} dans la liste, et « {} » hors de la liste. Garder l'un ou \
             l'autre : le pays de la liste, dont la nationalité DSN se déduit, ou le pays hors de la liste avec la \
             nationalité choisie.",
            fiche.qui,
            pays,
            hors
        )));
    }
    let connu = match socle::pays::du_nom(&hors)? {
        Some(p) => Some(p),
        None if hors.chars().count() == 2 => socle::pays::du_code(&hors)?,
        None => None,
    };
    Ok(connu.map(|p| {
        format!(
            "Le pays de nationalité de {} (« {} ») est dans la liste ({}, {}) : le choisir dans « Pays de nationalité » ; \
             la nationalité que la DSN déclare s'en déduit et ne se choisit pas.",
            fiche.qui,
            hors,
            p.nom,
            p.code
        )
    }))
}

/// Le code a declarer en `S21.G00.30.013` pour une fiche salarie. Refus : pas de code ; un 03 sans pays de nationalite
/// (la fiche ne dit pas si c'est l'EEE ou le reste du monde) ; un pays et un code qui se contredisent. Un pays de
/// nationalite hors de la liste livree (`socle::pays`) refuse aussi, sauf ecrit comme tel (`pays_nationalite_hors_liste`) :
/// la codification est alors celle que l'administrateur a choisie.
pub fn codification_ue(fiche: &Nationalite) -> R<CodeNationalite> {
    use CodeNationalite::{Francaise, HorsUnionEuropeenne, Inconnue, UnionEuropeenne};
    let code = fiche.code;
    if let Some(m) = faute_hors_liste(fiche)? {
        return Err(refus(m));
    }
    if let Some(hors) = fiche.hors.clone() {
        return match code {
            None => Err(refus(format!(
                "Le pays de nationalité de {} (« {} ») n'est pas dans la liste du logiciel : la nationalité que la DSN \
                 déclare ne s'en déduit pas. Dans sa fiche, la choisir (S21.G00.30.013, cahier technique DSN p. 173 : \
                 02 Union européenne, 03 Espace économique européen hors Union, 04 reste du monde).",
                fiche.qui,
                hors
            ))),
            Some(Francaise) => Err(refus(format!(
                "La fiche de {} dit la nationalité française (01) et un pays de nationalité hors de la liste (« {} ») : \
                 la France est dans la liste. Choisir 02, 03 ou 04, ou la France dans « Pays de nationalité ».",
                fiche.qui,
                hors
            ))),
            Some(c) => Ok(c),
        };
    }
    let Some(pays) = fiche.pays.clone() else {
        return match code {
            None => Err(socle::fichiers::absent("code_nationalite_dsn")),
            Some(HorsUnionEuropeenne) => Err(refus(format!(
                "La nationalité de {} est codée 03 sans pays de nationalité. La norme DSN \
                 fait du 03 l'Espace économique européen hors Union (Islande, Liechtenstein, Norvège), et le reste du monde \
                 est 04 (S21.G00.30.013, cahier technique p. 173). Dans sa fiche, indiquer son pays de nationalité, ou \
                 choisir de nouveau la nationalité (04 pour un pays hors Union et hors EEE).",
                fiche.qui
            ))),
            Some(c) => Ok(c),
        };
    };
    // LE PAYS SE LIT DANS LA LISTE LIVREE (`socle::pays`) : un code qu'elle ne connait pas refuse ; un territoire
    // prend la codification de son pays de rattachement.
    let Some(p) = socle::pays::du_code(&pays)? else {
        return Err(refus(socle::pays::inconnu_nationalite(&format!("Le pays de nationalité de {}", fiche.qui), &pays)));
    };
    let iso = p.code.clone();
    let attendu = codification(&p);
    match code {
        // Un code 03 ou 02 que le pays contredit d'un cran (03 pour un pays hors EEE, 02 pour l'Islande, le
        // Liechtenstein ou la Norvege) se recode depuis le pays : le pays tranche.
        Some(c) if c != attendu && !(c == HorsUnionEuropeenne && attendu == Inconnue) && !(c == UnionEuropeenne && attendu == HorsUnionEuropeenne) => Err(refus(format!(
            "La fiche de {} dit la nationalité {} et le pays de nationalité {}, qui se déclare {} (S21.G00.30.013 : 01 \
             France, 02 Union européenne, 03 EEE hors Union, 04 reste du monde). Corriger l'un ou l'autre dans sa fiche.",
            fiche.qui,
            c,
            iso,
            attendu
        ))),
        _ => Ok(attendu),
    }
}
