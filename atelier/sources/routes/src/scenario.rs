//! LE SCENARIO D'UNE COMPAGNIE : ce qu'on a fait dans l'interface, geste par geste, et son rejeu sans
//! navigateur.
//!
//! UN SCENARIO EST UN DOSSIER :
//! - `gestes.jsonl` : un geste par ligne, dans l'ordre ou ils ont lieu. Les gestes :
//!   - `{"geste": "debut", "jour", "logiciel"}` : le banc commence (rien a rejouer) ;
//!   - `{"geste": "ajouter", "nom", "venue"}` : le « + » de l'accueil d'un logiciel bride (`compagnies::ajouter_venant` ;
//!     `venue` : `logiciel_precedent` quand la compagnie vient d'un logiciel precedent, absente sinon), le decor
//!     de depart ; le banc y note `compagnie`, le dossier qu'il a cree, relatif a l'installation (`fictives/ajoutee-…`),
//!     et le rejeu verifie qu'il cree le meme ;
//!   - `{"geste": "simuler"}` : le geste d'atelier qui fait d'une fictive ajoutee une fictive aux envois simules
//!     (`rendre_simulee`) -- sans lui, ses envois sont refuses et elle ne peut pas vivre sa vie ;
//!   - `{"geste": "annoncer", "annonce"}` : ce que les organismes de la fictive repondront a partir de ce jour (un taux de
//!     PAS, un taux AT/MP, un refus : `organismes_simules::annoncer`) ;
//!   - `{"geste": "jour", "jour", "veille"}` : le calendrier avance, et chaque compagnie du scenario s'ouvre ce jour-la :
//!     son horloge joue ce qui etait du (`horloge::rattraper`, comme a l'ouverture). `veille` (le banc, quand une
//!     compagnie est ouverte) : sa veille passe, et son horloge joue dans la route `ouverture` qui suit, pas ici ;
//!   - `{"geste": "etat", "cle"}` : l'etat d'un mois (ou de l'ouverture) que la fenetre a refait en fond et qui a ecrit
//!     (`Enregistreur::fond`) ; le rejeu le refait (`etat_garde::lire`). `ok` (vrai par defaut) : il a reussi, et le rejeu
//!     le verifie (un etat peut ecrire, puis refuser) ;
//!   - `{"route", "charge"}` : une route du moteur, telle que la fenetre l'a servie (la page, la veille, le fil de fond :
//!     `interface::api::route`), avec la charge que le moteur a recue. `ecrit` (vrai par defaut) : la route a change un
//!     fichier de la compagnie -- le banc le note, et le rejeu ecarte les routes qui n'ont rien ecrit ; `ok` (vrai par
//!     defaut) : elle a reussi, et le rejeu le verifie. `pieces` : ce qu'on avait depose dans un dossier que la route lit
//!     (l'import), par cle de la charge : `{"chemin"}` (un fichier ou un dossier du scenario, nomme relativement a lui),
//!     ou `{"fichiers": {rel: empreinte}}` (ce que le banc a recopie dans `pieces/`) ; `dans_la_compagnie` pose la piece
//!     a cet endroit de la compagnie au lieu d'un dossier de travail.
//!   Chaque geste a son instant : `quand` (secondes UTC, ce que le banc ecrit), ou `heure`, a Paris (`AAAA-MM-JJ`, a 9 h,
//!   ou `AAAA-MM-JJTHH:MM[:SS]`) ; un geste `jour` sans l'un ni l'autre a lieu ce jour-la a 9 h, et tout autre geste sans
//!   date, une seconde apres celui qui le precede. Les instants ne reculent jamais. `compagnie` (le dossier relatif a
//!   l'installation) se tait quand le scenario n'en ajoute qu'une : c'est la derniere ajoutee. `n`, le rang que le banc
//!   ecrit, ne sert qu'a le relire.
//! - `pieces/` : le contenu de chaque fichier depose -- par empreinte (`pieces/<sha256>`, le banc : un PDF de bulletins,
//!   une DSN, un contrat...), ou sous le nom qu'un scenario ecrit a la main lui donne. Le scenario se rejoue ainsi sans
//!   rien d'exterieur : ni `/tmp`, ni le poste ou on l'a enregistre.
//!
//! L'ENREGISTREUR (`Enregistreur`) n'existe que dans le binaire de l'atelier (`moteur-atelier`, la feature `atelier`) :
//! `moteur-atelier interface --enregistrer <dossier> [--lectures] --jour J` (le banc, `atelier/outils/banc/`). Il garde
//! les routes dont la fiche ecrit ou engage (`routes::est_une_ecriture`) ; une lecture n'y entre qu'avec `--lectures`,
//! et sans lui elle est servie sans attendre le tour des autres. Le binaire livre n'en a pas une ligne.
//!
//! LE REJEU (`rejouer`) est dans le moteur ordinaire : les essais (`tests/outils/rejeu.rs`) et l'exemple `vie_fictive`
//! l'appellent dans leur processus. Chaque geste se rejoue a son instant, fige (`temps::figer`), les aleas en suite fixe
//! (`temps::aleas_fixes`) : deux rejeux ecrivent les memes octets. Les routes qui n'ont rien ecrit s'ecartent par defaut
//! (une lecture ne change pas la compagnie ; les garder coute le temps de les recalculer) ; `OptionsRejeu::lectures` les
//! rejoue aussi. `rejouer_et_lire` s'arrete a des jours choisis pour qu'on lise la compagnie telle qu'elle est ce
//! soir-la, puis reprend : un seul rejeu pour plusieurs dates.
//!
//! LES FICTIVES DE REFERENCE (`DOSSIER_REFERENCE`) sont des scenarios ecrits, non enregistres : leur rejeu est la preuve
//! commune, et `livrer` pose la compagnie qui en sort dans `fictives/`, ce que « Publier » livre.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use socle::date::Jour;
use socle::echec::{refus, R};
use socle::paris::{InstantEcrit, MinuteEcrite, Moment};
use socle::refus;

use compagnie::installation::Venue;
use crate::catalogue::Route;
use crate::enveloppe::{Charge, Options, Refus, Reponse};

/// Le fichier des gestes, dans le dossier du scenario.
pub const GESTES: &str = "gestes.jsonl";
/// Le dossier des pieces deposees, par empreinte.
pub const PIECES: &str = "pieces";
/// La version du format.
pub const FORMAT: u64 = 1;

/// UN GESTE, une ligne de `gestes.jsonl` : une route servie a la porte (`route`, sa `charge`), ou un geste du banc
/// (`geste`). Ce qu'un geste ne porte pas ne s'ecrit pas.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Geste {
    /// Le rang que le banc ecrit, pour relire le scenario.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u64>,
    /// L'instant, en secondes UTC (le banc).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quand: Option<Moment>,
    /// L'instant a Paris, ecrit a la main ou par le banc pour qui le relit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heure: Option<HeureDuGeste>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geste: Option<Genre>,
    /// La route, par son nom : un nom qu'aucune route ne porte se rejoue comme le moteur le refuse.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<String>,
    /// Le nom de la compagnie que le « + » ajoute (`ajouter`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nom: Option<String>,
    /// D'ou vient la compagnie ajoutee : absente, une compagnie neuve.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub venue: Option<Venue>,
    /// La compagnie, relative a l'installation ; absente, la derniere ajoutee.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compagnie: Option<String>,
    /// L'etat refait (`etat`) : un mois, ou l'ouverture.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub charge: Option<Charge>,
    /// Ce qu'on avait depose dans un dossier que la route lit, par cle de la charge.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pieces: Option<BTreeMap<String, Piece>>,
    /// La route a-t-elle change un fichier de la compagnie ? Absent : oui.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ecrit: Option<bool>,
    /// A-t-elle reussi (une route, un etat) ? Absent : oui.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ok: Option<bool>,
    /// La phrase de son refus, pour qui relit le scenario (le rejeu ne compare que `ok`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refus: Option<String>,
    /// Le jour (`debut`, `jour`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jour: Option<Jour>,
    /// La version du logiciel qui a commence le banc (`debut`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logiciel: Option<String>,
    /// Le format du scenario (`debut`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<u64>,
    /// Ce que les organismes de la fictive repondront (`annoncer`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annonce: Option<envois::organismes_simules::Annonce>,
    /// La compagnie ouverte a passe sa veille ce jour-la (`jour`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub veille: Option<bool>,
    /// Ce qu'est le geste, pour qui lit le scenario.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quoi: Option<String>,
}

/// LES GESTES DU BANC, qui ne sont pas des routes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Genre {
    Debut,
    Ajouter,
    Simuler,
    Annoncer,
    Jour,
    Etat,
}

impl Geste {
    /// Le geste du banc `genre`, sans rien d'autre.
    pub fn du_banc(genre: Genre) -> Geste {
        Geste { geste: Some(genre), ..Geste::default() }
    }

    /// Le geste a-t-il ecrit (absent : oui) ?
    fn a_ecrit(&self) -> bool {
        self.ecrit != Some(false)
    }

    /// A-t-il reussi (absent : oui) ?
    fn a_reussi(&self) -> bool {
        self.ok != Some(false)
    }
}

/// UNE PIECE DEPOSEE : un fichier ou un dossier du scenario (`chemin`, relatif a lui), ou ce que le banc a recopie dans
/// `pieces/` (`fichiers` : relatif a la piece -> empreinte ; un fichier seul a la cle `.`, `nom` son nom). `dans_la_compagnie`
/// la pose a cet endroit de la compagnie au lieu d'un dossier de travail.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Piece {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chemin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fichier: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nom: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dans_la_compagnie: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fichiers: Option<BTreeMap<String, String>>,
}

/// Les cles d'une charge qui nomment un dossier ou un fichier que la route LIT (une boite aux lettres d'import, un export
/// d'un autre logiciel) : ce qu'il contient entre dans le scenario.
#[cfg(feature = "atelier")]
const CLES_DE_DEPOT: [&str; 4] = ["dossier", "export", "fichier", "chemin"];

/// Les routes dont la cle `dossier` vide (ou absente) veut dire `import/` de la compagnie (`crate::import::dossier_import`).
#[cfg(feature = "atelier")]
fn lit_la_boite_aux_lettres(route: Route) -> bool {
    route == Route::Importer || route.nom().starts_with("importation")
}

/// LE MARQUEUR D'UNE FICTIVE AUX ENVOIS SIMULES, pose sur une fictive ajoutee (le geste `simuler`) : `copie_de` dit
/// « compagnie fictive … » (`compagnies::est_fictive`) sans la marque `ajoutee` -- ses envois sont joues
/// (`bridage::branchement`) et son horloge joue ce qui etait du (`horloge`). Le nom (« … sur le banc
/// d'essai ») n'est celui d'aucune fictive livree : la livraison des fictives (`compagnie::maj`) ne la prend jamais pour une
/// des siennes. Refuse hors d'une fictive ajoutee.
pub fn rendre_simulee(racine: &Path) -> R<()> {
    if !compagnie::installation::est_ajoutee(racine)? {
        return Err(refus!("{} n'est pas une compagnie ajoutee par le « + » : rien a changer.", racine.display()));
    }
    let nom = racine.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or_else(|| refus!("{} : un dossier est attendu", racine.display()))?;
    let marqueur = compagnie::installation::Marqueur {
        copie_de: format!("{} {} sur le banc d'essai", compagnie::installation::MARQUE_FICTIVE, nom),
        fictive: None,
        ajoutee: None,
        quand: Some(socle::paris::InstantEcrit::maintenant()),
        quoi: "Une compagnie ajoutee par le « + », devenue fictive aux envois SIMULES sur le banc d'essai de l'atelier (scenario.rs, geste `simuler`) : ses DPAE, DSN et courriels sont joues, rien ne part.".to_string(),
    };
    socle::ecriture::ecrire_json(&racine.join(compagnie::installation::MARQUEUR), &marqueur, 2)
}

/// Le chemin de `racine` relatif a l'installation `inst` (barres droites), ou le chemin entier s'il n'y est pas. L'un et
/// l'autre existent : un dossier qui ne se resout pas refuse.
pub fn relatif(inst: &Path, racine: &Path) -> R<String> {
    let resolu = |p: &Path| std::fs::canonicalize(p).map_err(|e| refus!("{} : {}", p.display(), e));
    let (i, r) = (resolu(inst)?, resolu(racine)?);
    Ok(match r.strip_prefix(&i) {
        Ok(p) => p.to_string_lossy().replace('\\', "/"),
        Err(hors) => {
            drop(hors);
            r.display().to_string()
        }
    })
}

/// L'heure lisible d'un instant, a Paris.
fn heure(t: Moment) -> String {
    InstantEcrit::de(t).iso()
}

/// L'INSTANT D'UN GESTE A PARIS, tel que le scenario l'ecrit : un jour (`AAAA-MM-JJ`, a 9 h), une minute
/// (`AAAA-MM-JJTHH:MM`) ou une seconde (`AAAA-MM-JJTHH:MM:SS`, ce que le banc ecrit). Il se lit et s'ecrit dans l'une de
/// ces trois formes, octet pour octet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeureDuGeste {
    Jour(Jour),
    Minute(MinuteEcrite),
    Seconde(InstantEcrit),
}

impl HeureDuGeste {
    /// L'une des trois formes, et rien d'autre ; `None` sinon.
    pub fn lire(texte: &str) -> Option<HeureDuGeste> {
        InstantEcrit::lire(texte)
            .map(HeureDuGeste::Seconde)
            .or_else(|| MinuteEcrite::lire(texte).map(HeureDuGeste::Minute))
            .or_else(|| Jour::du_texte(texte).filter(|j| j.iso() == texte).map(HeureDuGeste::Jour))
    }

    /// Sa forme ecrite.
    pub fn iso(self) -> String {
        match self {
            HeureDuGeste::Jour(j) => j.iso(),
            HeureDuGeste::Minute(m) => m.iso(),
            HeureDuGeste::Seconde(s) => s.iso(),
        }
    }

    /// L'instant qu'il dit ; un jour seul, a 9 h.
    fn instant(self) -> Option<Moment> {
        let n = match self {
            HeureDuGeste::Jour(j) => return Moment::a_paris(j, 9, 0, 0),
            HeureDuGeste::Minute(m) => m.naif(),
            HeureDuGeste::Seconde(s) => s.naif(),
        };
        let s = n.seconde_du_jour();
        Moment::a_paris(n.jour(), s / 3_600, s % 3_600 / 60, s % 60)
    }
}

impl Serialize for HeureDuGeste {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.iso())
    }
}

impl<'de> Deserialize<'de> for HeureDuGeste {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<HeureDuGeste, D::Error> {
        let s = String::deserialize(d)?;
        HeureDuGeste::lire(&s).ok_or_else(|| {
            serde::de::Error::custom(format!("{} : une date (AAAA-MM-JJ) ou une heure de Paris (AAAA-MM-JJTHH:MM[:SS]) est attendue", socle::fichiers::repr(&s)))
        })
    }
}

/// Les fichiers d'un dossier, relatifs a lui (barres droites), dans l'ordre des noms.
fn fichiers_de(dossier: &Path) -> R<Vec<(String, PathBuf)>> {
    fn parcourir(base: &Path, d: &Path, out: &mut Vec<(String, PathBuf)>) -> R<()> {
        for e in std::fs::read_dir(d).map_err(|e| refus!("{} : {}", d.display(), e))? {
            let p = e.map_err(|e| refus!("{} : {}", d.display(), e))?.path();
            if p.is_dir() {
                parcourir(base, &p, out)?;
            } else {
                let r = p.strip_prefix(base).map_err(|e| refus!("{} : {}", p.display(), e))?;
                out.push((r.to_string_lossy().replace('\\', "/"), p.clone()));
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    parcourir(dossier, dossier, &mut out)?;
    out.sort();
    Ok(out)
}

// ------------------------------------------------------------------ le rejeu

/// Ce qu'on demande au rejeu.
#[derive(Debug, Clone, Default)]
pub struct OptionsRejeu {
    /// Rejouer aussi les routes qui n'avaient rien ecrit.
    pub lectures: bool,
    /// Jouer jusqu'a la fin de ce jour (a Paris) ; des gestes d'apres, seule la saisie s'ecrit (`saisie_d_un_geste`).
    pub jusqu_au: Option<Jour>,
}

/// Ce que le rejeu a fait.
#[derive(Debug, Clone, Default)]
pub struct Rejeu {
    /// Les compagnies rejouees, dans l'ordre de leur « + » : (dossier relatif a l'installation, racine).
    pub compagnies: Vec<(String, PathBuf)>,
    pub routes_rejouees: usize,
    pub lectures_ecartees: usize,
    /// Les gestes sur une compagnie que le scenario n'a pas ajoutee (une fictive livree ouverte pour voir) : ecartes.
    pub autres_compagnies: usize,
    /// Les gestes d'apres le jour dit (`OptionsRejeu::jusqu_au`) qui ne sont pas une saisie : laisses a l'horloge ou a la personne.
    pub laisses: usize,
    /// Ce que l'horloge des fictives a joue pendant le rejeu, en phrases, precedees de l'heure du geste.
    pub horloge: Vec<String>,
    /// L'instant du dernier geste rejoue (secondes UTC) : le jour ou l'on lit la compagnie qui en sort.
    pub fin: Option<Moment>,
    /// Ce qui ne s'est pas passe comme a l'enregistrement : une route reussie qui refuse au rejeu, ou l'inverse.
    pub ecarts: Vec<String>,
}

/// Les gestes d'un scenario, lus tels qu'ils sont ecrits.
pub fn lire(scenario: &Path) -> R<Vec<Geste>> {
    let f = scenario.join(GESTES);
    let texte = std::fs::read_to_string(&f).map_err(|e| refus!("{} : {}", f.display(), e))?;
    texte
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, l)| socle::forme::lire_texte(l, "Un geste du scenario ne se lit pas.").map_err(|e| refus!("{} ligne {} : {}", f.display(), i + 1, e.phrase)))
        .collect()
}

/// L'instant qu'un geste ecrit : `quand`, sinon `heure` a Paris (`AAAA-MM-JJ` a 9 h, `AAAA-MM-JJTHH:MM[:SS]`), sinon le
/// jour d'un geste `jour`, a 9 h. `None` : le geste n'en ecrit pas.
fn instant_ecrit(g: &Geste) -> R<Option<Moment>> {
    if let Some(t) = g.quand {
        return Ok(Some(t));
    }
    let h = match (g.heure, g.geste) {
        (Some(h), _) => h,
        (None, Some(Genre::Jour)) => HeureDuGeste::Jour(g.jour.ok_or_else(|| refus("un geste `jour` sans jour"))?),
        _ => return Ok(None),
    };
    Ok(Some(h.instant().ok_or_else(|| refus(socle::date::HORS_CALENDRIER))?))
}

/// L'instant d'une seconde apres `t`.
fn seconde_apres(t: Moment) -> R<Moment> {
    t.plus(1).ok_or_else(|| refus(socle::date::HORS_CALENDRIER))
}

/// Les instants de chaque geste, dans l'ordre du fichier : celui qu'il ecrit, ou une seconde apres le precedent. Un
/// instant ecrit qui recule refuse le scenario ; un instant egal au precedent passe une seconde apres lui (deux gestes ne
/// partagent jamais un instant : chacun tire ses aleas depuis le sien).
fn instants(gestes: &[Geste]) -> R<Vec<Moment>> {
    let mut out: Vec<Moment> = Vec::with_capacity(gestes.len());
    let mut ecrit_avant: Option<Moment> = None;
    for (i, g) in gestes.iter().enumerate() {
        let geste = |m: String| refus!("geste {} : {}", i + 1, m);
        let precedent = out.last().copied();
        let t = match instant_ecrit(g).map_err(|e| e.reformuler(|p| format!("geste {} : {}", i + 1, p)))? {
            Some(t) => {
                if ecrit_avant.is_some_and(|a| t < a) {
                    return Err(geste(format!("{} vient avant le geste qui le precede : les gestes vont dans l'ordre du temps", heure(t))));
                }
                ecrit_avant = Some(t);
                match precedent {
                    Some(p) if t <= p => seconde_apres(p)?,
                    _ => t,
                }
            }
            None => seconde_apres(precedent.ok_or_else(|| geste("le premier geste n'a ni `quand` ni `heure`".to_string()))?)?,
        };
        out.push(t);
    }
    Ok(out)
}

/// Le contenu d'une piece : `chemin` (relatif au scenario) ou l'empreinte `sha` (dans `pieces/`).
fn source_de_piece(scenario: &Path, sha: &str) -> PathBuf {
    scenario.join(PIECES).join(sha)
}

/// Pose, avant la route, ce qu'on avait depose : un dossier de la compagnie se remet tel qu'il etait ; un dossier
/// hors d'elle se recree dans `travail/<rang>/<cle>` et la charge le nomme. Rend la charge a donner au moteur.
fn poser_les_pieces(scenario: &Path, racine: &Path, travail: &Path, rang: usize, geste: &Geste) -> R<Charge> {
    let mut charge = geste.charge.clone().ok_or_else(|| refus!("geste {} : une route sans charge (`{{}}` pour une route qui ne prend rien)", rang))?;
    let Some(pieces) = &geste.pieces else { return Ok(charge) };
    for (cle, p) in pieces {
        // Les fichiers de la piece : (relatif a la piece, source). Un fichier seul a la cle `.`.
        let fichiers: Vec<(String, PathBuf)> = match (&p.chemin, &p.fichiers) {
            (Some(c), _) => {
                let source = scenario.join(c);
                if source.is_dir() {
                    fichiers_de(&source)?
                } else if source.is_file() {
                    vec![(".".to_string(), source)]
                } else {
                    return Err(refus!("piece {} : {} n'existe pas dans le scenario", cle, c));
                }
            }
            (None, Some(f)) => f.iter().map(|(rel, sha)| (rel.clone(), source_de_piece(scenario, sha))).collect(),
            (None, None) => return Err(refus!("piece {} : ni `chemin` ni `fichiers`", cle)),
        };
        let nom = p.nom.clone().or_else(|| p.chemin.as_deref().and_then(|c| Path::new(c).file_name()).map(|n| n.to_string_lossy().into_owned()));
        let est_fichier = p.fichier == Some(true) || (fichiers.len() == 1 && fichiers[0].0 == ".");
        let dans = p.dans_la_compagnie.as_deref();
        let cible = match dans {
            Some(rel) => racine.join(rel),
            None => travail.join(rang.to_string()).join(cle),
        };
        if est_fichier {
            let (_, source) = fichiers.first().ok_or_else(|| refus("piece sans contenu"))?;
            let dest = match (dans, &nom) {
                (Some(_), _) => cible.clone(),
                (None, Some(n)) => cible.join(n),
                (None, None) => return Err(refus!("piece {} : un fichier sans nom", cle)),
            };
            if let Some(d) = dest.parent() {
                std::fs::create_dir_all(d).map_err(|e| refus(e.to_string()))?;
            }
            std::fs::copy(source, &dest).map_err(|e| refus!("piece {} : {}", source.display(), e))?;
            if dans.is_none() {
                charge.poser_texte(cle, dest.display().to_string());
            }
            continue;
        }
        socle::fichiers::effacer(&cible)?;
        std::fs::create_dir_all(&cible).map_err(|e| refus!("{} : {}", cible.display(), e))?;
        for (rel, source) in &fichiers {
            let dest = cible.join(rel);
            if let Some(d) = dest.parent() {
                std::fs::create_dir_all(d).map_err(|e| refus(e.to_string()))?;
            }
            std::fs::copy(source, &dest).map_err(|e| refus!("piece {} : {}", source.display(), e))?;
        }
        if dans.is_none() {
            charge.poser_texte(cle, cible.display().to_string());
        }
    }
    Ok(charge)
}

/// REJOUE le scenario `scenario` dans l'installation `inst` (un dossier vide, ou qui ne porte pas encore les compagnies
/// du scenario) : le decor du « + », chaque geste a son instant, et rien d'autre. Le temps du moteur est simule le temps
/// du rejeu, puis rendu a la montre. `travail` recoit les dossiers deposes hors de la compagnie.
pub fn rejouer(scenario: &Path, inst: &Path, options: &OptionsRejeu) -> R<Rejeu> {
    rejouer_et_lire(scenario, inst, options, &[], &mut |_, _| {})
}

/// `rejouer`, qui s'arrete le soir de chacun des jours `arrets` (dans l'ordre) -- apres le dernier geste de ce jour-la,
/// avant le premier du lendemain -- pour que `lire` regarde la compagnie telle qu'elle est (le jour, et ce que le rejeu a
/// fait jusque-la), puis reprend. Un jour d'arret apres le dernier geste se lit a la fin. Le temps du moteur reste
/// celui du dernier geste joue : `lire` lit comme un poste, par une route ou les fichiers, a la date qu'il dit.
pub fn rejouer_et_lire(scenario: &Path, inst: &Path, options: &OptionsRejeu, arrets: &[Jour], lire_au: &mut dyn FnMut(Jour, &Rejeu)) -> R<Rejeu> {
    let gestes = lire(scenario)?;
    let instants = instants(&gestes)?;
    std::fs::create_dir_all(inst).map_err(|e| refus!("{} : {}", inst.display(), e))?;
    // L'INSTALLATION DU REJEU, BRIDEE EN TOUTES LETTRES (le geste de l'accueil, `bridage.rs`) : comme celle du banc, et le
    // moteur ne lit qu'elle (`compagnies::base`) -- rien du depot ni du poste ou l'on rejoue.
    compagnie::bridage::brider(inst)?;
    let inst = std::fs::canonicalize(inst).map_err(|e| refus!("{} : {}", inst.display(), e))?;
    let travail = inst.join("pieces-deposees");
    socle::temps::liberer();
    socle::temps::aleas_fixes();
    compagnie::installation::poser_l_installation_du_rejeu(Some(inst.clone()));
    let rendu = rejouer_les_gestes(scenario, &inst, &travail, &gestes, &instants, options, arrets, lire_au);
    compagnie::installation::poser_l_installation_du_rejeu(None);
    socle::temps::liberer();
    rendu
}

/// La racine du rejeu de la compagnie `c` (relative a l'installation) ; sans nom, la derniere ajoutee.
fn racine_nommee(racines: &BTreeMap<String, PathBuf>, derniere: &Option<PathBuf>, c: Option<&str>) -> Option<PathBuf> {
    match c {
        Some(c) => racines.get(c).cloned(),
        None => derniere.clone(),
    }
}

/// Le jour a Paris d'un instant.
fn jour_de(t: Moment) -> Jour {
    InstantEcrit::de(t).jour()
}

fn rejouer_les_gestes(
    scenario: &Path,
    inst: &Path,
    travail: &Path,
    gestes: &[Geste],
    instants: &[Moment],
    options: &OptionsRejeu,
    arrets: &[Jour],
    lire_au: &mut dyn FnMut(Jour, &Rejeu),
) -> R<Rejeu> {
    let mut r = Rejeu::default();
    let mut arrets = arrets.iter().copied().peekable();
    // LA LECTURE D'UN SOIR, avant le premier geste du lendemain. Ce qu'elle tire d'aleas ne change rien : chaque geste
    // repart des siens a son instant (`temps::figer`).
    let mut lire_les_soirs = |avant: Jour, r: &Rejeu| {
        while let Some(j) = arrets.next_if(|j| *j < avant) {
            lire_au(j, r);
        }
    };
    // Le dossier enregistre -> la racine du rejeu ; un geste qui ne nomme pas sa compagnie vise la derniere ajoutee.
    let mut racines: BTreeMap<String, PathBuf> = BTreeMap::new();
    let mut derniere: Option<PathBuf> = None;
    for (i, (g, &t)) in gestes.iter().zip(instants).enumerate() {
        let n = match g.n {
            Some(n) => n as usize,
            None => i + 1,
        };
        // APRES LE JOUR DIT, la saisie seule, ce jour-la (a 9 h, ou a la suite du dernier geste joue) : la carte s'ecrit,
        // le reste est a l'horloge ou a la personne.
        let (g, t) = match options.jusqu_au.filter(|j| jour_de(t) > *j) {
            None => (g.clone(), t),
            Some(j) => match saisie_d_un_geste(g) {
                Some(s) => {
                    let neuf_heures = Moment::a_paris(j, 9, 0, 0).ok_or_else(|| refus(socle::date::HORS_CALENDRIER))?;
                    (s, match r.fin {
                        Some(f) => {
                            let apres = seconde_apres(f)?;
                            if apres > neuf_heures { apres } else { neuf_heures }
                        }
                        None => neuf_heures,
                    })
                }
                None => {
                    r.laisses += 1;
                    continue;
                }
            },
        };
        let g = &g;
        let jour = jour_de(t);
        lire_les_soirs(jour, &r);
        socle::temps::figer(t);
        r.fin = Some(t);
        let compagnie = g.compagnie.as_deref();
        match g.geste {
            Some(Genre::Ajouter) => {
                let nom = g.nom.as_deref().ok_or_else(|| refus!("geste {} : `ajouter` sans nom", n))?;
                // Un geste `ajouter` sans `venue` est le « + » d'une compagnie neuve, le seul chemin d'avant le choix.
                let venue = match g.venue {
                    Some(v) => v,
                    None => Venue::Neuve,
                };
                let c = crate::accueil::ajouter_venant(inst, nom, venue).map_err(|m| m.reformuler(|p| format!("geste {} : « + » refuse : {}", n, p)))?;
                let rel = relatif(inst, &c.racine).map_err(|m| m.reformuler(|p| format!("geste {} : {}", n, p)))?;
                let nomme = match compagnie {
                    Some(c) if c != rel => {
                        r.ecarts.push(format!("geste {} : le « + » a cree {} (enregistre : {})", n, rel, c));
                        c.to_string()
                    }
                    Some(_) | None => rel,
                };
                racines.insert(nomme.clone(), c.racine.clone());
                derniere = Some(c.racine.clone());
                r.compagnies.push((nomme, c.racine));
            }
            Some(Genre::Simuler) => {
                let racine = racine_nommee(&racines, &derniere, compagnie).ok_or_else(|| refus!("geste {} : `simuler` sur une compagnie que le scenario n'a pas ajoutee ({:?})", n, compagnie))?;
                rendre_simulee(&racine).map_err(|m| m.reformuler(|p| format!("geste {} : {}", n, p)))?;
            }
            Some(Genre::Annoncer) => {
                let racine = racine_nommee(&racines, &derniere, compagnie).ok_or_else(|| refus!("geste {} : `annoncer` sur une compagnie que le scenario n'a pas ajoutee ({:?})", n, compagnie))?;
                let annonce = g.annonce.as_ref().ok_or_else(|| refus!("geste {} : `annoncer` sans annonce", n))?;
                let b = compagnie::bridage::branchement(&racine).map_err(|m| m.reformuler(|p| format!("geste {} : {}", n, p)))?;
                envois::organismes_simules::annoncer(&racine, b, annonce).map_err(|e| e.reformuler(|p| format!("geste {} : {}", n, p)))?;
            }
            Some(Genre::Etat) => {
                let Some(racine) = racine_nommee(&racines, &derniere, compagnie) else {
                    r.autres_compagnies += 1;
                    continue;
                };
                let cle = g.cle.as_deref().ok_or_else(|| refus!("geste {} : `etat` sans cle", n))?;
                let o = Options::de_la_compagnie(&racine);
                let refait = crate::service::contexte(&o).and_then(|ctx| crate::etat_garde::lire(&o, &ctx, cle));
                crate::remettre_le_fil();
                // Comme une route : l'etat refait reussit, ou refuse, comme il l'a fait au banc (`ok`, vrai s'il est tu).
                let attendu = g.a_reussi();
                match refait {
                    // Refuse au banc, il refuse au rejeu : rien a dire.
                    Err(e) => {
                        if attendu {
                            r.ecarts.push(format!("geste {} ({}) : l'etat de {} refuse au rejeu : {}", n, heure(t), cle, e.phrase));
                        }
                    }
                    Ok(()) => {
                        if !attendu {
                            r.ecarts.push(format!("geste {} ({}) : l'etat de {} reussit au rejeu, refuse a l'enregistrement", n, heure(t), cle));
                        }
                    }
                }
            }
            // Au banc, la compagnie ouverte joue son horloge dans sa veille, la route `ouverture` qui suit (`veille`) : le
            // jour ne la joue pas une fois de plus, ce qu'un pas refuse se rejouant a chaque passage.
            Some(Genre::Jour) if g.veille == Some(true) => {}
            Some(Genre::Jour) => {
                for (_, racine) in &r.compagnies {
                    match crate::horloge::rattraper(racine, Some(jour), true) {
                        Ok(fait) => r.horloge.extend(fait.iter().map(|p| format!("{} {}", heure(t), p))),
                        Err(e) => r.ecarts.push(format!("geste {} ({}) : l'horloge refuse : {}", n, heure(t), e.phrase)),
                    }
                }
            }
            Some(Genre::Debut) => {}
            None => {
                let nom = g.route.as_deref().ok_or_else(|| refus!("geste {} : ni `geste` ni `route`", n))?;
                // Un nom qui n'est celui d'aucune route (une route d'avant, retiree) se rejoue comme le moteur le refuse.
                let route = Route::lire(nom);
                let Some(racine) = racine_nommee(&racines, &derniere, compagnie) else {
                    r.autres_compagnies += 1;
                    continue;
                };
                if !g.a_ecrit() && !options.lectures {
                    r.lectures_ecartees += 1;
                    continue;
                }
                let charge = poser_les_pieces(scenario, &racine, travail, n, g)?;
                let rep = match route {
                    Some(route) => crate::dans_la_fenetre(&Options::de_la_compagnie(&racine), route, &charge),
                    None => Reponse::refusee(Refus::de(None, crate::enveloppe::route_inconnue(nom))),
                };
                r.routes_rejouees += 1;
                let ok = rep.est_rendue();
                // Ce que l'horloge de la fictive a joue (la route `horloge-rattraper`, ou la veille par `ouverture`), date du geste.
                // Ces deux routes seules font jouer l'horloge ; une autre n'en dit rien.
                let jouees = match route {
                    Some(Route::HorlogeRattraper) => Some("phrases"),
                    Some(Route::Ouverture) => Some("horloge"),
                    Some(_) | None => None,
                };
                r.horloge.extend(jouees.iter().flat_map(|cle| rep.textes(cle)).map(|p| format!("{} {}", heure(t), p)));
                let attendu = g.a_reussi();
                if ok != attendu {
                    let refus: String = rep.refus().map(|e| format!(" : {}", e.phrase)).into_iter().collect();
                    r.ecarts.push(format!(
                        "geste {} ({} {}) : {} au rejeu, {} a l'enregistrement{}",
                        n,
                        heure(t),
                        nom,
                        if ok { "reussit" } else { "refuse" },
                        if attendu { "reussi" } else { "refuse" },
                        refus
                    ));
                }
            }
        }
    }
    for j in arrets {
        lire_au(j, &r);
    }
    Ok(r)
}

/// CE QUI SE SAISIT D'UN GESTE : une case d'une carte (`enregistrer-saisie`), une fiche (`enregistrer-fiche`), reussies
/// a l'enregistrement. Tout autre geste valide, envoie, depose, coche ou avance le calendrier : `None`.
fn saisie_d_un_geste(g: &Geste) -> Option<Geste> {
    if !matches!(g.route.as_deref().and_then(Route::lire)?, Route::EnregistrerSaisie | Route::EnregistrerFiche) {
        return None;
    }
    if !g.a_reussi() || !g.a_ecrit() {
        return None;
    }
    Some(Geste { quand: None, heure: None, ..g.clone() })
}

// ------------------------------------------------------------------ elaguer, verifier, la ligne de commande

/// ELAGUER un scenario pour le figer (phase 4) : `vers` recoit les memes gestes sans les routes qui n'ont rien ecrit (le
/// rejeu les ecarte deja), et les seules pieces que les gestes gardes nomment. Rend (gestes gardes, lectures otees).
pub fn elaguer(scenario: &Path, vers: &Path) -> R<(usize, usize)> {
    let gestes = lire(scenario)?;
    std::fs::create_dir_all(vers.join(PIECES)).map_err(|e| refus!("{} : {}", vers.display(), e))?;
    let copier = |rel: &Path| -> R<()> {
        let dest = vers.join(rel);
        if let Some(d) = dest.parent() {
            std::fs::create_dir_all(d).map_err(|e| refus!("{} : {}", d.display(), e))?;
        }
        std::fs::copy(scenario.join(rel), &dest).map(|_| ()).map_err(|e| refus!("piece {} : {}", rel.display(), e))
    };
    let (mut gardes, mut otees) = (Vec::new(), 0);
    for g in gestes {
        if g.route.is_some() && !g.a_ecrit() {
            otees += 1;
            continue;
        }
        for p in g.pieces.iter().flat_map(BTreeMap::values) {
            for sha in p.fichiers.iter().flat_map(BTreeMap::values) {
                copier(&Path::new(PIECES).join(sha))?;
            }
            if let Some(c) = &p.chemin {
                let source = scenario.join(c);
                if source.is_dir() {
                    for (rel, _) in fichiers_de(&source)? {
                        copier(&Path::new(c).join(rel))?;
                    }
                } else {
                    copier(Path::new(c))?;
                }
            }
        }
        gardes.push(socle::ecriture::json_en_ligne(&g));
    }
    let n = gardes.len();
    std::fs::write(vers.join(GESTES), gardes.join("\n") + "\n").map_err(|e| refus!("{} : {}", vers.display(), e))?;
    Ok((n, otees))
}

/// Ce qu'une comparaison de deux compagnies ne regarde pas : les caches que la fenetre refait en fond, son journal, l'etape
/// en cours d'un passage de la veille, et le journal des incidents (chaque incident porte le fil de la fenetre, ce que la
/// page a fait avant lui, et la fenetre y note les refus que la page a recus : `compagnie::incidents::noter`).
fn hors_comparaison(rel: &str) -> bool {
    rel == compagnie::installation::JOURNAL_INTERFACE || rel == compagnie::incidents::INCIDENTS || rel == "ouverture.etat.json" || rel.ends_with(crate::gestes::ouverture::ETAPE)
}

/// Les fichiers d'une compagnie, relatifs (barres droites), hors caches. Une compagnie que le banc n'a pas n'a aucun
/// fichier ; un dossier qui ne se lit pas se dit.
fn fichiers_compares(racine: &Path) -> R<BTreeMap<String, PathBuf>> {
    if !socle::fichiers::present(racine)? {
        return Ok(BTreeMap::new());
    }
    Ok(fichiers_de(racine)?.into_iter().filter(|(r, _)| !hors_comparaison(r)).collect())
}

/// Le contenu d'un fichier, le chemin de sa compagnie remplace par `<racine>` (un journal note des chemins).
fn contenu(p: &Path, racine: &Path) -> R<Vec<u8>> {
    let o = std::fs::read(p).map_err(|e| refus!("{} : {}", p.display(), e))?;
    Ok(match String::from_utf8(o) {
        Ok(t) => t.replace(&racine.to_string_lossy().into_owned(), "<racine>").into_bytes(),
        Err(e) => e.into_bytes(),
    })
}

/// VERIFIER UN BANC : rejoue son scenario dans `travail` (vide d'abord) et compare, fichier par fichier, chaque
/// compagnie que le scenario a ajoutee a celle du banc (`inst`). Le banc tire les memes aleas que le rejeu
/// (`temps::aleas`) : toute difference est un geste qui a ecrit sans etre enregistre (un fichier touche hors de la page, une
/// ecriture de fond), ou un ecart du rejeu. Rend les differences, en phrases (vide : le scenario suffit).
pub fn verifier(scenario: &Path, inst: &Path, travail: &Path) -> R<Vec<String>> {
    socle::fichiers::effacer(travail)?;
    let r = rejouer(scenario, travail, &OptionsRejeu::default())?;
    let mut ecarts: Vec<String> = r.ecarts.iter().map(|e| format!("au rejeu, {}", e)).collect();
    for (rel, rejouee) in &r.compagnies {
        let banc = inst.join(rel);
        let (a, b) = (fichiers_compares(&banc)?, fichiers_compares(rejouee)?);
        for (f, p) in &a {
            let Some(q) = b.get(f) else {
                ecarts.push(format!("{}/{} : au banc, pas au rejeu (ecrit sans etre enregistre ?)", rel, f));
                continue;
            };
            let (x, y) = (contenu(p, &banc)?, contenu(q, rejouee)?);
            if x != y {
                let ligne = String::from_utf8_lossy(&x).lines().zip(String::from_utf8_lossy(&y).lines()).position(|(u, v)| u != v);
                let ou: String = ligne.map(|l| format!(" (des la ligne {})", l + 1)).into_iter().collect();
                ecarts.push(format!("{}/{} : differe du rejeu{}", rel, f, ou));
            }
        }
        for f in b.keys().filter(|f| !a.contains_key(*f)) {
            ecarts.push(format!("{}/{} : au rejeu, pas au banc", rel, f));
        }
    }
    Ok(ecarts)
}

// ------------------------------------------------------------------ les fictives de reference

/// LES FICTIVES DE REFERENCE, a cote du logiciel (`<depot>/atelier/fictives/<nom>/`) : chacune est un scenario ecrit
/// comme la personne qui administre la compagnie l'aurait joue, par les routes seulement, avec les papiers qu'elle
/// importe. Le meme scenario est la preuve commune en ligne de commande (son rejeu, `tests/rejeu.rs`) et la fictive que
/// les installations recoivent (`livrer`, que « Publier » appelle par `publier::marquer_les_fictives`).
pub const DOSSIER_REFERENCE: &str = "atelier/fictives";

/// Les scenarios des fictives de reference d'un depot (`depot/atelier/fictives/<nom>/`, ceux qui ont des gestes), par
/// nom, dans l'ordre des noms. Un dossier qui ne se lit pas (ou manque) refuse : une publication sans ses fictives n'en
/// est pas une.
pub fn fictives_de_reference(depot: &Path) -> R<Vec<(String, PathBuf)>> {
    let dossier = depot.join(DOSSIER_REFERENCE);
    let Some(entrees) = socle::fichiers::entrees(&dossier)? else {
        return Err(refus!("Les fictives de référence manquent : {} n'existe pas.", dossier.display()));
    };
    Ok(entrees.into_iter().filter(|p| p.join(GESTES).is_file()).map(|p| (socle::fichiers::nom_du(&p), p)).collect())
}

/// Ce qu'une fictive livree ne porte pas de son rejeu : le marqueur (reecrit), les incidents du rejeu, les caches.
fn hors_livraison(rel: &str) -> bool {
    rel == compagnie::installation::MARQUEUR || rel == compagnie::incidents::INCIDENTS || hors_comparaison(rel)
}

/// LIVRER UNE FICTIVE DE REFERENCE : rejoue `scenario` (jusqu'au jour `jusqu_au`, ou entier) dans une installation de
/// travail, et pose la compagnie qui en sort dans `dest` (`poser_livree`). Les mois d'avant le jour sont faits ; ceux
/// d'apres portent deja leurs cartes, non validees : sur le poste, seul le temps passe, et l'horloge de la fictive
/// valide, envoie et depose a leur date (`horloge`). Un ecart du rejeu (une route qui refuse) refuse la livraison, et
/// `dest` reste tel qu'il etait.
pub fn livrer(scenario: &Path, dest: &Path, jusqu_au: Option<Jour>) -> R<Rejeu> {
    let nom = dest.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or_else(|| refus!("{} : un dossier est attendu", dest.display()))?;
    let travail = std::env::temp_dir().join(format!("puppetpaie-livrer-{}-{}", nom, std::process::id()));
    socle::fichiers::effacer(&travail)?;
    let rendu = rejouer(scenario, &travail, &OptionsRejeu { lectures: false, jusqu_au }).and_then(|r| {
        if !r.ecarts.is_empty() {
            return Err(refus!("le scenario {} s'ecarte au rejeu :\n{}", scenario.display(), r.ecarts.join("\n")));
        }
        let (_, racine) = r.compagnies.first().ok_or_else(|| refus!("le scenario {} n'ajoute aucune compagnie", scenario.display()))?;
        poser_livree(racine, dest, r.fin)?;
        Ok(r)
    });
    match rendu {
        Ok(r) => socle::fichiers::effacer(&travail).map(|()| r),
        Err(e) => Err(socle::fichiers::effacer_apres(e, &travail)),
    }
}

/// POSE LA COMPAGNIE REJOUEE `racine` dans `dest` (remplace), marquee fictive livree (`compagnies::MARQUE_FICTIVE` suivie
/// du nom du dossier, sans la marque `ajoutee`) : ses envois sont joues, son horloge joue ce qui vient apres `fin`, le
/// dernier geste rejoue.
fn poser_livree(racine: &Path, dest: &Path, fin: Option<Moment>) -> R<()> {
    let nom = dest.file_name().map(|n| n.to_string_lossy().into_owned()).ok_or_else(|| refus!("{} : un dossier est attendu", dest.display()))?;
    socle::fichiers::effacer(dest)?;
    for (rel, source) in fichiers_de(racine)?.into_iter().filter(|(rel, _)| !hors_livraison(rel)) {
        let cible = dest.join(&rel);
        if let Some(d) = cible.parent() {
            std::fs::create_dir_all(d).map_err(|e| refus!("{} : {}", d.display(), e))?;
        }
        std::fs::copy(&source, &cible).map_err(|e| refus!("{} : {}", cible.display(), e))?;
    }
    let marqueur = compagnie::installation::Marqueur {
        copie_de: format!("{} {}", compagnie::installation::MARQUE_FICTIVE, nom),
        fictive: None,
        ajoutee: None,
        quand: fin.map(socle::paris::InstantEcrit::de),
        quoi: "Compagnie FICTIVE de reference, livree avec le logiciel : le rejeu de son scenario (atelier/fictives/). Ses envois sont joues, rien n'en part.".to_string(),
    };
    socle::ecriture::ecrire_json(&dest.join(compagnie::installation::MARQUEUR), &marqueur, 2)
}

const USAGE: &str = "scenario rejouer <scenario> <installation neuve> [--lectures] [--jusqu-au AAAA-MM-JJ]\n\
                     scenario verifier <scenario> <installation du banc> <dossier de travail>\n\
                     scenario elaguer <scenario> <scenario sans lectures>\n\
                     scenario livrer <scenario> <dossier de la fictive> [--jusqu-au AAAA-MM-JJ]";

/// LA LIGNE DE COMMANDE DES SCENARIOS : `moteur-atelier scenario ...` (le banc) et l'exemple `vie_fictive` (sans le binaire
/// de l'atelier). `rejouer`, `verifier`, `elaguer`. Rend le code de sortie : 1 a la moindre difference.
pub fn commande(args: &[String]) -> i32 {
    let mut options = OptionsRejeu::default();
    let mut a: Vec<&str> = Vec::new();
    let mut it = args.iter().map(String::as_str);
    while let Some(x) = it.next() {
        match x {
            "--lectures" => options.lectures = true,
            "--jusqu-au" => match it.next().map(Jour::lire) {
                Some(Ok(j)) => options.jusqu_au = Some(j),
                _ => {
                    eprintln!("--jusqu-au attend un jour, AAAA-MM-JJ.\n{}", USAGE);
                    return 2;
                }
            },
            _ => a.push(x),
        }
    }
    // Rien ne sort de la machine (`garde_reseau`) : lancee a la main, la commande pose la variable, avant tout fil.
    if std::env::var_os(socle::garde_reseau::VARIABLE).is_none() {
        std::env::set_var(socle::garde_reseau::VARIABLE, "1");
    }
    let debut = std::time::Instant::now();
    match a.as_slice() {
        ["rejouer", sc, inst] => {
            let inst = Path::new(inst);
            let pleine = match std::fs::read_dir(inst) {
                Ok(mut d) => d.next().is_some(),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
                Err(e) => {
                    eprintln!("{} : {}", inst.display(), e);
                    return 2;
                }
            };
            if pleine {
                eprintln!("{} n'est pas vide : le rejeu part d'une installation neuve.", inst.display());
                return 2;
            }
            match rejouer(Path::new(sc), inst, &options) {
                Ok(r) => {
                    for p in &r.horloge {
                        dire!("      {}", p);
                    }
                    for (nom, racine) in &r.compagnies {
                        dire!("{} -> {}", nom, racine.display());
                    }
                    dire!(
                        "{} route(s) rejouee(s), {} lecture(s) ecartee(s), {} geste(s) sur une autre compagnie, en {} s.",
                        r.routes_rejouees,
                        r.lectures_ecartees,
                        r.autres_compagnies,
                        socle::num::secondes_au_dixieme(debut.elapsed())
                    );
                    for e in &r.ecarts {
                        dire!("ECART  {}", e);
                    }
                    i32::from(!r.ecarts.is_empty())
                }
                Err(m) => {
                    eprintln!("{}", m.en_lignes());
                    1
                }
            }
        }
        ["verifier", sc, inst, travail] => match verifier(Path::new(sc), Path::new(inst), Path::new(travail)) {
            Ok(e) if e.is_empty() => {
                dire!("Le rejeu refait la compagnie du banc, fichier pour fichier ({} s).", socle::num::secondes_au_dixieme(debut.elapsed()));
                0
            }
            Ok(e) => {
                for x in &e {
                    dire!("DIFFERENCE  {}", x);
                }
                1
            }
            Err(m) => {
                eprintln!("{}", m.en_lignes());
                1
            }
        },
        ["livrer", sc, dest] => match livrer(Path::new(sc), Path::new(dest), options.jusqu_au) {
            Ok(r) => {
                dire!("{} : {} route(s) rejouee(s), {} geste(s) d'apres le jour laisse(s) a l'horloge, en {} s.", dest, r.routes_rejouees, r.laisses, socle::num::secondes_au_dixieme(debut.elapsed()));
                0
            }
            Err(m) => {
                eprintln!("{}", m.en_lignes());
                1
            }
        },
        ["elaguer", sc, vers] => match elaguer(Path::new(sc), Path::new(vers)) {
            Ok((n, o)) => {
                dire!("{} geste(s) garde(s), {} lecture(s) otee(s) : {}", n, o, vers);
                0
            }
            Err(m) => {
                eprintln!("{}", m.en_lignes());
                1
            }
        },
        _ => {
            eprintln!("{}", USAGE);
            2
        }
    }
}

// ------------------------------------------------------------------ l'enregistreur (l'atelier seulement)

#[cfg(feature = "atelier")]
pub use enregistreur::Enregistreur;

#[cfg(feature = "atelier")]
mod enregistreur {
    use super::*;
    use std::io::Write;
    use std::sync::{Mutex, OnceLock};

    /// L'enregistreur de la fenetre de l'atelier : un seul par processus, pose au lancement (`demarrer`).
    pub struct Enregistreur {
        dossier: PathBuf,
        /// L'installation : les compagnies se nomment relativement a elle.
        inst: PathBuf,
        /// Un geste a la fois, de bout en bout : la route, son empreinte avant et apres, sa ligne. Le temps est fige
        /// pendant ce temps-la (`temps::figer`) ; l'ordre du fichier est celui ou les gestes ont eu lieu.
        tour: Mutex<u64>,
        /// Vrai : les lectures entrent aussi dans le scenario. Faux (le banc, les preuves) : seules les routes dont la
        /// fiche ecrit ou engage (`routes::est_une_ecriture`) ; une lecture est servie telle quelle, sans tour ni
        /// empreinte, en meme temps que les autres -- le rejeu les ecarte de toute facon.
        lectures: bool,
    }

    static ENREGISTREUR: OnceLock<Enregistreur> = OnceLock::new();

    /// L'enregistreur de ce processus, s'il y en a un.
    pub fn actif() -> Option<&'static Enregistreur> {
        ENREGISTREUR.get()
    }

    /// Ce qu'une empreinte de la compagnie ne regarde pas : les caches que la fenetre refait en fond, et son journal.
    fn hors_empreinte(rel: &str) -> bool {
        rel == compagnie::installation::JOURNAL_INTERFACE
            || rel == "ouverture.etat.json"
            || rel.ends_with(crate::gestes::ouverture::ETAPE)
            || rel.ends_with(".verrou")
    }

    /// L'empreinte d'une compagnie : chaque fichier, sa taille et sa date (hors caches ; `None` : datee hors du calendrier,
    /// ce qui se compare comme une date), ou ce que le disque en refuse ; ou, quand ses fichiers ne se listent pas, le
    /// refus tel que la route le rendrait.
    enum Empreinte {
        Lue(BTreeMap<String, Result<(u64, Option<Moment>), std::io::ErrorKind>>),
        Refusee(crate::enveloppe::Refus),
    }

    impl PartialEq for Empreinte {
        fn eq(&self, autre: &Empreinte) -> bool {
            match (self, autre) {
                (Empreinte::Lue(a), Empreinte::Lue(b)) => a == b,
                // Deux refus sont les memes quand la porte les ecrit pareil.
                (Empreinte::Refusee(a), Empreinte::Refusee(b)) => socle::ecriture::json_en_ligne(a) == socle::ecriture::json_en_ligne(b),
                (Empreinte::Lue(_), Empreinte::Refusee(_)) | (Empreinte::Refusee(_), Empreinte::Lue(_)) => false,
            }
        }
    }

    fn empreinte(racine: &Path) -> Empreinte {
        match fichiers_de(racine) {
            Ok(f) => Empreinte::Lue(
                f.into_iter()
                    .filter(|(r, _)| !hors_empreinte(r))
                    .map(|(r, p)| (r, std::fs::metadata(&p).map(|m| (m.len(), socle::paris::mtime(&m))).map_err(|e| e.kind())))
                    .collect(),
            ),
            Err(e) => Empreinte::Refusee(crate::enveloppe::Refus::de(None, e)),
        }
    }

    impl Enregistreur {
        /// Pose l'enregistreur de ce processus : `dossier` (cree s'il n'existe pas ; un scenario commence s'y poursuit),
        /// `inst` l'installation, `lectures` : les lectures aussi.
        pub fn demarrer(dossier: &Path, inst: &Path, lectures: bool) -> R<&'static Enregistreur> {
            std::fs::create_dir_all(dossier.join(PIECES)).map_err(|e| refus!("{} : {}", dossier.display(), e))?;
            // Un scenario commence se poursuit apres son dernier rang ; un neuf n'a pas encore de gestes.
            let deja = if dossier.join(GESTES).is_file() { lire(dossier)?.iter().filter_map(|g| g.n).fold(0, u64::max) } else { 0 };
            let e = Enregistreur { dossier: dossier.to_path_buf(), inst: inst.to_path_buf(), tour: Mutex::new(deja), lectures };
            ENREGISTREUR.set(e).map_err(|_| refus("Un enregistreur est deja pose dans ce processus."))?;
            Ok(ENREGISTREUR.get().expect("pose"))
        }

        pub fn dossier(&self) -> &Path {
            &self.dossier
        }

        /// Le dernier instant enregistre (le banc reprend apres lui).
        pub fn dernier_instant(&self) -> R<Option<Moment>> {
            if !self.dossier.join(GESTES).is_file() {
                return Ok(None);
            }
            Ok(lire(&self.dossier)?.iter().filter_map(|g| g.quand).reduce(|a, b| if b > a { b } else { a }))
        }

        /// Ecrit la ligne du geste `g`, au rang suivant et a son instant.
        fn ecrire(&self, n: &mut u64, g: Geste, quand: Moment) -> R<()> {
            *n += 1;
            let ligne = Geste { n: Some(*n), quand: Some(quand), heure: Some(HeureDuGeste::Seconde(InstantEcrit::de(quand))), ..g };
            let f = self.dossier.join(GESTES);
            let mut fichier = std::fs::OpenOptions::new().create(true).append(true).open(&f).map_err(|e| refus!("{} : {}", f.display(), e))?;
            writeln!(fichier, "{}", socle::ecriture::json_en_ligne(&ligne)).map_err(|e| refus!("{} : {}", f.display(), e))
        }

        /// Un geste qui n'est pas une route (`debut`, `ajouter`, `simuler`, `jour`), fait par `f` sous le meme tour et au
        /// meme instant fige ; `f` rend sa ligne (`Geste::du_banc`), ou son refus (rien n'est alors enregistre).
        pub fn geste<T>(&self, f: impl FnOnce() -> R<(T, Geste)>) -> R<T> {
            let mut n = socle::memoire::tenu(&self.tour);
            let quand = socle::temps::secondes();
            socle::temps::figer(quand);
            let rendu = f();
            socle::temps::defiger();
            let (t, ligne) = rendu?;
            self.ecrire(&mut n, ligne, quand)?;
            Ok(t)
        }

        /// L'ETAT D'UN MOIS QUE LA FENETRE REFAIT EN FOND (`etat_garde::FileDesEtats`) : ce qui se lit peut ecrire (une paie
        /// validee qui se recalcule, celle d'un contrat sans terme qui entre dans le mois), sans qu'aucune route de la
        /// page ne le demande. `f` le refait sous le meme tour, au meme instant fige ; s'il a ecrit, le geste `etat`
        /// (la compagnie, la cle) entre dans le scenario, et le rejeu le refait (`etat_garde::lire`).
        pub fn fond<T, E>(&self, racine: &Path, cle: &str, f: impl FnOnce() -> std::thread::Result<Result<T, E>>) -> std::thread::Result<Result<T, E>> {
            let mut n = socle::memoire::tenu(&self.tour);
            let quand = socle::temps::secondes();
            socle::temps::figer(quand);
            let avant = empreinte(racine);
            let rendu = f();
            let ecrit = empreinte(racine) != avant;
            socle::temps::defiger();
            if ecrit {
                // Un etat refait peut ecrire puis refuser (une paie recalculee, puis un engagement qui ne se calcule pas) :
                // le rejeu attend le meme refus.
                let ok = (!matches!(rendu, Ok(Ok(_)))).then_some(false);
                let ecrite = self.compagnie(racine).and_then(|c| self.ecrire(&mut n, Geste { compagnie: Some(c), cle: Some(cle.to_string()), ok, ..Geste::du_banc(Genre::Etat) }, quand));
                non_enregistre(ecrite);
            }
            rendu
        }

        /// Le dossier d'une compagnie, tel que le scenario le nomme.
        pub fn compagnie(&self, racine: &Path) -> R<String> {
            relatif(&self.inst, racine)
        }

        /// UNE ROUTE, servie par `servir` sous l'enregistreur : le temps fige a son debut, ce qu'on avait depose recopie,
        /// l'empreinte de la compagnie avant et apres, puis sa ligne. Une lecture, quand on ne les enregistre pas, est
        /// seulement servie.
        pub fn route(&self, racine: &Path, nom: Route, charge: &Charge, servir: impl FnOnce() -> Reponse) -> Reponse {
            if !self.lectures && !crate::est_une_ecriture(nom) {
                return servir();
            }
            let mut n = socle::memoire::tenu(&self.tour);
            // Ce que la route va lire, recopie avant elle : une piece qui ne se recopie pas refuse la route, que le
            // scenario ne saurait pas rejouer.
            let pieces = match self.recopier_les_pieces(racine, nom, charge) {
                Ok(p) => p,
                Err(m) => return Reponse::refusee_par(nom, Refus::nouveau(socle::echec::Classe::Disque, format!("Le banc d'essai ne recopie pas ce que la route lit : {}", m.phrase))),
            };
            let quand = socle::temps::secondes();
            socle::temps::figer(quand);
            let avant = empreinte(racine);
            let rep = servir();
            let ecrit = empreinte(racine) != avant;
            socle::temps::defiger();
            let ligne = |compagnie: String| Geste {
                route: Some(nom.nom().to_string()),
                compagnie: Some(compagnie),
                charge: Some(charge.sans_les_mots_de_passe()),
                pieces: Some(pieces).filter(|p| !p.is_empty()),
                ecrit: Some(ecrit),
                ok: Some(rep.est_rendue()),
                // Le refus se garde, pour qui relit le scenario (le rejeu ne compare que `ok`).
                refus: rep.refus().map(|r| r.phrase.clone()),
                ..Geste::default()
            };
            non_enregistre(self.compagnie(racine).and_then(|c| self.ecrire(&mut n, ligne(c), quand)));
            rep
        }

        /// Recopie un fichier dans `pieces/` (une fois par contenu) ; rend son empreinte.
        fn recopier(&self, p: &Path) -> R<String> {
            let octets = std::fs::read(p).map_err(|e| refus!("{} : {}", p.display(), e))?;
            let sha = socle::fichiers::sha256_hex(&octets);
            let dest = self.dossier.join(PIECES).join(&sha);
            if !dest.exists() {
                std::fs::write(&dest, &octets).map_err(|e| refus!("{} : {}", dest.display(), e))?;
            }
            Ok(sha)
        }

        /// Ce que la route lit hors du moteur : un dossier ou un fichier nomme par la charge (`CLES_DE_DEPOT`), ou la
        /// boite aux lettres `import/` de la compagnie. Chaque fichier entre dans `pieces/`.
        fn recopier_les_pieces(&self, racine: &Path, route: Route, charge: &Charge) -> R<BTreeMap<String, Piece>> {
            let mut out = BTreeMap::new();
            for cle in CLES_DE_DEPOT {
                let valeur = charge.texte(cle).map(str::trim);
                let chemin = match valeur {
                    Some(s) if !s.is_empty() => {
                        let p = PathBuf::from(crate::depots::normaliser(s));
                        if p.is_absolute() {
                            p
                        } else {
                            continue;
                        }
                    }
                    _ if cle == "dossier" && lit_la_boite_aux_lettres(route) => racine.join("import"),
                    _ => continue,
                };
                let dans = match chemin.strip_prefix(racine) {
                    Ok(r) => Some(r.to_string_lossy().replace('\\', "/")),
                    Err(hors) => {
                        drop(hors);
                        None
                    }
                };
                let mut fichiers = BTreeMap::new();
                let piece = if chemin.is_file() {
                    // Un fichier de la compagnie que le moteur a ecrit lui-meme (une DSN de `sorties/` a deposer) n'est
                    // pas une piece : le rejeu le refait.
                    if dans.is_some() {
                        continue;
                    }
                    fichiers.insert(".".to_string(), self.recopier(&chemin)?);
                    Piece { fichier: Some(true), nom: chemin.file_name().map(|n| n.to_string_lossy().into_owned()), ..Piece::default() }
                } else if chemin.is_dir() {
                    for (rel, p) in fichiers_de(&chemin)? {
                        fichiers.insert(rel, self.recopier(&p)?);
                    }
                    Piece::default()
                } else {
                    continue;
                };
                out.insert(cle.to_string(), Piece { dans_la_compagnie: dans, fichiers: Some(fichiers), ..piece });
            }
            Ok(out)
        }
    }

    /// Une ligne du scenario qui ne s'ecrit pas : la route a ete servie, le banc le dit sur sa console.
    fn non_enregistre(ecrite: R<()>) {
        if let Err(m) = ecrite {
            eprintln!("Banc d'essai : un geste ne s'enregistre pas dans le scenario : {}", m.en_lignes());
        }
    }
}

#[cfg(feature = "atelier")]
pub use enregistreur::actif as enregistreur_actif;
