//! LE PASSAGE DE LA SAUVEGARDE dans le Drive : envoyer ce qui a change (`compagnie::sauvegarde` : l'inventaire, l'index,
//! ce qui persiste), par le client Drive du poste (`routes::coffre::client_drive`) quand aucun n'est donne.

use std::collections::BTreeMap;
use socle::echec::Classe;
use std::path::Path;

use envois::drive::{Client, Drive, ErreurGoogle};

use compagnie::sauvegarde::{
    cause_de, ecrire_index, empreinte, hors_ligne, inventaire, lire_index, phrase_persistante, EchecIndexe, Etat, EtatFichier, Fichier, Index, Reporte, Sauvegarde,
    DOSSIER_DRIVE, GOOGLE_INDISPONIBLE,
};

/// Echecs passagers de suite (chacun deja retente par le client) au-dela desquels le reste du passage est reporte sans appel.
const INDISPONIBLE_D_AFFILEE: usize = 3;

/// `sauvegarder` : envoie ce qui a change. Rend le compte rendu, n'imprime rien.
///
/// `client` : le client Drive a utiliser. `None` : le vrai (`Client::nouveau`), construit AVANT
/// l'inventaire ; il refuse une copie de developpement.
pub fn sauvegarder(racine: &Path, racine_drive: &str, tout: bool, client: Option<&mut dyn Drive>) -> Result<Sauvegarde, ErreurGoogle> {
    let debut = std::time::Instant::now();
    let mut propre: Client;
    let client: &mut dyn Drive = match client {
        Some(c) => c,
        None => {
            propre = crate::coffre::client_drive(racine)?;
            &mut propre
        }
    };
    let (presents, ecartes) = inventaire(racine)?;
    let index = lire_index(racine)?;
    let mut dossiers: BTreeMap<String, String> = match &index {
        Some(i) if !tout => i.dossiers.clone(),
        _ => BTreeMap::new(),
    };

    let mut resultat = Sauvegarde {
        racine_drive: racine_drive.to_string(),
        compte: client.compte(),
        deposes: Vec::new(),
        identiques: 0,
        octets_deposes: 0,
        exclus: ecartes,
        disparus: 0,
        secondes: 0.0,
        incidents: Vec::new(),
        reportes: Vec::new(),
        persistant: None,
        hors_ligne: false,
    };
    // GOOGLE ABSENT POUR DE BON : le client a deja retente chaque appel (`drive::NOUVELLES_TENTATIVES`) ; apres
    // `INDISPONIBLE_D_AFFILEE` echecs passagers de suite, le reste du passage est reporte sans appeler, plutot que de
    // payer sept secondes d'attente par fichier.
    let mut indisponibles_de_suite = 0usize;
    // Ce qui n'a PAS pu etre depose. Ces fichiers n'entrent pas dans l'index : sans cela, le
    // passage suivant les croirait a jour.
    let mut en_echec: std::collections::HashSet<String> = std::collections::HashSet::new();
    let etat_de: BTreeMap<&str, &Etat> = presents.iter().map(|(r, e)| (r.as_str(), e)).collect();

    // Par dossier : un seul listage Drive sert a tous les fichiers qu'il porte.
    let mut par_dossier: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (relatif, _) in &presents {
        let parent = match relatif.rfind('/') {
            Some(i) => &relatif[..i],
            None => ".",
        };
        par_dossier.entry(parent.to_string()).or_default().push(relatif.clone());
    }

    for (dossier, fichiers) in &par_dossier {
        let mut segments: Vec<&str> = vec![racine_drive, DOSSIER_DRIVE];
        if dossier != "." && !dossier.is_empty() {
            segments.extend(dossier.split('/'));
        }
        let cle = segments.join("/");
        if indisponibles_de_suite >= INDISPONIBLE_D_AFFILEE {
            resultat.reportes.push(Reporte { quoi: dossier.clone(), dossier: true, cause: GOOGLE_INDISPONIBLE.into() });
            en_echec.extend(fichiers.iter().cloned());
            continue;
        }
        let essai: Result<(String, BTreeMap<String, envois::drive::Distant>), ErreurGoogle> = (|| {
            let parent = match dossiers.get(&cle) {
                Some(p) => p.clone(),
                None => client.chemin(&segments)?,
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
                if hors_ligne(&e.echec.phrase) {
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
                resultat.reportes.push(Reporte { quoi: dossier.clone(), dossier: true, cause: cause.into() });
                dossiers.remove(&cle);
                en_echec.extend(fichiers.iter().cloned());
                continue;
            }
        };

        let mut tries: Vec<&String> = fichiers.iter().collect();
        tries.sort();
        for relatif in tries {
            if indisponibles_de_suite >= INDISPONIBLE_D_AFFILEE {
                resultat.reportes.push(Reporte { quoi: relatif.clone(), dossier: false, cause: GOOGLE_INDISPONIBLE.into() });
                en_echec.insert(relatif.clone());
                continue;
            }
            let chemin = racine.join(relatif);
            let nom = match relatif.rfind('/') {
                Some(i) => &relatif[i + 1..],
                None => relatif.as_str(),
            };
            let mut faire = || -> Result<Option<EtatFichier>, ErreurGoogle> {
                let locale = empreinte(&chemin).map_err(|e| ErreurGoogle::fichier(&e, &chemin))?;
                // Le fichier de ce nom que `lister` connait (le meme pour decider ET pour remplacer).
                let connu = deja.get(nom);
                let distante = if tout { None } else { connu.and_then(|d| d.md5.clone()).filter(|m| !m.is_empty()) };
                if distante.as_deref() == Some(locale.as_str()) {
                    return Ok(None);
                }
                client.televerser(&chemin, &parent, None, connu)?;
                // Defaut 5 : « modifie » est ce qui est REMPLACE (un fichier que Drive connait, avec ou sans empreinte,
                // ou renvoye par `tout`) ; « neuf » est ce qui est CREE.
                Ok(Some(if connu.is_some() { EtatFichier::Modifie } else { EtatFichier::Neuf }))
            };
            let fait = faire();
            if fait.is_ok() {
                indisponibles_de_suite = 0;
            }
            match fait {
                Ok(None) => resultat.identiques += 1,
                Ok(Some(etat)) => {
                    let octets = etat_de[relatif.as_str()].octets;
                    resultat.deposes.push(Fichier { relatif: relatif.clone(), octets, etat });
                    resultat.octets_deposes += octets;
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
                    resultat.reportes.push(Reporte { quoi: relatif.clone(), dossier: false, cause: cause.into() });
                    en_echec.insert(relatif.clone());
                }
            }
        }
    }

    // L'index n'est ecrit qu'a la fin, et seulement pour ce qui est passe.
    let fichiers: BTreeMap<String, Etat> = presents.iter().filter(|(r, _)| !en_echec.contains(r)).map(|(r, e)| (r.clone(), *e)).collect();
    let maintenant = socle::temps::secondes();
    let quand = socle::paris::iso_minutes(maintenant).replacen('T', " ", 1);
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
                    None => EchecIndexe { depuis: socle::paris::HeureDeSaisie::de(maintenant), passages: 1, cause: r.cause.clone() },
                };
                (cle, e)
            })
            .collect()
    };
    // La derniere sauvegarde complete : celle que l'index d'avant disait deja, sinon le passage d'avant s'il etait complet.
    let derniere_complete = match &index {
        _ if echecs.is_empty() => None,
        Some(Index { derniere_complete: Some(c), .. }) => Some(*c),
        Some(i) if anciens.is_empty() && !i.quand.is_empty() => socle::paris::HeureDeSaisie::lire(&i.quand),
        _ => None,
    };
    let nouvel_index = Index {
        quand,
        racine_drive: racine_drive.to_string(),
        dossiers,
        fichiers,
        derniere_complete,
        echecs: (!echecs.is_empty()).then_some(echecs),
    };
    resultat.persistant = phrase_persistante(&nouvel_index, socle::paris::maintenant_naif())?;
    ecrire_index(racine, &nouvel_index)?;
    resultat.secondes = (socle::temps::duree(debut).as_secs_f64() * 10.0).round() / 10.0;
    Ok(resultat)
}
