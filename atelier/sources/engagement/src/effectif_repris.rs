//! L'EFFECTIF D'UNE ANNEE D'AVANT PUPPETPAIE, D'APRES SES PAIES REPRISES : ce que « Pour démarrer » propose a la
//! compagnie venue d'un autre logiciel, qu'elle confirme par la route `effectif` (`declarations::demarrage::manques`).
//! Le calcul est celui de R. 130-1 du code de la securite sociale (`droit::effectif`, le meme que l'Urssaf simulee) ; ce
//! module ne dit que les contrats que les pieces reprises portent, chaque paie versee (`importe::PaieVersee`) sous le
//! contrat qu'elle paie. Ce qu'une piece ne porte pas ne s'invente pas (regle 1) : l'annee ne se propose pas, et sa
//! phrase dit ce qui manque.

use std::collections::BTreeSet;

use droit::contexte::{Contexte, IdSalarie};
use droit::effectif::{ContratDansLEffectif, TempsDansLEffectif};
use droit::natures::{ModaliteDuTempsDeTravail, QuotiteDuContrat};
use socle::date::PeriodeOuverte;
use socle::echec::R;
use socle::grandeurs::{Effectif, Heures};
use socle::mois::{Annee, Graphie, Mois};

use crate::importe::{Importe, PaieVersee};

/// L'EFFECTIF D'UNE ANNEE D'APRES SES PAIES REPRISES : l'annee, l'effectif annuel que R. 130-1 en tire (`valeur`, au
/// centieme), et la phrase qui dit d'ou il vient et quels mois n'ont aucune paie reprise. Sans valeur, la phrase dit ce
/// qui manque pour le calculer.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct EffectifRepris {
    pub annee: Annee,
    #[serde(serialize_with = "socle::grandeurs::ecrit::nombre_json::option::serialize")]
    pub valeur: Option<Effectif>,
    pub phrase: String,
}

/// Le nom d'un salarie tel qu'une phrase le dit : celui de sa fiche, sinon sa cle.
fn nom(ctx: &Contexte, cle: &IdSalarie) -> String {
    match ctx.salaries.get(cle).and_then(|f| f.nomme()) {
        Some(n) => n,
        None => cle.to_string(),
    }
}

/// LES HEURES PAR MOIS DU CONTRAT que paie `p` (S21.G00.40.013, en heures) : celles que l'engagement dit (la case « Heures
/// par mois », `importation-nature`), sinon la quotite que la DSN deposee declare, sinon la duree du travail que le
/// bulletin imprime en en-tete (« le nombre d'heures de travail auquel le salaire se rapporte », C. trav. R. 3243-1,
/// 5°). Les heures d'une ligne de salaire sont celles que le mois paie (une absence, des conges les changent) : elles
/// ne disent pas la duree du contrat.
fn heures_du_contrat(i: &Importe, p: &PaieVersee) -> Option<Heures> {
    let declarees = match p.quotite {
        Some(QuotiteDuContrat::Heures(h)) => Some(h),
        Some(QuotiteDuContrat::Jours(_)) | None => None,
    };
    i.objet().carte().quotite_contrat.or(declarees).or(p.duree_du_travail)
}

/// LE CONTRAT QUE PAIE `p`, LE MOIS `mois`, tel que l'effectif le compte (`droit::effectif::ContratDansLEffectif`) : son
/// salarie, ses dates (`Importe::contrat_de_la_paie` ; un contrat sans fin, celui d'un permanent, ne se compte que du
/// mois paye : des mois d'apres, les pieces ne disent rien), son motif de recours (celui de l'engagement ou de la DSN,
/// sinon celui que la nature fixe ; un CDI n'en a pas), et son temps de travail (temps plein ou partiel, L. 3123-1 du
/// code du travail : ses heures par mois sur la plus courte de la duree de reference et de la duree legale, comme
/// `Engagement::modalite_temps_travail` le dit). Refus, qui nomme le salarie : ce qu'aucune piece ne dit et dont le
/// compte depend.
fn contrat_repris(ctx: &Contexte, i: &Importe, mois: Mois, p: &PaieVersee) -> R<ContratDansLEffectif<IdSalarie>> {
    let cle = i.cle_ecrite()?;
    let qui = nom(ctx, &cle);
    let Some(contrat) = i.contrat_de_la_paie(p)? else {
        return Err(socle::refus!("ni l'engagement repris ni la paie de {} ne disent le début du contrat de {}", mois.libelle(Graphie::Accentuee), qui));
    };
    let periode: PeriodeOuverte = match contrat.fin() {
        Some(_) => contrat,
        None => crate::contrat_dans_le_mois(&contrat, mois, &qui)?.into(),
    };
    let Some(nature) = i.nature() else {
        return Err(socle::refus!("la nature du contrat de {} (CDI ou CDD) reste à dire : un CDD qui remplace un salarié absent ne compte pas", qui));
    };
    let nat = nature.proprietes()?;
    let motif_recours = i.motif_recours().or(nat.motif_recours_fixe());
    if motif_recours.is_none() && !nat.duree_indeterminee {
        return Err(socle::refus!("aucune pièce ne dit le motif de recours du {} de {} : un CDD qui remplace un salarié absent ne compte pas", nat.libelle, qui));
    }
    let carte = i.objet().carte();
    let modalite = nat.modalite_fixee().or(carte.modalite_temps_travail);
    let temps = match modalite {
        // Le CDDU (99, « salarié non concerné ») n'est pas a temps partiel au sens de L. 3123-1 : il compte entier.
        Some(m) if m != ModaliteDuTempsDeTravail::TempsPartiel => TempsDansLEffectif::Plein,
        _ => {
            let Some(heures) = heures_du_contrat(i, p) else {
                return Err(socle::refus!(
                    "aucune pièce de {} ne dit les heures par mois du contrat de {} (la DSN les déclare, le bulletin les imprime en « Durée du travail ») : sans elles, un temps partiel ne se compte pas",
                    mois.libelle(Graphie::Accentuee),
                    qui
                ));
            };
            let legale = nat.quotite_reference_defaut().ok_or_else(|| socle::echec::livree(&droit::natures::BASE, &format!("{}.quotite_reference_defaut", nat.libelle)))?;
            let reference = match carte.quotite_reference {
                Some(r) if r < legale => r,
                Some(_) | None => legale,
            };
            TempsDansLEffectif::Partiel { quotite: QuotiteDuContrat::Heures(heures), reference: QuotiteDuContrat::Heures(reference) }
        }
    };
    Ok(ContratDansLEffectif { qui: cle, periode, motif_recours, temps })
}

/// L'effectif dit dans une phrase : au centieme, la virgule decimale, « salarié » au singulier sous deux.
fn dit(e: Effectif) -> String {
    let s = if e >= Effectif::personnes(2) { "salariés" } else { "salarié" };
    format!("{} {}", socle::texte::virgule(e.a_deux_decimales()), s)
}

/// LES EFFECTIFS DES ANNEES `annees`, D'APRES LES PAIES REPRISES (`EffectifRepris`) : une par annee dont une paie au
/// moins est reprise d'un ancien logiciel ; les autres n'en ont pas. L'effectif d'un mois compte chaque contrat repris
/// qui le touche (`droit::effectif::effectif_du_mois`) ; la moyenne de l'annee ne prend que les mois dont une paie est
/// reprise : un mois sans paie reprise n'est pas un mois sans salarie (R. 130-1, I), on ne sait pas s'il en avait, et la
/// phrase le nomme. Une paie dont le contrat ne se compte pas (`contrat_repris`) empeche l'annee de sa paie : sa phrase
/// dit pourquoi. Une panne remonte.
pub fn effectifs_repris(ctx: &Contexte, annees: &[Annee]) -> R<Vec<EffectifRepris>> {
    let mut importes = Vec::new();
    for o in crate::stock::objets(&ctx.racine)? {
        if o.est_non_actif() {
            continue;
        }
        if let Some(i) = Importe::de(o)? {
            importes.push(i);
        }
    }
    // Chaque paie reprise, sous son contrat ; ce qui ne se compte pas, l'annee de sa paie.
    let mut contrats: Vec<ContratDansLEffectif<IdSalarie>> = Vec::new();
    let mut manques: Vec<(Annee, String)> = Vec::new();
    let mut repris: BTreeSet<Mois> = BTreeSet::new();
    for i in &importes {
        for (mois, p) in i.paies() {
            repris.insert(*mois);
            match contrat_repris(ctx, i, *mois, p) {
                Ok(c) => contrats.push(c),
                Err(e) if e.dit_un_manque() => manques.push((mois.annee(), e.phrase)),
                Err(e) => return Err(e),
            }
        }
    }
    let mut out = Vec::new();
    for an in annees {
        let mois_repris: Vec<Mois> = an.les_mois().into_iter().filter(|m| repris.contains(m)).collect();
        if mois_repris.is_empty() {
            continue;
        }
        let mut raisons: Vec<String> = Vec::new();
        for (_, m) in manques.iter().filter(|(a, _)| a == an) {
            if !raisons.contains(m) {
                raisons.push(m.clone());
            }
        }
        if !raisons.is_empty() {
            out.push(EffectifRepris { annee: *an, valeur: None, phrase: format!("L'effectif de {} ne se calcule pas d'après les paies reprises : {}.", an, raisons.join(" ; ")) });
            continue;
        }
        let mut par_mois = Vec::new();
        for m in &mois_repris {
            par_mois.push(droit::effectif::effectif_du_mois(*m, &contrats)?);
        }
        let sans_paie: Vec<&str> = an.les_mois().into_iter().filter(|m| !repris.contains(m)).map(|m| m.nom(Graphie::Accentuee)).collect();
        let manquent = match sans_paie.len() {
            0 => String::new(),
            _ => format!(" Aucune paie reprise en {} : la moyenne ne compte que les mois repris.", sans_paie.join(", ")),
        };
        out.push(match droit::effectif::effectif_annuel(par_mois) {
            Some(e) => EffectifRepris { annee: *an, valeur: Some(e), phrase: format!("D'après les paies reprises de {} : {}.{}", an, dit(e), manquent) },
            None => EffectifRepris { annee: *an, valeur: None, phrase: format!("Les paies reprises de {} ne comptent aucun salarié employé.{}", an, manquent) },
        });
    }
    Ok(out)
}
