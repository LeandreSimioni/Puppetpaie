//! CE QUE DIT LE JOURNAL DES ENVOIS : les etats du mois, les resumes, la repetition d'un depot, le rattachement des
//! retours, les contrats partis, les signatures attendues, les embauches declarees. Rien n'y calcule de paie. Les lignes
//! se lisent dans leur forme (`journal::Ligne`) ; un contrat se designe par sa `Paire` (la cle du salarie, le numero du
//! contrat s'il en a un).

use documents::fiche_questionnaire::CodeDeDemande;
use droit::contexte::IdSalarie;
use droit::fiche::{Lue as _, Nic};
use engagement::numeros::NumeroContrat;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::Serialize;

use socle::fichiers::{EmpreinteCourte, NomDeFichier};
use socle::mois::Mois;
use socle::date::Jour;
use socle::paris::{InstantOuJour, Moment};
use crate::journal::{chemin, Rangement};
use crate::ligne::{Acte, AvenantParti, ContratAttendu, Ligne, Mode, PersonneCertifiee, RetourContrat, RetourOrganisme, VerdictDeLaPiece};
use crate::retours::{self, Annonce, CompteRenduDuTest, IdDeFlux, NatureDuRetour, Verdict};
use socle::echec::{Echec, R};
use crate::tables_journal::ALNUM;

pub const DSN: &str = "dsn";
pub const DPAE: &str = "dpae";
pub const DOCUMENTS: &str = "documents";
pub const SIGNATURE: &str = "contrat-signature";

/// UN CONTRAT, tel que le journal le designe : la cle du salarie et le numero du contrat (`None` sans numero).
pub type Paire = (IdSalarie, Option<NumeroContrat>);

fn paire(salarie: &IdSalarie, numero: Option<&NumeroContrat>) -> Paire {
    (salarie.clone(), numero.cloned())
}

/// L'indice, dans `paies`, de la paie du contrat n° `numero` de ce salarie.
pub fn paie_du_contrat(paies: &[engagement::calcul::Paie], salarie: &IdSalarie, numero: &NumeroContrat) -> R<Option<usize>> {
    for (i, x) in paies.iter().enumerate() {
        if x.salarie.id == *salarie && x.engagement.numero.as_ref() == Some(numero) && x.engagement.nat()?.modele_contrat().is_some() {
            return Ok(Some(i));
        }
    }
    Ok(None)
}

// ------------------------------------------------------------------ les etats

pub(crate) fn erreur_io(e: &std::io::Error, p: &Path) -> Echec {
    socle::echec::du_disque(p, e).reclasser(socle::echec::Classe::du_disque(e))
}

// ------------------------------------------------------------ les embauches

/// Les embauches qu'une DPAE reelle declare, par (salarie, numero du contrat) : partie, et pas refusee (`refuse_par_un_retour`),
/// et quand ; une DPAE refusee se redeclare. Une embauche sans numero ne se relie a rien.
pub fn embauches_par_depot(lignes: &[Ligne]) -> HashMap<Paire, InstantOuJour> {
    let mut d = HashMap::new();
    for l in lignes {
        let Some(e) = l.dpae().filter(|e| e.mode == Mode::Reel && e.accepte && !refuse_par_un_retour(l)) else { continue };
        for x in &e.detail.embauches {
            if let Some(n) = &x.numero {
                d.insert(paire(&x.salarie, Some(n)), e.quand.clone());
            }
        }
    }
    d
}

/// Les lignes d'une DSN de l'etablissement `nic` (son NIC ; `None` : `principal`, le NIC de la fiche entreprise).
pub fn de_l_etablissement<'a>(lignes: impl IntoIterator<Item = &'a Ligne>, nic: Option<&Nic>, principal: Option<&Nic>) -> Vec<&'a Ligne> {
    // Une ligne qui ne dit pas son etablissement est celle de l'etablissement principal. Une fiche qui ne dit pas encore
    // son NIC n'a que lui : aucune DSN n'est partie d'un autre.
    let voulu = nic.or(principal);
    lignes
        .into_iter()
        .filter(|l| match l.etablissement() {
            Some(e) => Some(e) == voulu,
            None => principal == voulu,
        })
        .collect()
}

/// CE QU'EST DEVENUE LA DSN REELLE D'UN MOIS (la derniere acceptee de son etablissement) : passee (le bilan OK, ou le
/// verdict qu'une ligne reprise garde), refusee, ses anomalies, l'attestation employeur recue.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EtatDsnDuMois {
    pub quand: InstantOuJour,
    pub fichier: Option<NomDeFichier>,
    #[serde(serialize_with = "socle::forme::vide_si_aucun::serialize")]
    pub flux: Option<IdDeFlux>,
    pub passee: bool,
    pub refusee: bool,
    pub anomalies: Vec<String>,
    pub aer_recue: bool,
}

pub fn etat_dsn_du_mois(lignes: &[Ligne], mois: Mois, nic: Option<&Nic>, principal: Option<&Nic>) -> Option<EtatDsnDuMois> {
    // Un signalement porte le mois de son evenement : ce n'est pas la DSN du mois.
    let du_mois = crate::journal::filtrer(lignes, Some(Acte::Dsn), Some(mois), true).filter(|l| !l.signalement());
    let l = de_l_etablissement(du_mois, nic, principal).into_iter().filter(|l| l.accepte()).last()?;
    let e = l.dsn()?;
    let (mut bilan_ok, mut aer, mut anomalies) = (false, false, Vec::new());
    for r in &e.retours {
        bilan_ok |= r.nature_retour == NatureDuRetour::CertificatDeConformite && r.verdict() == Some(Verdict::Ok);
        aer |= r.nature_retour == NatureDuRetour::AerSpectacle;
        if r.ko() {
            anomalies.extend(r.anomalies().into_iter().flatten().cloned());
        }
    }
    // UN DEPOT REPRIS DES RETOURS (`reprise_journal`) : sa piece EST le bilan de la plate-forme, et la ligne
    // en garde le verdict (`detail.etat_declaration`) -- tant qu'aucun bilan n'est range en face, c'est lui qui dit.
    let sans_bilan = !e.retours.iter().any(|r| r.nature_retour == NatureDuRetour::CertificatDeConformite);
    let repris_ok = sans_bilan && e.detail.reconstruit && e.detail.etat_declaration == Some(Verdict::Ok);
    let refusee = refuse_par_un_retour(l);
    Some(EtatDsnDuMois { quand: e.quand.clone(), fichier: e.fichier.clone(), flux: e.flux.clone(), passee: (bilan_ok || repris_ok) && !refusee, refusee, anomalies, aer_recue: aer })
}

/// Deux graphies du meme nom (la cle d'une fiche, le nom d'un certificat) : leurs lettres et chiffres, en capitales.
fn meme_nom(cle: &str, nom: &str) -> bool {
    let plat = |t: &str| -> String { t.to_uppercase().chars().filter(|&c| lettre_ou_chiffre(c)).collect() };
    let a = plat(cle);
    !a.is_empty() && a == plat(nom)
}

/// Vrai pour une lettre ou un chiffre Unicode (lettres Lu, Ll, Lt, Lm, Lo et Numeric_Type, d'apres la table deposee : `tables_journal`).
pub fn lettre_ou_chiffre(c: char) -> bool {
    socle::texte::dans_les_plages(ALNUM, c)
}

/// Ce que les retours d'une DPAE partie en disent : `(passee, refusee)`. Passee : le certificat (nature 41) ou un bilan OK ;
/// refusee : un retour KO, sans certificat ni bilan OK.
fn verdict_dpae(retours: &[RetourOrganisme]) -> (bool, bool) {
    let (mut passee, mut ko) = (false, false);
    for r in retours {
        passee |= r.nature_retour == NatureDuRetour::CertificatDpae || (r.nature_retour == NatureDuRetour::CertificatDeConformite && r.verdict() == Some(Verdict::Ok));
        ko |= r.ko();
    }
    (passee, ko && !passee)
}

/// CE QU'EST DEVENUE UNE EMBAUCHE DECLAREE (la derniere DPAE reelle partie qui la porte) : quand, son fichier, sa fin,
/// passee, refusee, les anomalies, les retours, et ce que le certificat de l'Urssaf en dit (le dossier, deja existante).
#[derive(Debug, Clone, serde::Serialize)]
pub struct EtatEmbauche {
    pub quand: InstantOuJour,
    pub fichier: Option<NomDeFichier>,
    pub fin: Option<socle::date::Jour>,
    pub passee: bool,
    pub refusee: bool,
    pub anomalies: Vec<String>,
    pub retours: Vec<RetourOrganisme>,
    pub dossier: Option<String>,
    pub deja_existante: bool,
}

/// Ce que chaque DPAE reelle partie est devenue, par (salarie, numero du contrat).
pub fn etat_des_embauches(lignes: &[Ligne]) -> HashMap<Paire, EtatEmbauche> {
    let mut etat = HashMap::new();
    for l in lignes {
        let Some(e) = l.dpae().filter(|e| e.mode == Mode::Reel && e.accepte) else { continue };
        let (passee, refusee) = verdict_dpae(&e.retours);
        let anomalies: Vec<String> = e.retours.iter().filter(|r| r.ko()).flat_map(|r| r.anomalies().into_iter().flatten().cloned()).collect();
        let certifiees: Vec<&PersonneCertifiee> = e.retours.iter().filter(|r| r.nature_retour == NatureDuRetour::CertificatDpae).flat_map(|r| r.personnes().into_iter().flatten()).collect();
        for x in &e.detail.embauches {
            let Some(n) = &x.numero else { continue };
            // La derniere ligne du certificat pour ce nom et ce premier jour.
            let cert = certifiees.iter().rev().find(|c| c.embauche == Some(x.debut) && meme_nom(x.salarie.as_str(), &c.nom));
            etat.insert(
                paire(&x.salarie, Some(n)),
                EtatEmbauche {
                    quand: e.quand.clone(),
                    fichier: e.fichier.clone(),
                    fin: x.fin.clone(),
                    passee,
                    refusee,
                    anomalies: anomalies.clone(),
                    retours: e.retours.clone(),
                    dossier: cert.map(|c| c.dossier.clone()),
                    deja_existante: cert.is_some_and(|c| c.deja_existante),
                },
            );
        }
    }
    etat
}

// -------------------------------------------------------------- les contrats

/// COMMENT UN CONTRAT EST PARTI : imprime (remis en main propre, `detail.main_propre`), ou par courriel. Ecrite
/// `imprime`, `courriel`. La phrase d'un envoi du journal (`resume`) dit de meme tout envoi par son `main_propre`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ModeDeRemise {
    Imprime,
    Courriel,
}

impl ModeDeRemise {
    /// Celui d'un envoi du journal, par `main_propre` : imprime s'il est remis en main propre, sinon par courriel.
    pub fn de(main_propre: bool) -> ModeDeRemise {
        if main_propre {
            ModeDeRemise::Imprime
        } else {
            ModeDeRemise::Courriel
        }
    }
}

/// LA REMISE D'UN CONTRAT PARTI, dite dans le recit de ce qui est parti : « imprimé », « envoyé à la signature ».
pub fn remise_dite(par: ModeDeRemise) -> &'static str {
    match par {
        ModeDeRemise::Imprime => "imprimé",
        ModeDeRemise::Courriel => "envoyé à la signature",
    }
}

/// LE MOT DE LA REMISE d'un contrat parti, dans les phrases : « imprimé », « envoyé ». SANS REMISE NOTEE (`None`) : le
/// contrat est parti (`engagement::departs`, `journal::noter_ligne`) par un courriel coupe en route
/// (`ligne_d_envoi::noter_echec_d_envoi`), dont la ligne n'est pas acceptee mais a servi le salarie, et que
/// `contrats_partis_par`, qui ne lit que les lignes acceptees, ne retient pas : c'etait un courriel, « envoyé ».
pub fn mot_de_la_remise(par: Option<ModeDeRemise>) -> &'static str {
    match par {
        Some(ModeDeRemise::Imprime) => "imprimé",
        Some(ModeDeRemise::Courriel) | None => "envoyé",
    }
}

/// Comment chaque contrat est parti la derniere fois.
pub fn contrats_partis_par(lignes: &[Ligne]) -> HashMap<Paire, ModeDeRemise> {
    let mut d = HashMap::new();
    for e in lignes.iter().filter_map(Ligne::signature).filter(|e| e.accepte) {
        let par = ModeDeRemise::de(e.detail.main_propre);
        for c in &e.detail.contrats {
            d.insert(paire(&c.salarie, c.numero.as_ref()), par);
        }
    }
    d
}

/// LES CONTRATS QU'UN COURRIEL A DEJA DEMANDE A SIGNER (un avenant n'en est pas un), par (salarie, numero) : le
/// redemander par courriel est un rappel ; un contrat parti imprime seulement part par courriel pour la premiere fois.
/// C'est le journal qui le dit, jamais celui qui demande.
pub fn contrats_demandes_par_courriel(lignes: &[Ligne]) -> HashSet<Paire> {
    lignes
        .iter()
        .filter_map(Ligne::signature)
        .filter(|e| e.accepte && !e.detail.main_propre)
        .flat_map(|e| e.detail.contrats.iter().filter(|c| !c.est_avenant()).map(|c| paire(&c.salarie, c.numero.as_ref())))
        .collect()
}

/// CE QUI EST REVENU D'UN CONTRAT PARTI (signe, ou la mention retiree), sur la ligne de son envoi (`journal::rattacher`),
/// dans l'ordre ou c'est revenu.
pub fn revenus(l: &Ligne) -> &[RetourContrat] {
    match l.signature().filter(|e| e.accepte) {
        Some(e) => e.retours.as_slice(),
        None => &[],
    }
}

/// Les contrats repartis apres leur signature sur une autre version de l'engagement que celle signee : ils attendent une
/// nouvelle signature. Un avenant n'en fait pas un.
pub fn renvoyes_depuis_la_signature(lignes: &[Ligne]) -> HashSet<Paire> {
    let mut envoye: HashMap<Paire, Option<u64>> = HashMap::new();
    let mut signe: HashMap<Paire, Option<u64>> = HashMap::new();
    let mut renvoye: HashSet<Paire> = HashSet::new();
    for l in lignes.iter().filter(|l| l.accepte()) {
        if let Some(e) = l.signature() {
            for c in &e.detail.contrats {
                let k = paire(&c.salarie, c.numero.as_ref());
                // La version de l'engagement sur laquelle ce contrat part (`journal::versions_des_contrats`).
                let v = e.versions.iter().flatten().find(|x| x.salarie == c.salarie && c.numero.as_ref() == Some(&x.numero)).map(|x| x.version);
                if c.est_avenant() {
                    envoye.insert(k, v);
                    continue;
                }
                if let Some(Some(deja)) = signe.get(&k) {
                    if v.is_some_and(|v| v != *deja) {
                        renvoye.insert(k.clone());
                    }
                }
                envoye.insert(k, v);
            }
        }
        for d in revenus(l).iter().filter(|d| d.recu) {
            let k = paire(&d.salarie, d.numero.as_ref());
            signe.insert(k.clone(), envoye.get(&k).copied().flatten());
            renvoye.remove(&k);
        }
    }
    renvoye
}

/// Les contrats dont le dernier mot est « recu » : (salarie, numero) -> quand.
pub fn contrats_recus(lignes: &[Ligne]) -> HashMap<Paire, Moment> {
    let mut etat: HashMap<Paire, Option<Moment>> = HashMap::new();
    for d in lignes.iter().flat_map(revenus) {
        // La note d'un avenant (signe, ou defait) ne dit rien du contrat lui-meme.
        if d.avenant.is_some() {
            continue;
        }
        etat.insert(paire(&d.salarie, d.numero.as_ref()), d.recu.then_some(d.quand));
    }
    etat.into_iter().filter_map(|(k, v)| v.map(|v| (k, v))).collect()
}

/// UNE SIGNATURE ATTENDUE : la demande partie sous `code` (le mois, quand, a qui, sa piece, ce qu'elle est : un contrat ou
/// un avenant), dont un contrat au moins n'est pas revenu signe ; `rang` est sa ligne au journal, `envoyes` les
/// identifiants des messages partis (jamais pris pour une reponse).
#[derive(Debug, Clone, serde::Serialize)]
pub struct Attendue {
    pub code: CodeDeDemande,
    pub mois: Option<Mois>,
    pub quand: InstantOuJour,
    pub salarie: droit::contexte::IdSalarie,
    /// L'adresse ou le contrat est parti (aucune : remis en main propre).
    pub adresse: Option<droit::fiche::Courriel>,
    pub fichier: NomDeFichier,
    /// L'acte du contrat parti (`avenant`, `remplace`) ; aucun pour le contrat lui-meme.
    pub acte: Option<crate::ligne::ActeDuContrat>,
    pub avenant: Option<i64>,
    pub rang: usize,
    pub apres_signature: bool,
    pub envoyes: Vec<crate::envoi::IdDeCourriel>,
    /// Le prenom que la fiche du salarie dit (`prenom_affichage`, sinon `prenom`).
    pub prenom: Option<String>,
    /// Le nom que la fiche du salarie dit (`ChampsSalarie::nomme`).
    pub nom: Option<String>,
    pub contrats: Vec<ContratAttendu>,
}

impl Attendue {
    /// Le salarie tel qu'une phrase le nomme : le nom de sa fiche, sinon sa cle (un affichage, jamais une piece).
    pub fn qui(&self) -> String {
        match &self.nom {
            Some(n) => n.clone(),
            None => self.salarie.to_string(),
        }
    }
}

pub fn signatures_attendues<'f>(fiche_de: impl Fn(&droit::contexte::IdSalarie) -> Option<&'f droit::fiche::ChampsSalarie>, lignes: &[Ligne], mois: Option<Mois>) -> Vec<Attendue> {
    // Par CONTRAT (salarie, numero) : une reponse ne releve que le contrat qu'elle signe (Retours Windows, B, suite).
    let recus = contrats_recus(lignes);
    let renvoyes = renvoyes_depuis_la_signature(lignes);
    let mut derniere_recue: HashMap<Paire, usize> = HashMap::new();
    for (i, l) in lignes.iter().enumerate() {
        for d in revenus(l).iter().filter(|d| d.recu) {
            derniere_recue.insert(paire(&d.salarie, d.numero.as_ref()), i);
        }
    }
    // Les reponses deja relevees, par envoi (son code) et par contrat ou avenant : une reponse ne se note qu'une fois.
    // La mention « signe » retiree a la main l'emporte sur la reponse deja lue ; seul un nouvel envoi (un autre code)
    // attend une nouvelle reponse.
    let releve = |code: &CodeDeDemande, salarie: &IdSalarie, numero: Option<&NumeroContrat>, avenant: Option<i64>| (code.clone(), salarie.clone(), numero.cloned(), avenant);
    let deja_releves: HashSet<(CodeDeDemande, IdSalarie, Option<NumeroContrat>, Option<i64>)> = lignes
        .iter()
        .flat_map(revenus)
        .filter(|d| d.recu && d.par_courriel())
        .filter_map(|d| d.code.as_ref().map(|c| releve(c, &d.salarie, d.numero.as_ref(), d.avenant)))
        .collect();
    let mut par_code: HashMap<CodeDeDemande, usize> = HashMap::new();
    let mut demandes: Vec<Attendue> = Vec::new();
    for (i, l) in lignes.iter().enumerate() {
        let Some(e) = l.signature().filter(|e| e.accepte) else { continue };
        if mois.is_some_and(|m| e.mois != Some(m)) {
            continue;
        }
        for c in &e.detail.contrats {
            let Some(code) = c.code.as_ref() else { continue };
            let k = paire(&c.salarie, c.numero.as_ref());
            let apres_signature = c.acte.is_some() || renvoyes.contains(&k);
            let n = *par_code.entry(code.clone()).or_insert_with(|| {
                let fiche = fiche_de(&c.salarie);
                let prenom = fiche.and_then(|f| f.prenom_affichage.en_texte().or_else(|| f.prenom.en_texte()));
                demandes.push(Attendue {
                    code: code.clone(),
                    mois: e.mois,
                    quand: e.quand.clone(),
                    salarie: c.salarie.clone(),
                    adresse: c.adresse.clone(),
                    fichier: c.fichier.clone(),
                    acte: c.acte,
                    avenant: c.avenant,
                    rang: i,
                    apres_signature: false,
                    envoyes: Vec::new(),
                    nom: fiche.and_then(|f| f.nomme()),
                    prenom,
                    contrats: Vec::new(),
                });
                demandes.len() - 1
            });
            let d = &mut demandes[n];
            if let Some(m) = &c.message {
                if !d.envoyes.contains(m) {
                    d.envoyes.push(m.clone());
                }
            }
            d.apres_signature |= apres_signature;
            d.contrats.push(ContratAttendu {
                salarie: c.salarie.clone(),
                debut: c.debut.clone(),
                fin: c.fin.clone(),
                numero: c.numero.clone(),
                spectacle: c.spectacle.clone(),
                fichier: c.fichier.clone(),
                avenant: c.avenant,
                mois: c.mois,
            });
        }
    }
    demandes
        .into_iter()
        .filter(|d| {
            d.contrats.iter().any(|c| {
                let k = paire(&c.salarie, c.numero.as_ref());
                let revenu = deja_releves.contains(&releve(&d.code, &c.salarie, c.numero.as_ref(), c.avenant))
                    || if d.apres_signature { derniere_recue.get(&k).is_some_and(|&rang| rang >= d.rang) } else { recus.contains_key(&k) };
                !revenu
            })
        })
        .collect()
}

/// Les avenants de chaque contrat (sa `Paire`), dans l'ordre du journal : chacun son numero, et `signe` (le jour ou il
/// est note revenu signe, ou rien).
pub fn avenants_par_contrat(lignes: &[Ligne]) -> HashMap<Paire, Vec<AvenantParti>> {
    let mut out: HashMap<Paire, Vec<AvenantParti>> = HashMap::new();
    for l in lignes.iter().filter(|l| l.accepte()) {
        if let Some(e) = l.signature() {
            for c in e.detail.contrats.iter().filter(|c| c.est_avenant()) {
                let liste = out.entry(paire(&c.salarie, c.numero.as_ref())).or_default();
                // Un courriel a demande de signer CETTE piece (le meme fichier) : un avenant refait sous le meme numero
                // est une autre piece, qu'aucun courriel n'a encore demandee.
                let par_courriel = !e.detail.main_propre || liste.iter().any(|a| a.numero == c.avenant && a.fichier == c.fichier && a.par_courriel);
                liste.retain(|a| a.numero != c.avenant);
                liste.push(AvenantParti {
                    numero: c.avenant,
                    quand: e.quand.clone(),
                    mois: c.mois,
                    fichier: c.fichier.clone(),
                    code: c.code.clone(),
                    contrat: c.numero.clone(),
                    effet: c.effet.clone(),
                    modifications: c.modifications.iter().flatten().cloned().collect(),
                    signe: None,
                    par_courriel,
                });
            }
        }
        for d in revenus(l) {
            // Le retour d'UN avenant ne vaut que pour lui, et se defait (`recu: false`).
            let Some(un) = d.avenant else { continue };
            if let Some(liste) = out.get_mut(&paire(&d.salarie, d.numero.as_ref())) {
                for a in liste.iter_mut().filter(|a| a.numero == Some(un)) {
                    if !d.recu {
                        a.signe = None;
                    } else if a.signe.is_none() {
                        a.signe = Some(d.quand);
                    }
                }
            }
        }
    }
    out
}

/// CE QUE LES DOCUMENTS DU MOIS DISAIENT, par salarie : le resume de sa paie (`envoi::resume_de`) dans la derniere ligne
/// d'envoi de documents qui l'a servi (un envoi coupe en route ne porte que ceux qu'il a servis). Qu'ils soient partis,
/// c'est l'engagement qui le sait (`journal::departs`) ; ceci n'en dit que le contenu.
pub fn resumes_des_documents(lignes: &[Ligne], mois: Mois) -> HashMap<IdSalarie, Option<crate::envoi::ResumeDePaie>> {
    let mut out = HashMap::new();
    for e in crate::journal::filtrer(lignes, Some(Acte::Documents), Some(mois), false).filter_map(Ligne::documents) {
        for s in &e.detail.salaries {
            out.insert(s.cle.clone(), s.resume.clone());
        }
    }
    out
}

/// Les flux des depots de test, chacun une fois.
pub fn flux_de_test(lignes: &[Ligne]) -> Vec<IdDeFlux> {
    let mut d: Vec<IdDeFlux> = Vec::new();
    for f in lignes.iter().filter(|l| l.mode() == Mode::Test).filter_map(Ligne::flux) {
        if !d.contains(f) {
            d.push(f.clone());
        }
    }
    d
}

// ------------------------------------------------------------------- resume

/// Un horodatage ISO a la francaise, ou vide.
pub fn quand_lisible(iso: &str) -> String {
    let c: Vec<char> = iso.chars().collect();
    if c.len() < 10 || c[4] != '-' || c[7] != '-' {
        return iso.to_string();
    }
    let tranche = |a: usize, b: usize| c[a..b].iter().collect::<String>();
    let jour = format!("{}/{}/{}", tranche(8, 10), tranche(5, 7), tranche(0, 4));
    if c.len() >= 16 {
        format!("{} à {}", jour, tranche(11, 16))
    } else {
        jour
    }
}

fn pluriel(n: usize) -> &'static str {
    if n > 1 {
        "s"
    } else {
        ""
    }
}

/// LA PHRASE D'UN ENVOI DU JOURNAL : quand il est parti, et ce qui en est revenu. Un contrat est revenu signe quand le
/// journal le note recu (`contrats_recus`) ; un avenant, quand sa propre note le dit (`avenants_par_contrat`) -- les
/// memes lectures que l'etat de l'engagement.
pub fn resume(ligne: Option<&Ligne>, recus: &HashMap<Paire, Moment>, avenants: &HashMap<Paire, Vec<AvenantParti>>) -> String {
    let Some(ligne) = ligne else { return "pas encore envoyé".into() };
    let en_test = ligne.mode() == Mode::Test;
    let mut quand = ligne.quand().dit();
    if en_test {
        quand = format!("en TEST le {}", quand);
    }
    if !ligne.accepte() {
        return format!("envoi du {} refusé au dépôt — {}", quand, ligne.reponse());
    }
    let parti = mot_de_la_remise(Some(ModeDeRemise::de(ligne.main_propre())));
    let le = if en_test { quand.clone() } else { format!("le {}", quand) };
    let prefixe = format!("{} {}", parti, le);
    match ligne {
        Ligne::ContratSignature(e) => {
            let n = e.detail.contrats.len();
            let revenus = e
                .detail
                .contrats
                .iter()
                .filter(|c| {
                    let k = paire(&c.salarie, c.numero.as_ref());
                    if c.est_avenant() {
                        avenants.get(&k).is_some_and(|l| l.iter().any(|a| a.numero == c.avenant && a.signe.is_some()))
                    } else {
                        recus.contains_key(&k)
                    }
                })
                .count();
            if n == 0 {
                prefixe
            } else if revenus == n {
                format!("{} — {}", prefixe, if n == 1 { "revenu signé".to_string() } else { format!("tous revenus signés ({})", n) })
            } else if revenus > 0 {
                format!("{} — {} contrat{} sur {} revenu{} signé{}", prefixe, revenus, pluriel(revenus), n, pluriel(revenus), pluriel(revenus))
            } else {
                format!("{} — pas encore revenu signé", prefixe)
            }
        }
        Ligne::Documents(_) | Ligne::ContratContresigne(_) | Ligne::RegularisationCasc(_) => prefixe,
        Ligne::DossierSalarie(_) => format!("{} — pas encore de retour", prefixe),
        Ligne::FicheDemande(e) if e.retours.is_empty() => format!("{} — pas encore de retour", prefixe),
        Ligne::FicheDemande(_) => format!("{} — retour reçu", prefixe),
        Ligne::Dsn(e) => resume_des_retours(&prefixe, &le, &e.retours),
        Ligne::Dpae(e) => resume_des_retours(&prefixe, &le, &e.retours),
    }
}

/// Ce que les comptes rendus d'une DSN ou d'une DPAE disent, a la suite de `prefixe` (`le_dit` : quand elle est partie,
/// en mots).
fn resume_des_retours(prefixe: &str, le_dit: &str, retours: &[RetourOrganisme]) -> String {
    let Some(dernier) = retours.last() else { return format!("{} — pas encore de retour", prefixe) };
    let refus: Vec<&RetourOrganisme> = retours.iter().filter(|r| r.ko()).collect();
    let certificat = retours.iter().rev().find(|r| r.nature_retour == NatureDuRetour::CertificatDpae);
    if !refus.is_empty() && certificat.is_none() {
        let mut motifs: Vec<&str> = Vec::new();
        for a in refus.iter().flat_map(|r| r.anomalies().into_iter().flatten()) {
            if !motifs.contains(&a.as_str()) {
                motifs.push(a);
            }
        }
        let suite = if motifs.is_empty() { String::new() } else { format!(" : {}", motifs.join(" ; ")) };
        return format!("envoyé {} — REFUSÉ{}", le_dit, suite);
    }
    if let Some(c) = certificat {
        let dossiers: Vec<&str> = c.personnes().into_iter().flatten().filter(|l| l.enregistree && !l.dossier.is_empty()).map(|l| l.dossier.as_str()).collect();
        let existantes: Vec<&str> = c.personnes().into_iter().flatten().filter(|l| l.deja_existante).map(|l| l.nom.as_str()).collect();
        let mut fin = " — certificat de conformité reçu : passée".to_string();
        if !dossiers.is_empty() {
            fin.push_str(&format!(" (dossier{} {})", pluriel(dossiers.len()), dossiers.join(", ")));
        }
        if !existantes.is_empty() {
            fin.push_str(&format!(" ; déjà existante à l'Urssaf, rien de créé : {}", existantes.join(", ")));
        }
        return format!("{}{}", prefixe, fin);
    }
    if retours.iter().any(|r| r.nature_retour == NatureDuRetour::CertificatDeConformite && r.verdict() == Some(Verdict::Ok)) {
        let apres = retours.iter().filter(|r| !matches!(r.nature_retour, NatureDuRetour::AccuseDEnregistrement | NatureDuRetour::CertificatDeConformite)).count();
        let suite = if apres > 0 { format!(" ; {} compte{}-rendu{} d'organismes reçu{}", apres, pluriel(apres), pluriel(apres), pluriel(apres)) } else { String::new() };
        return format!("{} — bilan OK, aucune anomalie{}", prefixe, suite);
    }
    // Le retour se dit par son libelle, sinon sa nature.
    let choix = match dernier.libelle.as_str() {
        "" => dernier.nature_retour.code(),
        l => l,
    };
    format!("{} — {}", prefixe, choix)
}

// ------------------------------------------------------ la repetition avant l'acte

/// UNE DECLARATION REFUSEE AU BILAN n'a rien depose : « c'est seulement après avoir satisfait à l'ensemble des
/// contrôles […] qu'une déclaration sera acceptée », et une annule-et-remplace « doit annuler une déclaration
/// précédemment reçue » (cahier technique DSN, § 4.1 et 4.2, `atelier/reference/norme-dsn/texte/chapitre-4-controles.txt`) :
/// la suivante est une normale. Une DPAE : un retour KO sans certificat ni bilan OK (`verdict_dpae`). Une DSN : un retour
/// KO ; pour un depot repris des retours (`reprise_journal`) sans bilan range en face, le verdict que la ligne garde.
pub fn refuse_par_un_retour(ligne: &Ligne) -> bool {
    match ligne {
        Ligne::Dpae(e) => verdict_dpae(&e.retours).1,
        Ligne::Dsn(e) => {
            if e.retours.iter().any(RetourOrganisme::ko) {
                return true;
            }
            let bilan = e.retours.iter().any(|r| r.nature_retour == NatureDuRetour::CertificatDeConformite);
            !bilan && e.detail.reconstruit && e.detail.etat_declaration == Some(Verdict::Ko)
        }
        Ligne::Documents(_) | Ligne::ContratSignature(_) | Ligne::ContratContresigne(_) | Ligne::RegularisationCasc(_) | Ligne::DossierSalarie(_) | Ligne::FicheDemande(_) => false,
    }
}

/// Le compte rendu d'un depot : KO des qu'un retour refuse ; OK seulement quand LE BILAN le dit -- la nature 11
/// (certificat de conformite ou bilan d'anomalies) pour une DSN, le certificat 41 pour une DPAE ; attendu sinon.
/// L'ACCUSE D'ENREGISTREMENT (nature 10) NE COMPTE PAS : il dit « envoi recu », `etat_envoi` OK, pas « declaration
/// conforme » (regle 5 ; `retours::NatureDuRetour` : « 10 - Accuse d'enregistrement electronique »). Un test dont seul
/// l'accuse est la ne doit pas s'afficher « testee — conforme » quand l'essai precedent du meme contenu a pris
/// B1-208-11.
fn compte_rendu_du_depot(retours: &[RetourOrganisme]) -> CompteRenduDuTest {
    let mut ok = false;
    for r in retours {
        if r.ko() {
            return CompteRenduDuTest::Ko;
        }
        ok = ok || (matches!(r.nature_retour, NatureDuRetour::CertificatDeConformite | NatureDuRetour::CertificatDpae) && r.verdict() == Some(Verdict::Ok));
    }
    if ok {
        CompteRenduDuTest::Ok
    } else {
        CompteRenduDuTest::Attendu
    }
}

/// Le dernier depot reel accepte de ce fichier (`empreinte`), s'il y en a un.
pub fn deja_depose(lignes: &[Ligne], quoi: Acte, empreinte: &EmpreinteCourte) -> Option<Ligne> {
    crate::journal::filtrer(lignes, Some(quoi), None, true).filter(|l| l.accepte() && l.empreinte() == Some(empreinte)).last().cloned()
}

/// Ce qui a fait echouer un depot de test, en une phrase : les anomalies des retours refuses, sinon la reponse du depot.
pub fn pourquoi_rate(ligne: &Ligne) -> String {
    let mut anomalies: Vec<&str> = Vec::new();
    for a in ligne.retours_organisme().iter().filter(|r| r.ko()).flat_map(|r| r.anomalies().into_iter().flatten()) {
        if !anomalies.contains(&a.as_str()) {
            anomalies.push(a);
        }
    }
    if !anomalies.is_empty() {
        return anomalies.join(" ; ");
    }
    let rep = ligne.reponse().trim();
    if rep.is_empty() {
        "motif non lu".into()
    } else {
        rep.to_string()
    }
}

/// OU EN EST LE TEST D'UN FICHIER (`repetition`) : sa derniere ligne de test conforme (`conforme`), son compte rendu
/// (`ok`, `ko`, `attendu`, ou rien sans test conforme), les essais rates du meme contenu et ceux d'un contenu d'avant sous
/// le meme nom, la couleur et les mots de l'encart qui le dit.
///
/// Elle se relit avec la reponse qui la porte (le refus d'un depot reel sans test, `routes::enveloppe::APart`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Repetition {
    pub empreinte: EmpreinteCourte,
    pub fichier: NomDeFichier,
    pub conforme: Option<Ligne>,
    /// Aucun sans test conforme.
    pub compte_rendu: Option<CompteRenduDuTest>,
    pub ratees: Vec<Ligne>,
    pub perimees: Vec<Ligne>,
    pub couleur: String,
    pub motif: Option<String>,
    pub etat: EtatDuTest,
    pub titre: String,
    pub phrase: String,
}

/// OU EN EST LE TEST, dit une fois : teste et conforme (le reel peut partir), en test (son compte rendu est attendu),
/// refuse (ses anomalies : `motif`), ou a faire (jamais essaye, ou un test d'un contenu d'avant).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EtatDuTest {
    Testee,
    EnTest,
    TestRefuse,
    Ecrite,
}

impl EtatDuTest {
    /// Son nom, tel que la reponse l'ecrit.
    pub fn nom(self) -> &'static str {
        match self {
            EtatDuTest::Testee => "testee",
            EtatDuTest::EnTest => "en_test",
            EtatDuTest::TestRefuse => "test_refuse",
            EtatDuTest::Ecrite => "ecrite",
        }
    }
}

impl Repetition {
    /// Le flux du test conforme, s'il y en a un.
    pub fn flux_conforme(&self) -> Option<IdDeFlux> {
        self.conforme.as_ref().and_then(|c| c.flux().cloned())
    }

    /// Les lignes qu'il porte, pour y poser ce que la page en montre.
    pub fn lignes_mut(&mut self) -> impl Iterator<Item = &mut Ligne> {
        self.conforme.iter_mut().chain(self.ratees.iter_mut()).chain(self.perimees.iter_mut())
    }
}

pub fn repetition(lignes: &[Ligne], quoi: Acte, empreinte: &EmpreinteCourte, nom: &NomDeFichier) -> Repetition {
    let mut conforme: Option<&Ligne> = None;
    let (mut ratees, mut perimees) = (Vec::new(), Vec::new());
    for l in crate::journal::filtrer(lignes, Some(quoi), None, false).filter(|l| l.mode() == Mode::Test) {
        if l.empreinte() == Some(empreinte) {
            if l.accepte() && !refuse_par_un_retour(l) {
                conforme = Some(l);
            } else {
                ratees.push(l.clone());
            }
        } else if l.fichier() == Some(nom) {
            perimees.push(l.clone());
        }
    }
    let compte_rendu = conforme.map(|c| compte_rendu_du_depot(c.retours_organisme()));
    let motif = ratees.last().map(pourquoi_rate);
    // LA COULEUR DE L'ENCART qui le dit (la charte des etats, `socle::ecran::couleur_de_l_etat`) : teste conforme, vert ;
    // essaye et rate (ou un test d'un contenu d'avant), a corriger, orange ; en test, ou jamais essaye, une information.
    let couleur = match conforme {
        Some(_) if compte_rendu == Some(CompteRenduDuTest::Ok) => "vert",
        Some(_) => "gris",
        None if !ratees.is_empty() || !perimees.is_empty() => "orange",
        None => "gris",
    };
    // CE QUE L'ENCART DU TEST DIT, titre et phrase, d'apres ou il en est : la page et `moteur route` les lisent tels quels.
    let cette = if quoi == Acte::Dpae { "Cette DPAE" } else { "Cette DSN" };
    const ENSUITE: &str = "Le dépôt réel se fait une fois le test revenu conforme.";
    let (etat, titre, phrase) = match (conforme, ratees.last(), perimees.last()) {
        (Some(c), _, _) if compte_rendu == Some(CompteRenduDuTest::Ok) => (
            EtatDuTest::Testee,
            format!("{} est testée — conforme. Voulez-vous l'envoyer ?", cette),
            format!("Déposée en test le {}, avec ce même contenu.", c.quand().dit()),
        ),
        (Some(c), _, _) => (
            EtatDuTest::EnTest,
            format!("{} est en test — son compte rendu est attendu", cette),
            format!("Envoyée en test le {}. {}", c.quand().dit(), ENSUITE),
        ),
        (None, Some(r), _) => (
            EtatDuTest::TestRefuse,
            "Le test est refusé".into(),
            format!(
                "Essayé le {} : {}. Le même refus attendrait le dépôt réel : c'est cela qu'il faut corriger.",
                r.quand().dit(),
                pourquoi_rate(r).trim_end_matches('.')
            ),
        ),
        (None, None, Some(p)) => (
            EtatDuTest::Ecrite,
            "Pas de test valable pour ce contenu".into(),
            format!("Un fichier du même nom a été essayé le {}, mais son contenu a changé depuis : le test ne vaut plus. {}", p.quand().dit(), ENSUITE),
        ),
        (None, None, None) => (EtatDuTest::Ecrite, "D'abord, le dépôt de test".into(), format!("Ce contenu n'a jamais été déposé en test. {}", ENSUITE)),
    };
    Repetition {
        empreinte: empreinte.clone(),
        fichier: nom.clone(),
        conforme: conforme.cloned(),
        compte_rendu,
        ratees,
        perimees,
        couleur: couleur.to_string(),
        motif,
        etat,
        titre,
        phrase,
    }
}

/// La pastille de chaque personne des certificats de conformite que la ligne porte : ce que la page en montre.
pub fn poser_les_pastilles(l: &mut Ligne) {
    let retours = match l {
        Ligne::Dsn(e) => &mut e.retours,
        Ligne::Dpae(e) => &mut e.retours,
        _ => return,
    };
    for p in retours.iter_mut().flat_map(|r| r.dpae.iter_mut().flatten()) {
        p.pastille = Some(p.pastille_due());
    }
}

// -------------------------------------------------------------------- flux_de

fn egal_sans_casse(c: char, voulu: char) -> bool {
    c.to_ascii_lowercase() == voulu || (voulu == 'i' && (c == '\u{131}' || c == '\u{130}'))
}

fn mot(c: &[char], mut j: usize, mot: &str) -> Option<usize> {
    for v in mot.chars() {
        if j < c.len() && egal_sans_casse(c[j], v) {
            j += 1;
        } else {
            return None;
        }
    }
    Some(j)
}

fn blancs(c: &[char], mut j: usize) -> usize {
    while j < c.len() && socle::texte::blanc(c[j]) {
        j += 1;
    }
    j
}

/// `<\s*idflux\s*>\s*([^<\s]+)\s*<`, insensible a la casse.
fn flux_xml(c: &[char]) -> Option<String> {
    for i in 0..c.len() {
        if c[i] != '<' {
            continue;
        }
        let Some(j) = mot(c, blancs(c, i + 1), "idflux") else { continue };
        let j = blancs(c, j);
        if j >= c.len() || c[j] != '>' {
            continue;
        }
        let d = blancs(c, j + 1);
        let mut f = d;
        while f < c.len() && c[f] != '<' && !socle::texte::blanc(c[f]) {
            f += 1;
        }
        if f == d {
            continue;
        }
        let k = blancs(c, f);
        if k < c.len() && c[k] == '<' {
            return Some(c[d..f].iter().collect());
        }
    }
    None
}

/// `"idflux"\s*:\s*"([^"]+)"`, insensible a la casse.
fn flux_json(c: &[char]) -> Option<String> {
    for i in 0..c.len() {
        if c[i] != '"' {
            continue;
        }
        let Some(j) = mot(c, i + 1, "idflux") else { continue };
        if j >= c.len() || c[j] != '"' {
            continue;
        }
        let j = blancs(c, j + 1);
        if j >= c.len() || c[j] != ':' {
            continue;
        }
        let d = blancs(c, j + 1);
        if d >= c.len() || c[d] != '"' {
            continue;
        }
        let mut f = d + 1;
        while f < c.len() && c[f] != '"' {
            f += 1;
        }
        if f < c.len() && f > d + 1 {
            return Some(c[d + 1..f].iter().collect());
        }
    }
    None
}

/// Le flux que la reponse d'un depot donne (`<idflux>`, `"idflux"`, ou la reponse nue d'au moins huit caracteres), s'il
/// a la forme d'un flux (`IdDeFlux`) ; aucun sinon.
pub fn flux_de(reponse: &str) -> Option<IdDeFlux> {
    if reponse.is_empty() {
        return None;
    }
    let c: Vec<char> = reponse.chars().collect();
    if let Some(m) = flux_xml(&c) {
        return IdDeFlux::lu(&m);
    }
    if let Some(m) = flux_json(&c) {
        return IdDeFlux::lu(&m);
    }
    let nu = reponse.trim_matches(socle::texte::blanc);
    if nu.chars().count() >= 8 {
        IdDeFlux::lu(nu)
    } else {
        None
    }
}

// ------------------------------------------------------- les verdicts des retours

enum Jeton {
    Etoile,
    Un,
    Car(char),
    Classe(Vec<(char, char)>, bool),
}

/// Le motif d'un `glob` de `fnmatch` : `*`, `?`, `[...]`.
fn jetons(motif: &str) -> Vec<Jeton> {
    let p: Vec<char> = motif.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < p.len() {
        let c = p[i];
        i += 1;
        match c {
            '*' => {
                if !matches!(out.last(), Some(Jeton::Etoile)) {
                    out.push(Jeton::Etoile);
                }
            }
            '?' => out.push(Jeton::Un),
            '[' => {
                let mut j = i;
                if j < p.len() && p[j] == '!' {
                    j += 1;
                }
                if j < p.len() && p[j] == ']' {
                    j += 1;
                }
                while j < p.len() && p[j] != ']' {
                    j += 1;
                }
                if j >= p.len() {
                    out.push(Jeton::Car('['));
                } else {
                    let mut corps: &[char] = &p[i..j];
                    i = j + 1;
                    let nie = corps.first() == Some(&'!');
                    if nie {
                        corps = &corps[1..];
                    }
                    let mut plages = Vec::new();
                    let mut k = 0;
                    while k < corps.len() {
                        if k + 2 < corps.len() && corps[k + 1] == '-' {
                            plages.push((corps[k], corps[k + 2]));
                            k += 3;
                        } else {
                            plages.push((corps[k], corps[k]));
                            k += 1;
                        }
                    }
                    out.push(Jeton::Classe(plages, nie));
                }
            }
            c => out.push(Jeton::Car(c)),
        }
    }
    out
}

fn correspond(j: &[Jeton], nom: &[char]) -> bool {
    let sans_casse = cfg!(windows);
    let egal = |a: char, b: char| if sans_casse { a.to_lowercase().eq(b.to_lowercase()) } else { a == b };
    let (mut ji, mut ni) = (0, 0);
    let mut reprise: Option<(usize, usize)> = None;
    while ni < nom.len() {
        if matches!(j.get(ji), Some(Jeton::Etoile)) {
            reprise = Some((ji, ni));
            ji += 1;
            continue;
        }
        // Un motif epuise avant le nom ne correspond pas a ce qui reste.
        let ok = j.get(ji).is_some_and(|x| match x {
            Jeton::Etoile | Jeton::Un => true,
            Jeton::Car(c) => egal(*c, nom[ni]),
            Jeton::Classe(plages, nie) => plages.iter().any(|&(a, b)| a <= nom[ni] && nom[ni] <= b) != *nie,
        });
        if ok {
            ji += 1;
            ni += 1;
        } else if let Some((rj, rn)) = reprise {
            reprise = Some((rj, rn + 1));
            ji = rj + 1;
            ni = rn + 1;
        } else {
            return false;
        }
    }
    while matches!(j.get(ji), Some(Jeton::Etoile)) {
        ji += 1;
    }
    ji == j.len()
}

/// LA CLE DU CONTENU TELECHARGE D'UN RETOUR (`retours::Annonce`) : son flux, sa nature, sa publication.
pub type CleDuContenu = (Option<IdDeFlux>, NatureDuRetour, Option<socle::paris::Moment>);

/// La cle du contenu telecharge du retour `r`.
pub fn cle_du_contenu(r: &Annonce) -> CleDuContenu {
    (r.flux.clone(), r.nature_retour, r.publication)
}

/// La piece archivee du retour `r` d'un envoi `quoi`, dans `retours/` (`retours/dpae/` pour une DPAE) ; son nom porte
/// `sansflux` quand le retour ne dit pas son flux (`releve_retours::archiver_retour`).
fn archive_du_retour(racine: &Path, r: &Annonce, quoi: Acte) -> R<Option<PathBuf>> {
    let dossier = crate::releve_retours::CanalDesRetours::de_l_acte(quoi).chemin(racine);
    if !dossier.is_dir() {
        return Ok(None);
    }
    let flux = match &r.flux {
        Some(f) => f.texte(),
        None => "sansflux",
    };
    let motif = jetons(&format!("*_nature{}_{}*", r.nature_retour, flux));
    let mut noms = Vec::new();
    for e in std::fs::read_dir(&dossier).map_err(|e| erreur_io(&e, &dossier))? {
        let e = e.map_err(|e| erreur_io(&e, &dossier))?;
        let nom = e.file_name().to_string_lossy().into_owned();
        if correspond(&motif, &nom.chars().collect::<Vec<_>>()) {
            noms.push(nom);
        }
    }
    noms.sort();
    Ok(noms.into_iter().find(|n| n != "journal.json").map(|n| dossier.join(n)))
}

fn lire_archive(f: &Path) -> R<Vec<u8>> {
    std::fs::read(f).map_err(|e| erreur_io(&e, f))
}

/// Ce que le certificat de conformite d'une DPAE (nature 41) dit de chaque personne ; rien s'il ne se lit pas.
fn verdict_du_certificat(contenu: &[u8]) -> R<Option<VerdictDeLaPiece>> {
    let texte: String = contenu.iter().map(|&b| b as char).collect();
    let lignes = declarations::dpae::lire_certificat(&texte)?;
    if lignes.is_empty() {
        return Ok(None);
    }
    let mut personnes = Vec::new();
    let mut anomalies: Vec<String> = Vec::new();
    for l in lignes {
        if let Some(lot) = l.lot.filter(|lot| *lot != declarations::dpae::CodeRetourDuLot::Correct) {
            anomalies.push(format!("{} (lot)", lot.libelle()));
        }
        let p = PersonneCertifiee {
            libelle: l.code.libelle().to_string(),
            nom: l.nom,
            prenom: l.prenom,
            embauche: l.embauche,
            dossier: l.dossier,
            code: l.code,
            enregistree: l.enregistree,
            deja_existante: l.deja_existante,
            formalites: l.formalites,
            pastille: None,
        };
        // Refusee : la ligne le dit dans ses anomalies.
        if !p.enregistree && !p.deja_existante {
            anomalies.push(format!("{} -- {}", p.libelle, p.nom));
        }
        personnes.push(p);
    }
    let mut vus: Vec<String> = Vec::new();
    for a in anomalies {
        if !vus.contains(&a) {
            vus.push(a);
        }
    }
    Ok(Some(VerdictDeLaPiece { verdict: if vus.is_empty() { Verdict::Ok } else { Verdict::Ko }, anomalies: vus, dpae: Some(personnes) }))
}

/// Ce que la piece du retour `r` d'un envoi `quoi` en dit : le bilan (nature 10 ou 11) ou le certificat d'une DPAE
/// (nature 41) ; rien pour une autre nature, ou une piece absente ou illisible.
fn verdict_du_retour(racine: &Path, r: &Annonce, quoi: Acte, contenu: Option<&[u8]>) -> R<Option<VerdictDeLaPiece>> {
    let nature = r.nature_retour;
    if !matches!(nature, NatureDuRetour::AccuseDEnregistrement | NatureDuRetour::CertificatDeConformite) && !(quoi == Acte::Dpae && nature == NatureDuRetour::CertificatDpae) {
        return Ok(None);
    }
    let octets = match contenu {
        Some(c) => c.to_vec(),
        None => match archive_du_retour(racine, r, quoi)? {
            None => return Ok(None),
            Some(f) => lire_archive(&f)?,
        },
    };
    if nature == NatureDuRetour::CertificatDpae {
        return verdict_du_certificat(&octets);
    }
    // Une piece sans bilan (ce n'est pas un rapport de depot), ou un bilan qui ne dit ni l'etat de la declaration ni
    // celui de l'envoi, ne donne pas de verdict.
    let Some(verdict) = retours::bilan_si_xml(&octets)?.and_then(|b| b.verdict()) else { return Ok(None) };
    let anomalies = retours::anomalies_lisibles_d_octets(&octets)?;
    Ok(Some(VerdictDeLaPiece { verdict, anomalies, dpae: None }))
}

/// L'INCIDENT D'UN RETOUR QUI REJETTE, tel que le journal des incidents l'ecrit (`compagnie::incidents`) : son
/// canal, l'endroit, le premier code d'anomalie (ou `KO`), le resume (dix anomalies au plus), et le detail.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IncidentDeRetour {
    pub canal: crate::releve_retours::CanalDesRetours,
    pub ou: String,
    pub quoi: String,
    pub resume: String,
    pub detail: DetailIncidentDeRetour,
}

/// Le detail d'un incident de retour : reel ou test, la nature du retour, le type de la declaration (une DSN qui le
/// dit), et trente anomalies au plus.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DetailIncidentDeRetour {
    pub reel: bool,
    pub nature_retour: NatureDuRetour,
    pub type_declaration: Option<droit::type_declaration::TypeDeLaDeclaration>,
    pub anomalies: Vec<AnomalieDeRetour>,
}

/// Une anomalie d'un incident : celle d'un bilan (categorie, code, message, rubrique), ou d'une personne que le certificat
/// d'une DPAE n'a pas enregistree (son code et son libelle).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AnomalieDeRetour {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categorie: Option<retours::CategorieDAnomalie>,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rubrique: Option<String>,
}

/// L'INCIDENT D'UN RETOUR QUI REJETTE : rien quand le retour ne rejette pas.
fn incident(racine: &Path, ligne: &Ligne, r: &Annonce, verdict: &VerdictDeLaPiece, contenus: &HashMap<CleDuContenu, Vec<u8>>) -> R<Option<IncidentDeRetour>> {
    if verdict.verdict != Verdict::Ko {
        return Ok(None);
    }
    let quoi = ligne.acte();
    let anomalies: Vec<AnomalieDeRetour> = if quoi == Acte::Dpae && r.nature_retour == NatureDuRetour::CertificatDpae {
        verdict
            .dpae
            .iter()
            .flatten()
            .filter(|d| d.refusee())
            .map(|d| AnomalieDeRetour { categorie: None, code: d.code.to_string(), message: d.code.libelle().to_string(), rubrique: None })
            .collect()
    } else {
        // Sans piece (ni telechargee, ni archivee), le rejet n'a pas d'anomalie lisible.
        let contenu = match contenus.get(&cle_du_contenu(r)) {
            Some(c) => Some(c.clone()),
            None => archive_du_retour(racine, r, quoi)?.map(|f| lire_archive(&f)).transpose()?,
        };
        let codees = contenu.map(|c| retours::anomalies_codees_d_octets(&c)).transpose()?;
        codees.into_iter().flatten().map(|a| AnomalieDeRetour { categorie: a.categorie, code: a.code, message: a.message, rubrique: Some(a.rubrique) }).collect()
    };
    // Sans doublon, les bloquantes d'abord, puis par code.
    let mut uniques: Vec<AnomalieDeRetour> = Vec::new();
    for a in anomalies {
        if !uniques.contains(&a) {
            uniques.push(a);
        }
    }
    let bloquant = Some(retours::CategorieDAnomalie::Bloquant);
    uniques.sort_by(|x, y| (x.categorie != bloquant, &x.code).cmp(&(y.categorie != bloquant, &y.code)));
    let phrases: Vec<String> = uniques.iter().take(10).map(|a| format!("{} : {}", a.code, a.message).trim_matches(|c| c == ' ' || c == ':').to_string()).collect();
    let resume = match phrases.join(" ; ") {
        r if r.is_empty() => "Rejet sans motif lisible".to_string(),
        r => r,
    };
    let quoi_ = match uniques.iter().find(|a| !a.code.is_empty()) {
        Some(a) => a.code.clone(),
        None => "KO".to_string(),
    };
    uniques.truncate(30);
    Ok(Some(IncidentDeRetour {
        canal: crate::releve_retours::CanalDesRetours::de_l_acte(quoi),
        ou: format!("retour-{}", quoi.nom()),
        quoi: quoi_,
        resume,
        detail: DetailIncidentDeRetour {
            reel: ligne.reel(),
            nature_retour: r.nature_retour,
            type_declaration: ligne.dsn().and_then(|e| e.detail.type_declaration),
            anomalies: uniques,
        },
    }))
}

/// Le nombre ajoute ; les incidents a noter (dans
/// l'ordre) s'ajoutent a `incidents`, que l'appelant ecrit dans le journal des
/// incidents, meme quand le rattachement s'arrete sur une erreur.
fn rattacher_retours(racine: &Path, retours: &[Annonce], contenus: &HashMap<CleDuContenu, Vec<u8>>, incidents: &mut Vec<IncidentDeRetour>) -> R<i64> {
    if !chemin(racine).exists() {
        return Ok(0);
    }
    let mut lignes = (*crate::journal::lu(racine)?.lignes).clone();
    let mut rangements: Vec<(usize, Rangement)> = Vec::new();
    let mut par_flux: HashMap<IdDeFlux, Vec<usize>> = HashMap::new();
    for (i, l) in lignes.iter().enumerate() {
        if let Some(f) = l.flux() {
            par_flux.entry(f.clone()).or_default().push(i);
        }
    }
    let mut ajoutes = 0;
    for r in retours {
        let siennes = match r.flux.as_ref().and_then(|f| par_flux.get(f)) {
            Some(l) => l.as_slice(),
            None => &[],
        };
        for &li in siennes {
            let quoi = lignes[li].acte();
            let verdict = verdict_du_retour(racine, r, quoi, contenus.get(&cle_du_contenu(r)).map(Vec::as_slice))?;
            let ligne = lignes[li].clone();
            let connus = match &mut lignes[li] {
                Ligne::Dsn(e) => &mut e.retours,
                Ligne::Dpae(e) => &mut e.retours,
                _ => continue,
            };
            let neuf = RetourOrganisme { nature_retour: r.nature_retour, verdict: None, anomalies: None, dpae: None, libelle: r.nature_retour.libelle_du_canal(quoi), statut: r.statut.clone(), publication: r.publication.clone() };
            if let Some(x) = connus.iter_mut().find(|x| x.meme(&neuf)) {
                if let Some(v) = verdict.filter(|_| x.verdict.is_none()) {
                    x.poser(v.clone());
                    rangements.push((li, Rangement::Complement { nature_retour: x.nature_retour, publication: x.publication.clone(), verdict: v.clone() }));
                    ajoutes += 1;
                    incidents.extend(incident(racine, &ligne, r, &v, contenus)?);
                }
                continue;
            }
            let mut neuf = neuf;
            if let Some(v) = verdict {
                incidents.extend(incident(racine, &ligne, r, &v, contenus)?);
                neuf.poser(v);
            }
            connus.push(neuf.clone());
            rangements.push((li, Rangement::Retour(neuf)));
            ajoutes += 1;
        }
    }
    crate::journal::ranger(racine, rangements).map_err(|m| m.reclasser(socle::echec::Classe::Disque))?;
    Ok(ajoutes)
}

/// Ce qu'un rattachement rend : le nombre de retours ranges (ou l'erreur qui l'a arrete), et les incidents qu'il a
/// releves, que `moteur` note au journal des incidents, meme quand le rattachement s'est arrete sur une erreur.
pub struct AvecIncidents<T> {
    pub resultat: R<T>,
    /// Dans l'ordre.
    pub incidents: Vec<IncidentDeRetour>,
}

/// Range les retours `retours` (la liste des retours) en face des lignes du journal qu'ils concernent (par le flux).
/// `contenus` donne, pour chaque `(flux, nature, publication)` telecharge, les octets du retour : le verdict se lit dedans.
pub fn rattacher_retours_avec_contenus(racine: &Path, retours: &[Annonce], contenus: &[(CleDuContenu, Vec<u8>)]) -> AvecIncidents<i64> {
    let mut incidents = Vec::new();
    let dico: HashMap<CleDuContenu, Vec<u8>> = contenus.iter().cloned().collect();
    let resultat = rattacher_retours(racine, retours, &dico, &mut incidents);
    AvecIncidents { resultat, incidents }
}

/// `rattacher_retours_avec_contenus` sans contenus : les retours deja archives.
pub fn rattacher_retours_ecrits(racine: &Path, retours: &[Annonce]) -> AvecIncidents<i64> {
    rattacher_retours_avec_contenus(racine, retours, &[])
}

// ------------------------------------------------------ le contrat parti

/// Le dernier contrat parti a la signature pour cette paie :
/// le mois, le numero, le fichier, la date d'envoi et, s'il est revenu, celle de la signature.
/// Un avenant n'en est pas un.
#[derive(Debug, Clone)]
pub struct Parti {
    pub mois: Mois,
    pub numero: Option<engagement::numeros::NumeroContrat>,
    pub fichier: NomDeFichier,
    pub quand: InstantOuJour,
    pub signe: Option<Moment>,
    /// L'empreinte du modele avec lequel il est parti (`ContratParti::modele`).
    pub modele: Option<EmpreinteCourte>,
}

impl Parti {
    /// Le jour qui date le contrat dans un avenant : celui de sa signature, sinon de son envoi.
    pub fn date(&self) -> Jour {
        match &self.signe {
            Some(s) => s.jour_de_paris(),
            None => self.quand.jour(),
        }
    }
}

/// Le contrat n° `numero` de ce salarie (`None` : son contrat sans numero), tel qu'il est parti a la signature.
pub fn contrat_parti(lignes: &[Ligne], salarie: &IdSalarie, numero: Option<&NumeroContrat>) -> Option<Parti> {
    let mut parti: Option<Parti> = None;
    for l in lignes.iter().filter(|l| l.accepte()) {
        if let Some(e) = l.signature() {
            for c in e.detail.contrats.iter().filter(|c| c.salarie == *salarie && c.numero.as_ref() == numero && !c.est_avenant()) {
                parti = Some(Parti { mois: c.mois, numero: c.numero.clone(), fichier: c.fichier.clone(), quand: e.quand.clone(), signe: None, modele: c.modele.clone() });
            }
        }
        for d in revenus(l) {
            if let Some(p) = parti.as_mut() {
                if d.recu && d.salarie == *salarie && d.numero.as_ref() == numero && p.signe.is_none() {
                    p.signe = Some(d.quand);
                }
            }
        }
    }
    parti
}

/// Le fichier d'une piece partie, dans `sorties/<mois>/` ou son `envoi/`. Un NOM, jamais un chemin : le type le tient.
pub fn chemin_piece(racine: &Path, mois: Mois, nom: &NomDeFichier) -> Option<PathBuf> {
    let base = racine.join("sorties").join(mois.to_string());
    [base.join("envoi").join(nom), base.join(nom)].into_iter().find(|f| f.is_file())
}

/// Les avenants du contrat parti -- pas ceux d'un contrat qu'il a remplace. `tous` : les avenants de son contrat
/// (`avenants_par_contrat`).
pub fn avenants_du_contrat(tous: Option<&Vec<AvenantParti>>, parti: &Parti) -> Vec<AvenantParti> {
    tous.into_iter().flatten().filter(|x| x.quand >= parti.quand).cloned().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un journal sur le disque, pour ce qui l'ecrit ou le lit.
    fn journal(nom: &str, lignes: &[Ligne]) -> PathBuf {
        let p = std::env::temp_dir().join(format!("paie-etats-{}-{}", nom, std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(p.join(crate::journal::JOURNAL)).unwrap();
        let texte: String = lignes.iter().enumerate().map(|(i, l)| format!("{{\"id\":\"{:016x}\",\"ligne\":{}}}\n", i, socle::ecriture::json_en_ligne(l))).collect();
        std::fs::write(chemin(&p), texte).unwrap();
        p
    }

    /// Une ligne du journal, ecrite en JSON (`detail`, `retours` : du JSON) et lue en sa forme.
    fn ligne(quoi: &str, mode: &str, quand: &str, empreinte: &str, fichier: &str, accepte: bool, flux: &str, detail: &str, retours: &str) -> Ligne {
        let texte = format!(
            "{{\"quoi\": \"{}\", \"quand\": \"{}\", \"origine\": \"\", \"mois\": \"2026-06\", \"fichier\": \"{}\", \"empreinte\": \"{}\", \
             \"mode\": \"{}\", \"qui\": \"?\", \"accepte\": {}, \"destinataire\": \"\", \"flux\": \"{}\", \"retours\": {}, \
             \"reponse\": \"\", \"trace\": null, \"detail\": {}}}",
            quoi, quand, fichier, empreinte, mode, accepte, flux, retours, detail
        );
        socle::forme::lire_texte(&texte, "ligne de test").unwrap()
    }

    fn contrat(sal: &str, code: &str) -> String {
        format!(
            "{{\"salarie\": \"{}\", \"debut\": \"2026-06-04\", \"fin\": \"2026-06-30\", \"numero\": \"SA1.26.01\", \"fichier\": \"c.pdf\", \"spectacle\": null, \
             \"code\": \"{}\", \"empreinte\": \"e100000000000000\", \"message\": \"<m1@test.invalid>\", \"mois\": \"2026-06\", \"ailleurs\": false}}",
            sal, code
        )
    }

    /// Un contrat revenu (`plus` : ce qu'il dit d'autre, en JSON).
    fn recu(sal: &str, recu: bool, plus: &str) -> String {
        format!(
            "{{\"salarie\": \"{}\", \"debut\": \"2026-06-04\", \"numero\": \"SA1.26.01\", \"nom\": \"{}\", \"recu\": {}, \"quand\": \"2026-06-12T10:00:00\", \"qui\": \"t\", \"reponse\": \"\"{}}}",
            sal, sal, recu, plus
        )
    }

    fn signature(quand: &str, c: &str, retours: &str) -> Ligne {
        ligne("contrat-signature", "reel", quand, "e100000000000000","f.dsn", true, "", &format!("{{\"contrats\": [{}], \"main_propre\": false}}", c), retours)
    }

    #[test]
    fn une_reponse_deja_relevee_ne_se_renote_pas_apres_la_mention_retiree() {
        let ctx = |_: &droit::contexte::IdSalarie| None;
        let lu = recu("ALPHA", true, ", \"origine\": \"courriel\", \"code\": \"SIG-CCCCCA\"");
        let retiree = recu("ALPHA", false, "");
        let mut l = vec![signature("2026-06-05T10:00:00", &contrat("ALPHA", "SIG-CCCCCA"), &format!("[{}, {}]", lu, retiree))];
        assert!(signatures_attendues(&ctx, &l, None).is_empty());
        l.push(signature("2026-06-13T10:00:00", &contrat("ALPHA", "SIG-CCCCCC"), "[]"));
        assert_eq!(signatures_attendues(&ctx, &l, None)[0].code.texte(), "SIG-CCCCCC");
    }

    #[test]
    fn la_repetition_et_le_deja_depose() {
        let avec = |mode: &str, quand: &str, empreinte: &str, fichier: &str, accepte: bool, retours: &str| {
            ligne("dsn", mode, quand, empreinte, fichier, accepte, "", "{\"repetition\": null, \"ordre_test\": null, \"reconstruit\": false}", retours)
        };
        let l = vec![
            avec("test", "2026-06-01T09:00:00", "eb00000000000000", "DSN.dsn", true, "[]"),
            avec("test", "2026-06-01T10:00:00", "ea00000000000000", "f.dsn", true, "[{\"nature\": \"11\", \"verdict\": \"OK\", \"libelle\": \"\", \"statut\": null, \"publication\": null}]"),
            avec("test", "2026-06-01T11:00:00", "ea00000000000000", "f.dsn", false, "[]"),
            avec("reel", "2026-06-02T09:00:00", "ea00000000000000", "f.dsn", true, "[]"),
        ];
        let (ea, eb) = (EmpreinteCourte::lire("ea00000000000000").unwrap(), EmpreinteCourte::lire("eb00000000000000").unwrap());
        let rep = repetition(&l, Acte::Dsn, &ea, &NomDeFichier::lire("DSN.dsn").unwrap());
        assert_eq!((rep.compte_rendu, rep.conforme.as_ref().unwrap().quand().iso()), (Some(CompteRenduDuTest::Ok), "2026-06-01T10:00:00".to_string()));
        assert_eq!((rep.ratees.len(), rep.perimees.len(), rep.etat), (1, 1, EtatDuTest::Testee));
        let vide = repetition(&l, Acte::Dpae, &ea, &NomDeFichier::lire("x").unwrap());
        assert_eq!((vide.conforme.is_none(), vide.compte_rendu, vide.etat), (true, None, EtatDuTest::Ecrite));
        assert_eq!(deja_depose(&l, Acte::Dsn, &ea).unwrap().quand().iso(), "2026-06-02T09:00:00");
        assert!(deja_depose(&l, Acte::Dsn, &eb).is_none());
        let r = |verdict: Verdict| RetourOrganisme { nature_retour: NatureDuRetour::CertificatDeConformite, verdict: Some(verdict), anomalies: None, dpae: None, libelle: String::new(), statut: None, publication: None };
        assert_eq!(compte_rendu_du_depot(&[r(Verdict::Ok), r(Verdict::Ko)]), CompteRenduDuTest::Ko);
        assert_eq!(compte_rendu_du_depot(&[]), CompteRenduDuTest::Attendu);
    }

    const BAN_KO: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ns2:rapport xmlns:ns2=\"http://www.gip-mds.fr/\" type=\"BAN\">\
         <envoi><idflux>WSBBBB</idflux></envoi><declaration><declaration_bilan><etat>KO</etat>\
         <declaration_anomalie><code>CST-04</code><categorie>bloquant</categorie><message>Code URSSAF invalide</message>\
         <identifiant>S21.G00.20.001</identifiant><nom_famille>FAVART</nom_famille></declaration_anomalie>\
         </declaration_bilan></declaration></ns2:rapport>";

    #[test]
    fn le_rattachement_pose_le_verdict_et_signale_l_incident() {
        let l = ligne("dsn", "test", "2026-06-01T10:00:00", "e100000000000000", "f.dsn", true, "WSBBBB", "{\"repetition\": null, \"ordre_test\": null, \"reconstruit\": false}", "[]");
        let autre = ligne("dpae", "reel", "2026-06-01T10:00:00", "e100000000000000", "f.dsn", true, "WSCCCC", "{\"repetition\": null, \"reconstruit\": false, \"embauches\": [], \"certificat\": false}", "[]");
        let r = journal("rattachement", &[l, autre]);
        let annonce = |flux: &str, nature: NatureDuRetour, seconde: i64| Annonce {
            publication: Some(socle::paris::Moment::a_paris(Jour::nouveau(2026, 6, 1).unwrap(), 11, 0, seconde).unwrap()),
            production: None,
            ops: None,
            delegataire: None,
            nature_retour: nature,
            statut: Some(retours::StatutDuRetour::Ok),
            id: None,
            url: None,
            libelle: None,
            flux: Some(IdDeFlux::lire(flux).unwrap()),
        };
        let retours = vec![
            annonce("WSBBBB", NatureDuRetour::CertificatDeConformite, 1),
            annonce("WSINCONNU", NatureDuRetour::CertificatDeConformite, 2),
            annonce("WSCCCC", NatureDuRetour::BilanDIdentification, 3),
        ];
        let mut contenus = HashMap::new();
        contenus.insert(cle_du_contenu(&retours[0]), BAN_KO.as_bytes().to_vec());
        let mut inc = Vec::new();
        assert_eq!(rattacher_retours(&r, &retours, &contenus, &mut inc).unwrap(), 2);
        assert_eq!(inc.len(), 1);
        assert_eq!((inc[0].canal, inc[0].ou.as_str(), inc[0].quoi.as_str()), (crate::releve_retours::CanalDesRetours::Dsn, "retour-dsn", "CST-04"));
        assert_eq!(inc[0].resume, "CST-04 : Code URSSAF invalide");
        assert!(!inc[0].detail.reel);
        assert_eq!(inc[0].detail.anomalies[0].rubrique.as_deref(), Some("S21.G00.20.001"));
        // ni le nom du salarie ni la valeur ne partent chez l'editeur
        assert!(!socle::ecriture::json_en_ligne(&inc[0]).contains("FAVART"));
        let fichier = crate::journal::lu(&r).unwrap().lignes;
        let pose = &fichier[0].retours_organisme()[0];
        assert_eq!(pose.verdict(), Some(Verdict::Ko));
        assert_eq!(pose.anomalies(), Some(&["Code URSSAF invalide (bloquant) -- FAVART".to_string()][..]));
        assert_eq!(pose.libelle, "Certificat de conformite / bilan d'anomalies");
        assert_eq!(fichier[1].retours_organisme()[0].libelle, "bilan d'identification des salaries");
        // un second passage ne rajoute rien
        assert_eq!(rattacher_retours(&r, &retours, &contenus, &mut Vec::new()).unwrap(), 0);
        std::fs::remove_dir_all(&r).unwrap();
    }
}
