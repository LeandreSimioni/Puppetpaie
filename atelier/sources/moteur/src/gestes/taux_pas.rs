//! L'ECRITURE DES TAUX DE PAS RECUS (`compagnie::taux_pas::TauxRecu`) : chacun s'ecrit dans la fiche de son salarie
//! (`routes::fiches::poser_taux_recu`), et les engagements qu'il touche se recalculent. `droit::pas` les lit.

use droit::contexte::Contexte;
use socle::echec::Echec;
use compagnie::taux_pas::{ko, TauxRecu};

/// Ecrit des taux recus ; rend les changements lisibles (`CLE : ...`). Chacun s'ecrit a sa date d'effet
/// (`droit::pas::date_d_effet`). Un taux deja recu (meme identifiant, meme taux, meme instant) ne s'ecrit pas deux fois.
pub(crate) fn enregistrer(ctx: &Contexte, releves: &[TauxRecu], source: &str) -> Result<Vec<String>, Echec> {
    let mut changements = Vec::new();
    let mut ecrits: Vec<(String, droit::fiche::TauxPas)> = Vec::new();
    for r in releves {
        let cle = r.salarie.clone();
        let lu: droit::fiche::TauxPas = droit::fiche::lire_champs(r, "Le taux de PAS reçu").map_err(ko)?;
        let Some(mad) = droit::pas::mise_a_disposition(&lu) else {
            return Err(ko(format!("{} : un taux de PAS recu sans sa date de mise a disposition ne s'ecrit pas ({}).", cle, r.mis_a_disposition)));
        };
        let depuis = droit::pas::date_d_effet(mad, r.en_retour_de.as_deref()).map_err(ko)?;
        let deja = droit::pas::recus(ctx, &cle).map_err(ko)?.into_iter().chain(ecrits.iter().filter(|(c, _)| *c == cle).map(|(_, v)| v.clone())).any(|v| v == lu);
        if deja {
            continue;
        }
        crate::routes::fiches::poser_taux_recu(ctx, &cle, lu.clone(), depuis, source)?;
        let dit = |o: &Option<String>| match o {
            Some(t) => t.clone(),
            None => "non dit".to_string(),
        };
        changements.push(format!("{} : taux {}, type {}, identifiant {}, mis a disposition le {}", cle, dit(&r.taux), dit(&r.type_taux), r.identifiant, mad.jj_mm_aaaa()));
        ecrits.push((cle, lu));
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
    let ctx = Contexte::nouveau(&ctx.racine, Some(ctx.date)).map_err(ko)?;
    let departs = envois::journal::lu(&ctx.racine).map_err(ko)?.departs;
    for mois in engagement::stock::mois_saisis(&ctx.racine).map_err(ko)?.into_iter().map(String::from) {
        engagement::etats::du_mois(&ctx, &mois, &departs).map_err(ko)?;
    }
    Ok(())
}
