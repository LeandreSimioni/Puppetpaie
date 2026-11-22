//! LE NUMERO DE CE PROCESSUS, le seul chemin du logiciel vers `std::process::id` : il distingue les fichiers
//! provisoires de deux processus et dit qui tient un verrou.

/// `std::process::id`.
#[cfg(not(target_os = "wasi"))]
pub fn numero() -> u32 {
    std::process::id()
}

/// Dans le navigateur (WASI), `std::process::id` panique : le logiciel y est seul dans son onglet, et se numerote 1.
#[cfg(target_os = "wasi")]
pub fn numero() -> u32 {
    1
}
