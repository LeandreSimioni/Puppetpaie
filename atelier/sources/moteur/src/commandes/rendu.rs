//! Ce que les commandes ecrivent : l'enveloppe `--json`, et le texte d'une valeur dans une ligne en colonnes.

use serde::Serialize;
use serde_json::Value;

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

/// Le texte d'une valeur JSON : `None` pour null, `True`, `0.5`.
pub fn texte(v: &Value) -> String {
    crate::valeur::valeur_en_texte(v)
}

/// Le texte d'une cle d'un objet JSON, `None` quand elle manque.
pub fn champ(v: &Value, cle: &str) -> String {
    v.get(cle).map(texte).unwrap_or_else(|| "None".into())
}
