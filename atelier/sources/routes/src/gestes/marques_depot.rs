//! Les arrets que la DSN d'un mois reporte (`arrets_pour`). Rien d'ici ne fait partir quoi que ce soit.

use droit::contexte::Contexte;
use socle::echec::{Classe, R};

/// Les arrets que la DSN de `mois` doit reporter : ceux du magasin des imprevus que la regle du bloc 60 y met
/// (`declarations::dsn::Arret::a_reporter` : les arrets qui croisent le mois, et ceux d'un mois anterieur saisis apres le
/// depart de sa DSN), lus en leur forme (`engagement::imprevus::ArretSaisi`).
pub(crate) fn arrets_pour(ctx: &Contexte, mois: &str, departs: &engagement::departs::Departs) -> R<Vec<declarations::dsn::Arret>> {
    let le_mois = socle::mois::Mois::lire(mois).map_err(|m| m.reclasser(Classe::Illisible))?;
    let mut out = Vec::new();
    for imp in crate::metier::imprevus::magasin(engagement::imprevus::tous(&ctx.racine))? {
        let engagement::imprevus::Imprevu::Arret(saisi) = imp else { continue };
        let arret = declarations::dsn::Arret { saisi };
        if arret.a_reporter(le_mois, departs).map_err(|m| m.reclasser(Classe::Illisible))? {
            out.push(arret);
        }
    }
    Ok(out)
}
