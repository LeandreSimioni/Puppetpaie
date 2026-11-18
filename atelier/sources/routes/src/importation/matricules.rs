//! LE MATRICULE QUE DECLARE UNE DSN QUI ARRIVE (S21.G00.30.019, « Le choix de la valeur que prend le matricule revient
//! a l'employeur » : `norme-dsn/texte-2027/bloc-S21.G00.30.txt`). Le moteur donne d'office le suivant a une fiche neuve
//! dont personne n'a dit le matricule (`fiches::ecriture`, `completer_neuve`), et la fiche le garde comme tel
//! (`matricule_d_office`). Quand une DSN reprise arrive ensuite et declare un autre matricule pour ce salarie, la DSN
//! fait foi (Léandre, 18/11/2026 : « oui », a « le matricule que declare une DSN qui arrive remplace-t-il celui attribue
//! d'office, tant que rien n'est parti d'ici ? ») : la fiche prend le matricule declare, de tout temps (un matricule que
//! rien n'a declare n'a rien dit au passe). L'import en deux fois (les bulletins, puis la DSN) amene ainsi aux matricules
//! de l'import en une fois, ou la DSN cree les fiches avec ses matricules.
//!
//! CE QUI NE SE REMPLACE PAS, et se dit (`SortDeLaValeur`) : le matricule d'un salarie qu'une DSN partie d'ici a declare
//! (une DSN mensuelle ou un signalement l'ecrivent en S21.G00.30.019 : `declarations::dsn`, `declarations::signalement` ;
//! la DPAE n'en porte pas), lu dans le journal (`engagement::departs`) ; un matricule que la compagnie a dit (saisi, ou
//! lu d'une piece : il n'est pas celui donne d'office) ; un matricule declare qu'une autre fiche porte sans le ceder.
//!
//! L'ECHANGE : les matricules declares peuvent permuter ceux d'office (MARIVAUX 4 → 2 pendant que MOLIERE 2 → 4). Tout se
//! decide sur l'etat final, avant d'ecrire, et s'ecrit en une fois (`ecrire`) : aucun etat intermediaire ne bute sur un
//! doublon. Un matricule d'office que la DSN declare pour un autre salarie (une fiche qui change, ou une fiche neuve) se
//! cede : sa fiche prend le suivant libre, d'office encore. Une fiche neuve dont le matricule declare est celui d'une
//! fiche qui ne le cede pas refuse l'import, avant que rien ne s'ecrive, et le refus dit quoi corriger.
//!
//! Comme pour les contrats (`nature::la_dsn_fait_foi`), la DSN remplace a l'import ou elle arrive : seule une fiche dont
//! une paie declaree arrive avec cet import prend le matricule declare ; un import rejoue n'en apporte aucune.

use std::collections::HashSet;

use droit::contexte::{Contexte, IdSalarie};
use droit::fiche::{ChangementSalarie, FicheDatee as _, FicheSalarieDatee, Matricule};
use engagement::departs::{Departs, Envoi};
use import::proposee::FicheProposee;
use import::reponse::{SortDeLaValeur, ValeurDeLaDsn, ValeurDeclaree, ValeurDuMatricule};
use socle::echec::{erreur, Classe, R};

/// UN MATRICULE QUE L'IMPORT ECRIT dans une fiche qui existe : le declare, ou le suivant que prend une fiche qui cede le
/// sien (`d_office` : il l'est encore).
struct Change {
    salarie: IdSalarie,
    matricule: Matricule,
    d_office: bool,
}

/// CE QUE L'IMPORT FAIT DES MATRICULES (`prevoir`) : ce qu'il ecrit (`ecrire`) et ce qu'il en dit.
pub(crate) struct Matricules {
    changes: Vec<Change>,
    pub(crate) dits: Vec<ValeurDeLaDsn>,
}

/// Le matricule donne d'office que la fiche porte encore : sa seule valeur, de tout temps, est celle que le moteur lui
/// a donnee (`matricule_d_office`). Un matricule saisi depuis, ou date, ne l'est plus.
fn d_office(f: &FicheSalarieDatee) -> Option<&Matricule> {
    match (f.code_salarie.as_deref(), f.matricule_d_office.as_deref()) {
        (Some([c]), Some([d])) if c.depuis.is_none() && d.depuis.is_none() => match (&c.valeur, &d.valeur) {
            (Some(c), Some(d)) if meme(c, d) => Some(c),
            _ => None,
        },
        _ => None,
    }
}

/// Deux matricules sont le meme, a leurs blancs de bord pres (`fiches::ecriture`, « un matricule, un salarie »).
fn meme(a: &Matricule, b: &Matricule) -> bool {
    a.trim() == b.trim()
}

/// Une DSN ou un signalement partis d'ici ont-ils declare un engagement du salarie `cle` (et donc son matricule) ?
fn declare_d_ici(departs: &Departs, cle: &IdSalarie) -> bool {
    departs.engagements().filter(|(c, _)| *c == cle).any(|(c, n)| departs.de(c, n).iter().any(|d| matches!(d.envoi, Envoi::Dsn | Envoi::Signalement)))
}

/// Une fiche qui existe, telle que l'import la trouve.
struct Porteur {
    cle: IdSalarie,
    nom: String,
    actuel: Option<Matricule>,
    /// Son matricule lui a ete donne d'office, et rien d'ici ne l'a declare : il se remplace, ou se cede.
    libre: bool,
    /// Une DSN partie d'ici a declare son matricule.
    parti: bool,
}

/// CE QUE L'IMPORT `ap` FERA DES MATRICULES, decide sur l'etat final, sans rien ecrire ; `departs` : ce que le journal
/// dit (lu une fois). Refus, sans rien ecrire, quand une fiche neuve declare le matricule d'une fiche qui ne le cede pas.
pub(crate) fn prevoir(ctx: &Contexte, ap: &crate::import::Apercu, departs: &Departs) -> R<Matricules> {
    let fichier = crate::fiches::lire_fichier::<FicheSalarieDatee>(ctx)?;
    let porteurs: Vec<Porteur> = fichier
        .fiches
        .iter()
        .map(|(cle, f)| {
            let au = f.au(&ctx.lue_au);
            let parti = declare_d_ici(departs, cle);
            Porteur {
                cle: cle.clone(),
                nom: match au.nomme() {
                    Some(n) => n,
                    None => cle.to_string(),
                },
                actuel: au.code_salarie.clone(),
                libre: d_office(f).is_some() && !parti,
                parti,
            }
        })
        .collect();
    // Les salaries dont une paie declaree arrive avec cet import : la DSN qui la declare est celle qui arrive.
    let deja: HashSet<_> = crate::import::paies_historique(ctx)?.iter().map(engagement::importe::PaieRelue::cle).collect();
    let arrivent: HashSet<IdSalarie> = ap
        .paies
        .iter()
        .filter(|x| !x.paie.contrat.d_un_bulletin())
        .filter_map(|x| {
            let (m, s, c) = x.cle();
            let s = s?;
            (!deja.contains(&(m, s.clone(), c))).then_some(s)
        })
        .collect();
    let mut dits = Vec::new();
    let dit = |p: &Porteur, valeur: ValeurDuMatricule, sort: SortDeLaValeur, phrase: socle::texte::Phrase| ValeurDeLaDsn {
        salarie: p.cle.clone(),
        nom: p.nom.clone(),
        valeur: ValeurDeclaree::Matricule(valeur),
        sort,
        phrase,
    };
    // 1. Les fiches qui existent et que la DSN qui arrive declare sous un autre matricule : celles qui le prennent
    // (`candidats`), et celles qui gardent le leur, et le disent.
    let mut candidats: Vec<(usize, Matricule)> = Vec::new();
    for (cle, declare) in &ap.matricules_declares {
        let Some(i) = porteurs.iter().position(|p| p.cle == *cle) else { continue };
        let p = &porteurs[i];
        let Some(avant) = p.actuel.clone() else { continue };
        if meme(&avant, declare) || !arrivent.contains(cle) {
            continue;
        }
        let valeur = ValeurDuMatricule::Declare { avant: avant.clone(), declaree: declare.clone() };
        if p.parti {
            dits.push(dit(p, valeur, SortDeLaValeur::DejaDeclaree, socle::phrase!(
                "{} : la DSN reprise déclare le matricule {}, la fiche porte le {} ; une DSN partie d'ici l'a déjà déclaré : rien n'est remplacé.",
                p.nom, declare, avant
            )));
        } else if !p.libre {
            dits.push(dit(p, valeur, SortDeLaValeur::DiteParLaCompagnie, socle::phrase!(
                "{} : la DSN reprise déclare le matricule {}, la fiche porte le {}, que la compagnie a dit : il reste. Changez-le dans sa fiche si c'est celui de la DSN.",
                p.nom, declare, avant
            )));
        } else {
            candidats.push((i, declare.clone()));
        }
    }
    // 2. Un candidat dont le matricule declare reste a une fiche qui ne le cede pas (dite par la compagnie, ou declaree
    // d'ici) garde le sien ; ce qu'un candidat garde peut en retenir un autre : jusqu'a ce que rien ne bouge.
    let final_de = |candidats: &[(usize, Matricule)], j: usize| -> Option<Matricule> {
        match candidats.iter().find(|(k, _)| *k == j) {
            Some((_, m)) => Some(m.clone()),
            None => porteurs[j].actuel.clone(),
        }
    };
    loop {
        let bloque = candidats.iter().position(|(i, m)| {
            (0..porteurs.len()).any(|j| j != *i && !porteurs[j].libre && final_de(&candidats, j).is_some_and(|f| meme(&f, m)))
        });
        let Some(k) = bloque else { break };
        let (i, m) = candidats.remove(k);
        let p = &porteurs[i];
        let qui = porteurs.iter().enumerate().find(|(j, q)| *j != i && !q.libre && final_de(&candidats, *j).is_some_and(|f| meme(&f, &m))).map(|(_, q)| q.nom.clone());
        let avant = p.actuel.clone().ok_or_else(|| socle::fichiers::absent("code_salarie"))?;
        dits.push(dit(p, ValeurDuMatricule::Declare { avant: avant.clone(), declaree: m.clone() }, SortDeLaValeur::DejaPorte, socle::phrase!(
            "{} : la DSN reprise déclare le matricule {}, qui est celui de {} ; la fiche garde le {}. Pour lui donner celui de la DSN, changez d'abord l'autre fiche, puis la sienne.",
            p.nom,
            m,
            match qui {
                Some(q) => q,
                None => "une autre fiche".to_string(),
            },
            avant
        )));
    }
    // 3. Les fiches neuves que la DSN cree avec son matricule.
    let neuves: Vec<(String, Matricule)> = ap
        .a_creer
        .iter()
        .filter_map(|a| match &a.donnees {
            FicheProposee::Salarie(s) => s.code_salarie.clone().map(|m| (match s.nomme() {
                Some(n) => n,
                None => "Un salarié de la DSN".to_string(),
            }, m)),
            FicheProposee::Entreprise(_) | FicheProposee::Spectacle(_) => None,
        })
        .collect();
    let mut voulus: Vec<(String, Matricule)> = candidats.iter().map(|(i, m)| (porteurs[*i].nom.clone(), m.clone())).collect();
    voulus.extend(neuves.iter().cloned());
    for (k, (qui, m)) in voulus.iter().enumerate() {
        if let Some((autre, _)) = voulus.iter().skip(k + 1).find(|(_, n)| meme(n, m)) {
            return Err(erreur(Classe::Refuse, format!(
                "Les DSN reprises déclarent le même matricule {} pour {} et pour {} : un matricule désigne un salarié et un seul. Rien n'a été importé.",
                m, qui, autre
            )));
        }
    }
    for (qui, m) in &neuves {
        let fixe = (0..porteurs.len()).find(|&j| !porteurs[j].libre && final_de(&candidats, j).is_some_and(|f| meme(&f, m)));
        if let Some(j) = fixe {
            let q = &porteurs[j];
            return Err(erreur(Classe::Refuse, format!(
                "{} : la DSN reprise déclare le matricule {}, qui est celui de {} (fiche {}), {}. Donnez un autre matricule à {} dans sa fiche, puis relancez l'import. Rien n'a été importé.",
                qui,
                m,
                q.nom,
                q.cle,
                if q.parti { "déjà déclaré d'ici" } else { "que la compagnie a dit" },
                q.nom
            )));
        }
    }
    // 4. Les matricules d'office que la DSN declare pour un autre salarie se cedent : la fiche prend le suivant libre,
    // d'office encore, plus grand que tout matricule que le fichier a porte ou que l'import pose.
    let mut changes: Vec<Change> = candidats.iter().map(|(i, m)| Change { salarie: porteurs[*i].cle.clone(), matricule: m.clone(), d_office: false }).collect();
    for (i, m) in &candidats {
        let p = &porteurs[*i];
        if let Some(avant) = p.actuel.clone() {
            dits.push(dit(p, ValeurDuMatricule::Declare { avant: avant.clone(), declaree: m.clone() }, SortDeLaValeur::Remplacee, socle::phrase!(
                "{} : le matricule {} lui avait été donné d'office ; la DSN reprise déclare le {}, qui le remplace.",
                p.nom, avant, m
            )));
        }
    }
    let mut pris: Vec<Matricule> = fichier.fiches.values().flat_map(|s| s.code_salarie.iter().flatten()).filter_map(|e| e.valeur.clone()).collect();
    pris.extend(voulus.iter().map(|(_, m)| m.clone()));
    for (j, p) in porteurs.iter().enumerate() {
        if !p.libre || candidats.iter().any(|(i, _)| *i == j) {
            continue;
        }
        let Some(avant) = p.actuel.clone() else { continue };
        let Some((pour, _)) = voulus.iter().find(|(_, m)| meme(m, &avant)) else { continue };
        let suivant = Matricule::suivant(pris.iter().map(|m| &**m))?;
        pris.push(suivant.clone());
        dits.push(dit(p, ValeurDuMatricule::Cede { avant: avant.clone(), suivant: suivant.clone(), pour: pour.clone() }, SortDeLaValeur::Remplacee, socle::phrase!(
            "{} : le matricule {} lui avait été donné d'office ; la DSN reprise le déclare pour {} : il prend le suivant, {}.",
            p.nom, avant, pour, suivant
        )));
        changes.push(Change { salarie: p.cle.clone(), matricule: suivant, d_office: true });
    }
    Ok(Matricules { changes, dits })
}

impl Matricules {
    /// L'import change-t-il un matricule ?
    pub(crate) fn ecrit(&self) -> bool {
        !self.changes.is_empty()
    }

    /// LES MATRICULES S'ECRIVENT EN UNE FOIS, de tout temps (`droit::fiche::dater`, sans `depuis`) : rien ne les a
    /// declares. Celui que cede une fiche est d'office encore ; celui que la DSN declare ne l'est pas.
    pub(crate) fn ecrire(&self, ctx: &Contexte, note: &str) -> R<()> {
        if self.changes.is_empty() {
            return Ok(());
        }
        let mut fichier = crate::fiches::lire_fichier::<FicheSalarieDatee>(ctx)?;
        for c in &self.changes {
            let brute = fichier.fiches.get_mut(&c.salarie).ok_or_else(|| erreur(Classe::Refuse, format!("La fiche {} n'existe plus : rien n'est importé.", c.salarie)))?;
            brute.changer(ChangementSalarie::CodeSalarie(Some(c.matricule.clone())), None, false, Some(note));
            if c.d_office {
                brute.changer(ChangementSalarie::MatriculeDOffice(Some(c.matricule.clone())), None, false, Some(note));
            }
        }
        crate::fiches::ecrire_fichier::<FicheSalarieDatee>(ctx, &fichier)
    }
}
