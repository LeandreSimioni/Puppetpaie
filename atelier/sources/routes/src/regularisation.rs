//! Les corrections des mois deja declares : la liste des corrections (`corrections_par_mois`, route `regularisations`) ;
//! et ce que la saisie d'un mois declare en dit (`modification_de_l_engagement`). Rien n'est ecrit.
//!
//! Corriger, c'est modifier l'engagement et le revalider. Ce qui est a corriger se tire de la comparaison entre la
//! version presente de l'engagement valide et celle sur laquelle la DSN est partie
//! (`produire_regularisations::corrections_dues`).

use serde::Serialize;

use socle::date::Jour;
use socle::echec::Classe;

use engagement::calcul::Paie;
use engagement::numeros::NumeroContrat;
use droit::contexte::Contexte;
use socle::echec::R;
use socle::mois::{Graphie, Mois};
use socle::num::{q2, Decimal};
use crate::etapes::Moteur;

// ------------------------------------------------------------------ le mois d'origine

/// Le resume d'une paie, comme la DSN la declare : ses montants au centime, son activite.
#[derive(Debug, Serialize)]
pub(crate) struct Resume {
    brut: String,
    net_a_payer: String,
    net_imposable: String,
    cotisations_salariales: String,
    cotisations_patronales: String,
    cout_employeur: String,
    cachets: i64,
    /// Les heures au centieme ; `None` : un engagement qui ne compte pas d'heures.
    heures: Option<String>,
    jours: i64,
    remuneration: Vec<String>,
}

/// Ce que la correction declare : l'ecart de chaque montant et de l'activite, de la paie declaree a la presente.
#[derive(Debug, Serialize)]
pub(crate) struct Ecart {
    brut: String,
    net_a_payer: String,
    net_imposable: String,
    cotisations_salariales: String,
    cotisations_patronales: String,
    cout_employeur: String,
    cachets: i64,
    /// `None` : l'une des deux paies ne compte pas d'heures.
    heures: Option<String>,
}

/// Les montants d'une paie au centime, dans l'ordre de `Resume`.
fn montants(p: &Paie) -> [Decimal; 6] {
    [p.brut, p.net_a_payer, p.net_imposable, p.total_sal, p.total_pat, p.cout_employeur].map(|x| x.round_dp(2))
}

/// Les heures d'une paie au centieme : la somme flottante des lignes (151,67 - 63) ne se montre pas avec ses residus.
fn heures(p: &Paie) -> R<Option<Decimal>> {
    match p.engagement.heures {
        Some(h) => Ok(Some(h.round_dp(2).normalize())),
        None => Ok(None),
    }
}

fn resume(p: &Paie) -> R<Resume> {
    let [brut, net_a_payer, net_imposable, cotisations_salariales, cotisations_patronales, cout_employeur] = montants(p).map(q2);
    Ok(Resume {
        brut,
        net_a_payer,
        net_imposable,
        cotisations_salariales,
        cotisations_patronales,
        cout_employeur,
        cachets: p.engagement.cachets,
        heures: heures(p)?.map(|h| h.to_string()),
        jours: p.engagement.jours,
        remuneration: p.remuneration.iter().map(|l| l.designation()).collect(),
    })
}

/// L'ecart de `avant` a `apres` : la page le montre sans le calculer (regle 2).
fn ecart(avant: &Paie, apres: &Paie) -> R<Ecart> {
    let (a, b) = (montants(avant), montants(apres));
    let [brut, net_a_payer, net_imposable, cotisations_salariales, cotisations_patronales, cout_employeur] = std::array::from_fn(|i| (b[i] - a[i]).to_string());
    let heures = match (heures(avant)?, heures(apres)?) {
        (Some(x), Some(y)) => Some((y - x).normalize().to_string()),
        _ => None,
    };
    Ok(Ecart {
        brut,
        net_a_payer,
        net_imposable,
        cotisations_salariales,
        cotisations_patronales,
        cout_employeur,
        cachets: apres.engagement.cachets - avant.engagement.cachets,
        heures,
    })
}

/// Ce que la page dit d'une correction qui change les dates du contrat : les dates declarees, les corrigees, ce que la
/// DSN suivante en declare (`les_dates_d_un_contrat_declare`), et, quand le contrat court sur d'autres mois, que chacun
/// se corrige pour lui-meme.
fn phrase_des_dates(mois: Mois, declarees: &Bornes, corrigees: &Bornes) -> String {
    let mut quoi = Vec::new();
    if declarees.0 != corrigees.0 {
        quoi.push("la nouvelle date de début avec l'ancienne, dans un bloc « Changements contrat » (S21.G00.41.021, fiche net-entreprises 1952)");
    }
    if declarees.1 != corrigees.1 {
        quoi.push("la nouvelle fin dans un bloc « Fin du contrat » (S21.G00.62, fiche net-entreprises 1722) quand le contrat a fini");
    }
    let mut phrase = format!(
        "Dates corrigées : {} (déclaré : {}). La DSN suivante déclare {}.",
        bornes_en_clair(corrigees),
        bornes_en_clair(declarees),
        quoi.join(", et ")
    );
    let autres: Vec<Mois> = mois_des_bornes(corrigees).into_iter().chain(mois_des_bornes(declarees)).filter(|&m| m != mois).fold(Vec::new(), |mut v, m| {
        if !v.contains(&m) {
            v.push(m);
        }
        v
    });
    if !autres.is_empty() {
        phrase.push_str(&format!(
            " Le contrat court aussi sur {} : chaque mois se corrige pour lui-même, et la DSN suivante porte un versement par mois civil (cahier technique DSN §2.2.2.5).",
            autres.iter().map(|m| m.libelle(Graphie::Accentuee)).collect::<Vec<_>>().join(" et ")
        ));
    }
    phrase
}

/// Les mois que couvrent des bornes ; sans fin, le seul premier mois.
fn mois_des_bornes(b: &Bornes) -> Vec<Mois> {
    let fin = match &b.1 {
        Some(f) => *f,
        None => b.0,
    };
    Mois::entre(&b.0, &fin)
}

// ------------------------------------------------- modifier la paie d'un mois deja declare

/// `de septembre 2026`, `d'octobre 2026`.
pub(crate) fn de_mois_en_clair(mois: Mois) -> String {
    socle::texte::de(&mois.libelle(Graphie::Accentuee))
}

/// Ce qui arrive a la DSN reelle deposee de `mois` quand « Modifier l'engagement » le touche (`modifier-engagement`) ;
/// `None` quand aucune DSN reelle acceptee n'est partie. Tant que l'annule-et-remplace est admis (jusqu'a la veille de
/// l'echeance, minuit), elle se remplace ; apres, la DSN deposee ne change plus et la correction se declare dans la DSN
/// qui porte (`dsn_qui_porte` : la premiere d'un mois suivant qui n'est pas partie), des que l'engagement modifie est
/// revalide (`produire_regularisations::regularisations_pour`). Celle que l'ancien logiciel a deposee ne se remplace
/// jamais ici (regle 4, `import::mois::dsn_de_l_ancien_logiciel`).
pub(crate) fn modification_de_l_engagement(m: &mut Moteur, ctx: &Contexte, mois: socle::mois::Mois) -> R<Option<DsnDeposee>> {
    if let Some(porteur) = import::mois::dsn_de_l_ancien_logiciel(ctx, mois)?.map(|d| d.porteur) {
        return Ok(Some(DsnDeposee::Corrigee { porteur }));
    }
    let dsn = compagnie::etat_dsn::du_mois(ctx, mois, None)?;
    if !dsn.deposee() || dsn.refusee() {
        return Ok(None);
    }
    // LA FRONTIERE EST LA VEILLE A MINUIT, pas le jour de l'echeance.
    let veille = m.limite(mois)?;
    if ctx.aujourdhui <= veille {
        return Ok(Some(DsnDeposee::Remplacee { limite: veille }));
    }
    Ok(Some(DsnDeposee::Corrigee { porteur: compagnie::etat_dsn::dsn_qui_porte(ctx, mois)? }))
}

/// Ce qui arrive a la DSN deposee d'un mois quand une modification la touche (`modification_de_l_engagement`).
pub(crate) enum DsnDeposee {
    /// Un annule-et-remplace, admis jusqu'a la veille de l'echeance, minuit (`limite`).
    Remplacee { limite: socle::date::Jour },
    /// Elle ne se remplace plus (sa limite passee, ou deposee par l'ancien logiciel) : la correction se declare dans
    /// la DSN du mois `porteur` (`dsn_qui_porte`).
    Corrigee { porteur: Mois },
}

/// Les bornes d'un contrat : son premier jour et sa fin.
type Bornes = (Jour, Option<Jour>);

fn bornes(p: &Paie) -> Bornes {
    (p.engagement.debut, p.engagement.fin)
}

/// « du 02/07/2026 au 25/07/2026 ».
fn bornes_en_clair(b: &Bornes) -> String {
    match &b.1 {
        Some(f) => format!("du {} au {}", b.0.jj_mm_aaaa(), f.jj_mm_aaaa()),
        None => format!("à partir du {}, sans fin", b.0.jj_mm_aaaa()),
    }
}

/// UN CONTRAT DECLARE NE SE RETIRE PAS D'UN MOIS DONT LA DSN NE SE REMPLACE PLUS. Passe la date d'exigibilite, une DSN
/// ne se remplace plus : « il convient de corriger les informations erronees dans la DSN du mois suivant » (fiche
/// net-entreprises 1722, `atelier/reference/sources/net-entreprises-base-connaissances/
/// net-entreprises-fiche1722-annulation-fin-de-contrat-transmise-a-tort-20250326.txt`). Ce qui s'y corrige : la paie,
/// par rappel, en approche differentielle ou par annulation et remplacement des montants (cahier technique DSN 2026.1,
/// §2.1.3.6, `atelier/reference/norme-dsn/texte/chapitre-2-principes-de-constitution.txt`, l. 607) ; et, d'un contrat,
/// sa fin, que le motif 099 annule (fiche 1722). INFERENCE : aucun de ces textes n'annule un contrat declare (son bloc
/// S21.G00.40) ; un contrat qui n'a pas eu lieu garde son bloc et voit ses montants annules (ses quantites a zero).
///
/// `avant` : l'engagement tel qu'il est ecrit ; `apres` : ce qu'il devient. Refus : un mois que sa DSN a declare pour
/// lui (`departs`) et dont la DSN ne se remplace plus (`modification_de_l_engagement`, `DsnDeposee::Corrigee`), qu'il ne couvre plus --
/// ou tous, quand le contrat change de salarie. Le refus nomme la DSN qui a declare le contrat pour ce mois : la sienne,
/// ou celle d'un mois suivant qui a porte sa correction (`Depart::par`). `Ok(None)` : rien d'indeclarable.
pub(crate) fn refus_non_declarable(
    m: &mut Moteur,
    ctx: &Contexte,
    avant: &engagement::stock::Objet,
    apres: &engagement::stock::Objet,
    departs: &engagement::departs::Departs,
) -> R<Option<String>> {
    let brut = |o: &engagement::stock::Objet| o.brut.clone();
    let (b_avant, b_apres) = (brut(avant), brut(apres));
    let (Some(cle), Some(numero)) = (b_avant.saisie.carte.salarie.clone(), b_avant.saisie.carte.numero_dit().map(String::from)) else {
        return Ok(None);
    };
    let meme_salarie = b_apres.saisie.carte.salarie.as_ref() == Some(&cle);
    for d in departs.de(&cle, &numero).iter().filter(|d| d.envoi == engagement::departs::Envoi::Dsn) {
        let Some(lu) = d.mois else { continue };
        if meme_salarie && apres.couvre(lu)? {
            continue;
        }
        if !matches!(modification_de_l_engagement(m, ctx, lu)?, Some(DsnDeposee::Corrigee { .. })) {
            continue;
        }
        let nom = cle.to_string();
        let debut = engagement::debut_brut(&b_avant).ok_or_else(|| socle::fichiers::absent("debut").reclasser(Classe::Illisible))?.jj_mm_aaaa();
        let dsn = match departs.de(&cle, &numero).iter().find(|x| x.envoi == engagement::departs::Envoi::Dsn && x.mois == Some(lu)).and_then(|x| x.par) {
            Some(par) => format!("dans la DSN {}, qui a porté la correction {}", de_mois_en_clair(par), de_mois_en_clair(lu)),
            None => format!("dans la DSN {}, qui ne se remplace plus", de_mois_en_clair(lu)),
        };
        return Ok(Some(format!(
            "Le contrat de {} du {} est déclaré {} : il ne se retire plus. La DSN corrige la paie d'un mois passé (rappel de paie, cahier technique DSN §2.1.3.6) et annule une fin de contrat (motif 099, fiche net-entreprises 1722), pas un contrat. S'il n'a pas eu lieu, gardez-le et mettez ses quantités à zéro : la DSN suivante déclarera l'annulation de ses montants. Rien n'est enregistré.",
            nom, debut, dsn
        )));
    }
    Ok(None)
}


/// La DPAE d'un contrat dont la date d'embauche a change (`crate::etapes::debut_declare`), deposee pour l'ancienne date `avant` :
/// vaut-elle pour la nouvelle `apres`, et la phrase qui le dit. Code du travail, `atelier/reference/sources/code-du-travail/`
/// (`xml/L1221-10.xml`, `xml/R1221-1.xml`, `xml/R1221-4.xml`) : « L'embauche d'un salarie ne peut intervenir qu'apres
/// declaration nominative » (L. 1221-10) ; la DPAE porte la « Date et heure d'embauche » (R. 1221-1, 3°) et part au plus
/// tot huit jours avant « la date previsible de l'embauche » (R. 1221-4). INFERENCE : reculee, l'embauche a eu lieu apres
/// la declaration, qui portait sa date previsible -- elle vaut ; avancee, l'embauche a precede la declaration -- une DPAE
/// a la date vraie est a deposer. La DPAE n'a pas d'annule-et-remplace (guide de l'API DPAE de l'Urssaf,
/// `atelier/reference/norme-dpae/5491-API-DPAE-Fonctionnement.pdf`, p. 3) et ne s'annule pas : une correction est une
/// nouvelle declaration (fiche net-entreprises 3300, `atelier/reference/sources/net-entreprises-base-connaissances/
/// net-entreprises-fiche3300-dpae-en-dsn-20261015.txt`, l. 451).
pub(crate) fn dpae_d_une_date_changee(avant: Jour, apres: Jour) -> R<(bool, String)> {
    let (a, b) = (avant.jj_mm_aaaa(), apres.jj_mm_aaaa());
    Ok(if avant < apres {
        (
            true,
            format!(
                "Embauche reportée du {} au {} : la DPAE déposée pour le {} vaut — elle a précédé l'embauche (code du travail, art. L. 1221-10) et portait sa date prévisible (art. R. 1221-4). Rien à refaire.",
                a, b, a
            ),
        )
    } else {
        (
            false,
            format!(
                "Embauche avancée du {} au {} : la DPAE déposée pour le {} ne l'a pas précédée (code du travail, art. L. 1221-10) et porte une autre date d'embauche (art. R. 1221-1, 3°). Une DPAE au {} est à déposer : une DPAE ne se remplace pas (pas d'annule-et-remplace, guide de l'API DPAE de l'Urssaf).",
                a, b, a, b
            ),
        )
    })
}


// ------------------------------------------------------------- les corrections d'un mois

/// Le nom d'un salarie tel que la page le montre : « NOM Prenom ».
fn nom_de(p: &Paie) -> String {
    documents::tableaux_export::qui_souple(&p.salarie)
}

/// AVANT, APRES, DIFFERENCE : deux paies ecrites d'un contrat (`avant` : la derniere declaree, `None` pour un contrat
/// qu'aucune DSN n'a declare ; `apres` : celle de la version presente), resumees comme la DSN les declare, et ce que la
/// correction en declare.
fn avant_apres(mois: Mois, avant: Option<&Paie>, apres: &Paie) -> R<Correction> {
    let r_avant = avant.map(resume).transpose()?;
    let r_apres = resume(apres)?;
    let ecart = avant.map(|a| ecart(a, apres)).transpose()?;
    let dates = match avant {
        Some(a) if bornes(a) != bornes(apres) => Some(phrase_des_dates(mois, &bornes(a), &bornes(apres))),
        _ => None,
    };
    let contrat = match &apres.engagement.numero {
        Some(n) => format!("contrat {}, {}", n, apres.engagement.debut.jj_mm_aaaa()),
        None => format!("contrat du {}", apres.engagement.debut.jj_mm_aaaa()),
    };
    let montants = match &r_avant {
        Some(a) => format!("brut {} → {}, net {} → {}", a.brut, r_apres.brut, a.net_a_payer, r_apres.net_a_payer),
        None => format!("ajouté : brut {}, net {}", r_apres.brut, r_apres.net_a_payer),
    };
    Ok(Correction {
        infos_dit: format!("{} — {}", contrat, montants),
        portee_dit: "prochaine DSN".to_string(),
        salarie: apres.salarie.id.to_string(),
        debut: apres.engagement.debut,
        nom: nom_de(apres),
        spectacle: apres.titre()?,
        numero: apres.engagement.numero.clone(),
        ajout: avant.is_none(),
        avant: r_avant,
        apres: r_apres,
        delta: ecart,
        dates_dites: dates,
        portee_par: None,
        portee_par_dit: None,
    })
}

/// UNE CORRECTION D'UN MOIS DECLARE, telle que la page la montre : le contrat (`infos_dit` : son numero, ses montants
/// declares et presents), la DSN qui la porte (`portee_dit`), la paie declaree (`avant`, `None` pour un contrat
/// qu'aucune DSN n'a declare : `ajout`), la presente (`apres`), l'ecart (`delta`), ses dates changees (`dates`) ; deja
/// portee, le mois de la DSN qui l'a portee (`portee_par`).
#[derive(Debug, Serialize)]
pub(crate) struct Correction {
    infos_dit: String,
    portee_dit: String,
    pub(crate) salarie: String,
    debut: socle::date::Jour,
    nom: String,
    spectacle: String,
    numero: Option<NumeroContrat>,
    avant: Option<Resume>,
    apres: Resume,
    delta: Option<Ecart>,
    /// Ses dates changees, en une phrase.
    #[serde(rename = "dates")]
    dates_dites: Option<String>,
    ajout: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) portee_par: Option<Mois>,
    #[serde(skip_serializing_if = "Option::is_none")]
    portee_par_dit: Option<String>,
}

/// Ce que la page montre des corrections de `mois` : celles a porter (la paie presente de chaque contrat dont la DSN
/// partie ne declare plus la paie, `produire_regularisations::corrections_dues`, comparee a la derniere declaree) et
/// celles deja portees par la DSN d'un mois suivant (chaque paie declaree par une correction, comparee a celle qu'elle
/// corrigeait). Vide quand le mois ne se corrige pas ailleurs que dans sa DSN (`se_corrige_ailleurs`).
pub(crate) fn corrections_du_mois(ctx: &Contexte, mois: Mois) -> R<(Vec<Correction>, Vec<Correction>)> {
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    if !declarations::produire_regularisations::se_corrige_ailleurs(ctx, &departs, mois)? {
        return Ok((Vec::new(), Vec::new()));
    }
    let Some(data) = engagement::stock::vue_du_mois(&ctx.racine, mois)? else { return Ok((Vec::new(), Vec::new())) };
    let mut a_porter = Vec::new();
    for d in declarations::produire_regularisations::corrections_dues(ctx, mois, None, &departs)? {
        let apres = d.valide.paie(ctx, &data, d.rang)?;
        a_porter.push(avant_apres(mois, d.declarees.last().map(|x| &x.paie), &apres)?);
    }
    let mut portees = Vec::new();
    for e in engagement::etats::du_mois(ctx, mois, &departs)? {
        let Some(v) = e.valide() else { continue };
        let declarees = engagement::etats::paies_declarees(ctx, v.objet(), &departs, mois, None).map_err(|e| e.reclasser(Classe::MoisNonRegularisable))?;
        for (i, x) in declarees.iter().enumerate() {
            let Some(par) = x.par else { continue };
            let mut c = avant_apres(mois, i.checked_sub(1).map(|j| &declarees[j].paie), &x.paie)?;
            c.portee_par = Some(par);
            c.portee_par_dit = Some(de_mois_en_clair(par));
            c.portee_dit = format!("portée par la DSN {}", de_mois_en_clair(par));
            portees.push(c);
        }
    }
    Ok((a_porter, portees))
}


/// LA DSN QUI PORTE LA CORRECTION DE `mois` POUR L'ENGAGEMENT `v`, telle que `departs` la connait : celle d'un mois
/// suivant qui l'a deja declaree (`engagement::etats::paies_declarees`, la derniere correction portee) quand aucune
/// correction n'est plus due (`engagement::etats::correction_due`) ; sinon la DSN qui portera la suivante
/// (`dsn_qui_porte`). Le versement de l'ecart d'un rectificatif est date du mois de cette DSN.
pub(crate) fn dsn_qui_porte_la_correction(ctx: &Contexte, v: &engagement::etats::Valide, departs: &engagement::departs::Departs, mois: Mois) -> R<Mois> {
    if engagement::etats::correction_due(ctx, v, departs, mois, None)?.is_none() {
        let declarees = engagement::etats::paies_declarees(ctx, v.objet(), departs, mois, None)?;
        if let Some(par) = declarees.last().and_then(|d| d.par) {
            return Ok(par);
        }
    }
    compagnie::etat_dsn::dsn_qui_porte(ctx, mois)
}
