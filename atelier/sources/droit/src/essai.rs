//! LA DUREE MAXIMALE DE LA PERIODE D'ESSAI d'un contrat, et le refus de ce qui la depasse. La periode d'essai se dit en
//! jours (`periode_essai_jours`, S21.G00.40.082 « Nombre de jours de période d'essai »), des jours calendaires
//! (convention 3090, art. 7.4 et 7.5 : « Toute période d'essai exprimée en jours se décompte en jours calendaires »).
//!
//! - Un contrat a duree determinee, contrat d'usage compris (L. 1242-2, 3°) : « un jour par semaine, dans la limite de
//!   deux semaines lorsque la durée initialement prévue au contrat est au plus égale à six mois et d'un mois dans les
//!   autres cas » (C. trav., art. L. 1242-10, `atelier/reference/sources/code-du-travail/xml/L1242-10.xml`). Sans terme
//!   precis, elle se compte sur la duree minimale du contrat, que la saisie ne dit pas : rien ne se juge alors.
//! - Un contrat a duree indeterminee : deux mois pour les ouvriers et les employes, trois pour les agents de maitrise et
//!   les techniciens, quatre pour les cadres (L. 1221-19, `xml/L1221-19.xml`) ; durees imperatives, sauf plus courtes
//!   par un accord collectif posterieur au 25/06/2008 ou par le contrat (L. 1221-22, `xml/L1221-22.xml`). La categorie
//!   se lit dans le classement conventionnel du salarie (S21.G00.40.002 : 03 cadre dirigeant et 04 cadre, 05 profession
//!   intermediaire, 06 employe, 07 ouvrier ; `atelier/reference/norme-dsn/texte/bloc-S21.G00.40.txt`) ; 01 et 02
//!   (l'exploitant ou l'artisan salarie de son entreprise) ne sont dans aucune des trois : rien ne se juge.
//!   La convention 3090 (art. 7.4, du 2020-03-01 : `atelier/reference/sources/kali-3090-titres-vii-x-xi-annexes-20260924.md`)
//!   fixe pour les artistes interpretes « 2 mois de travail effectif », et pour les autres categories les durees de la
//!   loi.
//!
//! INFERENCES, ecrites comme telles : une semaine commencee compte pour un jour (le plafond le plus large que la phrase
//! permet ; un controle faux refuserait du licite) ; « six mois », « un mois », « n mois » se comptent du premier jour du
//! contrat au meme quantieme (le dernier jour du mois s'il ne l'a pas), ce jour exclu.

use socle::date::{Duree, Jour};
use socle::echec::R;

use crate::natures::Nature;

/// La duree maximale de l'essai d'un contrat, en jours, et la phrase qui dit d'ou elle vient.
#[derive(Debug, Clone, PartialEq)]
pub struct Plafond {
    pub jours: Duree,
    pub pourquoi: String,
}

/// « dans la limite de deux semaines » (L. 1242-10).
const DEUX_SEMAINES: Duree = Duree::jours(14);
/// « au plus égale à six mois », « d'un mois dans les autres cas » (L. 1242-10).
const SIX_MOIS: Duree = Duree::mois(6);
const UN_MOIS: Duree = Duree::mois(1);
/// « 2 mois de travail effectif » (3090, art. 7.4).
const ESSAI_D_UN_ARTISTE_3090: Duree = Duree::mois(2);

/// LE PLAFOND DE L'ESSAI d'un contrat de nature `nat`, du `debut` a la `fin` (aucune pour un CDI) : `classement` est le
/// classement conventionnel du salarie (S21.G00.40.002), `artiste` dit s'il est artiste du spectacle, `idcc` la
/// convention de la compagnie. `None` quand le texte ne permet pas de le dire (un CDD sans terme, un CDI d'une
/// categorie que la loi ne nomme pas, ou sans classement).
pub fn plafond(nat: &Nature, debut: &Jour, fin: Option<&Jour>, classement: Option<crate::fiche::StatutConventionnel>, artiste: bool, idcc: Option<&crate::fiche::Idcc>) -> R<Option<Plafond>> {
    if !nat.duree_indeterminee {
        let Some(fin) = fin else { return Ok(None) };
        let par_semaine = Duree::du_au(debut, fin).un_jour_par_semaine_commencee()?;
        let six_mois = *fin < debut.apres(SIX_MOIS)?;
        let (limite, borne) = if six_mois { (DEUX_SEMAINES, "deux semaines au plus, le contrat durant six mois au plus".to_string()) } else {
            (UN_MOIS.en_jours_depuis(debut)?, "un mois au plus, le contrat durant plus de six mois".to_string())
        };
        // Deux durees en jours.
        let jours = if par_semaine <= limite { par_semaine } else { limite };
        return Ok(Some(Plafond {
            jours,
            pourquoi: format!(
                "un jour par semaine de contrat commencée du {} au {} ({}), {} : {} (code du travail, art. L. 1242-10)",
                debut.jj_mm_aaaa(),
                fin.jj_mm_aaaa(),
                par_semaine,
                borne,
                jours
            ),
        }));
    }
    if artiste && idcc == Some(&crate::fiche::IDCC_3090) {
        let jours = ESSAI_D_UN_ARTISTE_3090.en_jours_depuis(debut)?;
        return Ok(Some(Plafond {
            jours,
            pourquoi: format!(
                "deux mois pour un artiste interprète (convention collective IDCC 3090, art. 7.4), soit {} à partir du {}",
                jours,
                debut.jj_mm_aaaa()
            ),
        }));
    }
    use crate::fiche::StatutConventionnel::{AgentDeMaitrise, Cadre03, Cadre04, Employe06, Employe07};
    let (mois, qui, alinea) = match classement {
        Some(Cadre03 | Cadre04) => (Duree::mois(4), "un cadre", "3°"),
        Some(AgentDeMaitrise) => (Duree::mois(3), "un agent de maîtrise ou un technicien", "2°"),
        Some(Employe06 | Employe07) => (Duree::mois(2), "un ouvrier ou un employé", "1°"),
        None => return Ok(None),
    };
    let jours = mois.en_jours_depuis(debut)?;
    Ok(Some(Plafond {
        jours,
        pourquoi: format!(
            "{} pour {} (code du travail, art. L. 1221-19, {}), soit {} à partir du {}",
            mois,
            qui,
            alinea,
            jours,
            debut.jj_mm_aaaa()
        ),
    }))
}

/// LE REFUS D'UN ESSAI TROP LONG : `jours` stipules au-dela du `plafond` du contrat. `qui` nomme le contrat dans la phrase.
pub fn refuser_au_dela(jours: Duree, plafond: Option<&Plafond>, qui: &str) -> R<()> {
    match plafond {
        Some(p) if jours > p.jours => Err(socle::refus!(
            "{} : une période d'essai de {} dépasse la durée maximale de ce contrat — {}. La ramener à {} au plus, dans \
             « Plus d'options › Contrat » de l'engagement.",
            qui,
            jours,
            p.pourquoi,
            p.jours
        )),
        _ => Ok(()),
    }
}
