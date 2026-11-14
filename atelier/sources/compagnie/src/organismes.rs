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

use serde::Deserialize;

use droit::contexte::Contexte;
use droit::organismes::{attendus, composer, ligne, saisis, Adhesion, Attendu, CleDeContrat, Saisie, Saisies};
use socle::date::Jour;
use std::collections::BTreeMap;
use socle::ecriture::ecrire_json;
use socle::echec::R;
use socle::mois::Mois;

/// La charge de `organismes-enregistrer` : ce que la compagnie saisit de chaque contrat (`{cle: {libelle, organisme}}`,
/// rien sans lui ; les fautes de tous se disent ensemble, `Saisies`), et le mois a partir duquel ils valent (le mois en
/// cours sans lui).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChargeEnregistrer {
    contrats: Option<Saisies>,
    mois: Option<Mois>,
}

impl ChargeEnregistrer {
    /// Les contrats `contrats`, a partir du mois en cours (l'import).
    pub fn des_contrats(contrats: BTreeMap<CleDeContrat, Saisie>) -> ChargeEnregistrer {
        ChargeEnregistrer { contrats: Some(Saisies(contrats)), mois: None }
    }
}

impl Lus {
    /// Les lignes des contrats.
    pub fn lignes(&self) -> &[droit::organismes::Ligne] {
        &self.lignes
    }
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
    Ok(engagement::permanents::Permanents::de(ctx, debut, fin)?.ouverts())
}

/// La periode que l'ecran regarde : le mois `mois` de la charge (celui de l'onglet Paie), sinon celui du jour.
fn periode(ctx: &Contexte, mois: Option<Mois>) -> (Jour, Jour) {
    let mois = match mois {
        Some(m) => m,
        None => ctx.aujourdhui.mois_civil(),
    };
    (mois.premier_jour(), mois.dernier_jour())
}

/// Ce que l'ecran des contrats lit (`lire`).
pub struct Lus {
    pub idcc: Option<droit::fiche::Idcc>,
    pub a_saisir: bool,
    pub mois: Mois,
    pub lignes: Vec<droit::organismes::Ligne>,
}

/// `organismes-contrats` : une ligne par contrat attendu qui vaut pour la compagnie (`se_montre`) et pour la periode
/// regardee (`Attendu::couvre`) ; `a_saisir` : une de ces lignes attend encore ce que la compagnie seule peut dire.
pub fn lire(ctx: &Contexte, mois: Option<Mois>) -> R<Lus> {
    let periode = periode(ctx, mois);
    let ctx = &ctx.au(&periode.1)?;
    let (saisi, _) = saisis(ctx)?;
    let permanents = permanents_ouverts(ctx, &periode)?;
    let lignes: Vec<droit::organismes::Ligne> =
        attendus(ctx)?.iter().filter(|s| se_montre(s, permanents) && s.couvre(&periode)).map(|s| ligne(s, &saisi)).collect();
    let a_saisir = lignes.iter().any(|l| l.libelle.is_none() || l.organisme.is_none());
    Ok(Lus { idcc: ctx.entreprise.idcc.clone(), a_saisir, mois: periode.0.mois_civil(), lignes })
}

/// `organismes-enregistrer` : compose les adhesions du mois (sous la configuration du mois) avec ce qui est saisi, refuse
/// un contrat propose dont la reference manque, ecrit ce qui change a partir du premier jour du mois
/// (`droit::fiche::dater` : la premiere composition vaut de tout temps), et rend la lecture. Ce qui precede le mois ne
/// change pas.
pub fn enregistrer(ctx: &Contexte, charge: ChargeEnregistrer) -> R<Lus> {
    let saisies: BTreeMap<CleDeContrat, Saisie> = charge.contrats.into_iter().flat_map(|s| s.0).collect();
    let periode = periode(ctx, charge.mois);
    let du_mois = ctx.au(&periode.1)?;
    let (saisi, prises) = saisis(&du_mois)?;
    let compose = composer(&du_mois, &saisi, prises, &saisies, Some((&periode, permanents_ouverts(&du_mois, &periode)?))).map_err(|e| e.reclasser(socle::echec::Classe::Refuse))?;
    // Ce qu'un texte donne entier (sa reference de contrat et son identifiant) ne s'ecrit pas : il se recompose a la
    // date de chaque mois, sous la convention de ce mois.
    let fixes: Vec<CleDeContrat> = attendus(&du_mois)?.iter().filter(|s| s.libelle_donne().is_some()).map(|s| s.cle.clone()).collect();
    let adhesions: Vec<Adhesion> = compose.adhesions_prevoyance.into_iter().filter(|a| !fixes.contains(&a.cle)).collect();
    let (en_vigueur, _) = saisis(&du_mois.au(&periode.0)?)?;
    // Le fichier ne garde que la saisie : tout autre cle se recompose des textes (`droit::organismes`).
    let mut date = droit::organismes::fichier_organismes(ctx)?;
    date.quoi = Some(
        "Ce que la compagnie a saisi dans Configuration › Organismes et contrats, et les identifiants d'adhesion \
attribues, date : chaque cle est une liste {depuis, valeur}. Le reste se compose des textes livres a la date de chaque \
mois. Ne pas modifier a la main."
            .to_string(),
    );
    // Rien de saisi en vigueur s'ecrit aussi : la composition du mois devient la saisie de la periode.
    if en_vigueur.is_empty() || en_vigueur != adhesions {
        droit::fiche::dater(&mut date.adhesions_prevoyance, Some(adhesions), Some(&periode.0), false, None);
    }
    let p = chemin(ctx);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| socle::echec::refus(e.to_string()))?;
    }
    crate::avant::sauver_avant(&p)?;
    ecrire_json(&p, &date, 2)?;
    lire(ctx, charge.mois)
}
