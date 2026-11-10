//! LES ORGANISMES ET LES CONTRATS DE LA DSN : ce que la compagnie saisit, et seulement cela, s'ecrit dans
//! `config/organismes.json` ; ce qui se lit pour un mois. L'ecran (Configuration › Organismes et contrats) est la route
//! `gestes::organismes`.
//!
//! Le reste se compose a la date de chaque periode (`droit::organismes`, `Contexte::organismes`) : la partie normative,
//! les contrats que les textes donnent entiers (ceux de toutes les compagnies, ceux de la convention, la CASC-SVP, les
//! contributions de branche du bareme). Une compagnie qui n'a rien a saisir n'a rien a enregistrer.
//!
//! LE FICHIER EST DATE comme toute la configuration (`droit::fiche::dater`) : sa cle `adhesions_prevoyance` est une
//! liste `{depuis, valeur}`. Enregistrer pour un mois ecrit ce qui change a partir du premier jour de ce mois
//! (`droit::fiche::dater`) ; ce qui precede garde ses contrats. La premiere composition vaut de tout temps. Les identifiants
//! d'adhesion (S21.G00.15.005) attribues ne bougent plus.

use droit::fiche::Lue as _;
use serde::Deserialize;

use droit::contexte::Contexte;
use droit::organismes::{attendus, composer, ligne, saisis, Adhesion, Attendu, Saisie};
use socle::date::Jour;
use std::collections::BTreeMap;
use socle::ecriture::ecrire_json;
use socle::echec::Echec;

type R<T> = Result<T, Echec>;

/// La charge de `organismes-enregistrer` : ce que la compagnie saisit de chaque contrat (`{cle: {libelle, organisme}}`,
/// rien sans lui), et le mois a partir duquel ils valent (le mois en cours sans lui).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChargeEnregistrer {
    contrats: Option<BTreeMap<String, Saisie>>,
    mois: Option<String>,
}

impl ChargeEnregistrer {
    /// Les contrats `contrats`, a partir du mois en cours (l'import).
    pub fn des_contrats(contrats: BTreeMap<String, Saisie>) -> ChargeEnregistrer {
        ChargeEnregistrer { contrats: Some(contrats), mois: None }
    }
}

impl Lus {
    /// Les lignes des contrats.
    pub fn lignes(&self) -> &[droit::organismes::Ligne] {
        &self.lignes
    }
}

#[track_caller]
fn plain(m: impl Into<String>) -> Echec {
    socle::echec::noter_et(None, m)
}

#[track_caller]
fn refus(m: impl Into<String>) -> Echec {
    socle::echec::noter_et(Some(socle::echec::Classe::Refuse), m)
}

fn chemin(ctx: &Contexte) -> std::path::PathBuf {
    ctx.racine.join(socle::fichiers::CONFIG).join(droit::contexte::ORGANISMES)
}


/// Une ligne se montre-t-elle a cette compagnie ? Un contrat de la piece d'un permanent (`Attendu::piece` : son role,
/// prevoyance ou sante, les affiliations d'un salarie paye au mois) ne se montre que sur une periode ou un permanent se
/// propose ou est du (`permanents` : `engagement::permanents::Permanents::ouverts`, la meme reponse que partout).
fn se_montre(spec: &Attendu, permanents: bool) -> bool {
    spec.piece().is_none() || permanents
}

/// Les contrats des permanents se montrent-ils sur la periode regardee (`engagement::permanents`) ?
fn permanents_ouverts(ctx: &Contexte, (debut, fin): &(Jour, Jour)) -> R<bool> {
    Ok(engagement::permanents::Permanents::de(ctx, debut, fin).map_err(plain)?.ouverts())
}

/// La periode que l'ecran regarde : le mois `mois` de la charge (celui de l'onglet Paie), sinon celui du jour.
fn periode(ctx: &Contexte, mois: Option<&str>) -> R<(Jour, Jour)> {
    let mois = match mois.map(str::trim).filter(|m| !m.is_empty()) {
        Some(m) => socle::mois::Mois::lire(m).map_err(plain)?,
        None => ctx.aujourdhui.mois_civil(),
    };
    Ok((mois.premier_jour(), mois.dernier_jour()))
}

/// Ce que l'ecran des contrats lit (`lire`).
pub struct Lus {
    pub idcc: Option<String>,
    pub a_saisir: bool,
    pub mois: String,
    pub lignes: Vec<droit::organismes::Ligne>,
}

/// `organismes-contrats` : une ligne par contrat attendu qui vaut pour la compagnie (`se_montre`) et pour la periode
/// regardee (`Attendu::couvre`) ; `a_saisir` : une de ces lignes attend encore ce que la compagnie seule peut dire.
pub fn lire(ctx: &Contexte, mois: Option<&str>) -> R<Lus> {
    let periode = periode(ctx, mois)?;
    let ctx = &ctx.au(&periode.1).map_err(plain)?;
    let (saisi, _) = saisis(ctx).map_err(plain)?;
    let permanents = permanents_ouverts(ctx, &periode)?;
    let lignes: Vec<droit::organismes::Ligne> =
        attendus(ctx).map_err(plain)?.iter().filter(|s| se_montre(s, permanents) && s.couvre(&periode)).map(|s| ligne(s, &saisi)).collect();
    let a_saisir = lignes.iter().any(|l| l.libelle.is_none() || l.organisme.is_none());
    Ok(Lus { idcc: ctx.entreprise.idcc.portee().map(|i| i.trim().to_string()), a_saisir, mois: periode.0.mois_civil().to_string(), lignes })
}

/// `organismes-enregistrer` : compose les adhesions du mois (sous la configuration du mois) avec ce qui est saisi, refuse
/// un contrat propose dont la reference manque, ecrit ce qui change a partir du premier jour du mois
/// (`droit::fiche::dater` : la premiere composition vaut de tout temps), et rend la lecture. Ce qui precede le mois ne
/// change pas.
pub fn enregistrer(ctx: &Contexte, charge: ChargeEnregistrer) -> R<Lus> {
    let saisies: BTreeMap<String, Saisie> = charge.contrats.into_iter().flatten().collect();
    let periode = periode(ctx, charge.mois.as_deref())?;
    let du_mois = ctx.au(&periode.1).map_err(plain)?;
    let (saisi, prises) = saisis(&du_mois).map_err(plain)?;
    let compose = composer(&du_mois, &saisi, prises, &saisies, Some((&periode, permanents_ouverts(&du_mois, &periode)?))).map_err(refus)?;
    // Ce qu'un texte donne entier (sa reference de contrat et son identifiant) ne s'ecrit pas : il se recompose a la
    // date de chaque mois, sous la convention de ce mois.
    let fixes: Vec<String> = attendus(&du_mois).map_err(plain)?.iter().filter(|s| s.libelle_donne().is_some()).map(|s| s.cle.trim().to_string()).collect();
    let adhesions: Vec<Adhesion> = compose.adhesions_prevoyance.into_iter().filter(|a| !fixes.contains(&a.cle)).collect();
    let (en_vigueur, _) = saisis(&du_mois.au(&periode.0).map_err(plain)?).map_err(plain)?;
    // Le fichier ne garde que la saisie : tout autre cle se recompose des textes (`droit::organismes`).
    let mut date = droit::organismes::fichier_organismes(ctx).map_err(plain)?;
    date.quoi = Some(
        "Ce que la compagnie a saisi dans Configuration › Organismes et contrats, et les identifiants d'adhesion \
attribues, date : chaque cle est une liste {depuis, valeur}. Le reste se compose des textes livres a la date de chaque \
mois. Ne pas modifier a la main."
            .to_string(),
    );
    // Rien de saisi en vigueur s'ecrit aussi : la composition du mois devient la saisie de la periode.
    if en_vigueur.is_empty() || en_vigueur != adhesions {
        droit::fiche::dater(&mut date.adhesions_prevoyance, Some(adhesions), Some(&periode.0), None);
    }
    let p = chemin(ctx);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| plain(e.to_string()))?;
    }
    crate::avant::sauver_avant(&p)?;
    ecrire_json(&p, &date, 2).map_err(plain)?;
    lire(ctx, charge.mois.as_deref())
}
