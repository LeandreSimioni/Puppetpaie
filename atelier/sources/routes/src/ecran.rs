//! L'ECRAN QU'UNE ROUTE REND (`catalogue::Ecran::Montre`) : ce qu'il dit en tete, puis ses rangs, chacun son titre, ce
//! qu'il dit, sa pastille, ce qui s'y regarde et ses gestes. La page le montre tel quel (`js/avance.js`,
//! `montrerLEcran`) ; qui lit la reponse en ligne de commande voit la meme chose.

use crate::imprevus::GesteDeRoute;
use crate::ouverture::DocumentMontre;
use socle::ecran::Pastille;

/// UN ECRAN QU'UNE ROUTE REND : ce qu'il dit en tete (`intro`), puis ses rangs dans l'ordre ou il les montre. `C` : ce
/// qui designe la chose d'un rang aux routes de ses gestes et de ce qui s'y regarde ; `V`, `X` : ce que les cases de ses
/// gestes montrent d'abord et ce qu'elles offrent (`GesteDeRoute`).
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct EcranRendu<C, V, X> {
    pub(crate) intro: String,
    pub(crate) rangs: Vec<RangDeLEcran<C, V, X>>,
}

/// UN RANG D'UN ECRAN RENDU : son titre, ce qu'il dit (une ligne chacune), sa pastille et ce qui la suit (`etat`), le
/// document qui s'y regarde (`montre`, aucun : rien a regarder) et ses gestes, ceux seulement que le moteur ouvre.
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct RangDeLEcran<C, V, X> {
    pub(crate) titre: String,
    pub(crate) dit: Vec<String>,
    pub(crate) pastille: Pastille,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) etat: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) montre: Option<DocumentMontre<C>>,
    pub(crate) gestes: Vec<GesteDeRoute<C, V, X>>,
}
