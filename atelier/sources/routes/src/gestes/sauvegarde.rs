//! LE PASSAGE DE LA SAUVEGARDE dans le Drive : envoyer ce qui a change (`compagnie::sauvegarde` : l'inventaire, l'index,
//! ce qui persiste), par le client Drive du poste (`routes::coffre::client_drive`) quand aucun n'est donne.

use std::collections::BTreeMap;
use socle::echec::Classe;
use socle::fichiers::CheminRelatif;
use std::path::Path;

use envois::drive::{Client, Drive, EmpreinteMd5, ErreurGoogle};

use compagnie::sauvegarde::{
    cause_de, ecrire_index, inventaire, lire_index, phrase_persistante, EchecIndexe, Etat, EtatFichier, Fichier, Index, LaMain, PosteSurLeDrive, Reporte,
    SansLaMain, Sauvegarde, GOOGLE_INDISPONIBLE,
};

/// Echecs passagers de suite (chacun deja retente par le client) au-dela desquels le reste du passage est reporte sans appel.
const INDISPONIBLE_D_AFFILEE: usize = 3;

/// `sauvegarder` : envoie ce qui a change. Rend le compte rendu, n'imprime rien.
///
/// `client` : le client Drive a utiliser. `None` : le vrai (`Client::nouveau`), construit AVANT
/// l'inventaire ; il refuse une copie de developpement.
///
/// `avancer(verifies, envoyes, total)` : en entrant dans chaque dossier et apres chaque fichier envoye, combien de
/// fichiers sont verifies (identiques au Drive, ou envoyes), combien sont envoyes, sur combien a sauvegarder. Son erreur
/// (l'ecriture de l'etape) n'arrete pas l'envoi : elle se dit une fois dans les incidents du passage, et l'avancement ne
/// se dit plus jusqu'a sa fin.
///
/// `ce_poste` : ce poste, tel que la main le nomme sur le Drive (`PosteSurLeDrive::de_ce_poste` ; un test le tire d'une
/// cle de son dossier, `PosteSurLeDrive::du_coffre`).
pub fn sauvegarder(
    racine: &Path,
    racine_drive: &str,
    tout: bool,
    ce_poste: &PosteSurLeDrive,
    client: Option<&mut dyn Drive>,
    avancer: &mut dyn FnMut(usize, usize, usize) -> socle::echec::R<()>,
) -> Result<Sauvegarde, ErreurGoogle> {
    let mut propre: Client;
    let client: &mut dyn Drive = match client {
        Some(c) => c,
        None => {
            propre = crate::coffre::client_drive(racine)?;
            &mut propre
        }
    };
    // L'identite de la compagnie sur le Drive, ecrite avant l'inventaire a la premiere sauvegarde : elle part avec elle.
    let identite = compagnie::sauvegarde::identite(racine).map_err(|e| ErreurGoogle::from(e.reclasser(Classe::Illisible)))?;
    let nom = compagnie::installation::nom_de(racine).map_err(ErreurGoogle::from)?;
    let (presents, ecartes) = inventaire(racine)?;
    let index = lire_index(racine)?;

    let mut resultat = Sauvegarde {
        racine_drive: racine_drive.to_string(),
        compte: client.compte(),
        deposes: Vec::new(),
        identiques: 0,
        octets_deposes: socle::grandeurs::Octets::ZERO,
        exclus: ecartes,
        disparus: 0,
        incidents: Vec::new(),
        reportes: Vec::new(),
        persistant: None,
        hors_ligne: false,
        sans_la_main: None,
    };
    // GOOGLE ABSENT POUR DE BON : le client a deja retente chaque appel (`drive::NOUVELLES_TENTATIVES`) ; apres
    // `INDISPONIBLE_D_AFFILEE` echecs passagers de suite, le reste du passage est reporte sans appeler, plutot que de
    // payer sept secondes d'attente par fichier.
    let mut indisponibles_de_suite = 0usize;
    // Ce qui n'a PAS pu etre depose. Ces fichiers n'entrent pas dans l'index : sans cela, le
    // passage suivant les croirait a jour.
    let mut en_echec: std::collections::HashSet<CheminRelatif> = std::collections::HashSet::new();
    let etat_de: BTreeMap<&CheminRelatif, &Etat> = presents.iter().map(|(r, e)| (r, e)).collect();
    // Chaque echec de ce passage (la cle de l'index, l'erreur) : son detail technique part chez l'editeur (plus bas).
    let mut a_noter: Vec<(String, ErreurGoogle)> = Vec::new();

    // Par dossier (aucun pour la racine) : un seul listage Drive sert a tous les fichiers qu'il porte.
    let mut par_dossier: BTreeMap<Option<CheminRelatif>, Vec<CheminRelatif>> = BTreeMap::new();
    for (relatif, _) in &presents {
        par_dossier.entry(relatif.parent()).or_default().push(relatif.clone());
    }
    // Ce que `avancer` dit, depuis ce que le passage a fait jusqu'ici. L'affichage ne coupe pas une longue premiere
    // sauvegarde : son echec se dit une fois dans les incidents.
    let mut avancement_dit = true;
    let total = presents.len();
    let mut dire_l_avancement = |r: &mut Sauvegarde| {
        if !avancement_dit {
            return;
        }
        if let Err(e) = avancer(r.identiques + r.deposes.len(), r.deposes.len(), total) {
            avancement_dit = false;
            r.incidents.push(format!("L'avancement de la sauvegarde ne s'écrit plus : {}", e.phrase));
        }
    };

    // LE DOSSIER DE LA COMPAGNIE : celui que l'index garde s'il est encore en place, sinon celui que sa marque designe,
    // sinon un neuf. Qu'il ne se trouve pas, et rien ne part a ce passage : tout est reporte au suivant.
    // LA MAIN (`LaMain`) : ce poste sauvegarde s'il la tient ; personne ne la tenant, il la prend ; un autre poste la
    // tenant, ou une main qui ne se lit pas, rien ne part, et l'index le note (`SansLaMain`).
    let connu = index.as_ref().filter(|_| !tout).and_then(|i| i.compagnie.clone());
    // Un dossier neuf se cree avec la main de ce poste.
    let la_sienne = LaMain { poste: ce_poste.clone(), depuis: socle::temps::secondes().a_la_minute() };
    // Ce qui ne se lit pas dans la main : dit plus bas, avec son detail.
    let mut main_illisible: Option<socle::echec::Echec> = None;
    let trouve = match compagnie::sauvegarde::dossier_de_la_compagnie(client, racine_drive, &identite, &nom, connu.as_ref(), Some(&la_sienne)) {
        Ok((id, marque)) => match LaMain::de(marque.as_ref()) {
            Ok(Some(m)) if m.poste == *ce_poste => Ok(Ok(id)),
            Ok(Some(m)) => Ok(Err(SansLaMain::AutrePoste { depuis: m.depuis })),
            Err(e) => {
                main_illisible = Some(e);
                Ok(Err(SansLaMain::Illisible))
            }
            Ok(None) => client.marquer(&id, &la_sienne.marque(&identite)).map(|()| Ok(id)),
        },
        Err(e) => Err(e),
    };
    let compagnie_id = match trouve {
        Ok(Ok(id)) => Some(id),
        Ok(Err(sans)) => {
            resultat.sans_la_main = Some(sans);
            en_echec.extend(presents.iter().map(|(r, _)| r.clone()));
            None
        }
        Err(e) => {
            if e.injoignable {
                resultat.hors_ligne = true;
            } else {
                resultat.incidents.push(format!(". : {}", e.echec.phrase));
                let reporte = Reporte { quoi: ".".to_string(), dossier: true, cause: cause_de(&e).into() };
                a_noter.push((reporte.cle(), e));
                resultat.reportes.push(reporte);
            }
            en_echec.extend(presents.iter().map(|(r, _)| r.clone()));
            None
        }
    };
    // Tout renvoyer, ou un dossier que l'index ne connait pas : aucun de ses dossiers n'est connu d'avance, chacun se
    // cherche dans le Drive.
    let mut dossiers = match &compagnie_id {
        Some(c) if !tout => compagnie::sauvegarde::dossiers_connus(index.as_ref(), c),
        Some(_) | None => BTreeMap::new(),
    };

    for (dossier, fichiers) in par_dossier.iter().filter(|_| compagnie_id.is_some()) {
        let Some(cie) = compagnie_id.as_ref() else { break };
        dire_l_avancement(&mut resultat);
        let segments = compagnie::sauvegarde::DossierDuDrive::segments(dossier.as_ref());
        let cle = compagnie::sauvegarde::DossierDuDrive::de(dossier.as_ref());
        // Le dossier, tel qu'un dossier reporte le dit (`.` pour la racine).
        let dossier = match dossier {
            Some(d) => d.to_string(),
            None => ".".to_string(),
        };
        if indisponibles_de_suite >= INDISPONIBLE_D_AFFILEE {
            resultat.reportes.push(Reporte { quoi: dossier.clone(), dossier: true, cause: GOOGLE_INDISPONIBLE.into() });
            en_echec.extend(fichiers.iter().cloned());
            continue;
        }
        let essai: Result<(envois::drive::IdDrive, envois::drive::FichiersDuDossier), ErreurGoogle> = (|| {
            let parent = match dossiers.get(&cle) {
                Some(p) => p.clone(),
                None => client.chemin(cie, &segments)?,
            };
            dossiers.insert(cle.clone(), parent.clone());
            // Meme avec `tout`, on liste : c'est `lister` qui dit ce qui existe, donc ce qui se REMPLACE (aucune
            // recherche par fichier). `tout` ne sert qu'a ne pas comparer les empreintes.
            let deja = client.lister(&parent)?;
            Ok((parent, deja))
        })();
        let (parent, deja) = match essai {
            Ok(x) => x,
            Err(e) => {
                if e.injoignable {
                    // Pas de reseau : les dossiers suivants tomberaient pareil. On s'arrete,
                    // rien de ce qui reste n'entre a l'index, et le prochain passage reprendra tout.
                    resultat.hors_ligne = true;
                    en_echec.extend(par_dossier.values().flatten().cloned());
                    dossiers.remove(&cle);
                    break;
                }
                // Un dossier qui resiste ne doit pas emporter les autres : la sauvegarde est
                // partielle, elle le dit, et le prochain passage reprendra celui-la.
                resultat.incidents.push(format!("{} : {}", dossier, e.echec.phrase));
                let cause = cause_de(&e);
                indisponibles_de_suite = if cause == GOOGLE_INDISPONIBLE { indisponibles_de_suite + 1 } else { 0 };
                let reporte = Reporte { quoi: dossier.clone(), dossier: true, cause: cause.into() };
                a_noter.push((reporte.cle(), e));
                resultat.reportes.push(reporte);
                dossiers.remove(&cle);
                en_echec.extend(fichiers.iter().cloned());
                continue;
            }
        };

        let mut tries: Vec<&CheminRelatif> = fichiers.iter().collect();
        tries.sort();
        for relatif in tries {
            if indisponibles_de_suite >= INDISPONIBLE_D_AFFILEE {
                resultat.reportes.push(Reporte { quoi: relatif.to_string(), dossier: false, cause: GOOGLE_INDISPONIBLE.into() });
                en_echec.insert(relatif.clone());
                continue;
            }
            let chemin = relatif.dans(racine);
            let nom = relatif.nom();
            let mut faire = || -> Result<Option<EtatFichier>, ErreurGoogle> {
                let locale = EmpreinteMd5::du_fichier(&chemin).map_err(|e| ErreurGoogle::fichier(&e, &chemin))?;
                // Le fichier de ce nom que `lister` connait (le meme pour decider ET pour remplacer) ; deux de ce nom dans
                // le Drive : le fichier est reporte, en le disant, et rien ne se choisit.
                let connu = deja.le_seul(nom.texte())?;
                let distante = if tout { None } else { connu.and_then(|d| d.md5.as_ref()) };
                if distante == Some(&locale) {
                    return Ok(None);
                }
                client.televerser(&chemin, &parent, None, connu)?;
                // Defaut 5 : « modifie » est ce qui est REMPLACE (un fichier que Drive connait, avec ou sans empreinte,
                // ou renvoye par `tout`) ; « neuf » est ce qui est CREE.
                Ok(Some(if connu.is_some() { EtatFichier::Modifie } else { EtatFichier::Neuf }))
            };
            match faire() {
                Ok(None) => {
                    indisponibles_de_suite = 0;
                    resultat.identiques += 1;
                }
                Ok(Some(etat)) => {
                    indisponibles_de_suite = 0;
                    let octets = etat_de[relatif].octets;
                    resultat.deposes.push(Fichier { relatif: relatif.clone(), octets, etat });
                    resultat.octets_deposes += octets;
                    dire_l_avancement(&mut resultat);
                }
                // LE FICHIER A DISPARU ENTRE L'INVENTAIRE ET L'ENVOI, et ce n'est pas un incident :
                // `engagements/avant/` et `config/avant/` ne gardent que les dernieres copies, et
                // l'interface enregistre a chaque frappe. On ne l'inscrit pas a l'index non plus.
                Err(e) if e.echec.classe == Some(Classe::FichierAbsent) => {
                    resultat.disparus += 1;
                    en_echec.insert(relatif.clone());
                }
                Err(e) => {
                    resultat.incidents.push(format!("{} : {}", relatif, e.echec.phrase));
                    let cause = cause_de(&e);
                    indisponibles_de_suite = if cause == GOOGLE_INDISPONIBLE { indisponibles_de_suite + 1 } else { 0 };
                    let reporte = Reporte { quoi: relatif.to_string(), dossier: false, cause: cause.into() };
                    a_noter.push((reporte.cle(), e));
                    resultat.reportes.push(reporte);
                    en_echec.insert(relatif.clone());
                }
            }
        }
    }

    let maintenant = socle::temps::secondes();
    // SANS LA MAIN, rien n'est parti : l'index garde ce qu'il disait (sans index, un index qui ne dit aucun passage), et
    // note la main ailleurs. Une main illisible se dit avec son detail dans ce passage, et chez l'editeur la premiere fois.
    if let Some(sans) = &resultat.sans_la_main {
        if let Some(e) = &main_illisible {
            let detail = e.a_part.as_ref().and_then(socle::echec::APart::lieu).and_then(|l| l.detail.clone());
            resultat.incidents.push(match detail {
                Some(d) => format!("{} ({})", e.phrase, d),
                None => e.phrase.clone(),
            });
            let deja_dit = index.as_ref().is_some_and(|i| i.sans_la_main == Some(SansLaMain::Illisible));
            if !deja_dit {
                if let Err(n) = compagnie::incidents::depuis_exception(racine, e.classe, &e.phrase, None, "sauvegarde", e.a_part.as_ref(), None) {
                    resultat.incidents.push(format!("La main illisible : le journal des incidents ne l'a pas notée : {}", n.phrase));
                }
            }
        }
        let garde = match index.clone() {
            Some(i) => Index { sans_la_main: Some(sans.clone()), ..i },
            None => Index {
                quand: None,
                racine_drive: racine_drive.to_string(),
                compagnie: None,
                dossiers: BTreeMap::new(),
                fichiers: BTreeMap::new(),
                derniere_complete: None,
                echecs: None,
                sans_la_main: Some(sans.clone()),
            },
        };
        resultat.persistant = Some(sans.phrase());
        ecrire_index(racine, &garde)?;
        return Ok(resultat);
    }
    // LA MARQUE DU DOSSIER, quand ce passage a envoye quelque chose : la date de cette sauvegarde et ce que la compagnie
    // pese, que la vue du Drive montre (et que la suppression d'un dossier dit avant). Qu'elle ne se pose pas n'arrete rien.
    if let (Some(cie), false) = (&compagnie_id, resultat.deposes.is_empty() || resultat.hors_ligne) {
        let poids = presents.iter().map(|(_, e)| e.octets).sum();
        if let Err(e) = client.marquer(cie, &compagnie::sauvegarde::marque(&identite, maintenant.a_la_minute(), poids)) {
            resultat.incidents.push(format!("la date de cette sauvegarde ne s'est pas notée sur le dossier du Drive : {}", e.echec.phrase));
        }
    }
    // L'index n'est ecrit qu'a la fin, et seulement pour ce qui est passe.
    let fichiers: BTreeMap<CheminRelatif, Etat> = presents.iter().filter(|(r, _)| !en_echec.contains(r)).map(|(r, e)| (r.clone(), *e)).collect();
    let quand = maintenant.a_la_minute();
    // CE QUI RESTE EN ECHEC, D'UN PASSAGE A L'AUTRE : de quoi dire ce qui persiste (`souci`). Un passage hors ligne ne compte
    // pas (il n'a rien essaye) : il garde les echecs tels quels. Un passage sans rien de reporte les efface. Les deux cles ne
    // s'ecrivent que s'il y a un echec : l'index d'une sauvegarde complete reste celui d'avant.
    let anciens: BTreeMap<String, EchecIndexe> = index.as_ref().and_then(|i| i.echecs.clone()).into_iter().flatten().collect();
    let echecs: BTreeMap<String, EchecIndexe> = if resultat.hors_ligne {
        anciens.clone()
    } else {
        resultat
            .reportes
            .iter()
            .map(|r| {
                let cle = r.cle();
                let e = match anciens.get(&cle) {
                    Some(a) => EchecIndexe { depuis: a.depuis.clone(), passages: a.passages + 1, cause: r.cause.clone() },
                    None => EchecIndexe { depuis: maintenant.a_la_minute(), passages: 1, cause: r.cause.clone() },
                };
                (cle, e)
            })
            .collect()
    };
    // La derniere sauvegarde complete : celle que l'index d'avant disait deja, sinon le passage d'avant s'il etait complet.
    let derniere_complete = match &index {
        _ if echecs.is_empty() => None,
        Some(Index { derniere_complete: Some(c), .. }) => Some(*c),
        Some(i) if anciens.is_empty() => i.quand,
        _ => None,
    };
    let nouvel_index = Index {
        quand: Some(quand),
        racine_drive: racine_drive.to_string(),
        compagnie: compagnie_id,
        dossiers,
        fichiers,
        derniere_complete,
        echecs: (!echecs.is_empty()).then_some(echecs),
        sans_la_main: None,
    };
    resultat.persistant = phrase_persistante(&nouvel_index, socle::temps::secondes())?;
    ecrire_index(racine, &nouvel_index)?;
    noter_les_echecs(racine, &a_noter, nouvel_index.echecs.as_ref(), &mut resultat);
    Ok(resultat)
}

/// LE DETAIL D'UN ECHEC DE LA SAUVEGARDE CHEZ L'EDITEUR : sa phrase, sa classe et ce qu'il porte a part (le statut, le
/// corps de Google), par le journal des incidents, puisque le passage ne refuse pas et qu'aucune route ne le notera.
/// Une fois, au premier passage ou l'echec apparait (`EchecIndexe::passages`) : un echec qui dure ne fait pas un incident
/// a chaque passage. Une panne de Google (passagere, deja retentee, ou muette) ne se note pas : elle ne dit rien du
/// logiciel, et le passage suivant la retente. Le Drive plein est un refus d'usage (`incidents::REFUS`).
fn noter_les_echecs(racine: &Path, a_noter: &[(String, ErreurGoogle)], echecs: Option<&BTreeMap<String, EchecIndexe>>, resultat: &mut Sauvegarde) {
    for (cle, e) in a_noter {
        let premier = echecs.and_then(|x| x.get(cle)).is_some_and(|x| x.passages == 1);
        if !premier || envois::drive::passagere(e) || envois::suites::panne_de_passage(&e.echec) {
            continue;
        }
        let nature = (e.raison == Some(envois::drive::RaisonDeGoogle::DrivePlein)).then_some(compagnie::incidents::REFUS);
        if let Err(n) = compagnie::incidents::depuis_exception(racine, e.echec.classe, &e.echec.phrase, nature, "sauvegarde", e.echec.a_part.as_ref(), None) {
            resultat.incidents.push(format!("{} : le journal des incidents ne l'a pas noté : {}", cle, n.phrase));
        }
    }
}
