//! LE JOUR DU VERSEMENT DES SALAIRES : le jour ou part l'argent d'une paie. La compagnie le dit dans sa configuration
//! (fiche entreprise, datee comme elle), une fois pour ses permanents (`date_de_versement_permanents` : un contrat paye au
//! mois, `Nature::mensuelle`) et une fois pour ses intermittents (`date_de_versement_intermittents` : les autres, au
//! cachet ou a l'heure) ; un engagement paye un autre jour le dit
//! sur sa carte (`date_paiement`). C'est la « Date à laquelle le débiteur effectue le versement » (S21.G00.50.001,
//! `atelier/reference/norme-dsn/texte/bloc-S21.G00.50.txt`, l. 50-55), la « date de paiement » du bulletin (C. trav.,
//! art. R. 3243-1, 11°, `atelier/reference/sources/code-du-travail/articles-20261030.md`) et le jour ou se lit le taux de
//! prelevement a la source (BOI-IR-PAS-30-10-20, § 80, `atelier/reference/sources/bofip/BOI-IR-PAS-30-10-20-extrait-taux.md`).
//! Le seul lecteur : `engagement::Engagement::versement_dit`.
//!
//! Le choix de la compagnie, que seule elle connait (une case vide refuse en se nommant, `Versement::de_la_fiche`) :
//! - le dernier jour du mois ;
//! - le dernier jour du contrat : chaque paie a la fin de sa periode, le dernier jour du contrat dans le mois, ou la fin
//!   du mois pour un contrat qui continue ; un permanent, paye au mois, l'est donc a la fin du mois ;
//! - un jour fixe, dans le mois ou le mois suivant (« le 5 du mois suivant ») : la paie apres la periode d'emploi, la
//!   « pratique dite du décalage de paie » (BOI-IR-PAS-30-10-30-20, § 50,
//!   `atelier/reference/sources/bofip/BOI-IR-PAS-30-10-30-20-extrait-reversement.md`).
//!
//! Les intermittents ne sont pas sous le paiement mensuel (C. trav., art. L. 3242-1, al. 3,
//! `atelier/reference/sources/code-du-travail/xml/L3242-1.xml`) : aucun texte de la loi ne fixe leur jour de paie. Ce que
//! L. 3242-3 (deux paies par mois aux non-mensualises) dit d'eux, aucun texte depose ne le tranche : le moteur ne
//! l'applique pas. Un jour fixe dans le mois pour eux n'est pas refuse : la configuration demande seulement de le
//! confirmer (`a_confirmer_pour_les_intermittents`) ; la case « Payé le » d'une carte ne demande rien.
//!
//! Ce que les textes refusent (`refus`) :
//! - un versement apres le dernier jour du mois qui suit le mois principal declare (CCH-13 de S21.G00.50.001) ;
//! - en convention 1285, le salaire d'un artiste de cirque en contrat a duree determinee regle apres « la 1re semaine du
//!   mois suivant la fin dudit contrat » (CCNEAC, art. X.3.5.4, sous X.3.5 « Rémunération des artistes de cirque »,
//!   `atelier/reference/sources/kali-1285-x3-remuneration-artistes-20260929.md`, l. 324-333).
//! Rien d'autre : aucun texte n'ordonne le depot de la DSN et le versement, ni ne refuse un versement avant la fin du
//! contrat.
//!
//! INFERENCES, ecrites comme telles : un jour fixe que le mois n'a pas (« le 31 » en avril) est son dernier jour ; « la
//! 1re semaine du mois suivant » va du 1er au 7.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use socle::date::Jour;
use socle::mois::Mois;
use socle::echec::R;

use crate::fiche::{QuandDuVersement, VersementEcrit};
use crate::natures::Nature;

/// Le jour du versement que la compagnie a choisi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Versement {
    FinDuMois,
    FinDuContrat,
    /// Le quantieme, dans le mois de la paie ou le suivant.
    Fixe { quantieme: u32, mois_suivant: bool },
}

impl Versement {
    /// LE CHOIX QUE LA CASE ECRIT (`VersementEcrit`) : quand, et le numero du jour pour un jour du mois ou du mois
    /// suivant, de 1 a 31, que les deux autres choix n'ont pas. `Err` : ce qu'il faut corriger.
    fn de_l_ecrit(e: &VersementEcrit) -> Result<Versement, String> {
        let Some(quand) = e.quand else {
            return Err("Dire quand : choisir la fin du mois, la fin du contrat, un jour du mois ou \
                        un jour du mois suivant."
                .to_string());
        };
        let fixe = |mois_suivant: bool| match e.jour {
            Some(n) if (1..=31).contains(&n) => u32::try_from(n).map(|quantieme| Versement::Fixe { quantieme, mois_suivant }).map_err(|x| x.to_string()),
            Some(n) => Err(format!("Le {} n'est pas un jour du mois ; dire un numéro de 1 à 31.", n)),
            None => Err("Dire le numéro du jour, de 1 à 31.".to_string()),
        };
        match quand {
            QuandDuVersement::FinDuMois | QuandDuVersement::FinDuContrat if e.jour.is_some() => Err(
                "Un numéro de jour ne se dit qu'avec « un jour du mois » ou « un jour du mois suivant ».".to_string(),
            ),
            QuandDuVersement::FinDuMois => Ok(Versement::FinDuMois),
            QuandDuVersement::FinDuContrat => Ok(Versement::FinDuContrat),
            QuandDuVersement::JourDuMois => fixe(false),
            QuandDuVersement::JourDuMoisSuivant => fixe(true),
        }
    }

    /// Le choix tel que la case l'ecrit.
    fn ecrit(self) -> VersementEcrit {
        let (quand, jour) = match self {
            Versement::FinDuMois => (QuandDuVersement::FinDuMois, None),
            Versement::FinDuContrat => (QuandDuVersement::FinDuContrat, None),
            Versement::Fixe { quantieme, mois_suivant: false } => (QuandDuVersement::JourDuMois, Some(quantieme as usize)),
            Versement::Fixe { quantieme, mois_suivant: true } => (QuandDuVersement::JourDuMoisSuivant, Some(quantieme as usize)),
        };
        VersementEcrit { quand: Some(quand), jour }
    }

    /// La case de la fiche entreprise `ent` qui dit le jour d'un contrat de nature `nat`. Aucun texte ne fixe ce jour :
    /// seule la compagnie le connait, et le calcul refuse en nommant la case quand elle est vide.
    pub fn de_la_fiche(ent: &crate::fiche::ChampsEntreprise, nat: &Nature) -> R<Versement> {
        let (dit, cle, qui) = match nat.au_mois() {
            Some(au_mois) => (&ent.permanent.ouvrir(&au_mois).date_de_versement_permanents, "date_de_versement_permanents", "permanents"),
            None => (&ent.date_de_versement_intermittents, "date_de_versement_intermittents", "intermittents"),
        };
        match dit {
            Some(v) => Ok(*v),
            None => Err(socle::echec::cle(
                format!(
                    "La fiche entreprise ne dit pas le jour du versement des salaires des {qui} : le choisir dans la configuration, \
                     case « Jour du versement des {qui} »."
                ),
                cle,
            )),
        }
    }

    /// Ce que la configuration demande de confirmer avant d'ecrire ce jour pour les intermittents, quand il change : un
    /// jour fixe dans le mois paie avant leur fin les contrats qui finissent apres lui, ce qui n'est pas l'usage (un choix
    /// de la compagnie, que rien n'interdit : `None` pour les autres formes, et le 31, qui est la fin du mois).
    pub fn a_confirmer_pour_les_intermittents(self) -> Option<String> {
        match self {
            Versement::Fixe { quantieme, mois_suivant: false } if quantieme < 31 => Some(format!(
                "Payer le {} alors que des contrats peuvent finir jusqu'au 31 n'est pas l'usage. Continuer ?",
                if quantieme == 1 { "1er".to_string() } else { quantieme.to_string() }
            )),
            _ => None,
        }
    }

    /// Le jour du versement de la paie du mois `mois` dont la periode finit le `fin_periode`.
    pub fn jour(&self, mois: Mois, fin_periode: Jour) -> R<Jour> {
        match *self {
            Versement::FinDuMois => Ok(mois.dernier_jour()),
            Versement::FinDuContrat => Ok(fin_periode),
            Versement::Fixe { quantieme, mois_suivant } => {
                let m = if mois_suivant { mois.suivant() } else { mois };
                let jour = quantieme.min(m.jours());
                m.jour(jour).ok_or_else(|| socle::refus!("Le {} du mois n'existe pas.", jour))
            }
        }
    }
}

/// LA CASE DE LA FICHE ENTREPRISE (`date_de_versement_permanents`, `date_de_versement_intermittents`) : le choix, lu a
/// la porte dans sa forme ecrite (`VersementEcrit` : `{"quand": "jour_du_mois_suivant", "jour": 5}`) et ecrit de meme. Un
/// choix incomplet, ou un numero de jour avec la fin du mois ou du contrat, se refuse a la lecture ; la case vide est
/// nulle.
impl Serialize for Versement {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.ecrit().serialize(s)
    }
}

impl<'de> Deserialize<'de> for Versement {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Versement::de_l_ecrit(&VersementEcrit::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

/// Ses cases sont celles de sa forme ecrite : `quand`, un choix, et `jour`.
impl crate::fiche::FormeDeCase for Versement {
    fn cases() -> Vec<crate::fiche::Declaree> {
        <VersementEcrit as crate::fiche::FormeDeCase>::cases()
    }
}

/// Le dernier jour ou peut etre verse ce que declare la DSN du mois principal `mois` : le dernier jour du
/// mois suivant (CCH-13 de S21.G00.50.001, `atelier/reference/norme-dsn/texte/bloc-S21.G00.50.txt`, l. 61-62).
pub fn au_plus_tard_en_dsn(mois: Mois) -> Jour {
    mois.suivant().dernier_jour()
}

/// Le dernier jour ou se regle le salaire d'un artiste de cirque dont le contrat a duree determinee finit le `fin` :
/// « au plus tard, la 1re semaine du mois suivant la fin dudit contrat » (CCNEAC, art. X.3.5.4) ; le 7, par inference.
pub fn au_plus_tard_cirque_1285(fin: &Jour) -> R<Jour> {
    fin.mois_civil().suivant().premier_jour().apres(socle::date::Duree::jours(6))
}

/// Le refus d'un versement le `verse` pour `qui`, paye au titre du mois `mois` ; `fin_cirque` : la fin du contrat a duree
/// determinee d'un artiste de cirque en convention 1285, quand elle tombe dans ce mois. `None` : les textes l'admettent.
pub fn refus(qui: &str, verse: &Jour, mois: Mois, fin_cirque: Option<&Jour>) -> R<Option<String>> {
    let corriger = "Choisir un jour plus tôt : sur la carte (« Payé le »), ou dans la configuration (« Jour du versement \
                    des permanents » ou « des intermittents »).";
    let limite = au_plus_tard_en_dsn(mois);
    if *verse > limite {
        return Ok(Some(format!(
            "{} : versé le {}, après le {}, dernier jour où la DSN de ce mois peut déclarer un versement (norme DSN, \
             S21.G00.50.001, CCH-13). {}",
            qui,
            verse.jj_mm_aaaa(),
            limite.jj_mm_aaaa(),
            corriger
        )));
    }
    if let Some(fin) = fin_cirque {
        let limite = au_plus_tard_cirque_1285(fin)?;
        if *verse > limite {
            return Ok(Some(format!(
                "{} : versé le {}, après le {}. Le salaire d'un artiste de cirque en CDD se règle à la fin du contrat \
                 ou au plus tard la première semaine du mois qui suit sa fin (CCNEAC, art. X.3.5.4). {}",
                qui,
                verse.jj_mm_aaaa(),
                limite.jj_mm_aaaa(),
                corriger
            )));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le jour que le test ecrit (`AAAA-MM-JJ`), lu par sa forme.
    fn j(ecrit: &str) -> Jour {
        socle::forme::lire_texte(&format!("\"{}\"", ecrit), "le jour du test").unwrap()
    }

    /// CCH-13 de S21.G00.50.001 : au plus tard le dernier jour du mois qui suit le mois principal declare ; le 5 du mois
    /// suivant (le decalage de paie, BOI-IR-PAS-30-10-30-20, § 50) passe.
    #[test]
    fn un_versement_apres_le_mois_qui_suit_se_refuse() {
        let decembre: Mois = socle::forme::lire_texte("\"2026-12\"", "le mois du test").unwrap();
        let cinq = Versement::Fixe { quantieme: 5, mois_suivant: true }.jour(decembre, j("2026-12-31")).unwrap();
        assert_eq!(cinq, j("2027-01-05"));
        assert_eq!(refus("X", &cinq, decembre, None).unwrap(), None);
        assert_eq!(refus("X", &j("2027-01-31"), decembre, None).unwrap(), None);
        assert!(refus("X", &j("2027-02-01"), decembre, None).unwrap().is_some_and(|m| m.contains("CCH-13")));
    }

    /// CCNEAC, art. X.3.5.4 : le salaire d'un artiste de cirque en CDD se regle au plus tard la premiere semaine du mois
    /// qui suit la fin du contrat.
    #[test]
    fn un_artiste_de_cirque_de_la_1285_est_paye_la_premiere_semaine_qui_suit_son_contrat() {
        let fin = j("2026-10-28");
        let octobre = fin.mois_civil();
        assert_eq!(refus("X", &j("2026-11-07"), octobre, Some(&fin)).unwrap(), None);
        assert!(refus("X", &j("2026-11-10"), octobre, Some(&fin)).unwrap().is_some_and(|m| m.contains("X.3.5.4")));
        // Hors du cirque de la 1285, le 10 du mois suivant passe.
        assert_eq!(refus("X", &j("2026-11-10"), octobre, None).unwrap(), None);
    }
}
