//! L'ECRAN DES ORGANISMES ET DES CONTRATS DE LA DSN (Configuration › Organismes et contrats) : ses deux routes, qui
//! lisent et ecrivent par `compagnie::organismes`, et ce que l'ecran en lit.

use serde::{Deserialize, Serialize};

use socle::date::Jour;
use socle::echec::Echec;
use compagnie::organismes::{enregistrer, lire, ChargeEnregistrer, Lus};
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
