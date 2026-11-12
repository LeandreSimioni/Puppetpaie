//! CE QUE LE PROCESSUS GARDE EN MEMOIRE (un cache, l'etat du temps simule, les echecs) sous un `Mutex`.
//!
//! Un `Mutex` empoisonne n'existe qu'apres une panique (un defaut du logiciel) d'un fil qui le tenait, au milieu d'un
//! changement : ce qu'il garde n'est plus sur. Le reprendre tel quel cacherait le defaut sous une valeur peut-etre a
//! moitie ecrite ; le rendre en erreur ferait porter a chaque lecteur un etat que seul un defaut produit. Il panique
//! donc a son tour : une route le rend en refus (`routes::lib`, `catch_unwind`), le defaut se voit et se corrige a sa
//! cause.

use std::sync::{LockResult, Mutex, MutexGuard};

/// Le `Mutex` tenu ; empoisonne, la panique du fil qui l'a empoisonne se dit a nouveau, ici (`repris`).
pub fn tenu<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    repris(m.lock())
}

/// Le `Mutex` repris, par sa prise ou au reveil d'une attente (`Condvar::wait`, `wait_timeout`) ; empoisonne, la
/// panique du fil qui l'a empoisonne se dit a nouveau, ici : la seule reprise d'un verrou.
pub fn repris<T>(r: LockResult<T>) -> T {
    match r {
        Ok(g) => g,
        Err(_) => panic!("Défaut du logiciel : une donnée tenue en mémoire est restée à moitié écrite par une panique précédente."),
    }
}
