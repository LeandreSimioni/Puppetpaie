//! Le prelevement a la source : le taux et son identifiant, tels que la DGFiP les donne (les taux recus, dans la fiche du
//! salarie, `taux_pas`, un taux recu par valeur datee du jour ou il commence de valoir), et la grille de l'article 204 H du CGI quand aucun ne vaut. Ce que le calcul d'un engagement et la
//! DSN lisent.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize, Serializer};

use crate::contexte::{Contexte, IdSalarie};
use crate::fiche::{Lue, Montant, TauxPas};
use crate::forme_bareme::{GrillePas, PasParDefaut, Pile};
use crate::type_taux_pas::TypeDeTauxPas;
use socle::date::{jours_du_mois, Jour};
use socle::mois::Mois;
use socle::num::nombre;
use socle::fichiers::{repr, LectureDatee, ValeurLue};
use crate::contexte::CleLue;
use socle::echec::{refus, Echec, R};
use socle::num::{eur, pct, sub, Decimal};

/// Un refus du module. `SansGrille` : un bareme sans la grille de l'article 204 H (le prelevement ne se devine pas, le
/// calcul refuse).
#[derive(Debug)]
pub enum ErreurPas {
    SansGrille(Echec),
    Autre(Echec),
}

impl From<Echec> for ErreurPas {
    fn from(e: Echec) -> ErreurPas {
        ErreurPas::Autre(e)
    }
}

impl From<ErreurPas> for Echec {
    fn from(e: ErreurPas) -> Echec {
        match e {
            ErreurPas::SansGrille(e) | ErreurPas::Autre(e) => e,
        }
    }
}

type RPas<T> = Result<T, ErreurPas>;

/// UN TAUX DE PAS, dans la forme de la norme (S21.G00.50.006 : un ou deux chiffres, un point, deux chiffres,
/// `[0-9]{1,2}\.[0-9]{2}`, `formats-dsn-*-base.json` ; `atelier/reference/norme-dsn/texte/bloc-S21.G00.50.txt`). Il se lit
/// et s'ecrit dans cette forme, octet pour octet ; un autre texte n'est pas un taux de PAS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TauxDePas(Decimal);

impl TauxDePas {
    /// Le taux ecrit `texte`, s'il a la forme de la norme ; `None` sinon.
    pub fn lire(texte: &str) -> Option<TauxDePas> {
        let (entier, decimales) = texte.split_once('.')?;
        let chiffres = |t: &str| t.bytes().all(|b| b.is_ascii_digit());
        if !(1..=2).contains(&entier.len()) || decimales.len() != 2 || !chiffres(entier) || !chiffres(decimales) {
            return None;
        }
        // Ses chiffres, lus un a un : le taux en centiemes de pourcent.
        let centiemes = entier.bytes().chain(decimales.bytes()).fold(0i64, |n, b| n * 10 + i64::from(b - b'0'));
        Some(TauxDePas(Decimal::new(centiemes, 2)))
    }

    /// Le taux `d`, s'il s'ecrit dans la forme de la norme (`lire`) ; `None` sinon.
    pub fn du_decimal(d: Decimal) -> Option<TauxDePas> {
        TauxDePas::lire(&d.to_string())
    }

    /// Le taux, en pourcent.
    pub fn decimal(self) -> Decimal {
        self.0
    }
}

impl std::fmt::Display for TauxDePas {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Serialize for TauxDePas {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for TauxDePas {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<TauxDePas, D::Error> {
        let t = String::deserialize(d)?;
        TauxDePas::lire(&t).ok_or_else(|| serde::de::Error::custom(format!("{} n'est pas un taux de PAS (deux chiffres au plus, un point, deux chiffres)", repr(&t))))
    }
}

impl crate::fiche::FormeDeCase for TauxDePas {}

/// Un taux qui vaut : taux, type, identifiant, tels que le compte rendu les donne.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Triplet {
    pub taux: Option<TauxDePas>,
    pub type_taux: Option<TypeDeTauxPas>,
    pub identifiant: Option<String>,
}

// ------------------------------------------------------------- peremption

/// Un taux transmis par la DGFiP vaut jusqu'a la fin du deuxieme mois suivant
/// celui de sa mise a disposition : le debiteur « applique le taux calculé par
/// l'administration fiscale, au plus tard le deuxième mois suivant sa
/// transmission » (CGI, art. 1671, 2, `atelier/reference/sources/
/// code-general-des-impots/articles-20261021.md`) ; « le taux de prélèvement est
/// applicable jusqu'à la fin du deuxième mois qui suit sa mise à disposition » :
/// mis a disposition le 10 fevrier, il est « valide jusqu'au 30 avril »
/// (BOI-IR-PAS-30-10-20, § 70, `atelier/reference/sources/bofip/
/// BOI-IR-PAS-30-10-20-20181203.html`).
pub const VALIDITE_MOIS: u32 = 2;

/// Le jour de la mise a disposition d'un taux recu (`mis_a_disposition`, AAAAMMJJHHMMSS) : d'ou court sa validite.
pub fn mise_a_disposition(e: &TauxPas) -> Option<Jour> {
    e.mis_a_disposition.map(|i| i.jour_de_paris())
}

/// Dernier jour ou ce taux est celui du salarie ; `None` si on ne sait pas.
pub fn fin_de_validite(e: &TauxPas) -> Option<Jour> {
    let debut = mise_a_disposition(e)?;
    let mois = debut.mois + VALIDITE_MOIS;
    let (an, m) = (debut.an + ((mois - 1) / 12) as i32, (mois - 1) % 12 + 1);
    Jour::nouveau(an, m, jours_du_mois(an, m))
}

/// Un taux qu'on ne sait pas dater est traite comme perime ; le bareme (tout
/// type autre que 01) ne se perime pas.
pub fn est_perime(e: &TauxPas, le: Jour) -> bool {
    if !e.r#type.is_some_and(TypeDeTauxPas::transmis) {
        return false;
    }
    fin_de_validite(e).is_none_or(|fin| le > fin)
}

// ------------------------------------------------------------------ lecture

/// Vrai seulement si la fiche est la ET qu'elle n'a pas de NIR.
pub fn sans_nir(ctx: &Contexte, cle: &IdSalarie) -> bool {
    ctx.salaries.get(cle).is_some_and(|f| f.nir.portee().is_none())
}

/// Le jour auquel on demande si un taux vaut encore : celui du contexte.
pub fn le_jour(ctx: &Contexte, le: Option<Jour>) -> Jour {
    match le {
        Some(j) => j,
        None => {
            socle::fichiers::noter_lecture(&CleLue::Date, ValeurLue::Jour(ctx.date));
            ctx.date
        }
    }
}

/// LES TAUX RECUS POUR LE SALARIE `cle`, du premier mis a disposition au dernier (a instant egal, dans l'ordre ou ils
/// se sont ecrits), ceux de sa fiche s'il en a une. Une valeur de la fiche qui n'est pas un taux (la case vide d'avant le
/// premier) n'en est pas un.
pub fn recus(ctx: &Contexte, cle: &IdSalarie) -> R<Vec<TauxPas>> {
    let mut ecrits: Vec<_> = ctx.salarie_brut(cle).into_iter().flat_map(|fiche| fiche.taux_pas.iter().flatten()).collect();
    ecrits.sort_by_key(|e| e.depuis);
    let mut r: Vec<TauxPas> = ecrits.into_iter().filter_map(|e| e.valeur.dite()).cloned().collect();
    r.sort_by_key(|v| v.mis_a_disposition);
    Ok(r)
}


/// LE TAUX EN RETOUR DE LA DSN D'UN MOIS VAUT POUR LES MOIS QUI SUIVENT, JAMAIS POUR CELUI QU'ELLE DECLARE. Le compte
/// rendu est « établi par l'administration fiscale en retour de chaque déclaration sociale nominative » (BOI-IR-PAS-30-10-20,
/// § 20, `atelier/reference/sources/bofip/BOI-IR-PAS-30-10-20-extrait-taux.md`) ; le taux de celui qui repond a la DSN de
/// janvier sert a « précompter la retenue à la source sur les revenus versés au titre des mois de février, mars et avril »
/// (§ 70, exemple 1). La DSN d'un mois a deja declare le taux « précompté appliqué » a chacun de ses versements
/// (S21.G00.50.006, `atelier/reference/norme-dsn/texte/bloc-S21.G00.50.txt`) ; le compte rendu qui lui repond porte « un
/// identifiant propre » (CSS, art. R. 133-13, V, 1°, `atelier/reference/sources/code-de-la-securite-sociale/
/// articles-20261030.md`) : s'il valait pour les versements qu'elle declare, chaque DSN deposee avant eux serait a
/// refaire par sa propre reponse, et son annule-et-remplace par la sienne. INFERENCE (les textes supposent la DSN deposee
/// apres le versement) : un taux en retour de la DSN du mois `M` ne vaut pas pour la paie de `M`, ni d'un mois d'avant.
fn en_retour_d_un_mois_qui_n_est_pas_avant(e: &TauxPas, mois: Option<Mois>) -> bool {
    mois.zip(e.en_retour_de.dite()).is_some_and(|(m, retour)| *retour >= m)
}

/// Le jour ou un taux recu commence de valoir dans la fiche (`depuis` de sa valeur datee) : le jour de sa mise a
/// disposition `mad`, et, en retour de la DSN du mois `en_retour_de`, pas avant le premier jour du mois qui suit
/// (`en_retour_d_un_mois_qui_n_est_pas_avant`). Deux comptes rendus du meme jour, l'un pour le mois, l'autre en retour
/// de sa DSN, ne s'ecrasent donc pas.
pub fn date_d_effet(mad: Jour, en_retour_de: Option<Mois>) -> Jour {
    match en_retour_de {
        Some(m) => m.suivant().premier_jour().max(mad),
        None => mad,
    }
}

/// LE TAUX DU SALARIE AU JOUR `le`, pour la paie du mois `mois` : de ceux qu'il a recus, le dernier mis a disposition au
/// plus tard ce jour-la, hors ceux qui repondent a la DSN de ce mois ou d'un mois d'apres
/// (`en_retour_d_un_mois_qui_n_est_pas_avant`). Le debiteur « applique le taux calculé par l'administration fiscale, au
/// plus tard le deuxième mois suivant sa transmission » (CGI, art. 1671, 2, `atelier/reference/sources/
/// code-general-des-impots/articles-20261021.md`) : un taux vaut des sa mise a disposition (BOI-IR-PAS-30-10-20, § 70 et
/// 80, `atelier/reference/sources/bofip/BOI-IR-PAS-30-10-20-extrait-taux.md`), pour toute retenue faite a partir de ce
/// jour ; la retenue est « effectuée par le débiteur lors du paiement » (CGI, art. 1671, 1, meme fichier) : un taux
/// transmis apres le paiement d'un salaire ne vaut pas pour lui. Aucun taux recu avant ce jour : aucun (la grille de
/// l'article 204 H s'applique).
fn en_vigueur<'a>(recus: &'a [TauxPas], le: Jour, mois: Option<Mois>) -> Option<&'a TauxPas> {
    let mis_a_disposition: Vec<&TauxPas> =
        recus.iter().filter(|e| mise_a_disposition(e).is_some_and(|d| d <= le) && !en_retour_d_un_mois_qui_n_est_pas_avant(e, mois)).collect();
    let transmis = |e: &&TauxPas| e.r#type.is_some_and(TypeDeTauxPas::transmis);
    // Un compte rendu sans taux transmis ne fait pas oublier un taux transmis encore valide : la DGFiP releve comme une
    // erreur l'« utilisation d'un taux par defaut alors meme que le collecteur dispose d'un taux transmis par la DGFiP qui
    // est encore valide » (DGFiP, « Les comptes rendus metier (CRM) pour le PAS »,
    // `atelier/reference/sources/dgfip-pas/dgfip-comptes-rendus-metier-pas-20261111.pdf`).
    match mis_a_disposition.last() {
        Some(dernier) if !transmis(dernier) => match mis_a_disposition.iter().rev().find(|e| transmis(e) && !est_perime(e, le)) {
            Some(valide) => Some(*valide),
            None => Some(*dernier),
        },
        dernier => dernier.copied(),
    }
}

// ------------------------------------------ la grille de l'article 204 H du CGI

pub const ZONE_DEFAUT: &str = "metropole";

/// La section des taux par defaut du bareme (`pas_taux_par_defaut`).
fn section(bareme: &Pile) -> RPas<&PasParDefaut> {
    bareme.pas_taux_par_defaut.as_ref().ok_or_else(|| {
        ErreurPas::SansGrille(refus(
            "Le barème ne donne pas les taux par défaut du prélèvement à la \
             source : sans eux, le moteur ne sait pas quel taux appliquer à qui \
             n'a pas de taux de la DGFiP. Le barème livré de l'année en cours \
             les donne : mettre le logiciel à jour.",
        ))
    })
}

/// L'abattement contrats courts en vigueur : la moitie du SMIC net imposable (le bareme le porte a sa date).
pub fn abattement(bareme: &Pile) -> RPas<Decimal> {
    section(bareme)?.abattement_contrats_courts.map(|n| n.0).ok_or_else(|| ErreurPas::Autre(socle::fichiers::absent("abattement_contrats_courts")))
}

/// La grille en vigueur a cette date pour cette zone (elles changent au 1er mai).
pub fn grille<'a>(bareme: &'a Pile, le: Option<Jour>, zone: &str) -> RPas<&'a GrillePas> {
    let grilles = &section(bareme)?.grilles;
    let mut meilleure: Option<&GrillePas> = None;
    for g in grilles.iter().filter(|g| g.zone == zone) {
        if let Some(l) = le {
            if g.depuis > l {
                continue;
            }
        }
        // `max` garde la premiere des egales.
        if meilleure.is_none_or(|m| g.depuis > m.depuis) {
            meilleure = Some(g);
        }
    }
    meilleure.ok_or_else(|| {
        let connues: BTreeSet<&str> = grilles.iter().map(|g| g.zone.as_str()).collect();
        ErreurPas::SansGrille(socle::refus!(
            "Aucune grille de taux par defaut pour la zone {}{}. Zones \
             connues du bareme : {}.",
            repr(zone),
            match le {
                Some(j) => format!(" au {}", j.iso()),
                None => String::new(),
            },
            connues.into_iter().collect::<Vec<_>>().join(", ")
        ))
    })
}

/// `(taux, type de taux)` pour un montant DEJA abattu.
pub fn taux_par_defaut(bareme: &Pile, montant: Decimal, le: Option<Jour>, zone: &str) -> RPas<(Decimal, TypeDeTauxPas)> {
    let g = grille(bareme, le, zone)?;
    for t in &g.tranches {
        let (bas, haut, taux) = match t.as_slice() {
            [Some(b), h, Some(x)] => (b.0, h.map(|h| h.0), x.0),
            _ => {
                return Err(ErreurPas::Autre(socle::echec::refus_avec(
                    "Une tranche de la grille du PAS ne se lit pas : elle porte un bas, un haut et un taux.",
                    socle::echec::Lieu { cle: Some("pas_taux_par_defaut".to_string()), detail: Some(format!("{:?}", t)), ..Default::default() },
                )))
            }
        };
        // La derniere tranche n'a pas de haut.
        if montant >= bas && haut.is_none_or(|h| montant < h) {
            // Le taux de la grille (CGI 204 H, III) a deux decimales au plus : il s'ecrit avec ses deux decimales.
            let mut taux = taux.round_dp(2);
            taux.rescale(2);
            return Ok((taux, g.genre));
        }
    }
    // Seul un montant negatif tombe ici : taux zero.
    Ok((Decimal::new(0, 2), g.genre))
}

/// LE PRELEVEMENT D'UN VERSEMENT, par l'un des deux chemins : tel que la paie l'ecrit et le relit (les montants en
/// texte, `socle::num::nombre`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Versement {
    pub taux: Montant,
    #[serde(rename = "type")]
    pub type_taux: Option<TypeDeTauxPas>,
    pub identifiant: Option<String>,
    #[serde(serialize_with = "en_nombre", deserialize_with = "socle::forme::decimal")]
    pub abattement: Decimal,
    #[serde(serialize_with = "en_nombre", deserialize_with = "socle::forme::decimal")]
    pub montant_soumis: Decimal,
    #[serde(serialize_with = "en_nombre", deserialize_with = "socle::forme::decimal")]
    pub prelevement: Decimal,
}

fn en_nombre<S: Serializer>(x: &Decimal, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&nombre(*x))
}

pub fn plancher(x: Decimal, zero: Decimal) -> Decimal {
    if zero > x {
        zero
    } else {
        x
    }
}

/// Tout le bloc 50 cote PAS quand aucun taux DGFiP ne s'applique ; pas
/// d'identifiant (S21.G00.50.008 est interdite hors taux DGFiP).
pub fn rubriques_par_defaut(
    bareme: &Pile,
    remuneration_nette_fiscale: Decimal,
    le: Option<Jour>,
    contrat_court: bool,
    zone: &str,
) -> RPas<Versement> {
    let rnf = eur(remuneration_nette_fiscale);
    let zero = Decimal::new(0, 2);
    let abat = if contrat_court { abattement(bareme)? } else { zero };
    // L'abattement ne creuse pas : il se limite a ce qu'il y a a abattre.
    let abat = eur(abat.min(plancher(rnf, zero)));
    let soumis = eur(plancher(sub(rnf, abat), zero));
    let (taux, type_taux) = taux_par_defaut(bareme, soumis, le, zone)?;
    let prelevement = pct(soumis, taux);
    Ok(Versement {
        taux: Montant::Texte(taux.to_string()),
        type_taux: Some(type_taux),
        identifiant: None,
        abattement: abat,
        montant_soumis: soumis,
        prelevement,
    })
}

// ------------------------------------------- lequel des deux chemins, et pour
//                                              quel versement

/// Le triplet a declarer pour un versement du jour `le` de la paie du mois `mois`, ou `None` : pas de taux DGFiP
/// applicable, c'est la grille. Un taux perime compte pour rien.
/// UN CALCUL DEPEND DU TAUX DE SON SALARIE A SON JOUR : il le note (`pas <cle> <jour> <mois>`, `empreinte`), et un taux
/// recu qui le change le fait recalculer.
pub fn taux_dgfip(ctx: &Contexte, cle: &IdSalarie, le: Option<Jour>, mois: Option<Mois>) -> R<Option<Triplet>> {
    let t = triplet(ctx, cle, le, mois)?;
    if let Some(j) = le {
        socle::fichiers::noter_lecture(&CleLue::Datee(LectureDatee::Pas { salarie: cle.clone(), le: j, mois }), empreinte_du_triplet(&t));
    }
    Ok(t)
}

fn triplet(ctx: &Contexte, cle: &IdSalarie, le: Option<Jour>, mois: Option<Mois>) -> R<Option<Triplet>> {
    let s = situation(ctx, cle, le, mois)?;
    match s.situation {
        SituationDuTaux::Valide => Ok(Some(Triplet { taux: s.taux, type_taux: s.type_taux, identifiant: s.identifiant })),
        SituationDuTaux::Perime | SituationDuTaux::Bareme | SituationDuTaux::Inconnu | SituationDuTaux::SansNir => Ok(None),
    }
}

fn empreinte_du_triplet(t: &Option<Triplet>) -> ValeurLue {
    ValeurLue::Composee(match t {
        Some(t) => socle::ecriture::json_en_ligne(&(&t.taux, &t.type_taux, &t.identifiant)),
        None => "aucun".into(),
    })
}

/// Ce que `taux_dgfip` a note pour le salarie `cle` au jour `le` (`LectureDatee::Pas`), relu maintenant : l'empreinte
/// tient tant qu'il est le meme.
pub fn empreinte(ctx: &Contexte, cle: &IdSalarie, le: Jour, mois: Option<Mois>) -> R<ValeurLue> {
    Ok(empreinte_du_triplet(&triplet(ctx, cle, Some(le), mois)?))
}

/// La fiche d'un salarie sans ses taux recus : ce qu'une paie lit de la fiche hors du taux, qu'elle lit au jour de son
/// versement (`taux_dgfip`). Un taux recu ne fait recalculer que la paie dont il change le taux.
pub fn sans_taux_recus(fiche: &crate::fiche::ChampsSalarie) -> crate::fiche::ChampsSalarie {
    crate::fiche::ChampsSalarie { taux_pas: None, ..fiche.clone() }
}

/// Le prelevement d'UN versement. Un triplet s'applique tel quel, sur la
/// remuneration nette fiscale entiere, sans abattement.
pub fn du_versement(
    taux: Option<&Triplet>,
    bareme: &Pile,
    remuneration_nette_fiscale: Decimal,
    le: Option<Jour>,
    contrat_court: bool,
    zone: &str,
) -> RPas<Versement> {
    let Some(t) = taux else {
        return rubriques_par_defaut(bareme, remuneration_nette_fiscale, le, contrat_court, zone);
    };
    let taux = t.taux.ok_or_else(|| refus("nombre attendu, lu null"))?;
    // UNE REMUNERATION NETTE FISCALE NEGATIVE N'EST PAS UNE ASSIETTE NEGATIVE. Fiche DSN 1810 « Remuneration nette fiscale
    // negative » (`atelier/reference/sources/suspension-non-remuneree/net-entreprises-fiche1810-rnf-negative-pas-20250322.txt`) :
    // « La rubrique « Montant soumis au PAS - S21.G00.50.013 » ne peut pas etre renseignee avec une valeur negative. Il
    // convient alors de renseigner [...] a 0.00 » (l'exemple : 50.002 = -180.00, 50.009 = 0.00, 50.013 = 0.00) ; le cahier
    // technique refuse le signe moins en 50.009 et 50.013 et l'accepte en 50.002 (`norme-dsn/texte/bloc-S21.G00.50.txt`).
    // La grille (`rubriques_par_defaut`) plancher deja a zero ; le taux DGFiP aussi. Le cas : un mois sans salaire ou la
    // part patronale de sante reste due et se reintegre (CGI 83, 1° quater, meme dossier, `legi-extrait-code-general-des-
    // impots-20261015.md`).
    let soumis = plancher(eur(remuneration_nette_fiscale), Decimal::new(0, 2));
    Ok(Versement {
        taux: Montant::Texte(taux.to_string()),
        type_taux: t.type_taux,
        identifiant: t.identifiant.clone(),
        abattement: Decimal::new(0, 2),
        montant_soumis: soumis,
        prelevement: pct(soumis, taux.decimal()),
    })
}

// ------------------------------------------------------------- diagnostic

/// OU EN EST LE TAUX D'UN SALARIE.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SituationDuTaux {
    /// Un taux transmis par la DGFiP, en cours de validite.
    Valide,
    /// Un taux transmis par la DGFiP, perime.
    Perime,
    /// Un taux recu d'un autre type que 01 : la grille s'applique.
    Bareme,
    /// Aucun taux recu.
    Inconnu,
    /// Pas de NIR dans la fiche.
    SansNir,
}

impl SituationDuTaux {
    pub fn libelle(self) -> &'static str {
        match self {
            SituationDuTaux::Valide => "taux DGFiP en cours de validite",
            SituationDuTaux::Perime => "taux DGFiP perime -- ce n'est plus le taux du salarie",
            SituationDuTaux::Bareme => "aucun taux personnalise : c'est le bareme qui s'applique",
            SituationDuTaux::Inconnu => "aucun taux connu",
            SituationDuTaux::SansNir => "pas de NIR : la DGFiP n'a personne a qui repondre",
        }
    }
}

/// OU EN EST LE TAUX D'UN SALARIE : sa situation, le taux en vigueur tel qu'il est ecrit, et ce qu'il faudrait demander.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Situation {
    pub salarie: IdSalarie,
    pub situation: SituationDuTaux,
    pub libelle: &'static str,
    pub taux: Option<TauxDePas>,
    #[serde(rename = "type")]
    pub type_taux: Option<TypeDeTauxPas>,
    pub identifiant: Option<String>,
    pub valable_jusqu_a: Option<Jour>,
    pub a_demander: bool,
    #[serde(with = "socle::paris::ecrit::compacte::option")]
    pub dernier_recu: Option<socle::paris::Moment>,
}

/// Ou en est le taux d'un salarie au jour `le`, pour la paie du mois `mois` (`en_vigueur`). Ne refuse que si sa fiche est
/// illisible.
pub fn situation(ctx: &Contexte, cle: &IdSalarie, le: Option<Jour>, mois: Option<Mois>) -> R<Situation> {
    let le = le_jour(ctx, le);
    let recus = recus(ctx, cle)?;
    let e = en_vigueur(&recus, le, mois);
    let quoi = match e {
        None if sans_nir(ctx, cle) => SituationDuTaux::SansNir,
        None => SituationDuTaux::Inconnu,
        Some(e) if !e.r#type.is_some_and(TypeDeTauxPas::transmis) => {
            if sans_nir(ctx, cle) {
                SituationDuTaux::SansNir
            } else {
                SituationDuTaux::Bareme
            }
        }
        Some(e) => {
            if est_perime(e, le) {
                SituationDuTaux::Perime
            } else {
                SituationDuTaux::Valide
            }
        }
    };
    // Sans taux, ses cases ne disent rien.
    Ok(Situation {
        salarie: cle.clone(),
        situation: quoi,
        libelle: quoi.libelle(),
        taux: e.and_then(|e| e.taux),
        type_taux: e.and_then(|e| e.r#type),
        identifiant: e.and_then(|e| e.identifiant.clone()),
        valable_jusqu_a: e.and_then(fin_de_validite),
        a_demander: match quoi {
            SituationDuTaux::Perime | SituationDuTaux::Bareme | SituationDuTaux::Inconnu => true,
            SituationDuTaux::Valide | SituationDuTaux::SansNir => false,
        },
        dernier_recu: recus.last().and_then(|r| r.mis_a_disposition),
    })
}

fn archive(ctx: &Contexte, cle: &IdSalarie) -> bool {
    ctx.salaries.get(cle).is_some_and(|f| f.archivee())
}

/// Le meme tableau pour plusieurs salaries, dans l'ordre alphabetique. Sans liste : ceux des fiches, moins les archives.
pub fn diagnostic(ctx: &Contexte, cles: Option<&[IdSalarie]>, le: Option<Jour>, mois: Option<Mois>) -> R<Vec<Situation>> {
    let cles: BTreeSet<IdSalarie> = match cles {
        Some(c) => c.iter().cloned().collect(),
        None => ctx.salaries.keys().filter(|c| !archive(ctx, c)).cloned().collect(),
    };
    cles.iter().map(|c| situation(ctx, c, le, mois)).collect()
}

/// Les phrases a dire au depot de la DSN et a la production des bulletins (`depart::sans_taux_recu`), une par salarie
/// dont la paie prendrait la grille de l'article 204 H, III (CGI, art. 1671, 2 : « A défaut de taux transmis par
/// l'administration ») : ce que sa fiche sait des taux recus. Vide si tout va bien.
pub fn avertissements(ctx: &Contexte, cles: Option<&[IdSalarie]>, le: Option<Jour>, mois: Option<Mois>) -> R<Vec<String>> {
    let mut lignes = Vec::new();
    for s in diagnostic(ctx, cles, le, mois)? {
        let mut ligne = format!("{} : ", ctx.nomme(&s.salarie));
        ligne += &match s.situation {
            SituationDuTaux::Valide => continue,
            SituationDuTaux::Perime => {
                let taux = match s.taux {
                    Some(t) => format!(" ({} %)", t.to_string().replace('.', ",")),
                    None => String::new(),
                };
                match s.valable_jusqu_a {
                    Some(j) => format!("le dernier taux transmis par la DGFiP{} valait jusqu'au {}", taux, j.jj_mm_aaaa()),
                    None => format!("le dernier taux transmis par la DGFiP{} n'est plus valable", taux),
                }
            }
            SituationDuTaux::SansNir => "sans NIR dans sa fiche, la DGFiP ne peut transmettre aucun taux".into(),
            SituationDuTaux::Bareme | SituationDuTaux::Inconnu => "aucun taux transmis par la DGFiP".into(),
        };
        lignes.push(ligne);
    }
    Ok(lignes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un taux recu, ecrit en texte et lu par la porte du socle.
    fn lu(texte: &str) -> TauxPas {
        socle::forme::lire_texte(texte, "le taux").unwrap()
    }

    fn jour(an: i32, mois: u32, j: u32) -> Jour {
        Jour::nouveau(an, mois, j).unwrap()
    }

    fn recu(taux: &str, id: &str, le: &str, retour: Option<&str>) -> TauxPas {
        let retour = match retour {
            Some(m) => format!(r#", "en_retour_de": "{}""#, m),
            None => String::new(),
        };
        lu(&format!(r#"{{"taux": "{}", "type": "01", "identifiant": "{}", "mis_a_disposition": "{}"{}}}"#, taux, id, le, retour))
    }

    /// LA DATE D'EFFET D'UN TAUX TRANSMIS. Le debiteur « applique le taux calculé par l'administration fiscale, au plus
    /// tard le deuxième mois suivant sa transmission » (CGI, art. 1671, 2, `atelier/reference/sources/code-general-des-
    /// impots/articles-20261021.md`, l. 135) ; la retenue « est effectuée par le débiteur lors du paiement » (meme
    /// article, 1, l. 132) ; le taux est « applicable jusqu'à la fin du deuxième mois qui suit sa mise à disposition »
    /// (BOI-IR-PAS-30-10-20, § 70, `atelier/reference/sources/bofip/BOI-IR-PAS-30-10-20-extrait-taux.md`, l. 23-24). Un
    /// taux vaut donc pour tout paiement fait a partir du jour de sa mise a disposition, et pour aucun avant : mis a
    /// disposition le 13/03, il vaut pour le salaire paye le 14/03 ; celui du 15/04 ne vaut pas pour lui, il vaut pour
    /// celui paye le 15/04. Sans taux recu avant le paiement : aucun, la grille s'applique.
    #[test]
    fn un_taux_vaut_pour_tout_paiement_a_partir_de_sa_mise_a_disposition() {
        let taux = |id: &str, le: &str| recu("9.40", id, le, None);
        let recus = vec![taux("480747943", "20260313070000"), taux("985249073", "20260415100000")];
        let id = |le: Jour| en_vigueur(&recus, le, None).and_then(|e| e.identifiant.clone());
        assert_eq!(id(jour(2026, 3, 12)), None);
        assert_eq!(id(jour(2026, 3, 13)), Some("480747943".to_string()));
        assert_eq!(id(jour(2026, 3, 14)), Some("480747943".to_string()));
        assert_eq!(id(jour(2026, 4, 14)), Some("480747943".to_string()));
        assert_eq!(id(jour(2026, 4, 15)), Some("985249073".to_string()));
        assert_eq!(id(jour(2024, 12, 31)), None);
    }

    /// BOI-IR-PAS-30-10-20, § 70, exemple 1 (`atelier/reference/sources/bofip/BOI-IR-PAS-30-10-20-20181203.html`) :
    /// un taux mis a disposition le 10 fevrier « est valide jusqu'au 30 avril ».
    #[test]
    fn un_taux_vaut_jusqu_a_la_fin_du_deuxieme_mois_qui_suit_sa_mise_a_disposition() {
        let e = recu("5.00", "1", "20260210120000", None);
        assert_eq!(fin_de_validite(&e), Jour::nouveau(2026, 4, 30));
        assert!(!est_perime(&e, Jour::nouveau(2026, 4, 30).unwrap()));
        assert!(est_perime(&e, Jour::nouveau(2026, 5, 1).unwrap()));
    }

    /// LE TAUX EN RETOUR DE LA DSN D'UN MOIS VAUT POUR LES MOIS QUI SUIVENT (BOI-IR-PAS-30-10-20, § 20 et § 70, exemple 1 :
    /// le compte rendu de la DSN de janvier sert aux revenus de « février, mars et avril », `atelier/reference/sources/
    /// bofip/BOI-IR-PAS-30-10-20-extrait-taux.md`). La DSN de decembre, deposee le 21/12 pour un versement du 31/12 au
    /// taux du compte rendu du 05/10 (valide jusqu'au 31/12) : son compte rendu du 21/12 ne vaut pas pour la paie de
    /// decembre, il vaut pour celle de janvier, et s'ecrit dans la fiche a partir du 01/01. La reponse a une demande de
    /// taux (sans DSN mensuelle) vaut des sa mise a disposition.
    #[test]
    fn le_taux_en_retour_de_la_dsn_d_un_mois_vaut_pour_les_mois_qui_suivent() {
        let taux = |id: &str, le: &str, retour: Option<&str>| recu("2.30", id, le, retour);
        let recus = vec![taux("157352475", "20261005194311", Some("2026-09")), taux("713288315", "20261221184328", Some("2026-12"))];
        let id = |le: Jour, mois: Mois| en_vigueur(&recus, le, Some(mois)).and_then(|e| e.identifiant.clone());
        let decembre = Mois::nouveau(2026, 12).unwrap();
        assert_eq!(id(jour(2026, 12, 31), decembre), Some("157352475".to_string()));
        assert_eq!(id(jour(2027, 1, 5), decembre), Some("157352475".to_string()));
        assert_eq!(id(jour(2027, 1, 31), Mois::nouveau(2027, 1).unwrap()), Some("713288315".to_string()));
        let demande = vec![taux("480747943", "20261221184328", None)];
        assert!(en_vigueur(&demande, jour(2026, 12, 31), Some(decembre)).is_some());
        let mad = jour(2026, 12, 21);
        assert_eq!(date_d_effet(mad, Some(decembre)), jour(2027, 1, 1));
        assert_eq!(date_d_effet(mad, Some(decembre.precedent())), mad);
        assert_eq!(date_d_effet(mad, None), mad);
    }
}
