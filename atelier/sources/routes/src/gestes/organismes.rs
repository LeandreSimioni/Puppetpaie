//! L'ECRAN DES ORGANISMES ET DES CONTRATS DE LA DSN (Configuration › Organismes et contrats) : ses deux routes, qui
//! lisent et ecrivent par `compagnie::organismes`, et ce que l'ecran en lit.

use serde::{Deserialize, Serialize};

use socle::date::Jour;
use socle::echec::R;
use socle::mois::Mois;
use compagnie::organismes::{enregistrer, lire, ChargeEnregistrer, Lus};
use crate::enveloppe::{en_json, Charge, Dite, Porte, Rendu};
use crate::temps::TempsDeLaCase;

use crate::catalogue::{Acte, Affiche, Champ, Forme, Fiche, Refus, ILLISIBLE, OPERATION};
use crate::catalogue::Route;
use crate::catalogue::{Ecran, Place};

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Organismes);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::OrganismesContrats,
        geste: "Dire les contrats de la DSN (prévoyance, santé…) d'un mois, et ce qui manque.",
        charge: &[Champ { nom: "mois", libelle: Affiche::new("Mois"), forme: Forme::Mois, sens: "le mois regardé, AAAA-MM ; le mois en cours sinon", avance: false }],
        exemple: r#"{"mois":"2026-11"}"#,
        rend: "idcc, a_saisir, mois, contrats (une ligne par contrat attendu : libellé, organisme, periode, et les temps de ce qui en est saisi : avant, à venir, toutes ses valeurs datées), encarts",
        refus: &[OPERATION, ILLISIBLE],
        acte: Acte::Lit,
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
        phrase: 0,
        ecran: ECRAN,
    },
];

/// La charge de `organismes-contrats` : le mois regarde (le mois en cours sans lui).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeOrganismes {
    mois: Option<Mois>,
}

/// CE QUE RENDENT `organismes-contrats` ET `organismes-enregistrer` : la phrase, la convention, s'il reste a saisir, le mois
/// regarde, une ligne par contrat (et sa periode en clair), et ce que l'ecran en dit.
#[derive(Debug, Serialize)]
pub(crate) struct OrganismesDits {
    phrase: String,
    idcc: Option<String>,
    a_saisir: bool,
    mois: Mois,
    contrats: Vec<ContratDit>,
    encarts: Vec<crate::Encart>,
}

/// Une ligne de contrat, sa periode en clair (vide : elle vaut de tout temps), et les temps de ce que la compagnie en a
/// saisi (`temps_des_contrats`).
#[derive(Debug, Serialize)]
pub(crate) struct ContratDit {
    #[serde(flatten)]
    ligne: droit::organismes::Ligne,
    periode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    temps: Option<TempsDeLaCase<ContratSaisi>>,
}

/// Ce que la compagnie a saisi d'un contrat, a une date : sa reference et son organisme.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct ContratSaisi {
    libelle: String,
    organisme: String,
}

/// LES TEMPS DE CHAQUE CONTRAT SAISI a la date du jour (`crate::temps::d_une_case`), par sa cle : la case datee
/// `adhesions_prevoyance` de `config/organismes.json`, lue contrat par contrat (sa valeur a chaque version, une fois tant
/// qu'elle ne change pas ; absent d'une version, il n'y est pas saisi).
fn temps_des_contrats(ctx: &droit::contexte::Contexte) -> R<Vec<TempsDeLaCase<ContratSaisi>>> {
    let fichier = droit::organismes::fichier_organismes(ctx)?;
    let Some(liste) = fichier.adhesions_prevoyance.as_deref() else { return Ok(Vec::new()) };
    let mut par_cle: Vec<(String, Vec<droit::fiche::Datee<ContratSaisi>>)> = Vec::new();
    for version in droit::fiche::valeurs_datees(liste) {
        for a in version.valeur.iter().flatten() {
            if !par_cle.iter().any(|(c, _)| c == a.cle.trim()) {
                par_cle.push((a.cle.trim().to_string(), Vec::new()));
            }
        }
        for (cle, datees) in par_cle.iter_mut() {
            let valeur = version.valeur.iter().flatten().find(|a| a.cle.trim() == cle).map(|a| ContratSaisi { libelle: a.libelle.trim().to_string(), organisme: a.delegataire.trim().to_string() });
            if datees.last().is_some_and(|d| d.valeur == valeur) {
                continue;
            }
            datees.push(droit::fiche::Datee { depuis: version.depuis, valeur, note: version.note.clone() });
        }
    }
    let mut out = Vec::new();
    for (cle, datees) in par_cle {
        out.extend(crate::temps::d_une_case(&cle, &datees, &ctx.aujourdhui, |a, b| a == b)?);
    }
    Ok(out)
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
            (lire(ctx, c.mois)?, false)
        }
        Route::OrganismesEnregistrer => {
            let c: ChargeEnregistrer = charge.lire()?;
            (enregistrer(ctx, c)?, true)
        }
        autre => return Err(crate::enveloppe::route_inconnue(autre.nom())),
    };
    en_json(dire(lu, enregistre, temps_des_contrats(ctx)?)?)
}

/// Ce que l'ecran lit des contrats : sa phrase (ce qui reste a dire a la compagnie, sinon combien de contrats la DSN
/// declare), d'ou viennent les valeurs, les contrats de quel mois, ce qui manque ; la periode de chaque contrat.
fn dire(lu: Lus, enregistre: bool, mut temps: Vec<TempsDeLaCase<ContratSaisi>>) -> R<OrganismesDits> {
    let n = lu.lignes.len();
    let p = if lu.a_saisir {
        format!("Contrats de la DSN : {} ; il en reste à saisir.", n)
    } else {
        format!("Contrats de la DSN : {} ; rien à saisir.", n)
    };
    let phrase = if enregistre { format!("Enregistré. {}", p) } else { p };
    let mois = lu.mois;
    let mut encarts = vec![crate::Encart::de("information", "", "Ce que la DSN déclare de vos adhésions de prévoyance et de santé. La référence du contrat et le code de l'organisme se lisent sur le certificat d'adhésion et sur la fiche de paramétrage DSN de l'organisme. Le reste (organismes payés, Urssaf, taux AT/MP, compte France Travail) vient de la loi, de la convention et de la fiche Entreprise.")];
    encarts.push(crate::Encart::de("information", "", format!("Les contrats en vigueur en {}.", mois.libelle(socle::mois::Graphie::Accentuee))));
    if lu.a_saisir {
        encarts.push(crate::Encart::de("a_faire", "", "Une référence de contrat manque : la DSN refuse les salariés que ce contrat couvre tant qu'elle n'est pas enregistrée."));
    }
    let en_clair = |j: &Option<String>| -> R<Option<String>> {
        match j.as_deref().filter(|j| !j.is_empty()) {
            Some(j) => {
                let tete = j.get(..10).ok_or_else(|| socle::refus!("jour illisible : {}", socle::fichiers::repr(j)))?;
                Ok(Some(Jour::lire(tete)?.jj_mm_aaaa()))
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
        let temps = temps.iter().position(|t| t.champ == ligne.cle).map(|i| temps.swap_remove(i));
        contrats.push(ContratDit { ligne, periode, temps });
    }
    Ok(OrganismesDits { phrase, idcc: lu.idcc, a_saisir: lu.a_saisir, mois, contrats, encarts })
}
