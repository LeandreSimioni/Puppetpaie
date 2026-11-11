//! Le JSON ASCII et compact qu'on scelle ou qu'on envoie a un service : `\uXXXX` hors ASCII, cles triees a la demande,
//! separateurs `, ` et `: ` (ou `,` et `:` serres) ; les nombres a virgule comme les fichiers les ecrivent
//! (`ecriture::a_virgule`). Les octets qu'un coffre deja scelle ou qu'une signature couvre ne changent pas d'une version
//! a l'autre. Le JSON lui-meme s'ecrit dans `ecriture`, le bord (`ecriture::en_ascii`).

/// Le JSON en ASCII (tout autre caractere en `\uXXXX`), cles triees si `trie`. Sans
/// `indent`, les separateurs sont `, ` et `: ` (ou `,` et `:` si `serre`).
pub fn dumps<T: serde::Serialize + ?Sized>(v: &T, indent: Option<usize>, serre: bool, trie: bool) -> String {
    crate::ecriture::en_ascii(v, indent, serre, trie)
}
