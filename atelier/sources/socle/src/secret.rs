//! UN SECRET : un mot de passe, un jeton, une cle. Opaque : il ne s'affiche jamais (`Debug` ecrit « [secret] », ni
//! `Display` ni `Serialize`) ; il se lit d'une charge ou d'une reponse (`Deserialize`) ; il ne se devoile (`devoiler`)
//! qu'aux portes qui l'emploient (le transport, le coffre, la page servie a la fenetre). La ou le logiciel en garde un
//! dans un fichier qu'il ecrit lui-meme (la cle du poste, le jeton scelle), l'ecriture est nommee : `ecrit`.

/// Un secret. Rien ne l'ecrit sans le dire (`devoiler`, `ecrit`).
#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    /// Le secret de ce texte.
    pub fn nouveau(texte: impl Into<String>) -> Secret {
        Secret(texte.into())
    }

    /// Le texte du secret, pour la seule porte qui l'emploie.
    pub fn devoiler(&self) -> &str {
        &self.0
    }

    /// Le meme secret, transforme a la porte qui l'exige ainsi (`f` recoit le texte, rend le nouveau).
    pub fn transforme(&self, f: impl FnOnce(&str) -> String) -> Secret {
        Secret(f(&self.0))
    }

    /// Le secret est-il ce texte ? En temps constant pour une meme longueur : la comparaison ne dit pas, par sa duree,
    /// jusqu'ou les deux concordent.
    pub fn egal_a(&self, autre: &str) -> bool {
        let (a, b) = (self.0.as_bytes(), autre.as_bytes());
        a.len() == b.len() && a.iter().zip(b).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[secret]")
    }
}

impl<'de> serde::Deserialize<'de> for Secret {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Secret, D::Error> {
        String::deserialize(d).map(Secret)
    }
}

/// L'ECRITURE NOMMEE d'un secret que le logiciel garde lui-meme dans un fichier qu'il ecrit :
/// `#[serde(serialize_with = "socle::secret::ecrit")]`. Les memes octets qu'un texte.
pub fn ecrit<S: serde::Serializer>(secret: &Secret, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&secret.0)
}

/// L'ecriture nommee d'un secret facultatif (`None` : `null`).
pub fn ecrit_option<S: serde::Serializer>(secret: &Option<Secret>, s: S) -> Result<S::Ok, S::Error> {
    match secret {
        Some(x) => s.serialize_some(&x.0),
        None => s.serialize_none(),
    }
}

#[cfg(test)]
mod tests {
    use super::Secret;

    #[test]
    fn un_secret_ne_s_affiche_pas() {
        let s = Secret::nouveau("motdepasse");
        assert_eq!(format!("{:?}", s), "[secret]");
        assert_eq!(format!("{:?}", Some(s.clone())), "Some([secret])");
        assert!(s.egal_a("motdepasse") && !s.egal_a("motdepassf") && !s.egal_a("mot"));
    }
}
