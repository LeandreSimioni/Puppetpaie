//! LA PORTE DE L'ONGLET : le moteur compile en WebAssembly (`wasm32-wasip1`), tel que l'hote JavaScript d'un onglet du
//! navigateur l'appelle (la demo de puppetpaie.fr). Quatre exports, qui ne font que passer des octets entre la memoire du
//! module et la logique de l'onglet (`moteur::interface::onglet`, ou tout se decide) :
//!
//! - `onglet_entree(n)` : l'hote va ecrire une demande de `n` octets ; rend l'adresse ou l'ecrire ;
//! - `onglet_demarrer()` : la fenetre nait (`onglet::demarrage`) ; rend la longueur de la reponse ;
//! - `onglet_traiter()` : la demande ecrite est servie (`onglet::traiter`) ; rend la longueur de la reponse ;
//! - `onglet_sortie()` : l'adresse de la derniere reponse, que l'hote copie.
//!
//! Les tampons sont gardes ici ; Rust ne dereference aucun pointeur : il rend l'adresse d'un `Vec` qu'il tient, l'hote
//! lit et ecrit la memoire du module de son cote.
//!
//! L'ESPACE INTERDIT `unsafe_code` ; cette crate le leve, ici, une fois, pour le seul attribut d'export (`no_mangle`,
//! que le lint compte comme `unsafe` : le nom du symbole est celui que l'hote appelle). Aucun bloc `unsafe`.
#![allow(unsafe_code)]

use std::sync::{Mutex, MutexGuard};

/// La demande que l'hote ecrit (`onglet_entree`), lue par `onglet_traiter`.
static ENTREE: Mutex<Vec<u8>> = Mutex::new(Vec::new());
/// La derniere reponse, que l'hote copie (`onglet_sortie`).
static SORTIE: Mutex<Vec<u8>> = Mutex::new(Vec::new());

/// Le tampon, tenu. Un seul fil dans l'onglet, et une panique y abandonne le module : un verrou tombe n'arrive pas, et
/// le tampon se lit tel quel s'il arrivait.
fn tenu(m: &Mutex<Vec<u8>>) -> MutexGuard<'_, Vec<u8>> {
    match m.lock() {
        Ok(g) => g,
        Err(tombe) => tombe.into_inner(),
    }
}

/// La reponse rangee pour l'hote ; sa longueur.
fn ranger(octets: Vec<u8>) -> usize {
    let n = octets.len();
    *tenu(&SORTIE) = octets;
    n
}

/// L'hote va ecrire une demande de `n` octets : l'adresse ou les ecrire.
#[no_mangle]
pub extern "C" fn onglet_entree(n: usize) -> *mut u8 {
    let mut e = tenu(&ENTREE);
    e.clear();
    e.resize(n, 0);
    e.as_mut_ptr()
}

/// La fenetre de l'onglet nait : la longueur de la reponse (`onglet_sortie`).
#[no_mangle]
pub extern "C" fn onglet_demarrer() -> usize {
    ranger(moteur::interface::onglet::demarrage())
}

/// La demande ecrite (`onglet_entree`) est servie : la longueur de la reponse (`onglet_sortie`).
#[no_mangle]
pub extern "C" fn onglet_traiter() -> usize {
    let demande = std::mem::take(&mut *tenu(&ENTREE));
    ranger(moteur::interface::onglet::traiter(&demande))
}

/// L'adresse de la derniere reponse.
#[no_mangle]
pub extern "C" fn onglet_sortie() -> *const u8 {
    tenu(&SORTIE).as_ptr()
}
