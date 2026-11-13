//! La releve des reponses « Je signe ». La demande de signature (`crate::gestes::signature::demander_signature`) a garde,
//! au journal, un CODE (`SIG-XXXXXX`) par courriel parti. Cette releve cherche chaque code attendu dans la boite
//! (`envois::releve_signatures::chercher_reponse`, le tri strict) et fait tout ce qui suit : la preuve archivee
//! (`Signature_<salarie>_<code>.eml`), le contrat contresigne (`depart::contresigner` : le PDF parti, plus la mention de l'artiste dans sa case), le journal (sur la ligne de
//! l'envoi, par contrat, date de la SIGNATURE, `crate::contrats::contrat_recu`), et le renvoi du
//! contresigne au salarie (`envois::releve_signatures::renvoyer_contresigne`).
//!
//! NE LEVE PAS pour une boite absente : c'est un travail de fond, il dit ce qui l'empeche dans `incidents` et rend la
//! main.
//!
//! Dans une compagnie fictive, les salaries « signent » (`boite_des_signatures`) : chaque demande partie
//! recoit sa reponse aussitot -- celle que le bouton du courriel prepare --, et c'est la VRAIE releve qui la lit.
//! `relever_signatures_fictive` ne joue rien dans une compagnie qui n'est pas une compagnie fictive (la garde de
//! l'horloge).

use std::path::PathBuf;
use socle::echec::Classe;
use socle::fichiers::NomDeFichier;
use socle::mois::Mois;

use droit::contexte::Contexte;
use envois::courriel::Courriel;
use envois::envoi;
use envois::etats;
use envois::releve_signatures::{chercher_reponse, identifiants_de, renvoyer_contresigne, Contresigne, Reponse};
pub use envois::releve_signatures::{ReleveSignatures, SignatureRecue};
use socle::echec::R;
use socle::echec::erreur;
use socle::echec::Echec;
use crate::horloge::{HorlogeFictive, QUI_HORLOGE};
use envois::simulation_courriels::BoiteQuiRepond;

// ------------------------------------------------------------------ la compagnie fictive

/// La boite de la compagnie fictive, vue par la releve. Chaque demande de signature partie
/// y trouve aussitot la reponse « Je signe » que le bouton de son courriel prepare (a defaut, une reponse
/// au code dans les termes de `CORPS_REPONSE`) ; la vraie releve la lit, note la signature, contresigne le contrat et le
/// renvoie au salarie.
pub(crate) fn boite_des_signatures(ctx: &Contexte) -> R<BoiteQuiRepond> {
    let mut boite = BoiteQuiRepond::nouvelle(ctx)?;
    let lignes = envois::journal::lu(&ctx.racine).map(|j| j.lignes)?;
    let attendues = etats::signatures_attendues(|c| ctx.salaries.get(c).map(|f| &f.champs), &lignes, None);
    // Les courriels joues, par leur `Message-ID` : la demande retrouve ainsi son bouton.
    let envoyes = envois::simulation_courriels::courriels_par_identifiant(&ctx.racine)?;
    for a in &attendues {
        // Une demande remise en main propre n'a pas d'adresse : personne n'y repond par courriel.
        let Some(adresse) = a.adresse.as_deref() else { continue };
        let code = a.code.clone();
        let nom = a.qui();
        let (sujet, corps) = match envois::simulation_courriels::bouton_de_la_demande(&envoyes, &identifiants_de(a))? {
            Some(b) => b,
            None => (
                "Re: contrat".to_string(),
                envoi::formater(
                    crate::gestes::signature::CORPS_REPONSE,
                    &[("nom", nom), ("quoi", "le contrat d'engagement".to_string()), ("code", code.clone())],
                ),
            ),
        };
        let brut = envois::simulation_courriels::reponse_jouee(adresse, Courriel::compte(&boite), &sujet, &corps, &format!("signature-{}", code))
            .map_err(|m| m.reclasser(Classe::Illisible))?;
        boite.repondre(&code, brut);
    }
    Ok(boite)
}

/// Les salaries de la compagnie fictive signent, et la VRAIE releve lit leur reponse. Rien
/// ne se joue dans une compagnie qui n'est pas une compagnie fictive (la garde de l'horloge) : la releve rendue est alors
/// vide.
pub(crate) fn relever_signatures_fictive(ctx: &Contexte) -> R<ReleveSignatures> {
    if HorlogeFictive::verifier(&ctx.racine)?.is_none() {
        return Ok(ReleveSignatures::nouvelle());
    }
    let mut boite = boite_des_signatures(ctx)?;
    relever_signatures(ctx, Some(&mut boite), None, QUI_HORLOGE)
}


#[track_caller]
fn os(m: impl Into<String>) -> Echec {
    erreur(Classe::Disque, m)
}

/// La boite ou chercher les reponses, quand l'appelant n'en donne pas, selon le branchement de la compagnie
/// (`bridage::branchement`) : le simulateur (les salaries fictifs repondent) ; le message, rien ne se releve (la releve
/// renverrait le contrat contresigne au salarie : un envoi) ; le monde, la vraie boite.
fn boite_de_la_releve(ctx: &Contexte) -> R<BoiteDeLaReleve> {
    match compagnie::bridage::branchement(&ctx.racine)? {
        envois::branchement::Branchement::Simulateur => return Ok(BoiteDeLaReleve::Ouverte(Box::new(boite_des_signatures(ctx)?))),
        envois::branchement::Branchement::Message(r) => return Ok(BoiteDeLaReleve::Aucune(r.phrase.to_string())),
        envois::branchement::Branchement::Monde => {}
    }
    if !compagnie::boite::disponible(&ctx.racine)? {
        return Ok(BoiteDeLaReleve::Aucune(
            "Aucune boite courriel : les signatures ne peuvent pas etre relevees. Poser le mot de passe d'application de la boite (Configuration > Mots de passe)."
                .to_string(),
        ));
    }
    let vrai_client = compagnie::boite::client_du_contexte(ctx)?;
    Ok(BoiteDeLaReleve::Ouverte(Box::new(vrai_client)))
}

/// La boite de la releve, ou l'incident qui dit pourquoi rien ne se releve.
enum BoiteDeLaReleve {
    Ouverte(Box<dyn Courriel>),
    Aucune(String),
}

/// Cherche dans la boite les reponses « Je signe » encore attendues, et fait tout ce qui suit.
///
/// `client` : la boite, ouverte ici quand l'appelant n'en donne pas (`boite_de_la_releve`) ; sans boite, un incident et
/// rien d'autre. `mois` : seulement les demandes de ce mois. `qui` : `veille`, `interface`, `ligne de commande`,
/// `horloge`. Une erreur de transport ou de lecture se dit dans `incidents` ; seul un fichier qui ne s'ecrit pas remonte
/// en `Err`.
pub(crate) fn relever_signatures(ctx: &Contexte, client: Option<&mut dyn Courriel>, mois: Option<Mois>, qui: &str) -> R<ReleveSignatures> {
    let mut r = ReleveSignatures::nouvelle();
    let lignes = envois::journal::lu(&ctx.racine).map(|j| j.lignes)?;
    let attendues = etats::signatures_attendues(|c| ctx.salaries.get(c).map(|f| &f.champs), &lignes, mois);
    r.attendues = attendues.len();
    if attendues.is_empty() {
        return Ok(r);
    }
    let mut ouverte: Option<Box<dyn Courriel>> = None;
    let client: &mut dyn Courriel = match client {
        Some(c) => c,
        None => match boite_de_la_releve(ctx)? {
            BoiteDeLaReleve::Ouverte(b) => ouverte.insert(b).as_mut(),
            BoiteDeLaReleve::Aucune(incident) => {
                r.incidents.push(incident);
                return Ok(r);
            }
        },
    };

    let (signataire, raison) = crate::gestes::signature::signataire_et_raison(ctx)?;
    let expediteur = client.compte().to_lowercase();

    for a in &attendues {
        let code = a.code.clone();
        let a_nom = a.qui();
        let Some(a_mois) = a.mois else {
            r.incidents.push(format!("{} : la demande de signature ne dit pas son mois, elle ne se relève pas.", a_nom));
            continue;
        };
        let a_salarie = a.salarie.clone();
        let contrats = a.contrats.clone();
        let retenue = match chercher_reponse(client, a, &expediteur, &mut r.incidents) {
            Ok(x) => x,
            // Une erreur de transport ne se corrige pas en essayant le code suivant.
            Err(()) => break,
        };
        let Some(Reponse { identifiant: ident, brut, de, quand }) = retenue else { continue };

        // La preuve archivee : le message tel qu'il est arrive.
        let dossier = ctx.racine.join("sorties").join(a_mois.to_string()).join("envoi");
        std::fs::create_dir_all(&dossier).map_err(|e| os(format!("{} : {}", dossier.display(), e)))?;
        let preuve = dossier.join(format!("Signature_{}_{}.eml", a_salarie, code));
        std::fs::write(&preuve, &brut).map_err(|e| os(format!("{} : {}", preuve.display(), e)))?;
        let nom_preuve = NomDeFichier::du(&preuve)?.to_string();

        // UN PDF PAR FICHIER DISTINCT : des contrats groupes n'en ont qu'un (un par page) ; des avenants groupes en ont un
        // chacun.
        let mut fichiers: Vec<(Mois, NomDeFichier)> = Vec::new();
        for c in &contrats {
            let paire = (c.mois, c.fichier.clone());
            if !fichiers.contains(&paire) {
                fichiers.push(paire);
            }
        }
        let quand_lisible = quand.dit();
        // La mention porte le nom que la fiche dit : sans lui, rien ne se contresigne.
        let mention = a.nom.as_deref().map(|n| documents::contrat::mention_signature(n, &quand_lisible, &de, &code));
        let mut signes: Vec<(NomDeFichier, PathBuf)> = Vec::new();
        for (mois_f, nom_f) in &fichiers {
            let envoye = ctx.racine.join("sorties").join(mois_f.to_string()).join("envoi").join(nom_f);
            // La souche du fichier parti : son nom sans son extension (un nom qui n'en a pas est sa propre souche).
            let souche = match envoye.file_stem() {
                Some(s) => s.to_string_lossy().into_owned(),
                None => nom_f.to_string(),
            };
            let signe = envoye.with_file_name(envois::releve_signatures::nom_contresigne(&souche));
            // L'EXEMPLAIRE CONTRESIGNE : le PDF parti avec ce code, octet pour octet, plus la mention de l'artiste dans sa case.
            let fait = match &mention {
                Some(m) => crate::gestes::depart::contresigner(ctx, &lignes, &code, &envoye, &signe, m).map_err(|e| e.phrase),
                None => Err("la fiche du salarié ne dit pas son nom".to_string()),
            };
            match fait {
                Ok(()) => match signes.iter_mut().find(|(n, _)| n == nom_f) {
                    Some(deja) => deja.1 = signe,
                    None => signes.push((nom_f.clone(), signe)),
                },
                Err(e) => r.incidents.push(format!("{} : contrat contresigne impossible ({}).", a_nom, e)),
            }
        }
        let noms_signes = signes.iter().map(|(_, p)| NomDeFichier::du(p)).collect::<R<Vec<_>>>()?;

        // LE JOURNAL D'ABORD : une fois la ligne ecrite, la releve suivante n'attend plus ce code, et le renvoi ne se fera
        // pas deux fois. Un contrat que `contrat_recu` refuse (la paie n'est plus dans la saisie du mois...) devient un
        // incident NOMME (le salarie, le contrat, la raison) et la releve continue.
        // Une demande qui ne dit pas son acte est celle du contrat lui-meme (la forme du journal des envois,
        // `envois::etats`) : l'absence passe telle quelle.
        let acte = a.acte.clone();
        let est_avenant = acte.as_deref() == Some("avenant");
        let mut notes = 0usize;
        for c in &contrats {
            let sien = signes.iter().find(|(n, _)| *n == c.fichier).map(|(_, p)| NomDeFichier::du(p)).transpose()?;
            let courriel = crate::contrats::ParCourriel {
                code: code.clone(),
                adresse: de.clone(),
                quand: quand.clone(),
                message: ident.clone(),
                preuve: nom_preuve.clone(),
                fichier_signe: sien,
                // La mention posee : l'exemplaire contresigne se refait du contrat parti et d'elle (`depart::piece_partie`).
                mention: mention.clone(),
                acte: acte.clone(),
                avenant: if est_avenant { c.avenant.filter(|n| *n != 0).or(a.avenant) } else { None },
            };
            let debut = c.debut.iso();
            // Le contrat que cette reponse signe, par son numero : le jumeau du meme jour garde sa propre attente.
            let note = match a.mois {
                Some(mois) => {
                    let charge = crate::contrats::ChargeRecu { mois, salarie: c.salarie.clone(), debut: c.debut, numero: c.numero.clone(), avenant: None, recu: true };
                    crate::contrats::contrat_recu(ctx, &charge, qui, Some(&courriel)).map(|_| ())
                }
                None => Err(erreur(Classe::Refuse, "la demande de signature ne dit pas son mois")),
            };
            match note {
                Ok(_) => notes += 1,
                Err(e) => r.incidents.push(format!(
                    "{} : la signature de {} est arrivee mais le contrat du {} n'a pas pu etre enregistre ({}) ; il n'est pas note signe.",
                    a_nom, code, debut, e.phrase
                )),
            }
        }
        if notes == 0 {
            // Rien n'est note : la signature reste attendue (elle sera relevee de nouveau, et l'incident redit), rien n'est renvoye.
            continue;
        }
        r.recues.push(SignatureRecue {
            mois: a_mois,
            salarie: a_salarie.clone(),
            nom: a_nom.clone(),
            code: code.clone(),
            adresse: de.clone(),
            quand: quand.clone(),
            contrats: contrats.clone(),
            fichier_signe: noms_signes.clone(),
            preuve: nom_preuve.clone(),
            renvoye: false,
        });
        if signes.is_empty() {
            continue;
        }

        // Le contrat contresigne, renvoye au salarie.
        let pdfs: Vec<PathBuf> = signes.iter().map(|(_, p)| p.clone()).collect();
        let contresigne = Contresigne {
            a,
            contrats: &contrats,
            est_avenant,
            de: &de,
            quand_lisible: &quand_lisible,
            signes: &pdfs,
            signataire: &signataire,
            raison: &raison,
            expediteur: &expediteur,
            mois: a_mois,
        };
        match renvoyer_contresigne(&ctx.racine, client, &contresigne, qui) {
            Ok(()) => {
                let dernier = r.recues.len() - 1;
                r.recues[dernier].renvoye = true;
            }
            Err(e) => r.incidents.push(format!(
                "{} : le contrat contresigne n'a pas pu etre renvoye ({}). Il est dans {}.",
                a_nom,
                e.phrase,
                noms_signes.iter().map(NomDeFichier::texte).collect::<Vec<_>>().join(", ")
            )),
        }
    }
    client.fermer()?;
    Ok(r)
}
