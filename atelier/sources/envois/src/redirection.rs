//! LA REDIRECTION DE L'ATELIER : dans la construction de la feature `atelier` (le binaire `moteur-atelier`, jamais livre),
//! trois variables d'environnement pointent le client de la DSN (net-entreprises), celui de la DPAE (Urssaf) et celui
//! de Google (jetons et Drive) vers un faux serveur LOCAL. Une adresse qui n'est pas locale (`http://127.0.0.1:`,
//! `http://localhost:` ou `http://[::1]:`) est refusee : jamais une redirection vers l'exterieur. La construction
//! ordinaire ne lit jamais ces variables : les adresses des organismes y sont en dur.

use socle::garde_reseau::Url;

/// Le faux net-entreprises :`BASE/authentifier/`, `BASE/lister-retours-declarant/1.0/`, `BASE/lister-retours-flux/1.0/`, `BASE/deposer-dsn/1.0/`.
pub const VARIABLE_DSN: &str = "PUPPETPAIE_ATELIER_DSN";
/// Le faux Urssaf : `BASE/authentifier_dpae`, et les mêmes services que la DSN.
pub const VARIABLE_DPAE: &str = "PUPPETPAIE_ATELIER_DPAE";
/// Le faux Google : `BASE/token`, `BASE/drive/v3/files`, `BASE/upload/drive/v3/files`.
pub const VARIABLE_GOOGLE: &str = "PUPPETPAIE_ATELIER_GOOGLE";

/// La phrase du refus d'un acte reel dans le binaire de l'atelier : un depot reel, un envoi, une signature passent tous
/// par `depot_acte::verifier_preuve`, qui y refuse, mot de passe ou non.
pub const REFUS_ACTE_REEL: &str =
    "Ceci est le binaire de l'atelier (moteur-atelier) : il refuse tout acte reel (depot reel, envoi, signature), mot de passe ou non. Rien n'est parti.";

/// Ce que dit une redirection refusee : l'adresse qui n'est pas locale n'est jamais appelee (sans barre finale, comme une
/// base).
pub const ADRESSE_REFUSEE: Url = Url::du_moteur("http://127.0.0.1:9/adresse-non-locale-refusee-par-l-atelier");

/// Une adresse locale : le seul endroit ou l'atelier accepte d'aller.
pub fn est_locale(adresse: &Url) -> bool {
    ["http://127.0.0.1:", "http://localhost:", "http://[::1]:"].iter().any(|p| adresse.texte().starts_with(p))
}

/// La base d'une variable de redirection, sans barre finale ; `None` si elle n'est pas posee (ou vide). Une valeur qui
/// n'est pas une adresse locale (ou pas en Unicode) rend `ADRESSE_REFUSEE`, qui n'aboutit nulle part.
pub fn base(variable: &str) -> Option<Url> {
    let v = match std::env::var(variable) {
        Ok(v) => v,
        Err(std::env::VarError::NotPresent) => return None,
        Err(std::env::VarError::NotUnicode(_)) => return Some(ADRESSE_REFUSEE),
    };
    let v = v.trim().trim_end_matches('/');
    if v.is_empty() {
        return None;
    }
    Some(match Url::lire(v) {
        Ok(u) if est_locale(&u) => u,
        Ok(_) | Err(_) => ADRESSE_REFUSEE,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seules_les_adresses_locales_passent() {
        let locale = |t: &str| est_locale(&Url::lire(t).unwrap());
        assert!(locale("http://127.0.0.1:8080/dsn"));
        assert!(locale("http://localhost:1/"));
        assert!(!locale("https://services.net-entreprises.fr/"));
        assert!(!locale("http://127.0.0.1.exemple.fr:80/"));
        assert!(!locale("http://127.0.0.1/"));
    }
}
