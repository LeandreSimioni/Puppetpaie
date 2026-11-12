//! Ce qu'est une fiche : les champs, leur forme, et ce qui
//! les rend valides. Seule la part que l'accueil lit est portee : la
//! description des champs, la forme lisible comblee, la forme declaree qui en
//! decoule (`apercu_derive`) et les anomalies d'une fiche (`verifier_fiche`).
//!
//! La table des champs est `donnees/champs-base.json` : une seule source, lue dans sa forme (`Table`). Chaque champ y
//! porte, en `note`, son commentaire. Ce qu'une fiche recoit avant sa forme (l'ecran, l'import, l'ouverture) est une
//! `Saisie` : chaque case lue dans la forme que sa ligne de la table lui donne (`Lecture`), a la porte
//! (`socle::forme::lire_selon`).
//!
//! **Les majuscules sans accents ne se saisissent PAS.** Les formats
//! declaratifs (AEM, DPAE, DSN) sont a largeur fixe et n'acceptent pas les
//! accents : `salaries.json` porte `MARTIN CLAIRE`, `AVALLON`. Mais un
//! contrat imprime « 2 rue des Lilas ». L'humain ecrit « Béjart Duparc » ;
//! c'est le programme qui fabrique `BEJART DUPARC`. Ce module ne corrige rien
//! en silence : il montre la forme declaree a cote de la forme lisible.

use std::sync::OnceLock;

use serde::de::{DeserializeSeed, Deserializer, Error as _, IgnoredAny, MapAccess, SeqAccess, Unexpected, Visitor};
use serde::{Deserialize, Serialize, Serializer};

use droit::fiche::{CaseDeFiche, CaseDePiece, CaseEntreprise, CasePieceEntreprise, CasePieceSalarie, CaseSalarie, CaseSpectacle, Declaree, NomDeCase, NomDeFiche, Piece};
use socle::date::Jour;
use socle::echec::Echec;
use socle::fichiers::dossier_du_code;
use socle::forme::NombreLu;
use socle::texte::sans_accent;

type R<T> = Result<T, Echec>;

// ------------------------------------------------------------------ la table

/// LA FORME D'UNE CASE (`forme` dans `champs-base.json`) : ce qu'elle porte, et ce qu'on en verifie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Forme {
    Texte,
    Officiel,
    Affichage,
    Chiffres,
    Date,
    Email,
    Montant,
    Choix,
    Pays,
    Booleen,
    CodeAtmp,
    Structure,
    Histoire,
}

impl Forme {
    /// Le libelle de la forme, que la description des fiches donne a l'ecran.
    pub fn libelle(self) -> &'static str {
        match self {
            Forme::Texte => "texte libre",
            Forme::Officiel => "MAJUSCULES SANS ACCENTS (format declaratif a largeur fixe)",
            Forme::Affichage => "forme lisible, jamais declaree",
            Forme::Chiffres => "chiffres uniquement",
            Forme::Date => "AAAA-MM-JJ",
            Forme::Email => "adresse de courriel",
            Forme::Montant => "nombre decimal",
            Forme::Choix => "une valeur de la liste",
            Forme::Pays => "un pays de la liste (code ISO a deux lettres)",
            Forme::Booleen => "oui ou non",
            Forme::CodeAtmp => "un code risque AT/MP de l'arrete de l'annee",
            Forme::Structure => "sous-objet ou liste : edition libre",
            Forme::Histoire => "ce que ce champ valait avant, et jusqu'a quand",
        }
    }

    /// Une case de cette forme porte-t-elle un nombre ? Un montant, des chiffres (l'annee d'un taux AT/MP), un texte
    /// qu'une fiche ecrit en nombre (les conges a la reprise, `droit::fiche::CongesReprise`).
    fn admet_un_nombre(self) -> bool {
        matches!(self, Forme::Texte | Forme::Chiffres | Forme::Montant)
    }
}

/// Ce que l'ecran dit de chaque valeur d'une liste de choix (`libelles`, `""` : la ligne vide), dans l'ordre de la table.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Libelles(#[serde(serialize_with = "socle::ecriture::en_ordre")] pub Vec<(String, String)>);

impl<'de> Deserialize<'de> for Libelles {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        socle::forme::en_ordre(d).map(Libelles)
    }
}

impl Libelles {
    /// Le libelle de la valeur `valeur`, s'il est dit.
    pub fn de(&self, valeur: &str) -> Option<&str> {
        self.0.iter().find(|(k, _)| k == valeur).map(|(_, l)| l.as_str())
    }
}

/// UNE CONDITION DE `champs-base.json` (`si`, `si_entreprise`, `exige_si`) : chaque cle de la fiche vaut l'une des valeurs
/// listees, `"*"` admettant toute valeur non vide (et pas `false`) ; une liste de conditions est remplie quand l'une l'est.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Condition {
    Toutes(#[serde(serialize_with = "socle::ecriture::en_ordre")] Vec<(NomDeCase, Vec<String>)>),
    UneDe(Vec<Condition>),
}

/// LE CHEMIN D'UNE CASE DANS SA STRUCTURE (`champs` d'une ligne de la table) : `taux.t1_cadre`, la case `t1_cadre` du
/// bloc `taux`. Chaque nom est celui d'une case declaree (`NomDeCase`) ; un chemin que la forme de sa structure ne porte
/// pas refuse la lecture de la table (`ranger`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chemin {
    pub devant: Vec<NomDeCase>,
    pub nom: NomDeCase,
}

impl Chemin {
    /// Le chemin d'une case seule, hors de tout bloc.
    pub fn de(nom: impl Into<NomDeCase>) -> Chemin {
        Chemin { devant: Vec::new(), nom: nom.into() }
    }

    /// La case, si le chemin n'a qu'elle.
    pub fn seul(&self) -> Option<NomDeCase> {
        self.devant.is_empty().then_some(self.nom)
    }

    /// Ses noms, du bloc a la case.
    pub fn noms(&self) -> impl Iterator<Item = NomDeCase> + '_ {
        self.devant.iter().copied().chain(std::iter::once(self.nom))
    }

    /// Le chemin de la case `nom` sous celui-ci.
    pub fn sous(&self, nom: NomDeCase) -> Chemin {
        Chemin { devant: self.noms().collect(), nom }
    }
}

impl std::fmt::Display for Chemin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for n in &self.devant {
            write!(f, "{}.", n)?;
        }
        write!(f, "{}", self.nom)
    }
}

/// LE CHEMIN D'UNE CASE DANS SA FICHE : son champ (une case de ses pieces ou non), et, s'il est une structure, le chemin
/// de la case dans elle (`signataire.fonction`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheminDeFiche {
    pub champ: CaseDeFiche,
    pub dans: Option<Chemin>,
}

impl CheminDeFiche {
    /// Le chemin du champ seul.
    pub fn de(champ: impl Into<CaseDeFiche>) -> CheminDeFiche {
        CheminDeFiche { champ: champ.into(), dans: None }
    }

    /// Le champ, si le chemin n'a que lui.
    pub fn seul(&self) -> Option<CaseDeFiche> {
        self.dans.is_none().then_some(self.champ)
    }
}

impl std::fmt::Display for CheminDeFiche {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.dans {
            Some(d) => write!(f, "{}.{}", self.champ, d),
            None => write!(f, "{}", self.champ),
        }
    }
}

impl Serialize for Chemin {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Chemin {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let t = String::deserialize(d)?;
        let mut noms = Vec::new();
        for n in t.split('.') {
            noms.push(NomDeCase::lire(n).ok_or_else(|| D::Error::custom(format!("la case « {} » n'existe dans aucune fiche", t)))?);
        }
        let Some(nom) = noms.pop() else { return Err(D::Error::custom("un chemin de case vide")) };
        Ok(Chemin { devant: noms, nom })
    }
}

impl<'de> Deserialize<'de> for Condition {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Lue;
        impl<'de> Visitor<'de> for Lue {
            type Value = Condition;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("une condition (un objet) ou une liste de conditions")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> Result<Condition, A::Error> {
                let mut out: Vec<(NomDeCase, Vec<String>)> = Vec::new();
                while let Some((k, v)) = m.next_entry::<NomDeCase, Vec<String>>()? {
                    if out.iter().any(|(x, _)| *x == k) {
                        return Err(A::Error::custom(format!("la clé {} est répétée", k)));
                    }
                    out.push((k, v));
                }
                Ok(Condition::Toutes(out))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut s: A) -> Result<Condition, A::Error> {
                let mut out = Vec::new();
                while let Some(c) = s.next_element::<Condition>()? {
                    out.push(c);
                }
                Ok(Condition::UneDe(out))
            }
        }
        d.deserialize_any(Lue)
    }
}

/// La valeur d'une case comme la page la compare (`String(v ?? "").trim()`) : vide, un mot, ou une structure.
enum VueDeLaPage {
    Vide,
    Mot(String),
    Structure,
}

fn comme_la_page(v: Option<&Valeur>) -> VueDeLaPage {
    match v {
        None => VueDeLaPage::Vide,
        Some(Valeur::Texte(s)) => VueDeLaPage::Mot(s.trim().to_string()),
        Some(Valeur::OuiNon(b)) => VueDeLaPage::Mot(b.to_string()),
        Some(Valeur::Nombre(n)) => VueDeLaPage::Mot(n.to_string()),
        Some(Valeur::Mots(_) | Valeur::Bloc(_) | Valeur::Lignes(_)) => VueDeLaPage::Structure,
    }
}

impl Condition {
    /// LA CONDITION REMPLIE PAR LA FICHE `donnees` : la meme regle que la page applique pour replier un champ ; un manque de
    /// « Pour demarrer » et un refus de la DSN se jugent sur ce que l'ecran montre.
    pub fn remplie(&self, donnees: &Saisie) -> bool {
        match self {
            Condition::UneDe(l) => l.iter().any(|c| c.remplie(donnees)),
            Condition::Toutes(l) => l.iter().all(|(k, vals)| {
                let admet = |t: &str| vals.iter().any(|x| x == t);
                match comme_la_page(donnees.case(*k)) {
                    VueDeLaPage::Vide => admet(""),
                    VueDeLaPage::Mot(v) => admet(&v) || (admet("*") && !v.is_empty() && v != "false"),
                    VueDeLaPage::Structure => admet("*"),
                }
            }),
        }
    }
}

/// Une condition absente est remplie : le champ vaut pour toute fiche.
pub fn condition_remplie(si: Option<&Condition>, donnees: &Saisie) -> bool {
    si.is_none_or(|c| c.remplie(donnees))
}

/// UNE CASE DU TIROIR « Avancé » (`avance` de `champs-base.json`) : un cas particulier, et ce qui y est le cas
/// ordinaire. Le tiroir s'ouvre de lui-meme quand une de ses cases s'en ecarte (`Champ::ecart`) ; une case sans `avance`
/// est sur le chemin simple.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Avance {
    /// Toute valeur dite s'ecarte du cas ordinaire, qui est de n'en rien dire.
    CasParticulier,
    /// Toujours remplie (ce que le metier remplit, un matricule) : sa valeur n'est jamais un ecart.
    ToujoursRemplie,
    /// L'organisme que le code postal de la compagnie donne est le cas ordinaire (`declarations::urssaf`).
    DuCodePostal,
    /// Ces valeurs sont le cas ordinaire.
    Ordinaire(Vec<String>),
}

/// `Champ` : une ligne de la table.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Champ {
    pub cle: CaseDeFiche,
    pub libelle: String,
    pub forme: Forme,
    pub aide: String,
    pub longueur: Option<i64>,
    pub exemple: String,
    pub choix: Vec<String>,
    pub declare: bool,
    pub derive_de: Option<CaseDeFiche>,
    pub rubrique: Option<String>,
    /// LA PIECE DE LA CASE, tiree de sa forme (`droit::fiche::CaseDeFiche::piece`), jamais de la table ; `None` : tout
    /// calcul la lit. Posee a la lecture de la table (`table`).
    #[serde(skip)]
    pub piece: Option<Piece>,
    /// CE QUE SA FORME DECLARE (`droit::fiche::FormeDeCase`) : ses cases, s'il est une structure, que la saisie lit. Posee a
    /// la lecture de la table ; `None` pour la ligne de l'histoire, qui n'est pas une case.
    #[serde(skip)]
    pub declaree: Option<Declaree<CaseDeFiche>>,
    pub si: Option<Condition>,
    pub si_etablissements: bool,
    pub si_entreprise: Option<Condition>,
    /// La case est un cas particulier, rangee sous « Avancé », et ce qui y est le cas ordinaire (`avance`).
    pub avance: Option<Avance>,
    /// Quand la compagnie DOIT le remplir (`exige_si`, meme forme que `si`) : `Champ::exige`.
    pub exige_si: Option<Condition>,
    pub ecran: bool,
    /// Les actes qui ne se font pas sans lui (`exige_par` : `dsn`, `dpae`, `contrat`, `bulletin`, `certificat`, `paie`) :
    /// LA SEULE LISTE de ce qu'un acte exige d'une fiche, que « Pour demarrer » (`demarrage::manques`) et l'acte
    /// (`refus_des_manques`) lisent.
    pub exige_par: Option<Vec<String>>,
    /// Les gestes qu'il sert sans les bloquer (`utile_pour`, meme vocabulaire que `exige_par`) : sans lui, ils se font
    /// autrement, ce que dit `sinon`. « Pour demarrer » le dit : « utile pour ... ; sinon, ... ».
    pub utile_pour: Option<Vec<String>>,
    /// Ce qui se passe sans lui, quand rien ne se refuse (`sinon`).
    pub sinon: Option<String>,
    /// Les cases d'une structure (`champs`), decrites chacune ; une autre cle s'y refuse. Une case peut etre un chemin
    /// (`taux.t1_cadre` : la case `t1_cadre` du bloc `taux`).
    pub champs: Option<Vec<Case>>,
    /// Ce que l'ecran dit de chaque valeur d'une liste de choix (`libelles`, `""` : la ligne vide).
    pub libelles: Option<Libelles>,
    /// Une structure en lignes (`ligne` : le nom d'une ligne, « établissement ») ; absente : un seul objet.
    pub ligne: Option<String>,
    /// Le commentaire du champ.
    pub note: Option<String>,
}

/// UNE CASE D'UNE STRUCTURE (`champs` d'une ligne de la table) : ce qu'elle porte et comment l'ecran la montre. Ce qui
/// n'a de sens que pour un champ de la fiche (ce qu'il declare, sa case a cocher, les permanents) n'y existe pas.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub cle: Chemin,
    pub libelle: String,
    pub forme: Forme,
    pub ecran: bool,
    pub aide: Option<String>,
    pub exemple: Option<String>,
    pub derive_de: Option<Chemin>,
    pub exige_par: Option<Vec<String>>,
    pub choix: Option<Vec<String>>,
    pub libelles: Option<Libelles>,
    /// Les choix se montrent en boutons a cocher, chacun une phrase entiere (`boutons`).
    pub boutons: Option<bool>,
    /// La case ne se montre que si la configuration le dit (`si_configuration` : un drapeau de la route `etat`).
    pub si_configuration: Option<String>,
}

/// Ce qu'un champ et une case d'une structure ont en commun : leur nom, et la case dont ils se deduisent.
trait Ligne {
    type Nom: PartialEq;
    fn cle(&self) -> &Self::Nom;
    fn derive_de(&self) -> Option<&Self::Nom>;
    /// La case se deduit d'une forme lisible.
    fn calcule(&self) -> bool {
        self.derive_de().is_some()
    }
}

impl Ligne for Champ {
    type Nom = CaseDeFiche;
    fn cle(&self) -> &CaseDeFiche {
        &self.cle
    }
    fn derive_de(&self) -> Option<&CaseDeFiche> {
        self.derive_de.as_ref()
    }
}

impl Ligne for Case {
    type Nom = Chemin;
    fn cle(&self) -> &Chemin {
        &self.cle
    }
    fn derive_de(&self) -> Option<&Chemin> {
        self.derive_de.as_ref()
    }
}

/// La case que l'ecran montre pour `c` : celle dont il se deduit, s'il se deduit.
fn montre<'a, L: Ligne>(liste: &'a [L], c: &'a L) -> &'a L {
    match c.derive_de().and_then(|d| liste.iter().find(|x| x.cle() == d)) {
        Some(source) => source,
        None => c,
    }
}

impl Champ {
    /// `Champ.calcule` : le champ se deduit d'une forme lisible.
    pub fn calcule(&self) -> bool {
        Ligne::calcule(self)
    }

    /// La case que l'ecran montre pour lui : la forme lisible dont il se deduit, ou lui-meme.
    pub fn a_l_ecran(&self) -> CaseDeFiche {
        match self.derive_de {
            Some(source) => source,
            None => self.cle,
        }
    }

    /// Le champ s'applique-t-il a la fiche `donnees` : sa condition `si`, et `exige_si` quand il en a une.
    fn applicable(&self, donnees: &Saisie) -> bool {
        condition_remplie(self.si.as_ref(), donnees) && (self.exige_si.is_none() || self.exige(donnees))
    }

    /// Le champ de la fiche `donnees` est-il A L'ECRAN ? Il se saisit (ni deduit, ni hors de l'ecran), sa condition `si`
    /// est remplie par la fiche, celle de l'entreprise (`si_entreprise`, pour la fiche d'un salarie ou d'un spectacle :
    /// `entreprise`) aussi, sa piece est ouverte pour elle (`ouverte(piece, cle)` : la fiche peut avoir un calcul qui la
    /// lit, `routes::fiches::montres`) et la compagnie a plusieurs etablissements s'il les nomme (`si_etablissements`).
    /// La page montre ce que cette fonction dit (`routes::fiches::montres`), et rien d'autre.
    pub fn montre(&self, donnees: &Saisie, entreprise: Option<&Saisie>, ouverte: impl Fn(Piece, CaseDeFiche) -> bool, etablissements: bool) -> bool {
        self.ecran
            && !self.calcule()
            && condition_remplie(self.si.as_ref(), donnees)
            && entreprise.is_none_or(|e| condition_remplie(self.si_entreprise.as_ref(), e))
            && self.piece.is_none_or(|p| ouverte(p, self.cle))
            && !(self.si_etablissements && !etablissements)
    }

    /// La case du tiroir « Avancé » s'ecarte-t-elle du cas ordinaire dans la fiche `donnees` (`Avance`) ? `du_code_postal` :
    /// l'organisme Urssaf que le code postal de la compagnie donne. Vide ou « non » n'est jamais un ecart.
    pub fn ecart(&self, donnees: &Saisie, du_code_postal: Option<&str>) -> bool {
        // Une case hors du tiroir n'a pas de cas ordinaire dont s'ecarter.
        self.avance.as_ref().is_some_and(|a| self.ecart_de(a, donnees, du_code_postal))
    }

    fn ecart_de(&self, a: &Avance, donnees: &Saisie, du_code_postal: Option<&str>) -> bool {
        let mot = match comme_la_page(donnees.valeur(self.cle)) {
            VueDeLaPage::Vide => return false,
            VueDeLaPage::Structure => return donnees.valeur(self.cle).is_some_and(Valeur::dite) && *a != Avance::ToujoursRemplie,
            VueDeLaPage::Mot(m) if m.is_empty() || m == "false" => return false,
            VueDeLaPage::Mot(m) => m,
        };
        match a {
            Avance::CasParticulier => true,
            Avance::ToujoursRemplie => false,
            Avance::DuCodePostal => Some(mot.as_str()) != du_code_postal,
            Avance::Ordinaire(valeurs) => !valeurs.iter().any(|v| v.eq_ignore_ascii_case(&mot)),
        }
    }

    /// Le champ est-il EXIGE de la compagnie `donnees` ? Sa condition `exige_si` de `champs-base.json`, la seule : la
    /// fiche (« Pour demarrer », `demarrage::manques`) et la declaration qui le lit (la DSN, pour la licence) la lisent
    /// toutes deux. Un champ sans `exige_si` n'est exige de personne par cette voie. Un champ exige est toujours montre
    /// (`tests/routes/demarrage.rs` le verifie sur toutes les conventions).
    pub fn exige(&self, donnees: &Saisie) -> bool {
        self.exige_si.as_ref().is_some_and(|e| e.remplie(donnees)) && condition_remplie(self.si.as_ref(), donnees)
    }

    /// Ce que la valeur saisie de ce champ doit respecter.
    fn regle(&self) -> Regle<'_> {
        Regle {
            cle: self.cle,
            libelle: &self.libelle,
            forme: self.forme,
            rubrique: self.rubrique.as_deref().filter(|r| !r.is_empty()),
            choix: Some(self.choix.as_slice()).filter(|c| !c.is_empty()),
            longueur: self.longueur.filter(|l| *l != 0),
        }
    }

    /// Une ligne de `description` : ce que l'interface consomme.
    fn description(&self) -> Description<'_> {
        Description {
            cle: self.cle,
            libelle: &self.libelle,
            forme: self.forme,
            aide: &self.aide,
            longueur: self.longueur,
            exemple: &self.exemple,
            choix: &self.choix,
            declare: self.declare,
            forme_libelle: match self.champs {
                Some(_) => "sous-objet : ses cases sont dans `champs`, une autre cle se refuse",
                None => self.forme.libelle(),
            },
            calcule: self.calcule(),
            derive_de: self.derive_de,
            rubrique: self.rubrique.as_deref(),
            piece: self.piece,
            si: self.si.as_ref(),
            si_etablissements: self.si_etablissements,
            si_entreprise: self.si_entreprise.as_ref(),
            avance: self.avance.as_ref(),
            exige_si: self.exige_si.as_ref(),
            ecran: self.ecran,
            exige_par: self.exige_par.as_deref(),
            bloque: self.exige_par.as_deref().and_then(actes_en_clair),
            utile_pour: self.utile_pour.as_deref(),
            utile: self.utile_pour.as_deref().and_then(actes_en_clair),
            sinon: self.sinon.as_deref(),
            champs: self.champs.as_ref().map(|l| l.iter().map(Case::description).collect()),
            libelles: self.libelles.as_ref(),
            ligne: self.ligne.as_deref(),
            // Le bouton qui ajoute une ligne a une liste de cases (un etablissement).
            ajouter_dit: self.ligne.as_ref().map(|l| format!("+ Ajouter un {}", l)),
        }
    }
}

impl Case {
    /// La case se deduit d'une forme lisible.
    pub fn calcule(&self) -> bool {
        Ligne::calcule(self)
    }

    fn regle(&self) -> Regle<'_> {
        Regle {
            cle: self.cle.nom.into(),
            libelle: &self.libelle,
            forme: self.forme,
            rubrique: None,
            choix: self.choix.as_deref().filter(|c| !c.is_empty()),
            longueur: None,
        }
    }

    fn description(&self) -> DescriptionDeCase<'_> {
        DescriptionDeCase {
            cle: &self.cle,
            libelle: &self.libelle,
            forme: self.forme,
            forme_libelle: self.forme.libelle(),
            calcule: self.calcule(),
            derive_de: self.derive_de.as_ref(),
            ecran: self.ecran,
            aide: self.aide.as_deref(),
            exemple: self.exemple.as_deref(),
            choix: self.choix.as_deref(),
            exige_par: self.exige_par.as_deref(),
            bloque: self.exige_par.as_deref().and_then(actes_en_clair),
            libelles: self.libelles.as_ref(),
            boutons: self.boutons,
            si_configuration: self.si_configuration.as_deref(),
        }
    }
}

/// LA DESCRIPTION D'UN CHAMP, telle que la page la lit (`fiche.js`) : sa ligne de la table, et ce qui s'en deduit (le
/// libelle de sa forme, s'il se calcule, ce qu'il bloque ou sert, en clair).
#[derive(Debug, Clone, Serialize)]
pub struct Description<'a> {
    pub cle: CaseDeFiche,
    pub libelle: &'a str,
    pub forme: Forme,
    pub aide: &'a str,
    pub longueur: Option<i64>,
    pub exemple: &'a str,
    pub choix: &'a [String],
    pub declare: bool,
    pub forme_libelle: &'static str,
    pub calcule: bool,
    /// La rubrique part a l'ecran : un compte rendu d'organisme parle de « S21.G00.30.010 en anomalie », pas de
    /// « commune ».
    pub derive_de: Option<CaseDeFiche>,
    pub rubrique: Option<&'a str>,
    /// La piece de la case (`Champ::piece`) : elle reste dans la description, l'ecran replie le bloc dont aucune case
    /// n'est montree (`montres`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub piece: Option<Piece>,
    pub si: Option<&'a Condition>,
    pub si_etablissements: bool,
    pub si_entreprise: Option<&'a Condition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avance: Option<&'a Avance>,
    pub exige_si: Option<&'a Condition>,
    pub ecran: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exige_par: Option<&'a [String]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bloque: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub utile_pour: Option<&'a [String]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub utile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sinon: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub champs: Option<Vec<DescriptionDeCase<'a>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub libelles: Option<&'a Libelles>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ligne: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ajouter_dit: Option<String>,
}

/// La description d'une case d'une structure (`champs` d'une `Description`).
#[derive(Debug, Clone, Serialize)]
pub struct DescriptionDeCase<'a> {
    pub cle: &'a Chemin,
    pub libelle: &'a str,
    pub forme: Forme,
    pub forme_libelle: &'static str,
    pub calcule: bool,
    pub derive_de: Option<&'a Chemin>,
    pub ecran: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aide: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exemple: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub choix: Option<&'a [String]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exige_par: Option<&'a [String]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bloque: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub libelles: Option<&'a Libelles>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boutons: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub si_configuration: Option<&'a str>,
}

/// La table des trois fiches.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Table {
    entreprise: Vec<Champ>,
    salarie: Vec<Champ>,
    spectacle: Vec<Champ>,
}

static TABLE: OnceLock<R<Table>> = OnceLock::new();

/// LES CHEMINS DES CASES QU'UNE STRUCTURE DECLARE (`droit::fiche::Declaree`) : chaque case, ou celles de son bloc.
fn feuilles(declarees: &[Declaree], devant: &[NomDeCase], out: &mut Vec<Chemin>) {
    for d in declarees {
        if d.cases.is_empty() {
            out.push(Chemin { devant: devant.to_vec(), nom: d.cle });
        } else {
            let plus: Vec<NomDeCase> = devant.iter().copied().chain(std::iter::once(d.cle)).collect();
            feuilles(&d.cases, &plus, out);
        }
    }
}

/// LA TABLE LUE CONTRE LA FORME DES FICHES (`droit::fiche`) : chaque ligne d'une fiche est une case que sa forme declare,
/// chaque case qu'elle declare a sa ligne (la ligne de l'histoire en plus, qui n'est pas une case), et de meme les cases
/// d'une structure que la ligne decrit (`champs`) ; sinon la table ne se lit pas. CHAQUE CASE DANS SA PIECE : celle que sa
/// forme lui donne, et nulle autre : la table n'a pas de mot pour la dire (`deny_unknown_fields`).
fn ranger(mut t: Table) -> R<Table> {
    let fiches: [(NomDeFiche, &mut Vec<Champ>, Vec<Declaree<CaseDeFiche>>); 3] = [
        (NomDeFiche::Entreprise, &mut t.entreprise, droit::fiche::ChampsEntreprise::cases()),
        (NomDeFiche::Salarie, &mut t.salarie, droit::fiche::ChampsSalarie::cases()),
        (NomDeFiche::Spectacle, &mut t.spectacle, droit::fiche::ChampsSpectacle::cases()),
    ];
    let faute = |ou: String| socle::echec::livree("champs-base.json", &ou);
    for (nom, champs, declarees) in fiches {
        for c in champs.iter_mut().filter(|c| c.cle != CaseDeFiche::Case(NomDeCase::HISTORIQUE)) {
            let Some(d) = declarees.iter().find(|d| d.cle == c.cle) else {
                return Err(faute(format!("{}.{} : la fiche {} ne declare pas cette case", nom, c.cle, nom)));
            };
            if let Some(cases) = c.champs.as_deref().filter(|l| !l.is_empty()) {
                let mut declares = Vec::new();
                feuilles(&d.cases, &[], &mut declares);
                if let Some(s) = cases.iter().find(|s| !declares.contains(&s.cle)) {
                    return Err(faute(format!("{}.{}.{} : la structure ne declare pas cette case", nom, c.cle, s.cle)));
                }
                if let Some(x) = declares.iter().find(|x| !cases.iter().any(|s| s.cle == **x)) {
                    return Err(faute(format!("{}.{}.{} : une case de la structure sans sa ligne", nom, c.cle, x)));
                }
            }
            c.piece = c.cle.piece();
            c.declaree = Some(d.clone());
        }
        if let Some(d) = declarees.iter().find(|d| !champs.iter().any(|c| c.cle == d.cle)) {
            return Err(faute(format!("{}.{} : une case de la fiche sans sa ligne", nom, d.cle)));
        }
    }
    Ok(t)
}

/// Les champs de la fiche `nom` (`entreprise`, `salarie`, `spectacle`), tels que la table livree les decrit.
fn table(nom: NomDeFiche) -> R<&'static [Champ]> {
    let chemin = dossier_du_code().join("champs-base.json");
    let t = TABLE
        .get_or_init(|| socle::forme::lire_fichier::<Table>(&chemin, "La table des cases des fiches, livrée, ne se lit pas : le signaler au développeur.").and_then(ranger))
        .as_ref()
        .map_err(Echec::clone)?;
    // Lu une fois par processus (le code livre ne change pas sous la fenetre), mais note a chaque usage : un calcul
    // suivi dans un processus deja chaud a les memes entrees qu'un processus neuf.
    socle::fichiers::noter_si_besoin(&chemin);
    Ok(match nom {
        NomDeFiche::Entreprise => &t.entreprise,
        NomDeFiche::Salarie => &t.salarie,
        NomDeFiche::Spectacle => &t.spectacle,
    })
}

/// Les champs d'une fiche (`entreprise`, `salarie`, `spectacle`).
pub fn fiche(nom: NomDeFiche) -> R<Vec<Champ>> {
    Ok(table(nom)?.to_vec())
}

/// LES FORMES LISIBLES DES STRUCTURES DE LA FICHE `nom` : les cases d'ou une autre case de leur structure se deduit
/// (`derive_de`). L'ecran les comble ; elles ne se declarent pas.
pub fn formes_lisibles(nom: NomDeFiche) -> R<Vec<CaseDeFiche>> {
    Ok(table(nom)?.iter().flat_map(|c| c.champs.iter().flatten()).filter_map(|c| c.derive_de.as_ref().map(|d| d.nom.into())).collect())
}

/// Le champ `cle` de la fiche `nom`, s'il existe.
pub fn champ(nom: NomDeFiche, cle: impl Into<CaseDeFiche>) -> R<Option<Champ>> {
    let cle = cle.into();
    Ok(table(nom)?.iter().find(|c| c.cle == cle).cloned())
}

/// `description(nom_fiche)` : chaque champ, tel que l'interface le lit.
pub fn description(nom: NomDeFiche) -> R<Vec<Description<'static>>> {
    Ok(table(nom)?.iter().map(Champ::description).collect())
}

// ------------------------------------------------------------------ la saisie

/// UNE SAISIE : ce que l'ecran, l'import ou l'ouverture proposent pour une fiche, avant sa forme : une table `case connue
/// -> ce que la personne a tape`. Sa cle est une case que la forme declare (`CaseDeFiche`) : une case que la table ne
/// decrit pas refuse la saisie a la porte (`Lecture`). Chaque case y est lue dans la forme que sa ligne de la table lui
/// donne ; ce qui est tape n'est pas encore une valeur de la fiche (`Valeur`) : la verification la lit case par case
/// (`verifier_fiche`). Une case vide (absente ou nulle) n'y est pas. Les notes de la fiche
/// (`droit::fiche::est_une_note`) sont des textes a part.
///
/// UNE CASE D'UNE PIECE NE SE LIT PAS PAR SON NOM : `case` (et ce qui la lit) ne prend qu'un `NomDeCase`, qu'aucune case
/// d'une piece n'est ; une piece se lit par `piece`, qui prend une `CaseDePiece`. La mecanique de toutes les cases (la
/// table, ligne par ligne) passe par `valeur`, qui ne lit l'une que par l'autre.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Saisie {
    cases: Vec<(CaseDeFiche, Valeur)>,
    notes: Vec<(String, String)>,
}

/// LA VALEUR D'UNE CASE SAISIE, dans la forme que sa ligne lui donne : un texte (ce qui se tape, tel quel : la forme dit
/// ce qu'on en verifie), un nombre (un montant, des chiffres), oui ou non (`booleen`), les mots d'une structure que la
/// table ne decrit pas case par case (les jours travailles, les autres noms d'un spectacle), les cases d'une structure,
/// ou ses lignes.
#[derive(Debug, Clone, PartialEq)]
pub enum Valeur {
    Texte(String),
    Nombre(NombreLu),
    OuiNon(bool),
    Mots(Vec<String>),
    Bloc(Saisie),
    Lignes(Vec<Saisie>),
}

/// UNE SAISIE S'ECRIT comme elle se lit : ses cases, puis ses notes ; une case que la table ne decrit pas ne s'ecrit pas
/// (sa valeur ne se garde pas, et une fiche la refuse).
impl Serialize for Saisie {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut m = s.serialize_map(Some(self.cases.len() + self.notes.len()))?;
        for (k, v) in &self.cases {
            m.serialize_entry(k, v)?;
        }
        for (k, t) in &self.notes {
            m.serialize_entry(k, t)?;
        }
        m.end()
    }
}

impl Serialize for Valeur {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Valeur::Texte(t) => s.serialize_str(t),
            Valeur::Nombre(n) => n.serialize(s),
            Valeur::OuiNon(b) => s.serialize_bool(*b),
            Valeur::Mots(l) => l.serialize(s),
            Valeur::Bloc(b) => b.serialize(s),
            Valeur::Lignes(l) => l.serialize(s),
        }
    }
}

impl Valeur {
    /// La valeur dit-elle quelque chose ? Un texte vide, zero, non, une liste ou une structure vides ne disent rien.
    pub fn dite(&self) -> bool {
        match self {
            Valeur::Texte(t) => !t.is_empty(),
            Valeur::Nombre(n) => !n.est_nul(),
            Valeur::OuiNon(b) => *b,
            Valeur::Mots(l) => !l.is_empty(),
            Valeur::Bloc(s) => s.est_dite(),
            Valeur::Lignes(l) => !l.is_empty(),
        }
    }

    /// Le texte saisi, tel quel (un texte seulement).
    pub fn texte(&self) -> Option<&str> {
        match self {
            Valeur::Texte(t) => Some(t),
            Valeur::Nombre(_) | Valeur::OuiNon(_) | Valeur::Mots(_) | Valeur::Bloc(_) | Valeur::Lignes(_) => None,
        }
    }

    /// Le texte d'une valeur qui en a un : un texte tel quel, un nombre tel qu'il s'ecrit.
    pub fn mot(&self) -> Option<String> {
        match self {
            Valeur::Texte(t) => Some(t.clone()),
            Valeur::Nombre(n) => Some(n.to_string()),
            Valeur::OuiNon(_) | Valeur::Mots(_) | Valeur::Bloc(_) | Valeur::Lignes(_) => None,
        }
    }
}

impl Saisie {
    /// La case `cle`, hors des pieces, si la saisie la dit.
    pub fn case(&self, cle: impl Into<NomDeCase>) -> Option<&Valeur> {
        let cle = CaseDeFiche::Case(cle.into());
        self.cases.iter().find(|(k, _)| *k == cle).map(|(_, v)| v)
    }

    /// LA CASE `cle` D'UNE PIECE, si la saisie la dit : le seul chemin des pieces.
    pub fn piece(&self, cle: impl Into<CaseDePiece>) -> Option<&Valeur> {
        let cle = CaseDeFiche::Piece(cle.into());
        self.cases.iter().find(|(k, _)| *k == cle).map(|(_, v)| v)
    }

    /// La case `cle` d'une ligne de la table, d'une piece ou non : l'une par `case`, l'autre par `piece`.
    pub fn valeur(&self, cle: CaseDeFiche) -> Option<&Valeur> {
        match cle {
            CaseDeFiche::Case(n) => self.case(n),
            CaseDeFiche::Piece(p) => self.piece(p),
        }
    }

    /// La case au bout du chemin `chemin`, de bloc en bloc.
    pub fn au_chemin(&self, chemin: &Chemin) -> Option<&Valeur> {
        let mut ici = self;
        for n in &chemin.devant {
            match ici.case(*n) {
                Some(Valeur::Bloc(b)) => ici = b,
                _ => return None,
            }
        }
        ici.case(chemin.nom)
    }

    /// La case `cle`, si elle dit quelque chose (`Valeur::dite`).
    pub fn dite(&self, cle: impl Into<NomDeCase>) -> Option<&Valeur> {
        self.case(cle).filter(|v| v.dite())
    }

    /// Le texte de la case `cle`, si elle en dit un (`Valeur::mot`).
    pub fn mot(&self, cle: impl Into<NomDeCase>) -> Option<String> {
        self.dite(cle).and_then(Valeur::mot)
    }

    /// Le texte saisi dans la case `cle`, tel quel (un texte seulement).
    pub fn texte(&self, cle: impl Into<NomDeCase>) -> Option<&str> {
        self.case(cle).and_then(Valeur::texte)
    }

    /// La note `cle` de la fiche.
    pub fn note(&self, cle: &str) -> Option<&str> {
        self.notes.iter().find(|(k, _)| k == cle).map(|(_, t)| t.as_str())
    }

    /// Ses notes, dans l'ordre saisi.
    pub fn notes(&self) -> &[(String, String)] {
        &self.notes
    }

    /// La note `cle` vaut `t`.
    pub fn poser_note(&mut self, cle: &str, t: String) {
        match self.notes.iter_mut().find(|(k, _)| k == cle) {
            Some(n) => n.1 = t,
            None => self.notes.push((cle.to_string(), t)),
        }
    }

    /// La saisie porte-t-elle une case ou une note ?
    pub fn est_dite(&self) -> bool {
        !self.cases.is_empty() || !self.notes.is_empty()
    }

    /// Ses cases, dans l'ordre saisi.
    pub fn cases(&self) -> &[(CaseDeFiche, Valeur)] {
        &self.cases
    }

    /// La case `cle`, hors des pieces, a changer.
    pub fn case_mut(&mut self, cle: impl Into<NomDeCase>) -> Option<&mut Valeur> {
        let cle = CaseDeFiche::Case(cle.into());
        self.cases.iter_mut().find(|(k, _)| *k == cle).map(|(_, v)| v)
    }

    /// La case `cle` retiree : vide.
    pub fn retirer(&mut self, cle: impl Into<CaseDeFiche>) {
        let cle = cle.into();
        self.cases.retain(|(k, _)| *k != cle);
    }

    /// La case `cle` vaut `v`.
    pub fn poser(&mut self, cle: impl Into<CaseDeFiche>, v: Valeur) {
        let cle = cle.into();
        match self.cases.iter_mut().find(|(k, _)| *k == cle) {
            Some(c) => c.1 = v,
            None => self.cases.push((cle, v)),
        }
    }

    /// LA FICHE TYPEE `fiche` (`droit::fiche`) DANS LA FORME D'UNE SAISIE de la fiche `nom`, pour etre verifiee comme ce
    /// que l'ecran saisit : ce qu'elle ecrit, relu par la table (`socle::forme::relire_selon`).
    pub fn de_la_fiche<F: Serialize + ?Sized>(nom: NomDeFiche, fiche: &F) -> R<Saisie> {
        socle::forme::relire_selon(fiche, lecture(nom)?, "Une fiche ne se relit pas dans la table de ses cases : le signaler au développeur.")
    }

    /// La saisie, ses formes lisibles comblees (`a_combler`).
    pub fn comblee(&self, nom: NomDeFiche) -> R<Saisie> {
        let mut s = self.clone();
        for c in a_combler(nom, self)? {
            match c.bloc {
                None => s.poser(c.cle, Valeur::Texte(c.texte)),
                Some(b) => {
                    if let Some(Valeur::Bloc(p)) = s.case_mut(b) {
                        p.poser(c.cle, Valeur::Texte(c.texte));
                    }
                }
            }
        }
        Ok(s)
    }
}

/// CE QU'UNE TABLE ATTEND D'UNE CASE : sa forme (une case simple), ses cases (une structure, ou chaque ligne d'une
/// structure en lignes), des mots ou des cases qu'elle ne decrit pas (une structure sans `champs`), ou rien a lire
/// (l'histoire de la fiche, qui se lit et ne se saisit pas).
#[derive(Debug, Clone)]
enum Attendu {
    /// Une valeur de la forme de sa ligne ; un texte saisi se range dans la forme du type de la case
    /// (`Declaree::rangee`), quand elle en declare un.
    Simple(Forme, Option<fn(&str) -> String>),
    Bloc(Lecture),
    Lignes(Lecture),
    /// Une structure que la table ne decrit pas case par case : des mots, ou les cases que sa forme declare.
    Libre(&'static [Declaree]),
    /// Une case d'une telle structure : un texte, un nombre, oui ou non, des mots, ou les cases que sa forme declare.
    Quelconque(&'static Declaree),
    Histoire,
}

impl Attendu {
    /// Le texte saisi `t`, range dans la forme du type de sa case (`Declaree::rangee`) ; tel quel sans type qui le range.
    fn rangee(&self, t: &str) -> String {
        let ranger = match self {
            Attendu::Simple(_, r) => *r,
            Attendu::Quelconque(d) => d.ranger,
            Attendu::Bloc(_) | Attendu::Lignes(_) | Attendu::Libre(_) | Attendu::Histoire => None,
        };
        match ranger {
            Some(r) => r(t),
            None => t.to_string(),
        }
    }
}

/// Le niveau d'une lecture : les champs d'une fiche, les cases d'une structure (et celles que sa forme declare a ce
/// niveau), ou les cases d'une structure que la table ne decrit pas, que sa forme declare.
#[derive(Debug, Clone, Copy)]
enum Niveau {
    Fiche(NomDeFiche, &'static [Champ]),
    Structure(&'static [Case], &'static [Declaree]),
    Libre(&'static [Declaree]),
}

/// LA LECTURE D'UNE SAISIE SELON LA TABLE DE SA FICHE (`socle::forme::lire_selon`) : chaque case dans la forme que sa
/// ligne lui donne, une forme fausse refusee avec son chemin, une case que la table ne decrit pas refusee avec les cases
/// qu'elle connait. `devant` : le chemin d'un bloc dans sa structure (`taux` pour la case `taux.t1_cadre`).
#[derive(Debug, Clone)]
pub struct Lecture {
    niveau: Niveau,
    devant: Vec<NomDeCase>,
}

/// La lecture d'une saisie de la fiche `nom`.
pub fn lecture(nom: NomDeFiche) -> R<Lecture> {
    Ok(Lecture { niveau: Niveau::Fiche(nom, table(nom)?), devant: Vec::new() })
}

/// Les cases que la forme d'une ligne declare (`Champ::declaree`) ; la ligne de l'histoire n'en a pas.
fn sous_cases(c: &'static Champ) -> &'static [Declaree] {
    match &c.declaree {
        Some(d) => &d.cases,
        None => &[],
    }
}

impl Lecture {
    fn sous(&self, niveau: Niveau, devant: Vec<NomDeCase>) -> Lecture {
        Lecture { niveau, devant }
    }

    /// LA CASE `cle` ET CE QU'ON EN ATTEND ; une case que la table ne decrit pas : la phrase qui le dit.
    fn attendu(&self, cle: &str) -> socle::echec::R<(CaseDeFiche, Attendu)> {
        match self.niveau {
            Niveau::Fiche(fiche, champs) => {
                let Some(c) = champs.iter().find(|c| c.cle.nom() == cle) else {
                    let connues: Vec<&str> = champs.iter().filter(|c| !c.calcule()).map(|c| c.cle.nom()).collect();
                    return Err(socle::refus!("la case « {} » n'existe pas dans la fiche {} ; ses cases sont : {}", cle, fiche, connues.join(", ")));
                };
                let a = match (c.forme, c.champs.as_deref(), &c.ligne) {
                    (Forme::Histoire, _, _) => Attendu::Histoire,
                    (Forme::Structure, Some(cases), Some(_)) => Attendu::Lignes(self.sous(Niveau::Structure(cases, sous_cases(c)), Vec::new())),
                    (Forme::Structure, Some(cases), None) => Attendu::Bloc(self.sous(Niveau::Structure(cases, sous_cases(c)), Vec::new())),
                    (Forme::Structure, None, _) => Attendu::Libre(sous_cases(c)),
                    (f, _, _) => Attendu::Simple(f, c.declaree.as_ref().and_then(|d| d.ranger)),
                };
                Ok((c.cle, a))
            }
            Niveau::Structure(cases, declarees) => {
                let inconnue = || socle::refus!("la case « {} » n'existe pas ; ses cases sont : {}", cle, cases_dites(cases));
                let Some(d) = declarees.iter().find(|d| d.cle.nom() == cle) else { return Err(inconnue()) };
                let chemin = Chemin { devant: self.devant.clone(), nom: d.cle };
                match cases.iter().find(|s| s.cle == chemin) {
                    Some(s) => Ok((d.cle.into(), match s.forme {
                        Forme::Structure | Forme::Histoire => Attendu::Libre(&d.cases),
                        f => Attendu::Simple(f, d.ranger),
                    })),
                    None => {
                        let devant: Vec<NomDeCase> = chemin.noms().collect();
                        if cases.iter().any(|s| s.cle.devant.starts_with(&devant)) {
                            Ok((d.cle.into(), Attendu::Bloc(self.sous(Niveau::Structure(cases, &d.cases), devant))))
                        } else {
                            Err(inconnue())
                        }
                    }
                }
            }
            Niveau::Libre(declarees) => match declarees.iter().find(|d| d.cle.nom() == cle) {
                Some(d) => Ok((d.cle.into(), Attendu::Quelconque(d))),
                None => Err(socle::refus!("la case « {} » n'existe pas", cle)),
            },
        }
    }
}

impl<'de> DeserializeSeed<'de> for Lecture {
    type Value = Saisie;
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Saisie, D::Error> {
        d.deserialize_map(self)
    }
}

impl<'de> Visitor<'de> for Lecture {
    type Value = Saisie;
    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("les cases d'une fiche (un objet)")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> Result<Saisie, A::Error> {
        let mut s = Saisie::default();
        while let Some(cle) = m.next_key::<String>()? {
            if matches!(self.niveau, Niveau::Fiche(..)) && droit::fiche::est_une_note(&cle) {
                if let Some(t) = m.next_value::<Option<String>>()? {
                    s.notes.push((cle, t));
                }
                continue;
            }
            let (nom, a) = self.attendu(&cle).map_err(A::Error::custom)?;
            if let Some(v) = m.next_value_seed(a)? {
                s.cases.push((nom, v));
            }
        }
        Ok(s)
    }
}

impl Attendu {
    fn nombre<E: serde::de::Error>(self, n: NombreLu, lu: Unexpected) -> Result<Option<Valeur>, E> {
        match self {
            Attendu::Simple(f, _) if f.admet_un_nombre() => Ok(Some(Valeur::Nombre(n))),
            Attendu::Quelconque(_) => Ok(Some(Valeur::Nombre(n))),
            _ => Err(E::invalid_type(lu, &self)),
        }
    }
}

impl<'de> DeserializeSeed<'de> for Attendu {
    type Value = Option<Valeur>;
    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Option<Valeur>, D::Error> {
        match self {
            Attendu::Histoire => d.deserialize_ignored_any(IgnoredAny).map(|_| None),
            autre => d.deserialize_any(autre),
        }
    }
}

impl<'de> Visitor<'de> for Attendu {
    type Value = Option<Valeur>;
    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(match self {
            Attendu::Simple(Forme::Booleen, _) => "oui ou non (true ou false)",
            Attendu::Simple(forme, _) if forme.admet_un_nombre() => "un texte ou un nombre",
            Attendu::Simple(..) => "un texte",
            Attendu::Bloc(_) => "les cases d'une structure (un objet)",
            Attendu::Lignes(_) => "une liste de lignes",
            Attendu::Libre(_) => "une liste de mots, ou un objet",
            Attendu::Quelconque(_) => "une valeur",
            Attendu::Histoire => "l'histoire de la fiche",
        })
    }
    fn visit_unit<E: serde::de::Error>(self) -> Result<Option<Valeur>, E> {
        Ok(None)
    }
    fn visit_none<E: serde::de::Error>(self) -> Result<Option<Valeur>, E> {
        Ok(None)
    }
    fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Option<Valeur>, D::Error> {
        d.deserialize_any(self)
    }
    fn visit_bool<E: serde::de::Error>(self, b: bool) -> Result<Option<Valeur>, E> {
        match self {
            Attendu::Simple(Forme::Booleen, _) | Attendu::Quelconque(_) => Ok(Some(Valeur::OuiNon(b))),
            _ => Err(E::invalid_type(Unexpected::Bool(b), &self)),
        }
    }
    fn visit_str<E: serde::de::Error>(self, t: &str) -> Result<Option<Valeur>, E> {
        match self {
            Attendu::Simple(f, _) if f != Forme::Booleen =>Ok(Some(Valeur::Texte(self.rangee(t)))),
            Attendu::Quelconque(_) => Ok(Some(Valeur::Texte(self.rangee(t)))),
            _ => Err(E::invalid_type(Unexpected::Str(t), &self)),
        }
    }
    fn visit_i64<E: serde::de::Error>(self, i: i64) -> Result<Option<Valeur>, E> {
        self.nombre(NombreLu::from(i), Unexpected::Signed(i))
    }
    fn visit_u64<E: serde::de::Error>(self, u: u64) -> Result<Option<Valeur>, E> {
        self.nombre(NombreLu::from(u), Unexpected::Unsigned(u))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Option<Valeur>, A::Error> {
        match self {
            Attendu::Lignes(l) => {
                let mut out = Vec::new();
                while let Some(s) = a.next_element_seed(l.clone())? {
                    out.push(s);
                }
                Ok(Some(Valeur::Lignes(out)))
            }
            Attendu::Libre(_) | Attendu::Quelconque(_) => {
                let mut out = Vec::new();
                while let Some(m) = a.next_element::<String>()? {
                    out.push(m);
                }
                Ok(Some(Valeur::Mots(out)))
            }
            _ => Err(A::Error::invalid_type(Unexpected::Seq, &self)),
        }
    }
    fn visit_map<A: MapAccess<'de>>(self, m: A) -> Result<Option<Valeur>, A::Error> {
        let m = match socle::forme::table_ou_nombre(m)? {
            socle::forme::TableOuNombre::Nombre(n) => {
                let ecrit = n.ecrit();
                return self.nombre(n, Unexpected::Other(&ecrit));
            }
            socle::forme::TableOuNombre::Table(m) => m,
        };
        match self {
            Attendu::Bloc(l) => l.visit_map(m).map(|s| Some(Valeur::Bloc(s))),
            Attendu::Libre(d) => Lecture { niveau: Niveau::Libre(d), devant: Vec::new() }.visit_map(m).map(|s| Some(Valeur::Bloc(s))),
            Attendu::Quelconque(d) => Lecture { niveau: Niveau::Libre(&d.cases), devant: Vec::new() }.visit_map(m).map(|s| Some(Valeur::Bloc(s))),
            _ => Err(A::Error::invalid_type(Unexpected::Map, &self)),
        }
    }
}

// ------------------------------------------------------------------ ce qui manque

/// Le nom en clair d'un acte de `exige_par`.
fn acte_en_clair(acte: &str) -> &str {
    match acte {
        "dsn" => "la DSN",
        "dpae" => "la DPAE",
        "contrat" => "les contrats",
        "bulletin" => "les bulletins",
        "certificat" => "les certificats Congés Spectacles",
        "paie" => "toute paie",
        "envoi" => "l'envoi par courriel de ses contrats et de ses documents",
        autre => autre,
    }
}

/// « la DSN, la DPAE et les contrats » ; aucun acte, rien a dire.
pub fn actes_en_clair(actes: &[String]) -> Option<String> {
    let noms: Vec<&str> = actes.iter().map(|a| acte_en_clair(a)).collect();
    let (dernier, debut) = noms.split_last()?;
    Some(match debut {
        [] => dernier.to_string(),
        _ => format!("{} et {}", debut.join(", "), dernier),
    })
}

/// CE QU'UNE CASE SERT : les actes qui ne se font pas sans elle (`exige_par`), ou les gestes qu'elle sert sans les
/// bloquer (`utile_pour`) et ce qui se passe sans elle (`sinon`).
#[derive(Debug, Clone, PartialEq)]
pub enum Sert {
    Exigee(Vec<String>),
    Utile { gestes: Vec<String>, sinon: Option<String> },
}

/// Une case vide d'une fiche qu'un acte exige ou qu'un geste sert (`Sert`).
#[derive(Debug, Clone)]
pub struct Manquant {
    /// Son chemin dans la fiche (`signataire.fonction`).
    pub cle: CheminDeFiche,
    /// La case que l'ecran montre (la forme lisible d'un champ declare : `derive_de`).
    pub ecran: CheminDeFiche,
    /// « Qui signe (fonction) ».
    pub libelle: String,
    pub aide: String,
    pub sert: Sert,
    /// Les cases d'une structure, dites (`cases_dites`).
    pub cases: Option<String>,
}

/// Ce que sert une ligne qui porte `exige_par` ou `utile_pour` ; aucune : elle ne manque a personne.
fn ce_qu_elle_sert(exige_par: Option<&Vec<String>>, utile_pour: Option<&Vec<String>>, sinon: Option<&String>) -> Option<Sert> {
    match (exige_par, utile_pour) {
        (Some(a), _) => Some(Sert::Exigee(a.clone())),
        (None, Some(u)) => Some(Sert::Utile { gestes: u.clone(), sinon: sinon.cloned() }),
        (None, None) => None,
    }
}

/// CE QUI MANQUE A LA FICHE `donnees` (`entreprise`, `salarie`) pour les actes de la compagnie : chaque champ, et chaque
/// case de structure, qui porte `exige_par` ou `utile_pour`, s'il s'applique (`si`, `exige_si` ; une case suit sa
/// structure) et qu'il est vide. Un champ declare se dit par sa forme lisible (`derive_de`).
pub fn manquants(nom: NomDeFiche, donnees: &Saisie) -> R<Vec<Manquant>> {
    let tous = table(nom)?;
    let mut out = Vec::new();
    for c in tous.iter().filter(|c| c.applicable(donnees)) {
        let m = montre(tous, c);
        if let Some(sert) = ce_qu_elle_sert(c.exige_par.as_ref(), c.utile_pour.as_ref(), c.sinon.as_ref()) {
            if !donnees.valeur(c.cle).is_some_and(Valeur::dite) {
                out.push(Manquant {
                    cle: CheminDeFiche::de(c.cle),
                    ecran: CheminDeFiche::de(m.cle),
                    libelle: m.libelle.clone(),
                    aide: m.aide.clone(),
                    sert,
                    cases: c.champs.as_deref().map(cases_dites).filter(|t| !t.is_empty()),
                });
            }
        }
        let Some(cases) = c.champs.as_deref() else { continue };
        let bloc = match donnees.valeur(c.cle) {
            Some(Valeur::Bloc(b)) => Some(b),
            _ => None,
        };
        for s in cases {
            let Some(sert) = ce_qu_elle_sert(s.exige_par.as_ref(), None, None) else { continue };
            if bloc.and_then(|b| b.au_chemin(&s.cle)).is_some_and(Valeur::dite) {
                continue;
            }
            let ms = montre(cases, s);
            let aide = match ms.aide.as_deref().filter(|a| !a.is_empty()) {
                Some(a) => a.to_string(),
                None => c.aide.clone(),
            };
            out.push(Manquant {
                cle: CheminDeFiche { champ: c.cle, dans: Some(s.cle.clone()) },
                ecran: CheminDeFiche { champ: c.cle, dans: Some(ms.cle.clone()) },
                libelle: format!("{} ({})", c.libelle, en_minuscule(&ms.libelle)),
                aide,
                sert,
                cases: None,
            });
        }
    }
    Ok(out)
}

/// LES CASES DE LA FICHE `nom` QUI SERVENT A UN ACTE : chacune, ou celle dont elle se deduit (`derive_de`), porte
/// `exige_par` ou `utile_pour` : la meme liste que `manquants`. Le questionnaire du salarie n'en pose pas d'autre.
pub fn utiles(nom: NomDeFiche) -> R<Vec<CaseDeFiche>> {
    let mut out = Vec::new();
    for c in table(nom)?.iter().filter(|c| c.exige_par.is_some() || c.utile_pour.is_some()) {
        out.push(c.cle);
        out.extend(c.derive_de);
    }
    Ok(out)
}

/// LE REFUS D'UN ACTE quand une fiche qu'il lit n'a pas ce qu'il exige (`exige_par`) : tout ce qui manque, d'un coup,
/// fiche par fiche. `fiches` : (`entreprise` ou `salarie`, qui, la fiche) ; `sauf` : les cases qu'un fait de l'acte ne demande
/// pas. `None` : rien ne manque.
pub fn refus_des_manques(acte: &str, fiches: &[(NomDeFiche, String, &Saisie)], sauf: &[CaseDeFiche]) -> R<Option<String>> {
    let mut lignes = Vec::new();
    for (nom, qui, donnees) in fiches {
        let manque: Vec<String> = manquants(*nom, donnees)?
            .into_iter()
            .filter(|m| matches!(&m.sert, Sert::Exigee(actes) if actes.iter().any(|a| a == acte)) && !m.cle.seul().is_some_and(|c| sauf.contains(&c)))
            .map(|m| format!("« {} »", m.libelle))
            .collect();
        if !manque.is_empty() {
            lignes.push(format!("{} dans {}", manque.join(", "), qui));
        }
    }
    if lignes.is_empty() {
        return Ok(None);
    }
    Ok(Some(format!(
        "Pour {}, il manque, à remplir dans Configuration : {}. Rien n'est écrit.",
        acte_en_clair(acte),
        lignes.join(" ; ")
    )))
}

// ------------------------------------------------------- les outils de texte

pub(crate) use socle::texte::{blanc as espace, strip};

/// Les chiffres, et ceux que Unicode range avec eux.
pub fn est_chiffre(c: char) -> bool {
    c.is_ascii_digit() || matches!(c, '\u{b2}' | '\u{b3}' | '\u{b9}') || (c.is_numeric() && !c.is_alphabetic() && !matches!(c, '\u{bc}'..='\u{be}' | '\u{2150}'..='\u{218f}'))
}

pub fn tous_chiffres(s: &str) -> bool {
    !s.is_empty() && s.chars().all(est_chiffre)
}

/// Chaque mot avec sa première lettre en capitale, le reste en minuscules.
pub fn titre(s: &str) -> String {
    let mut out = String::new();
    let mut precedent_cased = false;
    for c in s.chars() {
        let cased = c.is_lowercase() || c.is_uppercase();
        if cased {
            if precedent_cased {
                out.extend(c.to_lowercase());
            } else {
                out.extend(c.to_uppercase());
            }
        } else {
            out.push(c);
        }
        precedent_cased = cased;
    }
    out
}

/// Les mots-outils d'un nom de lieu, qui s'ecrivent en minuscules hors de la tete (« rue des Tisserands »,
/// « Saint-Didier-au-Mont-d'Or ») ; les pieces declarees l'ecrivent en capitales.
const MOTS_OUTILS: &[&str] = &["de", "des", "du", "la", "le", "les", "et", "en", "au", "aux", "sur", "sous", "lès", "lez"];

/// Un nom de lieu (adresse, commune) ecrit en capitales, en lisible : `titre`, puis les mots-outils en minuscules sauf
/// le premier, et l'elision (« d' », « l' ») de meme.
pub fn lieu_lisible(s: &str) -> String {
    let t = titre(s);
    let mut out = String::new();
    let mut mot = String::new();
    let mut premier = true;
    let pousser = |mot: &mut String, out: &mut String, premier: &mut bool| {
        if mot.is_empty() {
            return;
        }
        let bas = mot.to_lowercase();
        if !*premier && MOTS_OUTILS.contains(&bas.as_str()) {
            out.push_str(&bas);
        } else if !*premier && (bas.starts_with("d'") || bas.starts_with("l'")) && mot.chars().count() > 2 {
            let mut c = mot.chars();
            out.extend(c.next().into_iter().flat_map(char::to_lowercase));
            out.extend(c);
        } else {
            out.push_str(mot);
        }
        *premier = false;
        mot.clear();
    };
    for c in t.chars() {
        if c == ' ' || c == '-' {
            pousser(&mut mot, &mut out, &mut premier);
            out.push(c);
        } else {
            mot.push(c);
        }
    }
    pousser(&mut mot, &mut out, &mut premier);
    out
}

/// `sans_accents` : NFD, marques retirees.
pub fn sans_accents(s: &str) -> String {
    sans_accent(s)
}

/// `texte[a:b]`, sur les caracteres.
fn tranche(s: &str, a: usize, b: usize) -> String {
    s.chars().skip(a).take(b.saturating_sub(a)).collect()
}


/// Une date ISO : AAAA-MM-JJ, AAAAMMJJ, et les semaines ISO.
fn date_iso(s: &str) -> Option<Jour> {
    if !s.is_ascii() {
        return None;
    }
    let b = s.as_bytes();
    let chiffres = |a: usize, n: usize| -> Option<u32> {
        let x = b.get(a..a + n)?;
        if x.iter().all(u8::is_ascii_digit) {
            x.iter().try_fold(0u32, |n, c| n.checked_mul(10)?.checked_add(u32::from(c - b'0')))
        } else {
            None
        }
    };
    let an = chiffres(0, 4)? as i32;
    let mut p = 4;
    let sep = b.get(p) == Some(&b'-');
    if sep {
        p += 1;
    }
    if b.get(p) == Some(&b'W') {
        p += 1;
        let semaine = chiffres(p, 2)?;
        p += 2;
        let jour = if p == b.len() {
            1
        } else {
            if sep {
                if b.get(p) != Some(&b'-') {
                    return None;
                }
                p += 1;
            }
            let j = chiffres(p, 1)?;
            p += 1;
            j
        };
        if p != b.len() || semaine < 1 || !(1..=7).contains(&jour) {
            return None;
        }
        // Le lundi de la semaine 1 est celui de la semaine du 4 janvier.
        let quatre = Jour::nouveau(an, 1, 4)?;
        let lundi1 = quatre.ordinal() - i64::from(quatre.jour_de_la_semaine());
        // Une annee compte 53 semaines si elle commence un jeudi, ou un
        // mercredi quand elle est bissextile ; 52 sinon.
        let premier = Jour::nouveau(an, 1, 1)?.jour_de_la_semaine();
        let semaines = if premier == 3 || (premier == 2 && socle::date::bissextile(an)) { 53 } else { 52 };
        if semaine > semaines {
            return None;
        }
        return Jour::depuis_ordinal(lundi1 + (i64::from(semaine) - 1) * 7 + i64::from(jour) - 1);
    }
    let mois = chiffres(p, 2)?;
    p += 2;
    if sep {
        if b.get(p) != Some(&b'-') {
            return None;
        }
        p += 1;
    }
    let jour = chiffres(p, 2)?;
    p += 2;
    if p != b.len() {
        return None;
    }
    Jour::nouveau(an, mois, jour)
}

// ------------------------------------------------------------ normalisation

const SEPARATEURS: &str = " -'.";
const CIVILITES: [&str; 6] = ["MONSIEUR ", "M ", "MADAME ", "MME ", "MLLE ", "MR "];

/// `EQUIVALENTS` : ce que l'on tape n'est pas ce que la norme lit.
fn equivalent(c: char) -> Option<&'static str> {
    Some(match c {
        '\u{2019}' | '\u{2018}' | '\u{02bc}' | '\u{0060}' | '\u{00b4}' | '\u{2032}' => "'",
        '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2212}' => "-",
        '\u{00c6}' | '\u{00e6}' => "AE",
        '\u{0152}' | '\u{0153}' => "OE",
        '\u{00df}' => "SS",
        '\u{00d8}' | '\u{00f8}' => "O",
        '\u{00a0}' | '\u{202f}' | '\u{00b0}' => " ",
        _ => return None,
    })
}

/// Capitales sans accents, signes ramenes a l'ASCII,
/// separateurs isoles.
fn ponctuation_propre(texte: &str, identite: bool) -> String {
    let mut v: String = String::new();
    for c in texte.chars() {
        match equivalent(c) {
            Some(e) => v.push_str(e),
            None => v.push(c),
        }
    }
    let v: String = sans_accents(&v).chars().flat_map(char::to_uppercase).collect();
    // Ce qui reste hors de l'ASCII imprimable ne se declare pas : une espace.
    let v: String = v.chars().map(|c| if (' '..='~').contains(&c) { c } else { ' ' }).collect();
    // `\s+` -> une espace, puis strip.
    let mut compact = String::new();
    let mut dans_espace = false;
    for c in v.chars() {
        if espace(c) {
            dans_espace = true;
        } else {
            if dans_espace {
                compact.push(' ');
            }
            dans_espace = false;
            compact.push(c);
        }
    }
    let v = strip(&compact).to_string();
    // Pas d'espace contre un trait d'union ni une apostrophe : ` ?([-']) ?`.
    let c: Vec<char> = v.chars().collect();
    let mut sans: Vec<char> = Vec::new();
    let mut i = 0;
    while i < c.len() {
        if c[i] == ' ' && i + 1 < c.len() && (c[i + 1] == '-' || c[i + 1] == '\'') {
            // l'espace avant le signe
            i += 1;
            continue;
        }
        sans.push(c[i]);
        if (c[i] == '-' || c[i] == '\'') && i + 1 < c.len() && c[i + 1] == ' ' {
            // l'espace apres le signe
            i += 2;
            continue;
        }
        i += 1;
    }
    // Deux separateurs accoles : on garde le premier -- sauf le double trait
    // d'union des noms doubles (une fois) et le point suivi d'une espace.
    let mut sortie: Vec<char> = Vec::new();
    let mut doubles = 0;
    for c in sans {
        let a = sortie.last().copied();
        if let Some(a) = a {
            if SEPARATEURS.contains(a) && SEPARATEURS.contains(c) {
                let avant = sortie.len().checked_sub(2).map(|k| sortie[k]);
                if a == c && a == '-' && doubles == 0 && avant != Some('-') {
                    doubles += 1;
                } else if !(a == '.' && c == ' ') {
                    continue;
                }
            }
        }
        sortie.push(c);
    }
    let mut v: String = sortie.into_iter().collect();
    // Une civilite n'a rien a faire dans un nom : le sexe a sa rubrique.
    if identite {
        for civilite in CIVILITES {
            if v.starts_with(civilite) && v.chars().count() > civilite.chars().count() {
                v = v[civilite.len()..].to_string();
                break;
            }
        }
    }
    strip(&v).to_string()
}

/// Retire de la forme declaree ce que SA rubrique
/// n'admet pas -- par une espace d'abord, par rien ensuite -- et rend la forme
/// naive si rien ne passe.
fn sans_caracteres_refuses(c: &Champ, v: String) -> R<String> {
    let Some(rubrique) = c.rubrique.as_deref().filter(|r| !r.is_empty()) else { return Ok(v) };
    if v.is_empty() {
        return Ok(v);
    }
    let refuses = crate::formats::caracteres_refuses(rubrique, &v)?;
    if refuses.is_empty() {
        return Ok(v);
    }
    for remplacant in [" ", ""] {
        let mut essai = v.clone();
        for r in &refuses {
            essai = essai.replace(*r, remplacant);
        }
        let essai = essai.split(espace).filter(|m| !m.is_empty()).collect::<Vec<_>>().join(" ");
        if !essai.is_empty() && crate::formats::mauvaise(rubrique, &essai)?.is_none() {
            return Ok(essai);
        }
    }
    Ok(v)
}

/// `normaliser(champ, texte)` : la forme que le champ DEVRAIT avoir.
pub fn normaliser(c: &Champ, texte: &str) -> R<String> {
    let v = strip(texte).to_string();
    Ok(match c.forme {
        Forme::Officiel => {
            let v = ponctuation_propre(&v, [CaseSalarie::Nom, CaseSalarie::Prenom].map(CaseDeFiche::from).contains(&c.cle));
            let v = sans_caracteres_refuses(c, v)?;
            v.trim_matches(|x| " '.-".contains(x)).to_string()
        }
        Forme::Chiffres => v.chars().filter(|x| !espace(*x)).collect(),
        // Un pays se garde par son code ISO, en capitales.
        Forme::Pays => v.to_uppercase(),
        _ => v,
    })
}

// Plus de defauts : `statut_conventionnel` et `statut_categoriel_rc` se montraient a 05 et
// 04 sur une fiche qui ne les portait pas, et l'import les ecrivait ainsi. Ils se lisent du metier ou se saisissent ;
// la DSN refuse ce qui ne se lit pas (`dsn::statuts_du_contrat`).

/// LES PERSONNES DE LA FICHE ENTREPRISE : le signataire, le contact DSN et celui de la sante au travail, avec leurs cases
/// declarees et la forme lisible de chacune.
pub fn personnes() -> [(NomDeCase, Vec<(NomDeCase, NomDeCase)>); 3] {
    use droit::fiche::{CaseContactSanteTravail, CaseDeclarantDsn, CaseSignataire};
    [
        (
            CaseEntreprise::Signataire.into(),
            vec![(CaseSignataire::Nom.into(), CaseSignataire::NomAffichage.into()), (CaseSignataire::Prenom.into(), CaseSignataire::PrenomAffichage.into())],
        ),
        (CaseEntreprise::DeclarantDsn.into(), vec![(CaseDeclarantDsn::NomPrenom.into(), CaseDeclarantDsn::NomPrenomAffichage.into())]),
        (
            CaseEntreprise::ContactSanteTravail.into(),
            vec![(CaseContactSanteTravail::NomPrenom.into(), CaseContactSanteTravail::NomPrenomAffichage.into())],
        ),
    ]
}

/// UNE FORME LISIBLE A DONNER a une saisie qui n'en a pas : la case `cle` (d'un bloc, `bloc`), son texte.
#[derive(Debug, Clone, PartialEq)]
pub struct Comble {
    pub bloc: Option<NomDeCase>,
    pub cle: CaseDeFiche,
    pub texte: String,
}

/// `a_combler` : la forme lisible de depart des fiches qui n'en ont pas, tiree de leur forme declaree. Rien n'est ecrit :
/// c'est une vue, pas une migration.
pub fn a_combler(nom: NomDeFiche, donnees: &Saisie) -> R<Vec<Comble>> {
    let lieux = [CaseSalarie::Adresse, CaseSalarie::Commune, CaseSalarie::LieuNaissance].map(CaseDeFiche::from);
    let mut out: Vec<Comble> = Vec::new();
    for c in table(nom)? {
        let Some(source) = c.derive_de else { continue };
        if donnees.valeur(source).is_some_and(Valeur::dite) || out.iter().any(|x| x.bloc.is_none() && x.cle == source) {
            continue;
        }
        let Some(Valeur::Texte(officiel)) = donnees.valeur(c.cle) else { continue };
        if officiel.is_empty() {
            continue;
        }
        let t = if lieux.contains(&c.cle) { lieu_lisible(officiel) } else { titre(officiel) };
        out.push(Comble { bloc: None, cle: source, texte: t });
    }
    if nom == NomDeFiche::Entreprise {
        for (cle, officiels) in personnes() {
            let Some(Valeur::Bloc(p)) = donnees.case(cle) else { continue };
            for (k, lisible) in officiels {
                if p.dite(lisible).is_some() {
                    continue;
                }
                if let Some(Valeur::Texte(o)) = p.case(k) {
                    out.push(Comble { bloc: Some(cle), cle: lisible.into(), texte: titre(o) });
                }
            }
        }
    }
    Ok(out)
}

/// `apercu_derive` : ce qui SERAIT declare, sans rien modifier : chaque forme declaree que sa forme lisible donne.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(transparent)]
pub struct Apercu(#[serde(serialize_with = "socle::ecriture::en_ordre")] pub Vec<(CaseDeFiche, String)>);

pub fn apercu_derive(nom: NomDeFiche, donnees: &Saisie) -> R<Apercu> {
    let mut out = Vec::new();
    for c in table(nom)? {
        let Some(source) = c.derive_de else { continue };
        match donnees.valeur(source).filter(|v| v.dite()) {
            Some(Valeur::Texte(t)) => out.push((c.cle, normaliser(c, t)?)),
            Some(autre) => out.extend(autre.mot().map(|m| (c.cle, m))),
            None => {}
        }
    }
    Ok(Apercu(out))
}

// ------------------------------------------------------------- verification

const RUBRIQUE_NIR: &str = "S21.G00.30.001";

/// `MORCEAUX_NIR` : ce que vaut chaque morceau d'un NIR (cahier technique
/// NEODeS 2026.1, definition de S21.G00.30.001) : SAAMMDDCCCNNN.
const MORCEAUX_NIR: [(usize, usize, &str, &str, &str); 6] = [
    (0, 1, "le sexe", "1 (homme) ou 2 (femme)", "sexe"),
    (1, 3, "l'annee de naissance", "00 a 99, les deux derniers chiffres", "annee"),
    (3, 5, "le mois de naissance", "01 a 12 -- ou 20, 30 a 42, 50 a 99 pour une immatriculation particuliere", "mois"),
    (5, 7, "le departement de naissance", "01 a 99, ou 2A ou 2B", "departement"),
    (7, 10, "la commune de naissance", "001 a 999", "commune"),
    (10, 13, "le numero d'ordre a l'etat civil", "001 a 999", "ordre a l'etat civil"),
];

/// Le nombre que des chiffres ASCII ecrivent ; un autre caractere n'en ecrit pas.
fn entier_ecrit(morceau: &str) -> Option<i64> {
    if morceau.is_empty() || !morceau.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    morceau.bytes().try_fold(0i64, |n, b| n.checked_mul(10)?.checked_add(i64::from(b - b'0')))
}

/// Les bornes du cahier technique, morceau par morceau.
fn morceau_de_nir_va(debut: usize, morceau: &str) -> bool {
    match debut {
        0 => morceau == "1" || morceau == "2",
        1 => tous_chiffres(morceau),
        3 => tous_chiffres(morceau) && entier_ecrit(morceau).is_some_and(|m| (1..=12).contains(&m) || m == 20 || (30..=42).contains(&m) || (50..=99).contains(&m)),
        5 => morceau == "2A" || morceau == "2B" || (tous_chiffres(morceau) && entier_ecrit(morceau).is_some_and(|n| (1..=99).contains(&n))),
        _ => tous_chiffres(morceau) && entier_ecrit(morceau).is_some_and(|n| n >= 1),
    }
}

/// `[0-9]{5}(2A|2B)[0-9]{6}`, en entier.
fn nir_de_corse(v: &str) -> bool {
    let c: Vec<char> = v.chars().collect();
    c.len() == 13
        && c[..5].iter().all(char::is_ascii_digit)
        && c[5] == '2'
        && (c[6] == 'A' || c[6] == 'B')
        && c[7..].iter().all(char::is_ascii_digit)
}

/// `explication_nir` : pourquoi ce NIR ne peut pas en etre un -- en francais.
fn explication_nir(nir: &str) -> R<Option<String>> {
    let v = strip(nir);
    if v.is_empty() {
        return Ok(None);
    }
    if !tous_chiffres(v) && !nir_de_corse(v) {
        return Ok(Some("un NIR ne porte que des chiffres (le departement de Corse excepte : 2A ou 2B)".into()));
    }
    let n = v.chars().count() as i64;
    if n != 13 {
        let manque = 13 - n;
        let morceaux: Vec<String> = MORCEAUX_NIR.iter().map(|(deb, fin, _, _, court)| format!("{} ({})", court, fin - deb)).collect();
        return Ok(Some(format!(
            "{} chiffres saisis, 13 attendus : il en {}. Un NIR se compose de {} ; sa cle, 2 chiffres de plus, se saisit dans la case d'a cote.",
            n,
            if manque > 0 { format!("manque {}", manque) } else { format!("a {} de trop", -manque) },
            morceaux.join(", ")
        )));
    }
    if v.starts_with(['7', '8']) {
        return Ok(Some(
            "le premier chiffre dit le sexe : 1 ou 2. Un numero provisoire commencant par 7 ou 8 ne se declare pas -- le cahier technique demande le NIA notifie par l'organisme, qui a la forme d'un NIR ; a defaut de NIR et de NIA, laisser cette case VIDE et renseigner le sexe : le numero technique temporaire (S21.G00.30.020) se fabrique alors tout seul".into(),
        ));
    }
    if v == "1999999999999" || v == "2999999999999" {
        return Ok(Some(
            "ce numero de remplissage est explicitement interdit par la norme (CCH-13). Pour un salarie pas encore immatricule, c'est le NIA notifie par l'organisme qui se saisit ici ; a defaut, laisser vide : la norme exige alors le numero technique temporaire (S21.G00.30.020), que le moteur fabrique du sexe, du SIREN et du matricule".into(),
        ));
    }
    if v.len() >= 6 && v.chars().rev().take_while(|c| *c == '9').count() >= 6 {
        return Ok(Some(
            "plus de cinq « 9 » a la suite en fin de numero : la norme ne l'admet pas (CCH-16). C'est la marque d'un numero complete au juge -- il vaut mieux une fiche qui refuse qu'une declaration qui identifie la mauvaise personne".into(),
        ));
    }
    if crate::formats::mauvaise(RUBRIQUE_NIR, v)?.is_none() {
        return Ok(None);
    }
    for (deb, fin, nom, attendu, _) in MORCEAUX_NIR {
        let morceau = tranche(v, deb, fin);
        if morceau_de_nir_va(deb, &morceau) {
            continue;
        }
        return Ok(Some(format!(
            "{} (caractere{} {}{}) vaut {} : la norme attend {}",
            nom,
            if fin - deb > 1 { "s" } else { "" },
            deb + 1,
            if fin - deb > 1 { format!(" a {}", fin) } else { String::new() },
            morceau,
            attendu
        )));
    }
    Ok(None)
}

/// LE DEPARTEMENT DE NAISSANCE QUE LE NIR DIT : ses 6e et 7e caracteres (`SAAMMDDCCCNNN`, DD « département de naissance »,
/// 01 a 99, 2A ou 2B ; 99 pour une personne nee a l'etranger : S21.G00.30.001 et .014,
/// `atelier/reference/norme-dsn/texte-2027/bloc-S21.G00.30.txt`). `None` pour un NIR qui n'a pas cette forme.
pub fn departement_du_nir(nir: &str) -> Option<String> {
    let n = strip(nir).to_uppercase();
    if n.chars().count() != 13 || !matches!(tranche(&n, 0, 1).as_str(), "1" | "2") {
        return None;
    }
    let dd = tranche(&n, 5, 7);
    let autres = format!("{}{}", tranche(&n, 0, 5), tranche(&n, 7, 13));
    let lisible = (dd.chars().all(|c| c.is_ascii_digit()) && dd != "00") || dd == "2A" || dd == "2B";
    (lisible && autres.chars().all(|c| c.is_ascii_digit())).then_some(dd)
}

/// `cle_nir` : 97 - (NIR mod 97), sur deux chiffres (`reference/sources/insee-nir-siret/`, INSEE, schema NIR, 4.1). En Corse,
/// 2A et 2B au 6e-7e rang valent 19 et 18 (equivalent, par inference, au zero moins 1 000 000 ou 2 000 000 du texte).
pub fn cle_nir(nir: &str) -> Option<droit::fiche::CleNir> {
    let mut n = strip(nir).to_uppercase();
    if n.chars().count() != 13 {
        return None;
    }
    if tranche(&n, 5, 7) == "2A" {
        n = n.replacen("2A", "19", 1);
    }
    if tranche(&n, 5, 7) == "2B" {
        n = n.replacen("2B", "18", 1);
    }
    // Les chiffres decimaux de toutes les ecritures comptent comme des chiffres.
    let mut x: u64 = 0;
    for c in n.chars() {
        x = x * 10 + valeur_decimale(c)? as u64;
    }
    Some(droit::fiche::CleNir::du_nombre(x))
}

/// La valeur d'un chiffre decimal Unicode (categorie Nd).
fn valeur_decimale(c: char) -> Option<u32> {
    if let Some(d) = c.to_digit(10) {
        return Some(d);
    }
    // Le zero de chaque suite de dix chiffres decimaux (Unicode 15).
    const ZEROS: [u32; 65] = [
        0x660, 0x6F0, 0x7C0, 0x966, 0x9E6, 0xA66, 0xAE6, 0xB66, 0xBE6, 0xC66, 0xCE6, 0xD66, 0xDE6, 0xE50, 0xED0, 0xF20, 0x1040, 0x1090, 0x17E0,
        0x1810, 0x1946, 0x19D0, 0x1A80, 0x1A90, 0x1B50, 0x1BB0, 0x1C40, 0x1C50, 0xA620, 0xA8D0, 0xA900, 0xA9D0, 0xA9F0, 0xAA50, 0xABF0, 0xFF10,
        0x104A0, 0x10D30, 0x11066, 0x110F0, 0x11136, 0x111D0, 0x112F0, 0x11450, 0x114D0, 0x11650, 0x116C0, 0x11730, 0x118E0, 0x11950, 0x11C50,
        0x11D50, 0x11DA0, 0x16A60, 0x16AC0, 0x16B50, 0x1D7CE, 0x1D7D8, 0x1D7E2, 0x1D7EC, 0x1D7F6, 0x1E140, 0x1E2F0, 0x1E4F0, 0x1E950,
    ];
    let p = c as u32;
    ZEROS.iter().find(|z| (**z..**z + 10).contains(&p)).map(|z| p - z).or_else(|| (0x1FBF0..0x1FBFA).contains(&p).then(|| p - 0x1FBF0))
}

/// LES JOURS TRAVAILLES D'UN PERMANENT : ceux que le calcul lira (`droit::conges::JoursTravailles`) ; la faute, dite sans
/// le nom du salarie que la fiche montre deja.
fn faute_des_jours(v: &Valeur) -> Option<String> {
    use droit::conges::JoursTravailles;
    use droit::fiche::JourDeLaSemaine;
    let illisible = JoursTravailles::case_illisible("le salarie");
    let faute = match v {
        Valeur::Mots(mots) => match mots
            .iter()
            .map(|m| JourDeLaSemaine::deserialize(serde::de::value::StrDeserializer::<serde::de::value::Error>::new(m)))
            .collect::<Result<Vec<JourDeLaSemaine>, _>>()
        {
            Ok(jours) => JoursTravailles::de_la_case(&Some(jours), "le salarie").err().map(|e| e.phrase),
            // Un mot qui n'est pas un jour de la semaine : la case ne se lit pas.
            Err(serde::de::value::Error { .. }) => Some(illisible),
        },
        _ => Some(illisible),
    };
    faute.map(|e| match e.split_once(" : ") {
        Some((_, m)) => m.to_string(),
        None => e.clone(),
    })
}

/// `forme_declarative` : la valeur telle qu'elle PARTIRA. Une date se saisit
/// en AAAA-MM-JJ et se declare en JJMMAAAA ; une date impossible ne part pas.
fn forme_declarative(forme: Forme, valeur: &str) -> Option<String> {
    match forme {
        Forme::Date => date_iso(valeur).map(|j| j.jjmmaaaa()),
        _ => Some(valeur.to_string()),
    }
}

/// `faute_de_norme` : ce que la norme reproche a cette valeur du champ `c` -- `None` si elle passe.
pub fn faute_de_norme(c: &Champ, valeur: &str) -> R<Option<String>> {
    let r = c.regle();
    faute_de_rubrique(r.forme, r.rubrique, valeur)
}

fn faute_de_rubrique(forme: Forme, rubrique: Option<&str>, valeur: &str) -> R<Option<String>> {
    let Some(rubrique) = rubrique else { return Ok(None) };
    if valeur.is_empty() {
        return Ok(None);
    }
    let Some(v) = forme_declarative(forme, valeur).filter(|v| !v.is_empty()) else { return Ok(None) };
    let Some(mut pourquoi) = crate::formats::mauvaise(rubrique, &v)? else {
        return Ok(None);
    };
    let refuses = crate::formats::caracteres_refuses(rubrique, &v)?;
    if !refuses.is_empty() {
        let liste: Vec<String> = refuses.iter().map(|c| format!("\u{ab} {} \u{bb}", c)).collect();
        pourquoi = format!("{} n'y {} pas admis", liste.join(", "), if refuses.len() > 1 { "sont" } else { "est" });
    }
    Ok(Some(format!("{} (rubrique {} du cahier technique NEODeS) : le point de depot refuserait la declaration", pourquoi, rubrique)))
}

/// Les regles de position du cahier technique
/// DSN, section 4.
fn faute_de_forme_officielle(v: &str) -> Option<String> {
    let c: Vec<char> = v.chars().collect();
    if c.is_empty() {
        return None;
    }
    if c[0] == '-' || c[0] == ' ' {
        return Some("ne peut pas commencer par un trait d'union ni un espace (cahier technique DSN, section 4)".into());
    }
    if SEPARATEURS.contains(c[c.len() - 1]) {
        return Some("ne peut pas finir par un espace, un trait d'union, une apostrophe ou un point (cahier technique DSN, section 4)".into());
    }
    for w in c.windows(2) {
        let (a, b) = (w[0], w[1]);
        if SEPARATEURS.contains(a) && SEPARATEURS.contains(b) {
            // Deux exceptions prevues : le double trait d'union des noms
            // doubles, et le point suivi d'un espace.
            if (a == b && a == '-') || (a == '.' && b == ' ') {
                continue;
            }
            return Some(format!(
                "\u{ab} {}{} \u{bb} : espace, trait d'union, apostrophe et point s'emploient isoles, jamais l'un a cote de l'autre (cahier technique DSN, section 4)",
                a, b
            ));
        }
    }
    if v.matches("--").count() > 1 {
        return Some("le double trait d'union ne peut apparaitre qu'une fois, entre le premier et le second nom".into());
    }
    let haut = v.to_uppercase();
    if CIVILITES.iter().any(|civ| haut.starts_with(civ)) {
        return Some("une civilite dans une rubrique d'identite est une anomalie DSN : le sexe a sa propre rubrique".into());
    }
    None
}

/// CE QU'UNE CASE EXIGE D'UNE VALEUR SAISIE : sa ligne de la table (un champ de la fiche, ou une case d'une structure).
#[derive(Clone, Copy)]
struct Regle<'a> {
    cle: CaseDeFiche,
    libelle: &'a str,
    forme: Forme,
    rubrique: Option<&'a str>,
    choix: Option<&'a [String]>,
    longueur: Option<i64>,
}

/// `verifier_champ` : un message si la valeur saisie est fautive, sinon `None`. Une case vide n'est pas une faute : ce
/// qu'un acte exige, il le dit a son heure (`exige_par`, `refus_des_manques`).
fn verifier_champ(c: Regle<'_>, valeur: Option<&Valeur>) -> R<Option<String>> {
    let Some(valeur) = valeur else { return Ok(None) };
    if matches!(valeur, Valeur::Texte(s) if strip(s).is_empty()) {
        return Ok(None);
    }
    let est = |k: CaseDeFiche| c.cle == k;
    if est(CasePieceSalarie::JoursTravailles.into()) {
        return Ok(faute_des_jours(valeur));
    }
    let Valeur::Texte(brut) = valeur else { return Ok(None) };
    // La signature des courriels est du HTML colle : sa seule regle est son poids (150 Ko, images comprises).
    if est(CaseEntreprise::CourrielSignature.into()) {
        return Ok(trop_lourde(brut));
    }
    let v = strip(brut);
    if c.forme == Forme::Officiel {
        if let Some(faute) = faute_de_forme_officielle(v) {
            return Ok(Some(faute));
        }
        let attendu: String = sans_accents(v).chars().flat_map(char::to_uppercase).collect();
        if v != attendu {
            return Ok(Some(format!(
                "doit etre en MAJUSCULES SANS ACCENTS : ce champ part dans un format a largeur fixe (AEM, DPAE, DSN). Forme attendue : {}",
                attendu
            )));
        }
    }
    // LE NIR A SON EXPLICATION A LUI, et elle passe devant tout le reste.
    if c.rubrique == Some(RUBRIQUE_NIR) {
        if let Some(explique) = explication_nir(v)? {
            return Ok(Some(explique));
        }
    }
    if c.forme == Forme::Chiffres && !tous_chiffres(v) {
        return Ok(Some("chiffres uniquement".into()));
    }
    if c.forme == Forme::Date && date_iso(v).is_none() {
        return Ok(Some("date attendue au format AAAA-MM-JJ".into()));
    }
    if c.forme == Forme::Montant && socle::num::decimal_a_virgule(v).is_none() {
        return Ok(Some("nombre attendu".into()));
    }
    if let (Forme::Choix, Some(choix)) = (c.forme, c.choix) {
        if !choix.iter().any(|x| x == v) {
            return Ok(Some(format!("valeur attendue parmi : {}", choix.join(", "))));
        }
    }
    // UN PAYS SE CHOISIT DANS LA LISTE LIVREE (`pays.rs`) : un code qu'elle ne connait pas refuse.
    // Hors de la liste : un pays de nationalite s'ecrit a part. Le pays de naissance se lit dans la table
    // « Pays » de la DSN (`pays::de_la_table_dsn`), le Kosovo et les Etats disparus compris.
    if c.forme == Forme::Pays {
        let naissance = est(CaseSalarie::PaysNaissance.into());
        let connu = if naissance { socle::pays::de_la_table_dsn(v)?.is_some() } else { socle::pays::du_code(v)?.is_some() };
        if !connu {
            let quoi = format!("Le {}", c.libelle.to_lowercase());
            return Ok(Some(if naissance {
                socle::pays::inconnu_naissance(&quoi, v)
            } else if est(CaseSalarie::PaysNationalite.into()) {
                socle::pays::inconnu_nationalite(&quoi, v)
            } else {
                socle::pays::inconnu(&quoi, v)
            }));
        }
    }
    // LE NUMERO D'ADHERENT CONGES SPECTACLES : dix chiffres et une lettre, dont la DSN tire S21.G00.40.022 (97 + ce
    // numero) ; la forme et sa source sont a un seul endroit, `conges_spectacles::faute_adherent`.
    if est(documents::conges_spectacles::CLE_ADHERENT.into()) {
        if let Some(faute) = documents::conges_spectacles::faute_adherent(&documents::conges_spectacles::forme_adherent(v)) {
            return Ok(Some(faute));
        }
    }
    // LE JOUR DU VERSEMENT DES SALAIRES, des permanents comme des intermittents, se lit comme la paie le lira
    // (`droit::versement`).
    if est(CasePieceEntreprise::DateDeVersementPermanents.into()) || est(CaseEntreprise::DateDeVersementIntermittents.into()) {
        return Ok(droit::versement::Versement::lire(v).err().map(|e| e.phrase));
    }
    // LE CODE APE SE RECONNAIT DANS LA NAF REV. 2 OU LA NAF 2025 (`naf.rs`) : les deux sont admises a la saisie,
    // un code qu'aucune ne connait refuse.
    if est(CaseEntreprise::Naf.into()) {
        if let Some(faute) = crate::naf::faute_de_saisie(v)? {
            return Ok(Some(faute));
        }
    }
    if let Some(l) = c.longueur {
        let n = v.chars().count() as i64;
        if n != l {
            return Ok(Some(format!("{} caracteres attendus, {} saisis", l, n)));
        }
    }
    // EN DERNIER : la norme ne parle que de ce qui a deja la bonne forme generale.
    faute_de_rubrique(c.forme, c.rubrique, v)
}

/// La gravite d'une anomalie : « erreur » quand la donnee est certainement fausse, « avertissement » quand elle est
/// seulement suspecte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Gravite {
    Erreur,
    Avertissement,
}

/// UNE ANOMALIE D'UNE FICHE : la case (`champ`), sa gravite, ce qui ne va pas ; si l'ecran peut passer outre (`forcable`)
/// et pourquoi ; le libelle de la case, que le moteur pose quand la page le montre (`libelle`). Elle se relit avec la
/// reponse qui la porte (le refus d'une fiche, `routes::enveloppe::APart`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Anomalie {
    pub champ: CaseDeFiche,
    pub gravite: Gravite,
    pub message: String,
    pub forcable: bool,
    pub pourquoi: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub libelle: Option<String>,
}

/// Ce qu'une verification constate, avant de dire si l'ecran peut passer outre.
struct Constat {
    champ: CaseDeFiche,
    gravite: Gravite,
    message: String,
}

fn constat(champ: impl Into<CaseDeFiche>, gravite: Gravite, message: impl Into<String>) -> Constat {
    Constat { champ: champ.into(), gravite, message: message.into() }
}

impl Constat {
    fn dit(self, forcable: bool, pourquoi: impl Into<String>) -> Anomalie {
        Anomalie { champ: self.champ, gravite: self.gravite, message: self.message, forcable, pourquoi: pourquoi.into(), libelle: None }
    }
}

/// CE QUE LE TYPE D'UNE CASE REFUSE (`droit::fiche::FormeDeCase::FAUTE`, un texte hors de la borne des normes) : la fiche
/// telle qu'elle se lira, case par case, case par case d'un bloc (le contact DSN, le signataire) et ligne par ligne d'une
/// structure en lignes (les etablissements). La faute d'une forme declaree se dit sur la forme lisible dont elle se
/// deduit, qui est ce qu'on corrige.
fn fautes_des_types(champs: &[Champ], donnees: &Saisie) -> Vec<Constat> {
    let mut fautes = Vec::new();
    for (c, d) in champs.iter().filter_map(|c| c.declaree.as_ref().map(|d| (c, d))) {
        match donnees.valeur(d.cle) {
            Some(Valeur::Texte(t)) => {
                if let Some(f) = d.faute.and_then(|faute| faute(t)) {
                    fautes.push(constat(c.a_l_ecran(), Gravite::Erreur, f));
                }
            }
            Some(Valeur::Bloc(bloc)) => {
                for s in &d.cases {
                    let (Some(faute), Some(t)) = (s.faute, bloc.texte(s.cle)) else { continue };
                    if let Some(f) = faute(t) {
                        fautes.push(constat(d.cle, Gravite::Erreur, f));
                    }
                }
            }
            Some(Valeur::Lignes(lignes)) => {
                for l in lignes {
                    for s in &d.cases {
                        let (Some(faute), Some(t)) = (s.faute, l.texte(s.cle)) else { continue };
                        if let Some(f) = faute(t) {
                            let qui = match l.texte(droit::fiche::CaseEtablissement::Libelle).filter(|q| !q.is_empty()) {
                                Some(q) => q,
                                None => "un établissement",
                            };
                            fautes.push(constat(d.cle, Gravite::Erreur, format!("{} : {}", qui, f)));
                        }
                    }
                }
            }
            Some(Valeur::Nombre(_) | Valeur::OuiNon(_) | Valeur::Mots(_)) | None => {}
        }
    }
    fautes
}

/// Le texte saisi dans la case `cle`, sans ses blancs de bord, s'il en reste.
fn texte_dit(d: &Saisie, cle: impl Into<NomDeCase>) -> Option<String> {
    d.mot(cle).map(|t| strip(&t).to_string()).filter(|t| !t.is_empty())
}

/// Ce qu'aucun champ ne peut verifier seul.
fn controles_croises(nom: NomDeFiche, d: &Saisie) -> R<Vec<Constat>> {
    use CaseEntreprise as E;
    use CaseSalarie as S;
    use CaseSpectacle as P;
    let mut out = Vec::new();

    // UNE AUTRE URSSAF QUE CELLE DE L'ADRESSE : permis, mais dit, et par le moteur, pour que la
    // ligne de commande l'entende comme la fenetre. C'est l'Urssaf du territoire qui recoit DPAE et DSN.
    if nom == NomDeFiche::Entreprise {
        if let Some(choisi) = texte_dit(d, E::CodeUrssaf) {
            let deduit = match d.mot(E::CodePostal) {
                Some(cp) => crate::urssaf::base()?.du_code_postal(&cp),
                None => None,
            };
            if let Some(deduit) = deduit.filter(|d| *d != choisi) {
                out.push(constat(
                    E::CodeUrssaf,
                    Gravite::Avertissement,
                    format!(
                        "l'Urssaf de votre adresse est la {} ; c'est elle qui reçoit normalement vos DPAE et DSN. Une Urssaf qui ne vous gère pas peut les faire rejeter.",
                        deduit
                    ),
                ));
            }
        }
    }

    if nom == NomDeFiche::Spectacle {
        // LE TITRE QUE L'IMPORT N'A PAS TROUVE : un numero d'objet seul (la DSN ne porte pas le titre) recoit « A COMPLETER ».
        // Ce n'est pas un titre : le contrat et le bulletin l'imprimeraient tel quel.
        if texte_dit(d, P::Titre).is_some_and(|t| t.eq_ignore_ascii_case("A COMPLETER")) {
            let message = match d.note(crate::demarrage::TITRE_PROPOSE).filter(|t| !t.is_empty()) {
                Some(t) => format!("à valider : les bulletins repris proposent « {} », et le contrat imprime le titre.", t),
                None => "à compléter : aucune pièce reprise ne porte le titre de ce spectacle, et le contrat l'imprime.".to_string(),
            };
            out.push(constat(P::Titre, Gravite::Avertissement, message));
        }
        if let Some(code) = texte_dit(d, P::CodeAnalytique) {
            if !code.chars().all(|c| c.is_ascii() && (c.is_alphanumeric() || c == '-' || c == '_')) {
                out.push(constat(
                    P::CodeAnalytique,
                    Gravite::Erreur,
                    "lettres sans accent, chiffres, tiret ou souligné seulement : les fichiers des logiciels de comptabilité coupent sur une espace, une virgule ou un point-virgule.",
                ));
            }
        }
    }

    if nom == NomDeFiche::Salarie {
        let nir = texte_dit(d, S::Nir);
        let cle = texte_dit(d, S::NirCle);
        // SANS NIR, LA FICHE N'EST PAS INCOMPLETE : elle est celle d'un salarie
        // pas encore immatricule, sous son numero technique temporaire, qui
        // commence par le sexe.
        let sexe_saisi = texte_dit(d, S::Sexe);
        // Le sexe qu'il faut alors, la DPAE et la DSN l'exigent a leur heure (`exige_par`, `exige_si` du champ `sexe`).
        if nir.is_none() {
            out.push(constat(
                S::Nir,
                Gravite::Avertissement,
                "pas de NIR : ce salarie sera declare sous un numero technique temporaire, qui l'identifie dans la DSN mais n'ouvre aucun droit. Des que la caisse notifie le NIR ou le NIA, le saisir ici -- la premiere declaration suivante portera les deux, pour faire le lien.",
            ));
        }
        if let (Some(nir), Some(sexe)) = (&nir, &sexe_saisi) {
            let premier = tranche(nir, 0, 1);
            if (premier == "1" || premier == "2") && *sexe != premier {
                out.push(constat(
                    S::Sexe,
                    Gravite::Erreur,
                    format!("le NIR commence par {}, cette case dit {}. Les deux ne peuvent pas etre vrais ; c'est le NIR qui a ete declare.", premier, sexe),
                ));
            }
        }
        if let (Some(attendue), Some(cle)) = (nir.as_deref().and_then(cle_nir), &cle) {
            if *attendue != **cle {
                out.push(constat(
                    S::NirCle,
                    Gravite::Erreur,
                    format!("cle incoherente avec le NIR : {} attendue. Un NIR faux passe les controles de format et se voit des mois plus tard, dans un bilan d'identification.", attendue),
                ));
            }
        }
        match (&nir, &cle) {
            (None, Some(_)) => out.push(constat(
                S::Nir,
                Gravite::Erreur,
                "le NIR et sa cle vont ensemble : une cle sans NIR ne verifie rien -- si le salarie n'est pas immatricule, effacer aussi la cle",
            )),
            (Some(_), None) => out.push(constat(S::NirCle, Gravite::Erreur, "le NIR et sa cle vont ensemble : il manque la cle, les deux chiffres qui le suivent")),
            _ => {}
        }
        if let (Some(Valeur::Texte(nee)), Some(nir)) = (d.dite(S::DateNaissance), &nir) {
            if nir.chars().count() == 13 && tous_chiffres(&tranche(nir, 1, 5)) {
                let (an, mo) = (tranche(nir, 1, 3), tranche(nir, 3, 5));
                let (n24, n57) = (tranche(nee, 2, 4), tranche(nee, 5, 7));
                if n24 != an || (mo != "00" && n57 != mo) {
                    out.push(constat(S::Nir, Gravite::Avertissement, format!("le NIR annonce {}/{}, la date de naissance dit {}/{}", mo, an, n57, n24)));
                }
            }
        }
        // LE LIEU DE NAISSANCE ET LE NIR SE CONTREDISENT : le departement de naissance est celui que le NIR porte, et 99
        // veut dire « ne a l'etranger » (S21.G00.30.001 et .014, `atelier/reference/norme-dsn/texte-2027/bloc-S21.G00.30.txt`).
        // La DPAE et la DSN declarent les deux ; ce qui ne va pas ensemble se dit, sans rien remplacer.
        let dep = texte_dit(d, S::DepartementNaissance).map(|t| t.to_uppercase());
        let pays = texte_dit(d, S::PaysNaissance).map(|t| t.to_uppercase());
        let du_nir = nir.as_deref().and_then(departement_du_nir);
        if let (Some(lu), Some(dep)) = (&du_nir, &dep) {
            if lu != dep {
                out.push(constat(
                    S::DepartementNaissance,
                    Gravite::Avertissement,
                    format!("le NIR dit né dans le département {}, cette case dit {}. Les deux partent dans la DPAE et la DSN : l'un des deux est faux.", lu, dep),
                ));
            }
        }
        if let Some(dep_vu) = dep.or(du_nir) {
            if dep_vu == "99" && pays.as_deref() == Some("FR") {
                out.push(constat(
                    S::PaysNaissance,
                    Gravite::Avertissement,
                    "le département de naissance 99 veut dire « né à l'étranger », et le pays de naissance dit la France. Si la personne est née en France, son NIR ou son département est faux ; sinon, choisir son pays de naissance.",
                ));
            } else if !["97", "98", "99"].contains(&dep_vu.as_str()) && pays.as_deref().is_some_and(|p| p != "FR") {
                out.push(constat(
                    S::PaysNaissance,
                    Gravite::Avertissement,
                    format!("le département de naissance {} veut dire « né en France métropolitaine », et le pays de naissance dit un autre pays. Une personne née à l'étranger a le département 99.", dep_vu),
                ));
            }
        }
        // LA CODIFICATION UE (S21.G00.30.013) : un ancien 03 sans pays de nationalite, ou un pays qui contredit
        // la case, se dit ici, avant la DSN qui le refuserait. La regle est dans `codification_ue`, pas ici.
        // Un pays de nationalite hors de la liste se dit sur sa propre case (forme « pays »), pas deux fois.
        // Un pays ecrit « hors de la liste » qui n'en est pas un, ou a cote d'un pays de la liste, se dit sur
        // sa case a lui ; sinon c'est la nationalite choisie qui manque ou qui ne va pas, sur la sienne.
        let (nom_d, nom_affichage, prenom, prenom_affichage) = (d.mot(S::Nom), d.mot(S::NomAffichage), d.mot(S::Prenom), d.mot(S::PrenomAffichage));
        let qui = socle::texte::salarie_nomme(nom_d.as_deref(), nom_affichage.as_deref(), prenom.as_deref(), prenom_affichage.as_deref());
        let (code, pays_n, hors) = (d.mot(S::CodeNationaliteDsn), d.mot(S::PaysNationalite), d.mot(S::PaysNationaliteHorsListe));
        let fiche = socle::codification_ue::Nationalite::nouvelle(code.as_deref(), pays_n.as_deref(), hors.as_deref(), qui);
        let pays_inconnu = match d.dite(S::PaysNationalite) {
            Some(Valeur::Texte(p)) => socle::pays::du_code(p)?.is_none(),
            None | Some(Valeur::Nombre(_) | Valeur::OuiNon(_) | Valeur::Mots(_) | Valeur::Bloc(_) | Valeur::Lignes(_)) => false,
        };
        let hors_liste = d.dite(S::PaysNationaliteHorsListe).is_some();
        if let Some(m) = socle::codification_ue::faute_hors_liste(&fiche)? {
            out.push(constat(S::PaysNationaliteHorsListe, Gravite::Erreur, m));
        } else if !pays_inconnu && (hors_liste || d.dite(S::CodeNationaliteDsn).is_some() || d.dite(S::PaysNationalite).is_some()) {
            if let Err(m) = socle::codification_ue::codification_ue(&fiche) {
                out.push(constat(S::CodeNationaliteDsn, Gravite::Erreur, m.phrase));
            }
        }
        // LES BORNES D'UNE DISPENSE DE SANTE (Q13) : elles bornent un motif, et la fin ne precede pas le debut. Ce que
        // le mois en fait est dans `calcul::dispense_de_la_periode`.
        let mot = |p: CasePieceSalarie| d.piece(p).filter(|v| v.dite()).and_then(Valeur::mot);
        let (depuis, jusqu_au) = (mot(CasePieceSalarie::SanteDispenseDepuis), mot(CasePieceSalarie::SanteDispenseJusquAu));
        if (depuis.is_some() || jusqu_au.is_some()) && !d.piece(CasePieceSalarie::SanteDispense).is_some_and(Valeur::dite) {
            out.push(constat(
                CasePieceSalarie::SanteDispense,
                Gravite::Erreur,
                "la dispense a des dates mais pas de motif : écrire le motif de la dispense (la demande écrite du salarié), ou retirer ses dates".to_string(),
            ));
        }
        if let (Some(a), Some(b)) = (&depuis, &jusqu_au) {
            if date_iso(a).is_some() && date_iso(b).is_some() && b < a {
                out.push(constat(CasePieceSalarie::SanteDispenseJusquAu, Gravite::Erreur, format!("la dispense finirait ({}) avant de commencer ({})", b, a)));
            }
        }
        // Le code emploi, le statut et le PCS de la fiche, confrontes au catalogue des metiers. Avertissements, pas
        // erreurs. Le catalogue lit un texte vide comme une case que la fiche ne dit pas : les textes dits, bout a bout.
        if let Some(code_emploi) = d.mot(S::CodeEmploiFt) {
            let (statut, pcs) = (d.mot(S::Statut), d.mot(S::PcsEse));
            for msg in droit::emplois::controler_fiche(&code_emploi, &statut.as_slice().concat(), &pcs.as_slice().concat())? {
                out.push(constat(S::CodeEmploiFt, Gravite::Avertissement, msg));
            }
        }
    }

    if nom == NomDeFiche::Entreprise {
        let (siret, siren, nic) = (d.mot(E::Siret), d.mot(E::Siren), d.mot(E::Nic));
        if let (Some(siret), Some(siren), Some(nic)) = (&siret, &siren, &nic) {
            let somme = format!("{}{}", siren, nic);
            if *siret != somme {
                out.push(constat(E::Siret, Gravite::Erreur, format!("SIRET != SIREN + NIC ({})", somme)));
            }
        }
        // La cle de Luhn (`droit::fiche::Siret::cle_juste`) se dit sans refuser la fiche : le texte ne l'impose pas.
        if let Some(siret) = &siret {
            if droit::fiche::Siret::lire(siret).is_ok_and(|s| !s.cle_juste()) {
                out.push(constat(E::Siret, Gravite::Erreur, "ce numéro ne peut pas être un SIRET : un chiffre est probablement mal tapé"));
            }
        }
    }
    Ok(out)
}

/// `verifier_fiche` : toutes les anomalies d'une fiche, champ par champ, puis
/// les croisees.
pub fn verifier_fiche(nom: NomDeFiche, donnees: &Saisie) -> R<Vec<Anomalie>> {
    let champs = table(nom)?;
    let donnees = donnees.comblee(nom)?;
    let mut constats: Vec<Constat> = Vec::new();
    for c in champs {
        let Some(msg) = verifier_champ(c.regle(), donnees.valeur(c.cle))? else { continue };
        // Un champ derive n'est pas saisi : le signaler a l'humain reviendrait
        // a lui reprocher le travail de la machine. On pointe la forme
        // lisible, qui est ce qu'il peut corriger.
        match c.derive_de {
            Some(source) => constats.push(constat(source, Gravite::Erreur, format!("{} (la forme declaree, « {} », s'en deduit)", msg, c.libelle))),
            None => constats.push(constat(c.cle, Gravite::Erreur, msg)),
        }
    }
    let des_types = fautes_des_types(champs, &donnees);
    constats.extend(controles_croises(nom, &donnees)?);
    // UNE REGLE DE FORME PEUT SE TROMPER et la valeur etre juste : son erreur se passe outre a l'ecran (`forcable`).
    // CE QUE L'ECRAN DIT D'UNE ERREUR, d'apres le champ : s'il part dans une declaration (`declare` ; un champ lisible,
    // par la forme declaree qui s'en deduit).
    let declare = |cle: CaseDeFiche| champs.iter().any(|c| c.declare && (c.cle == cle || c.derive_de == Some(cle)));
    // Une fiche mise de cote ne doit pas crier. Ses fautes restent visibles --
    // et redeviennent des erreurs a la reactivation, ce que le message dit.
    let inactive = donnees.case(CaseSalarie::Actif) == Some(&Valeur::OuiNon(false));
    let mut anomalies: Vec<Anomalie> = constats
        .into_iter()
        .map(|mut c| {
            let pourquoi = if declare(c.champ) {
                "Ces champs partent dans des déclarations. Si la règle se trompe et que la valeur est juste, on peut passer outre — mais il faut le vouloir."
            } else {
                "Si la règle se trompe et que la valeur est juste, on peut passer outre — mais il faut le vouloir."
            };
            if inactive && c.gravite == Gravite::Erreur {
                c.gravite = Gravite::Avertissement;
                c.message = format!("{} (fiche inactive : a corriger avant toute paie)", c.message);
            }
            c.dit(true, pourquoi)
        })
        .collect();
    // CE QUE LE TYPE DE LA CASE REFUSE ne se passe pas outre, fiche active ou non : la fiche ne se relirait pas.
    anomalies.extend(des_types.into_iter().map(|c| c.dit(false, "La fiche ne se relirait pas : la valeur se corrige.")));
    Ok(anomalies)
}

/// La signature des courriels de la fiche Entreprise (`courriel_signature`) : du HTML colle, dont la seule regle est le
/// poids : 150 Ko, tout le HTML compris.
pub const LIMITE_OCTETS: usize = 150 * 1024;

/// La phrase d'une signature trop lourde, ou rien.
pub fn trop_lourde(html: &str) -> Option<String> {
    let n = html.len();
    if n > LIMITE_OCTETS {
        Some(format!(
            "la signature pese {} Ko, 150 Ko au plus, images comprises. Une image collee dans la signature pese tout son poids ; un lien vers une image en ligne presque rien",
            n.div_ceil(1024)
        ))
    } else {
        None
    }
}

/// LES CASES D'UNE STRUCTURE, DITES (`champs`) : leur nom, leur forme et leurs valeurs, d'apres la seule table qui les
/// decrit (`champs-base.json`). « tva : assujettie ou exoneree ; part_tva : nombre decimal ; ... ».
pub fn cases_dites(cases: &[Case]) -> String {
    cases
        .iter()
        .filter(|s| !s.calcule())
        .map(|s| {
            let valeurs: Vec<&str> = s.choix.iter().flatten().map(String::as_str).filter(|v| !v.is_empty()).collect();
            let forme = match s.forme {
                Forme::Booleen => "true ou false".to_string(),
                _ if !valeurs.is_empty() => valeurs.join(" ou "),
                f => f.libelle().to_string(),
            };
            format!("{} : {}", s.cle, forme)
        })
        .collect::<Vec<_>>()
        .join(" ; ")
}

/// LES CASES D'UNE STRUCTURE (`champs`) DONT LA VALEUR N'A PAS SA FORME (une valeur hors de sa liste, un nombre, une date),
/// que l'ecriture `donnees` ajouterait a la fiche : sa forme les refuserait (`droit::fiche`). Cela ne se force pas. Une
/// case que la table ne decrit pas, elle, a deja refuse la saisie a la porte (`Lecture`).
pub fn fautes_des_structures(nom: NomDeFiche, donnees: &Saisie) -> R<Vec<Anomalie>> {
    let mut out = Vec::new();
    for c in table(nom)? {
        let Some(cases) = c.champs.as_deref() else { continue };
        let Some(Valeur::Bloc(o)) = donnees.valeur(c.cle) else { continue };
        for s in cases.iter().filter(|s| matches!(s.forme, Forme::Choix | Forme::Montant | Forme::Date)) {
            if let Some(faute) = verifier_champ(s.regle(), o.au_chemin(&s.cle))? {
                out.push(constat(c.cle, Gravite::Erreur, format!("« {} » : {}", s.cle, faute)).dit(false, format!("Les cases de « {} » : {}.", c.libelle, cases_dites(cases))));
            }
        }
    }
    Ok(out)
}

/// « Prénom » devient « prénom » dans une phrase ; « IBAN » reste « IBAN ».
fn en_minuscule(libelle: &str) -> String {
    let mut c = libelle.chars();
    match (c.next(), c.next()) {
        (Some(p), Some(s)) if s.is_lowercase() => p.to_lowercase().chain(libelle.chars().skip(1)).collect(),
        _ => libelle.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une saisie d'une seule case texte, lue selon la table comme une fiche (`Saisie::de_la_fiche`).
    fn une_case(nom: NomDeFiche, cle: &str, valeur: &str) -> Saisie {
        Saisie::de_la_fiche(nom, &std::collections::BTreeMap::from([(cle, valeur)])).unwrap()
    }

    /// La fiche offre ce que la norme 2026.1 admet : 40.003, 01, 02, 04, 98, 99, pas 03 ; 40.002, 03 « cadre dirigeant »,
    /// que la CCH-11 de 40.003 admet pour un cadre (cahier technique 2026.1, p. 182-183, `bloc-S21.G00.40.txt` l. 133-196).
    /// La fiche entreprise admet un code de la NAF rev. 2 ou de la NAF 2025 (decret n° 2025-736), et refuse le reste
    /// (`atelier/reference/sources/insee-naf-2025/legifrance-decret-2025-736-naf-2025-20250731.txt`).
    #[test]
    fn la_fiche_entreprise_admet_un_code_des_deux_nomenclatures() {
        let faute_naf = |code: &str| -> Option<String> {
            verifier_fiche(NomDeFiche::Entreprise, &une_case(NomDeFiche::Entreprise, "naf", code)).unwrap().into_iter().find(|a| a.champ == CaseEntreprise::Naf.into()).map(|a| a.message)
        };
        assert_eq!(faute_naf("9001Z"), None, "NAF rev. 2");
        assert_eq!(faute_naf("9020Y"), None, "NAF 2025");
        assert_eq!(faute_naf("9039G"), None, "NAF 2025");
        let e = faute_naf("9099Z").expect("un code qu'aucune nomenclature ne connait refuse");
        assert!(e.contains("ni de la NAF rev. 2 ni de la NAF 2025"), "{}", e);
        let e = faute_naf("90.20Y").expect("le point refuse");
        assert!(e.contains("sans le point : 9020Y"), "{}", e);
    }

    #[test]
    fn la_fiche_offre_les_statuts_que_la_norme_admet() {
        let erreur_sur = |champ: CaseSalarie, valeur: &str| -> bool {
            let champ = CaseDeFiche::from(champ);
            verifier_fiche(NomDeFiche::Salarie, &une_case(NomDeFiche::Salarie, champ.nom(), valeur)).unwrap().iter().any(|a| a.champ == champ && a.gravite == Gravite::Erreur)
        };
        assert!(erreur_sur(CaseSalarie::StatutCategorielRc, "03"), "03 n'est pas un statut categoriel de retraite complementaire en 2026.1");
        for rc in ["01", "02", "04"] {
            assert!(!erreur_sur(CaseSalarie::StatutCategorielRc, rc), "{}", rc);
        }
        assert!(!erreur_sur(CaseSalarie::StatutConventionnel, "03"), "03 cadre dirigeant est un classement conventionnel");
    }

    #[test]
    fn la_cle_du_nir_et_celle_de_la_corse() {
        // insee-nir-siret : le complement a 97, 2A et 2B retranches de 1 000 000 et 2 000 000.
        assert_eq!(cle_nir("1850578999048").as_deref(), Some("19"));
        assert!(cle_nir("185057899904").is_none());
        // 2A vaut 19 et 2B vaut 18 : un NIR corse a sa cle.
        assert_eq!(cle_nir("185052A999048").as_deref(), Some("60"));
        assert_eq!(cle_nir("185052B999048").as_deref(), Some("87"));
        assert!(explication_nir("185052A999048").unwrap().is_none());
        assert!(cle_nir("18505ZZ999048").is_none());
    }
}
