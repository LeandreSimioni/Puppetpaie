//! Le socle du moteur : ce que toute partie du dessus lit sans rien savoir de la paie. Les dates et l'heure de Paris, le
//! temps (celui de la machine, ou le temps simule de l'atelier), les nombres et leur arrondi, le texte, les fichiers lus
//! et ecrits, l'echec et sa memoire, le garde-fou reseau. Aucun droit, aucun metier.

pub mod base64;
pub mod codification_ue;
pub mod csv;
pub mod d28;
pub mod date;
pub mod echec;
pub mod ecran;
pub mod ecriture;
pub mod feries;
pub mod fichiers;
pub mod fil_de_fond;
pub mod forme;
pub mod garde_reseau;
pub mod html;
pub mod iban;
pub mod json_ascii;
pub mod lecture_xlsx;
pub mod ligne;
pub mod memoire;
pub mod mois;
pub mod nfkc;
pub mod num;
pub mod paris;
pub mod pays;
pub mod piece;
pub mod secret;
pub mod nfkd_ascii;
pub mod tables_nfkc;
pub mod temps;
pub mod texte;
pub mod verrou;

/// UN REFUS ORDINAIRE DONT LA PHRASE SE COMPOSE (`format!`) : `echec::refus`, son endroit note la ou il s'ecrit.
#[macro_export]
macro_rules! refus {
    ($($arg:tt)*) => {
        $crate::echec::refus(format!($($arg)*))
    };
}

/// Ecrit une ligne sur la sortie standard (`au_terminal`).
#[macro_export]
macro_rules! dire {
    () => {
        $crate::au_terminal(format_args!("\n"))
    };
    ($($arg:tt)*) => {
        $crate::au_terminal(format_args!("{}\n", format_args!($($arg)*)))
    };
}

/// Ecrit sans saut de ligne, comme `dire!`.
#[macro_export]
macro_rules! dire_sans_ligne {
    ($($arg:tt)*) => {
        $crate::au_terminal(format_args!($($arg)*))
    };
}

/// Ecrit sur la sortie standard. Un tuyau ferme (`| head`) n'est pas une erreur du programme : son lecteur a fini. Une
/// autre panne du terminal n'a personne a qui se dire : elle est notee (`echec::noter_echec`, que les incidents lisent).
#[doc(hidden)]
pub fn au_terminal(texte: std::fmt::Arguments<'_>) {
    use std::io::Write;
    match std::io::stdout().write_fmt(texte) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {}
        Err(e) => echec::noter_echec(Some(echec::Classe::du_disque(&e)), &format!("la sortie standard refuse d'ecrire : {}", e)),
    }
}
