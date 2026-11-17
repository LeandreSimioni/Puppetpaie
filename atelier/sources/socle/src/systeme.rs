//! LES APPELS AU NOYAU DE WINDOWS : la memoire du poste, le disque libre, l'heritage d'une prise d'ecoute, l'attente de
//! la fin d'un processus. C'est le seul endroit ou un entier signe est impose par l'exterieur (le `BOOL` de l'API, un
//! `i32`) et le seul `unsafe` du logiciel : chaque appel rend une valeur typee (des octets, un booleen), et le reste du
//! logiciel ne voit ni l'API ni ses entiers. L'espace interdit `unsafe_code` (`deny`) : ce module seul le leve.
#![allow(unsafe_code)]

use std::path::Path;
use std::time::Duration;

/// La memoire physique du poste, en octets : (totale, disponible) ; la raison de l'API sinon.
pub fn memoire_physique() -> std::io::Result<(u64, u64)> {
    #[repr(C)]
    struct EtatMemoire {
        longueur: u32,
        charge: u32,
        physique_totale: u64,
        physique_libre: u64,
        pagination_totale: u64,
        pagination_libre: u64,
        virtuelle_totale: u64,
        virtuelle_libre: u64,
        etendue_libre: u64,
    }
    extern "system" {
        fn GlobalMemoryStatusEx(etat: *mut EtatMemoire) -> i32;
    }
    let mut e = EtatMemoire {
        longueur: std::mem::size_of::<EtatMemoire>() as u32,
        charge: 0,
        physique_totale: 0,
        physique_libre: 0,
        pagination_totale: 0,
        pagination_libre: 0,
        virtuelle_totale: 0,
        virtuelle_libre: 0,
        etendue_libre: 0,
    };
    // SAFETY : la structure est celle de MEMORYSTATUSEX, sa longueur posee comme l'appel l'exige.
    match unsafe { GlobalMemoryStatusEx(&mut e) } != 0 {
        true => Ok((e.physique_totale, e.physique_libre)),
        false => Err(std::io::Error::last_os_error()),
    }
}

/// Le disque libre la ou vit `p`, en octets, pour l'utilisateur qui appelle ; la raison de l'API sinon.
pub fn disque_libre(p: &Path) -> std::io::Result<u64> {
    use std::os::windows::ffi::OsStrExt;
    extern "system" {
        fn GetDiskFreeSpaceExW(dossier: *const u16, libre_appelant: *mut u64, total: *mut u64, libre_total: *mut u64) -> i32;
    }
    let large: Vec<u16> = p.as_os_str().encode_wide().chain(std::iter::once(0)).collect();
    let (mut libre, mut total, mut libre_total) = (0u64, 0u64, 0u64);
    // SAFETY : un chemin termine par zero, trois sorties vivantes.
    match unsafe { GetDiskFreeSpaceExW(large.as_ptr(), &mut libre, &mut total, &mut libre_total) } != 0 {
        true => Ok(libre),
        false => Err(std::io::Error::last_os_error()),
    }
}

/// La prise `l` ne passe plus a un processus enfant : son drapeau d'heritage tombe.
pub fn sans_heritage(l: &std::net::TcpListener) {
    use std::os::windows::io::AsRawSocket;
    extern "system" {
        fn SetHandleInformation(objet: isize, masque: u32, drapeaux: u32) -> i32;
    }
    const HANDLE_FLAG_INHERIT: u32 = 1;
    // SAFETY : la prise est vivante (empruntee) ; on ne change que son drapeau d'heritage.
    unsafe {
        SetHandleInformation(l.as_raw_socket() as isize, HANDLE_FLAG_INHERIT, 0);
    }
}

/// Attend la fin du processus `pid`, `delai` au plus. Vrai s'il est parti, ou si le noyau ne le connait pas (deja parti,
/// ou jamais vu : rien a attendre).
pub fn attendre_la_fin(pid: u32, delai: Duration) -> bool {
    extern "system" {
        fn OpenProcess(acces: u32, heriter: i32, pid: u32) -> isize;
        fn WaitForSingleObject(objet: isize, millisecondes: u32) -> u32;
        fn CloseHandle(objet: isize) -> i32;
    }
    const SYNCHRONIZE: u32 = 0x0010_0000;
    // Au plus 49 jours : `u32::MAX` millisecondes serait l'attente sans fin de l'API (INFINITE).
    let millisecondes = delai.as_millis().min(u128::from(u32::MAX - 1)) as u32;
    // SAFETY : appels du noyau sur un numero de processus ; la poignee ouverte est refermee.
    unsafe {
        let h = OpenProcess(SYNCHRONIZE, 0, pid);
        if h == 0 {
            return true;
        }
        let r = WaitForSingleObject(h, millisecondes);
        CloseHandle(h);
        r == 0
    }
}
