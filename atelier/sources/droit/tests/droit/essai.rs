//! La duree maximale de la periode d'essai (`droit::essai`). Textes : code du travail L. 1242-10 (« un jour par semaine,
//! dans la limite de deux semaines lorsque la durée initialement prévue au contrat est au plus égale à six mois et d'un
//! mois dans les autres cas », `atelier/reference/sources/code-du-travail/xml/L1242-10.xml`), L. 1221-19 (deux, trois,
//! quatre mois, `xml/L1221-19.xml`) ; convention 3090, art. 7.4 (« artistes interprètes : 2 mois de travail effectif »,
//! `atelier/reference/sources/kali-3090-titres-vii-x-xi-annexes-20260924.md`).

use droit::essai::{plafond, refuser_au_dela};
use droit::fiche::{StatutConventionnel as S, IDCC_3090};
use droit::natures::NatureDeContrat as N;
use socle::date::{Duree, Jour, PeriodeOuverte};

#[test]
fn la_periode_d_essai_ne_depasse_pas_ce_que_le_texte_permet() {
    let (cddu, cdd, cdi) = (N::CdduSpectacle.proprietes().unwrap(), N::Cdd.proprietes().unwrap(), N::Cdi.proprietes().unwrap());
    let j = crate::outils::le_jour;
    let du = |debut: Jour, fin: Option<Jour>| PeriodeOuverte::du(debut, fin).unwrap();
    let jours = |p: Option<droit::essai::Plafond>| p.map(|p| p.jours);
    let d = Duree::jours;

    // Un CDD de six mois au plus : un jour par semaine, deux semaines au plus.
    let jules = plafond(&cdd, &du(j("2026-01-12"), Some(j("2026-06-30"))), Some(S::AgentDeMaitrise), false, Some(&IDCC_3090)).unwrap();
    assert_eq!(jours(jules.clone()), Some(d(14)));
    let refus = refuser_au_dela(d(30), jules.as_ref(), "VARENNE Jules").unwrap_err().phrase;
    assert!(refus.contains("L. 1242-10") && refus.contains("14 jours"), "{}", refus);
    assert!(refuser_au_dela(d(14), jules.as_ref(), "VARENNE Jules").is_ok());
    // Trois semaines de contrat : trois jours ; un contrat d'usage de trois jours : un jour.
    assert_eq!(jours(plafond(&cdd, &du(j("2026-03-02"), Some(j("2026-03-22"))), None, false, None).unwrap()), Some(d(3)));
    assert_eq!(jours(plafond(&cddu, &du(j("2026-11-20"), Some(j("2026-11-22"))), None, true, Some(&IDCC_3090)).unwrap()), Some(d(1)));
    // Plus de six mois : un jour par semaine, un mois au plus.
    assert_eq!(jours(plafond(&cdd, &du(j("2026-01-12"), Some(j("2026-07-20"))), None, false, None).unwrap()), Some(d(28)));
    assert_eq!(jours(plafond(&cdd, &du(j("2026-01-12"), Some(j("2026-12-31"))), None, false, None).unwrap()), Some(d(31)));
    // Un CDD sans terme : sa duree minimale n'est pas dite, rien ne se juge.
    assert_eq!(plafond(&cdd, &du(j("2026-01-12"), None), None, false, None).unwrap(), None);

    // Un CDI : deux mois pour un employe, trois pour un agent de maitrise, quatre pour un cadre ; un artiste de la 3090,
    // deux mois ; sans classement, rien.
    let cdi_du = du(j("2026-01-12"), None);
    assert_eq!(jours(plafond(&cdi, &cdi_du, Some(S::Employe06), false, None).unwrap()), Some(d(59)));
    assert_eq!(jours(plafond(&cdi, &cdi_du, Some(S::AgentDeMaitrise), false, None).unwrap()), Some(d(90)));
    assert_eq!(jours(plafond(&cdi, &cdi_du, Some(S::Cadre04), false, None).unwrap()), Some(d(120)));
    assert_eq!(jours(plafond(&cdi, &cdi_du, Some(S::Cadre04), true, Some(&IDCC_3090)).unwrap()), Some(d(59)));
    assert_eq!(plafond(&cdi, &cdi_du, None, false, None).unwrap(), None);
    let cadre = plafond(&cdi, &cdi_du, Some(S::Cadre04), false, None).unwrap();
    assert!(refuser_au_dela(d(121), cadre.as_ref(), "X").unwrap_err().phrase.contains("L. 1221-19"));
}
