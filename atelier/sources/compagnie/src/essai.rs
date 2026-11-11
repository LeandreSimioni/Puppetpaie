//! La COMPAGNIE D'ESSAI : un vrai depot de TEST -- net-entreprises pour la DSN et ses signalements,
//! l'Urssaf pour la DPAE -- depuis une machine qui n'a pas `prive/` (une seance dans le nuage, le poste d'un autre
//! developpeur).
//!
//! `moteur essai monter [DOSSIER] [--depuis FICTIVE]` copie une compagnie FICTIVE livree (ses salaries inventes, ses paies,
//! son bareme, ses organismes) dans DOSSIER, hors de tout depot Git, et lui donne l'IDENTITE d'une vraie compagnie, lue dans
//! UNE variable d'environnement, `PUPPETPAIE_COMPAGNIE` : un objet JSON sur une ligne, fragment de `config/entreprise.json`,
//! avec EXACTEMENT les cles de sa forme (`Variable`). Le reste de la seance nomme cette compagnie par
//! `PUPPETPAIE_DONNEES=DOSSIER` et se sert des routes ordinaires (`moteur route` : `deposer` en test, la DSN ou la DPAE du mois ;
//! `ouverture` pour relever les retours).
//!
//! POURQUOI UNE COPIE, ET PAS LA VARIABLE LUE PARTOUT. Sur le poste, une variable posee par megarde ecraserait la vraie fiche ;
//! ici, la variable n'est lue qu'au montage, et seulement pour ECRIRE la fiche de la copie (regle 1 : rien ne se devine).
//!
//! CE QUI REND LE REEL IMPOSSIBLE PAR CE CHEMIN, TROIS FOIS :
//! - la copie porte le marqueur `COPIE-DE-DEVELOPPEMENT.json`, dont `copie_de` dit « compagnie d'essai » et NON
//!   « compagnie fictive » : ses depots de test vont donc au reseau (une fictive les joue), et la derniere porte avant le
//!   reseau (`depot_acte::refuser_si_copie`) refuse toute DSN ou DPAE reelle ;
//! - la copie n'a pas de coffre : `verifier_preuve` refuse un acte reel faute de mot de passe du logiciel (« Aucun mot de passe
//!   du logiciel n'est pose ») ;
//! - dans `moteur-atelier`, tout acte reel est refuse avant toute autre question.
//!
//! CE QUI NE S'AFFICHE JAMAIS : une valeur de la variable. Ses refus nomment la CLE (« `declarant_dsn.civilite` manque »,
//! « `siren` : 9 chiffres attendus »), jamais ce qu'elle contient ; le montage dit « posee », pas plus. Les fichiers ecrits
//! dans la copie (la fiche, les DSN, les DPAE) portent l'identite : ils restent dans DOSSIER, que le montage refuse de placer
//! dans un depot Git et qu'il ferme par un `.gitignore`.
//!
//! CE QU'ON PREND DE LA FICTIVE, ET CE QU'ON N'EN PREND PAS. Tout ce qui n'est pas l'identite vient d'elle : salaries (NIR
//! inventes, cles justes), contrats, paies, spectacles, `organismes.json`, `banque`, bareme, convention. On n'en prend ni
//! son marqueur, ni ses retours (ce sont ceux de SON SIREN), ni son horloge (elle ne joue que dans une fictive), ni ses PDF ;
//! de sa fiche, tout ce qui dit son identite est remplace ou retire (toutes ses valeurs datees : une ancienne pourrait redonner, a une
//! date passee, la raison sociale de la fictive). Les DSN deja ecrites sont gardees : celle d'un mois relit celles des mois
//! d'avant (numeros de contrat, regle 4). Le journal est garde SAUF ses depots de DSN : ceux-la ont ete joues sous le SIREN
//! de la fictive, la plate-forme ne les connait pas sous celui de la variable, et ils feraient refuser la DSN du mois
//! (« deja partie ») ou partir un essai a un ordre que rien ne justifie. Ses DPAE et ses contrats revenus signes restent :
//! ce sont eux qui disent qu'un mois peut se fermer (une compagnie d'essai ne depose aucune DPAE en reel).
//!
//! INFERENCES, A CONFIRMER AU PREMIER DEPOT DE TEST (son bilan fait foi, regle 10) : que la plate-forme d'essai accepte un
//! en-tete reel sur un contenu fictif (NIR inventes a cle juste, references d'adhesion et organismes de la fictive) ; que
//! la civilite, la qualite et l'adresse ne sont pas confrontees au repertoire SIRENE.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use socle::echec::{refus_avec, Echec, R};

/// La variable : l'identite de la compagnie, un objet JSON sur une ligne.
pub const VARIABLE: &str = "PUPPETPAIE_COMPAGNIE";

/// Le marqueur d'une copie (`commandes::compagnie`, `compagnie::installation`).
const MARQUEUR: &str = "COPIE-DE-DEVELOPPEMENT.json";

/// Le debut de `copie_de` d'une compagnie d'essai. Ne commence PAS par « compagnie fictive » (`compagnies::MARQUE_FICTIVE`) :
/// une fictive joue ses depots, une compagnie d'essai les fait en test, pour de vrai.
pub const MARQUE_ESSAI: &str = "compagnie d'essai";

/// La fictive copiee quand `--depuis` ne dit rien : la fictive de reference livree (`scenario::livrer`, que « Publier »
/// refait), dont les mois joues ont leurs DSN et leurs salaries.
pub const FICTIVE_PAR_DEFAUT: &str = "fictive-fil-de-fer";

/// Le nom du dossier quand la commande n'en donne pas : dans le dossier temporaire du systeme.
pub const DOSSIER_PAR_DEFAUT: &str = "puppetpaie-essai";


/// Ce que la copie ne prend pas de la fictive (relatif a sa racine).
const NON_COPIES: [&str; 3] = [MARQUEUR, envois::retours::RETOURS, crate::installation::FICHIER_HORLOGE];

/// Une compagnie d'essai : son marqueur le dit (`installation::copie_de`).
pub fn est_essai(racine: &Path) -> R<bool> {
    Ok(crate::installation::copie_de(racine)?.is_some_and(|c| c.starts_with(MARQUE_ESSAI)))
}

/// Ce qu'un refus porte a part : le chemin `p`.
fn a_part(p: &Path) -> socle::echec::Lieu {
    socle::echec::Lieu { chemin: Some(p.to_string_lossy().into_owned()), ..Default::default() }
}

// ------------------------------------------------------------------ l'identite

/// UN TEXTE DE LA VARIABLE, sans les blancs qui l'entourent. Ne s'affiche jamais (pas de `Debug`).
struct Lu(String);

impl<'de> Deserialize<'de> for Lu {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Lu, D::Error> {
        String::deserialize(d).map(|t| Lu(t.trim().to_string()))
    }
}

/// LA VARIABLE `PUPPETPAIE_COMPAGNIE`, telle qu'elle s'ecrit : exactement ces cles, chacune un texte. Une cle qui manque ou
/// qui est en trop est refusee : une faute de frappe ne passe pas en silence.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Variable {
    siren: Lu,
    nic: Lu,
    raison_sociale: Lu,
    adresse: Lu,
    code_postal: Lu,
    commune: Lu,
    naf: Lu,
    medecine_travail: Lu,
    declarant: VariableDeclarant,
    declarant_dsn: VariableContact,
    api_dpae: VariableCompteDpae,
}

/// Le declarant inscrit au service DSN de net-entreprises.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VariableDeclarant {
    nom: Lu,
    prenom: Lu,
}

/// La personne que les organismes contactent (S10.G00.02, S20.G00.07).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VariableContact {
    civilite: Lu,
    nom_prenom: Lu,
    tel: Lu,
    email: Lu,
    qualite: Lu,
}

/// Le compte DPAE d'urssaf.fr. Son SIRET et son code de service peuvent etre vides : le SIRET n'est alors pas dit (celui de
/// la compagnie sert, `Client::dpae`), le service non plus (facultatif dans l'adresse d'authentification,
/// `releve_retours::adresses_dpae`).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VariableCompteDpae {
    nom: Lu,
    prenom: Lu,
    siret: Lu,
    service: Lu,
}

/// L'identite lue dans la variable, validee : les cases de la fiche qu'elle pose. Ne s'affiche jamais (pas de `Debug`).
pub struct Identite(droit::fiche::ChampsEntreprise);

fn chiffres(t: &str, n: usize) -> bool {
    t.len() == n && t.bytes().all(|b| b.is_ascii_digit())
}

/// Le refus d'une valeur mal formee : la cle et ce qu'il faut, jamais la valeur.
#[track_caller]
fn mal_formee(cle: &str, ce_qu_il_faut: &str) -> Echec {
    refus_avec(
        format!("L'identité de la compagnie d'essai ({}) porte une valeur mal formée. Rien n'est écrit.", VARIABLE),
        socle::echec::Lieu { cle: Some(cle.to_string()), detail: Some(ce_qu_il_faut.to_string()), ..Default::default() },
    )
}

/// La case typee de `t`, ou le refus de sa forme (`droit::fiche::texte_borne!` : les zones de la DPAE), qui ne cite pas la
/// valeur : la phrase du type l'ouvre entre guillemets, elle en est retiree.
fn case<T>(cle: &str, lire: fn(&str) -> socle::echec::R<T>, t: &str) -> R<T> {
    lire(t).map_err(|f| mal_formee(cle, &f.phrase.replace(&format!("« {} »", t), "La valeur")))
}

/// Un texte qui peut etre vide : vide, il n'est pas dit.
fn dit(t: &Lu) -> Option<String> {
    (!t.0.is_empty()).then(|| t.0.clone())
}

impl Variable {
    /// Chaque texte de la variable, par sa cle.
    fn textes(&self) -> [(&'static str, &str); 19] {
        [
            ("siren", &self.siren.0),
            ("nic", &self.nic.0),
            ("raison_sociale", &self.raison_sociale.0),
            ("adresse", &self.adresse.0),
            ("code_postal", &self.code_postal.0),
            ("commune", &self.commune.0),
            ("naf", &self.naf.0),
            ("medecine_travail", &self.medecine_travail.0),
            ("declarant.nom", &self.declarant.nom.0),
            ("declarant.prenom", &self.declarant.prenom.0),
            ("declarant_dsn.civilite", &self.declarant_dsn.civilite.0),
            ("declarant_dsn.nom_prenom", &self.declarant_dsn.nom_prenom.0),
            ("declarant_dsn.tel", &self.declarant_dsn.tel.0),
            ("declarant_dsn.email", &self.declarant_dsn.email.0),
            ("declarant_dsn.qualite", &self.declarant_dsn.qualite.0),
            ("api_dpae.nom", &self.api_dpae.nom.0),
            ("api_dpae.prenom", &self.api_dpae.prenom.0),
            ("api_dpae.siret", &self.api_dpae.siret.0),
            ("api_dpae.service", &self.api_dpae.service.0),
        ]
    }

    /// Les rubriques de la DSN que chaque cle remplit (`dsn.rs` : l'en-tete S10.G00.01, le contact S10.G00.02 et
    /// S20.G00.07, l'entreprise S21.G00.06 et l'etablissement S21.G00.11).
    fn rubriques(&self) -> [(&'static str, &str, &'static [&'static str]); 12] {
        [
            ("siren", &self.siren.0, &["S10.G00.01.001", "S21.G00.06.001"]),
            ("nic", &self.nic.0, &["S10.G00.01.002", "S21.G00.06.002", "S21.G00.11.001"]),
            ("raison_sociale", &self.raison_sociale.0, &["S10.G00.01.003"]),
            ("adresse", &self.adresse.0, &["S10.G00.01.004", "S21.G00.06.004", "S21.G00.11.003"]),
            ("code_postal", &self.code_postal.0, &["S10.G00.01.005", "S21.G00.06.005", "S21.G00.11.004"]),
            ("commune", &self.commune.0, &["S10.G00.01.006", "S21.G00.06.006", "S21.G00.11.005"]),
            ("naf", &self.naf.0, &["S21.G00.06.003", "S21.G00.11.002"]),
            ("declarant_dsn.civilite", &self.declarant_dsn.civilite.0, &["S10.G00.02.001"]),
            ("declarant_dsn.nom_prenom", &self.declarant_dsn.nom_prenom.0, &["S10.G00.02.002", "S20.G00.07.001"]),
            ("declarant_dsn.email", &self.declarant_dsn.email.0, &["S10.G00.02.004", "S20.G00.07.003"]),
            ("declarant_dsn.tel", &self.declarant_dsn.tel.0, &["S10.G00.02.005", "S20.G00.07.002"]),
            ("declarant_dsn.qualite", &self.declarant_dsn.qualite.0, &["S20.G00.07.004"]),
        ]
    }

    /// Les formes que la norme et le moteur exigent, verifiees AVANT d'ecrire : un refus plus tard (a l'ecriture de la DSN,
    /// au depot) citerait la valeur fautive a l'ecran.
    fn verifier(&self) -> R<()> {
        let vides: Vec<&str> = self.textes().into_iter().filter(|(cle, t)| t.is_empty() && !matches!(*cle, "api_dpae.siret" | "api_dpae.service")).map(|(cle, _)| cle).collect();
        if !vides.is_empty() {
            return Err(refus_avec(
                format!("L'identité de la compagnie d'essai ({}) a des valeurs vides. Rien n'est écrit.", VARIABLE),
                socle::echec::Lieu { cle: Some(vides.join(", ")), ..Default::default() },
            ));
        }
        let (siren, nic, siret_dpae) = (&self.siren.0, &self.nic.0, &self.api_dpae.siret.0);
        if !chiffres(siren, 9) || !declarations::champs::luhn(siren) {
            return Err(mal_formee("siren", "9 chiffres attendus, clé de Luhn juste"));
        }
        if !chiffres(nic, 5) || !declarations::champs::luhn(&format!("{}{}", siren, nic)) {
            return Err(mal_formee("nic", "5 chiffres attendus, et le SIRET (siren + nic) à clé de Luhn juste"));
        }
        if !siret_dpae.is_empty() && (!chiffres(siret_dpae, 14) || !declarations::champs::luhn(siret_dpae)) {
            return Err(mal_formee("api_dpae.siret", "14 chiffres attendus, clé de Luhn juste (ou vide : le SIRET de la compagnie)"));
        }
        if !chiffres(&self.code_postal.0, 5) {
            return Err(mal_formee("code_postal", "5 chiffres attendus"));
        }
        // Chaque rubrique de la DSN que la cle remplit, a son format (longueur, expression : `formats-dsn-*.json`, relevees
        // du cahier technique) : la DSN refuserait plus tard EN CITANT la valeur. La qualite, par exemple, est un CODE de
        // deux caracteres (S20.G00.07.004, « Type » du contact : 04 pour les cotisations de Securite sociale ; cahier
        // technique 2026.1, p. 139, `norme-dsn/texte/bloc-S20.G00.07.txt`), pas un intitule.
        for (cle, t, rubriques) in self.rubriques() {
            for r in rubriques {
                let pourquoi = declarations::formats::mauvaise(r, t)?;
                if !pourquoi.is_empty() {
                    return Err(mal_formee(cle, &format!("ne tient pas dans {} de la DSN : {}", r, pourquoi)));
                }
            }
        }
        // La DUE MtoM refuse les accents (`depot::identifiants`, qui citerait la valeur : on refuse ici, avant).
        for (k, t) in [("api_dpae.nom", &self.api_dpae.nom.0), ("api_dpae.prenom", &self.api_dpae.prenom.0)] {
            if *t != socle::texte::ascii_nfkd(t) {
                return Err(mal_formee(k, "sans accent, tel que le compte DPAE est ouvert sur urssaf.fr (la DUE MtoM refuse les accents)"));
            }
        }
        Ok(())
    }

    /// Les cases de la fiche que la variable pose, et ce qui en derive (le SIRET, le declarant net-entreprises, le compte
    /// DPAE), chacune en son type.
    fn champs(&self) -> R<droit::fiche::ChampsEntreprise> {
        use droit::fiche::{ApiDpae, ChampsEntreprise, Civilite, CodeMedecine, CodeNaf, CodePostal, CommuneEmployeur, Declarant, DeclarantDsn, LigneAdresse, RaisonSociale, Telephone};
        let siret = format!("{}{}", self.siren.0, self.nic.0);
        let civilite = match self.declarant_dsn.civilite.0.as_str() {
            "01" => Civilite::Monsieur,
            "02" => Civilite::Madame,
            _ => return Err(mal_formee("declarant_dsn.civilite", "01 (Monsieur) ou 02 (Madame), cahier technique DSN, S10.G00.02.001")),
        };
        Ok(ChampsEntreprise {
            siren: Some(self.siren.0.clone()),
            nic: Some(self.nic.0.clone()),
            siret: Some(siret.clone()),
            nom_affichage: Some("Compagnie d'essai".to_string()),
            raison_sociale: Some(case("raison_sociale", RaisonSociale::lire, &self.raison_sociale.0)?),
            adresse: Some(case("adresse", LigneAdresse::lire, &self.adresse.0)?),
            code_postal: Some(case("code_postal", CodePostal::lire, &self.code_postal.0)?),
            commune: Some(case("commune", CommuneEmployeur::lire, &self.commune.0)?),
            naf: Some(case("naf", CodeNaf::lire, &self.naf.0)?),
            medecine_travail: Some(case("medecine_travail", CodeMedecine::lire, &self.medecine_travail.0)?),
            // Le declarant inscrit au service DSN, tel qu'inscrit : `transport::declarant` le prend tel quel (sans couper
            // `nom_prenom` au premier espace, faux pour un nom compose).
            declarant: Some(Declarant { siret: Some(siret), nom: Some(self.declarant.nom.0.clone()), prenom: Some(self.declarant.prenom.0.clone()) }),
            declarant_dsn: Some(DeclarantDsn {
                civilite: Some(civilite),
                nom_prenom_affichage: None,
                nom_prenom: Some(self.declarant_dsn.nom_prenom.0.clone()),
                tel: Some(case("declarant_dsn.tel", Telephone::lire, &self.declarant_dsn.tel.0)?),
                email: Some(self.declarant_dsn.email.0.clone()),
                qualite: Some(self.declarant_dsn.qualite.0.clone()),
            }),
            api_dpae: Some(ApiDpae {
                nom: Some(self.api_dpae.nom.0.clone()),
                prenom: Some(self.api_dpae.prenom.0.clone()),
                siret: dit(&self.api_dpae.siret),
                service: dit(&self.api_dpae.service),
            }),
            note: Some(format!("COMPAGNIE D'ESSAI (moteur essai monter) : l'identite vient de {} ; le reste, de la compagnie fictive copiee. Ne se commite jamais.", VARIABLE)),
            ..ChampsEntreprise::default()
        })
    }
}

impl Identite {
    /// Lit et valide le texte de la variable, par la lecture qui ne cite aucune valeur (`socle::forme::lire_texte_sans_valeur`) :
    /// le refus nomme la variable et la cle, JAMAIS une valeur.
    pub fn lire(texte: &str) -> R<Identite> {
        let v: Variable = socle::forme::lire_texte_sans_valeur(texte, &format!("L'identité de la compagnie d'essai ({}) ne se lit pas : un objet sur une ligne, avec exactement ses clés, est attendu. Rien n'est écrit.", VARIABLE))?;
        v.verifier()?;
        Ok(Identite(v.champs()?))
    }

    /// La fiche de la fictive, sous cette identite : les cles de la variable posees, ce qui en derive recompose (le SIRET, le
    /// declarant net-entreprises, le compte DPAE), ce qui disait l'identite de la fictive retire. Chaque valeur posee vaut
    /// de tout temps, seule : une valeur datee de l'ancienne identite la redonnerait a une date passee.
    pub fn appliquer(&self, fiche: &droit::fiche::FicheEntrepriseDatee) -> R<droit::fiche::FicheEntrepriseDatee> {
        let mut brute = fiche.clone();
        // Ce que la fiche de la fictive dit de SON identite, et qui ne survit pas sous celle de la variable : ce que la variable
        // remplace, et ce qui en derive (l'affichage, le complement d'adresse).
        for liste in [&mut brute.siret, &mut brute.nom_affichage, &mut brute.adresse_affichage, &mut brute.commune_affichage] {
            *liste = None;
        }
        brute.complement_adresse = None;
        brute.etablissements = None;
        brute.note = None;
        brute.par_dessus(&<droit::fiche::FicheEntrepriseDatee as droit::fiche::FicheDatee>::neuve(&self.0));
        Ok(brute)
    }
}

// ------------------------------------------------------------------ le montage

/// Ce que le montage a fait (des chemins et des noms ; aucune valeur de la variable).
#[derive(Debug)]
pub struct Montage {
    pub dossier: PathBuf,
    /// Le nom de la fictive copiee.
    pub fictive: String,
    /// `false` : la copie existait deja, seule son identite a ete reecrite.
    pub neuf: bool,
}

/// Le dossier d'installation : celui qui porte `logiciel/` (`logiciel/donnees` -> `logiciel` -> l'installation).
fn installation() -> R<PathBuf> {
    let code = socle::fichiers::dossier_du_code();
    code.parent().and_then(Path::parent).map(Path::to_path_buf).ok_or_else(|| refus_avec("Le logiciel n'est dans aucune installation.", a_part(&code)))
}

/// La fictive a copier : un chemin (s'il designe un dossier), sinon un nom de `fictives/` de l'installation.
fn source(fictive: &str) -> R<PathBuf> {
    let p = PathBuf::from(fictive);
    let d = if fictive.contains('/') || fictive.contains('\\') || p.is_dir() { p } else { installation()?.join(crate::installation::DOSSIER_FICTIVES).join(fictive) };
    if !d.join(droit::contexte::ENTREPRISE).is_file() {
        return Err(refus_avec("Ce dossier n'est pas une compagnie : il n'a pas de fiche entreprise. Rien n'est écrit.", a_part(&d)));
    }
    // Seulement une FICTIVE : jamais `prive/` ni une autre compagnie reelle, dont les salaries partiraient sous un autre SIREN.
    if !crate::installation::est_fictive_livree(&d)? {
        return Err(refus_avec("Ce dossier n'est pas une compagnie fictive : une compagnie d'essai ne se monte que depuis une fictive. Rien n'est écrit.", a_part(&d)));
    }
    Ok(d)
}

/// Le chemin absolu, sans `..`, meme s'il n'existe pas encore (on remonte au premier ancetre qui existe).
fn absolu(p: &Path) -> R<PathBuf> {
    let p = if p.is_absolute() { p.to_path_buf() } else { std::env::current_dir().map_err(|e| socle::echec::du_disque(p, &e))?.join(p) };
    let mut reste = Vec::new();
    let mut a = p.as_path();
    loop {
        match a.canonicalize() {
            Ok(mut c) => {
                for r in reste.iter().rev() {
                    c.push(r);
                }
                return Ok(c);
            }
            // Ce qui n'existe pas encore se resout par son parent.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(socle::echec::du_disque(a, &e)),
        }
        match (a.file_name(), a.parent()) {
            (Some(n), Some(parent)) => {
                reste.push(n.to_os_string());
                a = parent;
            }
            _ => return Ok(p),
        }
    }
}

/// Le depot Git ou l'installation qui contient `dossier`, s'il y en a un : la copie porterait l'identite d'une vraie compagnie,
/// elle ne va jamais la ou un `git add` la prendrait, ni a cote de ce que les gens recoivent.
fn depot_englobant(dossier: &Path) -> R<Option<PathBuf>> {
    Ok(absolu(dossier)?.ancestors().find(|a| a.join(".git").exists() || a.join("logiciel").join("donnees").join("echeancier-base.json").is_file()).map(Path::to_path_buf))
}

fn copier(src: &Path, dst: &Path, racine: &Path) -> R<()> {
    std::fs::create_dir_all(dst).map_err(|e| socle::echec::du_disque(&dst, &e))?;
    for e in std::fs::read_dir(src).map_err(|e| socle::echec::du_disque(&src, &e))? {
        let e = e.map_err(|e| socle::echec::du_disque(&src, &e))?;
        let s = e.path();
        let rel = s.strip_prefix(racine).map_err(|e| socle::echec::refus(e.to_string()))?.to_string_lossy().replace('\\', "/");
        if NON_COPIES.contains(&rel.as_str()) || s.extension().is_some_and(|x| x.eq_ignore_ascii_case("pdf")) {
            continue;
        }
        let d = dst.join(e.file_name());
        if s.is_dir() {
            copier(&s, &d, racine)?;
        } else {
            std::fs::copy(&s, &d).map_err(|x| socle::echec::du_disque(&s, &x))?;
        }
    }
    Ok(())
}

/// Le journal de la copie sans les depots de DSN de la fictive (mensuelles et signalements, essais et reels) : voir l'en-tete.
fn retirer_les_depots_de_dsn(dossier: &Path) -> R<()> {
    envois::journal::ne_garder_que(dossier, |l| l.acte() != envois::journal::Acte::Dsn)
}

/// Monte (ou remet a jour) la compagnie d'essai dans `dossier`, depuis la fictive `fictive`, sous l'identite `texte` (le
/// contenu de la variable, `None` si elle est absente). Tout se verifie AVANT d'ecrire : un refus ne laisse rien.
///
/// Une copie qui existe deja et qui est une compagnie d'essai garde tout (journal, DSN, retours releves) : seule sa fiche
/// reprend l'identite de la variable. Un dossier qui existe et qui n'en est pas une, s'il n'est pas vide, est refuse.
pub fn monter(dossier: &Path, fictive: &str, texte: Option<&str>) -> R<Montage> {
    let texte = texte.filter(|t| !t.trim().is_empty()).ok_or_else(|| {
        refus_avec(
            format!("{} absente : elle porte l'identite de la compagnie (un objet JSON sur une ligne). La poser dans les reglages de l'environnement, puis ouvrir une nouvelle seance. Rien n'est ecrit.", VARIABLE),
            socle::echec::Lieu { detail: Some("ses cles : moteur essai monter --aide".to_string()), ..Default::default() },
        )
    })?;
    let id = Identite::lire(texte)?;
    if let Some(d) = depot_englobant(dossier)? {
        return Err(refus_avec(
            "Ce dossier est dans un dépôt Git ou une installation de PuppetPaie : la compagnie d'essai porte l'identité d'une vraie compagnie, elle se monte hors de tout dépôt (par exemple dans le dossier temporaire, le défaut). Rien n'est écrit.",
            socle::echec::Lieu { chemin: Some(dossier.to_string_lossy().into_owned()), detail: Some(format!("dans {}", d.to_string_lossy())), ..Default::default() },
        ));
    }
    let existe = dossier.exists();
    let deja = existe && est_essai(dossier)?;
    if existe && !deja {
        let plein = std::fs::read_dir(dossier).map_err(|e| socle::echec::du_disque(dossier, &e))?.next().is_some();
        if plein {
            return Err(refus_avec("Ce dossier existe et n'est pas une compagnie d'essai : choisir un dossier vide ou neuf. Rien n'est écrit.", a_part(dossier)));
        }
    }
    let (src, nom) = if deja {
        // La copie dit de quelle fictive elle vient ; `--depuis` ne la change pas (il faudrait une copie neuve).
        let copiee = crate::installation::marqueur(dossier)?
            .and_then(|m| m.fictive)
            .ok_or_else(|| refus_avec("Le marqueur de la compagnie d'essai ne dit pas de quelle fictive elle vient.", a_part(dossier)))?;
        (None, copiee)
    } else {
        let s = source(fictive)?;
        let nom = s.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or_else(|| refus_avec("La fictive à copier n'a pas de nom.", a_part(&s)))?;
        (Some(s), nom)
    };
    // La fiche sous la nouvelle identite, calculee AVANT toute ecriture.
    let base_fiche = match &src {
        Some(s) => s.join(droit::contexte::ENTREPRISE),
        None => dossier.join(droit::contexte::ENTREPRISE),
    };
    let fiche: droit::fiche::FicheEntrepriseDatee = droit::fiche::lire_fichier(&base_fiche, droit::contexte::ENTREPRISE)?;
    let fiche = id.appliquer(&fiche)?;

    if let Some(s) = &src {
        copier(s, dossier, s)?;
        retirer_les_depots_de_dsn(dossier)?;
        let quand = socle::paris::InstantEcrit::maintenant();
        let marqueur = crate::installation::Marqueur {
            copie_de: format!("{} (moteur essai monter, identite de {}), depuis la compagnie fictive {}", MARQUE_ESSAI, VARIABLE, nom),
            fictive: Some(nom.clone()),
            ajoutee: None,
            quand: Some(quand),
            quoi: "Compagnie d'ESSAI : ses depots de TEST vont au reseau, sous l'identite d'une vraie compagnie ; tout acte reel est refuse (depot_acte::refuser_si_copie, et pas de coffre). Ne se commite jamais.".to_string(),
        };
        socle::ecriture::ecrire_json(&dossier.join(MARQUEUR), &marqueur, 2)?;
        std::fs::write(dossier.join(".gitignore"), "# Compagnie d'essai : l'identite d'une vraie compagnie. Rien ici ne se commite.\n*\n")
            .map_err(|e| socle::echec::du_disque(&dossier, &e))?;
    }
    socle::ecriture::ecrire_json(&dossier.join(droit::contexte::ENTREPRISE), &fiche, 2)?;
    Ok(Montage { dossier: absolu(dossier)?, fictive: nom, neuf: !deja })
}

// ------------------------------------------------------------------ la commande

const AIDE: &str = "usage : moteur essai monter [DOSSIER] [--depuis FICTIVE]

Monte une COMPAGNIE D'ESSAI : la copie d'une compagnie fictive (defaut : fictive-fil-de-fer, dans fictives/ de
l'installation ; ou un chemin), sous l'identite d'une vraie compagnie lue dans la variable PUPPETPAIE_COMPAGNIE.
Ses depots de TEST vont au reseau (net-entreprises, Urssaf) ; tout depot reel y est refuse.
DOSSIER : hors de tout depot Git (defaut : <dossier temporaire>/puppetpaie-essai). Deja monte, seule l'identite
est reecrite (journal, DSN et retours gardes).

PUPPETPAIE_COMPAGNIE : un objet JSON sur une ligne, avec exactement ces cles (ses valeurs ne s'affichent jamais) :
  siren, nic, raison_sociale, adresse, code_postal, commune, naf, medecine_travail,
  declarant      {nom, prenom}                              (inscrit au service DSN de net-entreprises)
  declarant_dsn  {civilite, nom_prenom, tel, email, qualite} (la personne que les organismes contactent ;
                 civilite 01 ou 02, qualite : le code du type de contact, S20.G00.07.004, par exemple 04)
  api_dpae       {nom, prenom, siret, service}               (le compte DPAE d'urssaf.fr ; siret et service peuvent etre vides)
Les mots de passe restent dans NET_ENTREPRISES_MOTDEPASSE et URSSAF_MOTDEPASSE.

Puis, avec PUPPETPAIE_DONNEES=DOSSIER, par moteur route : deposer (quoi dsn ou dpae, mois AAAA-MM), en test ; ouverture, les retours
(la suite : atelier/docs/COMPAGNIE-D-ESSAI.md).";

/// `moteur essai ...` : rend le code de sortie.
pub fn commande(args: &[String]) -> i32 {
    let mut it = args.iter();
    match it.next().map(String::as_str) {
        Some("monter") => {}
        Some("-h" | "--help" | "--aide" | "aide") => {
            dire!("{}", AIDE);
            return 0;
        }
        _ => {
            eprintln!("{}", AIDE);
            return 2;
        }
    }
    let (mut dossier, mut fictive): (Option<PathBuf>, String) = (None, FICTIVE_PAR_DEFAUT.to_string());
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" | "--aide" => {
                dire!("{}", AIDE);
                return 0;
            }
            "--depuis" => match it.next() {
                Some(v) => fictive = v.clone(),
                None => {
                    eprintln!("moteur essai monter : --depuis attend une fictive.\n\n{}", AIDE);
                    return 2;
                }
            },
            x if x.starts_with("--") => {
                eprintln!("moteur essai monter : option inconnue : {}\n\n{}", x, AIDE);
                return 2;
            }
            x if dossier.is_none() => dossier = Some(PathBuf::from(x)),
            x => {
                eprintln!("moteur essai monter : un seul dossier ({} en trop).\n\n{}", x, AIDE);
                return 2;
            }
        }
    }
    crate::installation::fixer_le_code();
    // Sans dossier dit : dans le dossier temporaire du systeme.
    let dossier = match dossier {
        Some(d) => d,
        None => std::env::temp_dir().join(DOSSIER_PAR_DEFAUT),
    };
    let texte = match std::env::var(VARIABLE) {
        Ok(t) => Some(t),
        Err(std::env::VarError::NotPresent) => None,
        Err(std::env::VarError::NotUnicode(_)) => {
            eprintln!("{} n'est pas un texte lisible : un objet JSON sur une ligne est attendu. Rien n'est ecrit.", VARIABLE);
            return 1;
        }
    };
    match monter(&dossier, &fictive, texte.as_deref()) {
        Ok(m) => {
            let d = m.dossier.display();
            if m.neuf {
                dire!("Compagnie d'essai montee dans {}, depuis la compagnie fictive {}.", d, m.fictive);
            } else {
                dire!("Compagnie d'essai {} (depuis {}) : identite reecrite ; journal, DSN et retours gardes.", d, m.fictive);
            }
            dire!("Identite : {} (posee ; ses valeurs ne s'affichent pas). Tout depot reel y est refuse.", VARIABLE);
            dire!();
            dire!("La suite, avec PUPPETPAIE_DONNEES={} :", d);
            dire!("  echo '{{\"route\":\"deposer\",\"charge\":{{\"quoi\":\"dsn\",\"mois\":\"AAAA-MM\"}}}}' | moteur route --texte");
            dire!("  puis les retours : echo '{{\"route\":\"ouverture\",\"charge\":{{\"sauvegarder\":false,\"signatures\":false}}}}' | moteur route --texte");
            dire!("(atelier/docs/COMPAGNIE-D-ESSAI.md : l'ordre d'un essai deja fait d'ailleurs, les signalements)");
            0
        }
        Err(m) => {
            eprintln!("{}", m.en_lignes());
            1
        }
    }
}
