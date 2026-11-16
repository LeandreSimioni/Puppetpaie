//! LE MAINTIEN DE SALAIRE D'UN ARRET DE TRAVAIL : ce que la loi et la convention garantissent, lu une fois, sous une seule
//! forme (`MaintienDeSalaire`). Le module dit le droit ; il ne compte pas les jours d'un arret ni ne fait la paie
//! (`engagement::maintien`).
//!
//! - La loi : L1226-1 (une annee d'anciennete ; pas pour les intermittents), D1226-1 (90 % puis deux tiers de la
//!   remuneration brute), D1226-2 (dix jours de plus par cinq ans au-dela de la premiere annee, quatre-vingt-dix au plus),
//!   D1226-3 (sept jours de carence, sauf accident du travail et maladie professionnelle), D1226-4 (les douze mois
//!   anterieurs), D1226-5 (les indemnites journalieres se deduisent), D1226-8 (l'anciennete au premier jour de l'absence) :
//!   `atelier/reference/sources/code-du-travail/articles-20261015.md` ; les nombres dans `permanents-base.json`.
//! - La convention : `clauses-conventionnelles-base.json`, section `maintien_de_salaire` de chaque IDCC, chaque article lu
//!   dans le texte de la DILA (`atelier/reference/sources/kali-entrepot/`, `moteur-atelier releve-conventions article`). Une
//!   tranche dure des jours (`jours`), des mois de calendrier (`mois` : la 1285, X.4.6) ou tant que la caisse verse ses
//!   indemnites (`pendant_ij` : la 1285, X.4.7). `base` dit si le pourcentage porte sur le salaire brut ou net ;
//!   `plafond_net`, que « les sommes de toutes provenances » ne depassent pas le net qu'il aurait percu ; `subrogation`
//!   (`tout_salarie`), que la convention la donne a tout salarie qui a droit au maintien.

use crate::natures::Nature;
use crate::clauses;
use socle::date::{Anciennete, Duree, EnJours, EnMois};
use socle::echec::R;
use socle::grandeurs::{Coefficient, Taux};

/// Ce qui a cause l'arret, quand c'est une maladie ou un accident : le champ `origine` d'un motif S21.G00.60.001 de
/// `arrets-base.json` (`crate::arrets`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Origine {
    Maladie,
    AccidentDuTravail,
    MaladieProfessionnelle,
    Trajet,
}

impl Origine {
    /// L'origine d'un motif d'arret (`arrets-base.json`), ou `None` pour un motif qui n'appelle pas de maintien (ou
    /// que la norme ne connait pas).
    pub fn du_motif(code: crate::arrets::MotifDArret) -> R<Option<Origine>> {
        Ok(crate::arrets::motif_d_arret(code)?.and_then(|m| m.origine))
    }

    /// L'accident du travail et la maladie professionnelle : pas de carence legale (D1226-3), et l'indemnite journaliere
    /// de la caisse court des le lendemain de l'arret (CSS L433-1), au lieu du quatrieme jour (R323-1).
    pub fn professionnelle(self) -> bool {
        matches!(self, Origine::AccidentDuTravail | Origine::MaladieProfessionnelle)
    }
}

/// Combien dure une tranche : une duree de droit, en jours ou en mois (des mois de calendrier comptent de date a date
/// depuis le premier jour qu'elle indemnise), ou tant que la caisse verse ses indemnites journalieres, jusqu'a la fin de
/// l'arret.
#[derive(Debug, Clone, PartialEq)]
pub enum DureeDUneTranche {
    Jours(Duree<EnJours>),
    Mois(Duree<EnMois>),
    PendantLesIndemnites,
}

/// Une tranche : sa duree et son pourcentage (66,66 se lit deux tiers : `fraction`).
#[derive(Debug, Clone, PartialEq)]
pub struct Tranche {
    pub duree: DureeDUneTranche,
    pub taux: Taux,
}

/// Sur quoi porte le pourcentage garanti : `brut` ou `net` dans `clauses-conventionnelles-base.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Base {
    Brut,
    Net,
}

/// LE MAINTIEN DE SALAIRE : ce qu'un texte garantit pour un arret, a l'anciennete du salarie.
#[derive(Debug, Clone, PartialEq)]
pub struct MaintienDeSalaire {
    /// Le texte, pour les phrases : « la loi (L1226-1) », « la convention 1285 (art. X.4.6) ».
    pub texte: String,
    pub base: Base,
    pub carence: Duree<EnJours>,
    pub tranches: Vec<Tranche>,
    /// « un montant total superieur a la remuneration nette qu'il aurait effectivement percue » est exclu.
    pub plafond_net: bool,
    /// La subrogation que la convention donne ; la loi n'en donne pas (`None`).
    pub subrogation: Option<clauses::Subrogation>,
}

/// Le droit d'un salarie a un maintien, ou pourquoi il n'en a pas.
#[derive(Debug, Clone, PartialEq)]
pub enum Droit {
    Du(MaintienDeSalaire),
    PasDu(String),
}

/// Le pourcentage en fraction : deux tiers exacts pour 66,66 (D1226-1, 2°, « deux tiers ») et 66,66 de la 3090 et de la
/// 2642 (« 2/3 », « 66,66 % » : le meme deux tiers ecrit au centieme) ; le taux sur cent sinon.
pub fn fraction(taux: Taux) -> Coefficient {
    if taux == DEUX_TIERS_AU_CENTIEME {
        Coefficient::fraction(2, TROIS)
    } else {
        taux.en_coefficient()
    }
}

/// Les deux tiers que D1226-1, 2°, et les conventions ecrivent au centieme (« 66,66 % »).
const DEUX_TIERS_AU_CENTIEME: Taux = Taux::centiemes_de_point(6666);
/// Le denominateur des deux tiers.
const TROIS: std::num::NonZeroU32 = std::num::NonZeroU32::new(3).expect("trois n'est pas nul");
/// Le premier pourcentage de la loi (D1226-1, 1° : « 90 % »), celui des conventions qui ne disent que leurs jours a 90 %.
const QUATRE_VINGT_DIX: Taux = Taux::points(90);

// ------------------------------------------------------------------ la loi

/// Ce que la loi garantit (L1226-1, D1226-1 a D1226-3, D1226-8) a un salarie de cette nature et de cette anciennete au
/// premier jour de l'absence.
pub fn legal(nature: &Nature, anciennete: Anciennete, origine: Origine) -> R<Droit> {
    if let Some(refus) = crate::permanents::droit_au_maintien(Some(nature), anciennete)? {
        return Ok(Droit::PasDu(refus));
    }
    let carence = crate::permanents::jours_de_carence(origine.professionnelle(), origine == Origine::Trajet)?;
    let tranches = crate::permanents::duree_du_maintien(anciennete)?;
    Ok(Droit::Du(MaintienDeSalaire {
        texte: "la loi (L1226-1)".into(),
        base: Base::Brut,
        carence,
        tranches,
        plafond_net: false,
        subrogation: None,
    }))
}

// ------------------------------------------------------------------ la convention

#[track_caller]
fn manque(ou: &str, cle: &str) -> socle::echec::Echec {
    socle::echec::livree_dite(&clauses::FICHIER, cle, ou)
}

fn tranches(l: &[clauses::TrancheDeMaintien], ou: &str) -> R<Vec<Tranche>> {
    l.iter()
        .map(|t| {
            let duree = match (t.pendant_ij, t.mois, t.jours) {
                (Some(true), _, _) => DureeDUneTranche::PendantLesIndemnites,
                (_, Some(m), _) => DureeDUneTranche::Mois(m),
                (_, None, Some(j)) => DureeDUneTranche::Jours(j),
                (_, None, None) => return Err(manque(ou, "jours")),
            };
            Ok(Tranche { duree, taux: t.taux })
        })
        .collect()
}

/// Ce que la convention `idcc` garantit pour un arret de cette origine a cette anciennete (au premier jour de l'absence) ;
/// `None` quand elle ne prevoit rien (la loi seule s'applique). La clause de l'accident du travail vaut quand elle couvre
/// l'origine (`couvre`, les origines que l'article nomme), sauf passe
/// `comme_maladie_apres_mois` d'anciennete ; celle de la maladie sinon (une maladie, un accident « non professionnel »).
pub fn conventionnel(idcc: Option<&crate::fiche::Idcc>, anciennete: Anciennete, origine: Origine) -> R<Option<Droit>> {
    let Some(sec) = clauses::releve(idcc)?.and_then(|c| c.maintien_de_salaire) else { return Ok(None) };
    let accident = sec.accident_du_travail.filter(|at| {
        at.couvre.contains(&origine) &&at.comme_maladie_apres_mois.is_none_or(|apres| anciennete < apres)
    });
    let nom = clauses::nom(idcc);
    // (article, minimum d'anciennete, base, carence, plafond net, subrogation, tranches)
    let (article, minimum, base, carence, plafond_net, subrogation, liste) = match (accident, sec.maladie) {
        (Some(at), _) => {
            let ou = format!("{}, art. {}", nom, at.article);
            let mut tr = at.tranches.as_slice();
            if let Some(deux_ans) = at.tranches_deux_ans.as_deref().filter(|t| !t.is_empty() && anciennete >= Anciennete::ans(2)) {
                tr = deux_ans;
            }
            if let Some(apres) = at.tranches_apres.as_ref().filter(|a| anciennete > a.anciennete_mois) {
                tr = apres.tranches.as_slice();
            }
            let liste = tranches(tr, &ou)?;
            let minimum = at.anciennete_mois_minimum;
            (at.article, minimum, at.base, at.carence_jours, at.plafond_net == Some(true), at.subrogation, Ok(liste))
        }
        (None, Some(m)) => {
            let ou = format!("{}, art. {}", nom, m.article);
            let mut lignes: Vec<&clauses::LigneDeMaintien> = m.par_anciennete.iter().collect();
            lignes.sort_by_key(|l| l.anciennete_annees_min);
            let liste = match lignes.into_iter().filter(|l| anciennete >= l.anciennete_annees_min).last() {
                None => Err(format!("{} (art. {}) : aucune tranche a cette anciennete.", nom, m.article)),
                Some(l) => match l.tranches.as_deref() {
                    Some(t) if !t.is_empty() => Ok(tranches(t, &ou)?),
                    _ => Ok(vec![
                        Tranche { duree: DureeDUneTranche::Jours(l.jours_90.ok_or_else(|| manque(&ou, "jours_90"))?), taux: QUATRE_VINGT_DIX },
                        Tranche { duree: DureeDUneTranche::Jours(l.jours_66.ok_or_else(|| manque(&ou, "jours_66"))?), taux: DEUX_TIERS_AU_CENTIEME },
                    ]),
                },
            };
            let minimum = m.anciennete_mois_minimum;
            (m.article, minimum, m.base, m.carence_jours, m.plafond_net == Some(true), m.subrogation, liste)
        }
        (None, None) => return Ok(None),
    };
    if anciennete < minimum {
        return Ok(Some(Droit::PasDu(format!(
            "{} (art. {}) demande {} mois d'anciennete ; il y en a {}.",
            nom,
            article,
            socle::texte::nombre_dit(minimum.mois()),
            socle::texte::nombre_dit(anciennete.mois().round_dp(1))
        ))));
    }
    let tranches = match liste {
        Ok(t) => t,
        Err(pas_du) => return Ok(Some(Droit::PasDu(pas_du))),
    };
    Ok(Some(Droit::Du(MaintienDeSalaire {
        texte: format!("{} (art. {})", nom, article),
        base,
        carence,
        tranches,
        plafond_net,
        subrogation,
    })))
}

// ------------------------------------------------------------------ la phrase

/// Le regime en une phrase : « la convention 1285 (art. X.4.6) : 3 mois à 100 % puis 3 mois à 75 % du salaire brut,
/// sans carence ».
pub fn phrase(r: &MaintienDeSalaire) -> String {
    let tr: Vec<String> = r
        .tranches
        .iter()
        .map(|t| {
            let pct = socle::texte::virgule(t.taux);
            match t.duree {
                DureeDUneTranche::Jours(d) => format!("{} à {} %", d, pct),
                DureeDUneTranche::Mois(d) => format!("{} à {} %", d, pct),
                DureeDUneTranche::PendantLesIndemnites => format!("{} % tant que la caisse indemnise", pct),
            }
        })
        .collect();
    format!(
        "{} : {} du salaire {}, {}",
        r.texte,
        tr.join(" puis "),
        if r.base == Base::Net { "net" } else { "brut" },
        if r.carence.nulle() { "sans carence".to_string() } else { format!("après {} de carence", r.carence) }
    )
}
