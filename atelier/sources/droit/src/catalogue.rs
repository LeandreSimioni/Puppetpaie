//! Le catalogue des rubriques : ce que le calcul lit.
//!
//! Deux fichiers : `donnees/rubriques-base.json` (le logiciel) et
//! `config/rubriques-compagnie.json` (les donnees, sous la racine de la
//! compagnie). Les rubriques de la compagnie sont des regles du calcul : datees
//! comme toute la configuration, lues a la date de la periode. Le catalogue se
//! range par libelle.

use serde::Deserialize;
use crate::autre_revenu_brut::AutreElementDeRevenuBrut;
use socle::echec::Lieu;
use socle::fichiers::{dossier_du_code, repr, signature};
use socle::echec::R;
use socle::num::Decimal;
use crate::fiche::{ChampsRubrique, Datee, FicheDatee, Piece, RubriqueDatee, VoirListe};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};
use socle::date::{repr_texte, Jour};

/// L'UNITE D'UNE RUBRIQUE : ce que sa quantite compte, la liste fermee que le moteur sait compter (`unite` de chaque
/// rubrique de `rubriques-base.json` ; sa note : « Ici le nombre se saisit »). Une unite hors de la liste se refuse a
/// la lecture de la rubrique.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unite {
    Cachet,
    Heure,
    Jour,
    Semaine,
    Mois,
    Service,
    Forfait,
    Pourcentage,
}

impl Unite {
    /// Son nom ecrit.
    pub fn cle(self) -> &'static str {
        match self {
            Unite::Cachet => "cachet",
            Unite::Heure => "heure",
            Unite::Jour => "jour",
            Unite::Semaine => "semaine",
            Unite::Mois => "mois",
            Unite::Service => "service",
            Unite::Forfait => "forfait",
            Unite::Pourcentage => "pourcentage",
        }
    }
}

impl std::fmt::Display for Unite {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(self.cle())
    }
}

impl crate::fiche::FormeDeCase for Unite {}

/// Les natures juridiques. Elles ne commandent pas l'assujettissement (`soumis`, `imposable` le disent, rubrique par
/// rubrique) ; d'une ligne HORS assiette, elles disent ce qu'elle est pour le montant net social
/// (`Rubrique::hors_assiette`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NatureDeRubrique {
    Salaire,
    Frais,
    AvantageNature,
    Indemnite,
    Prime,
    Retenue,
    Absence,
}

impl NatureDeRubrique {
    /// Son nom ecrit.
    pub fn cle(self) -> &'static str {
        match self {
            NatureDeRubrique::Salaire => "salaire",
            NatureDeRubrique::Frais => "frais",
            NatureDeRubrique::AvantageNature => "avantage_nature",
            NatureDeRubrique::Indemnite => "indemnite",
            NatureDeRubrique::Prime => "prime",
            NatureDeRubrique::Retenue => "retenue",
            NatureDeRubrique::Absence => "absence",
        }
    }
}

impl std::fmt::Display for NatureDeRubrique {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(self.cle())
    }
}

impl crate::fiche::FormeDeCase for NatureDeRubrique {}

/// Ce qu'est une ligne hors assiette (`soumis` faux) pour le montant net social (S21.G00.58.004, type 03). Decret
/// n° 2023-1378 du 28/12/2023 : CSS R. 844-1 II (atelier/reference/sources/code-de-la-securite-sociale/
/// articles-20261010.md), C. trav. R. 3243-1, 9° bis ; FAQ du BOSS, Q 2.2, 2.11 et 2.12 (atelier/reference/sources/
/// montant-net-social/LISEZ-MOI.md, et la FAQ reproduite a cote).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HorsAssiette {
    /// Un remboursement de frais professionnels dans ses limites d'exoneration : n'y entre pas (Q 2.12).
    Frais,
    /// Une somme versee a l'occasion du travail, exoneree ou non (indemnites de rupture, Q 2.2) : y entre.
    Revenu,
    /// Un mouvement de tresorerie (acompte, avance, saisie) : pas un revenu, n'y entre pas (Q 2.12).
    Mouvement,
}

/// LE SENS D'UNE RUBRIQUE : ce que sa ligne fait au brut, l'ajouter (`+`) ou l'en retirer (`-`) (`sens` de
/// `rubriques-base.json`). Une liste du logiciel ; un autre sens se refuse a la lecture de la rubrique.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Sens {
    #[serde(rename = "+")]
    Plus,
    #[serde(rename = "-")]
    Moins,
}

impl Sens {
    /// Son nom ecrit.
    pub fn cle(self) -> &'static str {
        match self {
            Sens::Plus => "+",
            Sens::Moins => "-",
        }
    }

    /// Le signe qu'il donne au montant ou aux heures d'une ligne : 1 ou -1.
    pub fn signe(self) -> Decimal {
        match self {
            Sens::Plus => Decimal::ONE,
            Sens::Moins => Decimal::NEGATIVE_ONE,
        }
    }
}

impl crate::fiche::FormeDeCase for Sens {}

/// CE QU'UNE RUBRIQUE COMPTE POUR LES DROITS DES ANNEXES VIII ET X (`compte_pour` de `rubriques-base.json`) : ses heures
/// et son montant, ou ses heures pour l'ouverture des droits seulement. Une liste du logiciel ; une autre valeur se
/// refuse a la lecture de la rubrique.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComptePour {
    DroitsEtMontant,
    OuvertureDesDroits,
}

impl ComptePour {
    /// Son nom ecrit.
    pub fn cle(self) -> &'static str {
        match self {
            ComptePour::DroitsEtMontant => "droits_et_montant",
            ComptePour::OuvertureDesDroits => "ouverture_des_droits",
        }
    }
}

impl crate::fiche::FormeDeCase for ComptePour {}

/// UNE CASE QU'UNE RUBRIQUE FAIT SAISIR sur sa ligne (`saisie` de `rubriques-base.json`) : la quantite, le taux, les
/// heures, les jours (les dates de la ligne), la base, le taux de majoration, le montant, le genre d'un avantage en
/// nature. Une liste du logiciel, relevee dans `rubriques-base.json` ; une autre case se refuse a la lecture de la
/// rubrique. Leur ordre est celui de la ligne.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseSaisie {
    Quantite,
    Taux,
    Heures,
    Jours,
    Base,
    TauxMajoration,
    Montant,
    Genre,
}

impl CaseSaisie {
    /// Son nom ecrit.
    pub fn cle(self) -> &'static str {
        match self {
            CaseSaisie::Quantite => "quantite",
            CaseSaisie::Taux => "taux",
            CaseSaisie::Heures => "heures",
            CaseSaisie::Jours => "jours",
            CaseSaisie::Base => "base",
            CaseSaisie::TauxMajoration => "taux_majoration",
            CaseSaisie::Montant => "montant",
            CaseSaisie::Genre => "genre",
        }
    }
}

impl crate::fiche::FormeDeCase for CaseSaisie {}

/// D'OU VIENT UNE RUBRIQUE : livree avec le logiciel (`rubriques-base.json`), ou propre a la compagnie
/// (`config/rubriques-compagnie.json`). Une liste du logiciel, ecrite `base`, `compagnie`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OrigineDeRubrique {
    Base,
    Compagnie,
}

/// Le plafond annuel d'heures d'une rubrique (l'enseignement, annexe X) : selon l'age du salarie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlafondAnnuelHeures {
    pub moins_de_50_ans: i64,
    #[serde(rename = "50_ans_et_plus")]
    pub cinquante_ans_et_plus: i64,
}

/// LE CODE D'UNE RUBRIQUE : sa cle dans `rubriques` ou `au_mois` de `donnees/rubriques-base.json`, ou dans
/// `config/rubriques-compagnie.json` pour une rubrique propre a la compagnie ; une ligne de paie le porte pour dire de
/// quelle rubrique elle est. Il se lit d'un fichier ou d'une charge (`lire`, ou `serde` : la meme chaine). INFERENCE
/// (aucun texte n'en fixe la forme : c'est une cle du logiciel, et la compagnie nomme les siennes) : un code n'est
/// jamais vide ; rien de plus ne vaut pour tous les codes qui existent. Un code que le moteur compose lui-meme est une
/// constante (`du_moteur`), construite sans echec ; un code lu d'une donnee se lit (`lire`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, serde::Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct CodeDeRubrique(std::borrow::Cow<'static, str>);

impl CodeDeRubrique {
    /// Le code ecrit `texte` ; vide, il se refuse.
    pub fn lire(texte: &str) -> R<CodeDeRubrique> {
        if texte.is_empty() {
            return Err(socle::refus!("code de rubrique vide"));
        }
        Ok(CodeDeRubrique(std::borrow::Cow::Owned(texte.to_string())))
    }

    /// LE CODE D'UNE RUBRIQUE QUE LE MOTEUR COMPOSE, ecrit en Rust : une constante (`const X: CodeDeRubrique =
    /// CodeDeRubrique::du_moteur("…")`), dont le compilateur refuse un code vide.
    pub const fn du_moteur(code: &'static str) -> CodeDeRubrique {
        assert!(!code.is_empty(), "code de rubrique vide");
        CodeDeRubrique(std::borrow::Cow::Borrowed(code))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CodeDeRubrique {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for CodeDeRubrique {
    type Error = socle::echec::Echec;
    fn try_from(s: String) -> R<CodeDeRubrique> {
        CodeDeRubrique::lire(&s)
    }
}

impl From<CodeDeRubrique> for String {
    fn from(c: CodeDeRubrique) -> String {
        c.0.into_owned()
    }
}

/// La rubrique du cachet : celle que les annexes VIII et X convertissent en heures (`cumul::Constantes`).
pub const CACHET: CodeDeRubrique = CodeDeRubrique::du_moteur("cachet");

/// La rubrique de l'heure de travail d'un intermittent.
pub const HEURE: CodeDeRubrique = CodeDeRubrique::du_moteur("heure");

/// La rubrique du service de repetition.
pub const REPETITION: CodeDeRubrique = CodeDeRubrique::du_moteur("repetition");

/// Une forme de paie : ce que le bulletin appelle une ligne.
#[derive(Debug, Clone, PartialEq)]
pub struct Rubrique {
    pub code: CodeDeRubrique,
    pub libelle: String,
    pub unite: Unite,
    pub nature: NatureDeRubrique,
    pub saisie: Vec<CaseSaisie>,
    pub soumis: bool,
    pub imposable: bool,
    pub conges_spectacles: bool,
    pub source: String,
    pub sens: Sens,
    pub heures_forfait: Option<Decimal>,
    pub plafond: Option<Unite>,
    pub aem: Option<Unite>,
    pub dsn_type: Option<String>,
    pub dsn_autres_bruts: Option<crate::autre_revenu_brut::AutreElementDeRevenuBrut>,
    pub annexe: Option<crate::emplois::AnnexeChomage>,
    pub plafond_mensuel: Option<i64>,
    pub plafond_annuel_heures: Option<PlafondAnnuelHeures>,
    pub compte_pour: ComptePour,
    pub salaire_de_base: bool,
    pub retabli: bool,
    pub suspension: bool,
    pub arret_de_travail: bool,
    pub conges_pris: bool,
    pub dsn_bloc52: Option<crate::type_de_prime::TypeDePrime>,
    /// SA PIECE : `Piece::Permanent` pour une rubrique livree rangee dans `au_mois` de `rubriques-base.json`, que seul le
    /// calcul d'un contrat paye au mois paie, quand ses autres cases ne le disent pas : l'indemnite de conges d'un
    /// permanent (la caisse des Conges Spectacles paie ceux d'un intermittent, C. trav. D. 7121-38), les indemnites
    /// journalieres que la compagnie subrogee reverse (la subrogation est une case des permanents).
    pub piece: Option<Piece>,
    pub origine: OrigineDeRubrique,
    /// Ce que la ligne fait, que l'ecran dit sous elle quand son nom ne le dit pas (l'acompte).
    pub dit: Option<String>,
}

impl Rubrique {
    /// Les refus d'une rubrique mal formee.
    fn verifier(&self) -> R<()> {
        let code = &self.code;
        if self.source.is_empty() {
            return Err(mal_decrite(code, format!(
                "Rubrique {} : `source` est vide. Une rubrique dit d'ou vient \
                 sa regle, sinon personne ne pourra la verifier.", repr(code.as_str()))));
        }
        if self.conges_spectacles && !self.soumis {
            return Err(mal_decrite(code, format!(
                "Rubrique {} : `conges_spectacles` sans `soumis`. Les Conges \
                 Spectacles sont une cotisation : leur assiette ne peut pas \
                 depasser ce qui entre dans l'assiette des cotisations.", repr(code.as_str()))));
        }
        if self.heures_forfait.is_some() && self.unite == Unite::Heure {
            return Err(mal_decrite(code, format!(
                "Rubrique {} : `heures_forfait` sur une rubrique dont l'unite \
                 est l'heure. Les heures y sont un fait, pas un forfait : \
                 elles compteraient deux fois.", repr(code.as_str()))));
        }
        Ok(())
    }

    /// Ce qu'une ligne de cette rubrique, hors assiette, est pour le montant net social : la nature le dit
    /// (`frais`, `indemnite`, `retenue`). Une autre nature ne le dit pas d'une ligne hors assiette (une prime, un
    /// salaire exoneres sont-ils un revenu ? un avantage en nature hors assiette, quoi ?) : refus, regle 1.
    pub fn hors_assiette(&self) -> R<HorsAssiette> {
        match self.nature {
            NatureDeRubrique::Frais => Ok(HorsAssiette::Frais),
            NatureDeRubrique::Indemnite => Ok(HorsAssiette::Revenu),
            NatureDeRubrique::Retenue => Ok(HorsAssiette::Mouvement),
            autre => Err(mal_decrite(&self.code, format!(
                "Rubrique {} : hors assiette (`soumis` faux) et de nature {}. Le montant net social \
                 (S21.G00.58.004, bulletin) ne sait pas si cette ligne est un revenu, un remboursement de frais \
                 ou un mouvement de tresorerie, et ne le devine pas. Hors assiette, la nature doit etre `frais` \
                 (remboursement de frais professionnels : hors du montant net social), `indemnite` (une somme \
                 versee a l'occasion du travail, exoneree ou non : dans le montant net social) ou `retenue` \
                 (acompte, avance, saisie : hors du montant net social).",
                repr(self.code.as_str()), repr(autre.cle())))),
        }
    }

    /// La population a laquelle la rubrique est reservee : au mois, a l'activite, ou aucune.
    pub fn reservee_a(&self) -> Option<crate::natures::Paie> {
        if self.salaire_de_base
            || self.retabli
            || self.suspension
            || self.arret_de_travail
            || self.conges_pris
            || self.dsn_bloc52.is_some()
            || self.piece == Some(Piece::Permanent)
        {
            return Some(crate::natures::Paie::Mensuelle);
        }
        if self.unite == Unite::Cachet {
            return Some(crate::natures::Paie::Activite);
        }
        None
    }

    /// Le statut auquel la rubrique est reservee : le service et le cachet ne sont des unites de paie que pour un
    /// artiste. Les conventions payent au service les repetitions des artistes (1285, art. X.3.2 C et X.3.3,
    /// `atelier/reference/sources/kali-1285-x3-remuneration-artistes-20260929.md`) et au cachet leurs representations
    /// (3090, titre VII « Salaires minimaux », KALIARTI000049099195 de `kali-entrepot/` : « Artistes … Rémunération au
    /// cachet ») ; un technicien y est paye a l'heure, a la journee, a la semaine ou au mois (1285, titre VI,
    /// `kali-1285-titre-vi-duree-du-travail-20261102.md` ; 3090, titre III, art. 3.2, KALIARTI000028930755 : des services
    /// indivisibles comptes en heures, et le titre VII : « Techniciens … par heure, par mois » ; les grilles des
    /// techniciens de `minima-conventionnels.json`, aucune au service ni au cachet). L'assurance chomage ne connait le
    /// cachet que des artistes : annexe X, art. 3 (« Lorsque l'activité des artistes est déclarée sous la forme de
    /// cachets ») ; l'annexe VIII, celle des techniciens, ne compte que des heures (art. 3,
    /// `plafond-intermittents/unedic-annexe10-convention-20241115.txt`, `…annexe8…`).
    pub fn reservee_au_statut(&self) -> Option<crate::fiche::Statut> {
        matches!(self.unite, Unite::Service | Unite::Cachet).then_some(crate::fiche::Statut::Artiste)
    }

    /// Une ligne de cette rubrique doit dire ses heures : `heures` est dans
    /// sa `saisie` et aucun forfait n'en tient lieu (le service de repetition,
    /// la journee, la semaine, le mois, l'absence a la journee). Le catalogue
    /// le dit (`_heures` de `rubriques-base.json`) : « partout ailleurs les
    /// heures sont saisies, parce qu'elles sont un fait elles aussi ». Une
    /// ligne qui les tait se refuse (`engagement::engagements`) :
    /// sans elle, [`Rubrique::heures`] compterait zero en silence.
    pub fn heures_a_saisir(&self) -> bool {
        self.heures_forfait.is_none() && self.saisie.contains(&CaseSaisie::Heures)
    }

    /// Les heures que cette ligne fait compter, au total : le forfait
    /// l'emporte, sinon les heures saisies, sinon la quantite si l'unite est
    /// l'heure. Zero pour une rubrique qui demande ses heures et ne les a pas
    /// recues : la saisie qui la porte a deja ete refusee
    /// ([`Rubrique::heures_a_saisir`]).
    ///
    /// UNE convention : le `heures` d'une ligne de paie, comme
    /// `heures_forfait`, compte les heures PAR UNITE, et la quantite les
    /// multiplie. 14 services de 4 h : `quantite` 14, `heures` 4, soit 56 h.
    /// Un mois de permanent : `quantite` = la fraction du mois payee,
    /// `heures` = la quotite du contrat (`conges::lignes_du_mois`). Le produit
    /// se compte au centieme d'heure, comme la DSN l'ecrit.
    /// Le centieme d'heure se prend au plus pres, la moitie loin de zero ; le nombre rendu est dans sa forme la plus
    /// courte (`socle::num::court`).
    pub fn heures(&self, quantite: Decimal, heures_saisies: Option<Decimal>) -> Decimal {
        let centieme = |x: Decimal| socle::num::court(socle::num::mul(x, quantite).round_dp_with_strategy(2, rust_decimal::RoundingStrategy::MidpointAwayFromZero));
        if let Some(f) = self.heures_forfait {
            return centieme(f);
        }
        if let Some(h) = heures_saisies {
            return centieme(h);
        }
        if self.unite == Unite::Heure {
            return socle::num::court(quantite);
        }
        socle::num::court(Decimal::ZERO)
    }
}

// ------------------------------------------------------------------- lecture

/// Le refus d'une rubrique qui ne se construit pas : la phrase, et ce qui ne va pas a part.
#[track_caller]
fn mal_decrite(code: &CodeDeRubrique, detail: String) -> socle::echec::Echec {
    socle::echec::refus_avec(format!("La rubrique {} est mal décrite : le signaler au développeur.", repr(code.as_str())),Lieu { detail: Some(detail), ..Lieu::default() })
}

/// `rubriques-base.json`, entier : ses notes, les annexes VIII et X, les rubriques livrees (chacune une fiche que la
/// compagnie peut ajuster case par case, `ChampsRubrique`).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Base {
    #[serde(rename = "quoi")]
    _quoi: String,
    #[serde(rename = "note")]
    _note: String,
    annexes: Annexes,
    #[serde(deserialize_with = "socle::forme::en_ordre")]
    rubriques: Vec<(CodeDeRubrique, ChampsRubrique)>,
    /// LA PIECE D'UN SALARIE AU MOIS (`fiche::Piece::Permanent`) : les rubriques que seul le calcul d'un contrat paye au
    /// mois paie, rangees a part. Leur place dit leur piece ; aucune case ne la dit.
    #[serde(deserialize_with = "socle::forme::en_ordre")]
    au_mois: Vec<(CodeDeRubrique, ChampsRubrique)>,
}

impl Base {
    /// Chaque rubrique livree avec sa piece : la sienne pour celles qui sont rangees dans `au_mois`.
    fn rangees(self) -> Vec<(CodeDeRubrique, ChampsRubrique, Option<Piece>)> {
        let communes = self.rubriques.into_iter().map(|(c, v)| (c, v, None));
        communes.chain(self.au_mois.into_iter().map(|(c, v)| (c, v, Some(Piece::Permanent)))).collect()
    }
}

/// Le seuil et la fenetre des annexes VIII et X (`rubriques-base.json`, `annexes`).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Annexes {
    /// Les heures qui ouvrent les droits (507).
    pub seuil_heures: i64,
    /// La fenetre ou elles se comptent, en jours (365).
    pub fenetre_jours: i64,
    #[serde(rename = "note")]
    _note: String,
}

/// LA SEULE LECTURE DE `rubriques-base.json` : sa forme. Le refus dit le fichier et le chemin de la valeur fautive (la
/// rubrique, sa case : « rubriques.CACHET.unite »).
fn base() -> R<Base> {
    let b: Base = socle::forme::lire_fichier_en_phrase(&dossier_du_code().join("rubriques-base.json"), |chemin, detail| {
        let ou = if chemin.is_empty() { String::new() } else { format!(", à « {} »", chemin) };
        format!("Les rubriques livrées avec le logiciel (rubriques-base.json){} ne se lisent pas : {}. Le signaler au développeur.", ou, detail)
    })?;
    // La piece d'un salarie au mois est celle que le calcul compose (`RubriqueDuMois`) : ni plus, ni moins.
    let rangees: Vec<&str> = b.au_mois.iter().map(|(c, _)| c.as_str()).collect();
    let codes: Vec<CodeDeRubrique> = RubriqueDuMois::TOUTES.iter().map(|r| r.code()).collect();
    let du_calcul: Vec<&str> = codes.iter().map(CodeDeRubrique::as_str).collect();
    if rangees.len() != du_calcul.len() || !rangees.iter().all(|c| du_calcul.contains(c)) {
        return Err(socle::echec::refus_avec(
            "Les rubriques d'un salarié au mois ne sont pas celles que son calcul compose : le signaler au développeur.".to_string(),
            socle::echec::Lieu {
                fichier: Some("rubriques-base.json".into()),
                cle: Some("au_mois".into()),
                detail: Some(format!("la section porte {:?}, le calcul compose {:?} (`RubriqueDuMois`)", rangees, du_calcul)),
                ..Default::default()
            },
        ));
    }
    Ok(b)
}

/// LES RUBRIQUES DE LA PIECE D'UN SALARIE AU MOIS (`au_mois` de `rubriques-base.json`) : seul le calcul d'un contrat
/// paye au mois les compose, avec sa cle (`engagement::LignePaie::du_mois`, qui exige `natures::AuMois`) ; une carte ne
/// les saisit pas (`rubrique_a_saisir`). Leur code ne s'ecrit en Rust qu'ici.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RubriqueDuMois {
    /// L'indemnite des conges payes pris (L3141-24 ; la caisse des Conges Spectacles paie ceux d'un intermittent,
    /// D. 7121-38).
    IndemniteCongesPayes,
    /// Les indemnites journalieres que la compagnie subrogee reverse (la subrogation est une case des permanents).
    IjssNettes,
}

impl RubriqueDuMois {
    pub const TOUTES: [RubriqueDuMois; 2] = [RubriqueDuMois::IndemniteCongesPayes, RubriqueDuMois::IjssNettes];

    /// Le code `code` d'une ligne ecrite est-il le sien ? (Lire une ligne ne la compose pas.)
    pub fn nomme(self, code: &CodeDeRubrique) -> bool {
        self.code() == *code
    }

    /// Son code au catalogue : ne le prennent que les lignes qui la composent, avec la cle (`engagement::LignePaie`).
    pub fn code(self) -> CodeDeRubrique {
        match self {
            RubriqueDuMois::IndemniteCongesPayes => const { CodeDeRubrique::du_moteur("indemnite_conges_payes") },
            RubriqueDuMois::IjssNettes => const { CodeDeRubrique::du_moteur("ijss_nettes") },
        }
    }
}

/// UNE RUBRIQUE QU'UNE CARTE SAISIT, telle qu'elle vaut a `jour` : celle de la piece d'un salarie au mois ne se saisit
/// pas, le calcul qui tient sa cle la compose (`RubriqueDuMois`).
pub fn rubrique_a_saisir(racine: &Path, code: &CodeDeRubrique, jour: &Jour) -> R<Rubrique> {
    let r = rubrique(racine, code, jour)?;
    if r.piece.is_some() {
        return Err(socle::refus!(
            "La rubrique {} ne se saisit pas : le calcul d'un contrat payé au mois la pose lui-même.",
            repr(code.as_str())
        ));
    }
    Ok(r)
}

/// LES RUBRIQUES LIVREES, construites, sans celles d'aucune compagnie : ce que le logiciel sait d'une rubrique avant
/// toute configuration (la table de lecture des bulletins s'y rapporte).
pub fn livrees() -> R<Vec<Rubrique>> {
    base()?.rangees().into_iter().map(|(c, v, piece)| construire(c, v, OrigineDeRubrique::Base, piece)).collect()
}

/// Le seuil et la fenetre des annexes VIII et X.
pub fn annexes() -> R<Annexes> {
    Ok(base()?.annexes)
}

/// `config/rubriques-compagnie.json` : les rubriques propres a la compagnie, chacune une fiche datee (`RubriqueDatee`).
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RubriquesDeLaCompagnie {
    #[serde(deserialize_with = "socle::forme::en_ordre")]
    rubriques: Vec<(CodeDeRubrique, RubriqueDatee)>,
}

/// Le fichier des rubriques de la compagnie, s'il existe ; absent, il n'en dit aucune.
fn de_la_compagnie(racine: &Path) -> R<RubriquesDeLaCompagnie> {
    let p = racine.join(socle::fichiers::CONFIG).join(RUBRIQUES_COMPAGNIE);
    if !socle::fichiers::existe(&p) {
        return Ok(RubriquesDeLaCompagnie::default());
    }
    crate::fiche::lire_fichier(&p, &format!("config/{}", RUBRIQUES_COMPAGNIE))
}

/// LA RUBRIQUE DU CALCUL, construite de ses cases (`ChampsRubrique`), sa seule construction : une case que la rubrique
/// doit dire, ou qui ne dit pas ce que sa forme attend, se refuse en la nommant.
fn construire(code: CodeDeRubrique, f: ChampsRubrique, origine: OrigineDeRubrique, piece: Option<Piece>) -> R<Rubrique> {
    let faute = |cle: &str, quoi: &str| mal_decrite(&code, format!("Rubrique {} : `{}` {}.", repr(code.as_str()), cle, quoi));
    fn dite<T>(c: Option<T>, cle: &str, faute: &dyn Fn(&str, &str) -> socle::echec::Echec) -> R<T> {
        c.ok_or_else(|| faute(cle, "manque"))
    }
    let r = Rubrique {
        code: code.clone(),
        libelle: dite(f.libelle, "libelle", &faute)?,
        unite: dite(f.unite, "unite", &faute)?,
        nature: dite(f.nature, "nature", &faute)?,
        saisie: dite(f.saisie, "saisie", &faute)?,
        soumis: dite(f.soumis, "soumis", &faute)?,
        imposable: dite(f.imposable, "imposable", &faute)?,
        conges_spectacles: dite(f.conges_spectacles, "conges_spectacles", &faute)?,
        source: dite(f.source, "source", &faute)?,
        sens: dite(f.sens, "sens", &faute)?,
        heures_forfait: f.heures_forfait.map(|m| m.court()).transpose()?,
        plafond: f.plafond,
        aem: f.aem,
        dsn_type: f.dsn_type,
        dsn_autres_bruts: f.dsn_autres_bruts,
        annexe: f.annexe,
        plafond_mensuel: f.plafond_mensuel,
        plafond_annuel_heures: f.plafond_annuel_heures,
        compte_pour: dite(f.compte_pour, "compte_pour", &faute)?,
        salaire_de_base: dite(f.salaire_de_base, "salaire_de_base", &faute)?,
        retabli: dite(f.retabli, "retabli", &faute)?,
        suspension: dite(f.suspension, "suspension", &faute)?,
        arret_de_travail: dite(f.arret_de_travail, "arret_de_travail", &faute)?,
        conges_pris: dite(f.conges_pris, "conges_pris", &faute)?,
        dsn_bloc52: f.dsn_bloc52,
        piece,
        origine,
        dit: f.dit,
    };
    r.verifier()?;
    Ok(r)
}

/// Le fichier des rubriques propres a la compagnie, sous `config/`.
pub const RUBRIQUES_COMPAGNIE: &str = "rubriques-compagnie.json";

/// Une rubrique de la compagnie a-t-elle une valeur datee ? Sinon le catalogue est le meme a toute date.
fn rubriques_datees(racine: &Path) -> R<bool> {
    struct UneDatee(bool);
    impl VoirListe for UneDatee {
        fn liste<T: serde::Serialize>(&mut self, _: crate::fiche::CaseDeFiche, liste: &[Datee<T>]) {
            self.0 |= liste.iter().any(|e| e.depuis.is_some());
        }
    }
    let mut datee = UneDatee(false);
    for (_, r) in de_la_compagnie(racine)?.rubriques {
        r.voir_listes(&mut datee);
    }
    Ok(datee.0)
}

/// LE CATALOGUE A `jour` : les rubriques livrees, chacune sous les cases que la compagnie en dit a cette date, puis les
/// siennes (`config/rubriques-compagnie.json`, datees comme toute la configuration : une rubrique changee a une date ne
/// change pas les paies d'avant ; une rubrique qui ne dit encore rien a `jour` n'existe pas).
fn construire_catalogue(racine: &Path, jour: &Jour) -> R<Vec<Rubrique>> {
    // code -> (cases, piece, propre a la compagnie) ; une rubrique de la compagnie n'est dans aucune piece, et celle
    // qu'elle ajuste garde la sienne.
    let mut brut: Vec<(CodeDeRubrique, ChampsRubrique, Option<Piece>, bool)> = base()?.rangees().into_iter().map(|(c, v, p)| (c, v, p, false)).collect();
    if socle::fichiers::est_dossier(racine) {
        for (c, datee) in de_la_compagnie(racine)?.rubriques {
            let v = datee.au(jour);
            if v.est_vide() {
                continue;
            }
            match brut.iter_mut().find(|e| e.0 == c) {
                Some(e) => e.1.par_dessus(&v),
                None => brut.push((c, v, None, true)),
            }
        }
    }
    let mut out = Vec::new();
    for (code, d, piece, propre) in brut {
        out.push(construire(code, d, if propre { OrigineDeRubrique::Compagnie } else { OrigineDeRubrique::Base }, piece)?);
    }
    out.sort_by_key(|r| r.libelle.to_lowercase());
    Ok(out)
}

fn empreinte(p: &Path) -> String {
    format!("{}:{:?}", p.display(), signature(p))
}

/// Un catalogue construit, et si les rubriques de la compagnie y sont datees (il depend alors du jour lu).
type EnCache = (bool, Arc<Vec<Rubrique>>);
static CATALOGUES: OnceLock<Mutex<HashMap<String, EnCache>>> = OnceLock::new();

/// Les rubriques disponibles a `jour` (la date de la periode payee : les rubriques de la compagnie sont datees), par
/// libelle. `racine` est la racine des donnees de la
/// compagnie (`ctx.racine`). Construit une fois par etat des deux fichiers (et par jour, si la compagnie a date une
/// de ses rubriques).
pub fn catalogue(racine: &Path, jour: &Jour) -> R<Arc<Vec<Rubrique>>> {
    let fichiers = format!(
        "{}|{}",
        empreinte(&dossier_du_code().join("rubriques-base.json")),
        empreinte(&racine.join(socle::fichiers::CONFIG).join(RUBRIQUES_COMPAGNIE)),
    );
    let cache = CATALOGUES.get_or_init(|| Mutex::new(HashMap::new()));
    let connue = cache.lock().unwrap().get(&fichiers).map(|(datee, _)| *datee);
    let datee = match connue {
        Some(d) => d,
        None => rubriques_datees(racine)?,
    };
    let cle = if datee { format!("{}|{}", fichiers, jour.iso()) } else { fichiers.clone() };
    if let Some((_, c)) = cache.lock().unwrap().get(&cle) {
        // Un catalogue servi par le cache a lu ses fichiers quand meme, pour l'empreinte du calcul -- seulement quand un
        // calcul suivi les note (sinon, autant de lectures du disque pour rien a chaque rubrique).
        if !socle::fichiers::registre_ouvert() {
            return Ok(c.clone());
        }
        socle::fichiers::est_dossier(racine);
        for p in [
            dossier_du_code().join("rubriques-base.json"),
            racine.join(socle::fichiers::CONFIG).join(RUBRIQUES_COMPAGNIE),
        ] {
            if socle::fichiers::existe(&p) {
                socle::fichiers::noter_si_besoin(&p);
            }
        }
        return Ok(c.clone());
    }
    let c = Arc::new(construire_catalogue(racine, jour)?);
    let mut g = cache.lock().unwrap();
    if g.len() > 64 {
        g.clear();
    }
    g.insert(fichiers, (datee, c.clone()));
    g.insert(cle, (datee, c.clone()));
    Ok(c)
}

/// Une rubrique par son code, telle qu'elle vaut a `jour`. Refuse un code inconnu : mieux vaut s'arreter que payer une
/// ligne dont on ignore le regime.
pub fn rubrique(racine: &Path, code: &CodeDeRubrique, jour: &Jour) -> R<Rubrique> {
    let cat = catalogue(racine, jour)?;
    if let Some(r) = cat.iter().find(|r| r.code == *code) {
        return Ok(r.clone());
    }
    let mut connues: Vec<&str> = cat.iter().map(|r| r.code.as_str()).collect();
    connues.sort();
    Err(socle::refus!(
        "Rubrique inconnue : {}. Connues : {}.", repr(code.as_str()), connues.join(", ")))
}

/// Le code S21.G00.54.001 de cette ligne de paie, ou `None` : `genre` de la
/// saisie precise lequel des cinq avantages en nature.
pub fn code_54(r: &Rubrique, genre: Option<&str>) -> R<Option<AutreElementDeRevenuBrut>> {
    let Some(code) = r.dsn_autres_bruts else { return Ok(None) };
    let Some(genre) = genre.map(str::trim).filter(|g| !g.is_empty()) else {
        return Ok(Some(code));
    };
    if !code.avantage_en_nature() {
        return Err(socle::echec::a_la_cle(&format!("La ligne « {} » dit le genre d'un avantage en nature, et ce n'en est pas un : retirer ce genre.", r.libelle), "genre", repr_texte(genre)));
    }
    let avantages: Vec<AutreElementDeRevenuBrut> = AutreElementDeRevenuBrut::TOUS.iter().copied().filter(|a| a.avantage_en_nature()).collect();
    match avantages.iter().copied().find(|a| a.code() == genre) {
        Some(a) => Ok(Some(a)),
        None => Err(socle::echec::a_la_cle(
            &format!("La ligne « {} » porte un genre d'avantage en nature inconnu : le choisir dans la liste.", r.libelle),
            "genre",
            format!("{} ; connus : {}", repr_texte(genre), avantages.iter().map(|a| a.code()).collect::<Vec<_>>().join(", ")),
        )),
    }
}
