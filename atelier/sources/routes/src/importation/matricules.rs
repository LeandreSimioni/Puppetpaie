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
//! le numero dit le matricule de son salarie, et il ne se recalcule pas, regle 4 ; `former` ne redonne jamais un numero
//! pris) ; un matricule qui n'a pas ete donne d'office (saisi, lu d'une piece, ou deja
//! remplace par une DSN) ; un matricule declare qu'une de ces fiches porte ; un matricule que les pieces reprises donnent
//! a plus d'un salarie (un matricule designe un salarie et un seul : aucun ne le prend).
//!
//! L'ECHANGE : les matricules declares peuvent permuter ceux d'office (MARIVAUX 4 → 2 pendant que MOLIERE 2 → 4). Tout se
//! decide sur l'etat final, avant d'ecrire, et s'ecrit en une fois (`ecrire`) : aucun etat intermediaire ne bute sur un
//! doublon. Un matricule d'office que la DSN declare pour un autre salarie (une fiche qui le prend, ou une fiche neuve) se
//! cede : sa fiche prend le suivant, d'office encore. Le matricule qu'imprime le bulletin d'une fiche neuve vaut comme
//! celui de la DSN (c'est le meme logiciel qui parle) ; chaque phrase nomme la piece (`Source`). Une fiche a un seul
//! sort, et sa ligne le dit. Le matricule de chaque fiche neuve se decide ici aussi : le sien s'il se pose, sinon le
//! suivant, d'office, qu'aucune autre fiche ne prend ; celle dont le matricule ne se pose pas le dit. Une fiche sans
//! matricule qu'une piece complete passe par la meme decision. Rien ne se refuse en bloc (regle 1).
//!
//! Comme pour les contrats (`nature::la_dsn_fait_foi`), la DSN remplace a l'import ou elle arrive : seule une fiche dont
//! une paie declaree arrive avec cet import prend le matricule declare ; un import rejoue n'en apporte aucune.

use std::collections::HashSet;

use droit::contexte::{Contexte, IdSalarie};
use droit::fiche::{ChangementSalarie, FicheDatee as _, FicheSalarieDatee, Matricule, Nir};
use engagement::departs::{Departs, Envoi};
use engagement::numeros::NumeroContrat;
use import::proposee::FicheProposee;
use import::reponse::{SortDeLaValeur, Source, ValeurDeLaDsn, ValeurDeclaree, ValeurDuMatricule};
use socle::echec::{erreur, Classe, R};

/// UN MATRICULE QUE L'IMPORT ECRIT dans une fiche qui existe : le declare, ou le suivant que prend une fiche qui cede le
/// sien (`d_office` : il l'est encore).
struct Change {
    salarie: IdSalarie,
    matricule: Matricule,
    d_office: bool,
}

/// LE MATRICULE D'UNE FICHE NEUVE, decide sur l'etat final : celui que ses pieces lui donnent, ou le suivant, d'office
/// (`d_office`). La fiche se reconnait a son NIR (`ACreer::nir`), dans l'ordre des fiches a creer.
struct Neuve {
    nir: Option<Nir>,
    matricule: Matricule,
    d_office: bool,
}

/// CE QUE L'IMPORT FAIT DES MATRICULES (`prevoir`) : ce qu'il ecrit dans les fiches qui existent (`ecrire`), le matricule
/// de chaque fiche neuve (`poser`), et ce qu'il en dit (`dire`).
pub(crate) struct Matricules {
    changes: Vec<Change>,
    neuves: Vec<Neuve>,
    /// Les fiches qui existent sans matricule, a qui une piece en donne un qui ne se pose pas : la piece ne les complete
    /// pas de lui.
    restent_sans: Vec<IdSalarie>,
    /// Ce que l'import dit de chaque fiche qui existe dont la DSN declare autrement le matricule.
    dits: Vec<ValeurDeLaDsn>,
    /// La piece qui donne a un autre salarie chaque matricule cede.
    cedes_par: Vec<Source>,
    /// Ce qu'il dit des fiches neuves dont le matricule ne se pose pas (`neuves_ecartees`), puis des fiches sans
    /// matricule qui le restent.
    avertissements: Vec<String>,
    neuves_ecartees: usize,
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
    /// Chaque matricule que la fiche a porte, de tout temps : une fiche fixe les garde tous (un numero de contrat forme
    /// avec l'un d'eux reste le sien, `engagement::numeros::former`).
    historique: Vec<Matricule>,
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

/// QUI DEMANDE UN MATRICULE que la DSN qui arrive (ou une piece, pour une fiche neuve ou une fiche sans matricule)
/// declare.
enum Qui {
    /// Une fiche qui existe (son rang parmi les porteurs), dont le matricule est libre.
    Fiche(usize),
    /// Une fiche qui existe sans matricule (son rang parmi les porteurs) : la piece la complete.
    SansMatricule(usize),
    /// Une fiche neuve : son rang parmi les fiches a creer de l'apercu, et le nom que l'apercu lui donne.
    Neuve(usize, String),
}

/// Une demande, la piece qui donne le matricule, et ce qui l'empeche.
struct Demande {
    qui: Qui,
    matricule: Matricule,
    piece: Source,
    empeche: Option<Empeche>,
}

/// La piece `s` qui declare (ou imprime) un matricule, en tete d'une phrase : « la DSN reprise déclare ».
fn declare(s: Source) -> String {
    match s {
        Source::Dsn | Source::Aem => format!("{} déclare", s.reprise()),
        Source::Bulletin | Source::Certificat => format!("{} imprime", s.reprise()),
    }
}

/// La meme, d'un matricule deja nomme : « la DSN reprise le déclare », « le bulletin repris l'imprime ».
fn le_declare(s: Source) -> String {
    match s {
        Source::Dsn | Source::Aem => format!("{} le déclare", s.reprise()),
        Source::Bulletin | Source::Certificat => format!("{} l'imprime", s.reprise()),
    }
}

/// CE QUI EMPECHE UNE DEMANDE.
enum Empeche {
    /// Une fiche fixe porte ce matricule (son rang ; `true`), ou l'a porte (`false`).
    Porte(usize, bool),
    /// D'autres le demandent aussi (leurs noms).
    EnDouble(Vec<String>),
}

impl Demande {
    fn nom(&self, porteurs: &[Porteur]) -> String {
        match &self.qui {
            Qui::Fiche(i) | Qui::SansMatricule(i) => porteurs[*i].nom.clone(),
            Qui::Neuve(_, n) => n.clone(),
        }
    }
}

impl Empeche {
    /// Ce qui empeche, en fin de phrase : « qui est celui de X, … », « que d'autres pieces reprises donnent aussi a X ».
    fn dit(&self, porteurs: &[Porteur]) -> String {
        match self {
            Empeche::Porte(j, true) => format!("qui est celui de {}, {}", porteurs[*j].nom, porteurs[*j].pourquoi()),
            Empeche::Porte(j, false) => format!("qui a été celui de {}, {}", porteurs[*j].nom, porteurs[*j].pourquoi()),
            Empeche::EnDouble(autres) => format!(
                "que d'autres pièces reprises donnent aussi à {} : un matricule désigne un salarié et un seul, aucun ne le prend",
                autres.join(" et ")
            ),
        }
    }

    /// Le sort du matricule d'une fiche qui garde le sien.
    fn sort(&self) -> SortDeLaValeur {
        match self {
            Empeche::Porte(..) => SortDeLaValeur::DejaPorte,
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
                historique: f.code_salarie.iter().flatten().filter_map(|e| e.valeur.clone()).collect(),
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
    // L'apercu ne rattache une paie qu'a une fiche du fichier (par son NIR) : chacune y a son porteur.
    let rang_de = |cle: &IdSalarie| -> R<usize> {
        porteurs.iter().position(|p| p.cle == *cle).ok_or_else(|| {
            erreur(Classe::DefautDuLogiciel, format!("Défaut du logiciel : l'aperçu de l'import nomme la fiche {}, absente du fichier des salariés.", cle))
        })
    };
    for (cle, declare) in &ap.matricules_declares {
        let i = rang_de(cle)?;
        let p = &porteurs[i];
        // Une fiche sans matricule : la DSN la complete, et sa demande vient de ce qu'elle complete (2.).
        let Some(avant) = p.actuel.clone().filter(|a| !a.trim().is_empty()) else { continue };
        if meme(&avant, declare) || !arrivent.contains(cle) {
            continue;
        }
        let valeur = ValeurDuMatricule::Declare { avant: avant.clone(), declaree: declare.clone() };
        match &p.etat {
            Etat::Libre => demandes.push(Demande { qui: Qui::Fiche(i), matricule: declare.clone(), piece: Source::Dsn, empeche: None }),
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
    // 2. Les fiches qui existent sans matricule, qu'une piece complete d'un matricule (la piece que dit la reponse de
    // cette case), puis les fiches neuves que les pieces creent avec un matricule, nommees comme l'apercu les nomme.
    for a in &ap.a_completer {
        let (droit::fiche::FicheNommee::Salarie(cle), FicheProposee::Salarie(s)) = (&a.fiche, &a.champs) else { continue };
        let Some(m) = s.code_salarie.clone().filter(|m| !m.trim().is_empty()) else { continue };
        let piece = ap
            .reponse
            .salaries
            .iter()
            .filter(|f| matches!(f.fiche.nommee(), Some(droit::fiche::FicheNommee::Salarie(c)) if c == cle))
            .flat_map(|f| &f.completer)
            .find(|c| c.champ == droit::fiche::CaseSalarie::CodeSalarie.into())
            .map(|c| c.origine.source)
            .ok_or_else(|| erreur(Classe::DefautDuLogiciel, format!("Défaut du logiciel : l'aperçu de l'import complète le matricule de {} sans dire la pièce qui le donne.", cle)))?;
        demandes.push(Demande { qui: Qui::SansMatricule(rang_de(cle)?), matricule: m, piece, empeche: None });
    }
    for (k, a) in ap.a_creer.iter().enumerate() {
        let FicheProposee::Salarie(s) = &a.donnees else { continue };
        let Some(m) = s.code_salarie.clone().filter(|m| !m.trim().is_empty()) else { continue };
        let piece = *ap.matricules_lus_dans.get(&a.nir().cloned()).ok_or_else(|| {
            erreur(Classe::DefautDuLogiciel, "Défaut du logiciel : l'aperçu de l'import donne un matricule à une fiche neuve sans dire la pièce qui le donne.")
        })?;
        demandes.push(Demande { qui: Qui::Neuve(k, nom_de_la_neuve(ap, a)?), matricule: m, piece, empeche: None });
    }
    // 3. Ce qui empeche une demande : une autre fiche, fixe, qui porte ou a porte le matricule, ou une autre demande du
    // meme.
    let noms: Vec<String> = demandes.iter().map(|d| d.nom(&porteurs)).collect();
    for k in 0..demandes.len() {
        let m = &demandes[k].matricule;
        let soi = match demandes[k].qui {
            Qui::Fiche(i) | Qui::SansMatricule(i) => Some(i),
            Qui::Neuve(..) => None,
        };
        let autres: Vec<String> = (0..demandes.len()).filter(|&l| l != k && meme(&demandes[l].matricule, m)).map(|l| noms[l].clone()).collect();
        let porte = (0..porteurs.len()).find(|&j| Some(j) != soi && porteurs[j].fixe() && porteurs[j].historique.iter().any(|a| meme(a, m)));
        demandes[k].empeche = match porte {
            Some(j) => Some(Empeche::Porte(j, porteurs[j].actuel.as_ref().is_some_and(|a| meme(a, m)))),
            None if !autres.is_empty() => Some(Empeche::EnDouble(autres)),
            None => None,
        };
    }
    // 4. Les matricules que prennent les demandes que rien n'empeche ; une fiche libre dont le sien est l'un d'eux, et
    // qui n'en prend pas un autre, le cede.
    let accordes: Vec<(&Demande, String)> = demandes.iter().filter(|d| d.empeche.is_none()).map(|d| (d, d.nom(&porteurs))).collect();
    let mut changes: Vec<Change> = Vec::new();
    let mut cedes_par: Vec<Source> = Vec::new();
    let mut pris: Vec<Matricule> = fichier.fiches.values().flat_map(|s| s.code_salarie.iter().flatten()).filter_map(|e| e.valeur.clone()).collect();
    pris.extend(demandes.iter().map(|d| d.matricule.clone()));
    for (i, p) in porteurs.iter().enumerate() {
        if p.fixe() {
            continue;
        }
        // Une fiche libre a un matricule (le sien, donne d'office : `d_office`).
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
        let pour = accordes.iter().find(|(d, _)| meme(&d.matricule, &avant)).map(|(d, n)| (d.piece, n.clone()));
        match (pour, empechee) {
            (Some((piece, pour)), _) => {
                let suivant = Matricule::suivant(pris.iter().map(|m| &**m))?;
                pris.push(suivant.clone());
                let phrase = match empechee {
                    Some((d, e)) => socle::phrase!(
                        "{} : {} le matricule {}, {} ; le {}, qui lui avait été donné d'office, {} pour {} : il prend le suivant, {}.",
                        p.nom, declare(d.piece), d.matricule, e.dit(&porteurs), avant, le_declare(piece), pour, suivant
                    ),
                    None => socle::phrase!(
                        "{} : le matricule {} lui avait été donné d'office ; {} pour {} : il prend le suivant, {}.",
                        p.nom, avant, le_declare(piece), pour, suivant
                    ),
                };
                dits.push(dit(p, ValeurDuMatricule::Cede { avant: avant.clone(), suivant: suivant.clone(), pour }, SortDeLaValeur::Remplacee, phrase));
                cedes_par.push(piece);
                changes.push(Change { salarie: p.cle.clone(), matricule: suivant, d_office: true });
            }
            (None, Some((d, e))) => dits.push(dit(p, ValeurDuMatricule::Declare { avant: avant.clone(), declaree: d.matricule.clone() }, e.sort(), socle::phrase!(
                "{} : {} le matricule {}, {} ; la fiche garde le {}.",
                p.nom, declare(d.piece), d.matricule, e.dit(&porteurs), avant
            ))),
            (None, None) => {}
        }
    }
    // Une fiche sans matricule dont la demande est empechee le reste : la piece ne la complete pas de lui, et l'import
    // le dit. Celle dont la demande est accordee se complete (`importation::completer`), apres les fiches neuves.
    let mut avertissements = Vec::new();
    let mut restent_sans = Vec::new();
    for d in &demandes {
        let (Qui::SansMatricule(i), Some(e)) = (&d.qui, &d.empeche) else { continue };
        let p = &porteurs[*i];
        restent_sans.push(p.cle.clone());
        avertissements.push(format!(
            "{} : {} le matricule {}, {} ; la fiche reste sans matricule. Donnez-lui le sien dans sa fiche.",
            p.nom, declare(d.piece), d.matricule, e.dit(&porteurs)
        ));
    }
    // 5. Le matricule de chaque fiche neuve, dans l'ordre ou elles se creent : celui que ses pieces lui donnent, s'il se
    // pose ; sinon le suivant de ce que portent alors les fiches (celles qui existent, leurs matricules ecrits, les
    // fiches neuves d'avant elle), d'office, que ne prend aucune demande accordee. Une fiche neuve dont le matricule ne
    // se pose pas le dit, avec celui qu'elle recoit.
    let mut portes: Vec<Matricule> = porteurs
        .iter()
        .flat_map(|p| match changes.iter().find(|c| c.salarie == p.cle) {
            Some(c) => vec![c.matricule.clone()],
            None => p.historique.clone(),
        })
        .collect();
    let reserves: Vec<&Matricule> = accordes.iter().map(|(d, _)| &d.matricule).collect();
    let mut neuves: Vec<Neuve> = Vec::new();
    let mut ecartees: Vec<String> = Vec::new();
    for (k, a) in ap.a_creer.iter().enumerate() {
        if !matches!(a.donnees, FicheProposee::Salarie(_)) {
            continue;
        }
        let demande = demandes.iter().find(|d| matches!(d.qui, Qui::Neuve(r, _) if r == k));
        let (matricule, d_office) = match demande {
            Some(d) if d.empeche.is_none() => (d.matricule.clone(), false),
            _ => {
                let mut sautes: Vec<Matricule> = Vec::new();
                let donne = loop {
                    let m = Matricule::suivant(portes.iter().chain(&sautes).map(|m| &**m))?;
                    if !reserves.iter().any(|r| meme(r, &m)) {
                        break m;
                    }
                    sautes.push(m);
                };
                if let Some(Demande { qui: Qui::Neuve(_, nom), matricule, piece, empeche: Some(e) }) = demande {
                    ecartees.push(format!(
                        "{} : {} le matricule {}, {} ; sa fiche reçoit le {}, d'office. Changez-le dans sa fiche si vous savez lequel est le sien.",
                        nom, declare(*piece), matricule, e.dit(&porteurs), donne
                    ));
                }
                (donne, true)
            }
        };
        portes.push(matricule.clone());
        neuves.push(Neuve { nir: a.nir().cloned(), matricule, d_office });
    }
    let neuves_ecartees = ecartees.len();
    ecartees.extend(avertissements);
    Ok(Matricules { changes, neuves, restent_sans, dits, cedes_par, avertissements: ecartees, neuves_ecartees })
}

/// Le nom que l'apercu `ap` donne a la fiche neuve `a` d'un salarie : toute fiche de salarie a creer y a sa place.
fn nom_de_la_neuve(ap: &crate::import::Apercu, a: &crate::import::ACreer) -> R<String> {
    a.rang.and_then(|i| ap.reponse.salaries.get(i)).map(|f| f.libelle.clone()).ok_or_else(|| {
        erreur(Classe::DefautDuLogiciel, "Défaut du logiciel : une fiche de salarié à créer n'a pas sa place dans l'aperçu de l'import.")
    })
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

    /// LE MATRICULE DE CHAQUE FICHE NEUVE, tel que `prevoir` l'a decide, pose dans sa fiche a creer : rend, par fiche a
    /// creer, celui qu'elle recoit d'office (que sa fiche datee garde, `matricule_d_office`). Chaque fiche neuve d'un
    /// salarie a sa decision, reconnue a son NIR dans l'ordre : `prevoir` a lu les memes pieces. Une fiche sans matricule
    /// qui le reste ne se complete pas du matricule qui ne se pose pas (`a_completer`).
    pub(crate) fn poser(&self, a_creer: &mut [crate::import::ACreer], a_completer: &mut [crate::importation::completer::ACompleter]) -> R<Vec<Option<Matricule>>> {
        for a in a_completer.iter_mut() {
            if let (droit::fiche::FicheNommee::Salarie(cle), FicheProposee::Salarie(s)) = (&a.fiche, &mut a.champs) {
                if self.restent_sans.contains(cle) {
                    s.code_salarie = None;
                }
            }
        }
        let mut vues = vec![false; self.neuves.len()];
        let mut d_office = Vec::with_capacity(a_creer.len());
        for a in a_creer.iter_mut() {
            let nir = a.nir().cloned();
            let FicheProposee::Salarie(s) = &mut a.donnees else {
                d_office.push(None);
                continue;
            };
            let k = (0..self.neuves.len()).find(|&k| !vues[k] && self.neuves[k].nir == nir).ok_or_else(|| {
                erreur(Classe::DefautDuLogiciel, "Défaut du logiciel : l'import crée une fiche de salarié dont il n'a pas décidé le matricule.")
            })?;
            vues[k] = true;
            let n = &self.neuves[k];
            s.code_salarie = Some(n.matricule.clone());
            d_office.push(n.d_office.then(|| n.matricule.clone()));
        }
        Ok(d_office)
    }

    /// CE QUE LA REPONSE MONTRE DES MATRICULES, ce que l'import fait (ou ferait, l'apercu) : chaque fiche neuve avec le
    /// matricule qu'elle recoit (`a_creer`, apres `poser`), sans l'ecart du matricule que la fiche prend de la DSN, sans le
    /// matricule dont une fiche qui reste sans matricule ne se complete pas, chaque ligne (`dsn_fait_foi`) et ce qui se
    /// dit des fiches neuves et des fiches sans matricule.
    pub(crate) fn montrer(&self, reponse: &mut import::reponse::Importation, a_creer: &[crate::import::ACreer]) {
        for a in a_creer {
            let (FicheProposee::Salarie(posee), Some(i)) = (&a.donnees, a.rang) else { continue };
            if let Some(FicheProposee::Salarie(s)) = reponse.salaries.get_mut(i).map(|f| &mut f.donnees) {
                s.code_salarie = posee.code_salarie.clone();
            }
        }
        let pris: Vec<&IdSalarie> = self.pris_de_la_dsn().collect();
        let matricule: droit::fiche::CaseDeFiche = droit::fiche::CaseSalarie::CodeSalarie.into();
        for f in &mut reponse.salaries {
            let Some(droit::fiche::FicheNommee::Salarie(cle)) = f.fiche.nommee().cloned() else { continue };
            if pris.contains(&&cle) {
                f.ecarts.retain(|x| x.champ != matricule);
            }
            if self.restent_sans.contains(&cle) {
                f.completer.retain(|c| c.champ != matricule);
            }
        }
        reponse.dsn_fait_foi.extend(self.dits.iter().cloned());
        reponse.avertissements.extend(self.avertissements.iter().cloned());
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
            // La piece qui donne a un autre salarie les matricules cedes : une seule, nommee ; plusieurs, « les pieces ».
            let (sujet, donne) = match self.cedes_par.split_first() {
                Some((p, autres)) if autres.iter().all(|a| a == p) => (p.reprise(), "donne"),
                _ => ("les pièces reprises", "donnent"),
            };
            out.push_str(&match (cedes, fait) {
                (1, true) => format!(" Un matricule donné d'office passe à un autre salarié, à qui {} le {} : la fiche qui l'avait prend le suivant.", sujet, donne),
                (1, false) => format!(" Un matricule donné d'office passerait à un autre salarié, à qui {} le {} : la fiche qui l'avait prendrait le suivant.", sujet, donne),
                (n, true) => format!(" {} matricules donnés d'office passent à d'autres salariés, à qui {} les {} : les fiches qui les avaient prennent le suivant.", n, sujet, donne),
                (n, false) => format!(" {} matricules donnés d'office passeraient à d'autres salariés, à qui {} les {} : les fiches qui les avaient prendraient le suivant.", n, sujet, donne),
            });
        }
        if gardes > 0 {
            out.push_str(&format!(
                " La DSN reprise déclare autrement {} : la fiche garde le sien, la liste dit pourquoi.",
                socle::ecran::compte(gardes, "matricule", "matricules")
            ));
        }
        if self.neuves_ecartees > 0 {
            out.push_str(&format!(
                " {} d'office : la liste dit pourquoi.",
                socle::ecran::compte(self.neuves_ecartees, if fait { "fiche neuve a reçu son matricule" } else { "fiche neuve recevrait son matricule" }, if fait { "fiches neuves ont reçu leur matricule" } else { "fiches neuves recevraient leur matricule" })
            ));
        }
        if !self.restent_sans.is_empty() {
            out.push_str(&format!(
                " {} sans matricule : la liste dit pourquoi.",
                socle::ecran::compte(self.restent_sans.len(), if fait { "fiche reste" } else { "fiche resterait" }, if fait { "fiches restent" } else { "fiches resteraient" })
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
