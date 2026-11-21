//! LES SERVICES DE PREVENTION ET DE SANTE AU TRAVAIL (`spst-base.json`, extrait par `moteur-atelier extraire spst`) : le
//! referentiel « sst_dpae » de l'Urssaf, un service par identifiant (`droit::fiche::IdentifiantSpst`), son nom et sa
//! commune. La page du jeu le dit la liste « qui doit être utilisée dans le cadre de la Déclaration Préalable à
//! l'Embauche (DPAE) » (`atelier/reference/sources/urssaf-sst-dpae/`), la fiche consigne 3370 celle de la DSN. La case
//! « Service de santé au travail » de la fiche Entreprise s'y choisit par le nom du service (`choix`). `base`
//! est sa seule lecture.

use std::sync::{Arc, OnceLock};

use serde::Deserialize;

use droit::fiche::{CodeMedecine, IdentifiantSpst};
use socle::echec::R;
use socle::fichiers::{forme_en_cache, FormeGardee};

/// `spst-base.json`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaseSpst {
    #[serde(rename = "quoi")]
    _quoi: String,
    #[serde(rename = "note")]
    _note: String,
    pub services: Vec<Service>,
}

/// Un service du referentiel : son identifiant (`mt_lib_court`), son nom (`mt_lib`, sans les chiffres de son identifiant
/// devant) et sa commune (`mt_bur_dist`), tels que le referentiel les ecrit.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Service {
    pub code: IdentifiantSpst,
    pub nom: String,
    pub commune: String,
}

/// THALIE SANTE (ex-CMB, Centre medical de la Bourse), le service que nos conventions nomment pour les intermittents du
/// spectacle : la 3090, art. 11.8 (« l'article 3.2 de l'accord collectif national interbranche relatif à la santé au
/// travail des salariés intermittents du spectacle qui désigne le centre médical de la bourse ») ; la 3097, art. 26
/// (« l'employeur doit obligatoirement cotiser au centre médical de la Bourse ») ; la 1285, art. XVIII.1 (« Thalie Santé
/// (ex-CMB) ») : `atelier/reference/sources/kali-entrepot/`. Aucun texte depose ne l'impose a toute compagnie
/// d'intermittents : il ne se propose pas, il se range en tete de la liste.
pub const THALIE_SANTE: &str = "MT176";

/// Le referentiel livre, lu par sa forme.
pub fn base() -> R<Arc<BaseSpst>> {
    static CACHE: OnceLock<FormeGardee<BaseSpst>> = OnceLock::new();
    forme_en_cache(
        &CACHE,
        &const { socle::fichiers::CheminRelatif::du_moteur("spst-base.json") },
        "Le référentiel des services de santé au travail livré avec le logiciel ne se lit pas : le signaler au développeur.",
        Ok,
    )
}

impl BaseSpst {
    /// LES CHOIX DE LA CASE `medecine_travail` : chaque service, son code tel que la fiche l'ecrit
    /// (`CodeMedecine::du_referentiel` : MT999 pour la medecine interne) et ce que la page en montre, « nom, commune » (le
    /// nom seul pour la medecine interne, dont le referentiel ecrit la commune « SANS ») ; Thalie Sante d'abord
    /// (`THALIE_SANTE`), puis les autres par leur nom.
    pub fn choix(&self) -> R<Vec<(String, String)>> {
        let mut services: Vec<&Service> = self.services.iter().collect();
        services.sort_by(|a, b| (a.code.as_str() != THALIE_SANTE, &a.nom, &a.code).cmp(&(b.code.as_str() != THALIE_SANTE, &b.nom, &b.code)));
        services
            .into_iter()
            .map(|s| {
                let code = CodeMedecine::du_referentiel(&s.code)?.to_string();
                let dit = if s.code.interne() { s.nom.clone() } else { format!("{}, {}", s.nom, s.commune) };
                Ok((code, dit))
            })
            .collect()
    }
}
