//! L'ECRAN DES ORGANISMES ET DES CONTRATS DE LA DSN (Configuration › Organismes et contrats) : ce que la compagnie y
//! saisit, et seulement cela, s'ecrit dans `config/organismes.json`.
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
use serde::{Deserialize, Serialize};

use droit::contexte::Contexte;
use droit::organismes::{attendus, composer, ligne, saisis, Adhesion, Attendu, Saisie};
use socle::date::Jour;
use std::collections::BTreeMap;
use socle::ecriture::ecrire_json;
use socle::echec::Echec;
use crate::routes::enveloppe::{en_json, Charge, Dite, Porte, Rendu};

use crate::routes::catalogue::{Acte, Affiche, Champ, Forme, Fiche, Refus, ILLISIBLE, OPERATION};
use crate::routes::catalogue::Route;
use crate::routes::catalogue::{Ecran, Place};

type R<T> = Result<T, Echec>;

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Organismes);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::OrganismesContrats,
        geste: "Dire les contrats de la DSN (prévoyance, santé…) d'un mois, et ce qui manque.",
        charge: &[Champ { nom: "mois", libelle: Affiche::new("Mois"), forme: Forme::Mois, sens: "le mois regardé, AAAA-MM ; le mois en cours sinon", avance: false }],
        exemple: r#"{"mois":"2026-11"}"#,
        rend: "idcc, a_saisir, mois, contrats (une ligne par contrat attendu : libellé, organisme, periode), encarts",
        refus: &[OPERATION, ILLISIBLE],
        acte: Acte::Lit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::OrganismesEnregistrer,
        geste: "Écrire, à partir d'un mois, les contrats de la DSN que la compagnie dit.",
        charge: &[
            Champ { nom: "contrats", libelle: Affiche::new("Contrats"), forme: Forme::Objet, sens: "{clé du contrat: {libelle, organisme}}, ce qui est saisi", avance: false },
            Champ { nom: "mois", libelle: Affiche::new("À partir de"), forme: Forme::Mois, sens: "le mois à partir duquel ils valent, AAAA-MM ; le mois en cours sinon", avance: false },
        ],
        exemple: r#"{"mois":"2026-11","contrats":{"prevoyance_permanents":{"libelle":"PREVOYANCE","organisme":"P0983"},"sante_permanents":{"libelle":"SANTE","organisme":"P0983"}}}"#,
        rend: "ce que organismes-contrats rend",
        refus: &[Refus { classe: socle::echec::Classe::Refuse, corriger: "la référence du contrat que la phrase nomme ; rien n'est écrit" }, ILLISIBLE],
        acte: Acte::Ecrit,
        avance: false,
        phrase: 0,
        ecran: ECRAN,
    },
];

/// La charge de `organismes-contrats` : le mois regarde (le mois en cours sans lui).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChargeOrganismes {
    mois: Option<String>,
}

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
    pub(crate) fn des_contrats(contrats: BTreeMap<String, Saisie>) -> ChargeEnregistrer {
        ChargeEnregistrer { contrats: Some(contrats), mois: None }
    }
}

impl Lus {
    /// Les lignes des contrats.
    pub(crate) fn lignes(&self) -> &[droit::organismes::Ligne] {
        &self.lignes
    }
}

/// CE QUE RENDENT `organismes-contrats` ET `organismes-enregistrer` : la phrase, la convention, s'il reste a saisir, le mois
/// regarde, une ligne par contrat (et sa periode en clair), et ce que l'ecran en dit.
#[derive(Debug, Serialize)]
pub struct OrganismesDits {
    phrase: String,
    idcc: Option<String>,
    a_saisir: bool,
    mois: String,
    contrats: Vec<ContratDit>,
    encarts: Vec<crate::routes::Encart>,
}

/// Une ligne de contrat, et sa periode en clair (vide : elle vaut de tout temps).
#[derive(Debug, Serialize)]
pub struct ContratDit {
    #[serde(flatten)]
    ligne: droit::organismes::Ligne,
    periode: String,
}

impl Dite for OrganismesDits {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

/// Une route de ce module.
pub fn route(porte: &Porte, charge: &Charge) -> R<Rendu> {
    let ctx = porte.ctx;
    let (lu, enregistre) = match porte.route {
        Route::OrganismesContrats => {
            let c: ChargeOrganismes = charge.lire()?;
            (lire(ctx, c.mois.as_deref())?, false)
        }
        Route::OrganismesEnregistrer => {
            let c: ChargeEnregistrer = charge.lire()?;
            (enregistrer(ctx, c)?, true)
        }
        autre => return Err(crate::routes::enveloppe::route_inconnue(autre.nom())),
    };
    en_json(dire(lu, enregistre)?)
}

/// Ce que l'ecran lit des contrats : sa phrase (ce qui reste a dire a la compagnie, sinon combien de contrats la DSN
/// declare), d'ou viennent les valeurs, les contrats de quel mois, ce qui manque ; la periode de chaque contrat.
fn dire(lu: Lus, enregistre: bool) -> R<OrganismesDits> {
    let n = lu.lignes.len();
    let p = if lu.a_saisir {
        format!("Contrats de la DSN : {} ; il en reste à saisir.", n)
    } else {
        format!("Contrats de la DSN : {} ; rien à saisir.", n)
    };
    let phrase = if enregistre { format!("Enregistré. {}", p) } else { p };
    let mut encarts = vec![crate::routes::Encart::de("information", "", "Ce que la DSN déclare de vos adhésions de prévoyance et de santé. La référence du contrat et le code de l'organisme se lisent sur le certificat d'adhésion et sur la fiche de paramétrage DSN de l'organisme. Le reste (organismes payés, Urssaf, taux AT/MP, compte France Travail) vient de la loi, de la convention et de la fiche Entreprise.")];
    encarts.push(crate::routes::Encart::de("information", "", format!("Les contrats en vigueur en {}.", socle::mois::Mois::en_clair(&lu.mois))));
    if lu.a_saisir {
        encarts.push(crate::routes::Encart::de("a_faire", "", "Une référence de contrat manque : la DSN refuse les salariés que ce contrat couvre tant qu'elle n'est pas enregistrée."));
    }
    let en_clair = |j: &Option<String>| -> R<Option<String>> {
        match j.as_deref().filter(|j| !j.is_empty()) {
            Some(j) => {
                let tete = j.get(..10).ok_or_else(|| plain(format!("jour illisible : {}", socle::fichiers::repr(j))))?;
                Ok(Some(Jour::lire(tete).map_err(plain)?.jj_mm_aaaa()))
            }
            None => Ok(None),
        }
    };
    let mut contrats = Vec::new();
    for ligne in lu.lignes {
        let periode = match (en_clair(&ligne.depuis)?, en_clair(&ligne.jusqu_a)?) {
            (Some(de), Some(a)) => format!("Du {} au {}.", de, a),
            (Some(de), None) => format!("Depuis le {}.", de),
            (None, Some(a)) => format!("Jusqu'au {}.", a),
            (None, None) => String::new(),
        };
        contrats.push(ContratDit { ligne, periode });
    }
    Ok(OrganismesDits { phrase, idcc: lu.idcc, a_saisir: lu.a_saisir, mois: lu.mois, contrats, encarts })
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


/// Une ligne se montre-t-elle a cette compagnie ? Un contrat des permanents (`role` : prevoyance ou sante, les
/// affiliations d'un salarie paye au mois) ne se montre qu'a une compagnie qui en emploie (rien a remplir pour qui
/// n'emploie pas de permanent) : le curseur est `Contexte::emploie_des_permanents`, le meme que partout.
fn se_montre(ctx: &Contexte, spec: &Attendu) -> bool {
    spec.role.as_deref().is_none_or(|r| r.trim().is_empty()) || ctx.emploie_des_permanents()
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
    idcc: Option<String>,
    a_saisir: bool,
    mois: String,
    lignes: Vec<droit::organismes::Ligne>,
}

/// `organismes-contrats` : une ligne par contrat attendu qui vaut pour la compagnie (`se_montre`) et pour la periode
/// regardee (`Attendu::couvre`) ; `a_saisir` : une de ces lignes attend encore ce que la compagnie seule peut dire.
pub fn lire(ctx: &Contexte, mois: Option<&str>) -> R<Lus> {
    let periode = periode(ctx, mois)?;
    let ctx = &ctx.au(&periode.1).map_err(plain)?;
    let (saisi, _) = saisis(ctx).map_err(plain)?;
    let lignes: Vec<droit::organismes::Ligne> =
        attendus(ctx).map_err(plain)?.iter().filter(|s| se_montre(ctx, s) && s.couvre(&periode)).map(|s| ligne(s, &saisi)).collect();
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
    let compose = composer(&du_mois, &saisi, prises, &saisies, Some(&periode)).map_err(refus)?;
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
    crate::routes::mois::sauver_avant(&p)?;
    ecrire_json(&p, &date, 2).map_err(plain)?;
    lire(ctx, charge.mois.as_deref())
}
