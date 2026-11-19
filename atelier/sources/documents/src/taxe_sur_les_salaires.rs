//! La taxe sur les salaires (CGI art. 231) : ce que la fiche entreprise en dit et ce que le moteur en conclut, les
//! plafonds de la franchise en base de TVA, l'assiette d'une paie et les fractions aux taux majores. L'etat « taxe sur
//! les salaires » (`etats_compta`), la base de chaque salarie de chaque DSN mensuelle (`declarations::dsn`, composant 91)
//! et le bloc S21.G00.44 de la DSN de decembre (`declarations::assujettissement`) lisent les memes fonctions.

use droit::fiche::Lue as _;
use socle::echec::R;
use socle::texte::{euros, virgule, Phrase};
use socle::grandeurs::{Montant, Taux};

/// Le champ de la fiche entreprise qui dit la taxe sur les salaires.
pub const CHAMP_TAXE_SUR_LES_SALAIRES: droit::fiche::CaseEntreprise = droit::fiche::CaseEntreprise::TaxeSurLesSalaires;

// ------------------------------------------------------------ la taxe sur les salaires, salarie par salarie

/// L'ASSIETTE DE LA TAXE SUR LES SALAIRES D'UNE PAIE VALIDEE : celle de la CSG, sans la reduction de 1,75 % (CGI art.
/// 231, 1 : « La réduction mentionnée au I de l'article L. 136-2 du code de la sécurité sociale n'est pas applicable ») :
/// son assiette de la CSG, plus l'abattement de la part que la CSG a abattue (`calcul::Contenu::part_abattue_csg`, que
/// le calcul ecrit, sous la limite de l'abattement et regularisee sur l'annee, et qu'une paie ecrite avant se lit dans
/// ses chiffres : `paie_ecrite::FormeDePaie::part_abattue_csg` ; `Pile::abattement_de`). Elle ne contredit pas le calcul
/// de la CSG : elle lui rend ce qu'il a abattu, rien d'autre. L'etat « taxe sur les salaires » (`etats_compta`), la DSN
/// mensuelle (`base_au_taux_normal`) et celle de decembre (`declarations::assujettissement`) la lisent.
pub fn assiette_taxe_sur_les_salaires(p: &engagement::calcul::Paie, bar: &droit::forme_bareme::Pile) -> Montant {
    (p.base_csg + bar.abattement_de(p.part_abattue_csg)).au_centime()
}

/// LA BASE IMPOSABLE AU TAUX NORMAL D'UNE PAIE (S21.G00.79.001 de type « 91 », sous la base 03) : son assiette
/// (`assiette_taxe_sur_les_salaires`), multipliee par la part taxee des remunerations (`part`, le rapport de CGI art. 231,
/// 1, que `taxe_sur_les_salaires_de_la_fiche` conclut), au centime. Les textes (`atelier/reference/norme-dsn/texte/`) :
/// - `chapitre-2-principes-de-constitution.txt`, l. 2048-2049 : « Les bases imposables au taux normal sont déclarées
///   mensuellement au niveau de chaque salarié et ajustées de paie en paie selon le principe de la régularisation
///   progressive » ; l. 2051-2052 : « Les bases imposables aux taux majorés sont déclarées annuellement, uniquement au
///   niveau de l'établissement dans le bloc base assujettie » (le bloc 44 de decembre, `declarations::assujettissement`) ;
/// - `bloc-S21.G00.78.txt`, l. 127 : la DGFiP attend la base « 03 » ; `bloc-S21.G00.79.txt`, l. 67 : « DGFiP : "90",
///   "91" » ; la fiche net-entreprises 1265, tableau P27V01, feuille DGFiP (`atelier/reference/sources/
///   net-entreprises-base-connaissances/net-entreprises-fiche1265-combinaisons-78-79-81-p27v01-20260630.txt`, l. 317) :
///   « 03 - Assiette brute déplafonnée ; 91 - Base de taxe sur les salaires au taux normal ».
/// INFERENCE : la part taxee s'applique a la base de chaque paie comme a l'assiette de l'annee du bloc 44 (BOFiP
/// BOI-TPS-TS-30, § 240 : le rapport s'applique aux « rémunérations individuelles »).
pub fn base_au_taux_normal(p: &engagement::calcul::Paie, bar: &droit::forme_bareme::Pile, part: Taux) -> Montant {
    assiette_taxe_sur_les_salaires(p, bar).fois_taux(part).au_centime()
}

/// L'ASSIETTE DE LA TAXE SUR LES SALAIRES D'UNE PAIE DU MOIS (`assiette_taxe_sur_les_salaires`), ou d'une paie importee
/// telle que l'import la donne ; `None` : ses pieces ne disent pas son brut ou son assiette de la CSG.
///
/// UNE PAIE IMPORTEE ne dit que son brut et son assiette de la CSG (la DSN, bloc 78 de type 04) : ni la part que l'ancien
/// logiciel a abattue, ni le plafond qu'il a applique (`importe::PaieVersee`), et rien ne se reconstruit (regle 1). Son
/// assiette se prend telle que l'import la donne, sans recalcul (Leandre, 19/11/2026) : l'assiette de la CSG, plus
/// l'abattement de tout le brut au facteur de la CSG du bareme (`abattement_csg`). INFERENCE, une approximation : exacte
/// quand l'ancien logiciel a abattu tout le brut ; au-dessus de la limite, elle rend aussi l'abattement d'une part qu'il
/// n'a pas abattue.
pub fn assiette_d_une_paie_du_mois(p: &engagement::etats::PaieDuMois, bar: &droit::forme_bareme::Pile) -> R<Option<Montant>> {
    match p {
        engagement::etats::PaieDuMois::Validee(v) => Ok(Some(assiette_taxe_sur_les_salaires(v, bar))),
        engagement::etats::PaieDuMois::Importee(_) => {
            let (Some(brut), Some(base_csg)) = (p.brut(), p.base_csg()?) else { return Ok(None) };
            Ok(Some((base_csg - (brut * bar.abattement_csg).au_centime() + brut).au_centime()))
        }
    }
}

/// Les fractions d'une remuneration individuelle annuelle soumises aux taux majores (CGI art. 231, 2 bis) : celle
/// comprise entre `seuil_1` et `seuil_2`, et celle qui excede `seuil_2`.
pub fn fractions_majorees(base: Montant, seuil_1: Montant, seuil_2: Montant) -> (Montant, Montant) {
    let positif = |x: Montant| if x > Montant::ZERO { x } else { Montant::ZERO };
    let premiere = positif(base.min(seuil_2) - seuil_1);
    (premiere, positif(base - seuil_2))
}

/// LA TAXE SUR LES SALAIRES SE DEDUIT DE LA TVA. La fiche disait « la compagnie la doit-elle ? »
/// et « part des salaires taxee » : on demandait a l'administratrice la conclusion du droit, pas le fait qu'elle connait.
/// Elle dit maintenant ce qu'elle sait de sa TVA, et le moteur conclut, une seule fois, ici -- pour l'ecran (la route
/// `taxe-sur-les-salaires`), pour chaque DSN mensuelle (sa base 91) et celle de decembre (`blocs`) et pour l'etat « taxe sur les salaires » (`etats_compta`).
///
/// Le texte : CGI art. 231, 1 (version en vigueur depuis le 19/06/2025, deposee :
/// `atelier/reference/sources/code-general-des-impots/xml/231.xml`, lisible dans `articles-20261015.md`) :
/// - la taxe est a la charge des employeurs « lorsqu'ils ne sont pas assujettis a la taxe sur la valeur ajoutee ou ne
///   l'ont pas ete sur 90 % au moins de leur chiffre d'affaires au titre de l'annee civile precedant celle du paiement » ;
/// - son assiette : les remunerations multipliees par « le rapport [...] entre le chiffre d'affaires qui n'a pas ete
///   passible de la taxe sur la valeur ajoutee et le chiffre d'affaires total », ce total s'entendant « du total des
///   recettes et autres produits, y compris ceux [...] qui n'entrent pas dans le champ d'application de la [TVA] », le
///   numerateur « du total des recettes et autres produits qui n'ont pas ouvert droit a deduction » ;
/// - 2e alinea : « Les remunerations versees par les employeurs dont le chiffre d'affaires realise au cours de l'annee
///   civile precedant le versement de ces remunerations n'excede pas les plafonds definis aux I et I bis de l'article
///   293 B sont exonerees de la taxe sur les salaires. »
/// L'abattement des associations (art. 1679 A, depose a cote) ne change pas qui doit la taxe, seulement ce qu'il reste a
/// payer : l'etat le calcule.
///
/// La fiche (`taxe_sur_les_salaires` de config/entreprise.json) :
/// - `tva` : « assujettie » ou « exoneree » (exoneree de TVA, ou hors de son champ) ;
/// - `part_tva` : pour une compagnie assujettie, la part (en %) de ses recettes et produits de l'annee precedente,
///   subventions comprises, soumise a la TVA. INFERENCE : une recette soumise a la TVA est une recette qui a ouvert droit
///   a deduction ; le rapport de l'article 231 vaut donc 100 - `part_tva`. A 90 au moins, la taxe n'est pas due ;
/// - `sous_franchise` : quand la taxe serait due, le chiffre d'affaires de l'annee precedente n'excede-t-il pas les
///   plafonds de l'article 293 B ? (2e alinea) ;
/// - `association_1679a` : quand la taxe est due, l'abattement de l'article 1679 A s'applique-t-il ?
/// Chaque case ne se montre que si la precedente l'appelle (`champs`). Une case qui manque fait refuser chaque DSN
/// mensuelle, avec ce qu'il faut dire (regle 1 : rien ne se devine).
#[derive(Debug, Clone, PartialEq)]
pub struct RegimeTaxeSalaires {
    pub decision: Decision,
    /// L'abattement des associations (art. 1679 A), quand la taxe est due.
    pub association: bool,
    /// Les cases de la fiche que l'ecran montre, dans l'ordre : chacune parce que la precedente l'appelle.
    pub champs: Vec<&'static str>,
    /// La consequence, en clair, telle que l'ecran l'affiche sous les cases.
    pub consequence: socle::texte::Phrase,
}

/// Ce que le moteur conclut de la fiche : la taxe n'est pas due ; elle est due sur `p` % des remunerations (le rapport
/// de l'article 231, 1) ; ou la phrase de ce que la fiche doit encore dire.
#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    NonDue,
    DueSur(Taux),
    ADire(String),
}

/// `taxe_sur_les_salaires` de la fiche entreprise `ent`, et ce que le moteur en conclut (CGI art. 231, 1). `plafonds` :
/// ceux de la franchise en base de TVA, lus au bareme de l'annee (`plafonds_franchise`) ; la conclusion les nomme quand
/// on les a, elle dit « les plafonds » sinon. Ils ne changent rien a la decision.
pub fn regime_taxe_sur_les_salaires(ent: &droit::fiche::ChampsEntreprise, plafonds: Option<(Montant, Montant)>) -> RegimeTaxeSalaires {
    // Les deux plafonds, en mots de compagnie, pour la conclusion.
    let les_plafonds = match plafonds {
        Some((services, total)) => format!(
            "les deux plafonds de la franchise en base de TVA, {} de spectacles vendus et {} de recettes en tout",
            euros(services),
            euros(total)
        ),
        None => "les plafonds de la franchise en base de TVA".to_string(),
    };
    let fiche = ent.taxe_sur_les_salaires.dite();
    let mut champs = vec!["tva"];
    let manque = |champs: Vec<&'static str>, quoi: String, consequence: Phrase| RegimeTaxeSalaires {
        decision: Decision::ADire(format!(
            "Configuration › Entreprise, « Taxe sur les salaires » : {} (CGI art. 231, 1). Sans cette reponse, aucune DSN \
mensuelle ne se fait : chacune declare, si la compagnie la doit, la base de chaque salarie.",
            quoi
        )),
        association: false,
        champs,
        consequence,
    };
    let non_due = |champs: Vec<&'static str>, pourquoi: &str| RegimeTaxeSalaires {
        decision: Decision::NonDue,
        association: false,
        champs,
        consequence: socle::phrase!(
            "La compagnie ne doit pas la taxe sur les salaires : {}. La DSN de décembre le déclare.",
            pourquoi
        ),
    };
    let part_taxee = match fiche.and_then(|f| f.tva.dite()) {
        Some(droit::fiche::Tva::Exoneree) => Taux::CENT_POUR_CENT,
        Some(droit::fiche::Tva::Assujettie) => {
            champs.push("part_tva");
            let part = match fiche.and_then(|f| f.part_tva.dite()) {
                None => None,
                Some(&p) => match p {
                    p if p >= Taux::ZERO && p <= Taux::CENT_POUR_CENT => Some(p),
                    p => {
                        return manque(
                            champs,
                            format!("la part des recettes soumise a la TVA vaut {}, qui n'est pas un pourcentage de 0 a 100", socle::texte::virgule(p)),
                            Phrase::fixe("La part des recettes soumise à la TVA est un pourcentage, de 0 à 100."),
                        )
                    }
                },
            };
            let Some(part) = part else {
                return manque(
                    champs,
                    "dire quelle part de ses recettes de l'annee precedente etait soumise a la TVA".to_string(),
                    Phrase::fixe(
                        "Dites quelle part de vos recettes de l'an dernier était soumise à la TVA : à 90 % ou plus, la \
compagnie ne doit pas la taxe sur les salaires ; en dessous, elle la doit sur le reste.",
                    ),
                );
            };
            if part >= Taux::points(90) {
                return non_due(
                    champs,
                    &format!(
                        "elle est assujettie à la TVA sur {} % de ses recettes de l'an dernier, 90 % ou plus (code général \
des impôts, art. 231)",
                        virgule(part)
                    ),
                );
            }
            Taux::CENT_POUR_CENT - part
        }
        None => {
            return manque(
                champs,
                "dire si la compagnie est assujettie a la TVA ou exoneree".to_string(),
                Phrase::fixe(
                    "Dites si la compagnie est assujettie à la TVA ou exonérée : le logiciel en déduit si elle doit la taxe \
sur les salaires. Sans réponse, aucune DSN ne se fait.",
                ),
            )
        }
    };
    // La taxe serait due : sauf sous les plafonds de la franchise en base (231, 1, 2e alinea).
    champs.push("sous_franchise");
    let sur = if part_taxee == Taux::CENT_POUR_CENT {
        "sur tous ses salaires".to_string()
    } else {
        format!("sur {} % de ses salaires, la part de ses recettes qui n'était pas soumise à la TVA", virgule(part_taxee))
    };
    match fiche.and_then(|f| f.sous_franchise.dite().copied()) {
        Some(true) => {
            return non_due(
                champs,
                &format!(
                    "ses recettes de l'an dernier sont restées sous {} (code général des impôts, art. 231 et 293 B)",
                    les_plafonds
                ),
            )
        }
        Some(false) => {}
        _ => {
            return manque(
                champs,
                "dire si le chiffre d'affaires de l'annee precedente depasse les plafonds de la franchise en base de TVA \
(art. 293 B)"
                    .to_string(),
                socle::phrase!(
                    "La compagnie doit la taxe sur les salaires {}, sauf si ses recettes de l'an dernier sont restées sous \
{} (code général des impôts, art. 231 et 293 B) : dites-le.",
                    sur, les_plafonds
                ),
            )
        }
    }
    champs.push("association_1679a");
    let association = fiche.is_some_and(|f| f.association_1679a.oui());
    RegimeTaxeSalaires {
        decision: Decision::DueSur(part_taxee),
        association,
        champs,
        consequence: socle::phrase!(
            "La compagnie doit la taxe sur les salaires, {} (code général des impôts, art. 231). Chaque DSN déclare la \
base de chaque salarié au taux normal, celle de décembre les assiettes de l'année ; l'état « Taxe sur les salaires » (Exports) calcule ce qui reste à payer après la franchise et la \
décote{}.",
            sur,
            if association { ", puis l'abattement des associations (art. 1679 A)" } else { "" }
        ),
    }
}

/// LES PLAFONDS DE LA FRANCHISE EN BASE DE TVA, EN CLAIR. L'ecran demandait « sous les plafonds
/// de l'article 293 B ? » sans dire lesquels : « on pourrait pas avoir les plafonds directement dans le logiciel ? ». Le
/// moteur les lit au bareme de l'annee (`taxe_sur_les_salaires.plafonds_franchise`) et ecrit la question et ses deux
/// reponses avec les montants ; la page n'en porte aucun.
///
/// Les textes (deposes, lus dans `atelier/reference/sources/code-general-des-impots/LISEZ-MOI.md`) :
/// - CGI art. 231, 1, 2e alinea (`xml/231.xml`) : sont exonerees les remunerations des employeurs « dont le chiffre
///   d'affaires realise au cours de l'annee civile precedant le versement de ces remunerations n'excede pas les plafonds
///   definis aux I et I bis de l'article 293 B » ;
/// - CGI art. 293 B, I (`xml/293_B.xml`, version en vigueur depuis le 01/03/2025) : ligne « Annee civile precedente »,
///   85 000 EUR de « chiffre d'affaires national total » et 37 500 EUR de chiffre d'affaires « afferent aux prestations de
///   services » ;
/// - la notice 2502-SD (`formulaires/impots-2502-sd-2025-notice.pdf`, C, 1) : ce chiffre d'affaires « doit s'apprecier au
///   regard de l'ensemble des recettes et autres produits, y compris [...] subventions non imposables a la TVA ».
/// INFERENCES, ecrites comme telles : un spectacle vendu (cession, billetterie) est une prestation de services, d'ou le
/// plafond de 37 500 EUR pour une compagnie ; les subventions comptent dans le total (85 000 EUR), pas dans les prestations ;
/// la colonne « annee civile precedente » est celle que vise 231 (le chiffre d'affaires de l'annee d'avant) ; le I bis
/// (avocats, auteurs, artistes-interpretes) ne vise pas une compagnie employeur.
#[derive(Debug, Clone, PartialEq)]
pub struct QuestionFranchise {
    /// L'aide de la question, avec les montants de l'annee : un paragraphe par ligne (`\n`).
    pub aide: String,
    /// La reponse « sous les plafonds » (`sous_franchise` vrai), et « au-dessus » (faux).
    pub sous: String,
    pub dessus: String,
    /// Les plafonds lus au bareme : prestations de services, total. `None` si le bareme de l'annee ne les porte pas.
    pub plafonds: Option<(Montant, Montant)>,
}

/// Les plafonds de la franchise en base de TVA au bareme `bar` (`taxe_sur_les_salaires.plafonds_franchise`) : prestations
/// de services, total. `None` si le bareme ne les porte pas.
pub fn plafonds_franchise(bar: Option<&droit::forme_bareme::Pile>) -> Option<(Montant, Montant)> {
    let pf = &bar?.taxe_sur_les_salaires.as_ref()?.plafonds_franchise;
    Some((pf.prestations_de_services, pf.total))
}

/// La question des plafonds pour les remunerations de `annee` (le chiffre d'affaires de `annee - 1`), depuis le bareme
/// de `annee` (`bar`, `None` s'il n'y en a pas). Un bareme sans plafonds ne fait pas refuser : la question se pose sans
/// montants, et le dit ; une date d'entree en vigueur qui ne se lit pas refuse.
pub fn question_franchise(bar: Option<&droit::forme_bareme::Pile>, annee: socle::mois::Annee) -> R<QuestionFranchise> {
    let avant = annee.precedente()?;
    let pf = bar.and_then(|b| b.taxe_sur_les_salaires.as_ref()).map(|t| &t.plafonds_franchise);
    let lus = plafonds_franchise(bar);
    let Some((services, total)) = lus else {
        return Ok(QuestionFranchise {
            aide: format!(
                "Le barème {} du logiciel ne porte pas les plafonds de la franchise en base de TVA (code général des impôts, \
art. 293 B) : ils se lisent dans cet article, pour le chiffre d'affaires de {}. Sous ces plafonds, la compagnie ne doit pas la taxe sur les salaires.",
                annee, avant
            ),
            sous: "Sous les deux plafonds".to_string(),
            dessus: "Au-dessus d'un des plafonds".to_string(),
            plafonds: None,
        });
    };
    // Une version de 293 B entree en vigueur en cours d'annee se dit (2025 : le 01/03).
    let vigueur = pf.map(|pf| pf.en_vigueur_depuis);
    let depuis = if let Some(j) = vigueur.filter(|j| j.annee() == annee && *j != annee.premier_jour()) {
        format!(", en vigueur depuis le {}", j.jj_mm_aaaa())
    } else {
        String::new()
    };
    Ok(QuestionFranchise {
        aide: format!(
            "Les deux plafonds se comparent aux recettes de l'année civile {avant} (du 1er janvier au 31 décembre), pour \
les salaires de {annee}. Sous les deux, la compagnie ne doit pas la taxe sur les salaires ; au-dessus d'un seul, elle la \
doit.\n{s} : les spectacles vendus, c'est-à-dire les cessions, la billetterie, les ateliers et vos autres prestations. Les \
subventions n'y entrent pas.\n{t} : les recettes en tout, ces ventes plus les subventions et tous les autres produits.\n\
Ces montants sont dans le compte de résultat {avant} (ses produits), ou chez votre comptable. Code général des impôts, \
art. 231 et 293 B{depuis}.",
            avant = avant,
            s = euros(services),
            t = euros(total),
            annee = annee,
            depuis = depuis,
        ),
        sous: format!(
            "Sous les deux plafonds en {} : au plus {} de spectacles vendus (cessions, billetterie, ateliers…) et au plus \
{} de recettes en tout, subventions comprises",
            avant,
            euros(services),
            euros(total)
        ),
        dessus: format!(
            "Au-dessus d'un plafond en {} : plus de {} de spectacles vendus, ou plus de {} de recettes en tout",
            avant,
            euros(services),
            euros(total)
        ),
        plafonds: Some((services, total)),
    })
}

/// Ce que chaque DSN mensuelle lit (sa base 91, et en decembre le bloc 44) : `None` si la taxe n'est pas due, sinon la part taxee des remunerations en
/// pourcentage. Une fiche qui ne dit pas tout : refus, avec ce qu'il faut dire (`regime_taxe_sur_les_salaires`).
pub fn taxe_sur_les_salaires_de_la_fiche(ent: &droit::fiche::ChampsEntreprise) -> R<Option<Taux>> {
    match regime_taxe_sur_les_salaires(ent, None).decision {
        Decision::NonDue => Ok(None),
        Decision::DueSur(p) => Ok(Some(p)),
        Decision::ADire(phrase) => Err(socle::echec::refus(phrase)),
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_fractions_majorees_suivent_le_2_bis() {
        let d = Montant::euros;
        let (s1, s2) = (d(9147), d(18259));
        assert_eq!(fractions_majorees(d(5000), s1, s2), (Montant::ZERO, Montant::ZERO));
        assert_eq!(fractions_majorees(d(10000), s1, s2), (d(853), Montant::ZERO));
        // L'exemple du BOFiP (BOI-TPS-TS-30, § 240, `atelier/reference/sources/bofip/BOI-TPS-TS-30-20200624.html`), seuils
        // 2020 : 180 000 -> 7 977 et 164 019.
        assert_eq!(fractions_majorees(d(180000), d(8004), d(15981)), (d(7977), d(164019)));
    }
}
