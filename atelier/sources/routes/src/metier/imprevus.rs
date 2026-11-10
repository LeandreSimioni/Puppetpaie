//! LE MAGASIN DES ARRETS (`imprevus/AAAA-MM.json`, `engagement::imprevus`) : ses refus, les arrets qu'il porte, les
//! nomenclatures du bloc 60, et l'arret tel qu'il s'y ecrit. Les routes des imprevus (`routes::imprevus`), l'horloge et
//! les obligations le prennent ici.

use compagnie::{maintenant, INTERFACE};
use droit::arrets::Bloc60;
use droit::contexte::{Contexte, IdSalarie};
use engagement::imprevus::{self, ArretSaisi, GenreArret, Imprevu};
use engagement::Chiffre;
use socle::date::Jour;
use socle::echec::{Classe, R};

/// UN CONTRAT QUI COUVRE UN JOUR, lu dans les saisies : le salarie, son nom, le numero, le debut et la fin, le mois qui le
/// saisit, le spectacle (tels que l'engagement les ecrit), paye au mois ou non, sa nature ; `arret-contrats` le dit
/// (`dit`).
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct ContratAuJour {
    pub(crate) salarie: IdSalarie,
    pub(crate) nom: String,
    pub numero: Option<String>,
    pub debut: String,
    pub(crate) fin: Option<String>,
    pub(crate) mois: String,
    pub(crate) spectacle: Option<String>,
    pub(crate) mensuel: bool,
    pub(crate) nature: Option<droit::natures::NatureDeContrat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) dit: Option<String>,
}

/// Un refus du magasin des imprevus (`imprevus`) : un fichier abime, un genre inconnu.
pub(crate) fn magasin<T>(r: R<T>) -> R<T> {
    r.map_err(|e| e.reclasser(Classe::Refuse))
}

/// Les arrets de travail du magasin des imprevus (`imprevus/AAAA-MM.json`, genre `arret`), dans l'ordre des mois du
/// magasin.
pub(crate) fn tous_les_arrets(ctx: &Contexte) -> R<Vec<ArretSaisi>> {
    Ok(magasin(imprevus::tous(&ctx.racine))?
        .into_iter()
        .filter_map(|i| match i {
            Imprevu::Arret(a) => Some(a),
            Imprevu::Suspension(_) => None,
        })
        .collect())
}

/// Les nomenclatures du bloc 60 (`arrets-base.json`), lues par leur forme (`droit::arrets::bloc_60`).
pub(crate) fn nomenclatures() -> R<Bloc60> {
    droit::arrets::bloc_60()
}

/// CE QUI FAIT UN ARRET A ECRIRE : qui, quel contrat, le motif et les jours, et ce que la saisie en dit d'autre.
pub(crate) struct ArretDit<'a> {
    pub cle: &'a str,
    pub contrat: &'a ContratAuJour,
    pub motif: &'a str,
    pub dernier_jour: Jour,
    pub debut: Jour,
    pub fin: Jour,
    pub date_accident: Option<Jour>,
    pub jours_deja_indemnises: Option<u32>,
    pub note: Option<String>,
}

/// UN ARRET TEL QUE LE MAGASIN L'ECRIT (`engagement::imprevus::ArretSaisi`) : ce qu'il n'a pas encore reste vide (la
/// subrogation, que le droit tire ensuite : `poser_la_subrogation` ; la reprise ; les indemnites journalieres).
pub(crate) fn arret_a_ecrire(x: ArretDit) -> ArretSaisi {
    ArretSaisi {
        genre: GenreArret::Arret,
        salarie: x.cle.to_string(),
        contrat_debut: x.contrat.debut.clone(),
        contrat_numero: x.contrat.numero.clone(),
        motif: x.motif.to_string(),
        dernier_jour_travaille: x.dernier_jour.iso(),
        debut: x.debut.iso(),
        fin_previsionnelle: x.fin.iso(),
        subrogation: None,
        subrogation_debut: None,
        subrogation_fin: None,
        reprise: None,
        reprise_motif: None,
        date_accident: x.date_accident.map(|j| j.iso()),
        jours_deja_indemnises: x.jours_deja_indemnises.map(|j| Chiffre::entier(i64::from(j))),
        ij_brute: None,
        ij_nette: None,
        ij_brute_majoree: None,
        ij_nette_majoree: None,
        prolongations: Vec::new(),
        reprises: Vec::new(),
        saisi_le: maintenant(),
        qui: INTERFACE.to_string(),
        note: x.note,
    }
}
