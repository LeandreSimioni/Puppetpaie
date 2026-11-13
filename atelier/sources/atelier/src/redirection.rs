//! LA REDIRECTION DU BINAIRE DE L'ATELIER (`moteur-atelier`, jamais livre), qui allume la feature `atelier` de ce qu'il
//! prend. Le `moteur` livre ne la compile pas : il garde les adresses des organismes en dur et rien ne permet de les changer.
//!
//! - Trois variables d'environnement pointent le client de la DSN (net-entreprises), celui de la DPAE (Urssaf) et celui
//!   de Google (jetons et Drive) vers un faux serveur LOCAL. Une adresse qui n'est pas locale (`http://127.0.0.1:`,
//!   `http://localhost:` ou `http://[::1]:`) est refusee : jamais une redirection vers l'exterieur.
//! - Tout acte reel est refuse : un depot reel, un envoi, une signature passent tous par `compagnie::preuve::verifier_preuve` ;
//!   dans l'atelier elle refuse, mot de passe ou non (`envois::redirection::REFUS_ACTE_REEL`).
//!
//! Les adresses de la boite courriel (SMTP et IMAP, en TLS) et du serveur des incidents ne sont pas redirigees ici : celle
//! du serveur des incidents vient deja de la configuration de la compagnie.

use envois::redirection::{est_locale, VARIABLE_DPAE, VARIABLE_DSN, VARIABLE_GOOGLE};

/// Au demarrage de `moteur-atelier` : une variable posee sur autre chose qu'un serveur local arrete tout.
pub fn verifier_environnement() -> socle::echec::R<()> {
    for variable in [VARIABLE_DSN, VARIABLE_DPAE, VARIABLE_GOOGLE] {
        let v = match std::env::var(variable) {
            Ok(v) => v,
            Err(std::env::VarError::NotPresent) => continue,
            Err(std::env::VarError::NotUnicode(_)) => return Err(socle::refus!("{} ne se lit pas : ce n'est pas du texte. Rien n'est parti.", variable)),
        };
        let v = v.trim();
        let locale = match socle::garde_reseau::Url::lire(v) {
            Ok(u) => est_locale(&u),
            Err(_) => false,
        };
        if !v.is_empty() && !locale {
            return Err(socle::refus!("{} : l'atelier ne se redirige que vers un serveur local (http://127.0.0.1:PORT/...). Rien n'est parti.", variable));
        }
    }
    Ok(())
}
