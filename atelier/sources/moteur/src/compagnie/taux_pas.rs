//! Les taux de prelevement a la source que la DGFiP a fait connaitre (un compte rendu de nature 94, `suites`) ou que les
//! DSN deposees declarent (l'import) : chacun s'ecrit dans la fiche de son salarie (`taux_pas`,
//! `routes::fiches::poser_taux_recu`), et les engagements qu'il touche se recalculent. `droit::pas` les lit.

use droit::fiche::Lue as _;

use droit::contexte::Contexte;
use socle::date::Jour;
use socle::echec::Echec;

pub(crate) fn ko(m: String) -> Echec {
    Echec { classe: None, message: m }
}

/// UN TAUX RECU POUR UN SALARIE : la cle de sa fiche, et le taux tel que sa fiche l'ecrit (`droit::fiche::TauxPas`) --
/// le taux, son type, son identifiant, sa mise a disposition (`AAAAMMJJHHMMSS`) et, pour le compte rendu qui repond a une
/// DSN mensuelle, le mois qu'elle declare (`en_retour_de`).
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct TauxRecu {
    #[serde(skip)]
    pub(crate) salarie: String,
    pub(crate) taux: Option<String>,
    #[serde(rename = "type")]
    pub(crate) type_taux: Option<String>,
    pub(crate) identifiant: String,
    pub(crate) mis_a_disposition: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) en_retour_de: Option<String>,
}

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

/// `AAAAMMJJ000000` du jour `j`.
fn instant_du_jour(j: Jour) -> String {
    format!("{}000000", j.aaaammjj())
}

/// LE DERNIER TAUX TRANSMIS PAR LA DGFiP QUE LES DSN DEJA DEPOSEES DE `dossier` DECLARENT, par salarie (un releve tel que
/// `enregistrer` le prend). Une DSN qui declare un taux de type 01 avec son identifiant (S21.G00.50.006 a .008,
/// `norme-dsn/texte/bloc-S21.G00.50.txt` : « Identifiant du taux porté par le CRM PAS […] transmis par la DGFIP ») dit
/// que la DGFiP a transmis ce taux a l'employeur, qui l'applique (CGI, art. 1671, 2,
/// `atelier/reference/sources/code-general-des-impots/articles-20261021.md`). Sa validite court de sa mise a disposition
/// (`droit::pas::VALIDITE_MOIS`), que la DSN ne porte pas. Ce que les textes en disent : la retenue se fait « lors du
/// paiement » (CGI, art. 1671, 1) avec un taux « toujours valide à la date de son application » (BOI-IR-PAS-30-10-20,
/// § 80, `atelier/reference/sources/bofip/BOI-IR-PAS-30-10-20-20181203.html`) : le taux etait donc mis a disposition au
/// plus tard le jour du versement (S21.G00.50.001) et au plus tot deux mois pleins avant ; le compte rendu « est
/// transmis le mois au cours duquel la DSN […] a été déposée » (meme page, § 30), mais la DSN qui applique le
/// taux n'en porte pas la date. INFERENCE (aucun texte ne date un taux lu dans une DSN) : le releve est date du jour du versement
/// (`mis_a_disposition`), la mise a disposition la plus tardive que les textes permettent ; le compte rendu de la DSN suivante
/// le remplace. Chaque DSN se lit par `declarations::dsn_lue::lire_une_dsn` ; une DSN fautive refuse.
pub(crate) fn derniers_taux_des_dsn(ctx: &Contexte, dossier: &std::path::Path) -> Result<Vec<TauxRecu>, Echec> {
    let par_nir: std::collections::HashMap<String, String> = ctx
        .salaries
        .iter()
        .filter_map(|(cle, s)| s.nir.dite().filter(|n| !n.is_empty()).map(|n| (n.to_string(), cle.to_string())))
        .collect();
    let fichiers = declarations::dsn_lue::fichiers(dossier)?;
    // salarie -> mois -> releve (dans l'ordre d'insertion des salaries, mois tries).
    let mut trouve: Vec<(String, std::collections::BTreeMap<socle::mois::Mois, TauxRecu>)> = Vec::new();
    for f in fichiers {
        let Some(d) = declarations::dsn_lue::lire_une_dsn(&f)? else { continue };
        let Some(mois) = d.mois else { continue };
        for v in &d.versements {
            // « -1 » : un CDD court sans taux personnalise (`norme-dsn/texte/bloc-S21.G00.50.txt`, S21.G00.50.008) ;
            // aucun taux n'a ete transmis. Sans la date du versement (S21.G00.50.001), le taux ne se date pas.
            let (Some(id), Some(j), Some(cle)) = (v.identifiant_pas.as_ref().filter(|i| *i != "-1"), v.verse_le, par_nir.get(&v.nir)) else {
                continue;
            };
            let rel = TauxRecu {
                salarie: cle.clone(),
                taux: v.taux_pas.map(|t| t.to_string()),
                type_taux: v.type_pas.clone(),
                identifiant: id.to_string(),
                mis_a_disposition: instant_du_jour(j),
                en_retour_de: None,
            };
            match trouve.iter_mut().find(|(k, _)| k == cle) {
                Some((_, mm)) => {
                    mm.insert(mois, rel);
                }
                None => trouve.push((cle.clone(), [(mois, rel)].into_iter().collect())),
            }
        }
    }
    Ok(trouve.into_iter().filter_map(|(_, mm)| mm.into_values().next_back()).collect())
}
