//! `journal-reprise` : remet au journal les depots DSN que
//! les retours archives (`retours/*.xml`) attestent. `ecrire` absent : elle DIT ce qu'elle ferait. Aucun reseau,
//! aucun depot ; pas de mot de passe (rien ne part).
//!
//! Idempotente : un flux deja au journal est saute. Une ligne reprise porte la date du depot (lue dans la piece)
//! et sa `origine` (`retours/<piece>`) : c'est une reconstitution, pas un acte de ce logiciel.
//!
//! Une piece qui ne se lit pas (fichier inaccessible, XML mal forme) est NOMMEE dans `illisibles`, « nom (classe) »,
//! et les autres passent ; un XML bien forme qui n'est pas un bilan de depot est saute sans
//! bruit : le dossier melange des retours de toutes natures.
//!
//! UN DEPOT QUE LES RETOURS ATTESTENT EST UN DEPOT (sinon un mois importe se dit en retard alors qu'il est deja
//! declare) : le releve des retours et l'import remettent d'eux-memes au journal ce que les bilans archives attestent
//! (`reprendre_les_depots`), comme l'effectif se refait seul des retours 118. Un test que ce logiciel n'a pas envoye reste dehors (voir `reprise`).

use droit::fiche::Lue as _;
use std::path::{Path, PathBuf};

use serde::Serialize;

use droit::contexte::Contexte;
use socle::echec::R;
use socle::echec::erreur;
use socle::mois::Mois;
use crate::etats::AvecIncidents;
use crate::journal::{self, DetailDsn, Envoi, Ligne, Mode};
use crate::retours::Bilan;

/// La reponse d'une ligne reprise sans certificat de conformite : le journal ecrit la reponse en texte
/// (`journal::Envoi::reponse`), et la plate-forme n'en a pas dit.
const SANS_CERTIFICAT: &str = "";

/// Le depot que le bilan dit accepte : l'etat de la declaration, sinon celui de l'envoi, est OK.
fn accepte(b: &Bilan) -> bool {
    b.verdict().is_some_and(|v| v.eq_ignore_ascii_case("OK"))
}

/// Un depot reel (`essai_reel` 02), et non d'essai.
fn reel(b: &Bilan) -> bool {
    b.essai_reel.as_deref() == Some(crate::retours::ESSAI_REEL)
}

/// Un depot dont une piece de `retours/` temoigne.
struct Atteste {
    flux: String,
    bilan: Bilan,
    pieces: Vec<String>,
    deja: bool,
}

/// Les fichiers d'un dossier des retours dont le nom passe `garder`, tries ; aucun sans dossier.
fn fichiers_du_dossier(dossier: &Path, garder: impl Fn(&str) -> bool) -> R<Vec<(String, PathBuf)>> {
    let lecture = match std::fs::read_dir(dossier) {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(erreur("OSError", socle::echec::du_disque(&dossier, &e))),
    };
    let mut noms = Vec::new();
    for e in lecture {
        let e = e.map_err(|x| erreur("OSError", socle::echec::du_disque(&dossier, &x)))?;
        let nom = e.file_name().to_string_lossy().into_owned();
        if garder(&nom) && e.path().is_file() {
            noms.push((nom, e.path()));
        }
    }
    noms.sort();
    Ok(noms)
}

/// Lecture pure.
fn depots_attestes(ctx: &Contexte) -> R<(Vec<Atteste>, Vec<String>)> {
    let dossier = ctx.racine.join(crate::retours::RETOURS);
    std::fs::create_dir_all(&dossier).map_err(|e| erreur("OSError", socle::echec::du_disque(&dossier, &e)))?;
    let mut par_flux: Vec<(String, Vec<(Bilan, String)>)> = vec![];
    let mut illisibles: Vec<String> = vec![];
    for (nom, p) in fichiers_du_dossier(&dossier, |n| n.ends_with(".xml"))? {
        let octets = match std::fs::read(&p) {
            Ok(o) => o,
            Err(e) => {
                illisibles.push(format!("{} (OSError : {})", nom, e.kind()));
                continue;
            }
        };
        let b = match crate::retours::bilan_d_octets(&octets) {
            Ok(Some(b)) => b,
            Ok(None) => continue,
            Err(e) => {
                let classe = match e.classe {
                    Some(c) => c,
                    None => "Exception",
                };
                illisibles.push(format!("{} ({})", nom, classe));
                continue;
            }
        };
        match par_flux.iter_mut().find(|(f, _)| *f == b.flux) {
            Some((_, l)) => l.push((b, nom.clone())),
            None => par_flux.push((b.flux.clone(), vec![(b, nom.clone())])),
        }
    }
    let connus: Vec<String> = journal::lire(&ctx.racine, None, None, false)
        .map_err(socle::echec::Echec::from)?
        .iter()
        .filter(|l| !l.flux().is_empty())
        .map(|l| l.flux().to_string())
        .collect();
    let mut out: Vec<Atteste> = Vec::new();
    for (flux, couples) in par_flux {
        let bilans: Vec<Bilan> = couples.iter().map(|(b, _)| b.clone()).collect();
        let mut pieces: Vec<String> = couples.into_iter().map(|(_, n)| n).collect();
        pieces.sort();
        // Chaque flux a au moins un bilan : il ne s'y range que par lui.
        let Some(bilan) = crate::retours::fusionner_bilans(&bilans) else { continue };
        out.push(Atteste { deja: connus.contains(&flux), flux, bilan, pieces });
    }
    out.sort_by(|a, b| a.bilan.recu_le.cmp(&b.bilan.recu_le));
    Ok((out, illisibles))
}

/// UN DEPOT QUE LES RETOURS ATTESTENT, tel que la reprise le montre : son flux, quand il a ete recu, le mois qu'il
/// declare, reel ou test, accepte ou non, si son verdict est connu, le logiciel qui l'a depose, ses pieces, et s'il est
/// deja au journal. Ce que le bilan ne dit pas est vide.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DepotVu {
    pub flux: String,
    pub quand: Option<String>,
    pub mois: Option<Mois>,
    pub mode: Mode,
    pub accepte: bool,
    pub verdict_connu: bool,
    pub logiciel: Option<String>,
    pub pieces: Vec<String>,
    pub deja: bool,
}

/// CE QUE LA REPRISE REND : les depots trouves, ceux qu'elle ajoute (ou ajouterait), ceux deja au journal, ceux qu'elle
/// ne peut dater, les pieces illisibles, si elle a ecrit, et combien de retours se sont ranges en face.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Reprise {
    pub trouves: Vec<DepotVu>,
    pub ajoutes: Vec<DepotVu>,
    pub deja: Vec<DepotVu>,
    pub sans_mois: Vec<DepotVu>,
    pub illisibles: Vec<String>,
    pub ecrit: bool,
    pub rattaches: i64,
}

/// `journal-reprise` : `ecrire` absent, elle dit ce qu'elle ferait ; `qui` signe les lignes reprises. Rend le rapport, et les
/// incidents que le rattachement des retours a releves (`moteur` les note).
pub fn reprise(ctx: &Contexte, ecrire: bool, qui: &str, maintenant: &str) -> AvecIncidents<Reprise> {
    let mut incidents = Vec::new();
    let resultat = reprise_et_incidents(ctx, ecrire, qui, maintenant, &mut incidents);
    AvecIncidents { resultat, incidents }
}

fn reprise_et_incidents(ctx: &Contexte, ecrire: bool, qui: &str, maintenant: &str, incidents: &mut Vec<crate::etats::IncidentDeRetour>) -> R<Reprise> {
    let (attestes, illisibles) = depots_attestes(ctx)?;
    let mut rapport = Reprise { trouves: vec![], ajoutes: vec![], deja: vec![], sans_mois: vec![], illisibles, ecrit: ecrire, rattaches: 0 };
    let mut a_ecrire: Vec<&Atteste> = Vec::new();
    for d in &attestes {
        let b = &d.bilan;
        // UN TEST QUE CE LOGICIEL N'A PAS ENVOYE NE S'AFFICHE PAS ET NE SE REPREND PAS : sous le
        // meme compte net-entreprises, les essais de l'atelier (ligne de commande, `moteur --dev`, seances du nuage)
        // reviennent dans les retours de la compagnie. Un test n'engage rien : absent du journal, il vient d'ailleurs.
        // Il compte pourtant pour l'ordre (`ordre_signalements::dernier_ordre_des_mensuelles`, lu en silence). Un REEL
        // absent du journal est un depot de la compagnie (l'atelier n'en fait jamais) : il se reprend.
        if !reel(b) && !d.deja {
            continue;
        }
        let vue = DepotVu {
            flux: d.flux.clone(),
            quand: b.recu_le.clone(),
            mois: b.mois,
            mode: Mode::de(reel(b)),
            accepte: accepte(b),
            verdict_connu: b.etat_declaration.is_some(),
            logiciel: b.logiciel.clone(),
            pieces: d.pieces.clone(),
            deja: d.deja,
        };
        rapport.trouves.push(vue.clone());
        if d.deja {
            rapport.deja.push(vue);
            continue;
        }
        if b.recu_le.is_none() {
            // Sans date, la ligne ne saurait pas dire QUAND : on refuse plutot que de dater d'aujourd'hui.
            rapport.sans_mois.push(vue);
            continue;
        }
        rapport.ajoutes.push(vue);
        a_ecrire.push(d);
    }
    if !ecrire {
        return Ok(rapport);
    }
    for d in a_ecrire {
        let b = &d.bilan;
        let (Some(quand), Some(piece)) = (b.recu_le.clone(), d.pieces.last()) else { continue };
        let salaries = b.salaries.as_deref().map(|s| s.trim().parse::<usize>().map_err(|_| erreur("ValueError", socle::echec::cle("Le nombre de salariés d'un bilan ne se lit pas.", "salaries")))).transpose()?;
        let detail = DetailDsn {
            reconstruit: true,
            logiciel: b.logiciel.clone(),
            type_declaration: b.type_declaration.clone(),
            fraction: b.fraction.clone(),
            ordre: b.ordre.clone(),
            salaries,
            contrats: b.contrats.clone(),
            etat_envoi: b.etat_envoi.clone(),
            etat_declaration: b.etat_declaration.clone(),
            verdict_connu: Some(b.etat_declaration.is_some()),
            pieces: Some(d.pieces.clone()),
            ..Default::default()
        };
        let reponse = match &b.certificat {
            Some(c) => c.as_str(),
            None => SANS_CERTIFICAT,
        };
        let mut e = Envoi::nouveau(Mode::de(reel(b)), qui, accepte(b), "net-entreprises", reponse, detail);
        e.mois = b.mois;
        e.flux = d.flux.clone();
        e.quand = quand;
        e.origine = format!("retours/{}", piece);
        journal::noter(&ctx.racine, Ligne::Dsn(e), None, maintenant).map_err(socle::echec::Echec::from)?;
    }
    // Les retours deja archives se rangent en face des lignes qui viennent d'apparaitre -- par le flux.
    let p = ctx.racine.join(crate::retours::RETOURS).join("journal.json");
    if p.exists() {
        let retours: Vec<crate::retours::Annonce> =
            socle::forme::lire_fichier(&p, "Le journal des retours vus ne se lit pas.").map_err(|m| erreur("ValueError", m))?;
        let r = crate::etats::rattacher_retours_ecrits(&ctx.racine, &retours);
        incidents.extend(r.incidents);
        rapport.rattaches = r.resultat?;
    }
    Ok(rapport)
}

/// Remet au journal, sans qu'on le demande, les depots DSN que les retours archives attestent et que le journal ignore :
/// la reprise ecrite (`reprise` avec `ecrire`), par le releve des retours et par l'import. Rend le nombre de lignes
/// ajoutees. Idempotente (par le flux).
pub fn reprendre_les_depots(ctx: &Contexte, qui: &str, maintenant: &str) -> AvecIncidents<usize> {
    let r = reprise(ctx, true, qui, maintenant);
    AvecIncidents { resultat: r.resultat.map(|v| v.ajoutes.len()), incidents: r.incidents }
}

// ------------------------------------------------------------- la reprise du canal DPAE

/// Le NIR (sans espaces autour) -> la cle de la fiche. Si deux fiches portent le meme NIR, la derniere gagne.
fn salarie_par_nir(ctx: &Contexte) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = Vec::new();
    for (cle, s) in &ctx.salaries {
        let Some(nir) = s.nir.dite().filter(|n| !n.is_empty()) else { continue };
        let nir = nir.trim().to_string();
        match v.iter_mut().find(|(n, _)| *n == nir) {
            Some(e) => e.1 = cle.to_string(),
            None => v.push((nir, cle.to_string())),
        }
    }
    v
}

/// `20260904-224552_nature41_WSxxxx.txt` -> `WSxxxx` ; aucun pour un nom qui n'a pas cette forme.
fn flux_du_nom(nom: &str) -> Option<&str> {
    let reste = nom.splitn(3, '_').nth(2)?;
    Some(match reste.rsplit_once('.') {
        Some((a, _)) => a,
        None => reste,
    })
}

/// Une embauche qu'un certificat de DPAE atteste : la cle du salarie, et son premier jour (AAAA-MM-JJ).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EmbaucheAttestee {
    pub salarie: String,
    pub debut: String,
}

/// UNE DPAE QU'UN CERTIFICAT DE L'URSSAF ATTESTE (nature 41) : son flux (tel que le nom de la piece le porte), quand elle
/// a ete deposee et le mois de sa premiere embauche (quand le certificat les dit), la piece, et ses embauches.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DpaeAttestee {
    pub flux: String,
    pub quand: Option<String>,
    pub mois: Option<String>,
    pub piece: String,
    pub embauches: Vec<EmbaucheAttestee>,
}

/// Les DPAE dont `retours/dpae/` porte le certificat (nature 41). Une entree par FLUX, avec
/// les embauches qu'il declare ; les NIR qu'on ne sait rattacher a aucune fiche sont DITS, jamais devines. LECTURE PURE.
pub fn embauches_attestees(ctx: &Contexte) -> R<(Vec<DpaeAttestee>, Vec<String>)> {
    let dossier = ctx.racine.join(crate::retours::RETOURS_DPAE);
    let par_nir = salarie_par_nir(ctx);
    let (mut sortie, mut inconnus): (Vec<DpaeAttestee>, Vec<String>) = (vec![], vec![]);
    for (nom, p) in fichiers_du_dossier(&dossier, |n| crate::retours::archive_de_nature(n, "41"))? {
        let octets = std::fs::read(&p).map_err(|e| erreur("OSError", socle::echec::du_disque(&p, &e)))?;
        let texte: String = octets.iter().map(|&b| b as char).collect();
        let (mut embauches, mut quand, mut mois) = (Vec::new(), None, None);
        for ligne in declarations::dpae::relire(&texte) {
            let champ = |k: &str| ligne.iter().find(|(n, _)| *n == k).map(|(_, v)| v.trim().to_string()).filter(|v| !v.is_empty());
            let (Some(nir), Some(emb)) = (champ("nir"), champ("date_embauche")) else { continue };
            let e: Vec<char> = emb.chars().collect();
            if e.len() != 8 {
                continue;
            }
            let tr = |a: usize, b: usize| -> String { e[a..b].iter().collect() };
            let debut = format!("{}-{}-{}", tr(4, 8), tr(2, 4), tr(0, 2));
            let Some((_, cle)) = par_nir.iter().find(|(n, _)| *n == nir) else {
                // On ne devine pas qui c'est : un NIR qu'on ne reconnait pas est peut-etre une fiche archivee sous une autre cle, ou
                // une declaration faite pour quelqu'un d'autre.
                inconnus.push(format!("NIR {} ({}), embauche du {}", nir, nom, debut));
                continue;
            };
            if mois.is_none() {
                mois = Some(debut.chars().take(7).collect());
            }
            embauches.push(EmbaucheAttestee { salarie: cle.clone(), debut });
            let h: Vec<char> = champ("horodatage").into_iter().flat_map(|h| h.chars().collect::<Vec<_>>()).collect();
            if quand.is_none() && h.len() == 12 {
                let t = |a: usize, b: usize| -> String { h[a..b].iter().collect() };
                quand = Some(format!("{}-{}-{}T{}:{}:00", t(4, 8), t(2, 4), t(0, 2), t(8, 10), t(10, 12)));
            }
        }
        let Some(flux) = flux_du_nom(&nom) else { continue };
        if !embauches.is_empty() {
            sortie.push(DpaeAttestee { flux: flux.to_string(), quand, mois, piece: nom.clone(), embauches });
        }
    }
    sortie.sort_by(|a, b| a.quand.cmp(&b.quand));
    Ok((sortie, inconnus))
}

/// Une DPAE attestee telle que la reprise la montre : son flux entier quand un bilan le porte, reel ou test, acceptee
/// ou non, deja au journal ou non.
#[cfg(feature = "atelier")]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DpaeVue {
    #[serde(flatten)]
    pub attestee: DpaeAttestee,
    pub mode: Mode,
    pub accepte: bool,
    pub deja: bool,
}

/// Ce que la reprise du canal DPAE rend.
#[cfg(feature = "atelier")]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RepriseDpae {
    pub trouves: Vec<DpaeVue>,
    pub ajoutes: Vec<DpaeVue>,
    pub deja: Vec<DpaeVue>,
    pub inconnus: Vec<String>,
    pub ecrit: bool,
}

/// Remet au journal les DPAE que les certificats de l'Urssaf attestent. Memes formes
/// et memes regles que la reprise DSN : `ecrire` a faux par defaut (elle DIT), idempotente par le flux, chaque ligne porte `origine`.
/// Aucun reseau, aucun depot, pas de mot de passe.
/// Elle sert l'outil de l'atelier `moteur journal --dpae` (`moteur::atelier::outils::journal`) : pas un geste de l'ecran.
#[cfg(feature = "atelier")]
pub fn reprise_dpae(ctx: &Contexte, ecrire: bool, qui: &str, maintenant: &str) -> R<RepriseDpae> {
    let (attestees, inconnus) = embauches_attestees(ctx)?;
    let connus: Vec<String> = journal::lire(&ctx.racine, None, None, false)
        .map_err(socle::echec::Echec::from)?
        .iter()
        .filter(|l| !l.flux().is_empty())
        .map(|l| l.flux().to_string())
        .collect();

    // Le bilan XML du meme flux, quand il est la : il porte l'idflux ENTIER et dit si le depot etait reel ou de test.
    let dossier = ctx.racine.join(crate::retours::RETOURS_DPAE);
    let mut bilans: Vec<(String, Vec<Bilan>)> = Vec::new();
    for (_, p) in fichiers_du_dossier(&dossier, |n| n.ends_with(".xml"))? {
        let octets = std::fs::read(&p).map_err(|e| erreur("OSError", socle::echec::du_disque(&p, &e)))?;
        let Some(b) = crate::retours::bilan_si_xml(&octets)? else { continue };
        match bilans.iter_mut().find(|(f, _)| *f == b.flux) {
            Some((_, l)) => l.push(b),
            None => bilans.push((b.flux.clone(), vec![b])),
        }
    }

    let mut rapport = RepriseDpae { trouves: vec![], ajoutes: vec![], deja: vec![], inconnus, ecrit: ecrire };
    for e in attestees {
        // Le nom de fichier tronque le flux ; le bilan XML porte l'entier. On rattache par prefixe, et on garde l'entier quand on le trouve.
        let entier = match bilans.iter().find(|(f, _)| f.starts_with(&e.flux)) {
            Some((f, _)) => f.clone(),
            None => e.flux.clone(),
        };
        let b = bilans.iter().find(|(f, _)| *f == entier).and_then(|(_, l)| crate::retours::fusionner_bilans(l));
        // Sans bilan, le certificat seul atteste un depot reel et accepte.
        let (mode, accepte_) = match &b {
            Some(b) => (Mode::de(reel(b)), accepte(b)),
            None => (Mode::Reel, true),
        };
        let deja = connus.contains(&entier);
        let vue = DpaeVue { attestee: DpaeAttestee { flux: entier, ..e }, mode, accepte: accepte_, deja };
        rapport.trouves.push(vue.clone());
        if deja {
            rapport.deja.push(vue);
        } else {
            rapport.ajoutes.push(vue);
        }
    }

    if ecrire {
        for vue in &rapport.ajoutes {
            let a = &vue.attestee;
            let embauches = a
                .embauches
                .iter()
                .map(|x| Ok(crate::journal::EmbaucheDeclaree { salarie: droit::contexte::IdSalarie::lire(&x.salarie)?, numero: None, debut: x.debut.clone(), fin: None }))
                .collect::<R<Vec<_>>>()?;
            let detail = crate::journal::DetailDpae { reconstruit: true, embauches, certificat: true, ..Default::default() };
            let mut e = Envoi::nouveau(vue.mode, qui, vue.accepte, "urssaf-dpae", SANS_CERTIFICAT, detail);
            e.mois = a.mois.as_deref().map(Mois::lire).transpose()?;
            e.flux = a.flux.clone();
            // Un certificat qui ne dit pas l'heure du depot laisse la ligne a celle de sa reprise (`journal::noter`).
            if let Some(q) = &a.quand {
                e.quand = q.clone();
            }
            e.origine = format!("retours/dpae/{}", a.piece);
            journal::noter(&ctx.racine, Ligne::Dpae(e), None, maintenant).map_err(socle::echec::Echec::from)?;
        }
    }
    Ok(rapport)
}
