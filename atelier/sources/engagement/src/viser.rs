//! VISER UN MONTANT : le taux qu'il faudrait pour que des engagements de la saisie coutent, versent ou paient brut un
//! montant donne. La recherche refait le calcul de la saisie pour l'ecran (`pour_l_ecran`) a chaque essai, les salaires
//! des engagements vises multiplies par un meme facteur ; rien ne s'ecrit.

use crate::LignePaie;

use crate::calcul::Paie;
use crate::saisie::{Carte, RangDansLeMois, VueDuMois};
use droit::contexte::Contexte;
use socle::echec::Echec;
use crate::ligne::{QuantiteDeLigne, TauxDeLigne};
use socle::grandeurs::{Coefficient, Montant, MontantUnitaire};

/// CE QUI SE VISE : le brut ; le net a payer et le cout employeur, chacun avec ou sans les indemnites de frais (les
/// lignes de frais hors assiette : repas et nuitees dans leur limite, defraiements, `HorsAssiette::Frais`). Le net a
/// payer les compte (`Paie::net_a_payer`), le cout employeur non (`Paie::cout_employeur`) ; le brut n'en a jamais.
/// Le net a payer ne compte pas les avantages en nature (R. 3243-1, 10°) : viser un net, c'est viser l'argent recu.
/// L'acompte, une avance et non une indemnite, reste ce qu'il est : retire du net, hors du cout. La charge du montant
/// vise le dit `brut`, `net`, `net_sans_frais`, `cout`, `cout_avec_frais`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cible {
    Brut,
    Cout,
    CoutAvecFrais,
    Net,
    NetSansFrais,
}

/// Les indemnites de frais de la paie `p` : ses lignes hors assiette qui remboursent un frais (`HorsAssiette::Frais`).
fn frais_de(ctx: &Contexte, p: &Paie) -> R<Montant> {
    let mut total = Montant::ZERO;
    for l in p.remuneration.iter().filter(|l| !l.soumis) {
        let r = droit::catalogue::rubrique(&ctx.racine, &l.code, &ctx.lue_au).map_err(RefusVisee::Refus)?;
        if r.hors_assiette().map_err(RefusVisee::Refus)? == droit::catalogue::HorsAssiette::Frais {
            total += l.montant;
        }
    }
    Ok(total)
}

/// Pourquoi la recherche n'aboutit pas : un refus de la recherche elle-meme, ou le calcul de la saisie qui refuse.
#[derive(Debug, Clone)]
pub enum RefusVisee {
    Refus(Echec),
    Calcul(Echec),
}

/// Le meilleur essai : la saisie aux taux trouves, ses paies, le montant atteint, celui du depart, le facteur applique
/// aux salaires et le nombre d'essais.
pub struct Visee {
    pub saisie: VueDuMois,
    pub paies: Vec<Paie>,
    pub atteint: Montant,
    pub depart: Montant,
    pub facteur: Coefficient,
    pub tours: usize,
}

type R<T> = Result<T, RefusVisee>;

#[track_caller]
fn refus(m: impl Into<String>) -> RefusVisee {
    RefusVisee::Refus(socle::echec::refus(m))
}

const TOURS: usize = 12;

/// La rubrique de la ligne ; une rubrique que le catalogue ne connait pas se refuse.
fn rubrique_de(ctx: &Contexte, ligne: &LignePaie) -> R<droit::catalogue::Rubrique> {
    droit::catalogue::rubrique(&ctx.racine, ligne.rubrique(), &ctx.lue_au).map_err(RefusVisee::Refus)
}

/// Une ligne dont le taux est une case : en unites (un salaire, une absence, une indemnite), ou de frais
/// (`LignePaie::taux_saisi_mut`).
fn taux_saisi(l: &LignePaie) -> bool {
    l.taux_saisi().is_some()
}

/// Un SALAIRE se hausse ; un defraiement, une prime, un acompte, non.
fn ligne_ajustable(ctx: &Contexte, ligne: &LignePaie) -> R<bool> {
    Ok(taux_saisi(ligne) && rubrique_de(ctx, ligne)?.nature == droit::catalogue::NatureDeRubrique::Salaire && !ligne.taux().is_none_or(TauxDeLigne::est_nul))
}

/// Une ligne dont le taux est une case vide.
fn taux_vide(l: &LignePaie) -> bool {
    taux_saisi(l) && l.taux().is_none()
}

/// L'engagement de rang `i` de la saisie.
fn engagement_de(data: &mut VueDuMois, i: RangDansLeMois) -> R<&mut Carte> {
    i.dans_mut(&mut data.engagements).ok_or_else(|| refus(format!("La saisie n'a pas d'engagement de rang {}.", i)))
}

/// Une ligne de salaire SANS TAUX recoit un taux provisoire de 1 EUR.
fn poser_taux_provisoires(ctx: &Contexte, data: &mut VueDuMois, rangs: &[RangDansLeMois]) -> R<()> {
    for &i in rangs {
        let e = engagement_de(data, i)?;
        let lignes = e.lignes().to_vec();
        let mut ajustable = false;
        for l in &lignes {
            ajustable |= ligne_ajustable(ctx, l)?;
        }
        if e.salaire_mensuel.is_some() || ajustable {
            continue;
        }
        let mut sans_taux: Vec<(usize, String)> = Vec::new();
        for (k, ligne) in lignes.iter().enumerate() {
            if !taux_saisi(ligne) {
                continue;
            }
            let r = rubrique_de(ctx, ligne)?;
            if r.nature == droit::catalogue::NatureDeRubrique::Salaire && ligne.taux().is_none_or(TauxDeLigne::est_nul) && !ligne.quantite(r.unite).is_none_or(QuantiteDeLigne::est_nul) {
                sans_taux.push((k, r.libelle));
            }
        }
        if sans_taux.len() > 1 {
            let qui = match &e.salarie {
                Some(s) => s.to_string(),
                None => "cet engagement".to_string(),
            };
            let noms: Vec<&str> = sans_taux.iter().map(|(_, l)| l.as_str()).collect();
            return Err(refus(format!(
                "{} : {} lignes de salaire sans taux ({}). Le montant vise ne dit pas comment le partager entre elles : saisir un taux \
                 sur chacune, meme approximatif, et l'ajustement gardera leurs proportions.",
                qui,
                sans_taux.len(),
                noms.join(", ")
            )));
        }
        // Une ligne sans taux a sa case du taux (`taux_saisi`).
        if let Some(case) = sans_taux.first().and_then(|(k, _)| e.lignes.as_mut().and_then(|ls| ls.get_mut(*k))).and_then(LignePaie::taux_saisi_mut) {
            *case = Some(Montant::euros(1).pour_une_unite());
        }
    }
    // Les AUTRES blocs du mois repartent comme la page les enverrait d'ordinaire : sans leurs lignes incompletes.
    for (i, e) in RangDansLeMois::de_chacun(data.engagements.iter_mut()) {
        let Some(ls) = e.lignes.as_mut() else { continue };
        if rangs.contains(&i) {
            for case in ls.iter_mut().filter_map(LignePaie::taux_saisi_mut).filter(|c| c.is_none()) {
                *case = Some(MontantUnitaire::ZERO);
            }
        } else {
            ls.retain(|l| !taux_vide(l));
        }
    }
    Ok(())
}

/// Le salaire mensuel de la carte multiplie par `facteur`, au centime, dans sa forme la plus courte.
fn mis_a_l_echelle(x: MontantUnitaire, facteur: Coefficient) -> MontantUnitaire {
    socle::grandeurs::courte((x * facteur).au_centime())
}

/// L'engagement recopie, ses salaires multiplies -- et rien d'autre. Rend aussi s'il y avait quelque chose a multiplier.
fn a_l_echelle(ctx: &Contexte, e: &mut Carte, facteur: Coefficient) -> R<bool> {
    let mut touche = false;
    if let Some(s) = e.salaire_mensuel {
        e.salaire_mensuel = Some(mis_a_l_echelle(s, facteur));
        touche = true;
    }
    for l in e.lignes.iter_mut().flatten() {
        if !ligne_ajustable(ctx, l)? {
            continue;
        }
        // Une ligne ajustable dit son taux dans sa case (`taux_saisi`) : au centime, dans sa forme la plus courte
        // (`mis_a_l_echelle`).
        if let Some(Some(t)) = l.taux_saisi_mut() {
            *t = socle::grandeurs::courte((*t * facteur).au_centime());
        }
        touche = true;
    }
    Ok(touche)
}

/// La `cible` additionnee des paies des engagements de rangs `rangs` : le calcul du mois rend une paie par engagement de
/// la saisie `data`, dans son ordre.
fn somme(ctx: &Contexte, data: &VueDuMois, paies: &[Paie], rangs: &[RangDansLeMois], cible: Cible) -> R<Montant> {
    let mut total = socle::grandeurs::Montant::ZERO;
    let mut vues = 0;
    for (_, p) in RangDansLeMois::de_chacun(&data.engagements).zip(paies).filter(|((i, _), _)| rangs.contains(i)) {
        vues += 1;
        total += match cible {
            Cible::Net => p.net_a_payer,
            Cible::NetSansFrais => p.net_a_payer - frais_de(ctx, p)?,
            Cible::Brut => p.brut,
            Cible::Cout => p.cout_employeur,
            Cible::CoutAvecFrais => p.cout_employeur + frais_de(ctx, p)?,
        };
    }
    if vues != rangs.len() {
        return Err(refus("le calcul du mois n'a pas cette paie"));
    }
    Ok(total)
}

/// Les salaires des engagements de rangs `rangs` (tries, sans doublon, presents dans la saisie `data`) qui font
/// atteindre `vise` a leur `cible` additionnee. Une ligne de salaire sans taux en recoit un provisoire ; le facteur se
/// cherche par approches successives (au plus `TOURS` essais) et le MEILLEUR essai gagne, jamais le dernier : a egale
/// distance, celui qui ne depasse pas.
pub fn viser(ctx: &Contexte, data: &VueDuMois, rangs: &[RangDansLeMois], cible: Cible, vise: Montant) -> R<Visee> {
    // Un engagement ne se vise qu'une fois : un rang en double dans la charge se refuse, nomme.
    if let Some(double) = rangs.iter().enumerate().find_map(|(k, r)| rangs[..k].contains(r).then_some(*r)) {
        return Err(refus(format!("Le rang {} est en double dans la charge : un engagement ne se vise qu'une fois.", double)));
    }
    let mut data = data.clone();
    // Une ligne au tarif de la convention a son taux : elle n'est pas une ligne sans taux.
    crate::saisie_au_tarif(ctx, &mut data).map_err(RefusVisee::Calcul)?;
    poser_taux_provisoires(ctx, &mut data, rangs)?;
    let essai = |facteur: Coefficient| -> R<(VueDuMois, Vec<Paie>, Montant)> {
        let mut d = data.clone();
        let mut touche = false;
        for &i in rangs {
            touche |= a_l_echelle(ctx, engagement_de(&mut d, i)?, facteur)?;
        }
        if !touche {
            return Err(refus(
                "Rien a ajuster : cet engagement ne porte aucune ligne de salaire avec une quantite. Un defraiement, une prime ou un \
                 acompte ne se recalcule pas depuis une enveloppe -- c'est un montant, pas un taux. Poser d'abord une ligne de paie \
                 (cachet, service, heure, journee...) avec sa quantite ; le taux, lui, peut rester vide.",
            ));
        }
        let paies = crate::charger_donnees(ctx, &d).map_err(RefusVisee::Calcul)?.paies;
        let atteint = somme(ctx, &d, &paies, rangs, cible)?;
        Ok((d, paies, atteint))
    };

    let (_, _, depart) = essai(Coefficient::UN)?;
    if depart <= Montant::ZERO {
        return Err(refus(format!(
            "Cet engagement paie {} : il n'y a pas de taux à faire varier. Poser d'abord une quantité et un taux, puis viser le total.",
            socle::texte::euros(depart)
        )));
    }
    let rang_essai = |atteint: Montant| ((atteint - vise).abs(), atteint > vise);
    let mut facteur = vise.rapport(depart).ok_or_else(|| refus("Ajustement impossible : le départ est nul."))?.a_decimales(20);
    let mut vus = vec![facteur];
    let mut meilleur: Option<Visee> = None;
    for tour in 1..=TOURS {
        let (d, paies, atteint) = essai(facteur)?;
        if meilleur.as_ref().is_none_or(|m| rang_essai(atteint) < rang_essai(m.atteint)) {
            meilleur = Some(Visee { saisie: d, paies, atteint, depart, facteur, tours: tour });
        }
        if atteint == vise || atteint <= Montant::ZERO {
            break;
        }
        let suivant = (vise * facteur).rapport(atteint).ok_or_else(|| refus("Ajustement impossible : montants trop grands."))?.a_decimales(20);
        if vus.contains(&suivant) {
            break;
        }
        vus.push(suivant);
        facteur = suivant;
    }
    meilleur.ok_or_else(|| refus("aucun essai"))
}
