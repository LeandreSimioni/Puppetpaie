//! Le bulletin d'exemple d'un modele du bulletin (simplifie ou complet), sur une paie fictive : la fiche Entreprise
//! l'ouvre a cote de chaque modele (`fiches::rendue`, la case `modele_bulletin`).

use std::collections::HashMap;
use std::path::PathBuf;

use crate::catalogue::{Acte, Affiche, Champ, Forme, Ecran, Fiche, FORME, OPERATION};
use crate::catalogue::Route;
use crate::enveloppe::{en_json, Charge, Dite, Phrase, Porte, Rendu};
use droit::contexte::Contexte;
use documents::bulletin::jeux_et_description;
use droit::fiche::ModeleBulletin;
use socle::fichiers::dossier_du_code;
use socle::echec::{Classe, R};

const MODELE: Champ = Champ { nom: "modele", libelle: Affiche::new("Modèle"), forme: Forme::Texte, sens: "« simplifié » ou « complet »", avance: false };

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[Fiche {
    route: Route::BulletinModeleApercu,
    geste: "Montrer un bulletin d'exemple dans un modèle.",
    charge: &[MODELE],
    exemple: r#"{"modele":"complet"}"#,
    rend: "modele, pdf (en base64)",
    refus: &[OPERATION, FORME],
    pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
    acte: Acte::Lit,
    phrase: 0,
    ecran: Ecran::Lecture,
}];

/// Une route de ce module, branchee sur l'enveloppe.
pub fn route(porte: &Porte, charge: &Charge) -> R<Rendu> {
    match porte.route {
        Route::BulletinModeleApercu => en_json(apercu(charge.lire()?)?),
        autre => Err(super::enveloppe::route_inconnue(autre.nom())),
    }
}

/// `bulletin-modele-apercu` : le bulletin d'exemple dans le modele demande, en PDF.
fn apercu(c: ChargeModele) -> R<ApercuModele> {
    let modele = c.modele;
    let pdf = apercu_pdf(modele).map_err(|e| e.reclasser(Classe::Refuse))?;
    Ok(ApercuModele {
        phrase: socle::phrase!("Bulletin d'exemple dans le modèle {}, en PDF.", modele),
        modele,
        pdf: socle::base64::ecrire(&pdf),
    })
}

impl Dite for ApercuModele {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

// ------------------------------------------------------------------ le modele du bulletin

/// La charge de `bulletin-modele-apercu` : le modele, par sa cle (un modele connu, rien d'autre : un autre se refuse a la
/// lecture de la charge).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChargeModele {
    pub modele: ModeleBulletin,
}

/// Ce que rend `bulletin-modele-apercu` : le PDF d'exemple, en base64, et la phrase. La fenetre le lit aussi
/// (`interface::Fenetre::apercu_bulletin`).
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApercuModele {
    phrase: Phrase,
    modele: ModeleBulletin,
    pub pdf: String,
}

/// L'adresse ou la fenetre sert l'apercu d'un modele (`interface::Fenetre::apercu_bulletin`) : la fiche Entreprise la
/// rend sous chaque modele du bulletin (`ouverture::ExempleDuChoix`).
pub(crate) fn adresse_apercu(m: ModeleBulletin) -> String {
    format!("/apercu/bulletin?modele={}", m.dans_l_adresse())
}

// ------------------------------------------------------------------ l'apercu

/// La compagnie fictive de l'apercu, livree avec le logiciel.
pub(crate) fn dossier_apercu() -> PathBuf {
    dossier_du_code().join("apercu-bulletin")
}

/// LA PAIE FICTIVE DE L'APERCU (`apercu-bulletin/saisie.json`) : ce qu'elle est, son mois et ses cartes.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SaisieDeLApercu {
    #[serde(deserialize_with = "socle::forme::non_lue")]
    source: socle::forme::NonLue,
    mois: socle::mois::Mois,
    engagements: Vec<engagement::saisie::Carte>,
}

/// La description du bulletin de la paie fictive sous `modele` : le moteur du jour, la compagnie fictive, rien d'ecrit.
pub(crate) fn apercu_description(modele: ModeleBulletin) -> R<fabrique_pdf::Description> {
    let racine = dossier_apercu();
    let lue: SaisieDeLApercu = socle::forme::lire_fichier(&racine.join("saisie.json"), "La paie de l'aperçu ne se lit pas.")?;
    let saisie = engagement::saisie::VueDuMois { mois: lue.mois, taux_cachet: None, engagements: lue.engagements };
    let le_mois = saisie.mois;
    let ctx = Contexte::nouveau(&racine, Some(saisie.mois.dernier_jour()))?;
    let s = engagement::pour_l_ecran(&ctx, &saisie)?;
    let mut fiche = ctx.entreprise.clone();
    fiche.modele_bulletin = Some(modele);
    // Ni le logo ni la signature du poste sur une paie fictive : ses gabarits sont un dossier qui n'existe pas.
    let ent = ctx.pour_un_modele(&fiche);
    // Une paie de l'ecran n'a pas de numero de bulletin (il se reserve au depart) : l'apercu montre celui d'un premier
    // bulletin de l'annee.
    let numeros: Vec<Option<engagement::numeros::NumeroDeBulletin>> =
        std::iter::successors(Some(engagement::numeros::NumeroDeBulletin::premier(le_mois.annee())), |n| Some(n.suivant())).take(s.paies.len()).map(Some).collect();
    let departs = engagement::departs::Departs::default();
    let passe = documents::bulletin::passe_des_bulletins(&ctx, &s.paies, le_mois, &departs)?;
    jeux_et_description(&ctx, &s.paies, &numeros, &ent, le_mois, None, false, &HashMap::new(), &departs, &passe)
}

/// Le PDF de la paie fictive sous `modele`.
pub(crate) fn apercu_pdf(modele: ModeleBulletin) -> R<Vec<u8>> {
    documents::fabrique::fabriquer_octets(&apercu_description(modele)?)
}
