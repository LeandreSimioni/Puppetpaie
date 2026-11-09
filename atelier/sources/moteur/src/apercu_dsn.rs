//! `depot-apercu` pour une DSN : ce qui partirait, en clair, sans rien envoyer ni demander -- le fichier lu, son
//! annule-et-remplace, et ce qu'il declare compare aux engagements valides du mois, contrat par contrat.
//!
//! Aucun reseau, aucun mot de passe, aucune ecriture de ce module.
//!
//! Les pieces reprises du reste du moteur : `Moteur::etat_remplacement` (l'annule-et-
//! remplace d'un etablissement), `engagement::etats::validees_du_mois` et
//! `ecrire_documents::paies_aux_numeros_declares` (les paies validees, numerotees), `dsn_lue::lire` (la lecture du
//! fichier), `etats::{deja_depose, repetition}` (le journal).
//!
//! Un nombre `NaN`, `Infinity`, ou dont l'exposant sortirait des 28 chiffres de `rust_decimal`, se refuse comme un
//! nombre illisible.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use declarations::dsn_lue::{ContratDeclare, DsnLue, Mesures, VersementLu};
use documents::ecrire_documents::paies_aux_numeros_declares;
use droit::contexte::Contexte;
use droit::fiche::Lue as _;
use engagement::calcul::Paie;
use envois::depot;
use envois::etats;
use socle::date::Jour;
use socle::echec::{erreur, Classe, Echec};
use socle::fichiers::repr;
use socle::mois::Mois;
use socle::num::{entier_lu, q2, Decimal};

use crate::routes::depots::{lire_octets, nom_du_chemin, resoudre};
use crate::routes::etapes::Moteur;
use socle::ecran::Pastille;

type R<T> = Result<T, Echec>;

const TYPES: [(&str, &str); 5] = depot::TYPES;

/// Les paies validees du mois, a leurs numeros declares.
fn saisie_aux_numeros_declares(ctx: &Contexte, mois: &str) -> Result<Vec<Paie>, String> {
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    paies_aux_numeros_declares(ctx, mois, None, &departs).map(|t| t.1).map_err(|r| r.message)
}

// ------------------------------------------------------------------ recapitulatif_dsn

/// Un nombre en texte, sans ses zeros de fin ; `0` pour zero.
fn norm(d: Decimal) -> String {
    if d.is_zero() {
        "0".to_string()
    } else {
        d.normalize().to_string()
    }
}

/// Un contrat du fichier (`DsnLue`), et ce qu'il declare pour le mois et en rappel d'un mois deja declare. Son numero
/// (S21.G00.40.009) et son debut (S21.G00.40.001) sont obligatoires : un contrat qui ne les porte pas ne se lit pas.
struct ContratLu {
    numero: String,
    debut: Jour,
    fin: Option<Jour>,
    emploi: Option<String>,
    nature: Option<String>,
    objet: Option<String>,
    fin_declaree: Option<Jour>,
    motif_fin: Option<String>,
    mois: Mesures,
    rappel: Mesures,
}

impl ContratLu {
    fn de(d: &DsnLue, c: &ContratDeclare) -> Result<ContratLu, String> {
        let numero = c.numero.clone().ok_or_else(|| "Un contrat du fichier ne porte pas son numéro (S21.G00.40.009).".to_string())?;
        let debut = c.debut.ok_or_else(|| format!("Le contrat {} du fichier ne porte pas sa date de début (S21.G00.40.001).", numero))?;
        Ok(ContratLu {
            mois: d.mesures(&c.nir, &numero, false),
            rappel: d.mesures(&c.nir, &numero, true),
            debut,
            fin: c.fin,
            emploi: c.emploi.emploi.clone(),
            nature: c.nature_dsn.clone(),
            objet: c.objet.clone(),
            fin_declaree: c.fin_declaree,
            motif_fin: c.motif_fin.clone(),
            numero,
        })
    }
}

/// Un versement du fichier (S21.G00.50.001, .004, .002, .006, .009), tel qu'il l'ecrit ; ce qu'il ne dit pas est vide.
#[derive(Debug, Serialize)]
struct VersementDuFichier {
    date: Option<String>,
    net_verse: Option<String>,
    net_fiscal: Option<String>,
    taux_pas: Option<String>,
    montant_pas: Option<String>,
}

impl VersementDuFichier {
    fn de(v: &VersementLu) -> VersementDuFichier {
        let t = |m: Option<Decimal>| m.map(|m| m.to_string());
        VersementDuFichier { date: v.verse_le.map(|j| j.iso()), net_verse: t(v.net_verse), net_fiscal: t(v.net_imposable), taux_pas: t(v.taux_pas), montant_pas: t(v.pas) }
    }
}

/// Un texte de la paie ou du fichier dans la phrase d'un ecart : entre guillemets, « rien » quand il n'est pas dit.
fn en_mots(t: Option<&str>) -> String {
    match t {
        Some(t) => format!("« {} »", t),
        None => "rien".to_string(),
    }
}

/// CE QU'UN CONTRAT DU FICHIER DIT AUTREMENT QUE LA PAIE `p` QUI LE PORTE : son brut, ses mesures d'activite, son emploi.
fn ecarts_a_la_paie(p: &Paie, m: &Mesures, c: &ContratLu) -> Result<Vec<String>, String> {
    let mut ecarts = Vec::new();
    // Le brut que la paie declare en 51 de type 001 (`dsn::brut_du_51` : abattu sous deduction forfaitaire specifique), a
    // deux decimales.
    let brut = declarations::dsn::brut_du_51(p).round_dp(2);
    match m.brut {
        Some(f) if f.round_dp(2) == brut => {}
        Some(f) => ecarts.push(format!("brut : la paie dit {}, le fichier {}", q2(brut), q2(f))),
        None => ecarts.push(format!("brut : la paie dit {}, le fichier n'en déclare pas", q2(brut))),
    }
    // Les mesures du travail remunere (type 01) que la DSN ecrit pour cette paie (`dsn::mesures_d_activite`).
    let (mut jours, mut cachets, mut heures) = (Decimal::ZERO, Decimal::ZERO, Decimal::ZERO);
    for (t, valeur, code) in declarations::dsn::mesures_d_activite(p)? {
        if t != "01" {
            continue;
        }
        match code.as_str() {
            "10" | "35" => heures += valeur,
            "12" => jours += valeur,
            "37" => cachets += valeur,
            _ => {}
        }
    }
    for (quoi, paie, fichier) in [("jours", jours, m.jours), ("cachets", cachets, m.cachets), ("heures", heures, m.heures)] {
        if paie != fichier {
            ecarts.push(format!("{} : la paie dit {}, le fichier {}", quoi, norm(paie), norm(fichier)));
        }
    }
    let emploi = p.salarie.emploi.en_texte();
    if emploi != c.emploi {
        ecarts.push(format!("emploi : la paie dit {}, le fichier {}", en_mots(emploi.as_deref()), en_mots(c.emploi.as_deref())));
    }
    Ok(ecarts)
}

/// `recapitulatif_dsn` : ce que la DSN declare, contrat par contrat, comparé aux contrats saisis de l'etablissement `nic`
/// (tous sans etablissement). La paie du mois qui ne se lit pas (ou l'etablissement d'un salarie) ne se compare pas, et
/// la phrase le dit.
fn recapitulatif_dsn(ctx: &Contexte, d: &DsnLue, mois: &str, nic: Option<&str>) -> Result<RecapDsn, String> {
    let mut sans_comparaison: Option<String> = None;
    let liste = match saisie_aux_numeros_declares(ctx, mois) {
        Ok(l) => l,
        Err(m) => {
            sans_comparaison = Some(m);
            Vec::new()
        }
    };
    // `par_numero` : numero -> paie, dans l'ordre ou les numeros sont apparus (`ordre`).
    let mut ordre: Vec<String> = Vec::new();
    let mut par_numero: HashMap<String, usize> = HashMap::new();
    for (i, p) in liste.iter().enumerate() {
        if let Some(nic) = nic {
            match droit::bareme::etablissement_de(&ctx.entreprise, &p.salarie) {
                Ok(e) if e == nic => {}
                Ok(_) => continue,
                Err(m) => {
                    sans_comparaison = Some(m);
                    break;
                }
            }
        }
        let n = p.engagement.numero.as_ref().ok_or_else(|| format!("La paie du contrat du {} n'a pas de numéro de contrat.", p.engagement.debut.jj_mm_aaaa()))?.to_string();
        if !par_numero.contains_key(&n) {
            ordre.push(n.clone());
        }
        par_numero.insert(n, i);
    }
    let compare = sans_comparaison.is_none() && !liste.is_empty();
    let mut vus: HashSet<String> = HashSet::new();
    let mut salaries = Vec::new();
    let (mut n_contrats, mut n_ecarts, mut n_corrections) = (0usize, 0usize, 0usize);
    for sal in &d.individus {
        let lus = d.contrats_de(&sal.nir).map(|c| ContratLu::de(d, c)).collect::<Result<Vec<ContratLu>, String>>()?;
        let mut contrats = Vec::new();
        // Un contrat se dit par ce qu'il declare pour le mois, comparé a la paie du mois, et par ses rappels d'un mois deja
        // declare (`dsn_lue::Remuneration::rappel`), qui ne se comparent pas a la paie du mois. Un contrat qui n'a que des rappels
        // n'a pas de ligne du mois.
        for (c, correction) in lus.iter().flat_map(|c| [(c, false), (c, true)]) {
            let m = if correction { &c.rappel } else { &c.mois };
            if (correction || c.rappel.remunere) && !m.remunere {
                continue;
            }
            vus.insert(c.numero.clone());
            let p = par_numero.get(&c.numero).map(|&i| &liste[i]);
            let mut ecarts: Vec<String> = Vec::new();
            match p {
                _ if correction => {}
                None => {
                    if compare {
                        ecarts.push("aucune paie du mois ne porte ce contrat".to_string());
                    }
                }
                Some(p) => ecarts.extend(ecarts_a_la_paie(p, m, c)?),
            }
            if correction {
                n_corrections += 1;
            } else {
                n_contrats += 1;
            }
            if !ecarts.is_empty() {
                n_ecarts += 1;
            }
            let fin_dit = match (correction, c.fin_declaree) {
                (true, _) => "mois déjà déclaré".to_string(),
                (false, Some(j)) => format!("fin le {}", j.jj_mm_aaaa()),
                (false, None) => "en cours".to_string(),
            };
            contrats.push(ContratDuFichier {
                activite_dit: activite_dite(m, correction),
                dates_dit: dates_dites(c.debut, c.fin),
                fin_dit,
                numero: c.numero.clone(),
                debut: c.debut.iso(),
                fin: c.fin.map(|j| j.iso()),
                emploi: c.emploi.clone(),
                nature: c.nature.clone(),
                objet: c.objet.clone(),
                brut: m.brut.map(q2),
                jours: norm(m.jours),
                cachets: norm(m.cachets),
                heures: norm(m.heures),
                fin_declaree: c.fin_declaree.map(|j| j.iso()),
                motif_fin: c.motif_fin.clone(),
                remunere: m.remunere,
                paie: p.is_some(),
                correction,
                // Un contrat du fichier qui ne dit pas ce que dit la paie : a corriger, la compagnie le peut.
                pastille_ecarts: (!ecarts.is_empty()).then(|| Pastille::de("a_faire", &ecarts.join(" ; "))),
                ecarts,
            });
        }
        let versements = d.versements_de(&sal.nir).map(VersementDuFichier::de).collect();
        salaries.push(SalarieDuFichier { nir: sal.nir.clone(), nom: sal.nom.clone(), prenoms: sal.prenom.clone(), contrats, versements });
    }
    let mut absents = Vec::new();
    for n in &ordre {
        if vus.contains(n) {
            continue;
        }
        let p = &liste[par_numero[n]];
        let nom = p.salarie.nomme();
        let dit = match &nom {
            Some(x) => format!("{} — contrat {}, du {}", x, n, p.engagement.debut.jj_mm_aaaa()),
            None => format!("Contrat {}, du {}", n, p.engagement.debut.jj_mm_aaaa()),
        };
        absents.push(AbsentDuFichier {
            salarie: p.salarie.id.to_string(),
            dit,
            nom,
            numero: n.clone(),
            debut: p.engagement.debut.iso(),
            fin: p.engagement.fin.map(|f| f.iso()),
            pastille: Pastille::de("a_faire", "absent du fichier"),
        });
    }
    let conforme = compare && absents.is_empty() && n_ecarts == 0;
    let s = |n: usize| if n > 1 { "s" } else { "" };
    let mut titre = format!("{} contrat{} dans ce fichier", n_contrats, s(n_contrats));
    if n_corrections > 0 {
        titre += &format!(" (et {} correction{} d'un mois déjà déclaré)", n_corrections, s(n_corrections));
    }
    if compare {
        titre += &format!(", {} saisi{} dans la paie du mois", par_numero.len(), s(par_numero.len()));
    }
    let phrase = if conforme {
        "Tous les contrats de la paie sont dans la DSN, tels que la paie les dit.".to_string()
    } else if compare {
        let mut p = Vec::new();
        if !absents.is_empty() {
            let n = absents.len();
            p.push(format!("{} contrat{} de la paie manque{} au fichier.", n, s(n), if n > 1 { "nt" } else { "" }));
        }
        if n_ecarts > 0 {
            let dit = if n_ecarts > 1 { "disent" } else { "dit" };
            p.push(format!("{} contrat{} du fichier ne {} pas ce que dit la paie : réécrire le mois.", n_ecarts, s(n_ecarts), dit));
        }
        p.join(" ")
    } else if n_contrats == 0 && n_corrections == 0 {
        // Un mois sans salarie a sa DSN, « Sans individu » (cahier technique DSN 2026.1, § 2.4,
        // `atelier/reference/norme-dsn/texte/chapitre-2-principes-de-constitution.txt` l. 2291-2309).
        "Aucun salarié ce mois-ci : la DSN se dépose sans individu.".to_string()
    } else if let Some(m) = &sans_comparaison {
        format!("Le fichier ne se compare pas à la paie du mois : {}", socle::echec::phrase_de(m))
    } else {
        "Aucune paie validée ce mois-ci : le fichier ne se compare à rien.".to_string()
    };
    Ok(RecapDsn {
        salaries,
        absents,
        titre,
        phrase,
        compare,
        conforme,
        // La couleur du bilan : conforme, vert ; un ecart ou un absent, orange ; rien a comparer, gris.
        couleur: if conforme { "vert" } else if compare { "orange" } else { "gris" },
    })
}

// ------------------------------------------------------------------ l'apercu

/// CE QUE LA DSN DECLARE, CONTRAT PAR CONTRAT, compare aux contrats saisis (`recapitulatif_dsn`) : ses salaries, les
/// contrats de la paie absents du fichier, le titre et la phrase de l'encart, sa couleur.
#[derive(Debug, Serialize)]
pub(crate) struct RecapDsn {
    salaries: Vec<SalarieDuFichier>,
    absents: Vec<AbsentDuFichier>,
    titre: String,
    pub(crate) phrase: String,
    compare: bool,
    conforme: bool,
    couleur: &'static str,
}

/// Le recapitulatif, ou pourquoi il ne se lit pas.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum Recapitulatif {
    Lu(RecapDsn),
    Illisible { erreur: String },
}

/// Un salarie du fichier : son identite, ses contrats, ses versements.
#[derive(Debug, Serialize)]
struct SalarieDuFichier {
    nir: String,
    nom: Option<String>,
    prenoms: Option<String>,
    contrats: Vec<ContratDuFichier>,
    versements: Vec<VersementDuFichier>,
}

/// Un contrat du fichier, ce qu'il declare pour le mois (ou en correction d'un mois deja declare), en clair, et ses
/// ecarts a la paie. Ce que le fichier ne dit pas est vide.
#[derive(Debug, Serialize)]
struct ContratDuFichier {
    activite_dit: String,
    dates_dit: String,
    fin_dit: String,
    numero: String,
    debut: String,
    fin: Option<String>,
    emploi: Option<String>,
    nature: Option<String>,
    objet: Option<String>,
    brut: Option<String>,
    jours: String,
    cachets: String,
    heures: String,
    fin_declaree: Option<String>,
    motif_fin: Option<String>,
    remunere: bool,
    paie: bool,
    correction: bool,
    pastille_ecarts: Option<Pastille>,
    ecarts: Vec<String>,
}

/// Un contrat de la paie du mois que le fichier ne porte pas.
#[derive(Debug, Serialize)]
struct AbsentDuFichier {
    salarie: String,
    nom: Option<String>,
    numero: String,
    debut: String,
    fin: Option<String>,
    dit: String,
    pastille: Pastille,
}

/// L'ANNULE-ET-REMPLACE DU MOIS (`Moteur::etat_remplacement`), et si ce fichier remplace la DSN reelle deja partie.
#[derive(Debug, Serialize)]
pub(crate) struct RemplacementDuFichier {
    #[serde(flatten)]
    pub(crate) etat: crate::routes::etapes::Remplacement,
    pub(crate) fichier_remplace: bool,
}

/// L'annule-et-remplace du mois de l'etablissement du fichier. Le fichier remplace la DSN partie quand il est un
/// annule-et-remplace (type 03 ou 05) d'un ordre plus grand que le dernier depose.
fn remplacement_du_fichier(m: &mut Moteur, a: &DsnLue, mois: Mois) -> R<RemplacementDuFichier> {
    let r = m.etat_remplacement(mois, a.etablissement())?;
    let remplace = match (a.type_declaration.as_deref(), a.ordre.as_deref()) {
        (Some("03" | "05"), Some(ordre)) if r.deposee => {
            let n = entier_lu(ordre).ok_or_else(|| erreur(Classe::Illisible, format!("nombre entier illisible : {}", repr(ordre))))?;
            r.dernier_ordre.is_none_or(|d| n > i128::from(d))
        }
        _ => false,
    };
    Ok(RemplacementDuFichier { etat: r, fichier_remplace: remplace })
}

/// CE QUE LE FICHIER DSN DECLARE, en clair (`apercu_depot_dsn`) : son en-tete (ce qu'il ne dit pas est vide), ce qu'il
/// declare, ses ordres de telepaiement, s'il est deja depose (`deja_depose` : la ligne du journal), l'annule-et-remplace,
/// le recapitulatif et le test.
#[derive(Debug, Serialize)]
pub(crate) struct ApercuDsn {
    pub(crate) fichier: String,
    pub(crate) nom: String,
    pub(crate) description: String,
    pub(crate) mois: Mois,
    pub(crate) nature_envoi: Option<String>,
    pub(crate) reel: bool,
    pub(crate) signalement: bool,
    pub(crate) nature: String,
    pub(crate) libelle_nature: String,
    pub(crate) evenement: Option<String>,
    pub(crate) type_declaration: Option<String>,
    pub(crate) libelle_type: Option<&'static str>,
    pub(crate) ordre: Option<String>,
    pub(crate) fraction: Option<String>,
    pub(crate) remplace: Option<String>,
    pub(crate) constitution: Option<String>,
    pub(crate) siren: Option<String>,
    pub(crate) salaries: usize,
    pub(crate) lignes: usize,
    pub(crate) pas_versements: usize,
    pub(crate) pas_bareme: usize,
    pub(crate) ordres_phrase: &'static str,
    pub(crate) ordres_titre: String,
    pub(crate) ordres: Vec<declarations::ordres_paiement::OrdreEnClair>,
    pub(crate) ordres_erreur: Option<&'static str>,
    pub(crate) couleur_ordres: &'static str,
    pub(crate) deja_depose: Option<envois::ligne::Ligne>,
    pub(crate) remplacement: Option<RemplacementDuFichier>,
    pub(crate) recapitulatif: Option<Recapitulatif>,
    pub(crate) repetition: etats::Repetition,
}

/// La description du fichier en une phrase : son nom, son type, son ordre, sa fraction, sa constitution, ses salaries,
/// ses lignes, son SIREN ; ce qu'il ne dit pas ne se dit pas.
fn description(nom: &str, a: &DsnLue, libelle_type: Option<&str>) -> String {
    let mut d = nom.to_string();
    match (&a.type_declaration, libelle_type) {
        (Some(g), Some(l)) => d += &format!(" — {} (type {})", l, g),
        (Some(g), None) => d += &format!(" — type {}", g),
        (None, _) => {}
    }
    for (quoi, v) in [("ordre", a.ordre.clone()), ("fraction", a.fraction.clone()), ("constituée le", a.depot.map(|j| j.jj_mm_aaaa()))] {
        if let Some(v) = v {
            d += &format!(", {} {}", quoi, v);
        }
    }
    d += &format!(". {} salarié{}, {} lignes", a.individus.len(), if a.individus.len() > 1 { "s" } else { "" }, a.lignes);
    if let Some(s) = &a.emetteur {
        d += &format!(", SIREN {}", s);
    }
    d.push('.');
    d
}

/// `apercu_depot_dsn` : ce qui partirait, en clair, sans rien envoyer ni demander.
pub(crate) fn apercu_depot_dsn(ctx: &Contexte, fichier: &str) -> R<ApercuDsn> {
    let chemin = resoudre(&ctx.racine, fichier)?;
    let contenu = lire_octets(&chemin)?;
    let a = declarations::dsn_lue::lire(&contenu, "")?;
    let mut m = Moteur::nouveau(ctx);
    let lignes = m.lignes()?;
    let empreinte = envois::journal::empreinte(&contenu);
    let deja = etats::deja_depose(&lignes, envois::journal::Acte::Dsn, &empreinte);
    let nom = nom_du_chemin(&chemin);
    let signalement = a.est_signalement();
    let mois = a.mois_declare().map_err(|m| erreur(Classe::Refuse, format!("Ce fichier ne dit pas le mois qu'il déclare ({}).", m)))?;
    let mois_t = mois.to_string();
    let remplacement = if signalement { None } else { Some(remplacement_du_fichier(&mut m, &a, mois)?) };
    let recapitulatif = if signalement {
        None
    } else {
        Some(match recapitulatif_dsn(ctx, &a, &mois_t, a.etablissement()) {
            Ok(v) => Recapitulatif::Lu(v),
            Err(erreur) => Recapitulatif::Illisible { erreur },
        })
    };
    let repetition = etats::repetition(&lignes, envois::journal::Acte::Dsn, &empreinte, &nom);
    // LES ORDRES DE PAIEMENT EN CLAIR : a qui, pour quoi, combien, pour quand -- ceux de la DSN fabriquee
    // depuis les paies qu'elle couvre (`depart::refabriquer`) ; un fichier qui ne dit plus ce que disent les paies n'en
    // montre aucun.
    let (ordres, ordres_erreur) = if signalement {
        (Vec::new(), None)
    } else {
        match crate::gestes::depart::refabriquer(ctx, std::path::Path::new(&chemin), mois, a.etablissement())? {
            Some(f) => (declarations::ordres_paiement::decrire(&f.ordres, &mois_t, ctx.echeance_dsn()?)?, None),
            None => (Vec::new(), Some("Ce fichier ne dit plus ce que disent les paies du mois : le refaire (Produire la DSN) avant de le déposer.")),
        }
    };
    let libelle_type = a.type_declaration.as_deref().and_then(|g| TYPES.iter().find(|(c, _)| *c == g)).map(|(_, l)| *l);
    Ok(ApercuDsn {
        description: description(&nom, &a, libelle_type),
        fichier: chemin.clone(),
        nom,
        mois,
        nature_envoi: a.nature_envoi.clone(),
        reel: a.reel(),
        signalement,
        nature: a.nature().map_err(|m| erreur(Classe::Refuse, m))?.to_string(),
        libelle_nature: a.nature_lisible().ok_or_else(|| erreur(Classe::Refuse, "La DSN ne porte pas sa nature (S20.G00.05.001)."))?,
        evenement: if signalement { a.evenement().map(|j| j.iso()) } else { None },
        type_declaration: a.type_declaration.clone(),
        libelle_type,
        ordre: a.ordre.clone(),
        fraction: a.fraction.clone(),
        remplace: a.remplace.clone(),
        constitution: a.depot.map(|j| j.jj_mm_aaaa()),
        siren: a.emetteur.clone(),
        salaries: a.individus.len(),
        lignes: a.lignes,
        pas_versements: a.pas_versements(),
        pas_bareme: a.pas_bareme(),
        // Ce que la boite dit sous les ordres.
        ordres_phrase: if ordres.is_empty() { "Aucun ordre de télépaiement dans ce fichier." } else { "Chaque organisme prélèvera ce montant sur le compte de la compagnie ; la date se lit dans la DSN ou dans le texte qui la fixe (au survol)." },
        ordres_titre: format!("Ordres de télépaiement ({})", ordres.len()),
        ordres,
        ordres_erreur,
        // Un depot reel les declenche tous : a regarder (orange) ; joues, rien ne part (une information).
        couleur_ordres: socle::ecran::couleur_de_l_etat(if crate::compagnie::bridage::branchement(&ctx.racine)?.joue() { "information" } else { "a_faire" }),
        deja_depose: deja,
        remplacement,
        recapitulatif,
        repetition,
    })
}

/// Un nombre du fichier, a la francaise ; une correction le signe d'un vrai moins (« −184 » : ce qui s'ajoute a ce qui a
/// deja ete declare).
fn nombre_dit(x: Decimal) -> String {
    socle::texte::virgule(x).replacen('-', "−", 1)
}

/// L'ACTIVITE D'UN CONTRAT EN UNE COLONNE : « 4 cachets · 4 j », « 151,67 h » ; « correction : » devant celle d'un mois
/// deja declare ; « — » sans rien.
fn activite_dite(m: &Mesures, correction: bool) -> String {
    let mut l = Vec::new();
    if !m.cachets.is_zero() {
        l.push(format!("{} {}", nombre_dit(m.cachets), if m.cachets.abs() > Decimal::ONE { "cachets" } else { "cachet" }));
    }
    if !m.heures.is_zero() {
        l.push(format!("{} h", nombre_dit(m.heures)));
    }
    if !m.jours.is_zero() {
        l.push(format!("{} j", nombre_dit(m.jours)));
    }
    let a = if l.is_empty() { "—".to_string() } else { l.join(" · ") };
    if correction { format!("correction : {}", a) } else { a }
}

/// Les dates d'un contrat sur une ligne : « 06/10 → 09/10/2026 » dans une meme annee, le jour seul s'il n'en a qu'un.
fn dates_dites(debut: Jour, fin: Option<Jour>) -> String {
    match fin {
        Some(f) if f != debut => {
            let d = debut.jj_mm_aaaa();
            let d = if debut.iso().get(..4) == f.iso().get(..4) { d.chars().take(5).collect() } else { d };
            format!("{} → {}", d, f.jj_mm_aaaa())
        }
        _ => debut.jj_mm_aaaa(),
    }
}
