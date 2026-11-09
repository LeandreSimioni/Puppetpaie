//! Ce que les commandes ecrivent : l'enveloppe `--json`.

use serde::Serialize;

/// L'ENVELOPPE `--json` d'une commande qui a abouti : son nom et ce qu'elle rend.
#[derive(Serialize)]
struct Enveloppe<'a, T: Serialize> {
    commande: &'a str,
    ok: bool,
    resultat: T,
}

/// Ecrit l'enveloppe de `commande` en JSON indente de deux espaces, suivi d'un saut de ligne. Rend le code de sortie, 0.
pub fn rendre<T: Serialize>(commande: &str, resultat: T) -> i32 {
    dire!("{}", socle::ecriture::json_indente(&Enveloppe { commande, ok: true, resultat }, 2));
    0
}
