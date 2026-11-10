//! Les ecritures comptables d'un mois : le journal de paie en ecritures (`ecritures_du_mois`, le plan comptable livre
//! recouvert par `config/plan-comptable.json`) et leur CSV (`Compta_AAAA.csv`) : point-virgule, virgule decimale,
//! UTF-8 avec BOM, fins de ligne CRLF. Tout part des paies du mois (`engagement::etats::paies_du_mois` : ecrites ou importees).

use droit::fiche::Lue as _;
use std::collections::BTreeMap;

use droit::catalogue::{self, HorsAssiette};
use droit::contexte::Contexte;
use socle::echec::{erreur, Classe, Echec, R};
use socle::num::{eur2, zero2, Decimal};

use crate::plan_comptable::{compte_du_salarie, Plan, Refus};

fn refus(m: String) -> Echec {
    erreur(Classe::Refuse, m)
}

/// Une ligne du journal de paie : date, journal, compte, libelle, debit, credit, analytique, auxiliaire.
pub struct Ecriture {
    pub journal: String,
    pub date: String,
    pub compte: String,
    pub libelle: String,
    pub debit: Decimal,
    pub credit: Decimal,
    /// Le spectacle (le brut, les frais) ou le salarie (le net, une retenue) que la ligne porte ; aucun pour une charge,
    /// un tiers ou le prelevement a la source.
    pub analytique: Option<String>,
    pub mois: String,
    /// Le sous-compte individuel du salarie, quand le plan en tient un (`net.par_salarie`).
    pub auxiliaire: Option<String>,
    /// Le libelle du sous-compte : le salarie.
    pub auxiliaire_libelle: Option<String>,
    /// Posee par les etats des ecritures comptables (« PAIE-AAAA-MM »).
    pub piece: String,
    /// Le rang de l'ecriture (une par mois) dans l'etat, a partir de 1.
    pub numero: i64,
}

/// La date de l'ecriture d'un mois (son dernier jour, AAAA-MM-JJ) et son libelle (« Decembre 2026 »).
fn date_et_libelle(mois: &str) -> R<(String, String)> {
    let mois = socle::mois::Mois::lire(mois).map_err(|e| Echec { classe: Some(Classe::Illisible), ..e })?;
    Ok((mois.dernier_jour().iso(), mois.libelle(socle::mois::Graphie::Titre)))
}

/// Une ecriture pour le mois, depuis les paies validees.
/// Le brut par spectacle, la charge patronale et le tiers par ligne de bulletin, le prelevement a la source retenu
/// (une dette envers l'Etat, au compte `pas` du plan), le net par salarie -- le net PAYE, apres l'impot, celui du
/// bulletin. Un identifiant de ligne absent du plan fait refuser : on ne devine pas un compte.
pub fn ecritures_du_mois(ctx: &Contexte, mois: &str, plan: &Plan, departs: &engagement::departs::Departs) -> R<Vec<Ecriture>> {
    let paies = engagement::etats::paies_du_mois(ctx, mois, departs).map_err(|m| Echec { classe: Some(Classe::Refuse), ..m })?;
    let (date, lib_mois) = date_et_libelle(mois)?;
    let jrn = plan.journal.clone();
    let zero = zero2();

    let mut brut_par_spectacle: BTreeMap<String, Decimal> = BTreeMap::new();
    let mut net_par_salarie: BTreeMap<String, Decimal> = BTreeMap::new();
    let mut compte_de: BTreeMap<String, Option<String>> = BTreeMap::new();
    let mut frais_par_spectacle: BTreeMap<String, Decimal> = BTreeMap::new();
    let mut charge: BTreeMap<(String, String), Decimal> = BTreeMap::new();
    let mut tiers: BTreeMap<String, Decimal> = BTreeMap::new();
    // LE PRELEVEMENT A LA SOURCE : celui du bulletin (`prelevement::des_paies`, la meme lecture, la meme position), retenu sur
    // le net et du a la DGFiP (compte 4421 « Prelevements a la source (Impot sur le revenu) » du plan
    // comptable general, atelier/reference/sources/plan-comptable-general/LISEZ-MOI.md), tel que la paie le porte. Le PAS d'un
    // mois declare est celui de la paie declaree (`engagement::etats::paie_declaree`), lu et non recalcule au taux du
    // jour (CGI art. 1671 : `prelevement::des_paies`).
    let pas_des_paies = crate::prelevement::des_paies(ctx, &paies, departs).map_err(|m| Echec { classe: Some(Classe::Refuse), ..m })?;
    let mut pas_du_mois = zero;
    // LES RETENUES (une ligne hors assiette de nature `retenue` : acompte, avance, saisie) ne sont pas une charge : un
    // acompte solde une avance deja versee (425 « Personnel - Avances et acomptes et autres comptes debiteurs »), une
    // saisie est due a un tiers (427 « Personnel - Oppositions »), plan de comptes de l'ANC (meme LISEZ-MOI). Le plan
    // dit le compte de chaque rubrique (`retenues`) ; une retenue qu'il ne nomme pas fait refuser. Par (compte,
    // libelle, salarie).
    let mut retenues: BTreeMap<(String, String, String), Decimal> = BTreeMap::new();
    let ajouter = |m: &mut BTreeMap<String, Decimal>, k: String, v: Decimal| {
        let e = m.entry(k).or_insert(zero);
        *e += v;
    };
    for (p, pas) in paies.iter().zip(&pas_des_paies) {
        // UN PERMANENT N'A PAS DE SPECTACLE : son brut va a sa propre section.
        let titre = match p.spectacle.as_ref().and_then(|s| s.titre.dite()).filter(|t| !t.is_empty()) {
            Some(t) => t.clone(),
            None => match p.engagement.spectacle.as_deref().filter(|s| !s.is_empty()) {
                Some(s) => s.to_string(),
                None => "Salariés au mois".to_string(),
            },
        };
        let brut = eur2(p.brut);
        ajouter(&mut brut_par_spectacle, titre.clone(), brut);
        let qui = crate::tableaux_export::qui(&p.salarie)?;
        let mut retenu = zero;
        for l in p.remuneration.iter().filter(|l| !l.soumis) {
            if catalogue::rubrique(&ctx.racine, &l.code, &ctx.lue_au).and_then(|r| r.hors_assiette()).map_err(|m| Echec { classe: Some(Classe::Refuse), ..m })? != HorsAssiette::Mouvement {
                continue;
            }
            let Some(regle) = plan.retenues.get(&l.code) else {
                return Err(refus(format!(
                    "La retenue « {} » ({}) n'a pas de compte dans le plan comptable. L'ajouter dans config/plan-comptable.json sous « retenues » : un compte 425 pour un acompte ou une avance, 427 pour une saisie ou une opposition.",
                    l.code, l.libelle
                )));
            };
            // Le libelle du plan, sinon celui de la rubrique.
            let libelle = match &regle.libelle {
                Some(x) => x.clone(),
                None => l.libelle.clone(),
            };
            *retenues.entry((regle.compte.clone(), libelle, qui.clone())).or_insert(zero) += eur2(l.montant);
            retenu += eur2(l.montant);
        }
        // CE QUE LE NET PORTE EN PLUS DU BRUT : les indemnites non soumises (repas, defraiements) sont payees
        // sans etre du brut ; sans leur ligne au debit, l'ecriture ne s'equilibrerait pas. Les retenues en sont
        // otees : elles ont leur compte.
        let hors_brut = eur2(p.net_a_payer) - (brut - eur2(p.total_sal)) - retenu;
        if !hors_brut.is_zero() {
            ajouter(&mut frais_par_spectacle, titre.clone(), hors_brut);
        }
        // Le net paye est le net a payer moins l'impot retenu (le « MONTANT NET À PAYER » du bulletin) ; l'impot va
        // a l'Etat. Une paie sans prelevement ne retient rien.
        let mut net = eur2(p.net_a_payer);
        if let Some(v) = pas {
            let prelevement = eur2(v.prelevement);
            pas_du_mois += prelevement;
            net -= prelevement;
        }
        ajouter(&mut net_par_salarie, qui.clone(), net);
        let cle_salarie = p.salarie.id.to_string();
        let compte = compte_du_salarie(plan, &p.salarie, &cle_salarie).map_err(|r| match r {
            Refus::Operation(m) => refus(m),
            Refus::Autre(e) => e,
        })?;
        compte_de.insert(qui, compte);
        for l in &p.lignes {
            let Some(id) = l.id.as_deref() else {
                return Err(refus(format!("La ligne de bulletin « {} » ne dit pas sa cotisation : elle n'a pas de compte dans le plan comptable.", l.libelle)));
            };
            let Some(regle) = plan.lignes.get(id) else {
                return Err(refus(format!(
                    "La ligne de bulletin « {} » ({}) n'a pas de compte dans le plan comptable. L'ajouter dans config/plan-comptable.json sous « lignes », avec un compte de charge et un compte de tiers.",
                    id, l.libelle
                )));
            };
            // Le libelle du plan, sinon celui du bulletin.
            let libelle = match &regle.libelle {
                Some(x) => x.clone(),
                None => l.libelle.clone(),
            };
            if id == "reduction_generale" {
                if let Some(rg) = &p.reduction_generale {
                    // La RGDU : une charge negative, ventilee au tiers de chaque part telle que la paie l'a calculee.
                    *charge.entry((regle.charge.clone(), libelle)).or_insert(zero) += eur2(l.montant_pat);
                    for (part, id_tiers) in [(rg.urssaf, "maladie"), (rg.chomage, "chomage"), (rg.agirc_arrco, "retraite_t1")] {
                        if !part.is_zero() {
                            let r = plan.lignes.get(id_tiers).ok_or_else(|| Echec { classe: Some(Classe::Manque), ..socle::fichiers::absent(id_tiers) })?;
                            *tiers.entry(r.tiers.clone()).or_insert(zero) -= part;
                        }
                    }
                    continue;
                }
            }
            let (pat, sal) = (eur2(l.montant_pat), eur2(l.montant_sal));
            if !pat.is_zero() {
                *charge.entry((regle.charge.clone(), libelle)).or_insert(zero) += pat;
            }
            if !pat.is_zero() || !sal.is_zero() {
                *tiers.entry(regle.tiers.clone()).or_insert(zero) += pat + sal;
            }
        }
    }

    let mut out: Vec<Ecriture> = Vec::new();
    let mut ligne = |compte: &str, libelle: &str, debit: Decimal, credit: Decimal, analytique: Option<&str>, auxiliaire: Option<&str>| {
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
            date: date.clone(),
            compte: compte.to_string(),
            libelle: format!("{} -- {}", libelle, lib_mois),
            debit: eur2(debit),
            credit: eur2(credit),
            analytique: analytique.map(str::to_string),
            mois: mois.to_string(),
            auxiliaire: auxiliaire.map(str::to_string),
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
        // Un modele qui se remplit a vide ne fait pas de sous-compte.
        let aux = compte_de.get(qui).and_then(|c| c.as_deref()).filter(|c| !c.is_empty());
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
        let date: String = format!("{}/{}/{}", e.date.chars().skip(8).take(2).collect::<String>(), e.date.chars().skip(5).take(2).collect::<String>(), e.date.chars().take(4).collect::<String>());
        let compte = match &e.auxiliaire {
            Some(a) => a.clone(),
            None => e.compte.clone(),
        };
        let montant = |m: Decimal| (!m.is_zero()).then(|| m.to_string().replace('.', ","));
        rang(&[Some(e.journal.clone()), Some(date), Some(compte), Some(e.libelle.clone()), montant(e.debit), montant(e.credit), e.analytique.clone()]);
    }
    t.into_bytes()
}
