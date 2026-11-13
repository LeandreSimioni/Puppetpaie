//! LA BASE DES OBLIGATIONS (`logiciel/donnees/obligations-base.json`) : chaque obligation qu'un arret ou une fin de
//! contrat fait naitre, son delai, son point de depart et ses textes. Sa forme est declaree ici et `base` est la seule
//! fonction qui lit le fichier ; le signalement (`signalement::avertissement_hors_dsn`) et le moteur (`obligations`, qui
//! compte les obligations) la lisent par elle.

use serde::{Deserialize, Serialize};

use std::sync::{Arc, OnceLock};

use droit::arrets::MotifDArret;
use socle::fichiers::{forme_en_cache, FormeGardee};
use socle::echec::{Echec, R};

/// `obligations-base.json` : chaque obligation, son delai, son point de depart et ses textes. Ses `quoi` et `note` ne
/// servent pas au calcul (la regle d'un jour ouvre est dans le calendrier du moteur, `jours_ouvres_apres`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaseDesObligations {
    pub quoi: String,
    pub note: String,
    pub obligations: LesObligations,
}

/// Les obligations que le moteur sait compter, une par cle.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LesObligations {
    pub signalement_arret: DeBase,
    pub signalement_arret_rectificatif: DeBase,
    pub signalement_reprise: DeBase,
    pub signalement_fin_contrat: DeBase,
    pub declaration_accident: DeBase,
    pub declaration_maladie_professionnelle: DeBase,
    pub attestation_salaire: DeBase,
}

/// Une obligation telle que le fichier la livre : ce qu'elle est, ou elle se fait, qui la recoit, son delai (sans
/// subrogation, avec, ou unique), les motifs qui l'appellent ou n'en font pas naitre, ses suites et ses textes.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeBase {
    pub libelle: String,
    pub nait_de: String,
    pub qui: String,
    pub ou: Option<Ou>,
    pub destinataire: String,
    pub motifs: Vec<MotifDArret>,
    pub sauf_motifs: Option<SaufMotifs>,
    pub delai: Option<Delai>,
    pub delai_sans_subrogation: Option<Delai>,
    pub delai_avec_subrogation: Option<Delai>,
    pub connaissance_presumee: Option<String>,
    pub exceptions: Vec<String>,
    pub inferences: Vec<String>,
    pub a_avoir: Vec<String>,
    pub note: Option<String>,
    pub suites: Option<Suites>,
    pub textes: Vec<TexteDepose>,
}

/// Ou une obligation se fait : une phrase (un signalement, que le logiciel ecrit), ou les lieux ou la faire soi-meme
/// (hors DSN), que la page propose et que `accident-declare` et `attestation-declare` acceptent.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Ou {
    Phrase(String),
    Lieux(Vec<Lieu>),
}

/// Un lieu ou faire une obligation hors DSN : son code, ce qu'en dit l'ecran (court, puis en entier), le texte qui le
/// decrit, et pour net-entreprises ce qu'il faudrait pour y deposer un fichier.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Lieu {
    pub code: LieuHorsDsn,
    pub court: String,
    pub libelle: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    edi: Option<String>,
    texte: String,
}

/// OU SE FAIT UNE OBLIGATION HORS DSN : les lieux que `obligations-base.json` propose (`ou`), que la compagnie choisit en
/// la disant faite (`accident-declare`, `attestation-declare`) et que sa fiche de suivi garde. Ecrit par son mot ; un
/// autre mot se refuse a la lecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LieuHorsDsn {
    Courrier,
    NetEntreprises,
    NetEntreprisesEcran,
    NetEntreprisesEdi,
}

impl LieuHorsDsn {
    /// Tous, dans l'ordre de la liste.
    pub const TOUS: [LieuHorsDsn; 4] = [LieuHorsDsn::Courrier, LieuHorsDsn::NetEntreprises, LieuHorsDsn::NetEntreprisesEcran, LieuHorsDsn::NetEntreprisesEdi];

    /// Son mot, tel que la donnee, la charge et la fiche de suivi l'ecrivent.
    pub fn texte(self) -> &'static str {
        match self {
            LieuHorsDsn::Courrier => "courrier",
            LieuHorsDsn::NetEntreprises => "net-entreprises",
            LieuHorsDsn::NetEntreprisesEcran => "net-entreprises-ecran",
            LieuHorsDsn::NetEntreprisesEdi => "net-entreprises-edi",
        }
    }
}

impl std::fmt::Display for LieuHorsDsn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.texte())
    }
}

impl Serialize for LieuHorsDsn {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.texte())
    }
}

impl<'de> Deserialize<'de> for LieuHorsDsn {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let mot = String::deserialize(d)?;
        match LieuHorsDsn::TOUS.into_iter().find(|l| l.texte() == mot) {
            Some(l) => Ok(l),
            None => Err(serde::de::Error::custom(format!(
                "Lieu inconnu : {}. Les lieux sont {}.",
                socle::fichiers::repr(&mot),
                LieuHorsDsn::TOUS.map(LieuHorsDsn::texte).join(", ")
            ))),
        }
    }
}

/// Un texte depose qui fonde une obligation : sa reference, son fichier, ce qu'il dit.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TexteDepose {
    reference: String,
    fichier: droit::source::CheminDepose,
    dit: String,
}

/// Les motifs d'arret que la DSN ne couvre pas, pourquoi, et l'avertissement que le signalement en donne.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SaufMotifs {
    pub codes: Vec<MotifDArret>,
    #[serde(rename = "pourquoi")]
    _pourquoi: String,
    pub avertissement: String,
    #[serde(rename = "texte")]
    _texte: TexteDepose,
}

/// Un delai : en jours ouvres, en jours, en heures (dimanches et feries non compris), ou avec la DSN mensuelle ; et la
/// phrase qui le dit.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Delai {
    pub jours_ouvres: Option<i64>,
    pub jours: Option<i64>,
    pub heures: Option<i64>,
    #[serde(rename = "sans_dimanches")]
    _sans_dimanches: Option<bool>,
    #[serde(rename = "sans_feries")]
    _sans_feries: Option<bool>,
    #[serde(rename = "avec_la_mensuelle")]
    _avec_la_mensuelle: Option<bool>,
    #[serde(rename = "depart")]
    _depart: Option<String>,
    pub dit: String,
}

/// Ce qui suit une declaration d'accident.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Suites {
    pub reserves: Option<Reserves>,
    pub feuille_accident: Option<FeuilleDAccident>,
}

/// Les reserves motivees (R. 441-6) : dix jours francs apres la declaration.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reserves {
    pub libelle: String,
    pub jours_francs: i64,
    /// Ce d'ou courent les jours francs, en mots (« la déclaration »).
    #[serde(rename = "depuis")]
    _point_de_depart: String,
    pub texte: TexteDepose,
    pub inference: String,
}

/// La feuille d'accident a remettre au salarie (L. 441-5).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeuilleDAccident {
    pub libelle: String,
    pub texte: TexteDepose,
}

/// Une obligation que le moteur sait compter : chacune est un champ de `LesObligations`, et son nom (`nom`) est la cle du
/// fichier, celle que la page et les rappels lisent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CleDObligation {
    SignalementArret,
    SignalementArretRectificatif,
    SignalementReprise,
    SignalementFinContrat,
    DeclarationAccident,
    DeclarationMaladieProfessionnelle,
    AttestationSalaire,
}

impl CleDObligation {
    /// Son nom, la cle de `obligations-base.json`.
    pub fn nom(self) -> &'static str {
        match self {
            CleDObligation::SignalementArret => "signalement_arret",
            CleDObligation::SignalementArretRectificatif => "signalement_arret_rectificatif",
            CleDObligation::SignalementReprise => "signalement_reprise",
            CleDObligation::SignalementFinContrat => "signalement_fin_contrat",
            CleDObligation::DeclarationAccident => "declaration_accident",
            CleDObligation::DeclarationMaladieProfessionnelle => "declaration_maladie_professionnelle",
            CleDObligation::AttestationSalaire => "attestation_salaire",
        }
    }

    /// L'obligation dont `nom` est le nom ; `None` pour un nom qui n'en est aucun.
    pub fn de_son_nom(nom: &str) -> Option<CleDObligation> {
        [
            CleDObligation::SignalementArret,
            CleDObligation::SignalementArretRectificatif,
            CleDObligation::SignalementReprise,
            CleDObligation::SignalementFinContrat,
            CleDObligation::DeclarationAccident,
            CleDObligation::DeclarationMaladieProfessionnelle,
            CleDObligation::AttestationSalaire,
        ]
        .into_iter()
        .find(|c| c.nom() == nom)
    }
}

impl Serialize for CleDObligation {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.nom())
    }
}

impl std::fmt::Display for CleDObligation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.nom())
    }
}

impl LesObligations {
    /// L'obligation `cle`, telle que le fichier la livre.
    pub fn de(&self, cle: CleDObligation) -> &DeBase {
        match cle {
            CleDObligation::SignalementArret => &self.signalement_arret,
            CleDObligation::SignalementArretRectificatif => &self.signalement_arret_rectificatif,
            CleDObligation::SignalementReprise => &self.signalement_reprise,
            CleDObligation::SignalementFinContrat => &self.signalement_fin_contrat,
            CleDObligation::DeclarationAccident => &self.declaration_accident,
            CleDObligation::DeclarationMaladieProfessionnelle => &self.declaration_maladie_professionnelle,
            CleDObligation::AttestationSalaire => &self.attestation_salaire,
        }
    }
}

impl DeBase {
    /// Les motifs que la DSN ne couvre pas (`sauf_motifs.codes`), aucun sinon.
    pub fn sauf(&self) -> &[MotifDArret] {
        match &self.sauf_motifs {
            Some(s) => &s.codes,
            None => &[],
        }
    }

    /// Le delai `quoi` (`delai`, `delai_sans_subrogation`, `delai_avec_subrogation`), que l'obligation doit porter.
    pub fn delai_de<'a>(&self, d: &'a Option<Delai>, quoi: &str) -> R<&'a Delai> {
        d.as_ref().ok_or_else(|| livree(&format!("{}.{}", self.libelle, quoi)))
    }

    /// Le point de depart presume, que l'obligation doit porter quand elle court de la connaissance.
    pub fn presume(&self) -> R<String> {
        self.connaissance_presumee.clone().ok_or_else(|| livree("connaissance_presumee"))
    }

    /// Les lieux ou la faire soi-meme (aucun pour un signalement).
    pub fn lieux(&self) -> &[Lieu] {
        match &self.ou {
            Some(Ou::Lieux(l)) => l,
            _ => &[],
        }
    }
}

/// Une valeur que la base doit porter, et qu'elle ne porte pas.
#[track_caller]
pub fn livree(cle: &str) -> Echec {
    socle::echec::livree("obligations-base.json", cle)
}

/// `obligations-base.json`, lu par sa forme (une fois tant que le fichier ne change pas).
pub fn base() -> R<BaseDesObligations> {
    static CACHE: OnceLock<FormeGardee<BaseDesObligations>> = OnceLock::new();
    let lue: Arc<BaseDesObligations> =
        forme_en_cache(&CACHE, "obligations-base.json", "Les obligations livrées avec le logiciel ne se lisent pas : le signaler au développeur.", Ok)?;
    Ok((*lue).clone())
}
