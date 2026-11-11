//! PuppetPaie : la fenetre. Le serveur de la fenetre (`moteur::interface`) tourne dans ce processus, et sa page
//! s'affiche dans une fenetre Tauri (WebView2 sous Windows, WebKit sous Mac et Linux). Les routes du moteur tournent
//! dans ce processus (`moteur::interface::api::route`). `puppetpaie route ...` et `puppetpaie routes ...` sont `moteur route` et
//! `moteur routes` : la ligne de commande d'une installation Windows, et la mise a jour y eprouve le nouveau programme.
//!
//! Sans Tauri (un poste sans moteur de rendu), `moteur interface` ouvre la meme page dans le navigateur.

#![cfg_attr(windows, windows_subsystem = "windows")]

use std::sync::{Arc, Mutex, OnceLock};

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

static APPLICATION: OnceLock<tauri::AppHandle> = OnceLock::new();

/// La barre de titre que la page n'a pas : pose dans chaque page de la fenetre, avant ses scripts.
/// Ce qu'il peut demander a la fenetre est borne par `capabilities/fenetre.json`.
const CADRE: &str = include_str!("cadre.js");

// Les tailles de la fenetre, en points entiers.
const LARGEUR_MIN: u32 = 700;
const HAUTEUR_MIN: u32 = 500;
/// La taille d'ouverture qu'on vise : de quoi voir un mois et ses tableaux sans prendre tout l'ecran.
const LARGEUR_VISEE: u32 = 1360;
const HAUTEUR_VISEE: u32 = 900;

/// La taille d'ouverture, en points : celle qu'on vise, sans depasser neuf dixiemes de la zone de travail de l'ecran
/// (l'ecran moins la barre des taches, ramenee en points par l'echelle de l'ecran), ni descendre sous le minimum. Sans
/// ecran connu, celle qu'on vise.
fn taille_d_ouverture(ecran: Option<tauri::Monitor>) -> (u32, u32) {
    let Some(m) = ecran else { return (LARGEUR_VISEE, HAUTEUR_VISEE) };
    let zone = m.work_area().size.to_logical::<u32>(m.scale_factor());
    let borne = |vise: u32, dispo: u32, min: u32| vise.min(dispo * 9 / 10).max(min);
    (borne(LARGEUR_VISEE, zone.width, LARGEUR_MIN), borne(HAUTEUR_VISEE, zone.height, HAUTEUR_MIN))
}

/// La fenetre revient devant (un second lancement).
fn devant() {
    if let Some(w) = APPLICATION.get().and_then(|a| a.get_webview_window("principale")) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Les sous-commandes du moteur : `route` et `routes` (la ligne de commande, sous Windows la seule), et la fabrique.
    match args.first().map(String::as_str) {
        Some("route") => match routes::service::commande("route", &args[1..]) {
            Some(code) => std::process::exit(code),
            None => std::process::exit(2),
        },
        Some("routes") => std::process::exit(routes::catalogue::commande(&args[1..])),
        Some("pdf") => return fabrique_pdf::ligne_de_commande::executer(&args[1..]),
        Some("interface") => std::process::exit(moteur::interface::commande(&args[1..])),
        // Le lanceur Linux demande si la fenetre peut s'ouvrir : le programme se charge (WebKitGTK est la), et un ecran.
        Some("--peut-ouvrir") => {
            let ecran = |n: &str| std::env::var_os(n).is_some_and(|v| !v.is_empty());
            let ok = cfg!(any(windows, target_os = "macos")) || ecran("DISPLAY") || ecran("WAYLAND_DISPLAY");
            return if ok { std::process::ExitCode::SUCCESS } else { std::process::ExitCode::FAILURE };
        }
        // Le relais de la mise a jour (`compagnie::maj`) : lance depuis le `maj/` de l'installation, il bascule puis rouvre la fenetre.
        Some("maj-relais") => std::process::exit(compagnie::maj::relais(&args[1..])),
        // `puppetpaie maj` : chercher et preparer une version tout de suite (le bac a sable de l'atelier s'en sert).
        Some("maj") => std::process::exit(moteur::commandes::maj::commande(&args[1..])),
        _ => {}
    }

    if let Err(e) = compagnie::installation::fixer_le_code() {
        eprintln!("La fenetre ne demarre pas : {}", e.en_lignes());
        std::process::exit(1);
    }
    // UNE FENETRE PAR INSTALLATION (`compagnie::instance`). Celle de cette installation est deja
    // ouverte : elle revient devant. Une AUTRE installation ouverte sur le poste ne change rien : chacune a son creneau
    // (son verrou et le port de sa page), et elles s'ouvrent cote a cote.
    let prise = match moteur::interface::prendre_instance(devant) {
        Ok(compagnie::instance::Instance::Tenue(p)) => p,
        Ok(compagnie::instance::Instance::Devant) => return std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("La fenetre ne demarre pas : {}.", e);
            std::process::exit(1);
        }
    };
    // Une livraison de fictives prete se pose (`compagnie::maj::poser_les_fictives_pretes`), et le reste d'une
    // compagnie supprimee s'efface (`suppression::effacer_les_restes`).
    moteur::interface::preparer_l_installation();
    // Une version prete s'applique ICI, avant toute fenetre : le relais attend notre fin, bascule et ouvre la neuve.
    if compagnie::maj::au_lancement() {
        return std::process::ExitCode::SUCCESS;
    }
    // Sinon le logiciel cherche, en fond, une version plus recente pour l'installation : elle s'appliquera au prochain
    // lancement.
    compagnie::maj::chercher_en_fond();
    let (fenetre, port) = match moteur::interface::demarrer(moteur::interface::MINUTES_VEILLE, &prise) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("La fenetre ne demarre pas : {}.", e);
            std::process::exit(1);
        }
    };
    // Les compagnies de l'installation se tiennent a jour en fond des maintenant, sans rien ouvrir (calcul local seulement).
    fenetre.prechauffer();
    // LE PROFIL DU NAVIGATEUR DE LA FENETRE, a elle (WebView2 sous Windows, WebKit ailleurs) : un par fenetre
    // (`navigateur::dossier_local`), jamais celui d'une autre installation ouverte a cote -- ses cookies, ce qu'elle a
    // retenu, et son processus de rendu.
    let profil = fenetre.dossier_local().join("webview");
    let adresse = fenetre.adresse("127.0.0.1", port);
    let pour_fermer: Arc<Mutex<Option<tauri::AppHandle>>> = Arc::new(Mutex::new(None));
    let poignee = pour_fermer.clone();
    *fenetre.fermer.lock().unwrap_or_else(|e| e.into_inner()) = Some(Box::new(move || {
        if let Some(a) = poignee.lock().ok().and_then(|a| a.clone()) {
            a.exit(0);
        }
    }));

    let resultat = tauri::Builder::default()
        .setup(move |app| {
            let _ = APPLICATION.set(app.handle().clone());
            *pour_fermer.lock().unwrap_or_else(|e| e.into_inner()) = Some(app.handle().clone());
            let url = adresse.parse().map_err(|e| format!("adresse illisible : {}", e))?;
            // UNE FENETRE SANS BARRE NOIRE, OUVERTE UNE FOIS. Pas de bordure systeme : le haut de la
            // fenetre est le bandeau de la page, et `cadre.js` en fait la barre de titre (tirer, double-clic, les trois
            // boutons). Elle s'ouvre a sa taille, centree, et n'en change plus : l'ancienne s'ouvrait en 1200 x 800
            // puis s'agrandissait aussitot, « ca a l'air de le faire et apres ca le fait plus ».
            let (largeur, hauteur) = taille_d_ouverture(app.primary_monitor().ok().flatten());
            WebviewWindowBuilder::new(app, "principale", WebviewUrl::External(url))
                .title("PuppetPaie")
                .decorations(false)
                .inner_size(largeur.into(), hauteur.into())
                .min_inner_size(LARGEUR_MIN.into(), HAUTEUR_MIN.into())
                .center()
                .initialization_script(CADRE)
                .data_directory(profil)
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!());
    if let Err(e) = resultat {
        fenetre.ecrire(&format!("La fenetre Tauri ne s'ouvre pas ({}) ; le navigateur prend le relais.\n", e));
        let _ = moteur::interface::navigateur::ouvrir(&fenetre.adresse("puppetpaie.localhost", port), &fenetre.dossier_local().join("navigateur"), &|t| fenetre.ecrire(t));
        loop {
            std::thread::sleep(std::time::Duration::from_secs(5));
            if fenetre.silence() > std::time::Duration::from_secs(300) {
                return std::process::ExitCode::SUCCESS;
            }
        }
    }
    std::process::ExitCode::SUCCESS
}
