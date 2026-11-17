//! Les taux de prelevement a la source que les DSN deposees declarent (l'import), tels qu'ils se recoivent
//! (`droit::pas::TauxRecu`, avec la cle de la fiche de son salarie) ; leur ecriture dans la fiche de chaque salarie est
//! `gestes::taux_pas::enregistrer`.

use droit::fiche::Lue as _;

use droit::contexte::{Contexte, IdSalarie};
use droit::pas::TauxRecu;
use socle::date::Jour;

/// CE QUE LA DERNIERE DSN DEPOSEE D'UN SALARIE DIT DE SON TAUX DE PAS : un taux transmis par la DGFiP, ou un taux qu'elle
/// n'ecrit pas comme la norme (S21.G00.50.006), qui n'en est pas un : il ne pose rien, et se dit (le jour du versement
/// qui l'applique). Un taux transmis se recoit, pour le salarie de la cle.
#[derive(Debug, Clone)]
pub enum TauxDUneDsn {
    Transmis(IdSalarie, TauxRecu),
    Illisible { salarie: IdSalarie, verse_le: Jour },
}

impl TauxDUneDsn {
    /// Les taux transmis de `l`, a recevoir (`routes::gestes::taux_pas::enregistrer`).
    pub fn transmis(l: &[TauxDUneDsn]) -> Vec<(IdSalarie, TauxRecu)> {
        l.iter()
            .filter_map(|t| match t {
                TauxDUneDsn::Transmis(salarie, recu) => Some((salarie.clone(), recu.clone())),
                TauxDUneDsn::Illisible { .. } => None,
            })
            .collect()
    }
}

/// Le debut du jour `j` (`AAAAMMJJ000000`).
fn instant_du_jour(j: Jour) -> socle::echec::R<socle::paris::Moment> {
    socle::paris::Moment::a_paris(j, socle::paris::HeureDuJour::MINUIT).ok_or_else(|| socle::refus!("Le jour {} sort du calendrier.", j.jj_mm_aaaa()))
}

/// LE DERNIER TAUX TRANSMIS PAR LA DGFiP QUE LES DSN DEJA DEPOSEES DE `dossier` DECLARENT, par salarie (un releve tel que
/// `enregistrer` le prend). Une DSN qui declare un taux de type 01 avec son identifiant (S21.G00.50.006 a .008,
/// `norme-dsn/texte/bloc-S21.G00.50.txt` : « Identifiant du taux porté par le CRM PAS […] transmis par la DGFIP ») dit
/// que la DGFiP a transmis ce taux a l'employeur, qui l'applique (CGI, art. 1671, 2,
/// `atelier/reference/sources/code-general-des-impots/articles-20261021.md`). Sa validite court de sa mise a disposition
/// (`droit::pas::VALIDITE`), que la DSN ne porte pas. Ce que les textes en disent : la retenue se fait « lors du
/// paiement » (CGI, art. 1671, 1) avec un taux « toujours valide à la date de son application » (BOI-IR-PAS-30-10-20,
/// § 80, `atelier/reference/sources/bofip/BOI-IR-PAS-30-10-20-20181203.html`) : le taux etait donc mis a disposition au
/// plus tard le jour du versement (S21.G00.50.001) et au plus tot deux mois pleins avant ; le compte rendu « est
/// transmis le mois au cours duquel la DSN […] a été déposée » (meme page, § 30), mais la DSN qui applique le
/// taux n'en porte pas la date. INFERENCE (aucun texte ne date un taux lu dans une DSN) : le releve est date du jour du versement
/// (`mis_a_disposition`), la mise a disposition la plus tardive que les textes permettent ; le compte rendu de la DSN suivante
/// le remplace. Chaque DSN se lit par `declarations::dsn_lue::lire_une_dsn` ; une DSN fautive refuse ; un taux qui ne s'y
/// lit pas n'est pas un taux (`TauxDUneDsn::Illisible`).
pub fn derniers_taux_des_dsn(ctx: &Contexte, dossier: &std::path::Path) -> socle::echec::R<Vec<TauxDUneDsn>> {
    let par_nir: std::collections::HashMap<&droit::fiche::Nir, &droit::contexte::IdSalarie> = ctx.salaries.iter().filter_map(|(cle, s)| s.nir.dite().map(|n| (n, cle))).collect();
    let fichiers = declarations::dsn_lue::fichiers(dossier)?;
    // salarie -> mois -> releve (dans l'ordre d'insertion des salaries, mois tries).
    let mut trouve: Vec<(droit::contexte::IdSalarie, std::collections::BTreeMap<socle::mois::Mois, TauxDUneDsn>)> = Vec::new();
    for f in fichiers {
        let Some(d) = declarations::dsn_lue::lire_une_dsn(&f)? else { continue };
        let Some(mois) = d.mois else { continue };
        for v in &d.versements {
            // « -1 » : un CDD court sans taux personnalise (`norme-dsn/texte/bloc-S21.G00.50.txt`, S21.G00.50.008) ;
            // aucun taux n'a ete transmis. Sans la date du versement (S21.G00.50.001), le taux ne se date pas.
            let (Some(id), Some(j), Some(cle)) = (v.nets.identifiant_pas.as_ref().filter(|i| !i.sans_crm()), v.verse_le, v.nir.as_ref().and_then(|n| par_nir.get(n))) else {
                continue;
            };
            // Un taux que la DSN n'ecrit pas comme la norme (deux chiffres, deux decimales au plus) n'en pose aucun : une
            // DSN venue d'ailleurs se lit au mieux (`dsn_lue`, vide), et le compte rendu de l'import le dit.
            let rel = match v.nets.taux_pas {
                Some(t) => TauxDUneDsn::Transmis(
                    (*cle).clone(),
                    TauxRecu { taux: t, r#type: v.nets.type_pas, identifiant: id.clone(), mis_a_disposition: instant_du_jour(j)?, en_retour_de: None },
                ),
                None => TauxDUneDsn::Illisible { salarie: (*cle).clone(), verse_le: j },
            };
            match trouve.iter_mut().find(|(k, _)| k == *cle) {
                Some((_, mm)) => {
                    mm.insert(mois, rel);
                }
                None => trouve.push(((*cle).clone(), [(mois, rel)].into_iter().collect())),
            }
        }
    }
    Ok(trouve.into_iter().filter_map(|(_, mm)| mm.into_values().next_back()).collect())
}
