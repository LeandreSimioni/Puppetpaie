//! LES LIBELLES COURTS DES EXPORTS COMPTABLES : ce qui ne tient pas dans une zone du format du logiciel du comptable
//! (le libelle d'une ecriture, l'intitule d'un compte), la valeur courte que le moteur propose, et celle que la compagnie
//! a validee (`documents::libelles_courts`). L'export la lit ; sans elle, il se refuse et renvoie ici. Une route rend ce
//! qui depasse, une autre enregistre ; l'ecran (une boite des Exports) et le LLM font la meme chose.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use documents::libelles_courts::{self as lc, Depasse, FormatBorne, Quoi};
use documents::plan_comptable::FormatDExport;
use droit::contexte::Contexte;
use socle::echec::{erreur, Echec, R};

use crate::catalogue::{Acte, Ecran, Fiche, Forme, Place, Refus, Route, FORME, OPERATION};
use crate::declaration::{charge, dit, Legende, ParSaForme, ValeurDeLaCharge};
use crate::enveloppe::{en_json, Charge, Dite, Phrase, Porte, Rendu};

const FORMAT: Legende = dit("Format", "le format des écritures, celui d'un logiciel du comptable ; absent : celui du logiciel retenu (exports-periodes, compta)").avancee();

const ECRAN: Ecran = Ecran::Dessine(Place::Exports);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::LibellesCourts,
        geste: "Dire les libellés trop longs pour le format du comptable, et leur valeur courte.",
        charge: &ChargeLire::CHARGE,
        exemple: r#"{}"#,
        rend: "format, nom, libelles [quoi (libelle, compte ou compagnie), cle, zone, long, largeur, propose (la proposition du moteur), court (la valeur validée, si elle tient), valeur (ce que sa case montre : la validée, sinon la proposition)], phrase, ecran (titre de la boîte, son bouton raccourcir, ses colonnes et ses boutons) ; aucun format à zone bornée : libelles vide",
        refus: &[OPERATION, FORME],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Lit,
        phrase: 160,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::LibellesCourtsEnregistrer,
        geste: "Valider la valeur courte de libellés trop longs pour le format du comptable.",
        charge: &ChargeEnregistrer::CHARGE,
        exemple: r#"{"format":"ximport","libelles":[]}"#,
        rend: "ce que rend libelles-courts, après l'enregistrement, et sa phrase ; une liste vide : rien n'est écrit, libelles vide",
        refus: &[
            Refus { classe: socle::echec::Classe::Refuse, corriger: "la valeur que la phrase nomme : vide, trop longue, ou d'un libellé qui ne dépasse pas ; rien n'est écrit" },
            FORME,
        ],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Ecrit,
        phrase: 160,
        ecran: ECRAN,
    },
];

/// Une route de ce module, branchee sur l'enveloppe.
pub fn route(porte: &Porte, charge: &Charge) -> R<Rendu> {
    match porte.route {
        Route::LibellesCourts => en_json(libelles_courts(porte.ctx, charge.lire()?)?),
        Route::LibellesCourtsEnregistrer => en_json(enregistrer(porte.ctx, charge.lire()?)?),
        autre => Err(crate::enveloppe::route_inconnue(autre.nom())),
    }
}

impl Dite for Libelles {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

charge! {
    /// La charge de « libelles-courts » : le format, celui du logiciel retenu s'il n'est pas dit.
    #[derive(Debug)]
    struct ChargeLire {
        #[serde(default, deserialize_with = "format_ou_aucun")]
        format: Option<FormatDExport> = FORMAT,
    }
}

charge! {
    /// La charge de « libelles-courts-enregistrer » : le format, et les valeurs courtes validees.
    #[derive(Debug)]
    struct ChargeEnregistrer {
        #[serde(default, deserialize_with = "format_ou_aucun")]
        format: Option<FormatDExport> = FORMAT,
        libelles: Vec<Valide> = dit("Libellés courts", "[{quoi, cle, court}] : quoi et cle tels que libelles-courts les rend, court la valeur validée (la proposition, ou une autre), de largeur caractères au plus"),
    }
}

/// Le format d'une charge : absent, `null` ou vide (la page n'a pas de format choisi : son menu est vide), aucun ;
/// sinon un format d'export, ou le refus qui le dit.
fn format_ou_aucun<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<FormatDExport>, D::Error> {
    match Option::<String>::deserialize(d)? {
        None => Ok(None),
        Some(t) if t.is_empty() => Ok(None),
        Some(t) => FormatDExport::lire(&t).map(Some).map_err(|e| serde::de::Error::custom(e.phrase)),
    }
}

/// Une valeur courte validee : ce qu'elle remplace (`quoi`, `cle`, tels que rendus), et elle.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Valide {
    quoi: Quoi,
    cle: String,
    court: String,
}

impl ValeurDeLaCharge<ParSaForme> for Valide {
    const FORME: Forme = Forme::Objet;
}

/// Ce que rend la page des libelles courts, et ce que l'ecran en dit (`ecran`).
#[derive(Debug, Serialize)]
struct Libelles {
    format: Option<FormatDExport>,
    nom: Option<&'static str>,
    libelles: Vec<Ligne>,
    phrase: Phrase,
    ecran: EcranDesLibelles,
}

impl Libelles {
    fn de(format: Option<FormatDExport>, nom: Option<&'static str>, libelles: Vec<Ligne>, phrase: Phrase) -> Libelles {
        let titre = match nom {
            Some(n) => format!("{} — {}", LIBELLES_COURTS, n),
            None => LIBELLES_COURTS.to_string(),
        };
        let ecran = EcranDesLibelles { titre, raccourcir: "Les raccourcir", colonnes: ["Zone", "Libellé", "Valeur courte"], fermer: "Fermer", enregistrer: "Enregistrer" };
        Libelles { format, nom, libelles, phrase, ecran }
    }
}

const LIBELLES_COURTS: &str = "Libellés courts";

/// Ce que l'ecran des libelles courts dit : le titre de sa boite, le bouton qui l'ouvre (`raccourcir`), ses colonnes
/// (la zone, le libelle long, la valeur courte), et ses boutons.
#[derive(Debug, Serialize)]
struct EcranDesLibelles {
    titre: String,
    raccourcir: &'static str,
    colonnes: [&'static str; 3],
    fermer: &'static str,
    enregistrer: &'static str,
}

/// Une valeur trop longue : ce qu'elle est et sa cle, la zone en mots, son texte long, la place qu'elle a, la
/// proposition du moteur, la valeur validee si elle tient, et ce que sa case montre (`valeur` : la validee, sinon la
/// proposition).
#[derive(Debug, Serialize)]
struct Ligne {
    quoi: Quoi,
    cle: String,
    zone: String,
    long: String,
    largeur: usize,
    propose: String,
    court: Option<String>,
    valeur: String,
}

/// Le format de la charge, ou celui du logiciel retenu ; `None` s'il n'y en a pas.
fn format_de(ctx: &Contexte, format: Option<FormatDExport>) -> R<Option<FormatDExport>> {
    match format {
        Some(f) => Ok(Some(f)),
        None => documents::etats_compta::format_retenu(ctx),
    }
}

/// Ce qui depasse les zones de `format`, sur tous les mois de la compagnie.
fn depasses(ctx: &Contexte, format: FormatDExport) -> R<Vec<Depasse>> {
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    documents::etats_compta::trop_longs(ctx, format, &departs)
}

fn zone(d: &Depasse) -> String {
    match d.quoi {
        Quoi::Libelle => format!("Libellé, {} caractères et le mois", d.largeur),
        Quoi::Compte => format!("Compte {}, {} caractères", d.cle, d.largeur),
        Quoi::Compagnie => format!("Nom de la compagnie, {} caractères", d.largeur),
    }
}

/// LIRE : ce qui depasse, sa proposition, sa valeur validee.
fn libelles_courts(ctx: &Contexte, c: ChargeLire) -> R<Libelles> {
    let (format, borne) = match format_de(ctx, c.format)? {
        None => return Ok(Libelles::de(None, None, Vec::new(), Phrase::fixe("Aucun logiciel du comptable retenu : rien à raccourcir."))),
        Some(f) => match FormatBorne::de(f) {
            Some(b) => (f, b),
            None => {
                let phrase = socle::phrase!("Le format « {} » ne borne pas ses libellés : rien à raccourcir.", f);
                return Ok(Libelles::de(Some(f), None, Vec::new(), phrase));
            }
        },
    };
    let libelles: Vec<Ligne> = depasses(ctx, format)?
        .into_iter()
        .map(|d| {
            let propose = lc::proposer(&d.long, d.largeur);
            let valeur = d.court.clone().unwrap_or_else(|| propose.clone());
            Ligne { quoi: d.quoi, zone: zone(&d), propose, cle: d.cle, long: d.long, largeur: d.largeur, court: d.court, valeur }
        })
        .collect();
    let reste = libelles.iter().filter(|l| l.court.is_none()).count();
    let phrase = if libelles.is_empty() {
        socle::phrase!("Tous les libellés tiennent dans le format {}.", borne.nom())
    } else if reste == 0 {
        socle::phrase!("{} : {} libellés trop longs, chacun a sa valeur courte.", borne.nom(), libelles.len())
    } else {
        socle::phrase!("{} : {} libellés trop longs, {} sans valeur courte. Garder ou changer chaque proposition, puis enregistrer.", borne.nom(), libelles.len(), reste)
    };
    Ok(Libelles::de(Some(format), Some(borne.nom()), libelles, phrase))
}

/// Le refus d'une valeur : sa phrase, et sa cle a part (il se dit seul, sans ce qui manque pour demarrer).
fn faute(phrase: String, cle: &str) -> Echec {
    let a_part = socle::echec::Lieu { cle: Some(cle.to_string()), ..Default::default() };
    Echec { a_part: Some(a_part.into()), ..erreur(socle::echec::Classe::Refuse, phrase) }
}

/// ENREGISTRER : chaque valeur se verifie (un libelle qui depasse, une valeur non vide qui tient) ; une seule faute et
/// rien ne s'ecrit.
fn enregistrer(ctx: &Contexte, c: ChargeEnregistrer) -> R<Libelles> {
    let Some(format) = format_de(ctx, c.format)? else {
        return Err(erreur(socle::echec::Classe::Refuse, "Aucun logiciel du comptable retenu : aucun libellé court ne s'enregistre. Le choisir dans les Exports."));
    };
    let Some(borne) = FormatBorne::de(format) else {
        return Err(faute(format!("Le format « {} » ne borne pas ses libellés : il n'a pas de libellé court.", format), format.as_str()));
    };
    if c.libelles.is_empty() {
        let phrase = socle::phrase!("Aucun libellé court à enregistrer pour {} : rien n'est écrit.", borne.nom());
        return Ok(Libelles::de(Some(format), Some(borne.nom()), Vec::new(), phrase));
    }
    let trop: Vec<Depasse> = depasses(ctx, format)?;
    let mut fichier = lc::lire(&ctx.racine)?;
    let courts = fichier.formats.entry(borne).or_default();
    let mut vus = BTreeSet::new();
    for v in &c.libelles {
        let Some(d) = trop.iter().find(|d| d.quoi == v.quoi && d.cle == v.cle) else {
            return Err(faute(format!("« {} » n'est pas un libellé trop long pour {} : rien n'est enregistré.", v.cle, borne.nom()), &v.cle));
        };
        let court = v.court.split_whitespace().collect::<Vec<_>>().join(" ");
        if court.is_empty() {
            return Err(faute(format!("La valeur courte de « {} » est vide : rien n'est enregistré.", d.long), &v.cle));
        }
        let n = court.chars().count();
        if n > d.largeur {
            return Err(faute(format!("« {} » a {} caractères, la zone en prend {} pour {} : la raccourcir, rien n'est enregistré.", court, n, d.largeur, borne.nom()), &v.cle));
        }
        if !vus.insert((v.quoi, v.cle.clone())) {
            return Err(faute(format!("« {} » est donné deux fois : rien n'est enregistré.", d.long), &v.cle));
        }
        courts.poser(v.quoi, v.cle.clone(), court);
    }
    if fichier.quoi.is_none() {
        fichier.quoi = Some("Les libellés courts que la compagnie a validés, par format du logiciel du comptable (Exports, « Libellés courts »).".to_string());
    }
    lc::ecrire(&ctx.racine, &fichier)?;
    let mut r = libelles_courts(ctx, ChargeLire { format: Some(format) })?;
    let reste = r.libelles.iter().filter(|l| l.court.is_none()).count();
    let suite = if reste == 0 { format!("chaque libellé trop long pour {} a sa valeur courte", borne.nom()) } else { format!("{} encore sans valeur courte pour {}", reste, borne.nom()) };
    r.phrase = socle::phrase!("{} enregistré{} ; {}.", socle::ecran::compte(c.libelles.len(), "libellé court", "libellés courts"), if c.libelles.len() > 1 { "s" } else { "" }, suite);
    Ok(r)
}
