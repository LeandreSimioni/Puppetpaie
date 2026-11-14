//! Le prelevement a la source des paies d'un mois : celui de la paie declaree, sinon celui que l'engagement a ecrit
//! dans la paie (`engagement::etats`). Rien ne s'y calcule.

use engagement::calcul::Paie;
use droit::contexte::Contexte;
use droit::pas::Versement;
use socle::echec::R;

/// Le prelevement de la paie declaree (`engagement::etats::paie_declaree`), ou `None` : aucune DSN partie n'a declare
/// le mois de sa paie pour son engagement (une paie ajoutee apres le depot, que la correction du mois suivant
/// declarera), ou la paie n'a pas de numero de contrat.
///
/// LE PAS D'UN MOIS DECLARE SE LIT, IL NE SE RECALCULE PAS (regles 1 et 4). CGI art. 1671, 1 et 2
/// (`atelier/reference/sources/code-general-des-impots/articles-20261013.md`, l. 99 et 102) : la retenue « est effectuée
/// par le débiteur lors du paiement », au taux que l'administration a transmis et que l'employeur applique « au plus
/// tard le deuxième mois suivant sa transmission ». Ce qui a ete retenu l'a ete au paiement, au taux de ce jour-la : le
/// prelevement de la paie que la DSN a declaree (`engagement::etats::paie_declaree`), pas celui d'un recalcul au taux du
/// jour. Le mois declare est celui de la fin de la periode de la paie, comme le declare `dsn::bloc_versement` (50.001).
fn deja_declare(ctx: &Contexte, pa: &Paie, lus: &mut Lus, departs: &engagement::departs::Departs) -> R<Option<Versement>> {
    let e = &pa.engagement;
    let Some(numero) = e.numero.clone() else { return Ok(None) };
    let fin = e.fin_periode();
    let mois = fin.mois_civil();
    if !lus.objets.contains_key(&mois) {
        lus.objets.insert(mois, engagement::stock::objets_du_mois(&ctx.racine, mois)?);
    }
    let objet = lus.objets[&mois].iter().find(|o| o.carte().salarie.as_ref() == Some(&pa.salarie.id) && o.carte().numero.as_ref() == Some(&numero));
    let Some(o) = objet else { return Ok(None) };
    Ok(engagement::etats::paie_declaree(ctx, o, departs, mois)?.and_then(|p| p.pas.clone()))
}

/// Ce que `deja_declare` a lu une fois pour toutes les paies : les engagements de chaque mois.
#[derive(Default)]
struct Lus {
    objets: std::collections::HashMap<socle::mois::Mois, Vec<engagement::stock::Objet>>,
}

/// Le prelevement de chaque paie, par POSITION dans `paies` (`None` : un mois sans paie, sans bulletin). Une paie deja
/// declaree garde le prelevement de la paie declaree (`deja_declare`) ; une autre, celui que l'engagement a ecrit ou
/// calcule pour elle (`Paie::pas`). Aucun ne se calcule ici : une paie qui n'en porte pas se refuse.
pub fn des_paies(ctx: &Contexte, paies: &[Paie], departs: &engagement::departs::Departs) -> R<Vec<Option<Versement>>> {
    let mut lus = Lus::default();
    paies.iter().map(|pa| de_la_paie(ctx, pa, &mut lus, departs)).collect()
}

fn de_la_paie(ctx: &Contexte, pa: &Paie, lus: &mut Lus, departs: &engagement::departs::Departs) -> R<Option<Versement>> {
    if pa.engagement.sans_paie() {
        return Ok(None);
    }
    if let Some(v) = deja_declare(ctx, pa, lus, departs)? {
        return Ok(Some(v));
    }
    let v = pa.pas.clone().ok_or_else(|| {
        socle::echec::cle(
            format!("{} : la paie de {} ne porte pas de prelevement a la source. Revalider l'engagement.", pa.salarie.id.to_string(), pa.engagement.mois),
            "pas",
        )
    })?;
    Ok(Some(v))
}

/// LE PRELEVEMENT D'UNE PAIE DU MOIS : celui d'une paie validee (`des_paies`), ou celui que les pieces d'une paie
/// importee disent (`engagement::importe::PasImporte`), qui ne disent ni l'abattement ni le montant soumis.
#[derive(Debug, Clone)]
pub enum Prelevement {
    Calcule(Versement),
    Importe(engagement::importe::PasImporte),
}

impl Prelevement {
    pub fn prelevement(&self) -> socle::num::Decimal {
        match self {
            Prelevement::Calcule(v) => v.prelevement,
            Prelevement::Importe(p) => p.prelevement,
        }
    }

    pub fn abattement(&self) -> Option<socle::num::Decimal> {
        match self {
            Prelevement::Calcule(v) => Some(v.abattement),
            Prelevement::Importe(_) => None,
        }
    }

    pub fn montant_soumis(&self) -> Option<socle::num::Decimal> {
        match self {
            Prelevement::Calcule(v) => Some(v.montant_soumis),
            Prelevement::Importe(_) => None,
        }
    }

    pub fn type_taux(&self) -> Option<droit::type_taux_pas::TypeDeTauxPas> {
        match self {
            Prelevement::Calcule(v) => v.type_taux,
            Prelevement::Importe(p) => p.type_taux,
        }
    }

    /// Le taux, son echelle gardee ; `None` : les pieces d'une paie importee ne le disent pas.
    pub fn taux(&self) -> R<Option<socle::num::Decimal>> {
        match self {
            Prelevement::Calcule(v) => v.taux.dec().map(Some),
            Prelevement::Importe(p) => Ok(p.taux),
        }
    }
}

/// Le prelevement de chaque paie du mois, par POSITION dans `paies` (`None` : un mois sans paie, ou une paie importee
/// dont les pieces ne disent pas de prelevement) : celui d'une paie validee (`des_paies`), celui que les pieces d'une
/// paie importee disent.
pub fn du_mois(ctx: &Contexte, paies: &[engagement::etats::PaieDuMois], departs: &engagement::departs::Departs) -> R<Vec<Option<Prelevement>>> {
    let mut lus = Lus::default();
    paies
        .iter()
        .map(|pa| match pa {
            engagement::etats::PaieDuMois::Validee(p) => Ok(de_la_paie(ctx, p, &mut lus, departs)?.map(Prelevement::Calcule)),
            engagement::etats::PaieDuMois::Importee(p) => Ok(p.pas.clone().map(Prelevement::Importe)),
        })
        .collect()
}
