//! Configuration › Modèle de bulletin : simplifie ou complet, chacun montre sur une paie fictive ; la compagnie choisit.

use droit::fiche::Lue as _;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::catalogue::{Acte, Affiche, Champ, Forme, Ecran, Fiche, Place, FORME, OPERATION};
use crate::catalogue::Route;
use crate::enveloppe::{en_json, Charge, Dite, Porte, Rendu};
use droit::contexte::Contexte;
use documents::bulletin::jeux_et_description;
use droit::fiche::ModeleBulletin;
use socle::fichiers::dossier_du_code;
use socle::echec::{Classe, R};

const MODELE: Champ = Champ { nom: "modele", libelle: Affiche::new("Modèle"), forme: Forme::Texte, sens: "« simplifié » ou « complet »", avance: false };

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Bulletin);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::BulletinModeles,
        geste: "Dire le modèle du bulletin choisi, et les salariés qui en ont un autre.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "modele, les modèles connus, autrement (les salariés à un autre modèle)",
        refus: &[OPERATION, FORME],
        acte: Acte::Lit,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::BulletinModeleApercu,
        geste: "Montrer un bulletin d'exemple dans un modèle.",
        charge: &[MODELE],
        exemple: r#"{"modele":"complet"}"#,
        rend: "modele, pdf (en base64)",
        refus: &[OPERATION, FORME],
        acte: Acte::Lit,
        phrase: 0,
        ecran: Ecran::Lecture,
    },
    Fiche {
        route: Route::BulletinModeleChoisir,
        geste: "Choisir le modèle du bulletin des documents faits dès aujourd'hui.",
        charge: &[MODELE],
        exemple: r#"{"modele":"simplifié"}"#,
        rend: "ce que bulletin-modeles rend",
        refus: &[OPERATION, FORME],
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: ECRAN,
    },
];

/// Une route de ce module, branchee sur l'enveloppe.
pub fn route(porte: &Porte, charge: &Charge) -> R<Rendu> {
    match porte.route {
        Route::BulletinModeles => {
            let super::SansCharge {} = charge.lire()?;
            en_json(modeles(porte.ctx)?)
        }
        Route::BulletinModeleApercu => en_json(apercu(charge.lire()?)?),
        Route::BulletinModeleChoisir => en_json(choisir(porte.ctx, &charge.lire()?)?),
        autre => Err(super::enveloppe::route_inconnue(autre.nom())),
    }
}

/// `bulletin-modele-apercu` : le bulletin d'exemple dans le modele demande, en PDF.
fn apercu(c: ChargeModele) -> R<ApercuModele> {
    let modele = c.modele;
    let pdf = apercu_pdf(modele).map_err(|e| e.reclasser(Classe::Refuse))?;
    Ok(ApercuModele {
        phrase: format!("Bulletin d'exemple dans le modèle {}, en PDF.", modele),
        modele,
        pdf: socle::base64::ecrire(&pdf),
    })
}

impl Dite for ApercuModele {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

impl Dite for ModelesDuBulletin {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

// ------------------------------------------------------------------ le modele du bulletin

/// La charge de `bulletin-modele-apercu` et de `bulletin-modele-choisir` : le modele, par sa cle (un modele connu, rien
/// d'autre : un autre se refuse a la lecture de la charge).
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChargeModele {
    pub modele: ModeleBulletin,
}

/// Ce que rend `bulletin-modele-apercu` : le PDF d'exemple, en base64, et la phrase. La fenetre le lit aussi
/// (`interface::Fenetre::apercu_bulletin`).
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApercuModele {
    phrase: String,
    modele: ModeleBulletin,
    pub pdf: String,
}

/// L'adresse ou la fenetre sert l'apercu d'un modele (`interface::Fenetre::apercu_bulletin`).
pub(crate) fn adresse_apercu(m: ModeleBulletin) -> String {
    format!("/apercu/bulletin?modele={}", m.dans_l_adresse())
}

// ------------------------------------------------------------------ bulletin-modeles

/// Un modele du bulletin, tel que l'onglet le montre : sa cle, son nom, sa description, son apercu, et sa pastille s'il
/// est celui de la compagnie.
#[derive(Debug, Clone, serde::Serialize)]
struct ModeleDit {
    cle: ModeleBulletin,
    nom: &'static str,
    description: &'static str,
    apercu_dit: String,
    apercu: String,
    pastille: Option<socle::ecran::Pastille>,
}

/// Un salarie qui a demande l'autre modele (la case de sa fiche).
#[derive(Debug, Clone, serde::Serialize)]
struct SalarieAutrement {
    cle: droit::contexte::IdSalarie,
    nom: String,
    modele: ModeleBulletin,
}

/// Ce que l'onglet dit autour des modeles.
#[derive(Debug, Clone, serde::Serialize)]
struct EcranDesModeles {
    intro: &'static str,
    autrement: Option<String>,
    ensuite: &'static str,
}

/// LES MODELES DU BULLETIN (`bulletin-modeles`, et `bulletin-modele-choisir` apres le choix) : la phrase, le modele
/// choisi et s'il est celui du logiciel, les modeles connus, les salaries qui en ont un autre, et ce que l'onglet en dit.
#[derive(Debug, Clone, serde::Serialize)]
struct ModelesDuBulletin {
    phrase: String,
    choisi: ModeleBulletin,
    par_defaut: bool,
    modeles: Vec<ModeleDit>,
    salaries_autrement: Vec<SalarieAutrement>,
    ecran: EcranDesModeles,
}

fn modeles(ctx: &Contexte) -> R<ModelesDuBulletin> {
    let choisi = ctx.entreprise.modele_bulletin.dite();
    let modele = documents::bulletin::modele_de_la_compagnie(&ctx.entreprise);
    let mut autrement = Vec::new();
    for (cle, sal) in &ctx.salaries {
        if let Some(m) = documents::bulletin::modele_du_salarie(sal).map_err(|e| e.reclasser(Classe::Refuse))? {
            if m != modele {
                let nom = match (sal.prenom_affichage.portee(), sal.nom_affichage.portee()) {
                    (Some(prenom), Some(nom)) => format!("{} {}", prenom, nom),
                    (Some(un), None) | (None, Some(un)) => un.to_string(),
                    (None, None) => cle.to_string(),
                };
                autrement.push(SalarieAutrement { cle: cle.clone(), nom, modele: m });
            }
        }
    }
    let liste: Vec<ModeleDit> = ModeleBulletin::TOUS
        .iter()
        .map(|m| ModeleDit {
            cle: *m,
            nom: m.nom(),
            description: m.description(),
            apercu_dit: format!("Aperçu du modèle {}", m.nom()),
            apercu: adresse_apercu(*m),
            pastille: (*m == modele).then(|| socle::ecran::Pastille::de(socle::ecran::EtatDEcran::Fait, if choisi.is_none() { "celui de la compagnie (par défaut)" } else { "celui de la compagnie" })),
        })
        .collect();
    let dits: Vec<String> = autrement.iter().map(|s| format!("{} ({})", s.nom, s.modele)).collect();
    Ok(ModelesDuBulletin {
        phrase: format!("Modèle de bulletin : {}{}.", modele, if choisi.is_none() { " (celui du logiciel)" } else { "" }),
        choisi: modele,
        par_defaut: choisi.is_none(),
        modeles: liste,
        salaries_autrement: autrement,
        ecran: EcranDesModeles {
            intro: "Le bulletin que la compagnie remet à ses salariés. Chaque modèle est montré sur la même paie fictive (une compagnie et un salarié inventés) : mêmes lignes du modèle réglementaire, mêmes totaux, même net. Un salarié qui demande l'autre modèle l'obtient par la case « Modèle de bulletin demandé par le salarié » de sa fiche, dans Avancé.",
            autrement: (!dits.is_empty()).then(|| format!("Ces salariés ont demandé l'autre modèle (case de leur fiche, Avancé) et le gardent : {}.", dits.join(", "))),
            ensuite: "Le choix vaut pour les bulletins produits ensuite ; ceux déjà remis ne changent pas.",
        },
    })
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
    let numeros: Vec<Option<engagement::numeros::NumeroDeBulletin>> = (1..=s.paies.len() as i64).map(|n| Some(engagement::numeros::NumeroDeBulletin::du_rang(n, le_mois.annee()))).collect();
    let departs = engagement::departs::Departs::default();
    let passe = documents::bulletin::passe_des_bulletins(&ctx, &s.paies, le_mois, &departs)?;
    jeux_et_description(&ctx, &s.paies, &numeros, &ent, le_mois, None, false, &HashMap::new(), &departs, &passe)
}

/// Le PDF de la paie fictive sous `modele`.
pub(crate) fn apercu_pdf(modele: ModeleBulletin) -> R<Vec<u8>> {
    documents::fabrique::fabriquer_octets(&apercu_description(modele)?)
}

// ------------------------------------------------------------------ bulletin-modele-choisir

fn choisir(ctx: &Contexte, charge: &ChargeModele) -> R<ModelesDuBulletin> {
    // Le modele vaut pour les documents faits a partir d'aujourd'hui ; ceux d'avant gardent le leur.
    crate::fiches::poser_entreprise(ctx, droit::fiche::ChangementEntreprise::ModeleBulletin(Some(charge.modele)), Some(ctx.aujourdhui))?;
    let neuf = Contexte::nouveau(&ctx.racine, None).map_err(|e| e.reclasser(Classe::Refuse))?;
    let mut v = modeles(&neuf)?;
    v.phrase = format!("Modèle choisi : {}, pour les documents faits à partir d'aujourd'hui.", v.choisi);
    Ok(v)
}
