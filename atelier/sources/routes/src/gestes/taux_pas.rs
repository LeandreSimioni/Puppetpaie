//! L'ECRITURE DES TAUX DE PAS RECUS (`droit::pas::TauxRecu`, chacun avec la cle de la fiche de son salarie) : chacun
//! s'ecrit dans la fiche de son salarie (`routes::fiches::poser_taux_recu`), et les engagements qu'il touche se
//! recalculent. `droit::pas` les lit.

use droit::contexte::{Contexte, IdSalarie};
use droit::pas::TauxRecu;
use socle::echec::Echec;

/// Ecrit des taux recus ; rend les changements lisibles (`CLE : ...`). Chacun s'ecrit a sa date d'effet
/// (`droit::pas::date_d_effet`). Un taux deja recu (meme identifiant, meme taux, meme instant) ne s'ecrit pas deux fois.
pub fn enregistrer(ctx: &Contexte, releves: &[(IdSalarie, TauxRecu)], source: &str) -> Result<Vec<String>, Echec> {
    let mut changements = Vec::new();
    let mut ecrits: Vec<(&IdSalarie, &TauxRecu)> = Vec::new();
    for (cle, r) in releves {
        let mad = r.mise_a_disposition();
        let depuis = droit::pas::date_d_effet(mad, r.en_retour_de);
        let deja = droit::pas::recus(ctx, cle)?.iter().chain(ecrits.iter().filter(|(c, _)| *c == cle).map(|(_, v)| *v)).any(|v| v == r);
        if deja {
            continue;
        }
        crate::fiches::poser_taux_recu(ctx, cle, r.clone(), depuis, source)?;
        fn dit<T: std::fmt::Display>(o: &Option<T>) -> String {
            match o {
                Some(t) => t.to_string(),
                None => "non dit".to_string(),
            }
        }
        changements.push(format!("{} : taux {}, type {}, identifiant {}, mis a disposition le {}", cle, r.taux, dit(&r.r#type), r.identifiant, mad.jj_mm_aaaa()));
        ecrits.push((cle, r));
    }
    if !changements.is_empty() {
        refaire_les_engagements(ctx)?;
    }
    Ok(changements)
}

/// UN TAUX RECU REFAIT, DANS LE MEME GESTE, LES ENGAGEMENTS QU'IL TOUCHE : chaque mois saisi se relit
/// (`engagement::etats::du_mois`) dans la compagnie telle qu'elle est maintenant, et ses engagements valides dont
/// l'empreinte ne tient plus (le taux de leur salarie au jour de leur paiement, `droit::pas::taux_dgfip`) se reecrivent.
/// Un mois dont les documents sont partis ne bouge pas.
fn refaire_les_engagements(ctx: &Contexte) -> Result<(), Echec> {
    let ctx = engagement::stock::contexte(&ctx.racine, Some(ctx.date))?;
    let departs = envois::journal::lu(&ctx.racine)?.departs;
    for mois in engagement::stock::mois_saisis(&ctx.racine)? {
        engagement::etats::du_mois(&ctx, mois, &departs)?;
    }
    Ok(())
}
