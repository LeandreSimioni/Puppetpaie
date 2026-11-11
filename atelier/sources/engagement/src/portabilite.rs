//! LA PORTABILITE DES GARANTIES D'UN CONTRAT QUI FINIT (code de la securite sociale, L. 911-8,
//! `atelier/reference/sources/code-de-la-securite-sociale/xml/L911-8.xml`) : ce que le certificat de travail en signale
//! (6°), calcule ici depuis l'engagement et ses paies, sans recalcul.
//!
//! - Elle n'est pas due apres une fin « consecutive a une faute lourde » (premier alinea) : les motifs de fin 088 et 096
//!   (S21.G00.62.002, `contrats-base.json`). Le droit a l'assurance chomage, l'autre condition, se decide a France
//!   Travail et l'ancien salarie en justifie a l'organisme (5°) : il ne se devine pas du motif (une demission peut etre
//!   legitime : reglement general de l'assurance chomage, art. 2, § 2, `atelier/reference/sources/chomage-permanents/`).
//! - Les garanties maintenues sont « celles en vigueur dans l'entreprise » (3°) : une garantie collective a laquelle le
//!   salarie est affilie, c'est-a-dire qu'une paie de son engagement y cotise (la prevoyance, `prevoyance…` ; la sante,
//!   `mutuelle`), la sante sauf une dispense a la fin du contrat (une dispense renonce a la portabilite :
//!   `kali-3090-accord-sante-permanents-20240719-20261011.md`, l. 209). Un mois suspendu sans cotisation ne l'en retire
//!   pas.
//! - Sa duree maximale (1°) : « dans la limite de la duree du dernier contrat de travail ou, le cas echeant, des derniers
//!   contrats de travail lorsqu'ils sont consecutifs chez le meme employeur. Cette duree est appreciee en mois, le cas
//!   echeant arrondie au nombre superieur, sans pouvoir exceder douze mois. » La duree de l'indemnisation, l'autre limite,
//!   le moteur ne la connait pas.

use droit::fiche::Lue as _;
use socle::date::Jour;
use socle::echec::R;

use crate::calcul::Paie;
use crate::stock::Objet;
use crate::DateDeLigne;

/// Le plafond du maintien, en mois (L. 911-8, 1°).
const DOUZE_MOIS: u32 = 12;

/// Ce que le certificat signale.
#[derive(Debug, Clone, PartialEq)]
pub struct Portabilite {
    pub prevoyance: bool,
    pub sante: bool,
    /// Le debut du dernier contrat, ou du premier des derniers contrats consecutifs.
    pub depuis: Jour,
    /// D'autres contrats le precedent sans interruption : `depuis` est le debut du premier d'entre eux.
    pub consecutifs: bool,
    /// La duree maximale du maintien, en mois.
    pub mois: u32,
}

/// LA PORTABILITE DU CONTRAT QUE LA PAIE `p` TERMINE (son engagement, de numero `numero`, finit le `fin`) ; `None` : rien
/// n'est maintenu (une faute lourde, ou aucune garantie collective).
pub fn de(racine: &std::path::Path, p: &Paie, numero: &str, fin: &Jour) -> R<Option<Portabilite>> {
    // Les motifs de fin pour faute lourde (S21.G00.62.002).
    use droit::natures::MotifDeRupture::{LicenciementFauteLourde, RuptureAnticipeeFauteLourde};
    if matches!(p.engagement.motif_rupture, Some(LicenciementFauteLourde | RuptureAnticipeeFauteLourde)) {
        return Ok(None);
    }
    let objets = crate::stock::objets(racine)?;
    let siens: Vec<&Objet> = objets.iter().filter(|o| !o.est_non_actif() && o.carte().salarie.as_ref() == Some(&p.salarie.id)).collect();
    // Les garanties : ce que la paie qui finit cotise, et chaque paie de l'engagement avant elle.
    let mut ids: Vec<String> = p.lignes.iter().map(|l| l.id.clone()).collect();
    for o in siens.iter().filter(|o| o.carte().numero.as_deref() == Some(numero)) {
        for paie in crate::paies::presentes(o)?.into_values() {
            ids.extend(paie.paie.lignes.into_iter().map(|l| l.id));
        }
    }
    let prevoyance = ids.iter().any(|i| i == "prevoyance" || i.starts_with("prevoyance_"));
    // La dispense de sante est une case d'un salarie au mois : seul un contrat paye au mois la lit.
    let dispense = p.engagement.nat()?.au_mois().is_some_and(|au| p.salarie.permanent.ouvrir(&au).sante_dispense.remplie().is_some());
    let sante = ids.iter().any(|i| i == "mutuelle") && !dispense;
    if !prevoyance && !sante {
        return Ok(None);
    }
    let depuis = debut_des_contrats_consecutifs(&siens, p.engagement.debut)?;
    Ok(Some(Portabilite { prevoyance, sante, depuis, consecutifs: depuis < p.engagement.debut, mois: mois_arrondis(&depuis, fin)?.min(DOUZE_MOIS) }))
}

/// LES DERNIERS CONTRATS CONSECUTIFS CHEZ LE MEME EMPLOYEUR : en remontant depuis le contrat qui finit (`debut`), chaque
/// contrat du salarie qui finit au plus tard la veille de ce debut, sans jour entre les deux (ou qui le chevauche),
/// s'y ajoute. INFERENCE : L. 911-8 ne definit pas « consecutifs » ; on le lit au mot, des contrats qui se suivent
/// sans interruption. Un contrat dont la fin ne se lit pas (ni dite, ni jour joue) n'y entre pas.
fn debut_des_contrats_consecutifs(siens: &[&Objet], debut: Jour) -> R<Jour> {
    let mut bornes = Vec::new();
    for o in siens {
        let c = o.carte();
        let Some(d) = c.debut_lu() else { continue };
        let Some(f) = c.fin.or_else(|| c.dates().iter().map(DateDeLigne::jour).max()) else { continue };
        bornes.push((d, f));
    }
    let mut depuis = debut;
    loop {
        let veille = depuis.plus_jours(-1)?;
        match bornes.iter().filter(|(d, f)| *d < depuis && *f >= veille).map(|(d, _)| *d).min() {
            Some(d) => depuis = d,
            None => return Ok(depuis),
        }
    }
}

/// La duree de `debut` a `fin` (compris) en mois de calendrier entiers comptes depuis `debut`, un mois entame compte
/// entier (« arrondie au nombre superieur »), au plus `DOUZE_MOIS`. Le `n`-ieme mois finit la veille du meme quantieme
/// `n` mois plus tard, ou le dernier jour de ce mois-la quand il n'a pas ce quantieme (du 31/01, le premier mois finit
/// le 28/02).
fn mois_arrondis(debut: &Jour, fin: &Jour) -> R<u32> {
    let mut n = 0;
    while n < DOUZE_MOIS {
        n += 1;
        let meme_quantieme = debut.plus_mois(i64::from(n))?;
        let fin_du_mois = if meme_quantieme.jour == debut.jour { meme_quantieme.plus_jours(-1)? } else { meme_quantieme };
        if fin_du_mois >= *fin {
            break;
        }
    }
    Ok(n)
}
