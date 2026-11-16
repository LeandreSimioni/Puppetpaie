//! Les etats des cotisations et des organismes : les cotisations par organisme,
//! les versements aux organismes (bloc 20), le bordereau Urssaf (blocs 22 et 23), le FCAP-SVP de la 3090, les
//! contributions Afdas, le paritarisme de branche.
//!
//! L'ARGENT QUI A BOUGE se tire de chaque DSN telle qu'elle est partie (`depart::dsn_du_mois` : ses paies et les
//! corrections d'autres mois qu'elle portait, des versions qu'elle a emportees, par la fonction qui l'a fabriquee), ou
//! telle qu'elle partirait : les versements (bloc 20) et le bordereau Urssaf (blocs 22 et 23). CE QUE LE MOIS COUTE se
//! tire des paies de chaque mois (`engagement::etats::paies_du_mois`, ecrites ou importees) : la tranche A
//! (`dsn::base_declaree`).
//! Les montants annuels hors DSN (FCAP-SVP, Afdas, paritarisme) se calculent sur le brut du livre de paie ou sur la
//! tranche A declaree, avec les taux du bareme.

use documents::plan_comptable::plan;
use droit::fiche::Lue as _;
use droit::fiche::{Idcc, IDCC_1790, IDCC_2642, IDCC_3097};
use std::collections::{BTreeMap, BTreeSet};

use droit::contexte::Contexte;
use droit::forme_bareme::{CHOMAGE, MALADIE, RETRAITE_T1};
use engagement::calcul::BaseDeCotisation;
use engagement::ecarts::RegroupementDeComparaison;
use droit::organismes::QualifiantDAssiette;
use socle::echec::{erreur, refus_avec, Lieu, Classe};
use socle::mois::Annee;
use socle::mois::{Graphie, Mois};
use documents::etats_paie::{finir, lignes_livre, Demande, Lecture, Periode};
use documents::plan_comptable::{EtatDExport, RegroupementDEtat};
use socle::grandeurs::{Montant, Taux};
use documents::tableaux_export::{libelle_mois, qui, refus, section_du_mois, sections_de, ColonneDEtat as C, Ligne, Section, Tableau, R};

// ------------------------------------------------------------------ outils

/// `min(a, b)` : le premier des deux quand ils sont egaux.
fn plus_petit<G: PartialOrd>(a: G, b: G) -> G {
    if b < a {
        b
    } else {
        a
    }
}

/// `max(a, b)` : le premier des deux quand ils sont egaux.
fn plus_grand<G: PartialOrd>(a: G, b: G) -> G {
    if b > a {
        b
    } else {
        a
    }
}


/// Identifiant de ligne -> nom de l'organisme, lu dans le plan comptable (le meme endroit que
/// l'export compta : l'etat des cotisations et le credit des comptes 43 ne peuvent pas diverger).
fn organismes(ctx: &Contexte) -> R<Vec<(RegroupementDeComparaison, String)>> {
    let (plan, _) = plan(ctx)?;
    // Un tiers que le plan ne nomme pas se dit par son compte.
    let nom = |v: &documents::plan_comptable::RegleLigne| match plan.nom_du_tiers(&v.tiers) {
        Some(n) => n.to_string(),
        None => format!("Organisme {}", v.tiers),
    };
    Ok(plan
        .lignes
        .iter()
        .map(|(k, v)| (RegroupementDeComparaison::Cotisation(k.clone()), nom(v)))
        .chain(plan.regroupements.iter().map(|(k, v)| (k.clone(), nom(v))))
        .collect())
}

/// Ce que la convention `idcc` fait payer hors du bulletin (`hors_bulletin`), dans le socle livre de l'annee `an`, a
/// defaut dans le plus recent livre.
fn hors_bulletin(ctx: &Contexte, an: Annee, idcc: &Idcc) -> R<Option<droit::forme_bareme::HorsBulletin>> {
    let socle = match droit::contexte::socle_livre_lu(an).map_err(|e| e.reclasser(Classe::Disque))? {
        Some(s) => s,
        None => {
            let derniere = droit::contexte::annees_du_socle(&ctx.code.join("baremes"))?
                .into_iter()
                .max()
                .ok_or_else(|| erreur(Classe::Illisible, "Aucun barème n'est livré avec le logiciel : le signaler au développeur."))?;
            match droit::contexte::socle_livre_lu(derniere).map_err(|e| e.reclasser(Classe::Disque))? {
                Some(s) => s,
                None => return Ok(None),
            }
        }
    };
    Ok(socle.par_convention.get(idcc).and_then(|c| c.hors_bulletin.clone()))
}

/// L'IDCC de la fiche entreprise, que chacun de ces etats exige.
fn idcc_de(ctx: &Contexte) -> R<Idcc> {
    ctx.entreprise.idcc.exigee("idcc").cloned().map_err(|e| e.reclasser(Classe::Refuse))
}

// ===================================== 6. les cotisations par organisme

/// Une ligne de cumul : un organisme, une cotisation, ses taux -- dans l'ordre ou on l'a vue.
struct Cumul {
    cle: (Option<Mois>, String, String, String, Option<Taux>, Option<Taux>),
    mois: Mois,
    organisme: String,
    code: String,
    libelle: String,
    taux_sal: Option<Taux>,
    taux_pat: Option<Taux>,
    /// La somme des bases : aucune tant qu'aucune ligne n'en porte.
    base: Option<BaseDeCotisation>,
    part_sal: Montant,
    part_pat: Montant,
}

/// Ce qui est du a chaque organisme, rubrique par rubrique.
pub fn cotisations_par_organisme(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let regroupement = match d.regroupement {
        Some(r @ (RegroupementDEtat::Organisme | RegroupementDEtat::Mois)) => r,
        autre => {
            let ecrit = match autre {
                Some(r) => r.as_str(),
                None => "",
            };
            return Err(refus(format!("Regroupement inconnu pour les cotisations : « {} » (organisme ou mois).", ecrit)));
        }
    };
    let par_mois = regroupement == RegroupementDEtat::Mois;
    let p = Periode::de(ctx, d, true)?;
    let mut avert: Vec<String> = Vec::new();
    let orgs = organismes(ctx)?;
    let org_de = |id: &RegroupementDeComparaison| orgs.iter().find(|(k, _)| k == id).map(|(_, v)| v.clone());
    let mut lecture = Lecture::nouvelle(ctx, &d.departs);
    let mut cumul: Vec<Cumul> = Vec::new();
    let mut manque: BTreeSet<String> = BTreeSet::new();
    let mut ajouter = |m: Mois, org: &str, code: &str, libelle: &str, base: Option<BaseDeCotisation>, ts: Option<Taux>, ms: Option<Montant>, tp: Option<Taux>, mp: Option<Montant>| -> R<()> {
        let cle = (par_mois.then_some(m), org.to_string(), code.to_string(), libelle.to_string(), ts, tp);
        let i = match cumul.iter().position(|c| c.cle == cle) {
            Some(i) => i,
            None => {
                cumul.push(Cumul {
                    cle,
                    mois: m,
                    organisme: org.to_string(),
                    code: code.to_string(),
                    libelle: libelle.to_string(),
                    taux_sal: ts,
                    taux_pat: tp,
                    base: None,
                    part_sal: Montant::ZERO,
                    part_pat: Montant::ZERO,
                });
                cumul.len() - 1
            }
        };
        let c = &mut cumul[i];
        if let Some(b) = base.filter(|b| !b.est_nulle()) {
            c.base = Some(match c.base {
                Some(deja) => deja.plus(b)?,
                None => b,
            });
        }
        // Une part que la ligne ne porte pas (la colonne qu'un bulletin importe laisse vide) n'ajoute rien.
        if let Some(ms) = ms.filter(|x| !x.est_nul()) {
            c.part_sal += ms;
        }
        if let Some(mp) = mp.filter(|x| !x.est_nul()) {
            c.part_pat += mp;
        }
        Ok(())
    };
    // CE QUE LES PIECES IMPORTEES NE VENTILENT PAS : une paie reprise de la seule DSN (ses lignes de cotisation sont sur
    // un bulletin qu'on n'a pas), et une ligne de bulletin que la lecture de l'import ne rattache a aucune cotisation
    // (`importe::Importe::paie` : elle garde son libelle, sans cotisation). Son organisme ne se devine pas (regle 1) ;
    // l'etat dit ce qui n'y est pas, le plan comptable n'y est pour rien.
    let mut sans_bulletin: BTreeSet<Mois> = BTreeSet::new();
    let mut non_rattachees: BTreeMap<String, (BTreeSet<Mois>, Montant)> = BTreeMap::new();
    for m in p.mois.clone() {
        for paie in lecture.paies(m)?.iter() {
            let cotisations = paie.cotisations();
            if paie.importee().is_some() && cotisations.is_empty() {
                sans_bulletin.insert(m);
            }
            for c in &cotisations {
                let Some(cle) = c.id.as_ref() else {
                    if paie.importee().is_some() {
                        let (mois, total) = non_rattachees.entry(c.libelle.clone()).or_insert_with(|| (BTreeSet::new(), Montant::ZERO));
                        mois.insert(m);
                        // Les parts que la ligne porte ; la colonne qu'un bulletin importe laisse vide n'en porte pas.
                        *total = [c.montant_sal, c.montant_pat].into_iter().flatten().map(Montant::au_centime).fold(*total, |a, x| a + x);
                    } else {
                        manque.insert(format!("sans cotisation ({})", c.libelle));
                    }
                    continue;
                };
                let id = cle.as_str();
                let Some(org) = org_de(cle) else {
                    manque.insert(format!("{} ({})", id, c.libelle));
                    continue;
                };
                let (ts, tp) = (c.taux_sal, c.taux_pat);
                if *cle == RegroupementDeComparaison::Cotisation(droit::forme_bareme::REDUCTION_GENERALE) {
                    if let Some(rg) = paie.validee().and_then(|v| v.reduction_generale.as_ref()) {
                        let mut reparti = Montant::ZERO;
                        for (part, id_org, quoi) in [(rg.urssaf, MALADIE, "part Urssaf"), (rg.chomage, CHOMAGE, "part chômage"), (rg.agirc_arrco, RETRAITE_T1, "part retraite")] {
                            if !part.est_nul() {
                                let o = org_de(&RegroupementDeComparaison::Cotisation(id_org.clone())).ok_or_else(|| socle::fichiers::absent(id_org.as_str()).reclasser(Classe::Manque))?;
                                ajouter(m, &o, id, &format!("{} — {}", c.libelle, quoi), None, None, None, None, Some(-part))?;
                                reparti += part;
                            }
                        }
                        let calculee = c.montant_pat.ok_or_else(|| socle::fichiers::absent("montant_pat").reclasser(Classe::Illisible))?;
                        if -reparti != calculee {
                            avert.push(format!(
                                "{}, {} : la réduction générale ({}) ne tombe pas sur la somme de ses parts ({}).",
                                libelle_mois(m),
                                qui(paie.salarie())?,
                                socle::grandeurs::ecrit::texte::dit(calculee.au_centime()),
                                socle::grandeurs::ecrit::texte::dit(-reparti)
                            ));
                        }
                        continue;
                    }
                }
                ajouter(m, &org, id, &c.libelle, c.base.map(BaseDeCotisation::au_centime), ts, c.montant_sal.map(Montant::au_centime), tp, c.montant_pat.map(Montant::au_centime))?;
            }
        }
    }
    if !manque.is_empty() {
        return Err(refus_avec(
            format!(
                "Lignes de bulletin sans organisme dans le plan comptable : {}. Les y ajouter parmi les lignes (une cotisation) ou les regroupements (autres, convention, apec) : on ne devine pas à qui une cotisation est due.",
                manque.into_iter().collect::<Vec<_>>().join(", ")
            ),
            Lieu { fichier: Some(documents::plan_comptable::PLAN_COMPTABLE), cle: Some("lignes".to_string()), ..Default::default() },
        )
        .reclasser(Classe::Refuse));
    }
    let mois_dits = |s: &BTreeSet<Mois>| s.iter().map(|m| m.libelle(Graphie::Accentuee)).collect::<Vec<_>>().join(", ");
    if !sans_bulletin.is_empty() {
        avert.push(format!("Paies reprises de la seule DSN, sans leur bulletin, hors de l'état : {}.", mois_dits(&sans_bulletin)));
    }
    for (libelle, (mois, total)) in &non_rattachees {
        avert.push(format!("Ligne « {} » des bulletins repris ({}) : la lecture de l'import ne la rattache à aucune cotisation, hors de l'état ({}).", libelle, mois_dits(mois), socle::texte::euros(*total)));
    }
    let mut lignes = Vec::new();
    for c in cumul {
        let mut l = Ligne::nouvelle();
        l.m(C::Mois, c.mois).t(C::Organisme, c.organisme).t(C::Code, c.code).t(C::Libelle, c.libelle).taux(C::TauxSal, c.taux_sal).taux(C::TauxPat, c.taux_pat);
        l.base(C::Base, c.base.filter(|b| !b.est_nulle()))
            .montant(C::PartSal, c.part_sal)
            .montant(C::PartPat, c.part_pat)
            .montant(C::Total, c.part_sal + c.part_pat);
        lignes.push(l);
    }
    let tri = |l: &Ligne| vec![l.texte(C::Code), l.texte(C::Libelle)];
    let mut t = Tableau::nouveau(EtatDExport::CotisationsParOrganisme, "Cotisations par organisme", p.libelle.clone());
    t.mois = p.mois.clone();
    t.sections = if par_mois {
        sections_de(lignes, |l| (l.mois(C::Mois), l.texte(C::Organisme)), |k| Ok(format!("{} — {}", section_du_mois(&k.0)?, k.1)), Some(&tri))?
    } else {
        sections_de(lignes, |l| l.texte(C::Organisme), |k| Ok(k.clone()), Some(&tri))?
    };
    // Par organisme, la section porte deja son nom : une colonne qui le repeterait ne dirait rien de plus.
    let mut cols = if par_mois { vec![C::Mois.colonne(), C::Organisme.colonne()] } else { Vec::new() };
    cols.extend([
        C::Libelle.titree("Cotisation"),
        C::Code.colonne(),
        // La base ne se totalise pas : deux assiettes differentes ne s'additionnent pas.
        C::Base.colonne(),
        C::TauxSal.colonne(),
        C::PartSal.colonne(),
        C::TauxPat.titree("Taux pat. %"),
        C::PartPat.colonne(),
        C::Total.colonne(),
    ]);
    t.colonnes = cols;
    t.sources = vec![
        "lignes de cotisation du bulletin ; organisme : compte de tiers du plan comptable (plan-comptable-base.json ou config/plan-comptable.json)".to_string(),
        "une base additionnée sur plusieurs bulletins est la somme des assiettes, pas une assiette déclarée".to_string(),
    ];
    avert.push("Les « autres contributions dues par l'employeur » (FNAL, CSA, dialogue social, formation) sont versées à l'Urssaf sans ventilation, comme le bulletin les imprime.".to_string());
    t.avertissements = avert;
    finir(ctx, &mut t, Some(&p), &format!("cotisations-par-organisme-{}", regroupement), d.format,d.ecrire)?;
    Ok(t)
}

// ===================================== 7. les versements aux organismes (bloc 20)

/// Les paies de chaque mois de la periode (`engagement::etats::paies_du_mois` : ecrites ou importees), tous
/// etablissements. Un mois sans paie est dit.
fn paies_des_mois(ctx: &Contexte, p: &Periode, avert: &mut Vec<String>, departs: &engagement::departs::Departs) -> R<Vec<(Mois, Vec<engagement::etats::PaieDuMois>)>> {
    let (mut out, mut sans) = (Vec::new(), Vec::new());
    for &m in &p.mois {
        let paies = engagement::etats::paies_du_mois(ctx, m, departs).map_err(|e| e.reclasser(Classe::Refuse))?;
        if paies.is_empty() {
            sans.push(m.to_string());
        } else {
            out.push((m, paies));
        }
    }
    if !sans.is_empty() {
        avert.push(format!("Aucune paie pour {} : ces mois ne sont pas dans l'état.", sans.join(", ")));
    }
    if out.is_empty() {
        return Err(refus(format!("Aucune paie sur la période ({}) : il n'y a rien à lire.", p.libelle)));
    }
    Ok(out)
}

/// La DSN d'un mois et d'un etablissement (`None` : le principal) : celle qui est partie, telle qu'elle est partie, ou
/// celle qui partirait, et si elle est partie. La fournit qui lit le journal des envois (`routes::gestes::depart::dsn_du_mois`).
pub type DsnDuMois<'a> = &'a dyn Fn(&Contexte, Mois, Option<&droit::fiche::Nic>) -> socle::echec::R<Option<(crate::dsn::Fabriquee, bool)>>;

/// LA DSN DE CHAQUE MOIS DE LA PERIODE, par etablissement (le principal d'abord) : celle qui est partie telle qu'elle est
/// partie, ou celle qui partirait (`dsn_du_mois`) : `(mois, nic, la DSN, partie)`. Un mois sans DSN est dit.
fn dsn_des_mois(ctx: &Contexte, p: &Periode, avert: &mut Vec<String>, dsn_du_mois: DsnDuMois) -> R<Vec<(Mois, droit::fiche::Nic, crate::dsn::Fabriquee, bool)>> {
    let (mut out, mut sans) = (Vec::new(), Vec::new());
    // UN MOIS REPRIS D'UN ANCIEN LOGICIEL n'a pas de DSN de ce logiciel : ses versements se composent sur les bases du
    // calcul de chaque paie (`engagement::calcul`, les bases globales : assiettes majorees de 15 et 10 %, plafonnees,
    // abattues), que ses bulletins n'impriment pas.
    let importes = p.importes();
    if !importes.is_empty() {
        let quand = if importes.len() == p.mois.len() { format!("Les paies de {}", p.libelle) } else { format!("Les paies de {}", importes.iter().map(|m| m.libelle(Graphie::Accentuee)).collect::<Vec<_>>().join(", ")) };
        avert.push(format!(
            "{} sont reprises d'un ancien logiciel : leurs versements ne se composent pas ici. La DSN se compose sur les assiettes que ce logiciel calcule (majorées, plafonnées), et leurs bulletins ne les impriment pas.",
            quand
        ));
    }
    let principal = ctx.entreprise.nic_principal();
    for &m in p.mois.iter().filter(|m| !importes.contains(m)) {
        let mut une = false;
        for nic in ctx.entreprise.nics().map_err(|e| e.reclasser(Classe::Refuse))? {
            let etablissement = if Some(&nic) == principal { None } else { Some(&nic) };
            if let Some((f, partie)) = dsn_du_mois(ctx, m, etablissement).map_err(|e| e.reclasser(Classe::Refuse))? {
                out.push((m, nic, f, partie));
                une = true;
            }
        }
        if !une {
            sans.push(m.to_string());
        }
    }
    if !sans.is_empty() {
        avert.push(format!("Aucune DSN partie ni paie validée pour {} : ces mois ne sont pas dans l'état.", sans.join(", ")));
    }
    if out.is_empty() {
        // Seuls des mois repris : la raison qu'ils n'y sont pas est la reponse.
        if sans.is_empty() {
            if let Some(repris) = avert.pop() {
                return Err(refus(repris));
            }
        }
        return Err(refus(format!("Aucune DSN partie ni paie validée sur la période ({}) : il n'y a rien à lire.", p.libelle)));
    }
    Ok(out)
}

/// Le nom de la DSN d'un mois dans un etat : partie, ou a deposer.
fn nom_de_la_dsn(m: Mois, partie: bool) -> String {
    format!("DSN de {}{}", libelle_mois(m), if partie { "" } else { " (à déposer)" })
}

/// SIRET -> libelle, lu dans les organismes de la compagnie (`Contexte::organismes`).
fn libelles_organismes(ctx: &Contexte) -> R<Vec<(droit::organismes::IdentifiantDOrganisme, String)>> {
    let mut out: Vec<(droit::organismes::IdentifiantDOrganisme, String)> = Vec::new();
    for v in ctx.organismes()?.versements() {
        match out.iter_mut().find(|(c, _)| *c == v.siret) {
            Some(x) => x.1 = v.libelle.clone(),
            None => out.push((v.siret.clone(), v.libelle.clone())),
        }
    }
    Ok(out)
}

/// Chaque versement que la DSN de chaque mois ordonne (bloc S21.G00.20), partie
/// ou a deposer (`dsn_des_mois`).
pub fn versements_organismes(ctx: &Contexte, d: &Demande, dsn_du_mois: DsnDuMois) -> R<Tableau> {
    let p = Periode::de(ctx, d, true)?;
    let mut avert = Vec::new();
    let noms = libelles_organismes(ctx)?;
    let mut lignes = Vec::new();
    for (m, _nic, f, partie) in dsn_des_mois(ctx, &p, &mut avert, dsn_du_mois)? {
        for o in &f.ordres {
            let organisme = match noms.iter().find(|(k, _)| *k == o.organisme).map(|(_, v)| v.clone()).filter(|v| !v.is_empty()) {
                Some(n) => n,
                None => o.organisme.to_string(),
            };
            let mut l = Ligne::nouvelle();
            l.m(C::Mois, m).t(C::Organisme, organisme).t(C::Identifiant, o.organisme.to_string()).ot(C::Reference, o.reference.clone());
            l.t(C::Du, o.periode.debut().jj_mm_aaaa()).t(C::Au, o.periode.fin().jj_mm_aaaa()).montant(C::Montant, o.montant);
            l.t(C::Mode, "prélèvement SEPA");
            l.ot(C::Date, o.date_paiement.map(|j| j.jj_mm_aaaa())).t(C::Fichier, nom_de_la_dsn(m, partie));
            lignes.push(l);
        }
    }
    if lignes.iter().any(|l| l.texte(C::Date).is_empty()) {
        avert.push("Un versement sans date de paiement est prélevé à l'échéance légale de l'organisme.".to_string());
    }
    let mut t = Tableau::nouveau(EtatDExport::VersementsOrganismes, "Versements aux organismes (DSN)", p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    t.sections = sections_de(lignes, |l| l.mois(C::Mois), section_du_mois, None)?;
    t.colonnes = vec![
        C::Mois.colonne(),
        C::Organisme.colonne(),
        C::Identifiant.colonne(),
        C::Reference.colonne(),
        C::Du.colonne(),
        C::Au.colonne(),
        C::Montant.colonne(),
        C::Mode.colonne(),
        C::Date.titree("Date de paiement"),
        C::Fichier.titree("DSN"),
    ];
    t.sources = vec![
        "bloc S21.G00.20 « Versement organisme de protection sociale » (cahier technique DSN 2026.1), tiré de chaque DSN partie (des versions qu'elle a emportées) ou à déposer, par la fonction qui écrit la DSN ; libellés : donnees/organismes-base.json".to_string(),
    ];
    finir(ctx, &mut t, Some(&p), "versements-organismes", d.format,d.ecrire)?;
    Ok(t)
}

// ===================================== 8. le bordereau Urssaf (blocs 22 et 23)

/// Le bordereau Urssaf de chaque mois, CTP par CTP, tel que la DSN le porte, partie ou a deposer (`dsn_des_mois`), avec ceux des mois que ses corrections regularisent. Les taux ne sont
/// pas appliques ici : c'est l'Urssaf qui calcule depuis les assiettes, et un export ne calcule pas.
pub fn bordereau_urssaf(ctx: &Contexte, d: &Demande, dsn_du_mois: DsnDuMois) -> R<Tableau> {
    let p = Periode::de(ctx, d, true)?;
    let mut avert = Vec::new();
    let mut lignes = Vec::new();
    for (m, _nic, f, partie) in dsn_des_mois(ctx, &p, &mut avert, dsn_du_mois)? {
        for bordereau in f.bordereaux {
            let mut l = Ligne::nouvelle();
            l.m(C::Mois, m).t(C::Ctp, "").t(C::Libelle, format!("Total dû à l'Urssaf (bloc 22), {}", nom_de_la_dsn(m, partie)));
            l.t(C::Periode, format!("{} au {}", bordereau.periode.debut().jj_mm_aaaa(), bordereau.periode.fin().jj_mm_aaaa())).montant(C::Total, bordereau.total);
            lignes.push(l);
            for b in bordereau.lignes {
                let mut l = Ligne::nouvelle();
                // Le libelle, dans la table des CTP que lit le bordereau (`documents::bordereau`, sa seule lecture).
                l.m(C::Mois, m).t(C::Ctp, b.code.as_str()).ot(C::Libelle, documents::bordereau::libelle_du_ctp(&b.code)?);
                l.t(C::Qualifiant, match b.qualifiant {
                    // Les libelles de la norme (S21.G00.23.002, `bloc-S21.G00.23.txt` l. 35-36).
                    QualifiantDAssiette::AutreAssiette => "920 — autre assiette",
                    QualifiantDAssiette::AssiettePlafonnee => "921 — assiette plafonnée",
                });
                let taux_at = b.taux_at.map(documents::bordereau::TauxDeLaLigne::taux);
                l.taux(C::TauxAt, taux_at.map(|t| socle::grandeurs::decimales_ecrites(t, 2))).om(C::AssietteDeclaree, if b.montant.is_some() { None } else { b.assiette }).om(C::Montant, b.montant);
                lignes.push(l);
            }
        }
    }
    let mut t = Tableau::nouveau(EtatDExport::BordereauUrssaf, "Bordereau Urssaf (DSN)", p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    t.sections = sections_de(lignes, |l| l.mois(C::Mois), section_du_mois, None)?;
    t.colonnes = vec![
        C::Mois.colonne(),
        C::Ctp.colonne(),
        C::Libelle.colonne(),
        C::Qualifiant.colonne(),
        C::TauxAt.colonne(),
        C::AssietteDeclaree.colonne(),
        C::Montant.titree("Montant forfaitaire"),
        C::Periode.colonne(),
        C::Total.colonne(),
    ];
    t.sources = vec![
        "blocs S21.G00.22 « Bordereau de cotisation due » et S21.G00.23 « Cotisation agrégée » (cahier technique DSN 2026.1), tirés de chaque DSN partie (des versions qu'elle a emportées) ou à déposer, par la fonction qui écrit la DSN".to_string(),
        "libellés : table des codes types de personnel, open.urssaf.fr (histocodestypescsv), relevée le 18/09/2026".to_string(),
    ];
    finir(ctx, &mut t, Some(&p), "bordereau-urssaf", d.format,d.ecrire)?;
    Ok(t)
}

// ================================================== le FCAP-SVP (3090, titre V)

fn refuse_un_mois(quoi: &str, d: &Demande) -> R<()> {
    if d.mois.is_some() {
        return Err(refus(quoi));
    }
    Ok(())
}

/// 0,1 % de la tranche A de l'annee, plancher 80 EUR, plafond 300 EUR, paye HORS
/// DSN en debut d'annee. La tranche A est la base 02 (S21.G00.78) de chaque paie du mois, ecrite ou importee
/// (`dsn::base_declaree`) ; une annee que le logiciel ne tient pas depuis janvier se dit partielle.
pub fn fcap_svp(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    refuse_un_mois("Le FCAP-SVP est une contribution annuelle : il se demande pour une année, ou pour toutes les années.", d)?;
    let idcc = idcc_de(ctx)?;
    let Some(fp) = droit::bareme::prevoyance::financement(&idcc).map_err(|e| e.reclasser(Classe::Refuse))? else {
        return Err(refus(format!(
            "Le FCAP-SVP est la contribution de la convention 3090 (titre V) ; la fiche entreprise dit IDCC {}. Il n'y a rien à calculer.",
            idcc
        )));
    };
    let p = Periode::nouvelle(ctx, None, d.annee, d.toutes, true)?;
    let mut avert = Vec::new();
    let (taux, plancher, plafond) = (fp.taux, fp.plancher, fp.plafond);
    let mut par_an: Vec<(Annee, usize, Montant)> = Vec::new();
    let orgs = ctx.organismes().map_err(|e| e.reclasser(Classe::Refuse))?;
    for (m, paies) in paies_des_mois(ctx, &p, &mut avert, &d.departs)? {
        // La tranche A : la base 02 (« assiette brute plafonnee ») de chaque paie (`dsn::base_declaree`).
        let mut ta = Montant::ZERO;
        for q in &paies {
            let nic = droit::bareme::etablissement_de(&ctx.entreprise, q.salarie()).map_err(|e| e.reclasser(Classe::Refuse))?;
            let bar = crate::dsn::bar_du(&ctx.du_mois(m).map_err(|e| e.reclasser(Classe::Refuse))?, &ctx.entreprise, &orgs, &nic, m).map_err(|e| e.reclasser(Classe::Refuse))?;
            let plafonnee = droit::base_assujettie::BaseAssujettie::BrutePlafonnee;
            match q {
                engagement::etats::PaieDuMois::Validee(v) => {
                    for e in v.elementaires() {
                        ta += crate::dsn::base_declaree(e, &bar, plafonnee).map_err(|e| e.reclasser(Classe::Refuse))?;
                    }
                }
                engagement::etats::PaieDuMois::Importee(i) => {
                    let Some(base) = crate::dsn::base_declaree_importee(i, &bar, plafonnee).map_err(|e| e.reclasser(Classe::Refuse))? else {
                        return Err(refus(format!(
                            "{}, {} : les pièces importées ne disent pas son assiette brute plafonnée ; la tranche A de l'année ne se calcule pas sans elle.",
                            qui(q.salarie())?,
                            libelle_mois(m)
                        )));
                    };
                    ta += base;
                }
            }
        }
        let an = m.annee();
        match par_an.iter_mut().find(|(a, _, _)| *a == an) {
            Some(x) => {
                x.1 += 1;
                x.2 += ta;
            }
            None => par_an.push((an, 1, Montant::ZERO + ta)),
        }
    }
    par_an.sort_by(|a, b| a.0.cmp(&b.0));
    let mut lignes = Vec::new();
    for (an, mois, tranche_a) in par_an {
        let calcule = tranche_a.fois_taux(taux).au_centime();
        let du = plus_grand(plancher, plus_petit(plafond, calcule));
        let regle = if du == plancher && calcule < plancher {
            "plancher"
        } else if du == plafond && calcule > plafond {
            "plafond"
        } else {
            "0,1 % de la tranche A"
        };
        // UNE ANNEE QUE LE LOGICIEL NE TIENT PAS EN ENTIER : la compagnie n'y a de paies (ecrites ou importees) qu'a
        // partir d'un mois apres janvier ; les mois d'avant manquent, la tranche A et la contribution sont partielles.
        let premier = p.dispo.keys().next().copied().filter(|m| m.annee() == an);
        let regle = if let Some(premier) = premier.filter(|m| m.numero() > 1) {
            let titre = |m: socle::mois::Mois| m.libelle(socle::mois::Graphie::Titre);
            let (janvier, avant) = (premier.annee().janvier(), premier.precedent());
            let manquent = if avant == janvier { titre(avant) } else { format!("{} à {}", titre(janvier), titre(avant)) };
            avert.push(format!(
                "{} est partielle : le logiciel n'a de paies qu'à partir de {} ; {} manquent. La tranche A et la contribution de {} sont celles des mois lus, pas de l'année.",
                an,
                titre(premier),
                manquent,
                an
            ));
            format!("{} ; année partielle", regle)
        } else {
            regle.to_string()
        };
        let mut l = Ligne::nouvelle();
        l.t(C::Annee, an.to_string()).compte(C::NombreDeMois, mois).montant(C::TrancheA, tranche_a).taux(C::Taux, Some(taux)).montant(C::Calcule, calcule).montant(C::ContributionDue, du).t(C::Regle, regle);
        l.t(C::APayer, format!("début {} (inférence : le texte ne dit pas l'année de masse)", an.suivante()?));
        lignes.push(l);
    }
    avert.push("Le FCAP-SVP se paie hors DSN, au FCAP-SVP (ou à l'organisme à qui il délègue la collecte, art. 5.2), en début d'année civile (art. 5.3). Que la tranche A soit celle de l'année écoulée est une inférence, à confirmer sur un appel du FCAP-SVP.".to_string());
    let mut t = Tableau::nouveau(EtatDExport::FcapSvp, "FCAP-SVP — financement du paritarisme (IDCC 3090)", p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    t.avec_total = false;
    t.sections = vec![Section::nouvelle("", lignes)];
    t.colonnes = vec![
        C::Annee.colonne(),
        C::NombreDeMois.titree("Mois lus"),
        C::TrancheA.colonne(),
        C::Taux.colonne(),
        C::Calcule.colonne(),
        C::ContributionDue.colonne(),
        C::Regle.colonne(),
        C::APayer.colonne(),
    ];
    t.sources = vec![
        "CCN 3090, titre V, articles 5.1 (0,1 % de la tranche A, plancher 80 €, plafond 300 € par an et par entreprise), 5.2 et 5.3 — base KALI de la DILA, relevée le 23/09/2026".to_string(),
        "tranche A : assiette brute plafonnée que la DSN déclare (S21.G00.78.001 = 02), de chaque paie : écrite, ou importée (la DSN ou le bulletin de l'ancien logiciel)".to_string(),
    ];
    finir(ctx, &mut t, Some(&p), "fcap-svp", d.format,d.ecrire)?;
    Ok(t)
}

// ================================================================ l'Afdas

/// Ce que la production audiovisuelle (2642) paie une fois l'an a l'Afdas, hors DSN,
/// avant le 28 fevrier. La production cinematographique (3097) paie les memes, aux memes taux ; les espaces de
/// loisirs (1790) la formation conventionnelle de leurs permanents. La masse se lit dans le livre de paie,
/// bulletin par bulletin, avec la nature du contrat : un CDDU est un intermittent.
pub fn afdas_2642(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    refuse_un_mois("Les contributions Afdas de la 2642 sont annuelles : elles se demandent pour une année, ou pour toutes.", d)?;
    let idcc = idcc_de(ctx)?;
    let branche = if idcc == IDCC_2642 {
        "la production audiovisuelle (IDCC 2642)"
    } else if idcc == IDCC_3097 {
        "la production cinématographique (IDCC 3097)"
    } else if idcc == IDCC_1790 {
        "les espaces de loisirs, d'attractions et culturels (IDCC 1790)"
    } else {
        return Err(refus(format!(
            "Les contributions Afdas de cet état sont celles de la production audiovisuelle (IDCC 2642), de la production cinématographique (IDCC 3097) et des espaces de loisirs (IDCC 1790) ; la fiche entreprise dit IDCC {}.",
            idcc
        )));
    };
    let p = Periode::de(ctx, d, true)?;
    let mut avert = Vec::new();
    let mut par_an: Vec<(Annee, Montant, Montant)> = Vec::new();
    // Un permanent est une nature MENSUELLE du catalogue des contrats.
    let mensuelles: Vec<String> = droit::natures::natures().map_err(|e| e.reclasser(Classe::Refuse))?.iter().filter(|n| n.mensuelle()).map(|n| n.libelle.clone()).collect();
    for l in lignes_livre(ctx, &p, &mut avert, &d.departs)? {
        let Some(brut) = l.brut else { continue };
        let an = l.mois.annee();
        let i = match par_an.iter().position(|(a, _, _)| *a == an) {
            Some(i) => i,
            None => {
                par_an.push((an, Montant::ZERO, Montant::ZERO));
                par_an.len() - 1
            }
        };
        par_an[i].1 += brut;
        if mensuelles.contains(&l.libelle_nature) {
            par_an[i].2 += brut;
        }
    }
    par_an.sort_by(|a, b| a.0.cmp(&b.0));
    let mut lignes = Vec::new();
    for (an, masse, masse_permanents) in par_an {
        let Some(hb) = hors_bulletin(ctx, an, &idcc)? else {
            return Err(socle::echec::cle(format!("Le barème {} ne porte pas les contributions Afdas de la {}.", an, idcc), &format!("par_convention.{}.hors_bulletin", idcc))
                .reclasser(Classe::Refuse));
        };
        let mut cpnef = Montant::ZERO;
        if let Some(c) = &hb.cpnef_dialogue_social {
            let brut_cpnef = masse.fois_taux(c.taux).au_centime();
            let borne = |b: Option<Montant>, cle: &str| b.ok_or_else(|| socle::fichiers::absent(cle).reclasser(Classe::Manque));
            cpnef = plus_grand(borne(c.plancher, "plancher")?, plus_petit(borne(c.plafond, "plafond")?, brut_cpnef));
        }
        let tranches = &hb
            .contribution_conventionnelle_permanents
            .as_ref()
            .ok_or_else(|| socle::fichiers::absent("contribution_conventionnelle_permanents").reclasser(Classe::Manque))?
            .par_effectif;
        // LE TAUX DES PERMANENTS SE LIT A L'EFFECTIF DE L'ANNEE (retours de nature 118) : sans lui, il ne se devine pas
        // (regle 1) ; sans masse de permanents, il n'y a rien a calculer.
        let taux_p = match ctx.effectif()?.valeur(an) {
            Some(eff) => Some(
                tranches
                    .iter()
                    .find(|x| x.jusqu_a.is_none_or(|j| eff <= j))
                    .ok_or_else(|| socle::echec::cle("Le barème de l'année est incomplet : le signaler au développeur.", "tranches").reclasser(Classe::Manque))?
                    .taux,
            ),
            None if masse_permanents.est_nul() => None,
            None => {
                return Err(refus(format!(
                    "{} : l'effectif de l'année n'est pas connu (retours de nature 118), et le taux de la contribution conventionnelle des permanents en dépend : relever les retours. Rien n'est écrit.",
                    an
                )))
            }
        };
        let permanents = match taux_p {
            Some(t) => masse_permanents.fois_taux(t).au_centime(),
            None => Montant::ZERO,
        };
        let mut l = Ligne::nouvelle();
        l.t(C::Annee, an.to_string())
            .montant(C::Masse, masse)
            .montant(C::MassePermanents, masse_permanents)
            .montant(C::Cpnef, cpnef)
            .taux(C::TauxPermanents, taux_p)
            .montant(C::Permanents, permanents);
        l.montant(C::TotalHt, cpnef + permanents);
        l.t(C::APayer, format!("avant le 28/02/{}, à l'Afdas (Globaldec')", an.suivante()?));
        lignes.push(l);
    }
    if idcc == IDCC_1790 {
        avert.push("Montants hors taxes : l'Afdas facture la TVA en plus. Taux de l'avenant n° 52 (article 1.5) : 0,35 % sous 10 salariés, 0,30 % à partir de 10, sur les salaires des CDI et des CDD. La plaquette Afdas « Loisirs 2026 » place le seuil à 11 : l'écart ne touche qu'un effectif d'exactement 10.".to_string());
    } else {
        avert.push("Montants hors taxes : l'Afdas facture la TVA en plus. Le taux de la CPNEF est celui de l'Afdas, qui collecte (0,004 %, 15 à 2 000 € HT) ; la web-conférence Audiens 2026 donne l'inverse — à confirmer sur le premier appel de l'Afdas.".to_string());
    }
    avert.push("La contribution conventionnelle des intermittents (0,10 % et un forfait) n'est pas ici : elle est traitée comme sous la 3090.".to_string());
    let mut t = Tableau::nouveau(EtatDExport::Afdas2642, format!("Contributions Afdas de {}", branche), p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    t.avec_total = false;
    t.sections = vec![Section::nouvelle("", lignes)];
    let mut cols = vec![C::Annee.colonne(), C::Masse.colonne()];
    if idcc != IDCC_1790 {
        cols.push(C::Cpnef.colonne());
    }
    cols.extend([
        C::MassePermanents.colonne(),
        C::TauxPermanents.colonne(),
        C::Permanents.colonne(),
        C::TotalHt.colonne(),
        C::APayer.colonne(),
    ]);
    t.colonnes = cols;
    t.sources = vec![
        if idcc == IDCC_1790 { "CCN 1790, avenant n° 52, article 1.5 ; plaquette Afdas « Loisirs 2026 »" } else { "plaquette Afdas « Contributions 2026 Audiovisuel » (janvier 2026)" }.to_string(),
        "masse salariale : le livre de paie, bulletin par bulletin".to_string(),
    ];
    finir(ctx, &mut t, Some(&p), "afdas-2642", d.format,d.ecrire)?;
    Ok(t)
}

// ===================================================== le paritarisme de branche

/// Le financement du paritarisme d'une branche qui le collecte une fois
/// l'an, hors DSN, sur la masse salariale de l'annee ecoulee (3252, 3241, et toute couche qui porte
/// `hors_bulletin.financement_paritarisme`). La masse se lit dans le livre de paie.
pub fn paritarisme_branche(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    refuse_un_mois("Le paritarisme de branche est annuel : il se demande pour une année, ou pour toutes.", d)?;
    let idcc = idcc_de(ctx)?;
    let p = Periode::de(ctx, d, true)?;
    let mut avert: Vec<String> = Vec::new();
    // La masse de chaque annee : la somme de ses bruts, a partir de zero au centime.
    let mut par_an: std::collections::BTreeMap<Annee, Montant> = std::collections::BTreeMap::new();
    for l in lignes_livre(ctx, &p, &mut avert, &d.departs)? {
        let Some(brut) = l.brut else { continue };
        *par_an.entry(l.mois.annee()).or_insert(Montant::ZERO) += brut;
    }
    let mut lignes = Vec::new();
    for (an, masse) in par_an {
        let f = hors_bulletin(ctx, an, &idcc)?.and_then(|h| h.financement_paritarisme);
        let Some(f) = f else {
            return Err(socle::echec::cle(
                format!("La convention {} n'a pas de paritarisme annuel hors DSN dans le barème {}.", idcc, an),
                &format!("par_convention.{}.hors_bulletin.financement_paritarisme", idcc),
            )
            .reclasser(Classe::Refuse));
        };
        let taux = f.taux;
        let mut m = masse.fois_taux(taux).au_centime();
        if let Some(pl) = f.plancher {
            m = plus_grand(m, pl);
        }
        if let Some(pl) = f.plafond {
            m = plus_petit(m, pl);
        }
        // A qui payer, si la couche le dit.
        let a_payer: String = std::iter::once(format!("en {}", an.suivante()?))
            .chain(f.collecteur.iter().filter(|c| !c.is_empty()).map(|c| format!(", à {}", c)))
            .collect();
        if let Some(a) = f.avertissement.clone().filter(|a| !a.is_empty()) {
            if !avert.contains(&a) {
                avert.push(a);
            }
        }
        let mut l = Ligne::nouvelle();
        l.t(C::Annee, an.to_string()).montant(C::Masse, masse).taux(C::Taux, Some(taux)).montant(C::Montant, m).t(C::APayer, a_payer);
        lignes.push(l);
    }
    avert.push("Assiette : le brut du livre de paie, tous salariés. Le texte parle de masse salariale ou de masse déclarée en retraite complémentaire : pour une compagnie du spectacle, c'est le même brut.".to_string());
    let mut t = Tableau::nouveau(EtatDExport::ParitarismeBranche, format!("Financement du paritarisme de la convention {}", idcc), p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    t.avec_total = false;
    t.sections = vec![Section::nouvelle("", lignes)];
    t.colonnes = vec![C::Annee.titree("Année de la masse"), C::Masse.colonne(), C::Taux.colonne(), C::Montant.titree("Contribution"), C::APayer.colonne()];
    t.sources = vec!["la couche de la convention, `hors_bulletin`".to_string(), "masse salariale : le livre de paie, bulletin par bulletin".to_string()];
    finir(ctx, &mut t, Some(&p), "paritarisme-branche", d.format,d.ecrire)?;
    Ok(t)
}
