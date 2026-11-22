//! La lecture d'un message recu (`courriel_lecture`), et les dates et les pays que la releve d'une fiche lit
//! (`fiche_questionnaire`), contre `envois/tests/donnees/courriel_lecture/cas.json` : 229 messages (texte propre, adresse,
//! Message-ID, date), 25 dates, 26 pays. `PUPPETPAIE_REECRIRE_ORACLES=1` le reecrit depuis le moteur (les `eml` restent).
//!
//! « -----Original Message----- » coupe la citation, en LF comme en CRLF.
//!
//! Heure de Paris, quel que soit le fuseau du poste ; un en-tete `Date` illisible rend l'heure de la releve (`MAINTENANT`).
//! Aucun reseau.

use documents::fiche_questionnaire as fq;
use envois::courriel_lecture::{adresse_de, date_de, identifiant_de, texte_propre};
use serde::{Deserialize, Serialize};
use socle::base64::lire as base64_lire;
use socle::forme::NonLue;

/// L'oracle : les messages lus, les dates et les pays. Les cles du questionnaire d'avant la reponse typee
/// (`questionnaire`, `sujet`, `corps`, `html`, `alphabet`, `questions`) se lisent et ne disent rien, le temps que
/// l'oracle se reecrive sans elles.
#[derive(Deserialize, Serialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
struct Oracle {
    #[serde(deserialize_with = "socle::forme::non_lue", default, skip_serializing)]
    questionnaire: NonLue,
    #[serde(deserialize_with = "socle::forme::non_lue", default, skip_serializing)]
    sujet: NonLue,
    #[serde(deserialize_with = "socle::forme::non_lue", default, skip_serializing)]
    corps: NonLue,
    #[serde(deserialize_with = "socle::forme::non_lue", default, skip_serializing)]
    html: NonLue,
    #[serde(deserialize_with = "socle::forme::non_lue", default, skip_serializing)]
    alphabet: NonLue,
    #[serde(deserialize_with = "socle::forme::non_lue", default, skip_serializing)]
    questions: NonLue,
    messages: Vec<MessageLu>,
    dates: Vec<DateLue>,
    pays: Vec<PaysLu>,
}

/// Un message recu (`eml`, en base64) et ce que sa lecture en rend. `defaut_tirets` : une donnee du cas, que le test ne
/// recalcule pas. Les reponses lues a l'ancienne, par libelles (`reponses`, `fiche`, `illisible`), se lisent et ne
/// disent rien.
#[derive(Deserialize, Serialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
struct MessageLu {
    nom: String,
    eml: String,
    texte: String,
    adresse: Option<String>,
    message_id: Option<String>,
    date: String,
    defaut_tirets: bool,
    #[serde(deserialize_with = "socle::forme::non_lue", default, skip_serializing)]
    reponses: NonLue,
    #[serde(deserialize_with = "socle::forme::non_lue", default, skip_serializing)]
    fiche: NonLue,
    #[serde(deserialize_with = "socle::forme::non_lue", default, skip_serializing)]
    illisible: NonLue,
}

#[derive(Deserialize, Serialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
struct DateLue {
    texte: String,
    attendu: Option<socle::date::Jour>,
}

#[derive(Deserialize, Serialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
struct PaysLu {
    texte: String,
    attendu: Option<(String, Option<String>)>,
}

fn chemin() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/donnees/courriel_lecture/cas.json")
}

fn reecrire() -> bool {
    std::env::var("PUPPETPAIE_REECRIRE_ORACLES").is_ok_and(|v| v == "1")
}

#[test]
fn la_lecture_des_messages_des_dates_et_des_pays_rend_l_oracle() {
    let attendu: Oracle = socle::forme::lire_fichier(&chemin(), "L'oracle de la lecture des courriels ne se lit pas.").unwrap_or_else(|e| panic!("{}", e.phrase));
    assert!(attendu.messages.len() > 200, "le fichier de cas est complet");
    let messages: Vec<MessageLu> = attendu
        .messages
        .iter()
        .map(|cas| {
            let brut = base64_lire(&cas.eml).unwrap();
            let m = mailparse::parse_mail(&brut).unwrap_or_else(|e| panic!("{} : {}", cas.nom, e));
            MessageLu {
                nom: cas.nom.clone(),
                eml: cas.eml.clone(),
                texte: texte_propre(&m),
                adresse: match adresse_de(&m) {
                    envois::courriel_lecture::Expediteur::Adresse(a) => Some(a),
                    envois::courriel_lecture::Expediteur::Muet | envois::courriel_lecture::Expediteur::PasUneAdresse { .. } => None,
                },
                message_id: identifiant_de(&m),
                date: {
                    // L'heure de la releve, que l'oracle ecrit `MAINTENANT`.
                    // Le 1er janvier de l'an 1 a minuit UTC, une heure de Paris : aucun courriel n'en est date.
                    let releve: socle::paris::Moment = socle::piece::Zone::lue("0001-01-01T01:00:00", socle::piece::format::InstantIso).valeur().unwrap();
                    let lue = date_de(&m, releve);
                    if lue == releve { "MAINTENANT".to_string() } else { socle::paris::ecrit::seconde::texte(lue) }
                },
                defaut_tirets: cas.defaut_tirets,
                reponses: NonLue::default(),
                fiche: NonLue::default(),
                illisible: NonLue::default(),
            }
        })
        .collect();
    let obtenu = Oracle {
        questionnaire: NonLue::default(),
        sujet: NonLue::default(),
        corps: NonLue::default(),
        html: NonLue::default(),
        alphabet: NonLue::default(),
        questions: NonLue::default(),
        messages,
        dates: attendu.dates.iter().map(|d| DateLue { texte: d.texte.clone(), attendu: fq::date_lue(&d.texte) }).collect(),
        pays: attendu.pays.iter().map(|p| PaysLu { texte: p.texte.clone(), attendu: fq::pays_lu(&p.texte).unwrap().map(|(c, u)| (c.to_string(), Some(u.as_str().to_string()))) }).collect(),
    };
    if reecrire() {
        socle::ecriture::ecrire_json(&chemin(), &obtenu, 0).unwrap();
        return;
    }
    for (a, o) in attendu.messages.iter().zip(&obtenu.messages) {
        assert_eq!(a, o, "{} : la lecture differe de l'oracle", a.nom);
    }
    assert_eq!(attendu.dates, obtenu.dates, "dates");
    assert_eq!(attendu.pays, obtenu.pays, "pays");
    assert_eq!(attendu, obtenu);
}
