//! La lecture d'un message recu (`courriel_lecture`) et le questionnaire de la fiche (`fiche_questionnaire`), contre
//! `envois/tests/donnees/courriel_lecture/cas.json` : 229 messages (texte propre, adresse, Message-ID, date, reponses lues, fiche
//! produite), 25 dates, 26 pays, le questionnaire mot pour mot. `PUPPETPAIE_REECRIRE_ORACLES=1` le reecrit depuis le
//! moteur (les `eml` restent).
//!
//! La ligne du sexe se lit apres son dernier deux-points ; « -----Original Message----- » coupe la citation, en LF comme
//! en CRLF.
//!
//! Heure de Paris, quel que soit le fuseau du poste ; un en-tete `Date` illisible rend l'heure de la releve (`MAINTENANT`).
//! Aucun reseau.

use documents::fiche_questionnaire as fq;
use envois::courriel_lecture::{adresse_de, date_de, identifiant_de, texte_propre};
use serde::{Deserialize, Serialize};
use socle::base64::lire as base64_lire;

/// L'oracle : le questionnaire et ses textes, les messages lus, les dates et les pays.
#[derive(Deserialize, Serialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
struct Oracle {
    questionnaire: String,
    sujet: String,
    corps: String,
    html: String,
    alphabet: String,
    questions: Vec<(String, String, String)>,
    messages: Vec<MessageLu>,
    dates: Vec<DateLue>,
    pays: Vec<PaysLu>,
}

/// Un message recu (`eml`, en base64) et ce que sa lecture en rend. `defaut_tirets` : une donnee du cas, que le test ne
/// recalcule pas.
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
    reponses: Reponses,
    fiche: fq::FicheLue,
    illisible: Vec<String>,
}

/// Les reponses lues, dans l'ordre du questionnaire.
#[derive(PartialEq, Debug, Serialize)]
#[serde(transparent)]
struct Reponses(#[serde(serialize_with = "socle::ecriture::en_ordre")] Vec<(String, String)>);

impl<'de> Deserialize<'de> for Reponses {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        socle::forme::en_ordre(d).map(Reponses)
    }
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
fn la_lecture_des_reponses_et_le_questionnaire_rendent_l_oracle() {
    let attendu: Oracle = socle::forme::lire_fichier(&chemin(), "L'oracle de la lecture des courriels ne se lit pas.").unwrap_or_else(|e| panic!("{}", e.phrase));
    assert!(attendu.messages.len() > 200, "le fichier de cas est complet");
    let messages: Vec<MessageLu> = attendu
        .messages
        .iter()
        .map(|cas| {
            let brut = base64_lire(&cas.eml).unwrap();
            let m = mailparse::parse_mail(&brut).unwrap_or_else(|e| panic!("{} : {}", cas.nom, e));
            let propre = texte_propre(&m);
            let lues = fq::lire_reponses(&propre);
            let reponses: Vec<(String, String)> = fq::QUESTIONS.iter().filter_map(|(k, _, _)| lues.get(k).map(|v| (k.to_string(), v.clone()))).collect();
            assert_eq!(reponses.len(), lues.len(), "{} : une reponse hors du questionnaire", cas.nom);
            let (fiche, illisible) = fq::fiche_depuis_reponses(&lues).unwrap();
            MessageLu {
                nom: cas.nom.clone(),
                eml: cas.eml.clone(),
                texte: propre,
                adresse: match adresse_de(&m) {
                    envois::courriel_lecture::Expediteur::Adresse(a) => Some(a),
                    envois::courriel_lecture::Expediteur::Muet | envois::courriel_lecture::Expediteur::PasUneAdresse { .. } => None,
                },
                message_id: identifiant_de(&m),
                date: {
                    // L'heure de la releve, que l'oracle ecrit `MAINTENANT`.
                    let releve = socle::paris::Moment::de_secondes((1 - 719_163) * 86_400).unwrap();
                    let lue = date_de(&m, releve);
                    if lue == releve { "MAINTENANT".to_string() } else { socle::paris::ecrit::seconde::texte(lue) }
                },
                defaut_tirets: cas.defaut_tirets,
                reponses: Reponses(reponses),
                fiche,
                illisible,
            }
        })
        .collect();
    let obtenu = Oracle {
        questionnaire: fq::questionnaire("FIC-ABCDEF", &fq::posees(&|_| true), &|_| String::new()),
        sujet: fq::SUJET.to_string(),
        corps: fq::CORPS.to_string(),
        html: fq::HTML.to_string(),
        alphabet: fq::ALPHABET_CODE.to_string(),
        questions: fq::QUESTIONS.iter().map(|(a, b, c)| (a.to_string(), b.to_string(), droit::fiche::NomDeCase::from(*c).to_string())).collect(),
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
