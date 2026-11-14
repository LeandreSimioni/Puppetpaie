//! Les ecritures comptables d'un mois : le journal de paie en ecritures (`ecritures_du_mois`, le plan comptable livre
//! recouvert par `config/plan-comptable.json`) et leur CSV (`Compta_AAAA.csv`) : point-virgule, virgule decimale,
//! UTF-8 avec BOM, fins de ligne CRLF. Tout part des paies du mois (`engagement::etats::paies_du_mois` : ecrites ou importees).

use droit::fiche::Lue as _;
use std::collections::BTreeMap;

use droit::catalogue::{self, HorsAssiette};
use droit::contexte::Contexte;
use droit::forme_bareme::{CHOMAGE, MALADIE, REDUCTION_GENERALE, RETRAITE_T1};
use engagement::ecarts::RegroupementDeComparaison;
use socle::echec::{erreur, Classe, Echec, R};
use socle::mois::Mois;
use socle::num::{eur2, zero2, Decimal};

use crate::plan_comptable::{compte_du_salarie, Plan, Refus};

fn refus(m: String) -> Echec {
    erreur(Classe::Refuse, m)
}

/// Un compte qui manque au plan comptable de la compagnie : le fichier et la cle a remplir vont a part.
fn manque_au_plan(m: String, cle: String) -> Echec {
    socle::echec::refus_avec(m, socle::echec::Lieu { fichier: Some(crate::plan_comptable::PLAN_COMPTABLE.to_string()), cle: Some(cle), ..Default::default() })
        .reclasser(Classe::Refuse)
}

droit::cle_opaque! {
    /// LE CODE D'UN JOURNAL COMPTABLE (`journal` du plan comptable : « OD ») : celui de chaque ligne d'ecriture. Sa largeur
    /// et ses lettres, chaque logiciel du comptable les borne a l'ecriture (`plan_comptable::controler_plan`).
    pub struct CodeJournal, "code journal";
}

droit::cle_opaque! {
    /// LE NUMERO D'UN COMPTE du plan comptable (« 641100 », un sous-compte individuel « 421001 ») : ceux du brut, du net,
    /// des frais, du prelevement a la source, des retenues, des lignes, des tiers et de la provision de conges, celui de
    /// chaque ligne d'ecriture. Sa largeur et ses chiffres, chaque logiciel du comptable les borne a l'ecriture.
    pub struct NumeroDeCompte, "numéro de compte";
}

droit::cle_opaque! {
    /// LE POSTE ANALYTIQUE d'Oxygene (`poste_analytique` du plan comptable, la zone CODAGE) : celui des lignes ventilees,
    /// que seule la compagnie dit.
    pub struct PosteAnalytique, "poste analytique";
}

/// Une ligne du journal de paie : date, journal, compte, libelle, debit, credit, analytique, auxiliaire.
pub struct Ecriture {
    pub journal: CodeJournal,
    pub date: socle::date::Jour,
    pub compte: NumeroDeCompte,
    pub libelle: String,
    pub debit: Decimal,
    pub credit: Decimal,
    /// Le spectacle (le brut, les frais) ou le salarie (le net, une retenue) que la ligne porte ; aucun pour une charge,
    /// un tiers ou le prelevement a la source.
    pub analytique: Option<String>,
    pub mois: Mois,
    /// Le sous-compte individuel du salarie, quand le plan en tient un (`net.par_salarie`).
    pub auxiliaire: Option<NumeroDeCompte>,
    /// Le libelle du sous-compte : le salarie.
    pub auxiliaire_libelle: Option<String>,
    /// Posee par les etats des ecritures comptables (« PAIE-AAAA-MM »).
    pub piece: String,
    /// Le rang de l'ecriture (une par mois) dans l'etat, a partir de 1.
    pub numero: i64,
}

/// La date de l'ecriture d'un mois (son dernier jour) et son libelle (« Decembre 2026 »).
fn date_et_libelle(mois: Mois) -> (socle::date::Jour, String) {
    (mois.dernier_jour(), mois.libelle(socle::mois::Graphie::Titre))
}

/// Une ecriture pour le mois, depuis les paies validees.
/// Le brut par spectacle, la charge patronale et le tiers par ligne de bulletin, le prelevement a la source retenu
/// (une dette envers l'Etat, au compte `pas` du plan), le net par salarie -- le net PAYE, apres l'impot, celui du
/// bulletin. Un identifiant de ligne absent du plan fait refuser : on ne devine pas un compte.
pub fn ecritures_du_mois(ctx: &Contexte, mois: Mois, plan: &Plan, departs: &engagement::departs::Departs) -> R<Vec<Ecriture>> {
    let paies = engagement::etats::paies_du_mois(ctx, mois, departs).map_err(|m| m.reclasser(Classe::Refuse))?;
    let (date, lib_mois) = date_et_libelle(mois);
    let jrn = plan.journal.clone();
    let zero = zero2();

    let mut brut_par_spectacle: BTreeMap<String, Decimal> = BTreeMap::new();
    let mut net_par_salarie: BTreeMap<String, Decimal> = BTreeMap::new();
    let mut compte_de: BTreeMap<String, Option<NumeroDeCompte>> = BTreeMap::new();
    let mut frais_par_spectacle: BTreeMap<String, Decimal> = BTreeMap::new();
    let mut charge: BTreeMap<(NumeroDeCompte, String), Decimal> = BTreeMap::new();
    let mut tiers: BTreeMap<NumeroDeCompte, Decimal> = BTreeMap::new();
    // LE PRELEVEMENT A LA SOURCE : celui du bulletin (`prelevement::des_paies`, la meme lecture, la meme position), retenu sur
    // le net et du a la DGFiP (compte 4421 « Prelevements a la source (Impot sur le revenu) » du plan
    // comptable general, atelier/reference/sources/plan-comptable-general/LISEZ-MOI.md), tel que la paie le porte. Le PAS d'un
    // mois declare est celui de la paie declaree (`engagement::etats::paie_declaree`), lu et non recalcule au taux du
    // jour (CGI art. 1671 : `prelevement::des_paies`).
    let pas_des_paies = crate::prelevement::du_mois(ctx, &paies, departs).map_err(|m| m.reclasser(Classe::Refuse))?;
    let mut pas_du_mois = zero;
    // LES RETENUES (une ligne hors assiette de nature `retenue` : acompte, avance, saisie) ne sont pas une charge : un
    // acompte solde une avance deja versee (425 « Personnel - Avances et acomptes et autres comptes debiteurs »), une
    // saisie est due a un tiers (427 « Personnel - Oppositions »), plan de comptes de l'ANC (meme LISEZ-MOI). Le plan
    // dit le compte de chaque rubrique (`retenues`) ; une retenue qu'il ne nomme pas fait refuser. Par (compte,
    // libelle, salarie).
    let mut retenues: BTreeMap<(NumeroDeCompte, String, String), Decimal> = BTreeMap::new();
    let ajouter = |m: &mut BTreeMap<String, Decimal>, k: String, v: Decimal| {
        let e = m.entry(k).or_insert(zero);
        *e += v;
    };
    for (p, pas) in paies.iter().zip(&pas_des_paies) {
        // UN PERMANENT N'A PAS DE SPECTACLE : son brut va a sa propre section.
        let titre = match p.spectacle().and_then(|s| s.titre.dite()).filter(|t| !t.is_empty()) {
            Some(t) => t.clone(),
            None => match p.engagement().spectacle.as_deref().filter(|s| !s.is_empty()) {
                Some(s) => s.to_string(),
                None => "Salariés au mois".to_string(),
            },
        };
        let qui = crate::tableaux_export::qui(p.salarie())?;
        // Une ecriture ne s'equilibre qu'avec chacun de ses montants : celui que les pieces d'une paie importee taisent
        // ne s'invente pas, l'ecriture se refuse en le nommant.
        let dit = |m: Option<Decimal>, quoi: &str| -> R<Decimal> {
            m.map(eur2).ok_or_else(|| refus(format!("{}, {} : les pièces importées ne disent pas {} ; les écritures comptables du mois ne s'écrivent pas sans lui.", qui, mois, quoi)))
        };
        let brut = dit(p.brut(), "le brut")?;
        ajouter(&mut brut_par_spectacle, titre.clone(), brut);
        let mut retenu = zero;
        for l in p.remuneration().into_iter().filter(|l| !l.soumis) {
            let Some(code) = &l.code else {
                return Err(refus(format!("La ligne de bulletin « {} » ({}) ne dit pas sa rubrique : rien ne dit si c'est une retenue, elle n'a pas de compte dans le plan comptable.", l.libelle, qui)));
            };
            if catalogue::rubrique(&ctx.racine, code, &ctx.lue_au).and_then(|r| r.hors_assiette()).map_err(|m| m.reclasser(Classe::Refuse))? != HorsAssiette::Mouvement {
                continue;
            }
            let Some(regle) = plan.retenues.get(code.as_str()) else {
                return Err(manque_au_plan(
                    format!(
                        "La retenue « {} » ({}) n'a pas de compte dans le plan comptable. L'y ajouter parmi les retenues : un compte 425 pour un acompte ou une avance, 427 pour une saisie ou une opposition.",
                        code, l.libelle
                    ),
                    format!("retenues.{}", code),
                ));
            };
            // Le libelle du plan, sinon celui de la rubrique.
            let libelle = match &regle.libelle {
                Some(x) => x.clone(),
                None => l.libelle.clone(),
            };
            let montant = dit(l.montant, &format!("le montant de « {} »", l.libelle))?;
            *retenues.entry((regle.compte.clone(), libelle, qui.clone())).or_insert(zero) += montant;
            retenu += montant;
        }
        // CE QUE LE NET PORTE EN PLUS DU BRUT : les indemnites non soumises (repas, defraiements) sont payees
        // sans etre du brut ; sans leur ligne au debit, l'ecriture ne s'equilibrerait pas. Les retenues en sont
        // otees : elles ont leur compte.
        let net_a_payer = dit(p.net_a_payer(), "le net à payer")?;
        let hors_brut = net_a_payer - (brut - dit(p.total_sal(), "les cotisations salariales")?) - retenu;
        if !hors_brut.is_zero() {
            ajouter(&mut frais_par_spectacle, titre.clone(), hors_brut);
        }
        // Le net paye est le net a payer moins l'impot retenu (le « MONTANT NET À PAYER » du bulletin) ; l'impot va
        // a l'Etat. Une paie sans prelevement ne retient rien.
        let mut net = net_a_payer;
        if let Some(v) = pas {
            let prelevement = eur2(v.prelevement());
            pas_du_mois += prelevement;
            net -= prelevement;
        }
        ajouter(&mut net_par_salarie, qui.clone(), net);
        let compte = compte_du_salarie(plan, p.salarie(), &p.salarie().id).map_err(|r| match r {
            Refus::Operation(m) => refus(m),
            Refus::Autre(e) => e,
        })?;
        compte_de.insert(qui.clone(), compte);
        let reduction_generale = p.validee().and_then(|v| v.reduction_generale.as_ref());
        for l in p.cotisations() {
            let Some(id) = l.id.as_ref() else {
                return Err(refus(format!("La ligne de bulletin « {} » ne dit pas sa cotisation : elle n'a pas de compte dans le plan comptable.", l.libelle)));
            };
            let Some(regle) = plan.regle(id) else {
                let parmi = match id {
                    RegroupementDeComparaison::Cotisation(_) => "les lignes",
                    _ => "les regroupements",
                };
                return Err(manque_au_plan(
                    format!(
                        "La ligne de bulletin « {} » ({}) n'a pas de compte dans le plan comptable. L'y ajouter parmi {}, avec un compte de charge et un compte de tiers.",
                        id, l.libelle, parmi
                    ),
                    crate::plan_comptable::cle_dans_le_plan(id),
                ));
            };
            // Le libelle du plan, sinon celui du bulletin.
            let libelle = match &regle.libelle {
                Some(x) => x.clone(),
                None => l.libelle.clone(),
            };
            // Les parts que la ligne porte ; la colonne qu'un bulletin importe laisse vide n'en porte pas.
            let (pat, sal) = (l.montant_pat.map(eur2), l.montant_sal.map(eur2));
            if *id == RegroupementDeComparaison::Cotisation(REDUCTION_GENERALE) {
                if let (Some(rg), Some(pat)) = (reduction_generale, pat) {
                    // La RGDU : une charge negative, ventilee au tiers de chaque part telle que la paie l'a calculee.
                    *charge.entry((regle.charge.clone(), libelle)).or_insert(zero) += pat;
                    for (part, id_tiers) in [(rg.urssaf, MALADIE), (rg.chomage, CHOMAGE), (rg.agirc_arrco, RETRAITE_T1)] {
                        if !part.is_zero() {
                            let r = plan.lignes.get(&id_tiers).ok_or_else(|| socle::fichiers::absent(id_tiers.as_str()).reclasser(Classe::Manque))?;
                            *tiers.entry(r.tiers.clone()).or_insert(zero) -= part;
                        }
                    }
                    continue;
                }
            }
            let parts: Vec<Decimal> = [pat, sal].into_iter().flatten().filter(|x| !x.is_zero()).collect();
            if let Some(pat) = pat.filter(|x| !x.is_zero()) {
                *charge.entry((regle.charge.clone(), libelle)).or_insert(zero) += pat;
            }
            if !parts.is_empty() {
                *tiers.entry(regle.tiers.clone()).or_insert(zero) += parts.into_iter().fold(zero, |a, x| a + x);
            }
        }
    }

    let mut out: Vec<Ecriture> = Vec::new();
    let mut ligne = |compte: &NumeroDeCompte, libelle: &str, debit: Decimal, credit: Decimal, analytique: Option<&str>, auxiliaire: Option<&NumeroDeCompte>| {
        if debit.is_zero() && credit.is_zero() {
            return;
        }
        // Un montant negatif change de colonne plutot que de signe : un logiciel de compta n'aime pas un debit negatif.
        let (mut debit, mut credit) = (debit, credit);
        if debit.is_sign_negative() && !debit.is_zero() {
            credit = -debit;
            debit = zero;
        }
        if credit.is_sign_negative() && !credit.is_zero() {
            debit = -credit;
            credit = zero;
        }
        out.push(Ecriture {
            journal: jrn.clone(),
            date,
            compte: compte.clone(),
            libelle: format!("{} -- {}", libelle, lib_mois),
            debit: eur2(debit),
            credit: eur2(credit),
            analytique: analytique.map(str::to_string),
            mois,
            auxiliaire: auxiliaire.cloned(),
            auxiliaire_libelle: auxiliaire.and(analytique).map(str::to_string),
            piece: String::new(),
            numero: 0,
        });
    };
    for (titre, m) in &brut_par_spectacle {
        ligne(&plan.brut.compte, &plan.brut.libelle, *m, zero, Some(titre), None);
    }
    for (titre, m) in &frais_par_spectacle {
        ligne(&plan.frais.compte, &plan.frais.libelle, *m, zero, Some(titre), None);
    }
    // Les charges dans l'ordre du plan (un compte, un libelle), puis les tiers.
    for ((c, lib), m) in &charge {
        ligne(c, lib, *m, zero, None, None);
    }
    for (c, m) in &tiers {
        let nom = match plan.nom_du_tiers(c) {
            Some(n) => n.to_string(),
            None => format!("Organisme {}", c),
        };
        ligne(c, &nom, zero, *m, None, None);
    }
    // Au credit, une ligne pour le mois : la dette envers la DGFiP, qui la preleve sur la declaration du mois.
    if !pas_du_mois.is_zero() {
        ligne(&plan.pas.compte, &plan.pas.libelle, zero, pas_du_mois, None, None);
    }
    // Une retenue est negative sur le bulletin : elle passe au credit (un acompte retenu solde l'avance).
    for ((c, lib, qui), m) in &retenues {
        ligne(c, lib, *m, zero, Some(qui), None);
    }
    for (qui, m) in &net_par_salarie {
        let aux = compte_de.get(qui).and_then(Option::as_ref);
        ligne(&plan.net.compte, &plan.net.libelle, zero, *m, Some(qui), aux);
    }

    let d = out.iter().fold(zero, |a, e| a + e.debit);
    let c = out.iter().fold(zero, |a, e| a + e.credit);
    if d != c {
        // Par construction brut + charges = tiers + prelevement + retenues + net ; si ca ne tombe pas, une ligne du bulletin
        // manque au plan ou a change de nature. On ne livre pas une ecriture desequilibree.
        return Err(refus(format!(
            "L'écriture de {} n'est pas équilibrée (débit {}, crédit {}) : une ligne de bulletin n'est pas portée comme il faut par le plan comptable.",
            lib_mois, d, c
        )));
    }
    Ok(out)
}

/// Un champ qui porte le delimiteur, un guillemet ou un
/// saut de ligne se met entre guillemets, les guillemets doublés.
fn champ_csv(t: &str) -> String {
    if t.contains([';', '"', '\r', '\n']) {
        format!("\"{}\"", t.replace('"', "\"\""))
    } else {
        t.to_string()
    }
}

/// Point-virgule, virgule decimale, UTF-8 avec BOM ; une case vide ne porte rien.
pub fn csv_compta(ecritures: &[Ecriture]) -> Vec<u8> {
    let mut t = String::from("\u{feff}");
    let mut rang = |champs: &[Option<String>]| {
        t.push_str(&champs.iter().map(|c| c.as_deref().map(champ_csv).into_iter().collect::<String>()).collect::<Vec<_>>().join(";"));
        t.push_str("\r\n");
    };
    rang(&["Journal", "Date", "Compte", "Libelle", "Debit", "Credit", "Analytique"].map(|c| Some(c.to_string())));
    for e in ecritures {
        let date = e.date.jj_mm_aaaa();
        let compte = match &e.auxiliaire {
            Some(a) => a.to_string(),
            None => e.compte.to_string(),
        };
        let montant = |m: Decimal| (!m.is_zero()).then(|| m.to_string().replace('.', ","));
        rang(&[Some(e.journal.to_string()), Some(date), Some(compte), Some(e.libelle.clone()), montant(e.debit), montant(e.credit), e.analytique.clone()]);
    }
    t.into_bytes()
}
