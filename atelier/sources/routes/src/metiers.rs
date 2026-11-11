//! Configuration › Métiers : les metiers que la compagnie coche, ceux de sa convention et ceux qu'elle prend aux autres.

use droit::fiche::Lue as _;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::catalogue::{Acte, Affiche, Champ, Forme, Ecran, Fiche, Place, FORME, MANQUE, OPERATION};
use crate::catalogue::Route;
use crate::enveloppe::{en_json, Charge, Dite, Porte, Rendu};
use crate::fiches::{creer_dossier, ecrire, refus};
use compagnie::avant::sauver_avant;
use droit::contexte::Contexte;
use droit::emplois::{self, MetierDuDeroulant};
use socle::echec::R;

const METIERS_REND: &str = "emplois (les métiers proposés) et emplois_autres (ceux des autres conventions)";

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::DessineAvance(Place::Metiers);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::MetiersCatalogue,
        geste: "Dire les métiers du catalogue, ceux de la convention et les autres.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "metiers (le catalogue, chacun dit retenu ou non)",
        refus: &[OPERATION, FORME],
        acte: Acte::Lit,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::MetierRetenu,
        geste: "Proposer un métier du catalogue dans les listes, ou ne plus le proposer.",
        charge: &[Champ { nom: "code", libelle: Affiche::new("Métier"), forme: Forme::Texte, sens: "le code du métier au catalogue", avance: false }, Champ { nom: "retenu", libelle: Affiche::new("Le proposer"), forme: Forme::OuiNon, sens: "vrai : le proposer", avance: false }],
        exemple: r#"{"code":"CHE350","retenu":true}"#,
        rend: METIERS_REND,
        refus: &[OPERATION, MANQUE, FORME],
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: ECRAN,
    },
];

/// Une route de ce module, branchee sur l'enveloppe.
pub fn route(porte: &Porte, charge: &Charge) -> R<Rendu> {
    match porte.route {
        Route::MetiersCatalogue => {
            let super::SansCharge {} = charge.lire()?;
            en_json(metiers_catalogue(porte.ctx)?)
        }
        Route::MetierRetenu => en_json(metier_retenu(porte.ctx, charge.lire()?)?),
        autre => Err(super::enveloppe::route_inconnue(autre.nom())),
    }
}

impl Dite for MetiersDuCatalogue {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

impl Dite for EmploisRendus {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

/// `config/metiers-ajoutes.json` : les metiers que la compagnie a coches ou decoches dans Configuration > Metiers.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MetiersAjoutes {
    quoi: String,
    /// Pris a une autre convention collective.
    pub(crate) codes: Vec<String>,
    /// Decoches dans la sienne.
    pub(crate) retires: Vec<String>,
}

const QUOI_METIERS: &str = "Les metiers que cette compagnie a coches ou decoches dans Configuration > Metiers : `codes`, pris a une autre convention collective ; `retires`, decoches dans la sienne. Preference d'affichage : aucun calcul ne change. Le catalogue est livre avec le logiciel (donnees/emplois-base.json).";

/// Les metiers que la compagnie a ajoutes ou retires, sous sa racine.
pub(crate) const METIERS_AJOUTES: &str = "config/metiers-ajoutes.json";

impl MetiersAjoutes {
    /// LA SEULE LECTURE de `config/metiers-ajoutes.json` ; rien de pose s'il n'existe pas.
    pub(crate) fn lire(racine: &Path) -> R<MetiersAjoutes> {
        let p = racine.join(METIERS_AJOUTES);
        if !p.exists() {
            return Ok(MetiersAjoutes { quoi: QUOI_METIERS.into(), codes: Vec::new(), retires: Vec::new() });
        }
        socle::forme::lire_fichier(&p, "Les métiers retenus par la compagnie ne se lisent pas : effacez leur fichier, puis cochez-les de nouveau dans Configuration › Métiers.")
            .map_err(|m| m.reclasser(socle::echec::Classe::Illisible))
    }

    fn ecrire(&self, ctx: &Contexte) -> R<()> {
        let p = ctx.racine.join(METIERS_AJOUTES);
        if let Some(parent) = p.parent() {
            creer_dossier(parent)?;
        }
        sauver_avant(&p)?;
        ecrire(&p, &MetiersAjoutes { quoi: QUOI_METIERS.into(), codes: self.codes.clone(), retires: self.retires.clone() })
    }

    /// Ce metier est-il dans le deroulant de cette compagnie ? Ceux de sa convention le sont d'office ; elle en ajoute
    /// d'une autre, elle en retire des siens (Configuration > Metiers).
    fn visible(&self, m: &MetierDuDeroulant, idcc: Option<&str>) -> bool {
        !self.retires.contains(&m.code) && (de_la_convention(m, idcc) || self.codes.contains(&m.code))
    }
}

/// Le metier est-il de la convention `idcc` ? Sans IDCC (une compagnie qui s'installe), ou sans convention dite, rien ne
/// se cache.
fn de_la_convention(m: &MetierDuDeroulant, idcc: Option<&str>) -> bool {
    m.conventions.is_empty() || idcc.is_none_or(|i| m.conventions.iter().any(|c| c == i))
}

/// Les conventions d'un metier en mots : « Spectacle vivant privé (3090) » quand `idcc` est dit.
/// Une convention que le catalogue cite sans la nommer est un defaut de la donnee livree.
fn conventions_dites(m: &MetierDuDeroulant, noms: &BTreeMap<String, String>, avec_l_idcc: bool) -> R<String> {
    let dites = m
        .conventions
        .iter()
        .map(|c| match noms.get(c) {
            Some(nom) if avec_l_idcc => Ok(format!("{} ({})", nom, c)),
            Some(nom) => Ok(nom.clone()),
            None => Err(socle::echec::livree("emplois-base.json", &format!("conventions.{}", c)).reclasser(socle::echec::Classe::Illisible)),
        })
        .collect::<R<Vec<_>>>()?;
    Ok(dites.join(", "))
}

/// Les familles, et l'ordre ou les montrer : les artistes en tete.
fn rang_des_familles(familles: &[String]) -> HashMap<String, usize> {
    familles.iter().enumerate().map(|(i, f)| (f.clone(), i)).collect()
}

/// `metiers` : le catalogue des metiers que le deroulant de cette compagnie montre, groupe par famille (les artistes en
/// tete), chaque convention voyant ses metiers et seulement eux.
pub(crate) fn metiers(ctx: &Contexte) -> R<Vec<MetierDuDeroulant>> {
    let ordre = rang_des_familles(&emplois::familles()?);
    let retenus = MetiersAjoutes::lire(&ctx.racine)?;
    let idcc = ctx.entreprise.idcc.portee();
    let mut liste: Vec<MetierDuDeroulant> = emplois::catalogue(idcc)?.into_iter().filter(|m| retenus.visible(m, idcc)).collect();
    // Chaque famille du catalogue a son rang (`emplois::familles`).
    liste.sort_by_cached_key(|m| (ordre.get(&m.famille).copied(), m.libelle.to_lowercase()));
    Ok(liste)
}

/// Un metier d'une autre convention, que la compagnie peut ajouter a son deroulant : `de` dit lesquelles.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct MetierAutre {
    #[serde(flatten)]
    metier: MetierDuDeroulant,
    de: String,
}

/// `metiers_autres_conventions` : les metiers des AUTRES conventions, ceux que la compagnie peut ajouter a son deroulant.
pub(crate) fn metiers_autres_conventions(ctx: &Contexte) -> R<Vec<MetierAutre>> {
    let noms = emplois::noms_des_conventions()?;
    let retenus = MetiersAjoutes::lire(&ctx.racine)?;
    let idcc = ctx.entreprise.idcc.portee();
    let mut out: Vec<MetierAutre> =
        emplois::catalogue(idcc)?.into_iter().filter(|m| !retenus.visible(m, idcc)).map(|m| Ok(MetierAutre { de: conventions_dites(&m, &noms, true)?, metier: m })).collect::<R<_>>()?;
    out.sort_by(|a, b| (&a.de, a.metier.libelle.to_lowercase()).cmp(&(&b.de, b.metier.libelle.to_lowercase())));
    Ok(out)
}

/// Un metier de l'onglet Metiers : s'il est retenu, s'il est de la convention de la compagnie, et sinon de laquelle.
#[derive(Debug, Serialize)]
struct MetierDeLOnglet {
    #[serde(flatten)]
    metier: MetierDuDeroulant,
    retenu: bool,
    de_la_convention: bool,
    /// Les conventions d'un metier qui n'est pas de celle de la compagnie.
    de: Option<String>,
}

/// Ce que rend `metiers-catalogue`.
#[derive(Debug, Serialize)]
struct MetiersDuCatalogue {
    phrase: String,
    metiers: Vec<MetierDeLOnglet>,
    familles: Vec<String>,
}

/// L'onglet Metiers : tout le catalogue, chaque metier coche ou non.
fn metiers_catalogue(ctx: &Contexte) -> R<MetiersDuCatalogue> {
    let familles = emplois::familles()?;
    let ordre = rang_des_familles(&familles);
    let noms = emplois::noms_des_conventions()?;
    let retenus = MetiersAjoutes::lire(&ctx.racine)?;
    let idcc = ctx.entreprise.idcc.portee();
    let mut metiers: Vec<MetierDeLOnglet> = emplois::catalogue(idcc)?
        .into_iter()
        .map(|m| {
            let conv = de_la_convention(&m, idcc);
            let de = if conv { None } else { Some(conventions_dites(&m, &noms, false)?) };
            Ok(MetierDeLOnglet { retenu: retenus.visible(&m, idcc), de_la_convention: conv, de, metier: m })
        })
        .collect::<R<_>>()?;
    metiers.sort_by_cached_key(|m| (ordre.get(&m.metier.famille).copied(), m.metier.libelle.to_lowercase()));
    let phrase = format!(
        "Métiers du catalogue : {}. Ceux qui sont cochés se proposent dans la fiche d'un salarié et dans un engagement ; ceux de votre convention le sont d'office.",
        metiers.len()
    );
    Ok(MetiersDuCatalogue { phrase, metiers, familles })
}

// ---------------------------------------------------------------- les metiers

/// La charge de `metier-retenu`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MetierARetenir {
    code: String,
    retenu: bool,
}

/// Ce que rend `metier-retenu` : les metiers proposes et ceux des autres conventions.
#[derive(Debug, Serialize)]
struct EmploisRendus {
    phrase: String,
    emplois: Vec<MetierDuDeroulant>,
    emplois_autres: Vec<MetierAutre>,
}

fn metier_retenu(ctx: &Contexte, c: MetierARetenir) -> R<EmploisRendus> {
    let idcc = ctx.entreprise.idcc.portee();
    let catalogue = emplois::catalogue(idcc)?;
    let Some(m) = catalogue.iter().find(|m| m.code == c.code) else {
        return Err(refus(format!("Metier hors catalogue : {}.", c.code)));
    };
    let conv = de_la_convention(m, idcc);
    let mut retenus = MetiersAjoutes::lire(&ctx.racine)?;
    retenus.codes.retain(|x| *x != c.code);
    retenus.retires.retain(|x| *x != c.code);
    if c.retenu && !conv {
        retenus.codes.push(c.code.clone());
    }
    if !c.retenu && conv {
        retenus.retires.push(c.code.clone());
    }
    retenus.ecrire(ctx)?;
    let emplois = metiers(ctx)?;
    Ok(EmploisRendus { phrase: format!("Métiers retenus par la compagnie : {}.", emplois.len()), emplois, emplois_autres: metiers_autres_conventions(ctx)? })
}
