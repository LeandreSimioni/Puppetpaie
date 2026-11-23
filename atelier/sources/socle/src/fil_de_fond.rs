//! LE FIL DE FOND : le fil de la fenetre qui tient les etats des compagnies a jour des le lancement, AVANT
//! tout mot de passe (`etat_garde::Fil`) ; dans le navigateur, le temps d'un de ses travaux fait par qui l'attend
//! (`le_temps_d_un_travail`). Il ne fait que du calcul local : lire les fichiers de la compagnie (en clair sur
//! le disque) et ecrire ses etats gardes. Ce module le marque, et deux gardes le tiennent, quoi qu'une lecture demande :
//! - `garde_reseau::sortie_vers` refuse toute sortie hors de la machine depuis ce fil ;
//! - le coffre (`coffre.rs`) refuse d'y servir la cle du poste ou de deriver un mot de passe : rien ne s'y dechiffre.
//! Ce qui est en clair dans un coffre (ce qui est pose, depuis quand, s'il a un mot de passe) se lit comme ailleurs.

use std::cell::Cell;

thread_local! {
    static DE_FOND: Cell<bool> = const { Cell::new(false) };
}

/// Ce fil est le fil de fond (pour toute sa vie).
pub fn marquer() {
    DE_FOND.with(|d| d.set(true));
}

/// LE TEMPS D'UN TRAVAIL DU FIL DE FOND FAIT SUR UN AUTRE FIL (dans le navigateur, sans second fil, celui qui attend un
/// etat fait le travail de la file : `etat_garde::Fil::pomper`) : ce fil est le fil de fond tant que le garde vit, avec
/// ses deux refus, puis redevient ce qu'il etait.
#[must_use = "le fil n'est le fil de fond que tant que son garde vit"]
pub struct LeTempsDUnTravail(bool);

pub fn le_temps_d_un_travail() -> LeTempsDUnTravail {
    LeTempsDUnTravail(DE_FOND.with(|d| d.replace(true)))
}

impl Drop for LeTempsDUnTravail {
    fn drop(&mut self) {
        DE_FOND.with(|d| d.set(self.0));
    }
}

/// Sommes-nous sur le fil de fond ?
pub fn ici() -> bool {
    DE_FOND.with(|d| d.get())
}

/// Le refus, en une phrase.
pub fn refus(quoi: &str) -> String {
    format!("Le calcul en fond de la fenetre ne fait que lire et calculer : {} lui est refuse.", quoi)
}
