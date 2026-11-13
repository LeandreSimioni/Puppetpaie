//! LES OS, GARDES QUOI QU'ON CHANGE DESSUS : chaque route dit en une phrase breve ce qu'elle fait, et l'exemple de sa
//! fiche est accepte ; une crate ne prend qu'a celles du dessous, et seul le socle prend `serde_json` ; les donnees
//! livrees et les formes qui les lisent disent la meme chose. Trois tests courts lisent les sources des crates, sans
//! liste : aucun flottant, toute forme lue refuse une cle inconnue, aucune valeur par defaut si absent.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use routes::catalogue::{catalogue, Route};
use serde_json::{json, Value};

use crate::bulletins_fictifs::{a_heures, a_repetitions, page_a, pdf};
use crate::outils::rejeu::{rejouer_dossier, scenario};
use crate::outils::{dossier_de_test, route, route_avec};
use routes::accueil::ajouter_venant;
use compagnie::installation::Venue;
use routes::gestes::administrateur::Poste;
use compagnie::demande::VARIABLE_ESSAIS;

/// La longueur d'un geste, en caracteres : une phrase breve.
const GESTE_MAX: usize = 80;

pub(crate) fn espace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

/// La liste `cle` (`members`, `default-members`) du `Cargo.toml` de l'espace, dans son ordre.
fn liste_de_l_espace(cle: &str) -> Vec<String> {
    let espace_toml = std::fs::read_to_string(espace().join("Cargo.toml")).unwrap();
    let l = espace_toml.lines().find(|l| l.starts_with(&format!("{} = [", cle))).unwrap_or_else(|| panic!("Cargo.toml de l'espace : pas de `{}`", cle));
    l.split('"').skip(1).step_by(2).map(str::to_string).collect()
}

/// LES CRATES DE L'ESPACE, de bas en haut, telles que son `Cargo.toml` les declare (`members`) : le logiciel, puis
/// l'atelier, au-dessus de tout, que rien ne prend. Chacune ne prend qu'a celles d'avant elle
/// (`une_crate_ne_prend_qu_a_celles_du_dessous`).
fn membres() -> Vec<String> {
    liste_de_l_espace("members")
}

/// LES CRATES DU LOGICIEL, de bas en haut : ce que le `moteur` livre porte, tel que le `Cargo.toml` de l'espace le dit
/// (`default-members`).
fn le_logiciel() -> Vec<String> {
    liste_de_l_espace("default-members")
}

/// Les fichiers `.rs` sous `d`.
fn sources(d: &Path) -> Vec<PathBuf> {
    let mut v = Vec::new();
    for e in std::fs::read_dir(d).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            v.extend(sources(&p));
        } else if p.extension().is_some_and(|x| x == "rs") {
            v.push(p);
        }
    }
    v
}

/// Le module d'un source de l'espace, depuis son chemin relatif (`crate/src/a/b.rs` ou `crate/src/a/b/mod.rs` :
/// `crate::a::b`), tel que le compilateur le nomme.
pub(crate) fn module_de(rel: &str) -> String {
    rel.trim_end_matches(".rs").trim_end_matches("/mod").replacen("/src/", "::", 1).replace('/', "::")
}

/// UNE EXPLICATION S'ECRIT SOUS `quoi`, `note` OU `source`, et sous aucune cle en `_` : aucun fichier JSON livre
/// (`logiciel/donnees/`, a tous les niveaux) n'en porte une.
#[test]
fn aucun_fichier_livre_ne_porte_une_cle_en_souligne() {
    fn fichiers(d: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                fichiers(&p, out);
            } else if p.extension().is_some_and(|x| x == "json") {
                out.push(p);
            }
        }
    }
    fn cles(v: &Value, ou: &str, fautes: &mut Vec<String>) {
        match v {
            Value::Object(o) => {
                for (k, x) in o {
                    if k.starts_with('_') {
                        fautes.push(format!("{} : la cle « {} »", ou, k));
                    }
                    cles(x, &format!("{}/{}", ou, k), fautes);
                }
            }
            Value::Array(a) => a.iter().enumerate().for_each(|(i, x)| cles(x, &format!("{}/{}", ou, i), fautes)),
            _ => {}
        }
    }
    let racine = crate::outils::code_livre();
    let mut tous = Vec::new();
    fichiers(&racine, &mut tous);
    assert!(tous.len() > 30, "{} fichiers JSON lus", tous.len());
    let mut fautes = Vec::new();
    for p in tous {
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap_or_else(|e| panic!("{} : {}", p.display(), e));
        cles(&v, &p.strip_prefix(&racine).unwrap().to_string_lossy(), &mut fautes);
    }
    assert!(fautes.is_empty(), "des cles en `_` dans les fichiers livres : une explication s'ecrit `quoi`, `note` ou `source` ; une donnee, sous un nom sans `_` :\n{}", fautes.join("\n"));
}

/// Chaque route du catalogue a sa fiche, une seule, et chaque fiche est une route de l'enumeration `Route` qui se dit au
/// catalogue : une route de l'atelier (`au_catalogue` faux) n'en a pas. Le nom d'une route se relit en elle (`Route::lire`).
#[test]
fn chaque_route_dit_brievement_ce_qu_elle_fait() {
    let tout = catalogue();
    let mut vus = BTreeSet::new();
    for f in &tout {
        assert!(vus.insert(f.nom()), "{} est deux fois au catalogue", f.nom());
        assert!(f.route.au_catalogue() && Route::lire(f.nom()) == Some(f.route), "{} : sa fiche n'est pas une route du catalogue", f.nom());
        assert!(!f.geste.is_empty() && f.geste.chars().count() <= GESTE_MAX, "{} : le geste, en {} caracteres au plus : {}", f.nom(), GESTE_MAX, f.geste);
        assert!(!f.rend.is_empty() && !f.refus.is_empty(), "{} : sa fiche dit ce qu'elle rend et ses refus", f.nom());
        assert!(serde_json::from_str::<Value>(f.exemple).is_ok_and(|v| v.is_object()), "{} : l'exemple n'est pas un objet JSON", f.nom());
    }
    for r in Route::TOUTES.iter().filter(|r| r.au_catalogue()) {
        assert!(vus.contains(r.nom()), "{} : une route du catalogue sans fiche", r);
    }
}

/// CE QUE L'ECRAN AFFICHE EST FAIT POUR L'ECRAN (`routes::affiche`) : le bouton de chaque route sous « Avancé » et le
/// libelle de chaque champ, tels que le catalogue les rend a la page (`routes --avance`), et le libelle, l'aide et les
/// choix de chaque case de `donnees/champs-base.json` : brefs, sans fichier, chemin ni terme de code ; un libelle ou un
/// bouton commence par une capitale ; chaque case, que la page la montre ou non. Les phrases que rendent les routes
/// passent la meme verification (`l_exemple_de_chaque_fiche_est_accepte`). Le sens long d'une fiche de route (`sens`,
/// `geste`) peut nommer un fichier.
#[test]
fn ce_que_l_ecran_affiche_est_fait_pour_l_ecran() {
    use routes::affiche::{defaut, COURT};
    let mut fautes = Vec::new();
    let mut voir = |ou: String, t: &str, place: usize, court: bool| {
        if let Some(d) = defaut(t, place, court) {
            fautes.push(format!("{} : « {} » : {}", ou, t, d.corriger()));
        }
    };
    for f in routes::catalogue::catalogue().iter().map(|f| serde_json::to_value(f.ecrite().unwrap()).unwrap()) {
        let f = &f;
        let nom = f["nom"].as_str().unwrap();
        if let Some(b) = f["bouton"].as_str() {
            voir(format!("le bouton de {}", nom), b, COURT, true);
        }
        for c in ["charge", "cases", "cases_de_la_fin"].iter().flat_map(|k| f[*k].as_array().unwrap()) {
            voir(format!("le libellé de {}.{}", nom, c["nom"].as_str().unwrap()), c["libelle"].as_str().unwrap(), COURT, true);
        }
    }
    fn cases<'a>(v: &'a Value, ou: &str, out: &mut Vec<(String, &'a Value)>) {
        match v {
            Value::Object(o) if o.contains_key("cle") && o.contains_key("libelle") => {
                let ici = format!("{}.{}", ou, o["cle"].as_str().unwrap_or("?"));
                out.push((ici.clone(), v));
                for x in o.values() {
                    cases(x, &ici, out);
                }
            }
            Value::Object(o) => o.iter().for_each(|(k, x)| cases(x, &format!("{}{}", ou, k), out)),
            Value::Array(a) => a.iter().for_each(|x| cases(x, ou, out)),
            _ => {}
        }
    }
    let base: Value = serde_json::from_str(&std::fs::read_to_string(espace().join("../../logiciel/donnees/champs-base.json")).unwrap()).unwrap();
    let mut toutes = Vec::new();
    cases(&base, "", &mut toutes);
    // Chaque case, que la page la montre ou non : la ligne de commande la lit de meme (« un seul logiciel »).
    for (ou, c) in &toutes {
        voir(format!("champs-base.json {} : libelle", ou), c["libelle"].as_str().unwrap_or(""), CASE, true);
        if let Some(a) = c["aide"].as_str().filter(|a| !a.is_empty()) {
            voir(format!("champs-base.json {} : aide", ou), a, AIDE, false);
        }
        // Un choix sans libelle s'affiche tel qu'il s'ecrit.
        for v in c["choix"].as_array().into_iter().flatten().filter_map(Value::as_str).filter(|v| !v.is_empty()) {
            let l = c["libelles"][v].as_str().unwrap_or(v);
            voir(format!("champs-base.json {} : le choix « {} »", ou, v), l, CHOIX, false);
        }
        if let Some(l) = c["libelles"][""].as_str() {
            voir(format!("champs-base.json {} : le choix vide", ou), l, CHOIX, false);
        }
    }
    assert!(fautes.is_empty(), "ce que l'écran affiche n'est pas fait pour l'écran : récris-le bref, en mots, dans le moteur (le sens long reste au LLM) :\n{}", fautes.join("\n"));
}

/// La place du libelle d'une case de fiche (une ligne du formulaire), de son aide et d'un choix de sa liste, en caracteres.
const CASE: usize = 60;
const AIDE: usize = 500;
const CHOIX: usize = 100;

/// Les Treteaux du Banc au 02/11, avant son premier jour joue : une compagnie configuree, FULLER en saisie.
fn treteaux(test: &str) -> PathBuf {
    let d = dossier_de_test("structure", test);
    let sc = d.join("scenario");
    std::fs::create_dir_all(&sc).unwrap();
    let gestes = std::fs::read_to_string(scenario("treteaux-du-banc").join("gestes.jsonl")).unwrap();
    let debut: Vec<&str> = gestes.lines().take_while(|l| !l.contains("\"geste\":\"jour\"")).collect();
    std::fs::write(sc.join("gestes.jsonl"), debut.join("\n") + "\n").unwrap();
    rejouer_dossier(&sc, &d).1
}

/// Les exemples des fiches, joues dans l'ordre du catalogue sur une compagnie de test (Les Treteaux du Banc, au 02/11) :
/// chacun est accepte, rend sa phrase (ce que `moteur route --texte` imprime, jamais du JSON), et celle-ci tient dans sa
/// place.
#[test]
fn l_exemple_de_chaque_fiche_est_accepte() {
    let c = treteaux("exemples");
    // Le serveur du filet et du développeur : un faux, sur cette machine ; il donne une identité de poste et accepte tout,
    // chaque action rendant sa forme (`editeur.rs`).
    let (filet, _) = crate::outils::serveur(|r| {
        Some(match r.chemin.as_str() {
            "/poste" => crate::outils::http_json(200, &json!({"poste": "0".repeat(32), "cle": "0".repeat(64)})),
            "/rapport" => crate::outils::http_json(200, &json!({"recus": [], "ecartes": []})),
            "/retour" => crate::outils::http_json(200, &json!({"numero": "R-000001", "ticket": "", "ticket_numero": null})),
            "/reponses" => crate::outils::http_json(200, &json!({"retours": []})),
            _ => crate::outils::http_json(404, &json!({"erreur": "Action inconnue."})),
        })
    });
    std::fs::write(c.join("config/diagnostic.json"), json!({"serveur": filet}).to_string()).unwrap();
    // Les bulletins d'un ancien logiciel, dans la boîte de l'import : un artiste (janvier 2026, sans la CASC-SVP) et un
    // permanent (mai 2025) dont aucune pièce ne dit la nature du contrat.
    let (mut artiste, mut permanent) = (a_repetitions(), a_heures());
    for a in [&mut artiste, &mut permanent] {
        a.id.siret = "32520440200011";
        a.id.entreprise[0] = "Les Tréteaux du Banc";
    }
    permanent.id.nom = "TARTEMPION";
    permanent.id.nir = "185079999912388";
    permanent.id.conges_spectacles = None;
    permanent.cotisations.retain(|c| c.2 != "conges_spectacles");
    std::fs::create_dir_all(c.join("import")).unwrap();
    pdf(vec![page_a(&artiste)], &c.join("import/artiste.pdf"));
    pdf(vec![page_a(&permanent)], &c.join("import/permanent.pdf"));
    // Un permanent en CDI, arrete deux fois (un accident du travail, un deuil d'enfant) : ce que seul un contrat paye au mois
    // appelle (le maintien de salaire), et ce que ces motifs font declarer hors DSN.
    let cases = json!({"prenom_affichage": "Josef",
        "nom_affichage": "Svoboda", "nir": "1750544999001", "nir_cle": "19", "date_naissance": "1975-05-10", "lieu_naissance_affichage": "Villeneuve-sur-Loire",
        "departement_naissance": "44", "pays_naissance": "FR", "code_nationalite_dsn": "01", "adresse_affichage": "4 rue Kervégan", "code_postal": "44000",
        "commune_affichage": "Nantes", "email": "josef.svoboda@exemple.invalid", "emploi_affichage": "Directeur technique", "code_emploi_ft": "DIR210",
        "pcs_ese": "353C", "statut": "technicien", "statut_conventionnel": "04", "statut_categoriel_rc": "01", "date_anciennete": "2026-11-02", "sante_svp": {"niveau": "1", "conjoint": false, "enfants": 0},
        "jours_travailles": ["lundi", "mardi", "mercredi", "jeudi", "vendredi"], "actif": true});
    let changements: Vec<Value> = cases.as_object().unwrap().iter().map(|(k, v)| json!({"case": k, "valeur": v})).collect();
    let svoboda = json!({"nom": "salarie", "cle": null, "changements": changements});
    let gestes = std::fs::read_to_string(scenario("treteaux-du-banc").join("gestes.jsonl")).unwrap();
    let fiche_entreprise = gestes.lines().filter(|l| l.contains("\"route\":\"enregistrer-fiche\"") && l.contains("\"nom\":\"entreprise\"")).last().unwrap();
    let mut entreprise = serde_json::from_str::<Value>(fiche_entreprise).unwrap()["charge"].clone();
    for ch in entreprise["changements"].as_array_mut().unwrap() {
        if ch["case"] == "emploie_des_permanents" {
            ch["valeur"] = json!(true);
        }
    }
    entreprise["changements"].as_array_mut().unwrap().push(json!({"case": "date_de_versement_permanents", "valeur": "fin du mois"}));
    entreprise["depuis"] = json!("2026-11-01");
    entreprise["confirme"] = json!(true);
    for (nom, charge) in [
        ("enregistrer-fiche", entreprise),
        ("organismes-enregistrer", json!({"contrats": {"prevoyance_permanents": {"libelle": "PREVOYANCE", "organisme": "P0983"}, "sante_permanents": {"libelle": "SANTE", "organisme": "P0983"}}})),
        ("enregistrer-fiche", svoboda),
    ] {
        let r = route(&c, nom, charge, "2026-11-02");
        assert_eq!(r["ok"], json!(true), "le permanent des exemples : {} : {}", nom, r);
    }
    let cdi = json!({"salarie": "SVOBODA", "nature": "cdi", "debut": "2026-11-02", "salaire_mensuel": 3400, "quotite_contrat": crate::outils::nombre("151.67")});
    let r = crate::outils::valider(&c, "2026-11", None, &cdi, "2026-11-02");
    assert_eq!(r["ok"], json!(true), "le permanent des exemples : valider-engagement : {}", r);
    for charge in [
        json!({"salarie": "SVOBODA", "dernier_jour": "2026-11-02", "debut": "2026-11-03", "fin": "2026-11-10", "motif": "04", "date_accident": "2026-11-02"}),
        json!({"salarie": "SVOBODA", "dernier_jour": "2026-11-16", "debut": "2026-11-17", "fin": "2026-11-18", "motif": "19"}),
    ] {
        let r = route(&c, "arret-enregistrer", charge, "2026-11-02");
        assert_eq!(r["ok"], json!(true), "le permanent des exemples : arret-enregistrer : {}", r);
    }
    // La carte que valident, modifient et ajustent les exemples, ecrite comme l'ecran l'ecrit.
    let fuller = json!({"salarie": "FULLER", "nature": "cddu_spectacle", "spectacle": "le-banc-des-menteurs", "lignes": [{"rubrique": "cachet", "quantite": 1, "taux": 150, "lieu": null, "dates": ["2026-11-27"], "planning": []}]});
    let r = crate::outils::saisir(&c, "2026-11", None, &fuller, "2026-11-02");
    assert_eq!(r["ok"], json!(true), "la carte des exemples : {}", r);
    // Le retrait de l'engagement en dernier : les exemples des imprevus portent sur celui que valider-engagement pose.
    let mut fiches = catalogue();
    fiches.sort_by_key(|f| f.route == Route::RetirerEngagement);
    // Un administrateur, pour son tableau et son dépôt groupé : la compagnie est rattachée à son registre, après les
    // exemples du mot de passe du logiciel (celui d'une compagnie seule).
    let inst = c.parent().unwrap().parent().unwrap();
    let registre = json!({"quoi": "Le registre de l'administrateur.", "compagnies": [{"dossier": c.strip_prefix(inst).unwrap().to_string_lossy(), "depuis": "2026-01-02T09:00"}]});
    let mut phrases_fautives = Vec::new();
    for f in &fiches {
        if (f.route == Route::AdministrateurTableau || f.route == Route::AdministrateurDeposer) && !inst.join("administrateur.json").exists() {
            std::fs::write(inst.join("administrateur.json"), registre.to_string()).unwrap();
        }
        // Un secret ne se lit jamais dans la charge : la fausse demande des essais le tape (`demande::canal_de_la_route`).
        let exemple = serde_json::from_str(f.exemple).unwrap();
        let r = match f.route {
            Route::GoogleAutoriser => google_autoriser(&c, exemple),
            // La suppression efface une compagnie ajoutee a part, pas celle des exemples.
            Route::CompagnieSupprimer => {
                let autre = c.parent().unwrap().join("ajoutee-a-supprimer");
                crate::outils::copier(&c.join("config"), &autre.join("config"));
                std::fs::write(autre.join("COPIE-DE-DEVELOPPEMENT.json"), json!({"copie_de": "compagnie fictive ajoutee", "ajoutee": true, "quand": "2026-11-09T09:00:00", "quoi": "ajoutee"}).to_string()).unwrap();
                let r = route(&autre, f.nom(), exemple, "2026-11-02");
                assert!(!autre.exists(), "compagnie-supprimer : la compagnie est encore la");
                r
            }
            // Ce qui ne porte que sur une compagnie reelle, une fictive livree ou une compagnie creee en demo se joue dans
            // une installation a part.
            Route::CompagnieCouleur | Route::CompagnieRemettreAZero | Route::CompagniesGarder => route(&installation_a_part(&c, f.nom()).1, f.nom(), exemple, "2026-11-02"),
            Route::AdministrateurRattacher | Route::AdministrateurDetacher => administrateur(&c, f.route, exemple),
            // Le dossier portable : une installation a part qui porte deja les trois systemes (rien ne se telecharge).
            Route::PortableCompleter => {
                let (inst, reelle) = installation_a_part(&c, f.nom());
                for s in compagnie::maj::Systeme::TOUS {
                    let p = compagnie::maj::programme_de(&inst.join("logiciel"), s, "puppetpaie");
                    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                    std::fs::write(p, "").unwrap();
                }
                route(&reelle, f.nom(), exemple, "2026-11-02")
            }
            _ => route_avec(&c, f.nom(), exemple, "2026-11-02", &[(VARIABLE_ESSAIS, "exemple")]),
        };
        assert_eq!(r["ok"], json!(true), "{} refuse l'exemple de sa fiche : {}", f.nom(), r);
        assert!(r["resultat"]["phrase"].as_str().is_some_and(|p| !p.trim().is_empty()), "{} : sa phrase manque (moteur route --texte ne rend que des phrases, celles que l'écran lit) : {}", f.nom(), r);
        let phrases: routes::PhrasesRendues = socle::forme::lire(&r["resultat"], "les phrases que rend une route").unwrap();
        for p in phrases.toutes() {
            if let Some(d) = routes::affiche::defaut(p, usize::MAX, false) {
                phrases_fautives.push(format!("{} : « {} » : {}", f.nom(), p, d.corriger()));
            }
        }
        if f.phrase > 0 {
            let n = phrases.plus_longue();
            assert!(n <= f.phrase, "{} : une phrase qu'elle rend (la sienne, ou celle d'une de ses lignes) passe sa place ({}) : {} caractères : {}", f.nom(), f.phrase, n, r);
        }
    }
    assert!(phrases_fautives.is_empty(), "des phrases que rendent les routes ne sont pas faites pour l'écran (un fichier, un chemin, un terme de code) : récris-les en mots, dans le moteur :\n{}", phrases_fautives.join("\n"));
}


/// L'exemple de `google-autoriser`, sur un faux Google local (rien ne sort de la machine) : le test fait le navigateur
/// (il lit l'adresse de la fenetre de Google que le moteur dit sur sa sortie d'erreur, et renvoie un code a l'adresse
/// locale qu'elle nomme) ; le faux Google rend un jeton et l'adresse du compte.
fn google_autoriser(c: &Path, exemple: Value) -> Value {
    use std::io::{BufRead, Read, Write};
    use std::process::{Command, Stdio};
    let (google, _) = crate::outils::serveur(|r| {
        Some(match r.chemin.as_str() {
            "/token" => crate::outils::http_json(200, &json!({"refresh_token": "essai", "access_token": "essai", "scope": ""})),
            "/userinfo" => crate::outils::http_json(200, &json!({"email": "sauvegarde@exemple.invalid"})),
            _ => crate::outils::http(404, "text/plain", b""),
        })
    });
    // L'application du logiciel n'est livree qu'avec une version publiee : celle de la compagnie, inventee.
    std::fs::write(c.join("config/google.json"), json!({"client_id": "essai.apps.googleusercontent.com", "client_secret": "essai"}).to_string()).unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_moteur"));
    cmd.args(["route", "--racine", c.to_str().unwrap(), "--date", "2026-11-02"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    crate::outils::bac_a_sable(&mut cmd, c, "2026-11-02");
    cmd.env(routes::coffre::VARIABLE_GOOGLE_ESSAIS, &google).env(VARIABLE_ESSAIS, "exemple");
    let mut p = cmd.spawn().unwrap();
    p.stdin.take().unwrap().write_all(json!({"route": "google-autoriser", "charge": exemple}).to_string().as_bytes()).unwrap();
    let (envoi, recu) = std::sync::mpsc::channel();
    let erreur = p.stderr.take().unwrap();
    std::thread::spawn(move || {
        for l in std::io::BufReader::new(erreur).lines().map_while(Result::ok) {
            let _ = envoi.send(l);
        }
    });
    let adresse = recu.iter().find_map(|l| l.split_once(": https://").map(|(_, u)| u.to_string())).expect("l'adresse de la fenetre de Google");
    let valeur = |cle: &str| adresse.split(['?', '&']).find_map(|kv| kv.strip_prefix(&format!("{}=", cle))).unwrap().to_string();
    let retour = valeur("redirect_uri").replace("%3A", ":").replace("%2F", "/");
    let hote = retour.trim_start_matches("http://").trim_end_matches('/');
    let mut navigateur = std::net::TcpStream::connect(hote).unwrap();
    // Une seule ecriture : le moteur repond et ferme des la premiere ligne lue.
    let requete = format!("GET /?code=essai&state={} HTTP/1.1\r\nHost: {}\r\n\r\n", valeur("state"), hote);
    let _ = navigateur.write_all(requete.as_bytes());
    let _ = navigateur.read_to_end(&mut Vec::new());
    let sortie = p.wait_with_output().unwrap().stdout;
    serde_json::from_slice(&sortie).unwrap()
}

/// Une installation debloquee a part, montee pour un exemple : une compagnie creee en demo (`fictive:ajoutee-demo`), une
/// fictive livree (`fictive:fictive-essai`), une compagnie reelle (`cie:la-compagnie`). Rend l'installation et la reelle.
fn installation_a_part(c: &Path, test: &str) -> (PathBuf, PathBuf) {
    let inst = crate::outils::installation(&dossier_de_test("structure", test), "2026.10.22");
    ajouter_venant(&inst, "Demo", Venue::Neuve).unwrap();
    crate::outils::debloquer(&inst);
    let reelle = ajouter_venant(&inst, "La Compagnie", Venue::Neuve).unwrap().racine;
    let livree = inst.join("fictives").join("fictive-essai");
    crate::outils::copier(&c.join("config"), &livree.join("config"));
    std::fs::write(livree.join("COPIE-DE-DEVELOPPEMENT.json"), json!({"copie_de": "compagnie fictive essai", "quoi": "livree"}).to_string()).unwrap();
    (inst, reelle)
}

/// Les exemples de `administrateur-rattacher` et `administrateur-detacher`, dans une installation a part : la compagnie
/// reelle porte le mot de passe de l'administrateur, une seconde se rattache (puis se detache). Une compagnie reelle n'a
/// que le canal du poste (`demande::canal_de_la_route`) : les mots de passe que l'humain taperait sont ceux de la charge,
/// comme la page les donne, sur le coffre d'un poste de test.
fn administrateur(c: &Path, route: Route, exemple: Value) -> Value {
    const ADMIN: &str = "administrateur-essai-1";
    let (inst, a) = installation_a_part(c, route.nom());
    let poste = Poste { dossier_cle: inst.join("cle-du-poste"), maintenant: socle::paris::Moment::a_paris(socle::date::Jour::nouveau(2026, 11, 2).unwrap(), 10, 0, 0).unwrap() };
    poste.coffre(&a).poser_ouverture(ADMIN, None).unwrap();
    let b = ajouter_venant(&inst, "Seconde", Venue::Neuve).unwrap().racine;
    let mut charge = exemple;
    charge["administrateur"] = json!(ADMIN);
    if route == Route::AdministrateurDetacher {
        routes::coffre::administrateur(&b, Route::AdministrateurRattacher, serde_json::from_value(json!({"administrateur": ADMIN})).unwrap(), None, &poste).unwrap();
        charge["nouveau"] = json!("propre-essai-1");
        charge["confirmation"] = json!("propre-essai-1");
    }
    match routes::coffre::administrateur(&b, route, serde_json::from_value(charge).unwrap(), None, &poste) {
        Ok(v) => json!({"ok": true, "resultat": serde_json::to_value(v).unwrap()}),
        Err(e) => json!({"ok": false, "erreur": e.en_lignes()}),
    }
}

/// Les crates de l'espace, de bas en haut (`membres`) : chacune ne prend qu'a celles d'en dessous ; et seul le `socle`,
/// la porte, prend `serde_json` pour son code (`[dependencies]`) : une autre crate qui voudrait lire du JSON brut ne
/// compile pas.
#[test]
fn une_crate_ne_prend_qu_a_celles_du_dessous() {
    let ordre = membres();
    for (rang, nom) in ordre.iter().enumerate() {
        let t = std::fs::read_to_string(espace().join(nom).join("Cargo.toml")).unwrap();
        // Toute section de dependances du code (`[dependencies]`, `[target.'…'.dependencies]`), pas celles des essais.
        let sections = t.split("\n[").filter(|s| s.split(']').next().is_some_and(|h| h.ends_with("dependencies") && !h.contains("dev-")));
        for l in sections.flat_map(|s| s.lines().skip(1)) {
            let dep = l.split([' ', '=', '.']).next().unwrap();
            if let Some(r) = ordre.iter().position(|x| *x == dep) {
                assert!(r < rang, "{} prend a {}, qui n'est pas en dessous", nom, dep);
            }
            assert!(dep != "serde_json" || nom == "socle", "{} prend serde_json pour son code : seul le socle lit le JSON brut, les autres lisent des formes (`socle::forme`)", nom);
        }
    }
}

/// LES NIVEAUX DES LINTS TIENNENT (les `Cargo.toml`) : l'espace interdit le code mort ; chaque crate de l'espace reprend
/// ses niveaux (`[lints] workspace = true`) ; aucune configuration de Cargo ne les abaisse.
#[test]
fn les_niveaux_des_lints_tiennent() {
    let espace_toml = std::fs::read_to_string(espace().join("Cargo.toml")).unwrap();
    assert!(espace_toml.contains("dead_code = \"forbid\""), "l'espace interdit le code mort");
    for nom in membres() {
        let t = std::fs::read_to_string(espace().join(&nom).join("Cargo.toml")).unwrap();
        assert!(t.contains("[lints]\nworkspace = true"), "{} : `[lints] workspace = true`", nom);
    }
    for config in [espace().join(".cargo"), espace().join("../../.cargo")] {
        for f in std::fs::read_dir(&config).into_iter().flatten().flatten() {
            let t = std::fs::read_to_string(f.path()).unwrap();
            assert!(!(t.contains("-A") || t.contains("--cap-lints") || t.contains("-W")), "{} abaisse un lint", f.path().display());
        }
    }
}

/// LE CATALOGUE EST RANGE (lu dans `catalogue()` et `FICHES`, et joue par `moteur route`) : une route sous « Avancé »
/// n'a aucun champ objet, que la page ne saisit pas d'une case ; une lecture n'ecrit pas ; les routes qui ecrivent d'un
/// module ont une seule place ; une route inconnue se refuse en se nommant a part.
#[test]
fn le_catalogue_est_range() {
    use routes::catalogue::{Acte, Ecran, Forme, FICHES};
    for f in catalogue() {
        match f.ecran {
            Ecran::Auto(..) => assert!(f.charge.iter().all(|c| c.forme != Forme::Objet), "{} s'affiche sous « Avancé » : aucun champ de sa charge n'est un objet", f.nom()),
            Ecran::Lecture | Ecran::LectureAvancee => assert!(f.acte == Acte::Lit, "{} est une lecture et écrit", f.nom()),
            _ => {}
        }
    }
    for module in FICHES {
        let places: BTreeSet<&str> = module.iter().filter(|f| f.acte != Acte::Lit).filter_map(|f| f.ecran.place()).map(|p| p.nom()).collect();
        assert!(places.len() <= 1, "un module, une place : les routes qui écrivent de {:?} vont à {:?}", module.iter().map(|f| f.nom()).collect::<Vec<_>>(), places);
    }
    let r = route(&treteaux("inconnue"), "pas-une-route", json!({}), "2026-11-02");
    assert!(r["ok"] == json!(false) && r["refus"]["classe"] == json!("RouteInconnue") && r["refus"]["a_part"]["route"].is_string(), "{}", r);
}

/// L'ATELIER N'ENTRE JAMAIS DANS LE LOGICIEL :la crate `atelier` (le binaire `moteur-atelier`) allume la feature
/// `atelier` de ce qu'elle prend, et Cargo unifie les features de ce qu'il construit ensemble. Elle est donc hors de
/// `default-members` de l'espace (la suite ne l'allume jamais), au-dessus de tout le logiciel, et aucun manifeste ne la
/// prend, ni l'espace (toute section, essais compris) ni la fenetre.
#[test]
fn l_atelier_reste_hors_du_logiciel() {
    let (membres, logiciel) = (membres(), le_logiciel());
    assert_eq!(membres[..logiciel.len()], logiciel[..], "`default-members` porte le logiciel, de bas en haut, et les membres hors du logiciel viennent au-dessus");
    let hors: Vec<&String> = membres[logiciel.len()..].iter().collect();
    assert!(hors.iter().any(|h| *h == "atelier"), "l'atelier est un membre de l'espace, hors de `default-members`");
    for h in hors {
        let prend = regex::Regex::new(&format!(r#"^{}\s*=|path\s*=\s*"\.\./{}""#, h, h)).unwrap();
        for nom in logiciel.iter().map(String::as_str).chain(["fenetre", "fabrique-pdf"]) {
            let t = std::fs::read_to_string(espace().join(nom).join("Cargo.toml")).unwrap();
            let mut section = "";
            for l in t.lines() {
                if l.starts_with('[') {
                    section = l;
                } else if section.contains("dependencies") {
                    assert!(!prend.is_match(l), "{}/Cargo.toml prend la crate `{}` ({}) : {}", nom, h, section, l);
                }
            }
        }
    }
}

/// UNE LISTE DE LA NORME QUE LE LOGICIEL DECLARE (`droit::natures::ListeDeLaNorme`) EST CELLE DES VERSIONS LIVREES, ni
/// plus ni moins : pour chaque version (`norme-dsn-base.json`), la liste de valeurs que sa table des formats a relevee
/// pour la rubrique (`formats-dsn-*-base.json`, `moteur-atelier extraire formats`) est exactement l'ensemble des
/// variantes que leurs bornes (`depuis`, `jusqu_a`) disent de cette version ; donc chaque code d'une liste est une
/// variante, et chaque variante est dans au moins une liste.
#[test]
fn une_liste_de_la_norme_est_celle_des_versions_livrees() {
    use droit::natures::{ListeDeLaNorme, ModaliteDuTempsDeTravail, MotifDeRecours, MotifDeRupture};
    fn confronter<T: ListeDeLaNorme>(fautes: &mut Vec<String>) {
        let mut union = BTreeSet::new();
        for n in declarations::norme::versions().unwrap() {
            let valeurs = declarations::formats::format_en(&n, T::RUBRIQUE).unwrap().and_then(|f| f.valeurs);
            let Some(liste) = valeurs.map(|v| v.into_keys().collect::<Vec<String>>()) else {
                fautes.push(format!("{} ({}) : la table des formats de {} n'en a pas la liste", T::quoi(), T::RUBRIQUE, n.version));
                continue;
            };
            for code in &liste {
                match T::du_code_de_la_norme(code) {
                    None => fautes.push(format!("{} ({}) : le code {} de la liste {} n'est pas une variante", T::quoi(), T::RUBRIQUE, code, n.version)),
                    Some(v) if !n.a_dans_sa_liste(v).unwrap() => fautes.push(format!(
                        "{} ({}) : la liste {} a le code {}, que les bornes de sa variante excluent",
                        T::quoi(),
                        T::RUBRIQUE,
                        n.version,
                        code
                    )),
                    Some(_) => {}
                }
            }
            for v in T::toutes() {
                if !liste.iter().any(|c| c == v.code_de_la_norme()) && n.a_dans_sa_liste(*v).unwrap() {
                    fautes.push(format!(
                        "{} ({}) : la variante {} se dit de {}, dont la liste ne l'a pas",
                        T::quoi(),
                        T::RUBRIQUE,
                        v.code_de_la_norme(),
                        n.version
                    ));
                }
            }
            union.extend(liste);
        }
        for v in T::toutes() {
            if !union.contains(v.code_de_la_norme()) {
                fautes.push(format!("{} ({}) : la variante {} n'est dans aucune liste livree", T::quoi(), T::RUBRIQUE, v.code_de_la_norme()));
            }
        }
    }
    let mut fautes = Vec::new();
    confronter::<MotifDeRecours>(&mut fautes);
    confronter::<MotifDeRupture>(&mut fautes);
    confronter::<ModaliteDuTempsDeTravail>(&mut fautes);
    confronter::<droit::base_assujettie::BaseAssujettie>(&mut fautes);
    confronter::<droit::cotisation_etablissement::CotisationDEtablissement>(&mut fautes);
    confronter::<droit::nature_declaration::NatureDeLaDeclaration>(&mut fautes);
    confronter::<droit::type_declaration::TypeDeLaDeclaration>(&mut fautes);
    confronter::<droit::type_taux_pas::TypeDeTauxPas>(&mut fautes);
    confronter::<droit::autre_revenu_brut::AutreElementDeRevenuBrut>(&mut fautes);
    confronter::<droit::code_cotisation::CodeCotisation81>(&mut fautes);
    confronter::<droit::natures::VersionDeLaNorme>(&mut fautes);
    confronter::<droit::preavis::TypeDePreavis>(&mut fautes);
    confronter::<droit::type_de_prime::TypeDePrime>(&mut fautes);
    confronter::<droit::preavis::TransactionEnCours>(&mut fautes);
    confronter::<droit::preavis::RattachementDuVersement>(&mut fautes);
    confronter::<droit::arrets::MotifDArret>(&mut fautes);
    confronter::<droit::arrets::MotifDeReprise>(&mut fautes);
    confronter::<droit::arrets::SubrogationDeLArret>(&mut fautes);
    confronter::<droit::permanents::MotifDeSuspension>(&mut fautes);
    confronter::<droit::natures::UniteDeQuotite>(&mut fautes);
    confronter::<droit::natures::ModaliteDeDeclarationDeFin>(&mut fautes);
    confronter::<droit::natures::TypeDActivite>(&mut fautes);
    confronter::<droit::natures::UniteDeMesure>(&mut fautes);
    confronter::<droit::base_assujettie::TypeDeComposant>(&mut fautes);
    confronter::<droit::organismes::QualifiantDAssiette>(&mut fautes);
    confronter::<droit::cotisation_etablissement::TypeDeBoethExterne>(&mut fautes);
    confronter::<droit::nature_declaration::EssaiOuReel>(&mut fautes);
    confronter::<droit::nature_declaration::NatureDEvenementDeclencheur>(&mut fautes);
    confronter::<droit::apprentissage::CodeTaxe>(&mut fautes);
    assert!(fautes.is_empty(), "une liste de la norme declaree en `enum` est l'union des listes livrees :\n{}", fautes.join("\n"));
}

/// Chaque crate de l'espace : chaque dossier de `atelier/sources/` qui a son `Cargo.toml` (`fenetre/` et
/// `fabrique-pdf/` compris, hors de `members`).
pub(crate) fn dossiers_des_crates() -> Vec<PathBuf> {
    std::fs::read_dir(espace()).unwrap().map(|e| e.unwrap().path()).filter(|d| d.join("Cargo.toml").is_file()).collect()
}

/// Les sources de chaque crate de l'espace (`dossiers_des_crates`) : les `.rs` de son `src/`.
pub(crate) fn sources_des_crates() -> Vec<PathBuf> {
    dossiers_des_crates().into_iter().flat_map(|d| sources(&d.join("src"))).collect()
}

/// Les essais de chaque crate de l'espace (`dossiers_des_crates`) qui en a : les `.rs` de son `tests/`.
pub(crate) fn essais_des_crates() -> Vec<PathBuf> {
    dossiers_des_crates().into_iter().filter(|d| d.join("tests").is_dir()).flat_map(|d| sources(&d.join("tests"))).collect()
}

/// Chaque ligne des sources des crates ou `motif` se trouve : `crate/src/…:ligne : ce qu'il a trouve`.
fn lignes_ou(motif: &str) -> Vec<String> {
    let motif = regex::Regex::new(motif).unwrap();
    let mut v = Vec::new();
    for p in sources_des_crates() {
        for (n, l) in std::fs::read_to_string(&p).unwrap().lines().enumerate() {
            if let Some(m) = motif.find(l) {
                v.push(format!("{}:{} : `{}`", p.strip_prefix(espace()).unwrap().display(), n + 1, m.as_str().trim()));
            }
        }
    }
    v
}

/// AUCUN FLOTTANT : aucune source d'une crate n'ecrit `f32` ni `f64`. Un montant, un taux sont des `Decimal`, une
/// mesure d'un PDF une `fabrique_pdf::mesure::Mesure`, le reste des entiers.
#[test]
fn aucun_flottant_dans_le_code() {
    let fautes = lignes_ou(r"(?:^|[^A-Za-z])f(?:32|64)\b");
    assert!(fautes.is_empty(), "aucun flottant : un `Decimal`, une `Mesure` ou des entiers :\n{}", fautes.join("\n"));
}

/// AUCUNE VALEUR PAR DEFAUT SI ABSENT (`AGENTS.md`, « Un champ facultatif ne s'écrit que s'il est rempli ») : aucune
/// source d'une crate n'ecrit `unwrap_or`, `unwrap_or_default`, `unwrap_or_else`, `map_or` ni `map_or_else`. Une donnee
/// absente reste vide (l'`Option` se porte) ou se refuse ; une erreur remonte (`?`).
#[test]
fn aucune_valeur_par_defaut_si_absent() {
    let fautes = lignes_ou(r"\b(?:unwrap_or(?:_default|_else)?|map_or(?:_else)?)\b");
    assert!(fautes.is_empty(), "une absence ne devient pas une valeur : elle reste `None`, ou se refuse en nommant la case :\n{}", fautes.join("\n"));
}

/// UNE FORME LUE REFUSE CE QU'ELLE NE CONNAIT PAS (« Le corps et ses portes ») : dans les sources des crates, chaque
/// `#[derive(…Deserialize…)]` porte `deny_unknown_fields` entre lui et la declaration du type ; sauf une reponse d'un
/// tiers, qui le dit par son type : `impl socle::forme::ReponseDUnTiers for <elle>`, dans son fichier, et qui ne se lit
/// alors que par `socle::forme::lire_reponse_tiers`.
#[test]
fn chaque_forme_lue_refuse_une_cle_inconnue() {
    let mut fautes = Vec::new();
    for p in sources_des_crates() {
        let t = std::fs::read_to_string(&p).unwrap();
        let lignes: Vec<&str> = t.lines().collect();
        for i in (0..lignes.len()).filter(|i| lignes[*i].contains("derive(") && lignes[*i].contains("Deserialize")) {
            let fin = i + lignes[i..].iter().position(|x| x.contains("struct ") || x.contains("enum ")).unwrap();
            // Une forme a cles : une structure a champs nommes. Une enum ou une enveloppe (`Nom(String)`) n'a pas de cle.
            let Some((_, apres)) = lignes[fin].split_once("struct ") else { continue };
            // Lue par une autre forme (`try_from`, `from`) : c'est elle qui se lit, et elle passe ici a son tour.
            if !lignes[fin].trim_end().ends_with('{') || lignes[i..fin].iter().any(|x| x.contains("from = ")) {
                continue;
            }
            let nom: String = apres.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            let tiers = t.contains(&format!("ReponseDUnTiers for {} ", nom));
            if !lignes[i..=fin].join(" ").contains("deny_unknown_fields") && !tiers {
                fautes.push(format!("{}:{} : {}", p.strip_prefix(espace()).unwrap().display(), fin + 1, nom));
            }
        }
    }
    assert!(fautes.is_empty(), "une forme lue sans `deny_unknown_fields`, qui ne se declare pas reponse d'un tiers (`socle::forme::ReponseDUnTiers`) :\n{}", fautes.join("\n"));
}

/// Un source Rust dont les commentaires (de ligne et de bloc, imbriques), les textes (`"…"`, `b"…"`, `r#"…"#`,
/// `br"…"`) et les caracteres (`'…'`, `b'…'`, `'\u{…}'`) sont blanchis, ses sauts de ligne gardes : ce qui reste est
/// le code que le compilateur lit.
pub(crate) fn code_sans_textes(t: &str) -> Vec<char> {
    let mut c: Vec<char> = t.chars().collect();
    let n = c.len();
    let ident = |x: char| x.is_alphanumeric() || x == '_';
    let mut a_blanchir: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < n {
        let avant = if i > 0 { c[i - 1] } else { ' ' };
        let debut = i;
        match c[i] {
            '/' if c.get(i + 1) == Some(&'/') => {
                while i < n && c[i] != '\n' {
                    i += 1;
                }
            }
            '/' if c.get(i + 1) == Some(&'*') => {
                let mut profondeur = 0;
                while i < n {
                    if c[i] == '/' && c.get(i + 1) == Some(&'*') {
                        profondeur += 1;
                        i += 2;
                    } else if c[i] == '*' && c.get(i + 1) == Some(&'/') {
                        profondeur -= 1;
                        i += 2;
                        if profondeur == 0 {
                            break;
                        }
                    } else {
                        i += 1;
                    }
                }
            }
            '"' => {
                i += 1;
                while i < n && c[i] != '"' {
                    i += if c[i] == '\\' { 2 } else { 1 };
                }
                i += 1;
            }
            'r' if (c.get(i + 1) == Some(&'"') || c.get(i + 1) == Some(&'#'))
                && (!ident(avant) || (avant == 'b' && (i < 2 || !ident(c[i - 2])))) =>
            {
                let mut j = i + 1;
                while c.get(j) == Some(&'#') {
                    j += 1;
                }
                let dieses = j - i - 1;
                if c.get(j) != Some(&'"') {
                    i += 1;
                    continue;
                }
                j += 1;
                while j < n && !(c[j] == '"' && (0..dieses).all(|k| c.get(j + 1 + k) == Some(&'#'))) {
                    j += 1;
                }
                i = j + 1 + dieses;
            }
            '\'' if c.get(i + 1) == Some(&'\\') => {
                i += 3;
                while i < n && c[i] != '\'' {
                    i += 1;
                }
                i += 1;
            }
            '\'' if c.get(i + 2) == Some(&'\'') => i += 3,
            _ => {
                i += 1;
                continue;
            }
        }
        a_blanchir.push((debut, i.min(n)));
    }
    for (a, b) in a_blanchir {
        for x in &mut c[a..b] {
            if *x != '\n' {
                *x = ' ';
            }
        }
    }
    c
}

/// LES FICHES N'ONT QU'UNE FORME : ce que la lecture de `champs-base.json` ne tient pas (elle apparie lignes et cases,
/// `declarations::champs::ranger`) : les valeurs d'un choix sont celles de son type (la ligne vide en plus), et l'aide
/// d'une case que les normes bornent dit sa longueur (`droit::fiche::texte_borne!`). La ligne `historique` (forme
/// `histoire`) n'est pas une donnee.
#[test]
fn champs_base_et_les_fiches_ont_les_memes_cases() {
    use droit::fiche::{ChampsEntreprise, ChampsSalarie, Declaree};
    fn feuilles<'a>(d: &'a [Declaree], devant: &str, out: &mut Vec<(String, &'a Declaree)>) {
        for c in d {
            let chemin = format!("{}{}", devant, c.cle);
            if c.cases.is_empty() {
                out.push((chemin, c));
            } else {
                feuilles(&c.cases, &format!("{}.", chemin), out);
            }
        }
    }
    fn choix<K>(ou: &str, ligne: &Value, d: &Declaree<K>, fautes: &mut Vec<String>) {
        if ligne["forme"] != json!("choix") {
            return;
        }
        let ecrits: BTreeSet<&str> = ligne["choix"].as_array().into_iter().flatten().filter_map(Value::as_str).filter(|v| !v.is_empty()).collect();
        let declares: BTreeSet<&str> = d.valeurs.iter().copied().collect();
        if ecrits != declares {
            fautes.push(format!("{} : champs-base offre {:?}, la forme {:?}", ou, ecrits, declares));
        }
    }
    fn longueur<K>(ou: &str, ligne: &Value, d: &Declaree<K>, fautes: &mut Vec<String>) {
        let Some((n, unite)) = d.longueur else { return };
        let dite = format!("{} {}", n, unite);
        if !ligne["aide"].as_str().unwrap_or("").contains(&dite) {
            fautes.push(format!("{} : son aide ne dit pas sa longueur, « {} » (`texte_borne!`)", ou, dite));
        }
    }
    let base: Value = serde_json::from_str(&std::fs::read_to_string(espace().join("../../logiciel/donnees/champs-base.json")).unwrap()).unwrap();
    let mut fautes = Vec::new();
    for (nom, declarees) in [("entreprise", ChampsEntreprise::cases()), ("salarie", ChampsSalarie::cases())] {
        let lignes: Vec<&Value> = base[nom].as_array().unwrap().iter().filter(|l| l["forme"] != json!("histoire")).collect();
        // L'appariement des lignes et des cases, et les cles d'une ligne, la lecture de la table les tient
        // (`declarations::champs::ranger`, `deny_unknown_fields`).
        for l in &lignes {
            let cle = l["cle"].as_str().unwrap_or("");
            let ou = format!("{}.{}", nom, cle);
            let Some(d) = declarees.iter().find(|d| d.cle.nom() == cle) else { continue };
            choix(&ou, l, d, &mut fautes);
            longueur(&ou, l, d, &mut fautes);
            let Some(cases) = l["champs"].as_array().filter(|c| !c.is_empty()) else { continue };
            let mut declarees_ici = Vec::new();
            feuilles(&d.cases, "", &mut declarees_ici);
            for s in cases {
                let sc = s["cle"].as_str().unwrap_or("");
                if let Some((_, ds)) = declarees_ici.iter().find(|(c, _)| c == sc) {
                    choix(&format!("{}.{}", ou, sc), s, ds, &mut fautes);
                    longueur(&format!("{}.{}", ou, sc), s, ds, &mut fautes);
                }
            }
        }
    }
    assert!(fautes.is_empty(), "champs-base.json et la forme des fiches (droit::fiche) se contredisent :\n{}", fautes.join("\n"));
}

/// Les litteraux de chaine d'un source Rust (hors commentaires), avec la ligne ou chacun commence ; une fin de ligne
/// echappee (`\` puis le blanc qui suit) est retiree, comme le fait le compilateur.
pub(crate) fn litteraux_rs(t: &str) -> Vec<(usize, String)> {
    jetons_rs(t).into_iter().filter(|j| !j.3).map(|(l, _, _, _, s)| (l, s)).collect()
}

/// Les litteraux de chaine et les commentaires d'un source Rust : `(ligne, debut, fin, commentaire ?, texte)`, debut et
/// fin en caracteres.
fn jetons_rs(t: &str) -> Vec<(usize, usize, usize, bool, String)> {
    let c: Vec<char> = t.chars().collect();
    let (mut v, mut i, mut ligne) = (Vec::new(), 0, 1);
    while i < c.len() {
        match c[i] {
            '\n' => {
                ligne += 1;
                i += 1;
            }
            '/' if c.get(i + 1) == Some(&'/') => {
                let debut = i;
                while i < c.len() && c[i] != '\n' {
                    i += 1;
                }
                v.push((ligne, debut, i, true, c[debut..i].iter().collect()));
            }
            '/' if c.get(i + 1) == Some(&'*') => {
                let (debut, l) = (i, ligne);
                i += 2;
                while i + 1 < c.len() && !(c[i] == '*' && c[i + 1] == '/') {
                    ligne += (c[i] == '\n') as usize;
                    i += 1;
                }
                i += 2;
                v.push((l, debut, i.min(c.len()), true, c[debut..i.min(c.len())].iter().collect()));
            }
            '\'' => {
                // Un caractere ('"', '\'', '\u{a0}') ; sinon une duree de vie.
                if c.get(i + 1) == Some(&'\\') {
                    i += 2;
                    while i < c.len() && c[i] != '\'' {
                        i += 1;
                    }
                    i += 1;
                } else if c.get(i + 2) == Some(&'\'') {
                    i += 3;
                } else {
                    i += 1;
                }
            }
            'r' if (c.get(i + 1) == Some(&'"') || c.get(i + 1) == Some(&'#')) && (i == 0 || !(c[i - 1].is_alphanumeric() || c[i - 1] == '_')) => {
                let mut j = i + 1;
                let mut dieses = 0;
                while c.get(j) == Some(&'#') {
                    dieses += 1;
                    j += 1;
                }
                if c.get(j) != Some(&'"') {
                    i += 1;
                    continue;
                }
                let (debut, ouvre) = (ligne, i);
                let mut s = String::new();
                j += 1;
                while j < c.len() && !(c[j] == '"' && (0..dieses).all(|k| c.get(j + 1 + k) == Some(&'#'))) {
                    ligne += (c[j] == '\n') as usize;
                    s.push(c[j]);
                    j += 1;
                }
                i = j + 1 + dieses;
                v.push((debut, ouvre, i, false, s));
            }
            '"' => {
                let (debut, ouvre) = (ligne, i);
                let mut s = String::new();
                i += 1;
                while i < c.len() && c[i] != '"' {
                    if c[i] == '\\' && c.get(i + 1) == Some(&'\n') {
                        i += 1;
                        while i < c.len() && c[i].is_whitespace() {
                            ligne += (c[i] == '\n') as usize;
                            i += 1;
                        }
                        continue;
                    }
                    if c[i] == '\\' {
                        s.push(c[i]);
                        i += 1;
                    }
                    if let Some(&x) = c.get(i) {
                        ligne += (x == '\n') as usize;
                        s.push(x);
                    }
                    i += 1;
                }
                i += 1;
                v.push((debut, ouvre, i, false, s));
            }
            _ => i += 1,
        }
    }
    v
}

/// LA PAGE SE CHARGE : chaque script se lit sans erreur de syntaxe (`node --check`), et ses scripts, classiques
/// (`index.html`, `<script src>`), partagent une seule
/// portee. Un nom declare deux fois au premier niveau (`function`, `const`, `let`, `class`), dans un fichier ou d'un
/// fichier a l'autre, fait refuser le second fichier entier au navigateur, et tout ce qu'il porte manque a l'ecran.
#[test]
fn chaque_script_de_la_page_se_charge() {
    let decl = regex::Regex::new(r"(?m)^(?:async\s+)?(?:function\*?|const|let|class)\s+([A-Za-z_$][\w$]*)").unwrap();
    let mut vus: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    let mut mal_ecrits = Vec::new();
    for p in std::fs::read_dir(depot().join("logiciel/interface/js")).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "js")) {
        let t = std::fs::read_to_string(&p).unwrap();
        let nom = p.file_name().unwrap().to_string_lossy().into_owned();
        let syntaxe = std::process::Command::new("node").arg("--check").arg(&p).output().expect("node, pour lire chaque script de la page comme le navigateur le lit");
        if !syntaxe.status.success() {
            mal_ecrits.push(format!("{} : {}", nom, String::from_utf8_lossy(&syntaxe.stderr).lines().find(|l| l.contains("Error")).unwrap_or("")));
        }
        for c in decl.captures_iter(&t) {
            let ligne = t[..c.get(0).unwrap().start()].matches('\n').count() + 1;
            vus.entry(c[1].to_string()).or_default().push(format!("{}:{}", nom, ligne));
        }
    }
    assert!(mal_ecrits.is_empty(), "{} script(s) de la page que le navigateur refuserait (node --check) :\n{}", mal_ecrits.len(), mal_ecrits.join("\n"));
    let doubles: Vec<String> = vus.into_iter().filter(|(_, l)| l.len() > 1).map(|(n, l)| format!("{} : {}", n, l.join(", "))).collect();
    assert!(doubles.is_empty(), "{} nom(s) declares deux fois au premier niveau des scripts de la page : le navigateur refuserait le second fichier ; renommer l'un, ou ne garder qu'une declaration :\n{}", doubles.len(), doubles.join("\n"));
}

/// ON NE COUPE JAMAIS UN TEXTE : le CSS de la page (ses feuilles, et les `<style>` des pages que sert la fenetre) n'a ni
/// `text-overflow`, ni `line-clamp`, ni `overflow: hidden` ou `clip` (sur un axe ou les deux). Ce qui ne tient pas
/// defile (`overflow: auto`) ou passe a la ligne ; ce qui deborde se voit, et une phrase trop longue s'ecrit plus brève
/// dans le moteur.
#[test]
fn la_page_ne_coupe_aucun_texte() {
    let coupe = regex::Regex::new(r"(?i)text-overflow|line-clamp|overflow(?:-x|-y)?\s*:\s*(?:hidden|clip)").unwrap();
    let mut fautes = Vec::new();
    for p in fichiers_de_la_page() {
        let t = std::fs::read_to_string(&p).unwrap();
        let feuilles: Vec<(usize, String)> = if p.extension().is_some_and(|x| x == "css") {
            vec![(1, t.clone())]
        } else {
            blocs(&t, "<style", "</style>")
        };
        for (debut, f) in feuilles {
            for (n, l) in sans_commentaires_c(&f).lines().enumerate() {
                if coupe.is_match(l) {
                    let rel = p.canonicalize().unwrap().strip_prefix(depot()).unwrap().to_string_lossy().into_owned();
                    fautes.push(format!("{}:{} : {}", rel, debut + n, l.trim()));
                }
            }
        }
    }
    assert!(
        fautes.is_empty(),
        "{} regle(s) CSS coupent un texte : ce qui ne tient pas defile (`overflow: auto`) ou passe a la ligne, jamais ne se cache :\n{}",
        fautes.len(),
        fautes.join("\n")
    );
}

fn depot() -> PathBuf {
    espace().join("../..").canonicalize().unwrap()
}

/// Le CSS et le HTML de la page : les feuilles et `index.html` de `logiciel/interface/`, et les pages que sert la fenetre
/// (`moteur/src/interface/pages/`).
fn fichiers_de_la_page() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(depot().join("logiciel/interface")).unwrap().flatten().map(|e| e.path()).collect();
    v.extend(std::fs::read_dir(espace().join("moteur/src/interface/pages")).unwrap().flatten().map(|e| e.path()));
    v.retain(|p| p.extension().is_some_and(|x| x == "css" || x == "html"));
    v.sort();
    v
}

/// Ce qui est entre `ouvre` (jusqu'a son `>`) et `ferme`, avec la ligne du fichier ou chaque bloc commence.
fn blocs(t: &str, ouvre: &str, ferme: &str) -> Vec<(usize, String)> {
    let mut v = Vec::new();
    let mut reste = 0;
    while let Some(i) = t[reste..].find(ouvre) {
        let Some(j) = t[reste + i..].find('>') else { break };
        let debut = reste + i + j + 1;
        let fin = t[debut..].find(ferme).map_or(t.len(), |k| debut + k);
        v.push((t[..debut].matches('\n').count() + 1, t[debut..fin].to_string()));
        reste = fin;
    }
    v
}

/// Les commentaires `/* */` (et `//` si `ligne`, pour un script) d'un CSS ou d'un script, avec la ligne de chacun ; une
/// chaine (`'`, `"`, `` ` ``) n'en porte pas.
fn commentaires_c(t: &str, ligne: bool) -> Vec<(usize, String)> {
    let c: Vec<char> = t.chars().collect();
    let (mut v, mut i, mut n) = (Vec::new(), 0, 1);
    while i < c.len() {
        match c[i] {
            '\n' => n += 1,
            '/' if c.get(i + 1) == Some(&'*') => {
                let (debut, l) = (i, n);
                i += 2;
                while i + 1 < c.len() && !(c[i] == '*' && c[i + 1] == '/') {
                    n += (c[i] == '\n') as usize;
                    i += 1;
                }
                i += 1;
                v.push((l, c[debut..(i + 1).min(c.len())].iter().collect()));
            }
            '/' if ligne && c.get(i + 1) == Some(&'/') => {
                let debut = i;
                while i + 1 < c.len() && c[i + 1] != '\n' {
                    i += 1;
                }
                v.push((n, c[debut..=i].iter().collect()));
            }
            // Une expression reguliere d'un script (`/[&<>"]/g`) : apres un operateur ou une ouverture, pas une division.
            '/' if ligne && c[..i].iter().rev().find(|x| !x.is_whitespace()).is_none_or(|x| "(,=:[!&|?{};".contains(*x)) => {
                let mut classe = false;
                i += 1;
                while i < c.len() && c[i] != '\n' && (classe || c[i] != '/') {
                    match c[i] {
                        '\\' => i += 1,
                        '[' => classe = true,
                        ']' => classe = false,
                        _ => {}
                    }
                    i += 1;
                }
            }
            q @ ('\'' | '"' | '`') => {
                i += 1;
                while i < c.len() && c[i] != q {
                    i += (c[i] == '\\') as usize;
                    n += (c.get(i) == Some(&'\n')) as usize;
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    v
}

/// Un CSS sans ses commentaires, chaque fin de ligne gardee (les numeros de ligne restent justes).
fn sans_commentaires_c(t: &str) -> String {
    let mut s = t.to_string();
    for (_, c) in commentaires_c(t, false) {
        s = s.replacen(&c, &c.chars().filter(|x| *x == '\n').collect::<String>(), 1);
    }
    s
}

/// LA CARTE VALIDE A UNE VARIANTE PAR NATURE DE `contrats-base.json` (`droit::natures::NatureDeContrat`). Les motifs
/// de recours et de rupture du fichier sont ceux de leurs `enum` : `droit::natures` le verifie a la lecture.
#[test]
fn la_carte_valide_a_une_variante_par_nature() {
    use droit::natures::NatureDeContrat;
    let base: Value = serde_json::from_str(&std::fs::read_to_string(espace().join("../../logiciel/donnees/contrats-base.json")).unwrap()).unwrap();
    let ecrites: BTreeSet<&str> = base["natures"].as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(ecrites, NatureDeContrat::TOUTES.iter().map(|n| n.cle()).collect::<BTreeSet<_>>(), "une variante de la carte par nature du fichier");
}
