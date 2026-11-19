//! LES SUITES D'UN ENVOI, du cote de la compagnie (`envois::suites`) : ce qu'un envoi a releve en chemin et qui touche a
//! la compagnie -- noter les incidents, ranger les attestations France Travail, marquer les arrets qu'une DSN a portes,
//! faire taire le reveil du filet. Elles se font toutes, que l'envoi ait abouti ou non.

use droit::fiche::Lue as _;
use socle::echec::Classe;

use droit::contexte::Contexte;
use envois::suites::{ADire, Suites};
use socle::echec::R;

/// Fait les suites d'un envoi, et rend tout ce qu'il a a dire : ce que l'envoi en dit deja (`Suites::dit`), puis ce que
/// les suites laissent a faire ou n'ont pas pu faire. Un rangement d'AER qui echoue se dit ; une marque qui ne s'ecrit
/// pas est un refus. Une piece de retour que son lecteur ne lit pas, quelle que soit sa nature, se note chez l'editeur
/// (`compagnie::incidents::noter_retour_illisible`), comme un retour que l'organisme a retire
/// (`compagnie::incidents::noter_retour_retire`) ; les autres se rangent.
pub fn faire(ctx: &Contexte, s: Suites) -> R<ADire> {
    let mut faites = s.dit;
    let mut illisibles = s.illisibles;
    if !s.incidents_des_retours.is_empty() {
        compagnie::incidents::noter_ceux_des_retours(&ctx.racine, envois::etats::AvecIncidents { resultat: Ok(()), incidents: s.incidents_des_retours, illisibles: Vec::new() })?;
    }
    if let Some(d) = &s.depot_refuse {
        if let Err(e) = noter_incident_depot(ctx, d) {
            faites.incidents.push(format!("Le dépôt refusé ne s'est pas noté au journal des incidents : {}", e.phrase));
        }
    }
    // Ce que les suites ont lu jusqu'au bout : ses retours se notent vus.
    let mut lus: Vec<&(envois::retours::Annonce, std::path::PathBuf)> = Vec::new();
    if !s.taux_pas.is_empty() {
        match appliquer_taux_pas(ctx, &s.taux_pas, &mut illisibles) {
            Ok(sans_taux) => {
                faites.a_faire.extend(sans_taux);
                lus.extend(&s.taux_pas);
            }
            Err(e) => faites.incidents.push(format!("Taux de PAS recus mais non appliques : {}", e.phrase)),
        }
    }
    if !s.taux_atmp.is_empty() {
        match appliquer_taux_atmp(ctx, &s.taux_atmp, &mut illisibles) {
            Ok(a_faire) => {
                faites.a_faire.extend(a_faire);
                lus.extend(&s.taux_atmp);
            }
            Err(e) => faites.incidents.push(format!("Notification du taux AT/MP reçue mais non posée : {}", e.phrase)),
        }
    }
    // Une archive d'attestations que le lecteur ne lit pas ne se range pas, et n'empeche pas les autres.
    if !s.aer.is_empty() {
        match crate::documents::ranger_aer(ctx, None, Some(&s.aer)) {
            Ok((a_faire, pieces)) => {
                faites.a_faire.extend(a_faire);
                illisibles.extend(pieces);
            }
            Err(e) => faites.incidents.push(format!("Attestations France Travail recues mais non rangees : {}", e.phrase)),
        }
    }
    for (nature, url, echec) in &s.retires {
        if let Err(n) = compagnie::incidents::noter_retour_retire(&ctx.racine, *nature, url, echec) {
            faites.incidents.push(format!("Retour de nature {} retiré par l'organisme : l'incident ne s'est pas noté au journal des incidents : {}", nature, n.phrase));
        }
    }
    for p in &illisibles {
        if let Err(n) = compagnie::incidents::noter_retour_illisible(&ctx.racine, p) {
            faites.incidents.push(format!("Archive {} : l'incident ne s'est pas noté au journal des incidents : {}", p.archive, n.phrase));
        }
    }
    // UNE PIECE QUE SON LECTEUR NE LIT PAS N'EST PAS UN RETOUR VU (`envois::releve_retours::relever_retours`) : le
    // releve suivant la retelecharge et la relit.
    let mut vus: Vec<envois::retours::Annonce> = Vec::new();
    for (retour, chemin) in lus {
        let nom = socle::fichiers::NomDeFichier::du(chemin)?;
        let non_lue = illisibles.iter().any(|p| p.nature == retour.nature_retour && p.archive == nom);
        if !non_lue && s.vus_une_fois_lus.contains(retour) {
            vus.push(retour.clone());
        }
    }
    if !vus.is_empty() {
        envois::releve_retours::noter_vus(ctx, &vus, envois::releve_retours::CanalDesRetours::Dsn)?;
    }
    Ok(faites)
}

/// Les taux qu'un compte rendu DGFiP de nature 94 communique vont dans la fiche de leur salarie : un taux se lit
/// du compte rendu, il ne se devine pas (regle 1). Chaque releve se rattache a une fiche par le NIR ; un NIR qu'aucune
/// fiche ne porte ne se rattache a personne. La validite court de la mise a disposition (`droit::pas`). Le compte rendu
/// qui repond a une DSN mensuelle (le depot du journal au meme flux, de nature 01) dit le mois qu'elle declare
/// (`en_retour_de`, `droit::pas::date_d_effet`) : son taux vaut pour les mois qui suivent. Un releve sans taux lisible
/// ne pose aucun taux : la situation du salarie reste ce qu'elle etait, et la phrase rendue le dit.
fn appliquer_taux_pas(ctx: &Contexte, crm: &[(envois::retours::Annonce, std::path::PathBuf)], illisibles: &mut Vec<envois::retours::PieceIllisible>) -> R<Vec<String>> {
    let nirs: Vec<(droit::contexte::IdSalarie, droit::fiche::Nir)> = ctx.salaries.iter().filter_map(|(cle, s)| s.nir.dite().map(|nir| (cle.clone(), nir.clone()))).collect();
    let journal = envois::journal::lu(&ctx.racine).map_err(|m| m.reclasser(Classe::Illisible))?;
    let mois_declare = |retour: &envois::retours::Annonce| -> Option<socle::mois::Mois> {
        let flux = retour.flux.as_ref()?;
        let l = journal.lignes.iter().find(|l| l.flux() == Some(flux))?;
        (l.acte() == documents::Acte::Dsn && l.nature_dsn() == Some(droit::nature_declaration::NatureDeLaDeclaration::Mensuelle)).then(|| l.mois()).flatten()
    };
    let mut tous = Vec::new();
    let mut sans_taux = Vec::new();
    for (retour, chemin) in crm {
        let en_retour_de = mois_declare(retour);
        let octets = std::fs::read(chemin).map_err(|e| socle::echec::du_disque(&chemin, &e).reclasser(Classe::Disque))?;
        let nom = socle::fichiers::NomDeFichier::du(chemin)?;
        // Un compte rendu que le lecteur ne lit pas ne pose rien, et n'empeche pas les autres.
        let lus = match envois::retours::taux_pas_d_octets(&octets) {
            Ok(l) => l,
            Err(e) => {
                illisibles.push(envois::retours::PieceIllisible::de(envois::retours::NatureDuRetour::TauxPas, chemin, e)?);
                continue;
            }
        };
        let r = envois::retours::releves_vers_salaries(&lus.releves, &nirs);
        for x in &r.rattaches {
            // La mise a disposition est celle du compte rendu ; sans elle, le taux ne se date pas.
            let mis_a_disposition = retour.publication.ok_or_else(|| socle::echec::erreur(Classe::Illisible, format!("Un compte rendu de taux de PAS ne dit pas sa date de mise à disposition ({}).", nom)))?;
            let Some(taux) = x.releve.taux else {
                let qui = match ctx.salaries.get(&x.salarie).and_then(|s| s.nomme()).filter(|q| !q.is_empty()) {
                    Some(q) => q,
                    None => x.salarie.to_string(),
                };
                sans_taux.push(format!(
                    "Prélèvement à la source de {} : le compte rendu de la DGFiP ({}) ne donne pas de taux lisible : aucun taux ne se pose, et sa fiche reste comme elle était.",
                    qui, nom
                ));
                continue;
            };
            tous.push((x.salarie.clone(), droit::pas::TauxRecu { taux, r#type: x.releve.type_taux, identifiant: x.releve.identifiant.clone(), mis_a_disposition, en_retour_de }));
        }
    }
    crate::gestes::taux_pas::enregistrer(ctx, &tous, "CRM DGFiP nature 94")?;
    Ok(sans_taux)
}

/// LES NOTIFICATIONS DU TAUX AT/MP (nature 34) : le code risque et le taux se lisent de la notification (regle 1 ;
/// `envois::retours::taux_atmp_d_octets`, `atelier/reference/sources/gip-mds-api-dsn/`) et s'inscrivent dans la case de
/// la fiche entreprise a leur date d'effet, s'ils en different (`routes::atmp::recue`). Seul le siege (le SIREN et le NIC de la fiche
/// entreprise) recoit ici son taux : une notification pour un autre etablissement, ou qui donne plusieurs codes risques
/// pour la meme annee ne pose rien et se dit, dans ce qu'il reste a faire ; illisible, elle ne pose rien et se note.
fn appliquer_taux_atmp(
    ctx: &Contexte,
    notifications: &[(envois::retours::Annonce, std::path::PathBuf)],
    illisibles: &mut Vec<envois::retours::PieceIllisible>,
) -> R<Vec<String>> {
    use std::collections::BTreeMap;
    let mut a_faire = Vec::new();
    let (siren, nic) = (ctx.entreprise.siren.portee(), ctx.entreprise.nic.portee());
    let la_fiche = match (siren, nic) {
        (Some(s), Some(n)) => format!("qui n'est pas celui de la fiche Entreprise ({}{})", s, n),
        _ => "et la fiche Entreprise ne dit pas son SIREN et son NIC".to_string(),
    };
    for (retour, chemin) in notifications {
        let le_jour = retour.publication.map(|p| p.jour_de_paris());
        let publiee = envois::releve_retours::jour_de_publication_dit(retour);
        let archive = socle::fichiers::NomDeFichier::du(chemin)?;
        let source = match &publiee {
            Some(le) => format!("Notification AT/MP de la CARSAT reçue le {} (retour DSN de nature 34, archive retours/{})", le, archive),
            None => format!("Notification AT/MP de la CARSAT (retour DSN de nature 34, archive retours/{})", archive),
        };
        let octets = std::fs::read(chemin).map_err(|e| socle::echec::du_disque(&chemin, &e).reclasser(Classe::Disque))?;
        let lus = match envois::retours::taux_atmp_d_octets(&octets) {
            Ok(l) => l,
            Err(e) => {
                illisibles.push(envois::retours::PieceIllisible::de(envois::retours::NatureDuRetour::TauxAtMp, chemin, e)?);
                continue;
            }
        };
        let mut par_annee: BTreeMap<socle::mois::Annee, Vec<envois::retours::TauxAtmpNotifie>> = BTreeMap::new();
        for t in lus {
            if Some(&t.siren) != ctx.entreprise.siren.dite() || Some(&t.nic) != ctx.entreprise.nic.dite() {
                a_faire.push(format!(
                    "{} : le taux {} (code risque {}) est celui de l'établissement {}{}, {}. Rien n'est posé.",
                    source, socle::texte::virgule(t.taux), t.code_risque, t.siren, t.nic, la_fiche
                ));
                continue;
            }
            par_annee.entry(t.date_effet.annee()).or_default().push(t);
        }
        for (an, mut taux) in par_annee {
            taux.sort_by_key(|t| t.date_effet);
            let codes: std::collections::BTreeSet<&droit::bareme::CodeRisqueAt> = taux.iter().map(|t| &t.code_risque).collect();
            if codes.len() > 1 {
                a_faire.push(format!(
                    "{} : plusieurs codes risques pour {} ({}). La configuration n'en porte qu'un par année : rien n'est posé ; saisissez celui de l'activité principale dans Configuration › Entreprise, « Accidents du travail (AT/MP) ».",
                    source, an, codes.into_iter().map(droit::bareme::CodeRisqueAt::as_str).collect::<Vec<_>>().join(", ")
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
    let apres_test = d.mode == socle::mode::Mode::Reel && d.repetition.as_ref().is_some_and(|r| r.test.is_some());
    compagnie::incidents::noter(
        &ctx.racine,
        &compagnie::incidents::Note {
            nature: if d.quoi == documents::Acte::Dsn { compagnie::incidents::DSN } else { compagnie::incidents::DPAE },
            ou: &format!("depot-{}", d.quoi.nom()),
            quoi: socle::echec::classe_notee(d.classe),
            resume: &d.message,
            detail: DetailDuDepotRefuse { mode: d.mode, apres_test_conforme: apres_test, repetition: d.repetition.as_ref() },
            trace: None,
            sans_contexte: false,
        },
    )?;
    Ok(())
}

/// Ce que l'incident d'un depot refuse porte au developpeur : reel ou test, s'il suivait une repetition conforme, et
/// cette repetition.
#[derive(serde::Serialize)]
struct DetailDuDepotRefuse<'a> {
    mode: socle::mode::Mode,
    apres_test_conforme: bool,
    repetition: Option<&'a envois::journal::Ouverture>,
}
