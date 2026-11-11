//! LES SUITES D'UN ENVOI, du cote de la compagnie (`envois::suites`) : ce qu'un envoi a releve en chemin et qui touche a
//! la compagnie -- noter les incidents, ranger les attestations France Travail, marquer les arrets qu'une DSN a portes,
//! faire taire le reveil du filet. Elles se font toutes, que l'envoi ait abouti ou non.

use droit::fiche::Lue as _;
use socle::echec::Classe;

use droit::contexte::Contexte;
use envois::suites::Suites;
use socle::echec::R;

/// Ce que les suites laissent a dire : ce qu'il reste a faire, et ce qui n'a pas pu se faire.
#[derive(Default)]
pub struct Faites {
    pub a_faire: Vec<String>,
    pub incidents: Vec<String>,
}

/// Fait les suites d'un envoi. Un rangement d'AER qui echoue se dit ; une marque qui ne s'ecrit pas est un refus.
pub fn faire(ctx: &Contexte, s: Suites) -> R<Faites> {
    let mut faites = Faites::default();
    if !s.incidents_des_retours.is_empty() {
        compagnie::incidents::noter_ceux_des_retours(&ctx.racine, envois::etats::AvecIncidents { resultat: Ok(()), incidents: s.incidents_des_retours })?;
    }
    if let Some(d) = &s.depot_refuse {
        if let Err(e) = noter_incident_depot(ctx, d) {
            faites.incidents.push(format!("Le dépôt refusé ne s'est pas noté au journal des incidents : {}", e.phrase));
        }
    }
    if !s.taux_pas.is_empty() {
        if let Err(e) = appliquer_taux_pas(ctx, &s.taux_pas) {
            faites.incidents.push(format!("Taux de PAS recus mais non appliques : {}", e.phrase));
        }
    }
    if !s.taux_atmp.is_empty() {
        match appliquer_taux_atmp(ctx, &s.taux_atmp) {
            Ok(a_faire) => faites.a_faire.extend(a_faire),
            Err(e) => faites.incidents.push(format!("Notification du taux AT/MP reçue mais non posée : {}", e.phrase)),
        }
    }
    if !s.aer.is_empty() {
        match crate::documents::ranger_aer(ctx, None, Some(&s.aer)) {
            Ok(a_faire) => faites.a_faire.extend(a_faire),
            Err(e) => faites.incidents.push(format!("Attestations France Travail recues mais non rangees : {}", e.phrase)),
        }
    }
    Ok(faites)
}

/// Les taux qu'un compte rendu DGFiP de nature 94 communique vont dans la fiche de leur salarie : un taux se lit
/// du compte rendu, il ne se devine pas (regle 1). Chaque releve se rattache a une fiche par le NIR ; un NIR qu'aucune
/// fiche ne porte ne se rattache a personne. La validite court de la mise a disposition (`droit::pas`). Le compte rendu
/// qui repond a une DSN mensuelle (le depot du journal au meme flux, de nature 01) dit le mois qu'elle declare
/// (`en_retour_de`, `droit::pas::date_d_effet`) : son taux vaut pour les mois qui suivent.
fn appliquer_taux_pas(ctx: &Contexte, crm: &[(envois::retours::Annonce, std::path::PathBuf)]) -> R<()> {
    let nirs: Vec<(String, String)> = ctx.salaries.iter().filter_map(|(cle, s)| s.nir.portee().map(|nir| (cle.to_string(), nir.to_string()))).collect();
    let journal = envois::journal::lu(&ctx.racine).map_err(|m| m.reclasser(Classe::Illisible))?;
    let mois_declare = |retour: &envois::retours::Annonce| -> Option<socle::mois::Mois> {
        let flux = retour.flux.as_deref().filter(|f| !f.is_empty())?;
        let l = journal.lignes.iter().find(|l| l.flux() == flux)?;
        (l.acte() == envois::journal::Acte::Dsn && l.nature_dsn() == Some("01")).then(|| l.mois()).flatten()
    };
    let mut tous = Vec::new();
    for (retour, chemin) in crm {
        let en_retour_de = mois_declare(retour);
        let octets = std::fs::read(chemin).map_err(|e| socle::echec::du_disque(&chemin, &e).reclasser(Classe::Disque))?;
        let lus = envois::retours::taux_pas_d_octets(&octets)?;
        let r = envois::retours::releves_vers_salaries(&lus.releves, &nirs);
        for x in &r.rattaches {
            // La mise a disposition est celle du compte rendu ; sans elle, le taux ne se date pas.
            let mis_a_disposition = retour.publication.map(|p| p.compact()).ok_or_else(|| socle::echec::erreur(Classe::Illisible, format!("Un compte rendu de taux de PAS ne dit pas sa date de mise à disposition ({}).", socle::fichiers::nom_du(chemin))))?;
            tous.push(compagnie::taux_pas::TauxRecu {
                salarie: x.salarie.clone(),
                taux: x.releve.taux.clone(),
                type_taux: x.releve.type_taux.clone(),
                identifiant: x.releve.identifiant.clone(),
                mis_a_disposition,
                en_retour_de,
            });
        }
    }
    crate::gestes::taux_pas::enregistrer(ctx, &tous, "CRM DGFiP nature 94")?;
    Ok(())
}

/// LES NOTIFICATIONS DU TAUX AT/MP (nature 34) : le code risque et le taux se lisent de la notification (regle 1 ;
/// `envois::retours::taux_atmp_d_octets`, `atelier/reference/sources/gip-mds-api-dsn/`) et s'inscrivent dans la case de
/// la fiche entreprise a leur date d'effet, s'ils en different (`routes::atmp::recue`). Seul le siege (le SIREN et le NIC de la fiche
/// entreprise) recoit ici son taux : une notification pour un autre etablissement, illisible, ou qui donne plusieurs codes
/// risques pour la meme annee ne pose rien et se dit, dans ce qu'il reste a faire.
fn appliquer_taux_atmp(ctx: &Contexte, notifications: &[(envois::retours::Annonce, std::path::PathBuf)]) -> R<Vec<String>> {
    use std::collections::BTreeMap;
    let mut a_faire = Vec::new();
    let (siren, nic) = (ctx.entreprise.siren.portee(), ctx.entreprise.nic.portee());
    let la_fiche = match (siren, nic) {
        (Some(s), Some(n)) => format!("qui n'est pas celui de la fiche Entreprise ({}{})", s, n),
        _ => "et la fiche Entreprise ne dit pas son SIREN et son NIC".to_string(),
    };
    for (retour, chemin) in notifications {
        let le_jour = retour.publication.map(|p| p.naif().jour());
        let publiee = envois::releve_retours::jour_de_publication_dit(retour);
        let archive = socle::fichiers::nom_du(chemin);
        let source = match &publiee {
            Some(le) => format!("Notification AT/MP de la CARSAT reçue le {} (retour DSN de nature 34, archive retours/{})", le, archive),
            None => format!("Notification AT/MP de la CARSAT (retour DSN de nature 34, archive retours/{})", archive),
        };
        let octets = std::fs::read(chemin).map_err(|e| socle::echec::du_disque(&chemin, &e).reclasser(Classe::Disque))?;
        let lus = match envois::retours::taux_atmp_d_octets(&octets) {
            Ok(l) => l,
            Err(e) => {
                a_faire.push(format!("{} : {} Rien n'est posé ; le code et le taux de la notification (compte AT/MP sur net-entreprises) se saisissent dans Configuration › Entreprise, « Accidents du travail (AT/MP) ».", source, e.phrase));
                continue;
            }
        };
        let mut par_annee: BTreeMap<i32, Vec<envois::retours::TauxAtmpNotifie>> = BTreeMap::new();
        for t in lus {
            if Some(t.siren.as_str()) != siren || Some(t.nic.as_str()) != nic {
                a_faire.push(format!(
                    "{} : le taux {} (code risque {}) est celui de l'établissement {}{}, {}. Rien n'est posé.",
                    source, t.taux, t.code_risque, t.siren, t.nic, la_fiche
                ));
                continue;
            }
            par_annee.entry(t.date_effet.an).or_default().push(t);
        }
        for (an, mut taux) in par_annee {
            taux.sort_by_key(|t| t.date_effet);
            let codes: std::collections::BTreeSet<&str> = taux.iter().map(|t| t.code_risque.as_str()).collect();
            if codes.len() > 1 {
                a_faire.push(format!(
                    "{} : plusieurs codes risques pour {} ({}). La configuration n'en porte qu'un par année : rien n'est posé ; saisissez celui de l'activité principale dans Configuration › Entreprise, « Accidents du travail (AT/MP) ».",
                    source, an, codes.into_iter().collect::<Vec<_>>().join(", ")
                ));
                continue;
            }
            for t in &taux {
                let source = format!("{}, effet au {}.", source, t.date_effet.jj_mm_aaaa());
                if let Some(pourquoi) = crate::atmp::recue(ctx, t, le_jour, &source)? {
                    a_faire.push(pourquoi);
                }
            }
        }
    }
    Ok(a_faire)
}

/// Un depot refuse s'ecrit au journal des incidents, test comme reel. LE CAS URGENT SE DISTINGUE ICI : un refus en REEL
/// alors que la repetition etait conforme.
fn noter_incident_depot(ctx: &Contexte, d: &envois::suites::DepotRefuse) -> R<()> {
    let apres_test = d.reel && d.repetition.as_ref().is_some_and(|r| r.test.is_some());
    compagnie::incidents::noter(
        &ctx.racine,
        &compagnie::incidents::Note {
            nature: if d.quoi == "dsn" { compagnie::incidents::DSN } else { compagnie::incidents::DPAE },
            ou: &format!("depot-{}", d.quoi),
            quoi: &d.classe,
            resume: &d.message,
            detail: DetailDuDepotRefuse { reel: d.reel, apres_test_conforme: apres_test, repetition: d.repetition.as_ref() },
            ..Default::default()
        },
    )?;
    Ok(())
}

/// Ce que l'incident d'un depot refuse porte au developpeur : reel ou test, s'il suivait une repetition conforme, et
/// cette repetition.
#[derive(Default, serde::Serialize)]
struct DetailDuDepotRefuse<'a> {
    reel: bool,
    apres_test_conforme: bool,
    repetition: Option<&'a envois::journal::Ouverture>,
}
