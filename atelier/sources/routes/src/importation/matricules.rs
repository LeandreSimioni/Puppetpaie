//! LE MATRICULE QUE DECLARE UNE DSN QUI ARRIVE (S21.G00.30.019, « Le choix de la valeur que prend le matricule revient
//! a l'employeur » : `norme-dsn/texte-2027/bloc-S21.G00.30.txt`). Le moteur donne d'office le suivant a une fiche neuve
//! dont personne n'a dit le matricule (`fiches::ecriture`, `completer_neuve`), et la fiche datee le garde comme tel (sa
//! memoire `matricule_d_office`, hors de ses cases). Quand une DSN reprise arrive ensuite et declare un autre matricule
//! pour ce salarie, la DSN fait foi (Léandre, 18/11/2026 : « oui », a « le matricule que declare une DSN qui arrive
//! remplace-t-il celui attribue d'office, tant que rien n'est parti d'ici ? ») : la fiche prend le matricule declare, de
//! tout temps (un matricule que rien n'a declare n'a rien dit au passe). L'import en deux fois (les bulletins, puis la
//! DSN) amene ainsi aux matricules de l'import en une fois, ou la DSN cree les fiches avec ses matricules.
//!
//! CE QUI NE SE REMPLACE PAS, et se dit (`SortDeLaValeur`) : le matricule que ce qui est fait d'ici a engage (`Engage`) :
//! celui qu'une DSN ou un signalement partis d'ici ont declare (ils l'ecrivent en S21.G00.30.019 : `declarations::dsn`,
//! `declarations::signalement` ; la DPAE n'en porte pas), lu dans le journal (`engagement::departs`), et celui avec
//! lequel le moteur a forme un numero de contrat (`engagement::numeros::formes` : `<initiales><matricule>.<AA>.<rang>`,
//! le rang compte par salarie ; le remplacer donnerait les memes numeros a un autre salarie aux memes initiales, et un
//! numero ne se recalcule pas, regle 4) ; un matricule qui n'a pas ete donne d'office (saisi, lu d'une piece, ou deja
//! remplace par une DSN) ; un matricule declare qu'une de ces fiches porte ; un matricule que les pieces reprises donnent
//! a plus d'un salarie (un matricule designe un salarie et un seul : aucun ne le prend).
//!
//! L'ECHANGE : les matricules declares peuvent permuter ceux d'office (MARIVAUX 4 → 2 pendant que MOLIERE 2 → 4). Tout se
//! decide sur l'etat final, avant d'ecrire, et s'ecrit en une fois (`ecrire`) : aucun etat intermediaire ne bute sur un
//! doublon. Un matricule d'office que la DSN declare pour un autre salarie (une fiche qui le prend, ou une fiche neuve) se
//! cede : sa fiche prend le suivant, d'office encore. Une fiche a un seul sort, et sa ligne le dit. Une fiche neuve dont
//! le matricule ne se pose pas le reçoit d'office, et l'import le dit : rien ne se refuse en bloc (regle 1).
//!
//! Comme pour les contrats (`nature::la_dsn_fait_foi`), la DSN remplace a l'import ou elle arrive : seule une fiche dont
//! une paie declaree arrive avec cet import prend le matricule declare ; un import rejoue n'en apporte aucune.

use std::collections::HashSet;

use droit::contexte::{Contexte, IdSalarie};
use droit::fiche::{ChangementSalarie, FicheDatee as _, FicheSalarieDatee, Matricule};
use engagement::departs::{Departs, Envoi};
use engagement::numeros::NumeroContrat;
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

/// CE QUE L'IMPORT FAIT DES MATRICULES (`prevoir`) : ce qu'il ecrit (`ecrire`), les matricules que les fiches neuves ne
/// prennent pas (`poser_les_neuves`), et ce qu'il en dit (`dire`).
pub(crate) struct Matricules {
    changes: Vec<Change>,
    /// Les matricules que des fiches neuves portent et qui ne se posent pas : elles les recoivent d'office.
    ecartes: Vec<Matricule>,
    /// Ce que l'import dit de chaque fiche qui existe dont la DSN declare autrement le matricule.
    pub(crate) dits: Vec<ValeurDeLaDsn>,
    /// Ce qu'il dit des fiches neuves dont le matricule ne se pose pas.
    pub(crate) neuves: Vec<String>,
}

/// CE QUI, FAIT D'ICI, A ENGAGE LE MATRICULE D'UN SALARIE : il ne se remplace ni ne se cede.
enum Engage {
    /// Une DSN ou un signalement partis d'ici l'ont declare.
    Declare,
    /// Le moteur a forme ce numero de contrat avec lui (`engagement::numeros::formes`).
    Numerote(NumeroContrat),
}

/// CE QUE LE MATRICULE D'UNE FICHE QUI EXISTE EST, pour l'import.
enum Etat {
    /// Donne d'office, et rien d'ici ne l'a engage : il se remplace, ou se cede.
    Libre,
    Engage(Engage),
    /// Il n'a pas ete donne d'office : la fiche le garde.
    PasDOffice,
}

/// Une fiche qui existe, telle que l'import la trouve.
struct Porteur {
    cle: IdSalarie,
    nom: String,
    actuel: Option<Matricule>,
    etat: Etat,
}

impl Porteur {
    /// Son matricule reste le sien, quoi que la DSN declare.
    fn fixe(&self) -> bool {
        !matches!(self.etat, Etat::Libre)
    }

    /// Pourquoi il garde son matricule (une fiche fixe), en fin de phrase.
    fn pourquoi(&self) -> String {
        match &self.etat {
            Etat::Engage(Engage::Declare) => "qu'une DSN partie d'ici a déjà déclaré".to_string(),
            Etat::Engage(Engage::Numerote(n)) => format!("avec lequel le contrat {} a été numéroté ici", n),
            Etat::PasDOffice | Etat::Libre => "qui ne lui a pas été donné d'office".to_string(),
        }
    }
}

/// Le matricule donne d'office que la fiche porte encore : sa seule valeur, de tout temps, est celle que le moteur lui
/// a donnee (`matricule_d_office`). Un matricule saisi depuis, ou date, ne l'est plus.
fn d_office(f: &FicheSalarieDatee) -> Option<&Matricule> {
    match (f.code_salarie.as_deref(), f.matricule_d_office.as_ref()) {
        (Some([c]), Some(d)) if c.depuis.is_none() => c.valeur.as_ref().filter(|c| meme(c, d)),
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

/// QUI DEMANDE UN MATRICULE que la DSN qui arrive (ou une piece, pour une fiche neuve) declare.
enum Qui {
    /// Une fiche qui existe (son rang parmi les porteurs), dont le matricule est libre.
    Fiche(usize),
    /// Une fiche neuve, par le nom que l'apercu lui donne.
    Neuve(String),
}

/// Une demande, et ce qui l'empeche.
struct Demande {
    qui: Qui,
    matricule: Matricule,
    empeche: Option<Empeche>,
}

/// CE QUI EMPECHE UNE DEMANDE.
enum Empeche {
    /// Une fiche fixe porte ce matricule (son rang).
    Porte(usize),
    /// D'autres le demandent aussi (leurs noms).
    EnDouble(Vec<String>),
}

impl Demande {
    fn nom(&self, porteurs: &[Porteur]) -> String {
        match &self.qui {
            Qui::Fiche(i) => porteurs[*i].nom.clone(),
            Qui::Neuve(n) => n.clone(),
        }
    }
}

impl Empeche {
    /// Ce qui empeche, en fin de phrase : « qui est celui de X, … », « que d'autres pieces reprises donnent aussi a X ».
    fn dit(&self, porteurs: &[Porteur]) -> String {
        match self {
            Empeche::Porte(j) => format!("qui est celui de {}, {}", porteurs[*j].nom, porteurs[*j].pourquoi()),
            Empeche::EnDouble(autres) => format!(
                "que d'autres pièces reprises donnent aussi à {} : un matricule désigne un salarié et un seul, aucun ne le prend",
                autres.join(" et ")
            ),
        }
    }

    /// Le sort du matricule d'une fiche qui garde le sien.
    fn sort(&self) -> SortDeLaValeur {
        match self {
            Empeche::Porte(_) => SortDeLaValeur::DejaPorte,
            Empeche::EnDouble(_) => SortDeLaValeur::EnDouble,
        }
    }
}

/// CE QUE L'IMPORT `ap` FERA DES MATRICULES, decide sur l'etat final, sans rien ecrire ; `departs` : ce que le journal
/// dit (lu une fois par geste). Ne refuse rien : ce qui ne se pose pas se dit.
pub(crate) fn prevoir(ctx: &Contexte, ap: &crate::import::Apercu, departs: &Departs) -> R<Matricules> {
    let fichier = crate::fiches::lire_fichier::<FicheSalarieDatee>(ctx)?;
    let numerotes = engagement::numeros::formes(&ctx.racine)?;
    let porteurs: Vec<Porteur> = fichier
        .fiches
        .iter()
        .map(|(cle, f)| {
            let au = f.au(&ctx.lue_au);
            let etat = match (declare_d_ici(departs, cle), numerotes.get(cle)) {
                (true, _) => Etat::Engage(Engage::Declare),
                (false, Some(n)) => Etat::Engage(Engage::Numerote(n.clone())),
                (false, None) if d_office(f).is_some() => Etat::Libre,
                (false, None) => Etat::PasDOffice,
            };
            Porteur {
                cle: cle.clone(),
                nom: match au.nomme() {
                    Some(n) => n,
                    None => cle.to_string(),
                },
                actuel: au.code_salarie.clone(),
                etat,
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
    // 1. Les fiches qui existent et que la DSN qui arrive declare sous un autre matricule : une fiche fixe garde le sien
    // et le dit ; une fiche libre le demande.
    let mut demandes: Vec<Demande> = Vec::new();
    for (cle, declare) in &ap.matricules_declares {
        let Some(i) = porteurs.iter().position(|p| p.cle == *cle) else { continue };
        let p = &porteurs[i];
        let Some(avant) = p.actuel.clone() else { continue };
        if meme(&avant, declare) || !arrivent.contains(cle) {
            continue;
        }
        let valeur = ValeurDuMatricule::Declare { avant: avant.clone(), declaree: declare.clone() };
        match &p.etat {
            Etat::Libre => demandes.push(Demande { qui: Qui::Fiche(i), matricule: declare.clone(), empeche: None }),
            Etat::Engage(_) => dits.push(dit(p, valeur, SortDeLaValeur::DejaDeclaree, socle::phrase!(
                "{} : la DSN reprise déclare le matricule {}, la fiche porte le {}, {} : rien n'est remplacé.",
                p.nom, declare, avant, p.pourquoi()
            ))),
            Etat::PasDOffice => dits.push(dit(p, valeur, SortDeLaValeur::PasDOffice, socle::phrase!(
                "{} : la DSN reprise déclare le matricule {}, la fiche porte le {}, {} : il reste. Changez-le dans sa fiche si c'est celui de la DSN.",
                p.nom, declare, avant, p.pourquoi()
            ))),
        }
    }
    // 2. Les fiches neuves que les pieces creent avec un matricule, nommees comme l'apercu les nomme.
    for a in &ap.a_creer {
        let FicheProposee::Salarie(s) = &a.donnees else { continue };
        let Some(m) = s.code_salarie.clone().filter(|m| !m.trim().is_empty()) else { continue };
        let nom = a.rang.and_then(|i| ap.reponse.salaries.get(i)).map(|f| f.libelle.clone()).ok_or_else(|| {
            erreur(Classe::DefautDuLogiciel, "Défaut du logiciel : une fiche de salarié à créer n'a pas sa place dans l'aperçu de l'import.")
        })?;
        demandes.push(Demande { qui: Qui::Neuve(nom), matricule: m, empeche: None });
    }
    // 3. Ce qui empeche une demande : une fiche fixe qui porte le matricule, ou une autre demande du meme.
    let noms: Vec<String> = demandes.iter().map(|d| d.nom(&porteurs)).collect();
    for k in 0..demandes.len() {
        let m = &demandes[k].matricule;
        let autres: Vec<String> = (0..demandes.len()).filter(|&l| l != k && meme(&demandes[l].matricule, m)).map(|l| noms[l].clone()).collect();
        let porte = (0..porteurs.len()).find(|&j| porteurs[j].fixe() && porteurs[j].actuel.as_ref().is_some_and(|a| meme(a, m)));
        demandes[k].empeche = match porte {
            Some(j) => Some(Empeche::Porte(j)),
            None if !autres.is_empty() => Some(Empeche::EnDouble(autres)),
            None => None,
        };
    }
    // 4. Les matricules que prennent les demandes que rien n'empeche ; une fiche libre dont le sien est l'un d'eux, et
    // qui n'en prend pas un autre, le cede.
    let accordes: Vec<(&Demande, String)> = demandes.iter().filter(|d| d.empeche.is_none()).map(|d| (d, d.nom(&porteurs))).collect();
    let mut changes: Vec<Change> = Vec::new();
    let mut pris: Vec<Matricule> = fichier.fiches.values().flat_map(|s| s.code_salarie.iter().flatten()).filter_map(|e| e.valeur.clone()).collect();
    pris.extend(demandes.iter().map(|d| d.matricule.clone()));
    for (i, p) in porteurs.iter().enumerate() {
        if p.fixe() {
            continue;
        }
        let Some(avant) = p.actuel.clone() else { continue };
        // Sa demande, et ce qui l'empeche ; une demande que rien n'empeche prend le matricule declare.
        let empechee = match demandes.iter().find(|d| matches!(d.qui, Qui::Fiche(k) if k == i)) {
            Some(d) => match &d.empeche {
                None => {
                    dits.push(dit(p, ValeurDuMatricule::Declare { avant: avant.clone(), declaree: d.matricule.clone() }, SortDeLaValeur::Remplacee, socle::phrase!(
                        "{} : le matricule {} lui avait été donné d'office ; la DSN reprise déclare le {}, qui le remplace.",
                        p.nom, avant, d.matricule
                    )));
                    changes.push(Change { salarie: p.cle.clone(), matricule: d.matricule.clone(), d_office: false });
                    continue;
                }
                Some(e) => Some((d, e)),
            },
            None => None,
        };
        let pour = accordes.iter().find(|(d, _)| meme(&d.matricule, &avant)).map(|(_, n)| n.clone());
        match (pour, empechee) {
            (Some(pour), _) => {
                let suivant = Matricule::suivant(pris.iter().map(|m| &**m))?;
                pris.push(suivant.clone());
                let phrase = match empechee {
                    Some((d, e)) => socle::phrase!(
                        "{} : la DSN reprise déclare le matricule {}, {} ; le {}, qui lui avait été donné d'office, elle le déclare pour {} : il prend le suivant, {}.",
                        p.nom, d.matricule, e.dit(&porteurs), avant, pour, suivant
                    ),
                    None => socle::phrase!(
                        "{} : le matricule {} lui avait été donné d'office ; la DSN reprise le déclare pour {} : il prend le suivant, {}.",
                        p.nom, avant, pour, suivant
                    ),
                };
                dits.push(dit(p, ValeurDuMatricule::Cede { avant: avant.clone(), suivant: suivant.clone(), pour }, SortDeLaValeur::Remplacee, phrase));
                changes.push(Change { salarie: p.cle.clone(), matricule: suivant, d_office: true });
            }
            (None, Some((d, e))) => dits.push(dit(p, ValeurDuMatricule::Declare { avant: avant.clone(), declaree: d.matricule.clone() }, e.sort(), socle::phrase!(
                "{} : la DSN reprise déclare le matricule {}, {} ; la fiche garde le {}.",
                p.nom, d.matricule, e.dit(&porteurs), avant
            ))),
            (None, None) => {}
        }
    }
    // 5. Les fiches neuves dont le matricule ne se pose pas : elles le recoivent d'office, et l'import le dit.
    let mut ecartes: Vec<Matricule> = Vec::new();
    let mut neuves = Vec::new();
    for d in &demandes {
        let (Qui::Neuve(nom), Some(e)) = (&d.qui, &d.empeche) else { continue };
        if !ecartes.iter().any(|m| meme(m, &d.matricule)) {
            ecartes.push(d.matricule.clone());
        }
        neuves.push(format!(
            "{} : les pièces reprises lui donnent le matricule {}, {} ; sa fiche reçoit le suivant, d'office. Changez-le dans sa fiche si vous savez lequel est le sien.",
            nom, d.matricule, e.dit(&porteurs)
        ));
    }
    Ok(Matricules { changes, ecartes, dits, neuves })
}

impl Matricules {
    /// L'import change-t-il un matricule ?
    pub(crate) fn ecrit(&self) -> bool {
        !self.changes.is_empty()
    }

    /// Les salaries dont la fiche prend le matricule que la DSN declare : l'ecart de leur matricule est leve.
    pub(crate) fn pris_de_la_dsn(&self) -> impl Iterator<Item = &IdSalarie> {
        self.changes.iter().filter(|c| !c.d_office).map(|c| &c.salarie)
    }

    /// LES FICHES NEUVES DONT LE MATRICULE NE SE POSE PAS (`ecartes`) le recoivent d'office (`completer_neuve`) : leur
    /// case reste vide.
    pub(crate) fn poser_les_neuves(&self, a_creer: &mut [crate::import::ACreer]) {
        for a in a_creer {
            if let FicheProposee::Salarie(s) = &mut a.donnees {
                if s.code_salarie.as_ref().is_some_and(|m| self.ecartes.iter().any(|e| meme(e, m))) {
                    s.code_salarie = None;
                }
            }
        }
    }

    /// CE QUE L'IMPORT DIT DES MATRICULES, en phrases qui suivent la sienne (chacune commence par une espace) ; `fait` :
    /// l'import est fait, sinon c'est ce qu'il ferait (l'apercu).
    pub(crate) fn dire(&self, fait: bool) -> String {
        let declares = self.dits.iter().filter(|v| matches!(v.valeur, ValeurDeclaree::Matricule(ValeurDuMatricule::Declare { .. })) && v.sort == SortDeLaValeur::Remplacee).count();
        let cedes = self.dits.iter().filter(|v| matches!(v.valeur, ValeurDeclaree::Matricule(ValeurDuMatricule::Cede { .. }))).count();
        let gardes = self.dits.len() - declares - cedes;
        let mut out = String::new();
        if declares > 0 {
            out.push_str(&format!(
                " La DSN reprise fait foi : {} par ce qu'elle déclare.",
                socle::ecran::compte(declares, if fait { "matricule donné d'office est remplacé" } else { "matricule donné d'office serait remplacé" }, if fait { "matricules donnés d'office sont remplacés" } else { "matricules donnés d'office seraient remplacés" })
            ));
        }
        if cedes > 0 {
            out.push_str(&format!(
                " {} que la DSN reprise déclare pour un autre salarié : la fiche prend le suivant.",
                socle::ecran::compte(cedes, if fait { "matricule donné d'office est cédé" } else { "matricule donné d'office serait cédé" }, if fait { "matricules donnés d'office sont cédés" } else { "matricules donnés d'office seraient cédés" })
            ));
        }
        if gardes > 0 {
            out.push_str(&format!(
                " La DSN reprise déclare autrement {} : la fiche garde le sien, la liste dit pourquoi.",
                socle::ecran::compte(gardes, "matricule", "matricules")
            ));
        }
        if !self.neuves.is_empty() {
            out.push_str(&format!(
                " {} d'office : la liste dit pourquoi.",
                socle::ecran::compte(self.neuves.len(), if fait { "fiche neuve a reçu son matricule" } else { "fiche neuve recevrait son matricule" }, if fait { "fiches neuves ont reçu leur matricule" } else { "fiches neuves recevraient leur matricule" })
            ));
        }
        out
    }

    /// LES MATRICULES S'ECRIVENT EN UNE FOIS, de tout temps (`droit::fiche::dater`, sans `depuis`) : rien d'ici ne les a
    /// engages, ils n'ont rien dit au passe. Ce n'est pas une saisie face a la fiche montree (`fiches::ecrire_datee`, qui
    /// demande une date d'effet a toute valeur qui change) : c'est la decision de l'import, et `FicheDatee::changer` reste
    /// le seul qui ecrit une valeur datee. Celui que cede une fiche est d'office encore ; celui que la DSN declare ne
    /// l'est pas.
    pub(crate) fn ecrire(&self, ctx: &Contexte, note: &str) -> R<()> {
        if self.changes.is_empty() {
            return Ok(());
        }
        let mut fichier = crate::fiches::lire_fichier::<FicheSalarieDatee>(ctx)?;
        for c in &self.changes {
            // `prevoir` a lu le meme fichier dans le meme geste, et rien ne l'ecrit entre les deux.
            let brute = fichier.fiches.get_mut(&c.salarie).ok_or_else(|| {
                erreur(Classe::DefautDuLogiciel, format!("Défaut du logiciel : la fiche {} que l'import a lue n'est plus dans le fichier des salariés.", c.salarie))
            })?;
            brute.changer(ChangementSalarie::CodeSalarie(Some(c.matricule.clone())), None, false, Some(note));
            brute.matricule_d_office = c.d_office.then(|| c.matricule.clone());
        }
        crate::fiches::ecrire_fichier::<FicheSalarieDatee>(ctx, &fichier)
    }
}
