//! L'EFFECTIF D'UNE ANNEE D'AVANT PUPPETPAIE, D'APRES SES PAIES REPRISES : ce que « Pour démarrer » propose a la
//! compagnie venue d'un autre logiciel, qu'elle confirme par la route `effectif` (`declarations::demarrage::route`).
//! Le calcul est celui de L. 130-1 et R. 130-1 du code de la securite sociale (`droit::effectif`, le meme que l'Urssaf
//! simulee) ; ce module ne dit que les contrats que les pieces reprises portent, chaque paie versee
//! (`importe::PaieVersee`) sous le contrat qu'elle paie. Une proposition est une valeur que la compagnie confirme d'un
//! clic : elle est juste ou ne se fait pas, et sa phrase dit pourquoi (regle 1).

use std::collections::BTreeSet;

use droit::contexte::{Contexte, IdSalarie};
use droit::effectif::{ContratDansLEffectif, TempsDansLEffectif};
use droit::fiche::Lue as _;
use droit::natures::{ModaliteDuTempsDeTravail, QuotiteDuContrat};
use socle::date::{Jour, PeriodeOuverte};
use socle::echec::R;
use socle::grandeurs::{Effectif, Heures};
use socle::mois::{Annee, Graphie, Mois};

use crate::importe::{Attestation, Importe, PaieVersee};

/// L'EFFECTIF D'UNE ANNEE D'APRES SES PAIES REPRISES : l'annee, l'effectif que L. 130-1 et R. 130-1 en tirent (`valeur`,
/// au centieme), et la phrase qui dit d'ou il vient et ce que les pieces ne distinguent pas. Sans valeur, la phrase dit
/// pourquoi il ne se propose pas.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct EffectifRepris {
    pub annee: Annee,
    #[serde(serialize_with = "socle::grandeurs::ecrit::nombre_json::option::serialize")]
    pub valeur: Option<Effectif>,
    pub phrase: String,
}

/// LA PREMIERE EMBAUCHE QUE LES PIECES REPRISES DISENT, que « Pour démarrer » propose dans la case de la premiere
/// embauche (`premiere_embauche` de la fiche Entreprise) quand la compagnie ne l'a pas dite : le jour le plus ancien ou
/// commence un contrat qu'une piece reprise porte (`jour` ; L. 130-1, I, CSS : « l'année de création du premier emploi
/// salarié titulaire d'un contrat de travail », `atelier/reference/sources/code-de-la-securite-sociale/
/// articles-20261011.md` l. 294), et la piece qui le dit (`d_ou`). Une proposition, que la compagnie valide ou corrige :
/// rien ne dit au logiciel que les pieces reprises sont toutes les siennes (regle 1).
#[derive(Debug, Clone, PartialEq)]
pub struct PremiereEmbaucheReprise {
    pub jour: Jour,
    pub d_ou: String,
}

/// LE DEBUT DE CONTRAT LE PLUS ANCIEN QUE LES PIECES REPRISES DISENT (`PremiereEmbaucheReprise`), chaque engagement repris
/// aux dates du contrat que paie chacune de ses paies (`Importe::contrat_de_la_paie`) : celles qu'une DSN deposee declare
/// (S21.G00.40.001), ou qu'une AEM ou un certificat Congés Spectacles atteste ; un contrat connu par ses seuls bulletins
/// commence au premier jour de la periode que paie le plus ancien (`routes::importation::ecrite`) : un bulletin ne dit pas
/// le jour de l'embauche, seulement la periode qu'il paie, et la phrase le dit. A jour egal, la piece qui dit le contrat
/// l'emporte sur le bulletin. `None` : aucune piece reprise ne dit le debut d'un contrat.
fn premiere_embauche_reprise(ctx: &Contexte, importes: &[Importe]) -> R<Option<PremiereEmbaucheReprise>> {
    // (jour, connu par un bulletin seul, l'engagement, le mois de sa paie)
    let mut plus_ancien: Option<(Jour, bool, &Importe, Mois)> = None;
    for i in importes {
        let du_bulletin = !i.paies().values().any(|p| p.contrat.numero_declare().is_some())
            && i.contrat().is_none_or(|c| c.atteste_par == Attestation::Bulletin);
        for (mois, p) in i.paies() {
            // Une paie dont le contrat ne se lit pas (ce que l'effectif range en manque) ne propose rien, et ne ferme
            // pas la page.
            let contrat = match i.contrat_de_la_paie(p) {
                Ok(Some(c)) => c,
                Ok(None) => continue,
                Err(e) if e.dit_un_manque() => continue,
                Err(e) => return Err(e),
            };
            let jour = contrat.debut();
            if plus_ancien.is_none_or(|(j, b, _, _)| (jour, du_bulletin) < (j, b)) {
                plus_ancien = Some((jour, du_bulletin, i, *mois));
            }
        }
    }
    let Some((jour, _, i, mois)) = plus_ancien else { return Ok(None) };
    // Un engagement qui ne dit pas son salarie : la date ne se propose pas sans dire de qui elle vient.
    let qui = match i.cle_ecrite() {
        Ok(cle) => ctx.nomme(&cle),
        Err(e) if e.dit_un_manque() => return Ok(None),
        Err(e) => return Err(e),
    };
    let declaree = i.paies().values().any(|p| p.contrat.numero_declare().is_some());
    let d_ou = match (declaree, i.contrat().map(|c| c.atteste_par)) {
        (true, _) => format!("le début du contrat de {} que déclare une DSN déposée", qui),
        (false, Some(Attestation::Aem)) => format!("le début du contrat de {} qu'atteste son AEM", qui),
        (false, Some(Attestation::Certificat)) => format!("le début du contrat de {} qu'atteste son certificat Congés Spectacles", qui),
        (false, Some(Attestation::Bulletin) | None) => format!(
            "le premier jour de la période que paie le bulletin de {} de {} (un bulletin ne dit pas le jour de l'embauche, seulement la période qu'il paie)",
            qui,
            mois.libelle(Graphie::Accentuee)
        ),
    };
    Ok(Some(PremiereEmbaucheReprise { jour, d_ou }))
}

/// CE QU'AUCUNE PAIE REPRISE NE DISTINGUE, et que la compagnie confirme avec la valeur : les personnes que L. 1111-3 du
/// code du travail exclut de l'effectif (R. 130-1, III, CSS : `atelier/reference/sources/code-du-travail/
/// legifrance-code-du-travail-L1111-3-20261118.pdf`). La DSN les declare (S21.G00.40.008) ; la paie reprise n'en garde
/// rien, et la nature d'un contrat repris (CDI, CDD, CDDU) ne les dit pas.
const SANS_L_1111_3: &str = "Les apprentis, les contrats de professionnalisation et les contrats aidés (CIE, CAE) ne comptent pas (C. trav. L. 1111-3) : les paies reprises ne les distinguent pas ; s'il y en a eu, dites l'effectif sans eux.";

/// LES HEURES PAR MOIS DU CONTRAT `contrat` que paie `p`, le mois `mois` (S21.G00.40.013, en heures) : celles que
/// l'engagement dit (la case « Heures par mois », `importation-nature`), sinon la quotite que la DSN deposee declare,
/// sinon, d'un mois que le contrat et le bulletin paient entier et sans DSN, la duree du travail que le bulletin imprime
/// en en-tete (« le nombre d'heures de travail auquel le salaire se rapporte », C. trav. R. 3243-1, 5° : la lecture que
/// le glossaire, `Heures`, ne permet qu'a cette condition, comme la comparaison, `ecarts::salaire_et_heures`). Les heures
/// d'une ligne de salaire sont celles que le mois paie : elles ne disent pas la duree du contrat.
fn heures_du_contrat(i: &Importe, contrat: PeriodeOuverte, mois: Mois, p: &PaieVersee) -> Option<Heures> {
    let declarees = match p.quotite {
        Some(QuotiteDuContrat::Heures(h)) => Some(h),
        Some(QuotiteDuContrat::Jours(_)) | None => None,
    };
    let entier = p.contrat.d_un_bulletin() && crate::ecarts::couvre_le_mois(Some(contrat), mois) && crate::ecarts::couvre_le_mois(p.periode, mois);
    i.objet().carte().quotite_contrat.or(declarees).or(p.duree_du_travail.filter(|_| entier))
}

/// LE CONTRAT QUE PAIE `p`, LE MOIS `mois`, tel que l'effectif le compte (`droit::effectif::ContratDansLEffectif`).
///
/// Sa nature d'abord, puis son motif de recours (celui de l'engagement ou de la DSN, sinon celui que la nature fixe ; un
/// CDI n'en a pas) : un CDD qui remplace un absent ne compte pas, et son temps de travail ne se lit pas.
///
/// SES DATES (R. 130-1, II, quatrieme alinea : « à due proportion du nombre de jours du mois pendant lequel elles ont été
/// employées ») : celles du contrat (`Importe::contrat_de_la_paie`). Le contrat d'un permanent ne se compte que du mois
/// paye : des mois d'apres, les pieces ne disent rien. Les autres (le CDDU) ont leurs dates d'une DSN deposee
/// (S21.G00.40.001, .010) ou d'une AEM (le contrat imprime, `import::aem_lu`) ; connu par son seul bulletin, le contrat a
/// les dates que l'import lui donne partout, la periode que le bulletin imprime (`Importe::contrat_de_la_paie`), et le
/// second terme rendu le dit (`vrai`) : un logiciel qui imprimerait le mois entier pour deux jours de cachets ferait
/// compter trop, jamais trop peu, et la phrase de la proposition le dit a la compagnie qui confirme (Léandre,
/// 18/11/2026 : « j'ai donné tous les bulletins de paie, il devrait prendre ça comme acquis »).
///
/// SON TEMPS DE TRAVAIL (troisieme alinea) : temps plein ou partiel au sens de L. 3123-1 du code du travail ; a temps
/// partiel, ses heures par mois (`heures_du_contrat`) sur la plus courte de la duree de reference et de la duree legale,
/// comme `Engagement::modalite_temps_travail` le dit. INFERENCE : le CDDU, que sa nature dit « salarié non concerné » par
/// la modalite du temps de travail (S21.G00.40.014 = 99, `contrats-base.json`), n'est pas a temps partiel au sens de
/// L. 3123-1 (il n'a pas de duree contractuelle hebdomadaire, mensuelle ni annuelle a comparer a la duree legale) : il
/// compte entier, a due proportion des jours de son contrat.
///
/// Refus, qui nomme le salarie : ce qu'aucune piece ne dit et dont le compte depend.
fn contrat_repris(ctx: &Contexte, i: &Importe, mois: Mois, p: &PaieVersee) -> R<(ContratDansLEffectif<IdSalarie>, bool)> {
    let cle = i.cle_ecrite()?;
    let qui = ctx.nomme(&cle);
    let Some(nature) = i.nature() else {
        return Err(socle::refus!("la nature du contrat de {} (CDI ou CDD) reste à dire : un CDD qui remplace un salarié absent ne compte pas", qui));
    };
    let nat = nature.proprietes()?;
    let motif_recours = i.motif_recours().or(nat.motif_recours_fixe());
    if motif_recours.is_none() && !nat.duree_indeterminee {
        return Err(socle::refus!("aucune pièce ne dit le motif de recours du {} de {} : un CDD qui remplace un salarié absent ne compte pas", nat.libelle, qui));
    }
    let Some(contrat) = i.contrat_de_la_paie(p)? else {
        return Err(socle::refus!("ni l'engagement repris ni la paie de {} ne disent le début du contrat de {}", mois.libelle(Graphie::Accentuee), qui));
    };
    let du_bulletin_seul = !(i.permanent() || p.contrat.numero_declare().is_some() || i.contrat().is_some_and(|c| c.atteste_par == Attestation::Aem));
    let periode: PeriodeOuverte = match contrat.fin() {
        Some(_) => contrat,
        None => crate::contrat_dans_le_mois(&contrat, mois, &qui)?.into(),
    };
    let carte = i.objet().carte();
    let temps = || {
        match nat.modalite_fixee().or(carte.modalite_temps_travail) {
            Some(m) if m != ModaliteDuTempsDeTravail::TempsPartiel => return Ok(TempsDansLEffectif::PLEIN),
            Some(_) | None => {}
        }
        let Some(heures) = heures_du_contrat(i, contrat, mois, p) else {
            return Err(socle::refus!(
                "aucune pièce de {} ne dit les heures par mois du contrat de {} : la DSN les déclare ; la « Durée du travail » d'un bulletin ne les dit que d'un mois qu'il paie entier",
                mois.libelle(Graphie::Accentuee),
                qui
            ));
        };
        let legale = nat.quotite_reference_defaut().ok_or_else(|| socle::echec::livree(&droit::natures::BASE, &format!("{}.quotite_reference_defaut", nat.libelle)))?;
        let reference = match carte.quotite_reference {
            Some(r) if r < legale => r,
            Some(_) | None => legale,
        };
        TempsDansLEffectif::partiel(QuotiteDuContrat::Heures(heures), QuotiteDuContrat::Heures(reference)).map_err(|e| e.reformuler(|m| format!("le contrat de {} : {}", qui, m)))
    };
    Ok((ContratDansLEffectif::de(cle, periode, motif_recours, None, temps)?, du_bulletin_seul))
}

/// CE QUE LA COMPAGNIE CONFIRME AVEC LA VALEUR quand un contrat d'usage de l'annee n'est connu que par son bulletin.
const PAR_LE_BULLETIN_SEUL: &str = "Un contrat d'usage connu par son seul bulletin compte sur la période que ce bulletin imprime : si elle est plus longue que le contrat, l'effectif est compté trop haut ; une DSN déposée donnerait ses dates.";

/// L'effectif dit dans une phrase : au centieme, la virgule decimale, « salarié » au singulier sous deux.
fn dit(e: Effectif) -> String {
    let s = if e >= Effectif::personnes(2) { "salariés" } else { "salarié" };
    format!("{} {}", socle::texte::virgule(e.a_deux_decimales()), s)
}

/// Les mois dits dans une phrase : « mars, avril ».
fn mois_dits(m: &[Mois]) -> String {
    m.iter().map(|m| m.nom(Graphie::Accentuee)).collect::<Vec<_>>().join(", ")
}

/// LES EFFECTIFS DES ANNEES `annees`, D'APRES LES PAIES REPRISES (`EffectifRepris`) : une par annee dont une paie au
/// moins est reprise d'un ancien logiciel ; les autres n'en ont pas. Une annee ne se propose pas, et sa phrase dit
/// pourquoi :
/// - quand un engagement d'ici touche un de ses mois : ses paies faites dans PuppetPaie ne sont pas des paies reprises,
///   et la moyenne des seules paies reprises ne serait pas celle de l'annee ;
/// - quand c'est l'annee de la premiere embauche, et que le mois de cette embauche n'a pas de paie reprise : pour cette
///   annee-la, L. 130-1, I, troisieme alinea, ne demande pas une moyenne mais l'effectif present le dernier jour de ce
///   mois (`droit::effectif::effectif_au_jour`), qui se propose quand ce mois est repris ;
/// - quand la date de la premiere embauche n'est pas dite et que c'est l'annee de la plus ancienne paie reprise : elle
///   est peut-etre celle de la premiere embauche (les annees d'apres ne le sont pas : un salarie y etait deja paye), et
///   la phrase dit la date que les pieces reprises proposent (`premiere_embauche_reprise`), a valider ou a corriger ;
/// - quand une de ses paies a un contrat qui ne se compte pas (`contrat_repris`).
///
/// Sinon, l'effectif de chaque mois compte chaque contrat repris qui le touche (`droit::effectif::effectif_du_mois`) ; la
/// moyenne ne prend que les mois dont une paie est reprise : un mois sans paie reprise n'est pas un mois sans salarie
/// (R. 130-1, I), on ne sait pas s'il en avait, et la phrase le nomme (Léandre : « on propose quand même »). Ce qu'aucune
/// paie reprise ne distingue (`SANS_L_1111_3`) se dit avec la valeur. Une panne remonte.
///
/// Rendue avec, quand la date de la premiere embauche n'est pas dite, celle que les pieces reprises proposent
/// (`PremiereEmbaucheReprise`) : « Pour démarrer » en remplit la case, que la compagnie valide ou corrige.
pub fn effectifs_repris(ctx: &Contexte, annees: &[Annee]) -> R<(Vec<EffectifRepris>, Option<PremiereEmbaucheReprise>)> {
    let objets = crate::stock::objets(&ctx.racine)?;
    let mut importes = Vec::new();
    for o in &objets {
        if o.est_non_actif() {
            continue;
        }
        if let Some(i) = Importe::de(o.clone())? {
            importes.push(i);
        }
    }
    // Chaque paie reprise, sous son contrat ; ce qui ne se compte pas, l'annee de sa paie.
    let mut contrats: Vec<ContratDansLEffectif<IdSalarie>> = Vec::new();
    let mut manques: Vec<(Annee, String)> = Vec::new();
    let mut repris: BTreeSet<Mois> = BTreeSet::new();
    let mut par_le_bulletin: BTreeSet<Annee> = BTreeSet::new();
    for i in &importes {
        for (mois, p) in i.paies() {
            repris.insert(*mois);
            match contrat_repris(ctx, i, *mois, p) {
                Ok((c, du_bulletin_seul)) => {
                    if du_bulletin_seul {
                        par_le_bulletin.insert(mois.annee());
                    }
                    contrats.push(c);
                }
                Err(e) if e.dit_un_manque() => manques.push((mois.annee(), e.phrase)),
                Err(e) => return Err(e),
            }
        }
    }
    let premiere = ctx.entreprise.premiere_embauche.dite().copied();
    let proposee = match premiere {
        Some(_) => None,
        None => premiere_embauche_reprise(ctx, &importes)?,
    };
    let Some(plus_ancien) = repris.iter().next().copied() else { return Ok((Vec::new(), proposee)) };
    let mut out = Vec::new();
    for an in annees {
        let mois_repris: Vec<Mois> = an.les_mois().into_iter().filter(|m| repris.contains(m)).collect();
        if mois_repris.is_empty() {
            continue;
        }
        let rien = |phrase: String| EffectifRepris { annee: *an, valeur: None, phrase };
        // Ce que la compagnie confirme avec la valeur.
        let a_savoir = match par_le_bulletin.contains(an) {
            true => format!("{} {}", PAR_LE_BULLETIN_SEUL, SANS_L_1111_3),
            false => SANS_L_1111_3.to_string(),
        };
        // Les mois que la saisie d'ici touche.
        let mut ici = Vec::new();
        for m in an.les_mois() {
            let mut touche = false;
            for o in &objets {
                touche = touche || o.couvre(m)?;
            }
            if touche {
                ici.push(m);
            }
        }
        if !ici.is_empty() {
            out.push(rien(format!(
                "L'effectif de {} ne se calcule pas d'après les paies reprises : {} {} se paient dans PuppetPaie, et la moyenne des seules paies reprises ne serait pas celle de l'année.",
                an,
                mois_dits(&ici),
                an
            )));
            continue;
        }
        let mut raisons: Vec<String> = Vec::new();
        for (_, m) in manques.iter().filter(|(a, _)| a == an) {
            if !raisons.contains(m) {
                raisons.push(m.clone());
            }
        }
        if !raisons.is_empty() {
            out.push(rien(format!("L'effectif de {} ne se calcule pas d'après les paies reprises : {}.", an, raisons.join(" ; "))));
            continue;
        }
        match premiere {
            // L'ANNEE DE LA PREMIERE EMBAUCHE (L. 130-1, I, troisieme alinea) : l'effectif present le dernier jour du mois
            // de cette embauche.
            Some(j) if j.annee() == *an => {
                let m = j.mois_civil();
                let dernier = m.dernier_jour();
                if !repris.contains(&m) {
                    out.push(rien(format!(
                        "{} est l'année de la première embauche : son effectif est celui présent le {}, le dernier jour du mois de cette embauche (L. 130-1, I, CSS), et aucune paie de {} n'est reprise.",
                        an,
                        dernier.jj_mm_aaaa(),
                        m.libelle(Graphie::Accentuee)
                    )));
                    continue;
                }
                let e = droit::effectif::effectif_au_jour(dernier, &contrats).tronque_au_centieme();
                out.push(EffectifRepris {
                    annee: *an,
                    valeur: Some(e),
                    phrase: format!(
                        "D'après les paies reprises de {} : {} présent le {}, le dernier jour du mois de la première embauche ; pour cette année-là, c'est lui qui compte, pas une moyenne (L. 130-1, I, CSS). {}",
                        m.libelle(Graphie::Accentuee),
                        dit(e),
                        dernier.jj_mm_aaaa(),
                        a_savoir
                    ),
                });
                continue;
            }
            // L'annee de la date proposee (la case de la premiere embauche dit d'ou elle vient, `demarrage::route`) ;
            // sans proposition, celle de la plus ancienne paie reprise.
            None if match &proposee {
                Some(p) => p.jour.annee(),
                None => plus_ancien.annee(),
            } == *an =>
            {
                out.push(rien(match &proposee {
                    Some(p) => format!(
                        "{} est l'année de la première embauche proposée (le {}) : son effectif serait alors celui présent le dernier jour du mois de cette embauche, pas une moyenne (L. 130-1, I, CSS). Validez ou corrigez la date de la première embauche : l'effectif de {} se proposera ensuite d'après elle.",
                        an,
                        p.jour.jj_mm_aaaa(),
                        an
                    ),
                    None => format!(
                        "{} est peut-être l'année de la première embauche (la plus ancienne paie reprise est de {}) : son effectif serait alors celui présent le dernier jour du mois de cette embauche, pas une moyenne (L. 130-1, I, CSS). Dites la date de la première embauche : d'avant {}, l'effectif de {} se proposera d'après les paies reprises.",
                        an,
                        plus_ancien.libelle(Graphie::Accentuee),
                        an,
                        an
                    ),
                }));
                continue;
            }
            Some(_) | None => {}
        }
        let par_mois: Vec<Option<Effectif>> = mois_repris.iter().map(|m| droit::effectif::effectif_du_mois(*m, &contrats)).collect();
        let sans_paie: Vec<Mois> = an.les_mois().into_iter().filter(|m| !repris.contains(m)).collect();
        let manquent = match sans_paie.len() {
            0 => String::new(),
            _ => format!(" Aucune paie reprise en {} : la moyenne ne compte que les mois repris.", mois_dits(&sans_paie)),
        };
        out.push(match droit::effectif::effectif_annuel(par_mois) {
            Some(e) => EffectifRepris { annee: *an, valeur: Some(e), phrase: format!("D'après les paies reprises de {} : {}.{} {}", an, dit(e), manquent, a_savoir) },
            None => rien(format!("Les paies reprises de {} ne comptent aucun salarié employé.{}", an, manquent)),
        });
    }
    Ok((out, proposee))
}
