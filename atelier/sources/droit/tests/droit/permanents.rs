//! Les regles des permanents (`droit::permanents` : conges payes, fin de contrat, preavis et indemnites legales,
//! maintien de salaire) contre les references `tests/donnees/permanents/<fonction>.json` : une ligne par cas, l'entree et
//! ce que le droit en rend. `PUPPETPAIE_REECRIRE_ORACLES=1` les reecrit.

use crate::outils::typee::{cas_de, jouer, Rendu};
use droit::permanents as p;
use serde::{Deserialize, Serialize};
use socle::date::{Anciennete, Duree, Jour};
use socle::grandeurs::{Coefficient, Jours, Montant};

/// Une anciennete de la reference, en mois, ecrite en texte (`23.5`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct A(#[serde(serialize_with = "Anciennete::ecrite_en_mois")] Anciennete);

/// Un montant de la reference, ecrit en texte (`2000.00`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct M(#[serde(with = "socle::grandeurs::ecrit::texte")] Montant);

/// Un coefficient de la reference (des mois de salaire), ecrit en texte (`0.5`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct C(#[serde(with = "socle::grandeurs::ecrit::texte")] Coefficient);

/// Des jours de la reference, ecrits en texte (`2.5`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct J(#[serde(with = "socle::grandeurs::ecrit::texte")] Jours);

/// Ce que `indemnite_conges` rend : les deux calculs et la regle retenue.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IndemniteDesJours {
    dixieme: M,
    maintien: M,
    retenu: M,
    regle: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SalaireDeReference {
    moyenne_12_mois: M,
    tiers_3_mois: M,
    retenu: M,
    regle: String,
}

/// Une indemnite de la reference ; ses annees d'anciennete ecrites en texte (`8.333333333333333333333333333`).
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IndemniteLegale {
    montant: M,
    du: bool,
    pourquoi: Option<String>,
    annees: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlancherDeLaRupture {
    plancher: Option<M>,
    annees: String,
    pourquoi: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct IndemniteDuDepart {
    montant: M,
    du: bool,
    mois: C,
    #[serde(default, serialize_with = "Anciennete::ecrite_en_annees_entieres_opt", deserialize_with = "Anciennete::en_annees_entieres_opt")]
    palier: Option<Anciennete>,
    pourquoi: Option<String>,
}

/// Une tranche de la loi (`droit::maintien::Tranche`, qui dure des jours), dans la forme de sa reference
/// (`duree_du_maintien.json`).
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Tranche {
    #[serde(serialize_with = "Duree::ecrite_en_jours", deserialize_with = "Duree::en_jours")]
    jours: Duree<socle::date::EnJours>,
    #[serde(with = "socle::grandeurs::ecrit::texte")]
    taux: socle::grandeurs::Taux,
}

/// Une duree en jours, ecrite en son nombre dans la reference.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
struct EnJours(#[serde(serialize_with = "Duree::ecrite_en_jours", deserialize_with = "Duree::en_jours")] Duree<socle::date::EnJours>);

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fin {
    cle: String,
    rubrique: String,
}

fn legale(i: p::IndemniteLegale) -> IndemniteLegale {
    IndemniteLegale { montant: M(i.montant), du: i.du, pourquoi: i.pourquoi, annees: i.annees.map(Anciennete::annees_ecrites) }
}

// Les entrees de chaque fonction : ses arguments, dans l'ordre.
type Nature = droit::natures::NatureDeContrat;
type Motif = Option<String>;

/// Le motif de rupture d'un cas (S21.G00.62.002) : un code hors de la liste de la norme est une donnee invalide, que le
/// test refuse en la nommant.
fn motif(m: &Motif) -> Option<droit::natures::MotifDeRupture> {
    m.as_deref().map(|c| droit::natures::MotifDeRupture::du_code(c).unwrap_or_else(|| panic!("le motif {:?} de l'oracle n'est pas un code de S21.G00.62.002 : donnee invalide", c)))
}

fn arrondir_jours(a: &(J,)) -> J {
    J(a.0 .0.au_demi_superieur())
}
fn plafonner(a: &(J,)) -> Rendu<J> {
    p::plafonner(a.0 .0).map(J).into()
}
fn periode_de_reference(a: &(Jour,)) -> Rendu<(Jour, Jour)> {
    p::periode_de_reference(&a.0).map(|r| (r.debut(), r.fin())).into()
}
fn jours_acquis(a: &(Jour, Jour)) -> Rendu<J> {
    socle::date::Periode::du_au(a.0, a.1).and_then(|presence| p::jours_acquis(&presence)).map(J).into()
}
fn indemnite_conges(a: &(J, M, J, M)) -> Rendu<IndemniteDesJours> {
    p::indemnite_conges(a.0 .0, a.1 .0, a.2 .0, a.3 .0)
        .map(|i| IndemniteDesJours { dixieme: M(i.dixieme), maintien: M(i.maintien), retenu: M(i.retenu), regle: i.regle.to_string() })
        .into()
}

#[test]
fn les_conges() {
    jouer("permanents/arrondir_jours.json", cas_de("permanents/arrondir_jours.json"), arrondir_jours);
    jouer("permanents/plafonner.json", cas_de("permanents/plafonner.json"), plafonner);
    jouer("permanents/periode_de_reference.json", cas_de("permanents/periode_de_reference.json"), periode_de_reference);
    jouer("permanents/jours_acquis.json", cas_de("permanents/jours_acquis.json"), jours_acquis);
    jouer("permanents/indemnite_conges.json", cas_de("permanents/indemnite_conges.json"), indemnite_conges);
}

fn precarite_due(a: &(Nature, Motif)) -> Rendu<Option<String>> {
    p::precarite_due(&a.0.proprietes().unwrap(), motif(&a.1)).into()
}
fn indemnite_precarite(a: &(M,)) -> Rendu<M> {
    p::indemnite_precarite(a.0 .0).map(M).into()
}
fn anciennete_mois(a: &(Jour, Jour)) -> A {
    A(Anciennete::entre(&a.0, &a.1))
}
fn preavis_mois(a: &(A, Motif)) -> Rendu<Option<String>> {
    p::preavis_mois(a.0 .0, motif(&a.1), false).map(|m| m.map(socle::grandeurs::ecrit::texte::dit)).into()
}
fn salaire_de_reference(a: &(Vec<M>, Vec<M>)) -> SalaireDeReference {
    let montants = |l: &[M]| l.iter().map(|m| m.0).collect::<Vec<Montant>>();
    let s = p::salaire_de_reference(&montants(&a.0), &montants(&a.1)).unwrap();
    SalaireDeReference { moyenne_12_mois: M(s.moyenne_12_mois), tiers_3_mois: M(s.tiers_3_mois), retenu: M(s.retenu), regle: s.regle.to_string() }
}
fn indemnite_licenciement(a: &(A, M, Motif)) -> Rendu<IndemniteLegale> {
    p::indemnite_licenciement(a.0 .0, &mut || Ok(a.1 .0), motif(&a.2)).map(legale).into()
}
fn indemnite_mise_a_la_retraite(a: &(A, M)) -> Rendu<IndemniteLegale> {
    p::indemnite_mise_a_la_retraite(a.0 .0, &mut || Ok(a.1 .0)).map(legale).into()
}
fn indemnite_rupture_conventionnelle(a: &(A, M)) -> Rendu<PlancherDeLaRupture> {
    p::indemnite_rupture_conventionnelle(a.0 .0, &mut || Ok(a.1 .0)).map(|i| PlancherDeLaRupture { plancher: i.plancher.map(M), annees: i.annees.annees_ecrites(), pourquoi: i.pourquoi }).into()
}
fn indemnite_depart_retraite(a: &(A, M)) -> Rendu<IndemniteDuDepart> {
    p::indemnite_depart_retraite(a.0 .0, &mut || Ok(a.1 .0)).map(|i| IndemniteDuDepart { montant: M(i.montant), du: i.du, mois: C(i.mois), palier: i.palier.map(Anciennete::ans_revolus), pourquoi: i.pourquoi }).into()
}
fn rupture(a: &(Motif,)) -> Rendu<Option<Fin>> {
    p::rupture(motif(&a.0)).map(|o| o.map(|(fin, rubrique)| Fin { cle: fin.cle().to_string(), rubrique: rubrique.into() })).into()
}
fn code_52_permis(a: &(Option<droit::type_de_prime::TypeDePrime>, Motif)) -> Rendu<Option<String>> {
    p::code_52_permis(a.0, motif(&a.1)).into()
}

#[test]
fn la_fin_du_contrat() {
    jouer("permanents/precarite_due.json", cas_de("permanents/precarite_due.json"), precarite_due);
    jouer("permanents/indemnite_precarite.json", cas_de("permanents/indemnite_precarite.json"), indemnite_precarite);
    jouer("permanents/anciennete_mois.json", cas_de("permanents/anciennete_mois.json"), anciennete_mois);
    jouer("permanents/preavis_mois.json", cas_de("permanents/preavis_mois.json"), preavis_mois);
    jouer("permanents/salaire_de_reference.json", cas_de("permanents/salaire_de_reference.json"), salaire_de_reference);
    jouer("permanents/indemnite_licenciement.json", cas_de("permanents/indemnite_licenciement.json"), indemnite_licenciement);
    jouer("permanents/indemnite_mise_a_la_retraite.json", cas_de("permanents/indemnite_mise_a_la_retraite.json"), indemnite_mise_a_la_retraite);
    jouer("permanents/indemnite_rupture_conventionnelle.json", cas_de("permanents/indemnite_rupture_conventionnelle.json"), indemnite_rupture_conventionnelle);
    jouer("permanents/indemnite_depart_retraite.json", cas_de("permanents/indemnite_depart_retraite.json"), indemnite_depart_retraite);
    jouer("permanents/rupture.json", cas_de("permanents/rupture.json"), rupture);
    jouer("permanents/code_52_permis.json", cas_de("permanents/code_52_permis.json"), code_52_permis);
}

fn droit_au_maintien(a: &(Option<Nature>, A)) -> Rendu<Option<String>> {
    let n = a.0.map(|n| n.proprietes().unwrap());
    p::droit_au_maintien(n.as_deref(), a.1 .0).into()
}
fn jours_de_carence(a: &(bool, bool)) -> Rendu<EnJours> {
    p::jours_de_carence(a.0, a.1).map(EnJours).into()
}
fn duree_du_maintien(a: &(A,)) -> Rendu<Vec<Tranche>> {
    let en_jours = |t: droit::maintien::Tranche| match t.duree {
        droit::maintien::DureeDUneTranche::Jours(jours) => Tranche { jours, taux: t.taux },
        autre => panic!("une tranche de la loi dure des jours, pas {:?}", autre),
    };
    p::duree_du_maintien(a.0 .0).map(|t| t.into_iter().map(en_jours).collect()).into()
}

#[test]
fn le_maintien() {
    jouer("permanents/droit_au_maintien.json", cas_de("permanents/droit_au_maintien.json"), droit_au_maintien);
    jouer("permanents/jours_de_carence.json", cas_de("permanents/jours_de_carence.json"), jours_de_carence);
    jouer("permanents/duree_du_maintien.json", cas_de("permanents/duree_du_maintien.json"), duree_du_maintien);
}

/// Ce que `permanents-base.json` dit d'une indemnite (son code du bloc S21.G00.52, ce qu'elle supporte), sa rubrique
/// (`rubriques-base.json`) le porte a l'identique : la ligne que le solde ecrit se declare et se cotise comme le texte le dit.
#[test]
fn chaque_indemnite_a_le_code_et_le_regime_de_sa_rubrique() {
    let rg = p::regles().unwrap();
    let r = |code: &str| droit::catalogue::rubrique(std::path::Path::new("/nulle-part"), &droit::catalogue::CodeDeRubrique::lire(code).unwrap(), &Jour::nouveau(2026, 10, 1).unwrap()).unwrap();
    let codes = [
        (rg.conges_payes.indemnite_compensatrice.rubrique.to_string(), &rg.conges_payes.indemnite_compensatrice.dsn_bloc52),
        (rg.indemnite_precarite.rubrique.to_string(), &rg.indemnite_precarite.dsn_bloc52),
        (rg.preavis.delai_de_prevenance.rubrique.to_string(), &rg.preavis.delai_de_prevenance.dsn_bloc52),
        (rg.indemnite_licenciement.rubrique.to_string(), &rg.indemnite_licenciement.dsn_bloc52),
        (rg.rupture_conventionnelle.rubrique.to_string(), &rg.rupture_conventionnelle.dsn_bloc52),
        (rg.mise_a_la_retraite.rubrique.to_string(), &rg.mise_a_la_retraite.dsn_bloc52),
        (format!("{}_conventionnelle", rg.mise_a_la_retraite.rubrique), &rg.mise_a_la_retraite.dsn_bloc52_conventionnelle),
        (rg.depart_a_la_retraite.rubrique.to_string(), &rg.depart_a_la_retraite.dsn_bloc52),
        (format!("{}_conventionnelle", rg.depart_a_la_retraite.rubrique), &rg.depart_a_la_retraite.dsn_bloc52_conventionnelle),
    ];
    for (rubrique, code) in codes {
        assert_eq!(r(&rubrique).dsn_bloc52, Some(code.code), "{}", rubrique);
    }
    for (cle, regime) in &rg.regime_des_indemnites_de_rupture.par_indemnite {
        let lue = r(regime.rubrique.as_str());
        assert_eq!((lue.imposable, lue.soumis), (regime.imposable, regime.soumis), "{} ({})", cle, regime.rubrique);
    }
}
