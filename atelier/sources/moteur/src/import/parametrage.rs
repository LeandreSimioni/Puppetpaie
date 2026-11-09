//! L'IMPORT QUI PARAMETRE LA COMPAGNIE : ce que les DSN reprises disent de la configuration et que la fiche Entreprise ne
//! porte pas, pose apres l'ecriture des fiches (`ecrite::importer`). La fiche Entreprise recoit deja des DSN ce qu'elle
//! declare (`declarations::dsn_lue::lire` : l'entreprise, le compte qui paie, le numero Conges Spectacles, la licence, le
//! contact DSN, la medecine du travail interne) ; ici se pose ce qui vit ailleurs :
//!
//! - LE CODE RISQUE ET LE TAUX AT/MP (S21.G00.40.040, .043 ; `norme-dsn/texte/bloc-S21.G00.40.txt` l. 1224-1315) dans la
//!   case de la fiche entreprise (`taux_atmp`, `routes::atmp`), une ligne au 1er janvier de chaque annee dont une DSN est
//!   reprise, si aucune ne vaut deja a partir de ce jour ; puis, pour l'annee en cours, le taux collectif de l'arrete pour
//!   un risque « TC » (`routes::atmp::inscrire_l_arrete`).
//! - LES ORGANISMES ET LES CONTRATS DE LA DSN (`config/organismes.json`, `compagnie::organismes`) : chaque contrat
//!   que l'ecran attend encore (`organismes-contrats`, ceux des permanents des que la compagnie en emploie : `poser_organismes_repris`) recoit la reference
//!   (S21.G00.15.001) et l'organisme (S21.G00.15.002) d'une adhesion de la derniere DSN (`norme-dsn/texte/bloc-S21.G00.15.txt`).
//!   Un contrat dont la reference est fixee (celui de la branche) n'a rien a recevoir ; un contrat a saisir recoit
//!   l'adhesion de la DSN qu'aucun contrat fixe ne porte, s'il n'y en a qu'une et qu'un seul contrat l'attend (inference :
//!   c'est la sienne), ou celle dont le libelle nomme ce que le contrat couvre (`poser_organismes`) ; sinon rien ne se
//!   choisit, cela se dit, et « Pour demarrer » le demande.

use serde_json::{json, Map, Value};

use crate::import::dsn_aem::LectureDsn;
use declarations::dsn_lue::Adhesion;
use droit::contexte::Contexte;
use crate::valeur::{valeur_en_texte, vrai};

fn texte(v: &Value, k: &str) -> String {
    v.get(k).filter(|x| vrai(x)).map(valeur_en_texte).map(|s| s.trim().to_string()).unwrap_or_default()
}

/// Le code risque et le taux AT/MP des DSN reprises (voir l'en-tete), chacun dans la case de son annee, a partir de son
/// 1er janvier (`routes::fiches::inscrire_taux_atmp`) ; une annee dont la case dit deja un taux n'en recoit pas. Rend
/// ce qui s'est inscrit, ou pourquoi rien, en phrases.
fn poser_atmp(ctx: &Contexte, lecture: &LectureDsn, faits: &mut Vec<String>) {
    for (an, (mois, code, taux)) in &lecture.atmp {
        let (Some(janvier), true) = (socle::date::Jour::nouveau(*an, 1, 1), *an <= ctx.aujourdhui.an) else { continue };
        if ctx.entreprise_au(&janvier).taux_atmp.and_then(|t| t.depuis).is_some_and(|d| d >= janvier) {
            continue;
        }
        let en_clair = socle::mois::Mois::en_clair(mois);
        let t = droit::fiche::TauxAtmp {
            code_risque: Some(code.clone()),
            valeur: taux.map(|v| droit::fiche::Montant::Flottant(socle::num::flottant(v))),
            depuis: Some(janvier),
            provenance: Some(droit::fiche::ProvenanceAtmp::Import),
            notification: None,
        };
        let source = format!("Lu dans la DSN de {} reprise par l'import (S21.G00.40.040, S21.G00.40.043).", en_clair);
        let ecrit = match droit::bareme::taux_atmp_refuse(&t) {
            Ok(None) => crate::routes::fiches::inscrire_taux_atmp(ctx, t, &source).map_err(|e| e.message),
            Ok(Some(m)) | Err(m) => Err(m),
        };
        let dit = match taux {
            Some(v) => format!("taux {} %", v.normalize().to_string().replace('.', ",")),
            None => "sans taux (pas encore de notification)".to_string(),
        };
        faits.push(match ecrit {
            Ok(_) => format!("Accidents du travail {} : code risque {}, {}, lus dans la DSN de {}.", an, code, dit, en_clair),
            Err(m) => format!("Accidents du travail {} : le code risque {} et le {} de la DSN de {} ne s'inscrivent pas. {}", an, code, dit, en_clair, m),
        });
    }
    // L'annee en cours : le taux collectif de l'arrete pour un risque « TC », comme a l'ouverture.
    match crate::routes::atmp::inscrire_l_arrete(ctx, ctx.aujourdhui.an) {
        Ok(dits) => faits.extend(dits),
        Err(e) => faits.push(format!("Accidents du travail {} : le taux collectif de l'arrêté ne s'inscrit pas. {}", ctx.aujourdhui.an, e.message)),
    }
}

/// Le role d'un contrat attendu (`role` de `organismes-base.json` : `prevoyance`, `sante`) que le libelle d'une adhesion
/// nomme, et lui seul : « SANTE CCNEAC » nomme la sante, « PREVOYANCE CCNEAC » la prevoyance, « CCNEAC » rien.
fn role_nomme(libelle: &str) -> Option<&'static str> {
    let mots = crate::import::bulletin_lu::jetons(libelle);
    let a = |m: &str| mots.iter().any(|x| x == m);
    match (a("prevoyance"), a("sante")) {
        (true, false) => Some("prevoyance"),
        (false, true) => Some("sante"),
        _ => None,
    }
}

/// Les organismes et les contrats de la DSN (voir l'en-tete), pour les contrats que l'ecran attend encore. Un contrat
/// deja saisi a son libelle et n'attend plus rien.
///
/// QUEL CONTRAT DE LA DSN EST LE SIEN : la DSN dit la reference du contrat et son organisme (S21.G00.15.001, .002), pas
/// ce qu'il couvre (`bloc-S21.G00.15.txt` : « Référence du contrat de Prévoyance » vaut pour la prevoyance, la sante et
/// la retraite supplementaire ; ni le bloc S21.G00.70 ni les composants S21.G00.79 ne distinguent la sante). Un seul
/// contrat a saisir et une seule adhesion libre : c'est la sienne. Sinon, un contrat a saisir de role prevoyance ou
/// sante recoit l'adhesion libre dont le libelle nomme ce role, et elle seule (inference : la compagnie a nomme son
/// contrat par ce qu'il couvre). Ce qui ne se rattache pas ainsi se dit, et « Pour demarrer » le demande.
fn poser_organismes(ctx: &Contexte, lecture: &LectureDsn, faits: &mut Vec<String>) {
    if lecture.adhesions.is_empty() {
        return;
    }
    let Ok(ecran) = crate::compagnie::organismes::lire(ctx, &json!({})) else { return };
    let lignes: Vec<Value> = ecran["contrats"].as_array().cloned().unwrap_or_default();
    let fixes: Vec<String> = lignes.iter().map(|l| texte(l, "libelle").to_uppercase()).filter(|l| !l.is_empty()).collect();
    let a_saisir: Vec<&Value> = lignes.iter().filter(|l| texte(l, "libelle").is_empty()).collect();
    let libelle = |a: &Adhesion| a.libelle.trim().to_string();
    let organisme = |a: &Adhesion| a.organisme.as_deref().unwrap_or_default().trim().to_string();
    let libres: Vec<&Adhesion> =
        lecture.adhesions.iter().filter(|a| !libelle(a).is_empty() && !fixes.contains(&libelle(a).to_uppercase())).collect();
    if a_saisir.is_empty() || libres.is_empty() {
        return;
    }
    let roles: Vec<(String, String)> = droit::organismes::attendus(ctx)
        .unwrap_or_default()
        .iter()
        .map(|s| (s.cle.trim().to_string(), s.role.as_deref().unwrap_or_default().trim().to_string()))
        .collect();
    let role_de = |l: &Value| roles.iter().find(|(c, _)| *c == texte(l, "cle")).map(|(_, r)| r.clone()).unwrap_or_default();
    let mut paires: Vec<(&Value, &Adhesion, bool)> = Vec::new();
    if let ([l], [a]) = (a_saisir.as_slice(), libres.as_slice()) {
        paires.push((l, a, false));
    } else {
        for l in &a_saisir {
            let r = role_de(l);
            let siens: Vec<&&Adhesion> = libres.iter().filter(|a| role_nomme(&libelle(a)) == Some(r.as_str())).collect();
            let rivaux = a_saisir.iter().filter(|x| role_de(x) == r).count();
            if let ([a], 1) = (siens.as_slice(), rivaux) {
                paires.push((l, a, true));
            }
        }
    }
    let mut saisies = Map::new();
    for (l, a, _) in &paires {
        saisies.insert(texte(l, "cle"), json!({"libelle": libelle(a), "organisme": organisme(a)}));
    }
    if !saisies.is_empty() && crate::compagnie::organismes::enregistrer(ctx, &json!({"contrats": saisies})).is_ok() {
        for (l, a, par_libelle) in &paires {
            faits.push(format!(
                "{} : contrat {} chez {}, lu dans la DSN reprise (S21.G00.15.001, S21.G00.15.002){}.",
                texte(l, "quoi"),
                libelle(a),
                organisme(a),
                if *par_libelle { ", rattaché par ce que son libellé nomme" } else { "" }
            ));
        }
    } else {
        paires.clear();
    }
    let restes: Vec<String> = libres.iter().filter(|a| !paires.iter().any(|(_, p, _)| std::ptr::eq(*p, **a))).map(|a| libelle(a)).collect();
    let attendent = a_saisir.iter().any(|l| !paires.iter().any(|(p, _, _)| std::ptr::eq(*p, *l)));
    if attendent && !restes.is_empty() {
        faits.push(format!(
            "Contrats de la DSN reprise que rien ne rattache à un contrat attendu : {} ; à choisir dans Configuration › Organismes et contrats.",
            restes.join(", ")
        ));
    }
}

/// Ce que les DSN reprises de la compagnie (`historique/bulletins/`) disent de sa configuration, pose ; rend ce qui l'a ete.
pub(crate) fn parametrer(ctx: &Contexte) -> Vec<String> {
    let lecture = match crate::import::dsn_aem::lire_dsn(&ctx.racine.join("historique").join("bulletins")) {
        Ok(l) => l,
        Err(e) => return vec![e.phrase()],
    };
    if lecture.fichiers == 0 {
        return Vec::new();
    }
    let mut faits = Vec::new();
    poser_atmp(ctx, &lecture, &mut faits);
    poser_organismes(ctx, &lecture, &mut faits);
    poser_taux_pas(ctx, &mut faits);
    faits
}

/// LE TAUX DE PRELEVEMENT A LA SOURCE que la DGFiP a transmis a la compagnie et que ses DSN reprises appliquent
/// (S21.G00.50.006 a .008) : au magasin du PAS, comme un taux recu, date du jour du versement qui
/// l'applique (`compagnie::taux_pas::derniers_taux_des_dsn`, une inference). La validite se lit ensuite comme pour tout taux recu (`droit::pas`).
fn poser_taux_pas(ctx: &Contexte, faits: &mut Vec<String>) {
    let dossier = ctx.racine.join("historique").join("bulletins");
    let releves = match crate::compagnie::taux_pas::derniers_taux_des_dsn(ctx, &dossier) {
        Ok(r) => r,
        Err(e) => return faits.push(e.phrase()),
    };
    let Ok(changes) = crate::compagnie::taux_pas::enregistrer(ctx, &releves, "DSN reprises par l'import") else { return };
    for r in &releves {
        let cle = r.salarie.clone();
        if !changes.iter().any(|c| c.starts_with(&format!("{} :", cle))) {
            continue;
        }
        let qui = ctx.salaries.get(cle.as_str()).and_then(|s| s.nomme());
        faits.push(format!(
            "Prélèvement à la source de {} : taux {} % transmis par la DGFiP, appliqué dans la DSN de {} (S21.G00.50.006 à .008).",
            qui.filter(|q| !q.is_empty()).unwrap_or(cle),
            r.taux.clone().unwrap_or_default().replace('.', ","),
            // Le mois du versement qui l'applique (`mis_a_disposition`, AAAAMMJJ000000) : celui que la DSN declare.
            r.mis_a_disposition.get(..8).and_then(socle::date::lire_aaaammjj).map(|j| j.mois_civil().libelle(socle::mois::Graphie::Accentuee)).unwrap_or_default()
        ));
    }
}

/// Les contrats des permanents que les DSN reprises portent, poses quand la compagnie se met a employer un permanent
/// (la nature d'un contrat repris dite, `import::nature::dire`) : l'ecran ne les attendait pas encore a l'import.
pub(crate) fn poser_organismes_repris(ctx: &Contexte) -> Vec<String> {
    let lecture = match crate::import::dsn_aem::lire_dsn(&ctx.racine.join("historique").join("bulletins")) {
        Ok(l) => l,
        Err(e) => return vec![e.phrase()],
    };
    let mut faits = Vec::new();
    if lecture.fichiers > 0 {
        poser_organismes(ctx, &lecture, &mut faits);
    }
    faits
}

