//! La fenetre sans Tauri : la page dans un navigateur de la famille Chromium en mode application
//! (`--app`, une fenetre sans onglets ni barre d'adresse, son propre profil), sinon dans le navigateur par defaut.
//! `PUPPETPAIE_NAVIGATEUR` choisit a la main : une commande (l'adresse est ajoutee a la fin), ou `defaut`.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

/// Ce qui appartient a une installation sur un poste et qu'on ne met pas dans ses donnees (le profil du navigateur de
/// la fenetre, le jeton de sa page, le journal d'avant toute compagnie) : en mode portable, a sa racine.
pub const DOSSIER_PORTABLE: &str = ".fenetre";

/// Le dossier propre a une fenetre sur ce poste : le profil de son navigateur, le jeton de sa page, son journal d'avant
/// toute compagnie. EN PORTABLE (`PORTABLE.json`), dans l'installation (`.fenetre/`) : rien ne reste sur le poste. SINON,
/// dans le dossier de l'utilisateur, un par PORT de page (`fenetres/<port>/`) -- le port est l'origine que le
/// navigateur connait : ce qu'il a retenu pour elle reste avec elle, et deux fenetres ouvertes ensemble (deux ports)
/// n'ont jamais le meme profil.
pub fn dossier_local(inst: &Path, port: u16) -> PathBuf {
    if inst.join(crate::compagnie::coffre::MARQUEUR_PORTABLE).is_file() {
        return inst.join(DOSSIER_PORTABLE);
    }
    dossier_utilisateur().join("fenetres").join(port.to_string())
}

/// Ce qui appartient a l'utilisateur de ce poste (le profil du navigateur) : ni dans les donnees, ni a cote du code. Le
/// dossier des donnees de la personne, et quand la variable ne le dit pas, celui que le systeme lui donne par defaut
/// (`atelier/reference/sources/dossiers-de-configuration/`) :
/// - Windows, `LOCALAPPDATA` : FOLDERID_LocalAppData, « Default Path %LOCALAPPDATA% (%USERPROFILE%\AppData\Local) »
///   (Microsoft, KNOWNFOLDERID) ;
/// - macOS, `~/Library/Application Support/<identifiant du paquet>` : « Contains all app-specific data and support
///   files » ; « all of these items should be put in a subdirectory whose name matches the bundle identifier of the
///   app » (Apple, macOS Library Directory Details) ; l'identifiant est celui de la fenetre (`fenetre/tauri.conf.json`) ;
/// - ailleurs, `XDG_DATA_HOME` : « If $XDG_DATA_HOME is either not set or empty, a default equal to $HOME/.local/share
///   should be used » ; un chemin relatif ne vaut rien (« If an implementation encounters a relative path in any of these
///   variables it should consider the path invalid and ignore it ») (XDG Base Directory Specification 0.8).
pub fn dossier_utilisateur() -> PathBuf {
    use crate::compagnie::coffre::{env_du_poste, maison_de, IDENTIFIANT_DU_PAQUET};
    if cfg!(windows) {
        let base = match env_du_poste("LOCALAPPDATA") {
            Some(b) => PathBuf::from(b),
            None => maison_de(&env_du_poste).join("AppData").join("Local"),
        };
        return base.join("PuppetPaie");
    }
    if cfg!(target_os = "macos") {
        return maison_de(&env_du_poste).join("Library").join("Application Support").join(IDENTIFIANT_DU_PAQUET);
    }
    let base = match env_du_poste("XDG_DATA_HOME") {
        Some(b) if Path::new(&b).is_absolute() => PathBuf::from(b),
        Some(_) | None => maison_de(&env_du_poste).join(".local").join("share"),
    };
    base.join("puppetpaie")
}

fn candidats() -> Vec<PathBuf> {
    if cfg!(windows) {
        let racines: Vec<PathBuf> =
            ["PROGRAMFILES(X86)", "PROGRAMFILES", "LOCALAPPDATA"].iter().filter_map(|v| std::env::var_os(v).map(PathBuf::from)).collect();
        let relatifs = [
            r"Microsoft\Edge\Application\msedge.exe",
            r"Google\Chrome\Application\chrome.exe",
            r"Chromium\Application\chrome.exe",
            r"BraveSoftware\Brave-Browser\Application\brave.exe",
        ];
        relatifs.iter().flat_map(|rel| racines.iter().map(move |r| r.join(rel))).collect()
    } else if cfg!(target_os = "macos") {
        ["Google Chrome", "Microsoft Edge", "Chromium", "Brave Browser"]
            .iter()
            .map(|n| PathBuf::from(format!("/Applications/{n}.app/Contents/MacOS/{n}")))
            .collect()
    } else {
        // Les dossiers du `PATH` ; sans `PATH`, aucun.
        let chemins: Vec<PathBuf> = std::env::var_os("PATH").iter().flat_map(std::env::split_paths).collect();
        ["google-chrome", "google-chrome-stable", "chromium", "chromium-browser", "microsoft-edge", "microsoft-edge-stable", "brave-browser", "vivaldi"]
            .iter()
            .flat_map(|n| chemins.iter().map(move |d| d.join(n)))
            .collect()
    }
}

/// Le premier navigateur de la famille Chromium installe.
pub fn chromium() -> Option<PathBuf> {
    candidats().into_iter().find(|c| c.is_file())
}

fn sans_console(c: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    c.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
}

/// Ouvre la page. Rend le processus du navigateur quand on le tient (un Chromium a nous), sinon `None`. `profil` : le
/// profil du Chromium a nous, un par fenetre (`dossier_local`) : deux installations ouvertes ensemble ne
/// partagent pas le leur (un second Chromium sur le meme profil passe la main au premier et s'arrete aussitot).
pub fn ouvrir(url: &str, profil: &std::path::Path, journal: &dyn Fn(&str)) -> Option<Child> {
    // Le choix fait a la main : une commande, ou `defaut` ; rien de pose, le premier Chromium.
    let choix: Option<String> = match std::env::var("PUPPETPAIE_NAVIGATEUR") {
        Ok(v) if !v.trim().is_empty() => Some(v.trim().to_string()),
        Ok(_) | Err(std::env::VarError::NotPresent) => None,
        Err(std::env::VarError::NotUnicode(v)) => {
            journal(&format!("PUPPETPAIE_NAVIGATEUR ne se lit pas ({:?}) : le premier navigateur Chromium.\n", v));
            None
        }
    };
    if !choix.as_deref().is_some_and(|c| c.eq_ignore_ascii_case("defaut")) {
        let commande: Option<(PathBuf, Vec<String>)> = if let Some(choix) = &choix {
            let mut morceaux = choix.split_whitespace().map(String::from);
            morceaux.next().map(|exe| (PathBuf::from(exe), morceaux.chain([url.to_string()]).collect()))
        } else {
            chromium().map(|exe| {
                let mut a = vec![format!("--app={}", url), "--no-first-run".into(), "--no-default-browser-check".into(), "--window-size=940,640".into()];
                // Un Chromium en snap n'ecrit pas hors de son bac : il garde son profil.
                // Le programme lui-meme, au bout de ses liens ; un lien qui ne se resout pas se juge sur son propre chemin.
                let reel = match std::fs::canonicalize(&exe) {
                    Ok(r) => r,
                    Err(e) => {
                        journal(&format!("{} ne se resout pas ({}).\n", exe.display(), e));
                        exe.clone()
                    }
                };
                if !reel.to_string_lossy().contains("/snap/") {
                    let _ = std::fs::create_dir_all(profil);
                    a.insert(0, format!("--user-data-dir={}", profil.display()));
                }
                (exe, a)
            })
        };
        if let Some((exe, args)) = commande {
            match sans_console(Command::new(&exe).args(&args)).spawn() {
                Ok(p) => {
                    journal(&format!("Fenetre : {}\n", exe.display()));
                    return Some(p);
                }
                Err(e) => journal(&format!("Navigateur {} injoignable ({}) ; navigateur par defaut.\n", exe.display(), e)),
            }
        }
    }
    journal("Fenetre : le navigateur par defaut.\n");
    if let Err(e) = crate::compagnie::poste::ouvrir_adresse(url) {
        journal(&format!("Le navigateur par defaut ne s'ouvre pas : {}\n", socle::echec::phrase_de(&e)));
    }
    None
}
