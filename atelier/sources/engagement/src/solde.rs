//! Le solde de tout compte :
//! ce qui est du au salarie quand son contrat se termine -- conges non pris,
//! precarite, preavis, indemnite de rupture -- et les phrases qui le disent.

use droit::catalogue::rubrique;
use droit::fiche::Lue;
use droit::clauses;
use droit::conges::jours_ouvrables;
use crate::conges::{
    avertir, brut_non_dit, compteur_conges, debut_de, info, jours_dits, jours_ouvrables_de, paies_versees, qui,
    salarie_de, taux_au_montant,
};
use crate::saisie::Carte;
use crate::ligne::PaiementDeLigne;
use crate::LignePaie;
use droit::contexte::{aujourdhui, Contexte};
use droit::natures::{AuMois, MotifDeRupture, Nature};
use socle::mois::Mois;
use crate::Avis;
use socle::texte::euros;
use socle::echec::R;
use socle::grandeurs::{courte, Coefficient, Jours, Montant, MontantUnitaire, NonNul};
use socle::grandeurs::ecrit::texte;
use droit::permanents::{self, jours_acquis, IndemniteDePreavis};
use socle::date::{Anciennete, Periode};

/// Les lignes et les avertissements du solde.
#[derive(Default)]
struct Solde {
    lignes: Vec<LignePaie>,
    avertissements: Vec<Avis>,
    /// La ligne de l'indemnite compensatrice de conges payes (son rang dans `lignes`), ses jours et son montant.
    conges_payes: Option<(usize, Jours, Montant)>,
}

/// La ligne de `rubrique` qui paie `paiement`, telle que le solde l'ecrit, sa quantite et son taux dans leur forme la plus
/// courte.
fn ligne(rubrique: &droit::catalogue::CodeDeRubrique, paiement: PaiementDeLigne) -> LignePaie {
    LignePaie::composee(rubrique.clone(), paiement.courte())
}

/// La ligne d'une indemnite de `rubrique` qui verse `montant` en une fois, dans l'unite de sa rubrique
/// (`PaiementDeLigne::une_fois`).
fn ligne_unique(ctx: &Contexte, rubrique: &droit::catalogue::CodeDeRubrique, montant: Montant) -> R<LignePaie> {
    let unite = droit::catalogue::rubrique(&ctx.racine, rubrique, &ctx.lue_au)?.unite;
    Ok(LignePaie::composee(rubrique.clone(), PaiementDeLigne::une_fois(unite, courte(montant))))
}

/// Le plus grand de `a` et `b` : `a` a egalite (son ecriture gardee).
fn plus_grand<T: PartialOrd>(a: T, b: T) -> T {
    if b > a {
        b
    } else {
        a
    }
}

/// Le plus petit de `a` et `b` : `a` a egalite (son ecriture gardee).
fn plus_petit<T: PartialOrd>(a: T, b: T) -> T {
    if b < a {
        b
    } else {
        a
    }
}

/// Un montant d'indemnite qu'un texte donne, ou zero (au centime) quand il n'en donne pas : rien n'est du.
fn du_ou_rien(v: Option<Montant>) -> Montant {
    match v {
        Some(x) => x,
        None => Montant::ZERO,
    }
}

/// L'article de la convention qui donne une indemnite ; la convention elle-meme quand son releve ne dit pas l'article.
fn l_article(article: Option<&str>, idcc: Option<&droit::fiche::Idcc>) -> String {
    match article.filter(|a| !a.is_empty()) {
        Some(a) => a.to_string(),
        None => clauses::nom(idcc),
    }
}

/// L'IDCC de la fiche, tel que les clauses le lisent (`None` : la compagnie n'en dit pas).
fn idcc(ctx: &Contexte) -> Option<&droit::fiche::Idcc> {
    ctx.entreprise.idcc.dite()
}

// ------------------------------------------------------------ le profil

fn profil(ctx: &Contexte, e: &Carte) -> R<clauses::Profil> {
    let fiche = crate::fiche_du_salarie(ctx, e.salarie.as_ref().ok_or_else(|| socle::fichiers::absent("salarie"))?)?.champs;
    clauses::profil(idcc(ctx), &fiche, e.nature_lue()?.au_mois().as_ref(), &dit_par_l_engagement(e)?)
}

/// CE QUE L'ENGAGEMENT `e` DIT DU SALARIE la ou la fiche le dit aussi (`clauses::DitParLEngagement`) : son statut, son
/// code emploi (`metier`), la case cadre, son debut. Une case vide ne
/// dit rien ; la carte n'a pas de case pour la categorie dans la convention, que seule la fiche dit.
pub(crate) fn dit_par_l_engagement(e: &Carte) -> R<clauses::DitParLEngagement> {
    Ok(clauses::DitParLEngagement { categorie_conventionnelle: None, statut: e.emploi.as_ref().and_then(|x| x.statut), metier: e.metier.clone(), cadre: e.cadre, debut: e.debut })
}

/// L'age du salarie de `e` au dernier jour ecrit du contrat (ou aujourd'hui), en annees revolues ; `None` quand sa
/// fiche ne dit pas sa naissance.
pub(crate) fn age(ctx: &Contexte, e: &Carte) -> R<Option<Anciennete>> {
    let fiche = e.salarie.as_ref().map(|s| crate::fiche_du_salarie(ctx, s)).transpose()?;
    let Some(naissance) = fiche.and_then(|f| f.date_naissance.dite().copied()) else {
        return Ok(None);
    };
    let jour = match e.fin {
        Some(f) => f,
        None => aujourdhui()?,
    };
    Ok(Some(Anciennete::age(&naissance, &jour)))
}

// ------------------------------------------------------------ le preavis

/// LE PREAVIS QUE LE SOLDE COMPTE pour une fin de contrat : sa duree, ce qu'en donnent la loi et la convention, et ce
/// qui le dit en mots.
pub struct PreavisCompte {
    /// Sa duree et d'ou elle vient ; aucune quand elle ne se compte pas (`pourquoi` le dit).
    pub duree: Option<DureeDuPreavis>,
    /// Ce que la loi en donne, et la convention, en mois de salaire.
    pub legal: Option<Coefficient>,
    pub convention: Option<Coefficient>,
    pub dit: String,
    pub pourquoi: Option<String>,
}

/// LA DUREE D'UN PREAVIS QUI SE COMPTE, en mois de salaire (ce que paie son indemnite compensatrice : une fraction de mois
/// quand la convention le dit en jours), et d'ou elle vient : l'une ne va pas sans l'autre.
pub struct DureeDuPreavis {
    pub mois: Coefficient,
    pub source: SourceDuPreavis,
}

/// D'OU VIENT LA DUREE D'UN PREAVIS : le motif de la rupture (il n'en donne aucun), la loi (L1234-1), ou l'article de la
/// convention qui en donne plus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceDuPreavis {
    MotifDeRupture,
    Loi,
    Convention(String),
}

impl DureeDuPreavis {
    /// Qui donne la duree, en mots (« la loi (L1234-1) », l'article de la convention).
    pub fn qui(&self) -> &str {
        match &self.source {
            SourceDuPreavis::MotifDeRupture => "le motif de rupture",
            SourceDuPreavis::Loi => "la loi (L1234-1)",
            SourceDuPreavis::Convention(article) => article,
        }
    }
}

/// LE TERME D'UN PREAVIS DE `mois` MOIS ENTIERS qui suit la fin `fin` du travail : le meme quantieme `mois` mois plus tard,
/// le dernier jour du mois quand ce quantieme n'existe pas. Lecture : le preavis court du lendemain de la fin saisie (le
/// texte, L1234-3, le fait courir de la presentation de la lettre, que le moteur ne connait pas). `None` : une duree
/// qui n'est pas en mois entiers.
fn terme_du_preavis(fin: &socle::date::Jour, mois: Coefficient) -> R<Option<socle::date::Jour>> {
    match mois.en_mois_entiers() {
        Ok(n) => fin.apres(n).map(Some),
        Err(socle::grandeurs::MoisNonEntiers::Fraction) => Ok(None),
        Err(socle::grandeurs::MoisNonEntiers::HorsDuCalendrier) => Err(socle::refus!("préavis hors du calendrier : {} mois", texte::dit(mois))),
    }
}

/// `age` : celui du salarie au dernier jour ecrit du contrat (`age`).
pub(crate) fn duree_du_preavis(ctx: &Contexte, e: &Carte, anciennete: Anciennete, motif: Option<MotifDeRupture>, age: Option<Anciennete>) -> R<PreavisCompte> {
    let idcc = idcc(ctx);
    let cdd =!e.nature_lue()?.duree_indeterminee;
    let legal = permanents::preavis_mois(anciennete, motif, cdd)?;
    let prof = profil(ctx, e)?;
    let categorie = prof.categorie.as_ref();
    let conv = clauses::preavis_mois(idcc, categorie, anciennete, motif, age)?;
    let conv_mois = conv.mois;
    // Une convention qui ne donne pas de duree dit pourquoi ; une qui en donne une dit son article (`clauses::preavis_mois`).
    let conv_pourquoi = || conv.pourquoi.as_deref().ok_or_else(|| socle::echec::refus("le préavis de la convention ne donne pas de durée et ne dit pas pourquoi"));
    let conv_article = || conv.article.as_deref().ok_or_else(|| socle::echec::refus("le préavis de la convention donne une durée sans dire son article"));
    let categorie_dite: String = categorie.map(clauses::categorie_en_mots).into_iter().collect();
    let mut out = PreavisCompte { duree: None, legal, convention: conv_mois, dit: String::new(), pourquoi: None };
    let rel = clauses::releve(idcc)?;
    let releve = rel.as_ref().is_some_and(|r| r.loi_seule(clauses::Section::Preavis).is_none());
    // La rupture anticipee d'un CDD par le salarie qui part pour un CDI (L1243-2) : un preavis en jours, que ni la loi du
    // licenciement ni la convention ne disent, et que le moteur ne compte pas.
    if let Some(regle) = permanents::regle_du_preavis(motif, cdd)? {
        out.convention = None;
        out.pourquoi = Some(format!("{} Dites sa durée sur la carte (« durée du préavis, en mois »).", regle));
        return Ok(out);
    }
    let nul = |m: Option<Coefficient>| m.is_some_and(Coefficient::est_nul);
    if nul(legal) || nul(conv_mois) {
        out.duree = Some(DureeDuPreavis { mois: Coefficient::ZERO, source: SourceDuPreavis::MotifDeRupture });
        out.pourquoi = if nul(conv_mois) { conv.pourquoi.clone() } else { None };
        return Ok(out);
    }
    if releve && conv_mois.is_none() && categorie.is_some() {
        out.pourquoi = Some(format!("{}. Dites sa durée sur la carte (« durée du préavis, en mois »).", conv_pourquoi()?));
        return Ok(out);
    }
    if releve && conv_mois.is_none() {
        let article = rel.as_ref().and_then(|r| r.preavis.as_ref()).map(|p| p.article.clone()).ok_or_else(|| socle::echec::livree(&droit::clauses::FICHIER, "preavis.article"))?;
        out.pourquoi = Some(format!(
            "la durée du préavis dépend de la catégorie du salarié dans {} \
             (art. {}), et {}. Dites-la sur la fiche du salarié \
             (« Catégorie dans la convention » : artiste, employé, \
             agent de maîtrise, cadre).",
            clauses::nom(idcc),
            article,
            prof.source
        ));
        return Ok(out);
    }
    let candidats: Vec<Coefficient> = [legal, conv_mois].into_iter().flatten().collect();
    if candidats.is_empty() {
        out.pourquoi = Some(format!(
            "la loi ne dit pas sa durée en dessous de six mois d'ancienneté \
             (L1234-1, 1° : « la convention ou l'accord collectif de travail \
             ou, à défaut, les usages ») et {}. Dites-la sur la carte \
             (« durée du préavis, en mois »).",
            conv_pourquoi()?
        ));
        return Ok(out);
    }
    let mois = candidats.iter().copied().fold(candidats[0], plus_grand);
    // La categorie entre parentheses, quand le preavis de la convention en depend.
    let pour_la_categorie = if categorie_dite.is_empty() { String::new() } else { format!(" ({})", categorie_dite) };
    if conv_mois.is_none() {
        out.duree = Some(DureeDuPreavis { mois, source: SourceDuPreavis::Loi });
        out.dit = format!("L1234-1 ; {} : seule la loi est calculée", conv_pourquoi()?);
    } else if legal.is_none() || conv_mois > legal {
        out.duree = Some(DureeDuPreavis { mois, source: SourceDuPreavis::Convention(conv_article()?.to_string()) });
        out.dit = format!(
            "{}{} ; {}",
            conv_article()?,
            pour_la_categorie,
            match legal {
                None => "la loi n'en donne pas sous six mois".to_string(),
                Some(l) => format!("la loi en donne {}", texte::dit(l)),
            }
        );
    } else {
        out.duree = Some(DureeDuPreavis { mois, source: SourceDuPreavis::Loi });
        out.dit = format!(
            "L1234-1 ; {} en donne {}{}",
            conv_article()?,
            conv.mois.iter().map(|m| texte::dit(*m)).collect::<String>(),
            pour_la_categorie
        );
    }
    Ok(out)
}

// ------------------------------------------------------ le solde lui-meme

/// LES RELATIONS DE TRAVAIL QUI SE POURSUIVENT PAR UN CDI : l'indemnite de fin de contrat est due « lorsque, a l'issue
/// d'un contrat de travail a duree determinee, les relations contractuelles de travail ne se poursuivent pas par un
/// contrat a duree indeterminee » (C. trav. L1243-8, `atelier/reference/sources/code-du-travail/articles-20261102.md`).
/// Le debut du CDI valide du meme salarie qui commence apres le debut du CDD `e` et au plus tard le premier jour de
/// semaine qui suit son terme `fin`, ou `None`. INFERENCE : « a l'issue » se lit sans jour de semaine entre les deux (un
/// CDD qui finit le vendredi, un CDI qui commence le lundi) ; un jour ferie entre eux n'est pas compte. La saisie de ce
/// salarie dans les mois lus se note (`stock::cle_de_saisie`) : qu'un CDI s'y valide ou s'en retire, la paie du dernier
/// mois du CDD se recalcule.
fn cdi_a_l_issue(ctx: &Contexte, e: &Carte, fin: &socle::date::Jour) -> R<Option<socle::date::Jour>> {
    let salarie = salarie_de(e)?;
    let debut = debut_de(e)?;
    // Le premier jour de semaine (du lundi au vendredi) apres le terme.
    let mut reprise = fin.apres(socle::date::Duree::jours(1))?;
    while !socle::date::Semaine::DuLundiAuVendredi.contient(reprise.jour_de_la_semaine()) {
        reprise = reprise.apres(socle::date::Duree::jours(1))?;
    }
    let mut mois = vec![fin.mois_civil()];
    if reprise.mois_civil() != fin.mois_civil() {
        mois.push(reprise.mois_civil());
    }
    let mut trouve = None;
    for m in mois {
        let Some(s) = crate::stock::vue_et_empreinte(&ctx.racine, m)? else { continue };
        socle::fichiers::noter_lecture(&crate::stock::cle_de_saisie(m, Some(salarie)), socle::fichiers::ValeurLue::Saisie(crate::stock::empreinte_de_saisie(&s.0, Some(salarie))));
        for x in &s.0.engagements {
            let sien = x.valide.is_some() && x.salarie.as_ref() == Some(salarie);
            if !sien || !x.nature_lue()?.duree_indeterminee {
                continue;
            }
            let Some(j) = x.debut else { continue };
            if j > debut && j <= reprise {
                trouve = Some(j);
            }
        }
    }
    Ok(trouve)
}

/// Les lignes du dernier bulletin : conges non pris, precarite, preavis,
/// indemnite de rupture. `deja` : les lignes de CE mois deja construites.
pub(crate) fn solde_de_tout_compte(ctx: &Contexte, au: &AuMois, e: &Carte, mois: Mois, deja: &[LignePaie]) -> R<(Vec<LignePaie>, Vec<Avis>)> {
    let nat = e.nature_lue()?;
    let mut stc = Solde::default();
    let Some(fin) = e.fin else {
        return Ok((stc.lignes, stc.avertissements));
    };
    let salaire = crate::conges::salaire_mensuel(ctx, e)?;
    let qui = qui(ctx, e);
    let motif = e.motif_rupture;
    let chomes = ctx.entreprise.feries_chomes();

    // --- l'indemnite compensatrice de conges payes (L3141-28 ; L1242-16 pour un CDD), quelle que soit la cause de la
    // rupture. L3141-28 « [n'est pas applicable] lorsque l'employeur est tenu d'adherer a une caisse de conges » (L3141-30) :
    // la caisse des Conges Spectacles « assure le service des conges » (D7121-38) du salarie que l'employeur lui declare
    // (D7121-41 ; `caisse_conges::affilie`, le meme que la paie lit). Ce salarie n'a pas d'indemnite de l'employeur.
    if crate::calcul::saisie_declaree_a_la_caisse_des_conges(ctx, e, &fin)? {
        stc.avertissements.push(info(format!(
            "{} : ses conges sont payes par la caisse des Conges Spectacles, pas d'indemnite compensatrice (L3141-30).",
            qui
        )));
    } else {
        let salarie = salarie_de(e)?;
        let cpt = compteur_conges(ctx, au, salarie, &fin, mois)?;
        // L'indemnite paie le solde du compteur : un compteur qui ignore les mois d'avant PuppetPaie la sous-paierait.
        // Sa periode la plus ancienne : celle du report, s'il y en a un, sinon la periode en cours.
        let depuis = match cpt.report {
            Some(report) => report.debut(),
            None => cpt.periode.debut(),
        };
        if let Some((debut, premier)) = crate::conges::compte_sans_la_reprise(ctx, au, &salarie, &depuis)? {
            stc.avertissements.push(avertir(format!(
                "{} : son contrat a commencé le {}, avant {}, premier mois payé ici ; ses congés acquis avant ne sont pas \
                 comptés dans l'indemnité de ses congés non pris (L3141-28). Saisissez dans sa fiche « Congés payés à la \
                 reprise », recopiés du dernier bulletin de l'ancien logiciel.",
                qui,
                debut.jj_mm_aaaa(),
                premier.libelle(socle::mois::Graphie::Accentuee),
            )));
        }
        let mut pris_ce_mois = Jours::ZERO;
        for c in e.conges_pris.iter().flatten() {
            pris_ce_mois += jours_ouvrables_de(c, chomes)?;
        }
        let contrat = e.contrat()?.ok_or_else(|| socle::fichiers::absent("debut"))?;
        let dans_le_mois = crate::contrat_dans_le_mois(&contrat, mois, &qui)?;
        let acquis_ce_mois = jours_acquis(&dans_le_mois)?;
        let solde = cpt.solde() + acquis_ce_mois - pris_ce_mois;
        if solde > Jours::ZERO {
            // Le maintien (L3141-24, II, `atelier/reference/sources/code-du-travail/xml/L3141-24.xml`) : ce que le salarie
            // aurait percu s'il avait travaille ces jours, le salaire du mois entier reparti sur les jours ouvrables du
            // mois entier, comme les conges pris (`conges::lignes_du_mois`), quel que soit le jour ou le contrat finit.
            let maintien = (solde * salaire_d_un_jour_ouvrable(salaire, mois, chomes)?).au_centime();
            // Les jours pris ce mois-ci ont deja consomme le report d'abord :
            // le reste du report est ce qui se paie au dixieme d'avant.
            let entame = plus_petit(pris_ce_mois, cpt.report_restant());
            let mut cpt2 = cpt.clone();
            cpt2.reports = cpt.reports - entame;
            let ind = cpt2.indemnite_des_jours(solde, maintien)?;
            stc.conges_payes = Some((stc.lignes.len(), solde, ind.retenu));
            stc.lignes.push(ligne(&permanents::regles()?.conges_payes.indemnite_compensatrice.rubrique, PaiementDeLigne::Jours(solde, taux_au_montant(ind.retenu, solde)?)));
            stc.avertissements.push(info(format!(
                "{} : {} jour(s) de conges acquis et non pris, payes {} ({}) -- \
                 art. L3141-28, due quelle que soit la cause de la rupture.",
                qui,
                jours_dits(solde),
                euros(ind.retenu),
                ind.regle
            )));
        } else if solde < Jours::ZERO {
            stc.avertissements.push(avertir(format!(
                "{} : le compteur de conges est NEGATIF ({} jours) -- le salarie \
                 a pris plus que ce qu'il a acquis. La retenue sur le solde de \
                 tout compte n'est pas calculee : elle se decide, et elle est \
                 encadree. A regler a la main.",
                qui,
                jours_dits(solde)
            )));
        }
    }

    // --- l'indemnite de fin de CDD (L1243-8)
    let cdi = if nat.duree_indeterminee { None } else { cdi_a_l_issue(ctx, e, &fin)? };
    match permanents::precarite_due(&nat, motif)? {
        None if cdi.is_some() => {
            stc.avertissements.push(info(format!(
                "{} : pas d'indemnite de fin de contrat -- les relations de travail se poursuivent par un CDI, a compter du {} (L1243-8).",
                qui,
                cdi.iter().map(|j| j.jj_mm_aaaa()).collect::<String>()
            )));
        }
        None => {
            if e.precarite_due == Some(false) {
                stc.avertissements.push(info(format!(
                    "{} : indemnite de precarite ecartee a la main{}.",
                    qui,
                    e.precarite_pourquoi.as_deref().filter(|p| !p.is_empty()).map(|p| format!(" ({})", p)).into_iter().collect::<String>()
                )));
            } else {
                let total = remuneration_du_contrat(ctx, e, mois, brut_des_lignes(ctx, deja)?)?;
                // Le taux de la loi : la fiche entreprise ne porte aucun accord qui le change.
                let montant = permanents::indemnite_precarite(total)?;
                if montant > Montant::ZERO {
                    // Une ligne en pourcentage : sa base, la remuneration totale, et le taux de la loi en pourcentage ; elle
                    // paie le montant de `indemnite_precarite`.
                    let pourcentage = permanents::regles()?.indemnite_precarite.taux.en_taux();
                    stc.lignes.push(LignePaie::en_pourcentage(
                        permanents::regles()?.indemnite_precarite.rubrique.clone(),
                        courte(total),
                        courte(pourcentage),
                    ));
                    let pour_cent = socle::texte::virgule(pourcentage);
                    stc.avertissements.push(info(format!(
                        "{} : indemnité de fin de contrat {} % sur {} de \
                         rémunération totale = {} (L1243-8).",
                        qui, pour_cent, euros(total), euros(montant)
                    )));
                } else {
                    // Une indemnite nulle ne s'ecrit pas : la norme refuse `0.00`.
                    stc.avertissements.push(avertir(format!(
                        "{} : indemnité de fin de contrat NULLE -- la \
                         rémunération totale du contrat, chacun de ses mois \
                         payé, est de {} (L1243-8).",
                        qui, euros(total)
                    )));
                }
            }
        }
        Some(pourquoi) => {
            if !nat.duree_indeterminee {
                stc.avertissements.push(info(format!(
                    "{} : pas d'indemnite de fin de contrat -- {}",
                    qui, pourquoi
                )));
            }
        }
    }

    // --- le preavis non effectue : L1234-5 pour un licenciement ; les autres fins de contrat ont leurs articles
    // (`preavis.indemnite_compensatrice`), et certaines n'ouvrent aucune indemnite (037, 059, 039, 091).
    let contrat = Periode::du_au(debut_de(e)?, fin)?;
    let anciennete = contrat.anciennete();
    let mut anciennete_de_l_indemnite = anciennete;
    let indemnite = if e.preavis_non_effectue == Some(true) {
        Some(permanents::indemnite_de_preavis(motif)?)
    } else {
        None
    };
    if let Some(IndemniteDePreavis::Aucune(texte)) = &indemnite {
        let saisi = if e.preavis_mois.is_some() {
            " La durée du préavis saisie n'y change rien."
        } else {
            ""
        };
        stc.avertissements.push(avertir(format!(
            "{} : préavis déclaré non effectué, et aucune indemnité compensatrice de préavis n'est due pour ce motif : \
             le logiciel n'écrit pas de ligne.{} {}",
            qui, saisi, texte
        )));
    } else if let Some(indemnite) = &indemnite {
        let duree = duree_du_preavis(ctx, e, anciennete, motif, age(ctx, e)?)?;
        let mut mois_preavis = duree.duree.as_ref().map(|d| d.mois);
        if let Some(saisi) = e.preavis_mois {
            if let Some(d) = &duree.duree {
                if saisi < d.mois {
                    return Err(socle::refus!(
                        "{} : preavis de {} mois saisi, et {} en donne {}. Un \
                         contrat peut accorder plus que la loi et la convention, \
                         jamais moins.",
                        qui, texte::dit(saisi), d.qui(), texte::dit(d.mois)
                    ));
                }
            }
            mois_preavis = Some(saisi);
        } else if mois_preavis.is_none() {
            let pourquoi = match &duree.pourquoi {
                Some(p) => p.clone(),
                None => "la durée du préavis ne se lit ni de la loi ni de la convention : saisissez-la (préavis, en mois)".to_string(),
            };
            return Err(socle::refus!("{} : preavis non effectue, et {}", qui, pourquoi));
        }
        match mois_preavis.filter(|m| !m.est_nul()) {
            Some(m) => {
                // « L'inexécution du préavis de licenciement n'a pas pour conséquence d'avancer la date à laquelle le
                // contrat prend fin » (C. trav. L1234-4, `code-du-travail/xml/L1234-4.xml`) : l'ancienneté que
                // l'indemnité de licenciement compte (R1234-1, « par année de service dans l'entreprise ») court
                // jusqu'au terme du préavis.
                if matches!(indemnite, IndemniteDePreavis::Licenciement) {
                    if let Some(terme) = terme_du_preavis(&fin, m)? {
                        anciennete_de_l_indemnite = Anciennete::entre(&contrat.debut(), &terme);
                        stc.avertissements.push(info(format!(
                            "{} : le préavis non effectué ne fait pas finir le contrat plus tôt (code du travail, L1234-4) : \
                             l'ancienneté de l'indemnité de licenciement se compte jusqu'au {}, terme du préavis (lecture : il court du lendemain de la fin du travail).",
                            qui,
                            terme.jj_mm_aaaa()
                        )));
                    }
                }
                // La rubrique, et avec elle le code du bloc 52, suit le motif : 023, ou 032 pour le 092 (L1226-14).
                let rubrique = permanents::rubrique_de_l_indemnite_de_preavis(motif)?;
                // Les mois du preavis sont la part du mois que la ligne paie (sa rubrique compte des mois).
                stc.lignes.push(LignePaie::composee(rubrique, PaiementDeLigne::PartDuMois(courte(m), courte(salaire))));
                let article = match indemnite {
                    IndemniteDePreavis::AutreArticle(t) => format!(" Pour ce motif, l'indemnite vient d'un autre article : {}", t),
                    IndemniteDePreavis::ATrancher(t) => format!(
                        " Pour ce motif, aucun texte depose ne dit si L1234-5 s'applique, ni qu'il ne s'applique pas : {} \
                         A trancher.",
                        t
                    ),
                    IndemniteDePreavis::Licenciement | IndemniteDePreavis::Aucune(_) => String::new(),
                };
                // L1234-5 : « indemnité de congés payés comprise » ; un mois de travail acquiert ses jours (L3141-3).
                let jours = permanents::regles()?.conges_payes.acquisition.jours_par_mois * m;
                let paye = m * salaire;
                let conges = conges_non_effectues(ctx, e, mois, &fin, jours, salaire, paye.au_centime(), &mut stc)?;
                stc.avertissements.push(avertir(format!(
                    "{} : {} mois de préavis non effectué, payés {} \
                     (L1234-5){}. L'article le veut « indemnité de congés payés comprise » : {}{}",
                    qui,
                    socle::texte::virgule(m),
                    euros(paye),
                    if duree.dit.is_empty() { String::new() } else { format!(" -- {}", duree.dit) },
                    conges,
                    article
                )));
            }
            None => {
                // Un preavis a zero n'est pas un preavis de zero mois : il n'y en a pas.
                let sans = permanents::raison_sans_preavis(motif, !nat.duree_indeterminee)?;
                let raison = match sans.or(duree.pourquoi.filter(|p| !p.is_empty())) {
                    Some(r) => r,
                    None => "aucun preavis pour ce motif.".to_string(),
                };
                stc.avertissements.push(info(format!(
                    "{} : preavis declare non effectue, et il n'y a PAS de \
                     preavis a payer -- {}",
                    qui, raison
                )));
            }
        }
    }

    // --- le delai de prevenance d'une fin d'essai a l'initiative de l'employeur (L1221-25)
    delai_de_prevenance(ctx, au, e, mois, deja, &fin, salaire, motif, &nat, &qui, &mut stc)?;

    // --- l'indemnite de rupture, et laquelle depend du motif
    indemnite_de_rupture(ctx, e, mois, anciennete_de_l_indemnite, motif, &nat, &mut stc)?;

    // --- le type de preavis que le signalement declarera (S21.G00.63.001) et ce que ce solde verse : le
    // signalement refuse un desaccord (`fctu::generer`) ; le solde le dit des la saisie.
    if let Some(type_preavis) = e.preavis.as_ref().map(|p| p.genre) {
        let codes = droit::preavis::codes_52_de_l_indemnite_de_preavis()?;
        let mut versees = Vec::new();
        for l in &stc.lignes {
            let r = rubrique(&ctx.racine, l.rubrique(), &ctx.lue_au)?;
            if let Some(c) = r.dsn_bloc52.filter(|c| codes.contains(c)) {
                versees.push((c, r.libelle));
            }
        }
        if let Some(pourquoi) = droit::preavis::desaccord_du_preavis(type_preavis, &versees)? {
            stc.avertissements.push(avertir(format!(
                "{} : {} Le signalement de fin de contrat le refusera tel quel.",
                qui, pourquoi
            )));
        }
    }

    stc.avertissements.push(info(format!(
        "{} : le CERTIFICAT DE TRAVAIL et le RECU POUR SOLDE DE TOUT COMPTE \
         sont dus au salarie ; le logiciel les ecrit (Paie, « Fin de \
         contrat », « Les deux papiers… »). L'attestation France \
         Travail, elle, part d'elle-meme avec le signalement de fin de \
         contrat.",
        qui
    )));
    Ok((stc.lignes, stc.avertissements))
}

/// CE QUE L1221-25 DECIDE d'une fin de periode d'essai a l'initiative de l'employeur (034), quel que soit le paiement du
/// contrat (au mois ou a l'activite) : rien n'est du, et la phrase qui dit pourquoi ; ou le delai de prevenance n'est pas
/// respecte, et la part du delai que le contrat n'a pas laissee courir.
pub(crate) enum DelaiDePrevenanceDecide {
    /// Rien a payer : l'essai stipule d'un contrat a duree determinee n'atteint pas une semaine, la faute grave est dite,
    /// ou le delai expire au plus tard le jour ou le contrat finit.
    RienADu(Avis),
    /// Le delai n'est pas respecte : sa rubrique (`preavis.delai_de_prevenance.rubrique`), ce qui le dit en mots, et la
    /// part non effectuee, du lendemain de la fin du contrat au jour ou le delai expire.
    NonRespecte { rubrique: droit::catalogue::CodeDeRubrique, dit: String, non_effectue: Periode },
}

/// L1221-25 (`atelier/reference/sources/code-du-travail/xml/L1221-25.xml`) : « Lorsqu'il est mis fin, par l'employeur, au
/// contrat en cours ou au terme de la période d'essai définie aux articles L. 1221-19 à L. 1221-24 ou à l'article
/// L. 1242-10 pour les contrats stipulant une période d'essai d'au moins une semaine, le salarié est prévenu dans un délai
/// [...] » ; « son inexécution ouvre droit pour le salarié, sauf s'il a commis une faute grave, à une indemnité
/// compensatrice ». La decision pour la carte `e` d'un contrat du `debut` a la `fin`, au `motif` de sa fin ; `None` : le
/// motif n'est pas le 034. Un contrat a duree determinee (un CDDU compris : L1242-10 est l'essai de tout CDD) dit son essai
/// stipule, en jours, ou le calcul refuse en nommant la case ; la notification de la rupture aussi.
pub(crate) fn delai_de_prevenance_decide(
    e: &Carte, debut: &socle::date::Jour, fin: &socle::date::Jour, motif: Option<MotifDeRupture>, nat: &Nature, qui: &str,
) -> R<Option<DelaiDePrevenanceDecide>> {
    let Some(regle) = permanents::regle_du_delai_de_prevenance(motif)? else {
        return Ok(None);
    };
    // Un CDD : seulement s'il stipule une periode d'essai d'au moins une semaine (L1221-25, renvoi a L1242-10).
    if !nat.duree_indeterminee {
        let minimum = regle.cdd_essai_minimum_jours;
        let Some(essai) = e.periode_essai_jours else {
            return Err(socle::echec::cle(format!(
                "{} : fin de periode d'essai a l'initiative de l'employeur (034) d'un contrat a duree determinee : L1221-25 \
                 ne donne un delai de prevenance que si le contrat stipule une periode d'essai d'au moins une semaine. \
                 La saisie ne dit pas celle du contrat, en jours ; le moteur ne la devine pas.",
                qui
            ), "periode_essai_jours"));
        };
        if essai < minimum {
            return Ok(Some(DelaiDePrevenanceDecide::RienADu(info(format!(
                "{} : periode d'essai stipulee de {}, moins d'une semaine : pas de delai de prevenance (L1221-25), \
                 rien a payer.",
                qui,
                essai
            )))));
        }
    }
    if e.faute_grave == Some(true) {
        return Ok(Some(DelaiDePrevenanceDecide::RienADu(info(format!(
            "{} : fin de periode d'essai pour faute grave : pas d'indemnite du delai de prevenance (L1221-25, « sauf s'il a \
             commis une faute grave »).",
            qui
        )))));
    }
    let Some(notification) = e.notification_rupture else {
        return Err(socle::echec::cle(format!(
            "{} : fin de periode d'essai a l'initiative de l'employeur (034) : la date de notification de la rupture n'est \
             pas dite (« Notifiée le »). Le delai de prevenance de L1221-25 se compte depuis elle, selon la presence \
             du salarie ; le moteur ne la devine pas.",
            qui
        ), "notification_rupture"));
    };
    if notification < *debut || notification > *fin {
        return Err(socle::refus!(
            "{} : la notification de la rupture ({}) doit tomber entre le debut ({}) et la fin ({}) du contrat.",
            qui,
            notification.iso(),
            debut.iso(),
            fin.iso()
        ));
    }
    let delai = permanents::delai_de_prevenance(debut, &notification)?;
    let dit = format!(
        "fin de periode d'essai notifiee le {} apres {} de presence : delai de prevenance de {}, jusqu'au {}",
        notification.jj_mm_aaaa(),
        delai.presence_jours,
        delai.texte,
        delai.expiration.jj_mm_aaaa()
    );
    if delai.expiration <= *fin {
        return Ok(Some(DelaiDePrevenanceDecide::RienADu(info(format!(
            "{} : {} ; le contrat finit le {} : delai respecte, rien a payer.",
            qui,
            dit,
            fin.jj_mm_aaaa()
        )))));
    }
    let non_effectue = Periode::du_au(fin.apres(socle::date::Duree::jours(1))?, delai.expiration)?;
    Ok(Some(DelaiDePrevenanceDecide::NonRespecte { rubrique: regle.rubrique, dit, non_effectue }))
}

/// L1221-25 POUR UN CONTRAT PAYE AU MOIS : la decision (`delai_de_prevenance_decide`), puis, si le delai n'est pas
/// respecte, l'indemnite egale aux salaires que le salarie aurait percus jusqu'a l'expiration du delai, « indemnité
/// compensatrice de congés payés comprise » (`preavis.delai_de_prevenance`). La ligne : la part du delai apres la fin du
/// contrat, le salaire de chaque mois pour ses heures (`droit::periodes::part_du_mois`) ; DSN 52 « 017 ». Les conges que
/// ces jours acquierent s'ajoutent a l'indemnite compensatrice de conges payes du solde (`conges_non_effectues`).
fn delai_de_prevenance(
    ctx: &Contexte, au: &AuMois, e: &Carte, mois: Mois, deja: &[LignePaie], fin: &socle::date::Jour, salaire: MontantUnitaire, motif: Option<MotifDeRupture>, nat: &Nature,
    qui: &str, stc: &mut Solde,
) -> R<()> {
    let debut = debut_de(e)?;
    let (rubrique, dit, non_effectue) = match delai_de_prevenance_decide(e, &debut, fin, motif, nat, qui)? {
        None => return Ok(()),
        Some(DelaiDePrevenanceDecide::RienADu(avis)) => {
            stc.avertissements.push(avis);
            return Ok(());
        }
        Some(DelaiDePrevenanceDecide::NonRespecte { rubrique, dit, non_effectue }) => (rubrique, dit, non_effectue),
    };
    // La part non effectuee : du lendemain de la fin au dernier jour du delai, le salaire de chaque mois traverse pour ses
    // heures (`droit::periodes::part_du_mois`, l'horaire reel du mois).
    let (mut fraction, mut jours) = (Coefficient::ZERO, Jours::ZERO);
    for mois_de_j in non_effectue.mois() {
        let Some(dans_le_mois) = non_effectue.croise(&Periode::du_mois(mois_de_j)) else { continue };
        let horaire = crate::conges::horaire_du_mois(ctx, au, salarie_de(e)?, &debut, mois_de_j)?;
        let part = droit::periodes::part_du_mois(&dans_le_mois, mois_de_j, droit::periodes::RegleDuMoisIncomplet::HoraireReel, &horaire)?;
        fraction += part.fraction;
        jours += part.jours;
    }
    stc.lignes.push(ligne(&rubrique, PaiementDeLigne::PartDuMois(fraction, salaire)));
    let montant = (fraction * salaire).au_centime();
    let conges = conges_non_effectues(ctx, e, mois, fin, jours_acquis(&non_effectue)?, salaire, montant, stc)?;
    let avantages = avantages_du_mois(ctx, deja)?;
    let avantages = if avantages.is_empty() {
        "Les avantages : le bulletin de ce mois n'en porte aucun, il n'y a rien à ajouter.".to_string()
    } else {
        format!(
            "Les avantages : le bulletin de ce mois porte {} ; la part qui en revient aux jours du délai n'est PAS ajoutée \
             (le moteur ne sait pas la compter) : l'ajouter à la main.",
            avantages.join(", ")
        )
    };
    stc.avertissements.push(avertir(format!(
        "{} : {} ; le contrat finit le {} : {} jour(s) travaillé(s) du délai non effectué(s), payés {} au salaire mensuel, pour \
         leurs heures (L1221-25, indemnité compensatrice ; DSN 52 : 017). L1221-25 la veut « indemnité compensatrice de congés \
         payés comprise » : {} {} Le décompte de la présence et du délai est une lecture (permanents-base.json, \
         `preavis.delai_de_prevenance.note`).",
        qui,
        dit,
        fin.jj_mm_aaaa(),
        socle::grandeurs::ecrit::sans_zero::dit(jours),
        euros(montant),
        conges,
        avantages
    )));
    Ok(())
}

/// L1221-25 POUR UN CONTRAT PAYE A L'ACTIVITE (un CDDU), le mois ou il finit : la meme decision
/// (`delai_de_prevenance_decide`) ; si le delai n'est pas respecte, l'indemnite « égale au montant des salaires et
/// avantages que le salarié aurait perçus s'il avait accompli son travail jusqu'à l'expiration du délai de prévenance ».
/// Un tel contrat est paye a ses cachets et heures des jours travailles, et ses jours apres la fin ne sont pas sur la
/// carte (les jours coches sont les jours travailles) : ce montant, seule la compagnie le connait. La carte le dit
/// (`indemnite_delai_prevenance`), ou le calcul refuse en nommant la case et les dates du delai non effectue. La ligne :
/// ce montant, en une fois, dans la rubrique du delai (DSN 52 « 017 »). Rend la ligne, s'il y en a une, et ce qui le dit.
pub(crate) fn delai_de_prevenance_a_l_activite(ctx: &Contexte, e: &Carte, eng: &crate::Engagement, qui: &str) -> R<(Option<LignePaie>, Vec<Avis>)> {
    let contrat = eng.contrat();
    let Some(fin) = contrat.fin().filter(|f| f.mois_civil() == eng.mois) else {
        return Ok((None, Vec::new()));
    };
    let nat = eng.nat()?;
    let (rubrique, dit, non_effectue) = match delai_de_prevenance_decide(e, &contrat.debut(), &fin, e.motif_rupture, &nat, qui)? {
        None => return Ok((None, Vec::new())),
        Some(DelaiDePrevenanceDecide::RienADu(avis)) => return Ok((None, vec![avis])),
        Some(DelaiDePrevenanceDecide::NonRespecte { rubrique, dit, non_effectue }) => (rubrique, dit, non_effectue),
    };
    let du_au = format!("du {} au {}", non_effectue.debut().jj_mm_aaaa(), non_effectue.fin().jj_mm_aaaa());
    let Some(montant) = e.indemnite_delai_prevenance else {
        return Err(socle::echec::cle(format!(
            "{} : {} ; le contrat finit le {} : le délai n'est pas respecté, {}. L1221-25 doit au salarié, sauf faute grave, \
             « le montant des salaires et avantages [qu'il] aurait perçus s'il avait accompli son travail jusqu'à l'expiration \
             du délai de prévenance, indemnité compensatrice de congés payés comprise ». Saisissez dans sa carte ce qu'il \
             aurait perçu {}, cachets, heures et avantages compris (« {} ») ; s'il a commis une faute grave, cochez « {} » : \
             rien n'est dû.",
            qui,
            dit,
            fin.jj_mm_aaaa(),
            du_au,
            du_au,
            crate::carte::libelle(crate::saisie::Champ::IndemniteDelaiPrevenance),
            crate::carte::libelle(crate::saisie::Champ::FauteGrave)
        ), "indemnite_delai_prevenance"));
    };
    if montant < Montant::ZERO {
        return Err(socle::echec::cle(format!(
            "{} : l'indemnité du délai de prévenance saisie est négative ({}) : saisissez ce qu'il aurait perçu {}.",
            qui,
            euros(montant),
            du_au
        ), "indemnite_delai_prevenance"));
    }
    // Les conges de ces jours : L1221-25 les compte dans l'indemnite ; un salarie dont la caisse des Conges Spectacles
    // paie les conges n'en a pas chez l'employeur (L3141-30).
    let ctx_fin = ctx.au(&fin)?;
    let fiche = crate::calcul::fiche_de_l_engagement(&ctx_fin, eng)?;
    let conges = if droit::caisse_conges::affilie(&nat, &fiche, &contrat)? {
        "Ses congés sont payés par la caisse des Congés Spectacles (L3141-30) : rien à y ajouter."
    } else {
        "L1221-25 la veut « indemnité compensatrice de congés payés comprise » : le montant saisi les comprend."
    };
    // Une indemnite nulle ne s'ecrit pas : la norme refuse `0.00`.
    if montant.est_nul() {
        return Ok((None, vec![info(format!(
            "{} : {} ; le contrat finit le {} : délai non effectué {}, et l'indemnité saisie est nulle (il n'aurait rien perçu \
             ces jours-là) : aucune ligne.",
            qui,
            dit,
            fin.jj_mm_aaaa(),
            du_au
        ))]));
    }
    let l = ligne_unique(ctx, &rubrique, montant)?;
    Ok((Some(l), vec![avertir(format!(
        "{} : {} ; le contrat finit le {} : délai non effectué {}, payé {} tel que saisi (L1221-25, indemnité compensatrice ; \
         DSN 52 : 017). {}",
        qui,
        dit,
        fin.jj_mm_aaaa(),
        du_au,
        euros(montant),
        conges
    ))]))
}

/// LE SALAIRE D'UN JOUR OUVRABLE au maintien (L3141-24, II) : le salaire mensuel `salaire` reparti sur les jours
/// ouvrables du mois entier `mois` (`droit::conges::jours_ouvrables`). Un mois en compte toujours (six jours par semaine,
/// hors les feries chomes) : aucun est un defaut du calendrier, qui remonte.
pub(crate) fn salaire_d_un_jour_ouvrable(salaire: MontantUnitaire, mois: Mois, chomes: Option<&str>) -> R<MontantUnitaire> {
    let jours = NonNul::de(jours_ouvrables(&Periode::du_mois(mois), chomes)?).ok_or_else(|| socle::refus!("Défaut du logiciel : le mois {} n'a aucun jour ouvrable.", mois))?;
    Ok(salaire.par_jour(jours))
}

/// LES CONGES D'UN TEMPS PAYE SANS ETRE EFFECTUE : le delai de prevenance (L1221-25) et le preavis (L1234-5) sont payes
/// « indemnité compensatrice de congés payés comprise ». Les `jours` que ce temps aurait acquis (L3141-3), payes au plus
/// favorable du dixieme de ce qu'il paie (`montant`, sa remuneration) et du maintien (le salaire du mois `mois` sur ses
/// jours ouvrables, comme le solde ; L3141-24), s'ajoutent a la ligne de l'indemnite compensatrice de conges payes du
/// solde, ou en font une. Un salarie dont la caisse des Conges Spectacles paie les conges n'en a pas chez l'employeur
/// (L3141-30). Rend la phrase qui le dit.
fn conges_non_effectues(
    ctx: &Contexte, e: &Carte, mois: Mois, fin: &socle::date::Jour, jours: Jours, salaire: MontantUnitaire, montant: Montant,
    stc: &mut Solde,
) -> R<String> {
    if crate::calcul::saisie_declaree_a_la_caisse_des_conges(ctx, e, fin)? {
        return Ok("ses congés sont payés par la caisse des Congés Spectacles (L3141-30), rien à ajouter.".to_string());
    }
    if jours.est_nul() {
        return Ok("ce temps n'acquiert aucun jour de congé.".to_string());
    }
    let par_jour = salaire_d_un_jour_ouvrable(salaire, mois, ctx.entreprise.feries_chomes())?;
    let ind = permanents::indemnite_conges(jours, montant, jours, (jours * par_jour).au_centime())?;
    let rubrique = permanents::regles()?.conges_payes.indemnite_compensatrice.rubrique.clone();
    let (rang, total_jours, total) = match stc.conges_payes {
        Some((rang, j, m)) => (rang, j + jours, m + ind.retenu),
        None => (stc.lignes.len(), jours, ind.retenu),
    };
    let l = ligne(&rubrique, PaiementDeLigne::Jours(total_jours, taux_au_montant(total, total_jours)?));
    match stc.lignes.get_mut(rang) {
        Some(x) => *x = l,
        None => stc.lignes.push(l),
    }
    stc.conges_payes = Some((rang, total_jours, total));
    Ok(format!(
        "ces jours acquièrent {} jour(s) de congé (L3141-3), payés {} ({}, L3141-24), ajoutés à l'indemnité compensatrice de \
         congés payés.",
        socle::texte::virgule(jours.au_demi_superieur()),
        euros(ind.retenu),
        ind.regle
    ))
}

/// Les avantages en nature que les lignes du mois `deja` portent (leur libelle).
fn avantages_du_mois(ctx: &Contexte, deja: &[LignePaie]) -> R<Vec<String>> {
    let mut out = Vec::new();
    for l in deja {
        let r = rubrique(&ctx.racine, l.rubrique(), &ctx.lue_au)?;
        if r.nature == droit::catalogue::NatureDeRubrique::AvantageNature {
            out.push(r.libelle);
        }
    }
    Ok(out)
}

/// Ce que la convention donne pour une rupture qui n'est pas un licenciement : de `bas` a `haut` (egaux quand le texte n'a
/// qu'une lecture), son article, ce qui manque pour le dire, et le delai de prevenance d'une retraite.
struct Conventionnelle {
    releve: bool,
    article: Option<String>,
    bas: Option<Montant>,
    haut: Option<Montant>,
    pourquoi: Option<String>,
    prevenance_mois: Option<socle::date::Duree<socle::date::EnMois>>,
}

impl Conventionnelle {
    fn de_la_retraite(r: clauses::Retraite) -> Conventionnelle {
        Conventionnelle { releve: r.releve, article: r.article, bas: r.montant, haut: r.montant, pourquoi: r.pourquoi, prevenance_mois: r.prevenance_mois }
    }
}

/// Ce que la fin d'un contrat ouvre au salarie, selon sa cause : licenciement,
/// rupture conventionnelle (043), mise a la retraite (038), depart (039).
fn indemnite_de_rupture(
    ctx: &Contexte,
    e: &Carte,
    mois: Mois,
    anciennete: Anciennete,
    motif: Option<MotifDeRupture>,
    nat: &Nature,
    stc: &mut Solde,
) -> R<()> {
    let qui = qui(ctx, e);
    let Some((fin_cdi, rubrique_sec)) = permanents::rupture(motif)? else {
        return indemnite_de_licenciement(ctx, e, mois, anciennete, motif, nat, stc);
    };
    if !nat.duree_indeterminee {
        return Err(socle::refus!(
            "{} : motif de rupture {} ({}) sur un {}. Ces ruptures-la sont \
             celles d'un contrat a duree INDETERMINEE -- leurs articles sont \
             au titre III du livre II, qui ne traite que de lui.",
            qui,
            match motif {
                Some(m) => m.to_string(),
                None => "non dit".to_string(),
            },
            fin_cdi.cle().replace('_', " "),
            nat.libelle
        ));
    }
    let mut lue = None;
    let mut retenu = || -> R<Montant> { Ok(salaire_de_reference(&mut lue, ctx, e, mois)?.retenu) };
    let mut rubrique = rubrique_sec.clone();
    // Le montant de ces indemnites ne se saisit pas : la carte n'a de case que pour celle du licenciement
    // (`indemnite_licenciement`).
    let saisi: Option<Montant> = None;
    let idcc = idcc(ctx);
    let prof = profil(ctx, e)?;
    let (mut conv_bas, mut conv_haut): (Option<Montant>, Option<Montant>) = (None, None);
    let mut conv_article: Option<String> = None;
    let mut conv_releve = false;
    let mut ouvert = false;
    let mut supplement_rub = format!("{}_conventionnelle", rubrique_sec);
    // Le palier de la loi (en annees) et ses mois de salaire, au depart a la retraite.
    let mut palier: Option<(Anciennete, socle::grandeurs::MoisDeSalaire)> = None;
    let (legale, article, quoi, c): (permanents::IndemniteLegale, &str, &str, Conventionnelle);

    if fin_cdi == permanents::FinDeCdi::DepartALaRetraite {
        let depart = permanents::indemnite_depart_retraite(anciennete, &mut retenu)?;
        palier = depart.palier.filter(|p| *p != Anciennete::ZERO).map(|p| (p, depart.mois));
        legale = permanents::IndemniteLegale { montant: depart.montant, du: depart.du, pourquoi: depart.pourquoi, annees: None };
        article = "L1237-9 et D1237-1";
        quoi = "depart a la retraite";
        c = Conventionnelle::de_la_retraite(clauses::indemnite_depart_retraite(idcc, anciennete, &mut retenu)?);
    } else if fin_cdi == permanents::FinDeCdi::MiseALaRetraite {
        legale = permanents::indemnite_mise_a_la_retraite(anciennete, &mut retenu)?;
        article = "L1237-7";
        quoi = "mise a la retraite";
        stc.avertissements.push(avertir(format!(
            "{} : MISE A LA RETRAITE -- la condition d'age n'est pas \
             verifiee par le moteur (L1237-5 renvoie a L351-8 du code de la \
             securite sociale). Si elle n'est pas remplie, << la rupture du \
             contrat de travail par l'employeur constitue un licenciement >> \
             (L1237-8), et ce qui est declare ici serait faux.",
            qui
        )));
        let mise = clauses::indemnite_mise_a_la_retraite(idcc, anciennete, &mut retenu)?;
        let vise = prof.categorie == Some(droit::fiche::CategorieConventionnelle::CadreDeDirection) && ctx.entreprise.annexe_convention.dite() == Some(&droit::fiche::AnnexeConvention::A1) && anciennete > Anciennete::ans(10);
        if let Some(cdd) = mise.cadre_de_direction.as_ref().filter(|_| vise) {
            if saisi.is_none() {
                return Err(socle::echec::cle(format!(
                    "{} : mise a la retraite d'un cadre de direction de plus \
                     de dix ans d'anciennete -- {} ({}) fixe l'indemnite a << 6 \
                     mois de salaires minimum >>, et le texte ne dit pas quel \
                     salaire. Saisir le montant retenu : le moteur \
                     l'accepte a partir du minimum legal.",
                    qui,
                    clauses::nom(idcc),
                    cdd
                ), rubrique.as_str()));
            }
            ouvert = true;
        }
        c = Conventionnelle::de_la_retraite(mise);
    } else {
        let rc = permanents::indemnite_rupture_conventionnelle(anciennete, &mut retenu)?;
        if let (Some(_), Some(pourquoi)) = (rc.plancher, rc.pourquoi.as_deref().filter(|p| !p.is_empty())) {
            stc.avertissements.push(info(format!("{} : {}", qui, pourquoi)));
        }
        legale = permanents::IndemniteLegale { montant: du_ou_rien(rc.plancher), du: rc.plancher.is_some(), pourquoi: rc.pourquoi, annees: None };
        article = "L1237-13";
        quoi = "rupture conventionnelle";
        supplement_rub = "indemnite_rupture_conventionnelle_supplement".into();
        // 3090, art. 7.6 : le plancher est l'indemnite CONVENTIONNELLE de
        // licenciement, et non celle du code du travail.
        let lic = indemnite_conventionnelle(ctx, e, anciennete, &mut retenu, motif)?;
        let rc_sec = clauses::releve(idcc)?.and_then(|r| r.rupture_conventionnelle);
        let mut cc = Conventionnelle { releve: lic.releve, article: lic.article.clone(), bas: None, haut: None, pourquoi: lic.pourquoi.clone(), prevenance_mois: None };
        if let (true, Some(rc_sec)) = (lic.du, rc_sec) {
            cc.bas = lic.bas;
            cc.haut = lic.haut;
            let renvoi = lic.article.as_deref().ok_or_else(|| socle::echec::refus("l'indemnité conventionnelle de licenciement est due sans dire son article"))?;
            cc.article = Some(format!("{} (art. {}, qui renvoie a {})", clauses::nom(idcc), rc_sec.article, renvoi));
        }
        c = cc;
    }
    if c.releve {
        conv_releve = true;
        conv_article = c.article.clone();
        conv_bas = c.bas;
        conv_haut = c.haut;
    }
    let conv_pourquoi = c.pourquoi.clone();
    // Une indemnite legale qui n'est pas due dit pourquoi (`permanents`).
    let pourquoi_legal = || legale.pourquoi.as_deref().ok_or_else(|| socle::refus!("{} : l'indemnité légale de {} n'est pas due et ne dit pas pourquoi", qui, quoi));

    let du_legal = legale.du;
    let m_legal = if du_legal { legale.montant } else { Montant::ZERO };
    let bas = plus_grand(m_legal, du_ou_rien(conv_bas.filter(|b| !b.est_nul())));
    let haut = plus_grand(m_legal, du_ou_rien(conv_haut.filter(|b| !b.est_nul())));
    let du = du_legal || bas > Montant::ZERO;
    let phrase = phrase_de_la_convention(conv_releve, conv_article.as_deref(), conv_bas, conv_haut, m_legal, conv_pourquoi.as_deref(), idcc);

    let montant;
    if let Some(saisi) = saisi {
        montant = saisi;
        if !du {
            return Err(socle::refus!(
                "{} : indemnité de {} de {} saisie, et ni la loi ni la \
                 convention ne donnent de montant à quoi la comparer -- {}. \
                 Le moteur ne déclare pas une indemnité dont il ne sait pas \
                 dire ce qu'elle supporte.",
                qui,
                quoi,
                euros(montant),
                pourquoi_legal()?
            ));
        }
        if montant < bas {
            return Err(socle::refus!(
                "{} : indemnité de {} de {}, INFÉRIEURE au minimum de {} \
                 (loi, {} : {} ; {}). Ce n'est pas au moteur de la \
                 corriger.",
                qui, quoi, euros(montant), euros(bas), article, euros(m_legal), phrase
            ));
        }
        if montant > haut && !ouvert {
            return Err(socle::refus!(
                "{} : indemnité de {} de {}, alors que le montant LÉGAL \
                 est de {} ({}) -- {}. La part au-delà a un autre régime \
                 social -- CSG et CRDS dues dès le premier euro qui dépasse \
                 le montant légal ou conventionnel -- et le moteur ne sait pas \
                 l'assujettir sur une seule ligne : il refuse plutôt que de la \
                 déclarer de travers.",
                qui, quoi, euros(montant), euros(m_legal), article, phrase
            ));
        }
    } else if let (Some(cb), Some(ch)) = (conv_bas, conv_haut) {
        if cb != ch && ch > m_legal && fin_cdi != permanents::FinDeCdi::RuptureConventionnelle {
            return Err(socle::echec::cle(format!(
                "{} : {} se lit de deux façons, de {} à {} ; la loi donne {}. \
                 Saisir le montant retenu.",
                qui,
                l_article(conv_article.as_deref(), idcc),
                euros(cb),
                euros(ch),
                euros(m_legal)
            ), rubrique.as_str()));
        }
        montant = bas;
    } else {
        montant = bas;
    }

    if !du {
        stc.avertissements.push(info(format!(
            "{} : pas d'indemnite legale de {} -- {} Et {}.",
            qui,
            quoi,
            pourquoi_legal()?,
            phrase
        )));
        return Ok(());
    }
    if fin_cdi == permanents::FinDeCdi::RuptureConventionnelle && saisi.is_none() {
        // Le montant est celui de la convention signee, que le moteur n'a pas.
        stc.avertissements.push(avertir(format!(
            "{} : RUPTURE CONVENTIONNELLE -- l'indemnité spécifique est due \
             et n'a PAS été écrite : son montant est celui de la convention \
             signée, que le moteur n'a pas. Le plancher légal est de {} \
             ({}) ; {}. Ajouter à la paie une ligne « {} » avec le montant \
             convenu.",
            qui,
            euros(m_legal),
            article,
            phrase,
            libelle_rubrique(ctx, &rubrique)?
        )));
        return Ok(());
    }
    if fin_cdi == permanents::FinDeCdi::RuptureConventionnelle {
        // Deux rubriques pour une seule indemnite : l'impot depend d'un fait du salarie.
        let Some(droit) = e.en_droit_a_la_retraite else {
            return Err(socle::echec::cle(format!(
                "{} : rupture conventionnelle -- il manque un fait, et il \
                 change l'impot du salarie : est-il EN DROIT DE BENEFICIER \
                 D'UNE PENSION de retraite d'un regime legalement \
                 obligatoire ? S'il ne l'est pas, son indemnite n'est pas \
                 imposable (CGI 80 duodecies 6°) ; s'il l'est, elle l'est. Le dire sur l'engagement.",
                qui
            ), "en_droit_a_la_retraite"));
        };
        if droit {
            rubrique = const { droit::catalogue::CodeDeRubrique::du_moteur("indemnite_rupture_conventionnelle_imposable") };
            supplement_rub = "indemnite_rupture_conventionnelle_supplement_imposable".into();
        }
    }
    if montant <= Montant::ZERO {
        stc.avertissements.push(info(format!(
            "{} : pas d'indemnite de {} -- {} Et {}.",
            qui,
            quoi,
            pourquoi_legal()?,
            phrase
        )));
        return Ok(());
    }
    let reference = salaire_de_reference(&mut lue, ctx, e, mois)?;
    verifier_les_plafonds(ctx, mois, montant, quoi, &qui)?;
    let part_legale = plus_petit(montant, m_legal);
    let supplement = montant - part_legale;
    if part_legale > Montant::ZERO {
        stc.lignes.push(ligne_unique(ctx, &rubrique, part_legale)?);
    }
    if supplement > Montant::ZERO {
        stc.lignes.push(ligne_unique(ctx, &droit::catalogue::CodeDeRubrique::lire(&supplement_rub)?, supplement)?);
    }
    // L137-12 : une contribution patronale sur la part exoneree, due et non calculee.
    let regles = permanents::regles()?;
    let contrib = &regles.regime_des_indemnites_de_rupture.contribution_patronale;
    if let Some(m) = motif {
        if contrib.motifs.contains(&m) {
            let taux = contrib.taux;
            stc.avertissements.push(avertir(format!(
                "{} : l'employeur doit en plus la CONTRIBUTION DE {} % sur la \
                 part exonérée de cette indemnité (L137-12 du code de la \
                 sécurité sociale, au profit de la CNAV). Le moteur ne la \
                 calcule PAS : elle n'est dans aucune section du barème. Sur {} \
                 exonérés, elle vaut {}.",
                qui,
                socle::texte::virgule(taux),
                euros(montant),
                euros(montant.fois_taux(taux))
            )));
        }
    }
    let detail = match palier {
        Some((ans, mois)) => format!(" -- palier de {} ans de la loi, {} mois de salaire", ans.ans_revolus_dits(), mois.ecrits_sans_zero()),
        None => String::new(),
    };
    stc.avertissements.push(info(format!(
        "{} : indemnité de {} {} ({} mois d'ancienneté, salaire de \
         référence {} par {}){} -- loi ({}) : {} ; {}.{}",
        qui,
        quoi,
        euros(montant),
        anciennete.au_demi_mois_superieur().mois_dits(),
        euros(reference.retenu),
        reference.regle,
        detail,
        article,
        euros(m_legal),
        phrase,
        if supplement > Montant::ZERO {
            format!(" Le supplément de {} se déclare à part, sous son code conventionnel.", euros(supplement))
        } else {
            String::new()
        }
    )));
    if let Some(prevenance) = c.prevenance_mois.filter(|m| !m.nulle() && fin_cdi != permanents::FinDeCdi::RuptureConventionnelle) {
        stc.avertissements.push(avertir(format!(
            "{} : {} impose un DELAI DE PREVENANCE de {}, par lettre \
             recommandee, a qui prend l'initiative du depart a la retraite. \
             Le moteur ne le verifie pas.",
            qui,
            l_article(conv_article.as_deref(), idcc),
            prevenance
        )));
    }
    Ok(())
}

/// Ce que la convention a dit d'une indemnite de rupture, en une phrase.
fn phrase_de_la_convention(
    releve: bool,
    article: Option<&str>,
    bas: Option<Montant>,
    haut: Option<Montant>,
    m_legal: Montant,
    pourquoi: Option<&str>,
    idcc: Option<&droit::fiche::Idcc>,
) -> String {
    if !releve {
        return format!("les clauses de {} ne sont pas relevees : seule la loi est calculee", clauses::nom(idcc));
    }
    let article = l_article(article, idcc);
    let Some(bas) = bas else {
        return format!(
            "{} n'ouvre rien ici{}",
            article,
            pourquoi.filter(|p| !p.is_empty()).map(|p| format!(" ({})", p)).into_iter().collect::<String>()
        );
    };
    if let Some(haut) = haut.filter(|h| *h != bas) {
        format!("{} donne de {} à {} selon la lecture", article, euros(bas), euros(haut))
    } else if bas == m_legal {
        format!("{} donne la même chose", article)
    } else if bas > m_legal {
        format!("{} donne {}, et c'est elle qui s'applique", article, euros(bas))
    } else {
        format!("{} donne moins ({}) : la loi s'applique", article, euros(bas))
    }
}

/// Les deux plafonds de L242-1 II 7° : au-dela, le moteur refuse.
fn verifier_les_plafonds(ctx: &Contexte, mois: Mois, montant: Montant, quoi: &str, qui: &str) -> R<()> {
    let regles = permanents::regles()?;
    let regles_plafonds = &regles.regime_des_indemnites_de_rupture.plafonds;
    // Le plafond annuel : douze plafonds mensuels.
    let annuel = ctx.plafond_securite_sociale(mois.annee())?.mensuel().fois(12);
    for (combien, quel) in [
        (regles_plafonds.cotisations_exoneration_pass, "exoneree"),
        (regles_plafonds.cotisations_assujettissement_total_pass, "hors de l'assiette"),
    ] {
        let seuil = annuel * combien;
        if montant > seuil {
            return Err(socle::refus!(
                "{} : indemnité de {} de {}, au-delà de {} plafonds \
                 annuels de Sécurité sociale ({}). Elle n'est plus {} \
                 en entier (L242-1 II 7° du code de la sécurité sociale) et \
                 le moteur ne sait pas partager une ligne entre une part \
                 exonérée et une part soumise : il refuse.",
                qui,
                quoi,
                euros(montant),
                socle::grandeurs::ecrit::sans_zero::dit(combien),
                euros(seuil),
                quel
            ));
        }
    }
    Ok(())
}

/// L'indemnite de licenciement (L1234-9, R1234-2) : la loi sous 007, ce que la
/// convention donne de plus sous 021.
fn indemnite_de_licenciement(
    ctx: &Contexte,
    e: &Carte,
    mois: Mois,
    anciennete: Anciennete,
    motif: Option<MotifDeRupture>,
    nat: &Nature,
    stc: &mut Solde,
) -> R<()> {
    let qui = qui(ctx, e);
    let saisi = e.indemnite_licenciement;
    if saisi.is_none() && !(nat.duree_indeterminee && motif.is_some()) {
        return Ok(());
    }
    let mut lue = None;
    let legale = permanents::indemnite_licenciement(anciennete, &mut || Ok(salaire_de_reference(&mut lue, ctx, e, mois)?.retenu), motif)?;
    let conv = indemnite_conventionnelle(ctx, e, anciennete, &mut || Ok(salaire_de_reference(&mut lue, ctx, e, mois)?.retenu), motif)?;
    let m_legal = if legale.du { legale.montant } else { Montant::ZERO };
    let conv_du = conv.du;
    let (conv_bas, conv_haut) = (conv.bas, conv.haut);
    let bas = plus_grand(m_legal, if conv_du { du_ou_rien(conv_bas) } else { Montant::ZERO });
    let haut = plus_grand(m_legal, if conv_du { du_ou_rien(conv_haut) } else { Montant::ZERO });

    let montant;
    if let Some(saisi) = saisi {
        montant = saisi;
        if montant > haut {
            return Err(socle::refus!(
                "{} : indemnité de licenciement de {}, alors que \
                 l'indemnité LÉGALE est de {}{}. La part supra-légale et \
                 supra-conventionnelle a un autre régime social (CSG/CRDS \
                 dues au-delà, cotisations au premier euro au-delà de dix \
                 plafonds annuels) et le moteur ne sait pas l'assujettir : il \
                 refuse plutôt que de la déclarer de travers.",
                qui,
                euros(montant),
                euros(m_legal),
                dit_conventionnel(&conv)?
            ));
        }
        if montant < bas {
            return Err(socle::refus!(
                "{} : indemnité de licenciement de {}, INFÉRIEURE à ce \
                 qui est dû ({} : loi {}{}).",
                qui,
                euros(montant),
                euros(bas),
                euros(m_legal),
                dit_conventionnel(&conv)?
            ));
        }
    } else {
        if conv_du && conv_bas != conv_haut && conv_haut.is_some_and(|h| h > m_legal) {
            return Err(socle::refus!(
                "{} : {} se lit de deux façons, qui ne donnent pas le même \
                 montant : {}. La loi donne {}. Ajouter à la paie une \
                 ligne « Indemnité de licenciement » avec le montant retenu, \
                 entre {} et {}.",
                qui,
                article_de(&conv)?,
                lectures(&conv)?,
                euros(m_legal),
                euros(bas),
                euros(haut)
            ));
        }
        montant = bas;
    }
    if montant <= Montant::ZERO {
        let pourquoi = legale.pourquoi.clone().filter(|p| !p.is_empty()).or_else(|| conv.pourquoi.clone().filter(|p| !p.is_empty()));
        if let Some(pourquoi) = pourquoi {
            stc.avertissements.push(info(format!("{} : pas d'indemnite de licenciement -- {}", qui, pourquoi)));
        }
        return Ok(());
    }
    let reference = salaire_de_reference(&mut lue, ctx, e, mois)?;
    verifier_les_plafonds(ctx, mois, montant, "licenciement", &qui)?;
    let part_legale = plus_petit(montant, m_legal);
    if part_legale > Montant::ZERO {
        stc.lignes.push(ligne_unique(ctx, &permanents::regles()?.indemnite_licenciement.rubrique, part_legale)?);
    }
    let supplement = montant - part_legale;
    if supplement > Montant::ZERO {
        stc.lignes.push(ligne_unique(ctx, &const { droit::catalogue::CodeDeRubrique::du_moteur("indemnite_licenciement_conventionnelle") }, supplement)?);
    }
    stc.avertissements.push(info(format!(
        "{} : indemnité de licenciement {} ({} mois d'ancienneté, salaire \
         de référence {} par {}) -- loi (L1234-9 et R1234-2) : {} ; {}.{}",
        qui,
        euros(montant),
        anciennete.au_demi_mois_superieur().mois_dits(),
        euros(reference.retenu),
        reference.regle,
        euros(m_legal),
        phrase_conventionnelle(&conv, m_legal)?,
        if supplement > Montant::ZERO {
            format!(" Le supplément de {} se déclare à part, en 021.", euros(supplement))
        } else {
            String::new()
        }
    )));
    if conv.hors_primes && supplement > Montant::ZERO {
        stc.avertissements.push(avertir(format!(
            "{} : l'assiette de {} est le salaire de BASE, hors primes ; le \
             logiciel a pris les bruts des derniers mois. S'ils comptent des \
             primes, ajouter a la paie une ligne « Indemnite de \
             licenciement » avec le montant recalcule.",
            qui,
            article_de(&conv)?
        )));
    }
    Ok(())
}

/// L'article d'une indemnite que la convention releve (`clauses::indemnite_licenciement`) : une convention relevee le dit.
fn article_de(conv: &clauses::Indemnite) -> R<&str> {
    conv.article.as_deref().ok_or_else(|| socle::echec::livree(&droit::clauses::FICHIER, "indemnite_licenciement.article"))
}

/// Le nom qu'un administrateur voit dans la liste des rubriques.
fn libelle_rubrique(ctx: &Contexte, code: &droit::catalogue::CodeDeRubrique) -> R<String> {
    Ok(rubrique(&ctx.racine, code, &ctx.lue_au)?.libelle)
}

/// L'indemnite de licenciement de la convention de la compagnie.
fn indemnite_conventionnelle(
    ctx: &Contexte,
    e: &Carte,
    anciennete: Anciennete,
    reference: &mut dyn FnMut() -> R<Montant>,
    motif: Option<MotifDeRupture>,
) -> R<clauses::Indemnite> {
    clauses::indemnite_licenciement(
        idcc(ctx),
        ctx.entreprise.annexe_convention.dite().copied(),
        &profil(ctx, e)?,
        anciennete,
        reference,
        motif,
        age(ctx, e)?,
    )
}

fn lectures(conv: &clauses::Indemnite) -> R<String> {
    let l: Vec<String> = conv.lectures.iter().map(|(n, v)| format!("{} {}", n.replace('_', " "), euros(*v))).collect();
    Ok(l.join(" ; "))
}

/// Les montants d'une indemnite que la convention doit (`du`).
fn bas_et_haut(conv: &clauses::Indemnite) -> R<(Montant, Montant)> {
    let manque = || socle::echec::livree(&droit::clauses::FICHIER, "indemnite_licenciement");
    Ok((conv.bas.ok_or_else(manque)?, conv.haut.ok_or_else(manque)?))
}

fn dit_conventionnel(conv: &clauses::Indemnite) -> R<String> {
    if !conv.releve {
        return Ok(String::new());
    }
    let article = article_de(conv)?;
    if !conv.du {
        return Ok(format!(" et rien par {}", article));
    }
    let (bas, haut) = bas_et_haut(conv)?;
    Ok(if bas == haut {
        format!(" et {} par {}", euros(bas), article)
    } else {
        format!(" et de {} à {} par {}", euros(bas), euros(haut), article)
    })
}

/// Ce que la convention a dit, pour l'avertissement.
fn phrase_conventionnelle(conv: &clauses::Indemnite, m_legal: Montant) -> R<String> {
    if !conv.releve {
        let pourquoi = match &conv.pourquoi {
            Some(p) => p.clone(),
            None => "les clauses de la convention ne sont pas relevees".to_string(),
        };
        return Ok(format!("{} : seule la loi est calculee", pourquoi));
    }
    let article = article_de(conv)?;
    if !conv.du {
        let pourquoi = conv.pourquoi.as_deref().ok_or_else(|| socle::echec::livree(&droit::clauses::FICHIER, "indemnite_licenciement.pourquoi"))?;
        return Ok(format!("{} n'ouvre rien ici ({})", article, pourquoi));
    }
    let bas = bas_et_haut(conv)?.0;
    Ok(if bas == m_legal {
        format!("{} donne la même chose", article)
    } else if bas > m_legal {
        format!("{} donne {}, et c'est elle qui s'applique", article, euros(bas))
    } else {
        format!("{} donne moins ({}) : la loi s'applique", article, euros(bas))
    })
}

/// Le salaire de reference des indemnites de fin de contrat (R1234-4), sur les bruts des derniers mois. Il se lit une
/// fois, et seulement quand une indemnite le demande (`lue` le garde) : sans paies precedentes il refuse, et une
/// indemnite qui n'est pas due ne doit pas refuser pour lui.
fn salaire_de_reference(lue: &mut Option<permanents::SalaireDeReference>, ctx: &Contexte, e: &Carte, mois: Mois) -> R<permanents::SalaireDeReference> {
    if let Some(r) = lue {
        return Ok(r.clone());
    }
    let (douze, trois) = bruts_precedents(ctx, e, mois)?;
    let r = permanents::salaire_de_reference(&douze, &trois)?;
    *lue = Some(r.clone());
    Ok(r)
}

/// Les bruts mensuels des douze et des trois derniers mois (R1234-4, `atelier/reference/sources/code-du-travail/
/// articles-20261110.md` : « la moyenne mensuelle des douze derniers mois précédant le licenciement, ou lorsque la durée
/// de service du salarié est inférieure à douze mois, la moyenne mensuelle de la rémunération de l'ensemble des mois
/// précédant le licenciement » ; « le tiers des trois derniers mois »). Les mois qui comptent sont ceux du service du
/// salarie, douze au plus avant `mois`. INFERENCE : sa « duree de service » part du debut du contrat de la carte, comme
/// l'anciennete de l'indemnite elle-meme (`contrat.anciennete()`, plus haut) : les deux lisent la meme notion par le meme
/// jour. Le texte ne dit pas si un contrat anterieur dans l'entreprise (un CDD suivi d'un CDI) entre dans ce service :
/// il n'y entre pas ici. Un mois d'avant ce jour ne compte pas, que d'autres salaries y soient payes ou non. Chaque
/// mois se lit par `brut_d_un_mois_de_service`.
fn bruts_precedents(ctx: &Contexte, e: &Carte, mois: Mois) -> R<(Vec<Montant>, Vec<Montant>)> {
    let salarie = salarie_de(e)?;
    let depuis = debut_de(e)?;
    let refus = |pourquoi: String| {
        socle::refus!(
            "Le salaire de référence de l'indemnité de fin de contrat (R1234-4) se calcule sur les bruts des douze et des trois \
             derniers mois : {}, puis refaire la fin du contrat.",
            pourquoi
        )
    };
    let mut bruts = Vec::new();
    for i in 1..=12 {
        let m = mois.avant(socle::date::Duree::mois(i))?;
        if m < depuis.mois_civil() {
            break;
        }
        bruts.push(brut_d_un_mois_de_service(ctx, e, salarie, m, depuis)?.map_err(refus)?);
    }
    let trois = bruts.iter().take(3).copied().collect();
    Ok((bruts, trois))
}

/// Le brut que portent des lignes deja construites, sans les charger : la
/// meme somme que `calcul.calculer`.
fn brut_des_lignes(ctx: &Contexte, lignes: &[LignePaie]) -> R<Montant> {
    let mut total = Montant::ZERO;
    for l in lignes {
        let r = rubrique(&ctx.racine, l.rubrique(), &ctx.lue_au)?;
        if !r.soumis {
            continue;
        }
        total += r.sens.de(l.paiement(r.unite)?.montant().au_centime());
    }
    Ok(total)
}

/// La remuneration totale brute versee pendant le contrat (L1243-8, « la rémunération totale brute versée au salarié »,
/// que l'employeur l'ait payee ici ou dans l'ancien logiciel) : le brut verse chaque mois d'avant depuis le debut du
/// contrat (`brut_d_un_mois_de_service`), plus le mois courant, dont le brut arrive tout calcule.
fn remuneration_du_contrat(ctx: &Contexte, e: &Carte, mois: Mois, ce_mois: Montant) -> R<Montant> {
    let salarie = salarie_de(e)?;
    let mut total = ce_mois;
    let debut = debut_de(e)?;
    let refus = |pourquoi: String| {
        socle::refus!("L'indemnité de fin de contrat (L1243-8) se calcule sur la rémunération totale brute du contrat : {}, puis refaire la fin du contrat.", pourquoi)
    };
    for m in debut.mois_civil().jusqu_a(mois).into_iter().filter(|m| *m != mois) {
        total += brut_d_un_mois_de_service(ctx, e, salarie, m, debut)?.map_err(refus)?;
    }
    Ok(total)
}

/// LE BRUT VERSE AU SALARIE `salarie` DE LA CARTE `e` EN `m`, UN MOIS QUE LE DEBUT `debut` DE SON CONTRAT FAIT COMPTER : la somme des bruts de
/// ses paies telles qu'un calcul d'un autre mois les lit (`conges::paies_versees`, le seul chemin) -- validees ici, ou
/// calculees comme leur validation les calculera pour un engagement en saisie (`etats::CommeALaValidation`, ouvert par
/// `charger_donnees`) ou en cours de validation (ses mois deja calcules), et reprises de l'ancien logiciel. Le salaire de
/// reference et la remuneration du contrat le lisent, et lui seul.
///
/// `Err(pourquoi)`, que le calcul qui le lit refuse avec sa raison : aucune paie de ce mois n'est connue (une absence
/// n'est pas un zero), et le refus dit les trois gestes qui la font connaitre (valider l'engagement d'ici qui le paie,
/// importer la paie de l'ancien logiciel, ou corriger le debut du contrat) ; ou une paie reprise ne dit pas
/// son brut.
fn brut_d_un_mois_de_service(
    ctx: &Contexte,
    e: &Carte,
    salarie: &droit::contexte::IdSalarie,
    m: Mois,
    debut: socle::date::Jour,
) -> R<Result<Montant, String>> {
    let paies = paies_versees(ctx, m, salarie)?;
    if paies.is_empty() {
        return Ok(Err(format!(
            "{} compte pour {} (le début du contrat : le {}), et aucune paie de ce mois n'est connue, ni validée ici ni reprise de \
             l'ancien logiciel ; si un engagement d'ici le paie, le valider ; si l'ancien logiciel l'a payé, importer \
             cette paie ; sinon, corriger le début du contrat",
            m.libelle(socle::mois::Graphie::Accentuee),
            qui(ctx, e),
            debut.jj_mm_aaaa()
        )));
    }
    let brut: Option<Montant> = paies.iter().map(crate::etats::PaieAnterieure::brut).sum();
    Ok(brut.ok_or_else(|| brut_non_dit(m)))
}
