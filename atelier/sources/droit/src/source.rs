//! LE FONDEMENT D'UNE VALEUR : le texte depose qui la donne (`Source`), ou plusieurs textes d'emetteurs distincts qui
//! convergent (`Fondement::Convergence`). Toute regle vient d'un texte depose dans `atelier/reference/` (atelier/AGENTS.md,
//! « Les sources ») ; dans un fichier, le champ s'appelle `source`. Le bareme le porte a chaque valeur qu'un texte fonde,
//! la banque des conseils a chaque conseil.
//!
//! Un seul verificateur, en test (`moteur/tests/fichiers/sources.rs`) : chaque fichier est depose, chaque citation s'y
//! lit, une convergence a au moins deux emetteurs distincts.

use serde::Deserialize;

/// Le chemin d'un texte depose, depuis la racine du depot : sous `atelier/reference/`, ou le mode d'emploi du logiciel
/// (`atelier/README.md`, ce que le logiciel fait). Un article d'un entrepot de la DILA (`articles.jsonl.gz`) se nomme
/// `<entrepot>#<code>` (`legi-entrepot/articles.jsonl.gz#travail`, `kali-entrepot/articles.jsonl.gz#3090`), et
/// l'article de la source est sa cle dans l'entrepot (`D7121-41`, `KALIARTI000027609780`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, serde::Serialize)]
#[serde(try_from = "String")]
pub struct CheminDepose(String);

/// Le mode d'emploi, la seule source hors de `atelier/reference/`.
pub const MODE_D_EMPLOI: socle::fichiers::CheminRelatif = socle::fichiers::CheminRelatif::du_moteur("atelier/README.md");
/// La reference de l'atelier, ou se deposent les textes.
const REFERENCE: socle::fichiers::CheminRelatif = socle::fichiers::CheminRelatif::du_moteur("atelier/reference");

/// Le chemin `c` est-il sous la reference : un de ses dossiers l'est-il ?
fn sous_la_reference(c: &socle::fichiers::CheminRelatif) -> bool {
    let mut dossier = c.parent();
    while let Some(d) = dossier {
        if d == REFERENCE {
            return true;
        }
        dossier = d.parent();
    }
    false
}

/// Le fichier d'un chemin depose, sans le code d'entrepot qui suit son `#`.
fn sans_code(s: &str) -> &str {
    match s.split_once('#') {
        Some((fichier, _)) => fichier,
        None => s,
    }
}

/// CE QU'UNE SOURCE ECRITE A DE FAUX, a sa lecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FauteDeSource {
    /// Le chemin n'est pas un chemin relatif, n'est pas sous `atelier/reference/` (ni le mode d'emploi), ou remonte (`..`).
    HorsDeLaReference,
    /// Un `#` hors d'un entrepot de la DILA, ou un entrepot sans son code.
    CodeDEntrepot,
    /// Une source seule sans son fichier, son emetteur ou sa citation.
    SourceIncomplete,
    /// Une convergence qui porte autre chose que ses sources et son raisonnement.
    ConvergenceMelee,
}

impl std::fmt::Display for FauteDeSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            FauteDeSource::HorsDeLaReference => "ce fichier n'est pas un texte déposé dans la référence de l'atelier",
            FauteDeSource::CodeDEntrepot => "un article d'un entrepôt de la DILA nomme son code après le fichier, et lui seul",
            FauteDeSource::SourceIncomplete => "une source nomme son fichier, son émetteur et sa citation",
            FauteDeSource::ConvergenceMelee => "une convergence porte ses sources et son raisonnement, et rien d'autre",
        })
    }
}

impl TryFrom<String> for CheminDepose {
    type Error = FauteDeSource;
    fn try_from(s: String) -> Result<Self, FauteDeSource> {
        let fichier = sans_code(&s);
        // La porte : le chemin ecrit dans un fichier se lit en chemin relatif, compare a celui que le moteur nomme.
        let lu = match socle::fichiers::CheminRelatif::lire(fichier) {
            Ok(c) => c,
            Err(_) => return Err(FauteDeSource::HorsDeLaReference),
        };
        // Un chemin relatif ne remonte pas (`CheminRelatif::lire` refuse `..`) : sous la reference, il y reste.
        if !(sous_la_reference(&lu) || lu == MODE_D_EMPLOI) {
            return Err(FauteDeSource::HorsDeLaReference);
        }
        if s.contains('#') != fichier.ends_with(".jsonl.gz") {
            return Err(FauteDeSource::CodeDEntrepot);
        }
        Ok(CheminDepose(s))
    }
}

impl CheminDepose {
    /// Le chemin tel qu'ecrit.
    pub fn texte(&self) -> &str {
        &self.0
    }
    /// Le fichier depose, sans le code d'entrepot.
    pub fn fichier(&self) -> &str {
        sans_code(&self.0)
    }
    /// Le code d'un entrepot de la DILA (`travail`, `3090`), si c'en est un.
    pub fn code(&self) -> Option<&str> {
        self.0.split_once('#').map(|(_, c)| c)
    }
}

/// Qui a ecrit le texte : sa force se lit la (la loi l'emporte sur la convention, la convention etendue sur la plaquette
/// de l'organisme qui collecte).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Emetteur {
    /// Codes, lois, decrets, arretes, ordonnances (Legifrance).
    Loi,
    /// La convention collective et ses accords (les partenaires sociaux ; KALI, BOCC).
    Convention,
    /// Le Bulletin officiel de la securite sociale.
    Boss,
    /// Le ministere du travail : ses circulaires, au Bulletin officiel du ministere.
    Ministere,
    /// La DGFiP : BOFiP, notices des formulaires.
    Dgfip,
    Urssaf,
    Unedic,
    FranceTravail,
    CourDesComptes,
    /// La caisse du Regime local d'Alsace-Moselle.
    RegimeLocal,
    AgircArrco,
    Audiens,
    Afdas,
    CascSvp,
    /// Le GIP-MDS : la norme DSN et son cahier technique.
    GipMds,
    NetEntreprises,
    /// Le mode d'emploi du logiciel : ce que PuppetPaie fait, pas une regle.
    Puppetpaie,
}

/// Un texte depose, l'endroit, et ce qu'on y lit a l'identique.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub fichier: CheminDepose,
    pub emetteur: Emetteur,
    pub article: Option<String>,
    pub citation: String,
    /// L'annexe du reglement d'assurance chomage qui porte l'article (« X », « VIII »).
    pub annexe: Option<crate::emplois::AnnexeChomage>,
}

/// Ce qui fonde une valeur : les textes d'un meme emetteur (`Texte`), ou des textes d'emetteurs distincts qui convergent,
/// avec le raisonnement qui les joint. Dans un fichier, une source seule s'ecrit a plat (`{fichier, emetteur, article,
/// citation}`), plusieurs textes d'un emetteur `{sources}`, une convergence `{sources, raisonnement}`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(try_from = "FondementLu")]
pub enum Fondement {
    Texte(Vec<Source>),
    Convergence { sources: Vec<Source>, raisonnement: String },
}

/// La forme ecrite d'un fondement, avant de savoir lequel c'est.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FondementLu {
    fichier: Option<CheminDepose>,
    emetteur: Option<Emetteur>,
    article: Option<String>,
    citation: Option<String>,
    annexe: Option<crate::emplois::AnnexeChomage>,
    sources: Option<Vec<Source>>,
    raisonnement: Option<String>,
}

impl TryFrom<FondementLu> for Fondement {
    type Error = FauteDeSource;
    fn try_from(l: FondementLu) -> Result<Self, FauteDeSource> {
        match (l.sources, l.raisonnement) {
            (Some(sources), Some(raisonnement)) if l.fichier.is_none() && l.emetteur.is_none() && l.article.is_none() && l.citation.is_none() => {
                Ok(Fondement::Convergence { sources, raisonnement })
            }
            (Some(sources), None) if l.fichier.is_none() && l.emetteur.is_none() && l.article.is_none() && l.citation.is_none() && !sources.is_empty() => {
                Ok(Fondement::Texte(sources))
            }
            (None, None) => match (l.fichier, l.emetteur, l.citation) {
                (Some(fichier), Some(emetteur), Some(citation)) => Ok(Fondement::Texte(vec![Source { fichier, emetteur, article: l.article, citation, annexe: l.annexe }])),
                _ => Err(FauteDeSource::SourceIncomplete),
            },
            _ => Err(FauteDeSource::ConvergenceMelee),
        }
    }
}

impl Fondement {
    /// Les textes qui fondent la valeur.
    pub fn sources(&self) -> Vec<&Source> {
        match self {
            Fondement::Texte(sources) | Fondement::Convergence { sources, .. } => sources.iter().collect(),
        }
    }

    /// LE TEXTE QUI CITE LE NOMBRE `ecrit` (un montant tel que les textes l'impriment, `socle::texte::chiffres_en_euros` :
    /// « 135,54 », « 2 094,16 ») : la premiere source dont la citation le porte entier, aux espaces insecables pres (ni
    /// « 8,70 » dans « 98,70 », ni « 98,7 » dans « 98,70 », ni « 234,00 » dans « 1 234,00 » : une espace, ordinaire ou
    /// insecable, precedee d'un groupe d'un a trois chiffres qui n'est pas la fin d'un autre montant, separe des milliers).
    /// Deux montants d'une ligne de tableau (« 135,54 120,67 ») restent deux montants. Ce qui tient un montant affiche
    /// au texte qu'on affiche avec lui (`engagement::minima::ce_que_fixe_la_convention`).
    pub fn qui_cite(&self, ecrit: &str) -> Option<&Source> {
        let ecrit = ecrit.replace(['\u{a0}', '\u{202f}'], " ");
        self.sources().into_iter().find(|s| {
            let citation = s.citation.replace(['\u{a0}', '\u{202f}'], " ");
            citation.match_indices(ecrit.as_str()).any(|(i, _)| {
                let mut avant = citation[..i].chars().rev().peekable();
                let juste_avant = avant.next();
                let apres = citation[i + ecrit.len()..].chars().next();
                let colle = juste_avant.is_some_and(|c| c.is_ascii_digit() || c == ',');
                // Le groupe de chiffres avant l'espace, et ce qui le precede.
                let mut groupe: usize = 0;
                while avant.next_if(char::is_ascii_digit).is_some() {
                    groupe += 1;
                }
                let millier = juste_avant == Some(' ') && (1..=3).contains(&groupe) && !avant.peek().is_some_and(|c| *c == ',' || *c == '.');
                !colle && !millier && !apres.is_some_and(|c| c.is_ascii_digit())
            })
        })
    }
}

/// UN OBJET D'UN FICHIER LIVRE ET SES NOTES (atelier/AGENTS.md : `source`, `note`, `quoi`) : elles ne se lisent pas au
/// calcul. La forme est stricte (`deny_unknown_fields`) et sa source, un `Fondement`, se verifie au test des sources.
macro_rules! forme_notee {
    ($(#[$m:meta])* pub struct $nom:ident { $($corps:tt)* }) => {
        $(#[$m])*
        #[derive(Debug, Clone, serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $nom {
            $($corps)*
            pub quoi: Option<String>,
            pub note: Option<String>,
            pub source: Option<$crate::source::Fondement>,
        }
    };
}
pub(crate) use forme_notee;
