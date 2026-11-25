//! L'ECRAN QU'UNE ROUTE REND (`catalogue::Ecran::Rendu`), tel qu'on le voit et le saisit : ce qu'il dit en tete, ses
//! rangs (chacun son titre, ce qu'il dit, sa pastille, ce qui s'y regarde et ses gestes), ses cases, l'historique de ses
//! cases datees et le geste qui les enregistre. La page le pose tel quel (`js/avance.js`, `poserLEcranRendu`) ; qui lit
//! la reponse en ligne de commande voit la meme chose.

use serde::Serialize;

use crate::catalogue::{Affiche, Route};
use crate::fiches::{CaseDeLaFiche, HistoireDUneCase};
use crate::imprevus::GesteDeRoute;
use crate::ouverture::DocumentMontre;
use crate::{Encart, SansCharge};
use droit::fiche::CaseDeFiche;
use socle::ecran::Pastille;

/// CE QUE LA PAGE FAIT DE LA REPONSE D'UN GESTE, dit par la route (`apres`) : la page ne le devine pas a la forme de la
/// reponse, elle suit ce mot ; qui lit la reponse en ligne de commande le lit de meme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ApresLeGeste {
    /// La reponse porte un ecran (`EcranRendu`) : il se pose a la place de celui ou le geste s'est fait.
    PoserLEcran,
    /// L'ecran ou le geste s'est fait se relit (la route qui rend sa place).
    Relire,
    /// La compagnie que la reponse nomme (`ouvrir`) s'ouvre.
    Ouvrir,
    /// La compagnie ouverte s'est fermee le temps du geste et revient changee (« Prendre la main » a recupere le Drive) :
    /// la page la relit entiere.
    Rouvrir,
}

/// UN ECRAN QU'UNE ROUTE REND : ce qu'il dit en tete (`encarts`), ses rangs dans l'ordre ou il les montre, ses cases
/// (`CaseDeLaFiche`, dessinees comme celles d'une fiche), l'historique de ses cases datees en mots, et le geste qui
/// enregistre ses cases (aucun sans cases). Ce qui est vide ne s'ecrit pas. `C` : la charge que l'ecran donne a ses
/// routes (ce qui designe la chose d'un rang a ses gestes et a ce qui s'y regarde, ou la charge du geste qui
/// enregistre) ; `K` : la cle de ses cases ; `V`, `X` : ce que les cases des gestes de ses rangs montrent d'abord et ce
/// qu'elles offrent (`GesteDeRoute`).
#[derive(Debug, Serialize)]
pub(crate) struct EcranRendu<C, K = CaseDeFiche, V = SansCharge, X = SansCharge> {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) encarts: Vec<Encart>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) rangs: Vec<RangDeLEcran<C, V, X>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) cases: Vec<CaseDeLaFiche<K>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) historique: Vec<HistoireDUneCase>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) enregistrer: Option<EnregistrerLEcran<C>>,
}

/// UN RANG D'UN ECRAN RENDU : son titre, ce qu'il dit (une ligne chacune), sa pastille et ce qui la suit (`etat`), le
/// document qui s'y regarde (`montre`, aucun : rien a regarder) et ses gestes, ceux seulement que le moteur ouvre.
#[derive(Debug, Clone, Serialize)]
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

/// LE GESTE QUI ENREGISTRE LES CASES D'UN ECRAN RENDU : sa route, sa charge (`C`), le champ de cette charge ou vont les
/// cases de l'ecran (`dans` : chacune sous sa cle, une case vide nulle ; aucun : chacune sous sa cle dans la charge
/// meme, une case vide absente, celles d'une boite de saisie, `imprevus::SaisieDUnImprevu`), et son libelle. Ce n'est
/// pas un `GesteDeRoute` : un geste fait saisir dans sa boite des champs de la charge de sa route, que sa fiche decrit ;
/// celui-ci envoie les cases que l'ecran a rendues et posees, toutes ensemble.
#[derive(Debug, Serialize)]
pub(crate) struct EnregistrerLEcran<C> {
    route: Route,
    charge: C,
    #[serde(skip_serializing_if = "Option::is_none")]
    dans: Option<&'static str>,
    libelle: &'static str,
}

impl<C> EnregistrerLEcran<C> {
    /// Les cases vont toutes dans le champ `dans` de la charge (sa route rend l'ecran enregistre).
    pub(crate) fn de(route: Route, charge: C, dans: &'static str, libelle: Affiche) -> EnregistrerLEcran<C> {
        EnregistrerLEcran { route, charge, dans: Some(dans), libelle: libelle.texte() }
    }

    /// Les cases vont chacune sous sa cle dans la charge meme.
    pub(crate) fn a_plat(route: Route, charge: C, libelle: Affiche) -> EnregistrerLEcran<C> {
        EnregistrerLEcran { route, charge, dans: None, libelle: libelle.texte() }
    }
}
