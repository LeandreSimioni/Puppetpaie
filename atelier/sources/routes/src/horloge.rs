//! L'horloge de la compagnie fictive : `rattraper`, `dernier_rattrapage`
//! et tout ce qu'ils appellent.
//!
//! A chaque ouverture, la COMPAGNIE FICTIVE joue ce qu'une compagnie bien tenue aurait fait AVANT aujourd'hui, a sa
//! date, et seulement ce qui n'est pas fait : chaque pas regarde d'abord ce que le moteur en dit (le journal, les etapes
//! du mois, les obligations), et ne fait rien si c'est fait. Elle ne saisit jamais : elle valide, envoie, depose.
//! - Le jour de chaque embauche : ses engagements se valident a 5 h, son contrat part a la signature a 6 h, sa DPAE a 7 h
//!   (test puis reel) avec le signalement d'amorcage, et le contrat revient signe a 8 h. Un contrat parti puis rendu faux
//!   avant d'etre revenu signe repart dans sa version presente.
//! - Un contrat revenu signe que l'engagement a rendu faux part en avenant a l'instant ou l'engagement a ete revalide, et
//!   revient signe.
//! - Les signalements d'arret, de reprise anticipee et de fin de contrat que le moteur dit dus (`obligations`) partent le
//!   jour de leur date butoir, a 9 h 30 (test puis reel). Un signalement que la saisie ne permet pas d'ecrire n'est pas
//!   joue : le pas le dit une fois, les rappels le disent a la personne, et l'horloge ne corrige pas une saisie.
//! - Le 1er du mois suivant, ce qui reste en saisie du mois se valide ; a l'echeance de la DSN, la DSN se fabrique et part
//!   (test puis reel), puis les documents aux salaries, fabriques a leur envoi, et les bulletins rectificatifs d'un mois
//!   dont les documents etaient partis.
//! - Chaque mois que la saisie touche se joue, ceux d'un contrat sans terme compris, et chaque mois dont la DSN est due
//!   sans engagement, qui part sans individu a son echeance (`rappels::mois_a_parcourir`).
//! - Fin janvier, l'annee passee se clot et s'exporte pour la comptabilite. Une echeance annuelle n'est faite que par son
//!   fait (le taux AT/MP pose, la revue des spectacles) : l'horloge n'en coche aucune.
//! Ce qui vient apres aujourd'hui reste au visiteur.
//!
//! L'ORDRE EST CELUI DU CALENDRIER : l'horloge rassemble les pas dus, chacun avec son instant, les trie et les joue du plus
//! ancien au plus recent, de mois en mois sans rien sauter (une embauche du 3 avril se joue avant la DSN de mars du 15).
//!
//! FAIT : un mois dont l'echeance est passee, dont toutes les etapes sont vertes (`etapes_restantes`) et dont aucun
//! signalement n'est a faire, sa date butoir passee ou non, hors celui qui attend la saisie. Il ne se relit plus tant
//! que sa saisie et ses imprevus ne changent pas (`empreinte_du_mois`) et que sa DSN n'est pas refusee.
//!
//! UN REFUS ATTEND SA CORRECTION : une DSN refusee par un bilan laisse son mois a faire ; a chaque ouverture, l'horloge la
//! refait datee de son echeance, et ne redepose jamais les octets qu'un organisme a refuses (`deposer_les_dsn`). Quand une
//! personne a corrige ce qui la faisait refuser, le fichier change, et il part a l'instant ou l'horloge le trouve ; ses
//! documents suivent.
//!
//! ELLE NE JOUE QUE DANS UNE COMPAGNIE FICTIVE, ET LE CODE LE DIT A CHAQUE ENDROIT QUI AGIT.
//! - `rattraper` commence par `HorlogeFictive::verifier` : sans compagnie fictive active (`bridage::branchement` : le paquet
//!   soude, ou une compagnie dont le marqueur dit « compagnie fictive »), elle rend une liste vide et ne
//!   touche a rien -- pas meme `config/horloge.json`.
//! - Tout geste qui agit recoit la `HorlogeFictive` et la RE-VERIFIE avant d'agir (`pas`, `deposer_*_joue`) : un marqueur
//!   change en route arrete le pas, il ne le laisse pas partir vers un vrai point de depot.
//! - Les depots ne passent que par `depot_acte::deposer_dsn` et `deposer_dpae`, qui jouent la reponse `FICTIF-...` quand la
//!   simulation est active et ne s'ouvrent jamais autrement ici (la re-verification les precede) ; aucune preuve, aucun
//!   secret, aucun coffre n'est lu.
//! - Les courriels ne passent que par `BoiteMuette` : un client du trait `Courriel` qui rend un identifiant et ne garde
//!   rien -- ni `courriels-simules/` (qui montre au visiteur ce que SES gestes envoient), ni la vraie boite. Ce module
//!   n'appelle jamais `crate::documents::client_courriel`.
//!
//! Elle ne leve jamais pour un PAS : un pas en echec devient sa phrase (« Le jj/mm/aaaa : ... — non joue (...) ») et les
//! autres continuent. Seuls les refus du decor (une fiche entreprise illisible, une echeance introuvable) remontent.

use std::collections::{BTreeMap, BTreeSet};
use socle::echec::Classe;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::contrats::{AvenantVoulu, ChargeRecu, ChargeSignature, ContratASigner, ContratVise};
use droit::contexte::Contexte;
use envois::courriel::Courriel;
use socle::date::Jour;
use socle::mois::{Annee, Mois};
use socle::paris::Moment;
use compagnie::preuve::Preuve;
use crate::gestes::depot_acte;
use envois::etats;
use crate::enveloppe::{en_json, Charge, Dite, Rendu};
use socle::echec::R;
use socle::echec::erreur;
use socle::echec::Echec;
use crate::etapes::Moteur;
use compagnie::installation::FICHIER_HORLOGE;

/// Qui signe, au journal, ce que l'horloge joue.
pub(crate) const QUI_HORLOGE: &str = "horloge";

const ORIGINE: &str = "compagnie fictive : joue par l'horloge";
const QUOI: &str = "L'horloge de la compagnie fictive : le dernier jour joue, ce qui n'est plus a regarder, et ce qu'elle a fait en dernier (le bandeau le dit).";

// ------------------------------------------------------------------ la garde

/// LA GARDE : la preuve que la racine est une COMPAGNIE FICTIVE, exigee de tout ce qui agit. Elle ne se fabrique
/// que par `verifier`, qui lit `bridage::branchement` ; `reverifier` la relit avant chaque geste.
pub(crate) struct HorlogeFictive<'a> {
    racine: &'a Path,
    /// LE JOUR JOUE : celui que l'horloge rattrape. Chaque contexte qu'elle ouvre porte ce jour (`aujourdhui` ET `date`),
    /// jamais l'horloge reelle de la machine : la cloture d'une annee, l'export, le jour inscrit
    /// a l'echeancier, la DPAE se jugent au jour joue (le contexte de l'interface, lui, garde la date reelle).
    jour: Jour,
}

impl<'a> HorlogeFictive<'a> {
    /// `None` si les envois de cette racine ne sont pas joues.
    pub fn verifier(racine: &'a Path) -> R<Option<HorlogeFictive<'a>>> {
        Ok(compagnie::bridage::branchement(racine)?.joue().then(|| HorlogeFictive { racine, jour: droit::contexte::jour_reel() }))
    }

    /// Le jour que cette compagnie fictive joue.
    fn au_jour(self, jour: Jour) -> HorlogeFictive<'a> {
        HorlogeFictive { jour, ..self }
    }

    /// Un contexte AU JOUR JOUE (`date` et `aujourdhui`).
    fn contexte(&self) -> R<Contexte> {
        self.contexte_a(self.jour)
    }

    /// Un contexte lu a `date` (la fin du mois qu'on ecrit), dont `aujourdhui` reste le jour joue.
    fn contexte_a(&self, date: Jour) -> R<Contexte> {
        let mut ctx = Contexte::nouveau(self.racine, Some(date))?;
        ctx.aujourdhui = self.jour;
        Ok(ctx)
    }

    /// Relit le marqueur : refuse tout geste si la racine a cesse d'etre une compagnie fictive.
    fn reverifier(&self) -> R<()> {
        if compagnie::bridage::branchement(self.racine)?.joue() {
            Ok(())
        } else {
            Err(erreur(Classe::Refuse, "Ce dossier n'est pas une compagnie fictive : l'horloge ne joue rien ici."))
        }
    }
}

/// La boite muette du rattrapage : le passe joue n'a pas a remplir `courriels-simules/`, qui montre au visiteur ce que
/// SES gestes envoient. Elle rend un identifiant et ne garde rien.
struct BoiteMuette {
    compte: String,
}

impl BoiteMuette {
    fn nouvelle() -> BoiteMuette {
        BoiteMuette { compte: "fictive@puppetpaie.invalid".to_string() }
    }
}

impl Courriel for BoiteMuette {
    fn compte(&self) -> &str {
        &self.compte
    }

    fn envoyer(&mut self, _message_mime: &[u8]) -> R<String> {
        Ok(format!("<{}@fictive.puppetpaie>", envois::courriel::hexa_au_hasard(8)?))
    }

    fn rechercher(&mut self, _requete: &str, _maximum: usize) -> R<Vec<String>> {
        Ok(Vec::new())
    }

    fn message_brut(&mut self, _identifiant: &str) -> R<Vec<u8>> {
        Ok(Vec::new())
    }

    fn fermer(&mut self) -> R<()> {
        Ok(())
    }
}

// ------------------------------------------------------------------ petits outils

/// Le mois en lettres : « octobre 2026 ».
fn mois_fr(mois: Mois) -> String {
    mois.libelle(socle::mois::Graphie::Accentuee)
}

/// `jour` a `heure:minute`, a Paris ; une heure que le changement d'heure saute se refuse.
fn a_l_heure(jour: Jour, heure: i64, minute: i64) -> R<Moment> {
    let m = Moment::a_paris(jour, heure, minute, 0).ok_or_else(|| erreur(Classe::Illisible, socle::date::HORS_CALENDRIER))?;
    if m.jour_de_paris() != jour || m.seconde_du_jour_de_paris() != heure * 3_600 + minute * 60 {
        return Err(erreur(Classe::DefautDuLogiciel, format!("L'instant d'un pas de l'horloge n'existe pas a Paris : {} a {:02}:{:02}.", jour.iso(), heure, minute)));
    }
    Ok(m)
}

/// Les refus ordinaires d'une operation (un refus du moteur sans classe en est un).
fn est_une_erreur_d_operation(e: &Echec) -> bool {
    e.classe.is_none()
        || matches!(
        e.classe,
        Some(
            Classe::Refuse
                | Classe::SortieAbsente
                | Classe::AucunDocument
                | Classe::BoiteAbsente
                | Classe::PreuveInsuffisante
                | Classe::SaisieIncomplete
                | Classe::ExerciceClos
        )
    )
}

/// Le message entier, ses lignes mises bout a bout sur une seule, ou le nom de la classe quand le message est vide.
fn motif_de(e: &Echec) -> String {
    let message = e.en_lignes();
    if message.trim().is_empty() {
        return e.classe_notee().to_string();
    }
    message.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ")
}

/// Un pas joue -- le geste, ses lignes de journal datees, et sa phrase. Une erreur devient la phrase, sans
/// arreter les autres pas. `geste` rend ce qu'il a fait (`Joue`).
fn pas(d: &HorlogeFictive, quoi: &str, quand: Moment, mut geste: impl FnMut(&Contexte) -> R<Joue>) -> Vec<String> {
    let racine = d.racine;
    let echec = |e: &Echec| non_joue(quand, quoi, e);
    // LE CONTEXTE DU JOUR DU PAS : ce qu'il ecrit (une date de constitution, un jour inscrit) est celui ou il a lieu.
    let jour_du_pas = quand.jour_de_paris();
    let instant = quand;
    let au_jour = HorlogeFictive { racine, jour: jour_du_pas.min(d.jour) };
    let ctx = match au_jour.contexte_a(au_jour.jour) {
        Ok(c) => c,
        Err(e) => return echec(&e),
    };
    // Le geste a lieu a son heure : ce qu'il ecrit (un nom de fichier, une ligne de journal) en porte la date, et une
    // ligne du journal dit qu'il a ete joue (`origine`).
    // La re-verification precede TOUT geste : ce qui suit ne s'execute que dans une compagnie fictive.
    let jouer = || envois::journal::dater_pendant(quand, ORIGINE, || d.reverifier().and_then(|_| geste(&ctx)));
    let fait = socle::temps::pendant_le_pas_fictif(instant, &format!("{} {}", socle::paris::ecrit::seconde::texte(quand), quoi), jouer);
    match fait {
        Ok(Joue::Fait) => vec![format!("Le {} : {}", quand.jour_de_paris().jj_mm_aaaa(), quoi)],
        Ok(Joue::Rien) => Vec::new(),
        Err(e) => echec(&e),
    }
}

/// La phrase d'un pas que le moteur refuse : son jour, ce qu'il devait faire, et le refus.
fn non_joue(quand: Moment, quoi: &str, e: &Echec) -> Vec<String> {
    vec![format!("Le {} : {} — non joué ({})", quand.jour_de_paris().jj_mm_aaaa(), quoi, motif_de(e))]
}

/// Ce qu'un pas a fait : quelque chose (sa phrase se dit), ou rien, faute de quoi faire (ni phrase, ni date).
enum Joue {
    Fait,
    Rien,
}

impl Joue {
    /// `Fait` si `fait`.
    fn si(fait: bool) -> Joue {
        if fait {
            Joue::Fait
        } else {
            Joue::Rien
        }
    }
}

/// JOUE. La re-verification precede l'appel. Rend ce que ses retours laissent a faire.
fn deposer_dsn_joue(d: &HorlogeFictive, ctx: &Contexte, fichier: &str, reel: bool) -> R<Vec<String>> {
    d.reverifier()?;
    Ok(depot_acte::deposer_dsn(ctx, fichier, reel, None, QUI_HORLOGE, false, "")?.suites)
}

/// JOUE. La re-verification precede l'appel.
fn deposer_dpae_joue(d: &HorlogeFictive, ctx: &Contexte, fichier: &str, reel: bool) -> R<()> {
    d.reverifier()?;
    depot_acte::deposer_dpae(ctx, fichier, reel, None, QUI_HORLOGE, false, "")?;
    Ok(())
}

/// LES DSN DU MOIS QUI NE SONT PAS PARTIES et que `garder` retient (sur ce que le fichier declare), deposees chacune en
/// test puis en reel -- le bilan du test lu avant le reel (`depot_acte::exiger_repetition`) --, dans
/// l'ordre de `crate::etapes::ordre_de_depot`. Rend vrai si l'une est partie ; `a_faire` recoit ce que le rangement des AER
/// jouees laisse a faire (les suites du depot, ses retours etant la des la reponse).
/// CE QUI EST PARTI NE REPART PAS : un fichier parti en reel et qu'aucun bilan ne refuse est fait ; les octets qu'un
/// organisme a refuses (en test ou en reel) ne repartent jamais. Seul un fichier change depuis son refus -- la correction
/// d'une personne -- part.
fn deposer_les_dsn(
    d: &HorlogeFictive,
    ctx: &Contexte,
    mois: Mois,
    garder: impl Fn(&declarations::dsn_lue::DsnLue) -> bool,
    a_faire: &mut Vec<String>,
) -> R<bool> {
    let f = Moteur::nouveau(ctx).fichiers_deposables(mois)?;
    let chemins: Vec<String> = f.dsn.iter().map(|x| x.chemin.clone()).collect();
    let journal = envois::journal::lu(&ctx.racine)?;
    let lignes: Vec<&envois::journal::Ligne> = envois::journal::filtrer(&journal.lignes, Some(envois::journal::Acte::Dsn), None, false).filter(|l| l.accepte()).collect();
    let mut fait = false;
    for chemin in chemins {
        let p = Path::new(&chemin);
        let nom = socle::fichiers::nom_du(p);
        let octets = std::fs::read(p).map_err(|e| socle::echec::du_disque(p, &e).reclasser(Classe::du_disque(&e)))?;
        // LA DEMANDE DE TAUX NE PART QU'AVEC LA DPAE (`depot_acte::deposer_dpae`) : un signalement d'amorcage reste d'un
        // essai ne part pas avec la DSN du mois.
        let lue = declarations::dsn_lue::lire(&octets, &nom)?;
        if !garder(&lue) || lue.nature_declaration == Some(declarations::amorcage::NATURE) {
            continue;
        }
        let empreinte = envois::journal::empreinte(&octets);
        let mut deja = false;
        for l in lignes.iter().filter(|l| l.fichier() == Some(nom.as_str())) {
            let refuse = etats::refuse_par_un_retour(l);
            deja |= (l.reel() && !refuse) || (refuse && l.empreinte() == Some(empreinte.as_str()));
        }
        if deja {
            continue;
        }
        // Un signalement qui ne part pas se nomme : le pas est celui de la DSN du mois.
        let signalement = lue.nature_declaration != Some(declarations::dsn_lue::MENSUELLE);
        for reel in [false, true] {
            a_faire.extend(deposer_dsn_joue(d, ctx, &chemin, reel).map_err(|mut e| {
                if signalement {
                    e = Echec { a_part: None, ..e.reformuler(|p| format!("le signalement {} ne part pas : {}", nom, p)) };
                }
                e
            })?);
        }
        fait = true;
    }
    Ok(fait)
}

// ------------------------------------------------------------------ dernier_rattrapage

/// `dernier_rattrapage` : ce que l'horloge a joue aujourd'hui, pour le bandeau « pendant votre absence ». Vide si elle
/// n'a rien joue ce jour-la.
pub(crate) fn dernier_rattrapage(racine: &Path, aujourdhui: Option<Jour>) -> R<Vec<String>> {
    let jour = match aujourdhui {
        Some(j) => j,
        None => droit::contexte::aujourdhui()?,
    };
    // Une horloge qui n'a jamais joue, ou rien ce jour-la, n'a rien a dire.
    Ok(memoire(racine)?.and_then(|m| m.dernier).filter(|dernier| dernier.le == jour).into_iter().flat_map(|dernier| dernier.fait).collect())
}

/// LA MEMOIRE DE L'HORLOGE (`config/horloge.json`) : le dernier jour joue, les mois dont tout est fait (chacun avec
/// l'empreinte de ce qui le fait, `empreinte_du_mois`), les annees finies, et ce qu'elle a fait en dernier (le bandeau
/// le dit ; rien quand on ne le retient pas).
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Memoire {
    quoi: String,
    le: Jour,
    mois_faits: BTreeMap<Mois, String>,
    annees_faites: BTreeSet<i32>,
    dernier: Option<DernierRattrapage>,
}

/// Ce que l'horloge a fait en dernier : le jour, et ses phrases.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DernierRattrapage {
    le: Jour,
    fait: Vec<String>,
}

/// La memoire de l'horloge, lue ; aucune tant qu'elle n'a jamais joue (le fichier n'existe pas).
fn memoire(racine: &Path) -> R<Option<Memoire>> {
    let f = racine.join(FICHIER_HORLOGE);
    if !socle::fichiers::existe(&f) {
        return Ok(None);
    }
    socle::forme::lire_fichier(&f, "La mémoire de l'horloge ne se lit pas.").map(Some).map_err(|m| m.reclasser(Classe::Illisible))
}

// ------------------------------------------------------------------ le calendrier

/// Un geste du calendrier. Chacun regarde, a son instant, ce que le moteur en dit, et ne fait rien si c'est fait.
enum Geste {
    /// Les engagements du mois qui commencent au plus tard `jusqu_au` se valident.
    Valider { mois: Mois, jusqu_au: Jour },
    /// Les contrats de l'embauche du `debut` partent a la signature.
    Contrats { mois: Mois, debut: Jour },
    /// La DPAE de l'embauche du `debut`, et son signalement d'amorcage.
    Dpae { mois: Mois, debut: Jour },
    /// Les contrats de l'embauche du `debut` reviennent signes.
    Signes { mois: Mois, debut: Jour },
    /// L'avenant du contrat (`salarie`, `debut`) part a la signature, puis revient signe.
    Avenant { mois: Mois, salarie: String, debut: Jour, nom: String },
    /// Le signalement d'arret (04) ou de reprise (05) de l'arret de `salarie` au `dernier` jour travaille.
    Arret { mois: Mois, salarie: String, dernier: Jour, cle: crate::obligations::CleDObligation, nature: crate::imprevus::NatureSignalement },
    /// Le signalement de fin du contrat de `salarie` qui finit le `fin`.
    Fin { mois: Mois, salarie: String, fin: Jour },
    /// La DSN du mois, fabriquee a son echeance.
    Dsn { mois: Mois, echeance: Jour },
    /// Les documents du mois aux salaries.
    Documents { mois: Mois },
    /// L'exercice `annee` se clot.
    Cloture { annee: i32 },
    /// L'exercice `annee` s'exporte pour la comptabilite.
    Export { annee: i32 },
    /// Un pas que le moteur empeche de regarder : son refus se dit, a l'instant ou l'horloge joue.
    Empeche { refus: Echec },
}

/// Un pas du calendrier : son instant (a Paris), sa phrase, son geste.
struct Pas {
    quand: Moment,
    quoi: String,
    geste: Geste,
}

// ------------------------------------------------------------------ rattraper

/// `rattraper(ctx, aujourdhui, retenir)` : joue ce qui est du avant `aujourdhui` et ne l'a pas ete. Rend ce qui a ete
/// fait, en phrases. Ne leve jamais pour un pas : il devient une phrase, et les autres continuent -- la compagnie fictive
/// s'ouvre toujours. Hors compagnie fictive : liste vide, rien n'est touche.
pub fn rattraper(racine: &Path, aujourdhui: Option<Jour>, retenir: bool) -> R<Vec<String>> {
    // LA GARDE, d'abord et avant tout : sans horloge fictive, rien ne se rattrape.
    Ok(HorlogeFictive::verifier(racine)?.map(|d| rattraper_l_horloge(racine, d, aujourdhui, retenir)).transpose()?.into_iter().flatten().collect())
}

fn rattraper_l_horloge(racine: &Path, d: HorlogeFictive, aujourdhui: Option<Jour>, retenir: bool) -> R<Vec<String>> {
    let jour = match aujourdhui {
        Some(j) => j,
        None => droit::contexte::aujourdhui()?,
    };
    let d = d.au_jour(jour);
    // SA MEMOIRE (config/horloge.json) : le dernier jour joue, et les mois dont tout est fait, chacun avec l'empreinte de
    // ce qui le fait (sa saisie et ses imprevus). La veille repasse toutes les vingt minutes : un mois fait ne se relit
    // que si ce qui le fait a change (une correction, un arret saisi apres coup).
    // Une horloge qui n'a jamais joue n'a encore rien de fait.
    let (mut clos, mut annees_faites, ancien) = match memoire(racine)? {
        Some(m) => (m.mois_faits, m.annees_faites, m.dernier),
        None => (BTreeMap::new(), BTreeSet::new(), None),
    };

    // CE QUI EST DU, chaque pas avec son instant : les mois qui ne sont pas faits, les annees qui ne sont pas finies.
    let saisis = engagement::stock::mois_saisis(racine)?;
    let mut calendrier: Vec<Pas> = Vec::new();
    let mut echus: Vec<socle::mois::Mois> = Vec::new();
    {
        let ctx = d.contexte()?;
        // Les mois saisis, et ceux dont la DSN est due sans engagement : ce que le panneau et les rappels reclament.
        for le_mois in crate::rappels::mois_a_parcourir(&ctx, jour)? {
            let premier = le_mois.premier_jour();
            if jour < premier {
                break;
            }
            // Un mois fait dont la DSN est refusee apres coup (un bilan arrive plus tard) n'est plus fait.
            if clos.get(&le_mois) == Some(&empreinte_du_mois(racine, le_mois)?) && !compagnie::etat_dsn::du_mois(&ctx, le_mois, None)?.refusee() {
                continue;
            }
            // Un mois echu dont tout est deja fait (par la personne, avant que l'horloge ne passe) est fait : rien n'est a
            // jouer pour lui.
            let suivant = le_mois.dernier_jour().plus_jours(1)?;
            if jour >= suivant && mois_fait(&ctx, le_mois, jour)? {
                clos.insert(le_mois, empreinte_du_mois(racine, le_mois)?);
                continue;
            }
            if les_pas_du_mois(&d, &ctx, le_mois, &mut calendrier)? {
                echus.push(le_mois);
            }
        }
    }
    // Les annees depuis le premier mois saisi ; aucune sans saisie.
    let annees: Vec<i32> = saisis.first().map(|premier| premier.annee()).into_iter().flat_map(|debut| debut..=jour.an).filter(|a| !annees_faites.contains(a)).collect();
    for annee in &annees {
        les_pas_de_l_annee(*annee, jour, &mut calendrier)?;
    }

    // L'ORDRE DU CALENDRIER : du plus ancien au plus recent, de mois en mois sans rien sauter ; a instant egal, dans
    // l'ordre ou les pas se sont dits (le tri est stable).
    calendrier.sort_by(|a, b| a.quand.cmp(&b.quand));
    let mut fait: Vec<String> = Vec::new();
    for p in &calendrier {
        fait.extend(jouer(&d, p));
    }

    // FAIT (`mois_fait`) : un mois echu, apres ce que l'horloge vient de jouer. Un mois
    // refuse se regarde a chaque ouverture ; un signalement qui attend la saisie (`fins_a_signaler`, `a_saisir` : une case
    // de la carte vide) est a la personne, et les rappels le lui disent : le mois se rouvre quand sa saisie change
    // (`empreinte_du_mois`).
    for le_mois in echus {
        let ctx = d.contexte()?;
        if mois_fait(&ctx, le_mois, jour)? {
            clos.insert(le_mois, empreinte_du_mois(racine, le_mois)?);
        } else {
            clos.remove(&le_mois);
        }
    }
    for annee in annees {
        if annee_finie(&d, annee - 1)? {
            annees_faites.insert(annee);
        }
    }
    let dernier = if !fait.is_empty() && retenir {
        Some(DernierRattrapage { le: jour, fait: fait.clone() })
    } else if !retenir {
        None
    } else {
        ancien
    };
    let sortie = Memoire { quoi: QUOI.to_string(), le: jour, mois_faits: clos, annees_faites, dernier };
    socle::ecriture::ecrire_json(&racine.join(FICHIER_HORLOGE), &sortie, 2).map_err(|m| m.reclasser(Classe::Disque))?;
    Ok(fait)
}

/// LES PAS D'UN MOIS jusqu'au jour joue, chacun a son instant :
/// - le jour de chaque embauche du mois : ses engagements se valident a 5 h, ses contrats partent a la signature a 6 h
///   (imprimes, sans courriel dans la fiche),
///   sa DPAE a 7 h, ses contrats reviennent signes a 8 h ;
/// - l'avenant d'un contrat, a l'instant ou son engagement a ete valide en dernier ;
/// - les signalements dus, le jour de leur date butoir, a 9 h 30 ;
/// - le 1er du mois suivant a 9 h, ce qui reste en saisie se valide ;
/// - le jour de l'echeance de la DSN, la DSN a 10 h, puis les documents a 11 h. Un mois dont la DSN a ete refusee attend
///   sa correction : sa DSN et ses documents se regardent a l'instant ou l'horloge joue.
/// Rend vrai si l'echeance du mois est passee (le mois peut alors etre fait).
fn les_pas_du_mois(d: &HorlogeFictive, ctx: &Contexte, le_mois: Mois, calendrier: &mut Vec<Pas>) -> R<bool> {
    let jour = d.jour;
    let mois = le_mois;
    let debuts: BTreeSet<Jour> = engagement::stock::objets_du_mois(d.racine, le_mois)
        ?
        .iter()
        .filter(|o| !o.importe_seul())
        .filter_map(|o| engagement::debut_brut(&o.brut))
        .filter(|debut| debut.mois_civil() == mois && *debut <= jour)
        .collect();
    for debut in debuts {
        let j = debut.jj_mm_aaaa();
        calendrier.push(Pas {
            quand: a_l_heure(debut, 5, 0)?,
            quoi: format!("engagements de l'embauche du {} validés", j),
            geste: Geste::Valider { mois: le_mois, jusqu_au: debut },
        });
        calendrier.push(Pas {
            quand: a_l_heure(debut, 6, 0)?,
            quoi: format!("contrats de l'embauche du {} envoyés à la signature", j),
            geste: Geste::Contrats { mois: le_mois, debut },
        });
        calendrier.push(Pas {
            quand: a_l_heure(debut, 7, 0)?,
            quoi: format!("DPAE de l'embauche du {} envoyée", j),
            geste: Geste::Dpae { mois: le_mois, debut },
        });
        calendrier.push(Pas {
            quand: a_l_heure(debut, 8, 0)?,
            quoi: format!("contrats de l'embauche du {} revenus signés", j),
            geste: Geste::Signes { mois: le_mois, debut },
        });
    }
    // LES AVENANTS se lisent dans l'apercu des signatures, qui relit la paie de chaque engagement valide du mois : une paie
    // que le moteur refuse de calculer (un mois dont la version n'a pas les taux) empeche ce pas, et lui seul ; il se dit,
    // nomme par le refus (le salarie, le mois, la raison), et les autres pas se jouent.
    let apercu = match crate::contrats::signature_apercu(ctx, le_mois) {
        Ok(a) => Some(a.contrats),
        Err(e) if est_une_erreur_d_operation(&e) => {
            calendrier.push(Pas { quand: maintenant_du(jour)?, quoi: format!("avenants de {} regardés", mois_fr(mois)), geste: Geste::Empeche { refus: e } });
            None
        }
        Err(e) => return Err(e),
    };
    for c in apercu.into_iter().flatten() {
        let Some(quand) = revalide_le(ctx, mois, &c.salarie, c.debut)? else { continue };
        if quand.jour_de_paris() > jour {
            continue;
        }
        calendrier.push(Pas { quand, quoi: String::new(), geste: Geste::Avenant { mois: le_mois, salarie: c.salarie, debut: c.debut, nom: c.nom } });
    }
    calendrier.extend(signalements_dus(ctx, le_mois, jour, false)?);

    let suivant = le_mois.dernier_jour().plus_jours(1)?;
    if jour < suivant {
        return Ok(false);
    }
    let fin = le_mois.dernier_jour();
    let valide_le = match compagnie::installation::cree_le(d.racine)? {
        Some(c) if c > suivant => c.min(jour),
        _ => suivant,
    };
    calendrier.push(Pas {
        quand: a_l_heure(valide_le, 9, 0)?,
        quoi: format!("engagements de {} validés", mois_fr(mois)),
        geste: Geste::Valider { mois: le_mois, jusqu_au: fin },
    });
    // Le jour ou la DSN est due (`echeance_dsn`, reporte au jour ouvre) : elle part ce matin-la, avant midi.
    let echeance = declarations::echeance_dsn::de_la_compagnie(ctx, mois)?.jour;
    if jour < echeance {
        return Ok(false);
    }
    let (quand_dsn, quand_documents) = if compagnie::etat_dsn::du_mois(ctx, mois, None)?.refusee() {
        let maintenant = maintenant_du(jour)?;
        (maintenant, maintenant)
    } else {
        // Les documents partent apres la validation : la paie partie d'un engagement revalide apres l'echeance repart, en
        // bulletin rectificatif, a l'instant de sa revalidation.
        let revalide = engagement::stock::objets_du_mois(d.racine, le_mois)
            ?
            .iter()
            .filter_map(|o| o.brut.valide.as_ref().map(|v| v.le))
            .max();
        let documents = a_l_heure(echeance, 11, 0)?;
        let documents = match revalide {
            Some(r) if r > documents => r,
            _ => documents,
        };
        (a_l_heure(echeance, 10, 0)?, documents)
    };
    calendrier.push(Pas { quand: quand_dsn, quoi: format!("DSN de {} déposée", mois_fr(mois)), geste: Geste::Dsn { mois: le_mois, echeance } });
    calendrier.push(Pas {
        quand: quand_documents,
        quoi: format!("documents de {} envoyés aux salariés", mois_fr(mois)),
        geste: Geste::Documents { mois: le_mois },
    });
    Ok(true)
}

/// FIN JANVIER DE `annee`, l'annee d'avant se clot a 10 h, puis s'exporte pour la comptabilite a 11 h.
fn les_pas_de_l_annee(annee: i32, jour: Jour, calendrier: &mut Vec<Pas>) -> R<()> {
    let fin_janvier = Jour::nouveau(annee, 1, 31).ok_or_else(|| erreur(Classe::Illisible, "date impossible"))?;
    if jour < fin_janvier {
        return Ok(());
    }
    let precedente = annee - 1;
    calendrier.push(Pas { quand: a_l_heure(fin_janvier, 10, 0)?, quoi: format!("exercice {} clos", precedente), geste: Geste::Cloture { annee: precedente } });
    calendrier.push(Pas {
        quand: a_l_heure(fin_janvier, 11, 0)?,
        quoi: format!("exercice {} exporté pour la comptabilité", precedente),
        geste: Geste::Export { annee: precedente },
    });
    Ok(())
}

/// L'instant ou l'horloge joue, le jour `jour` : maintenant, si c'est ce jour-la ; sinon ce jour a midi.
fn maintenant_du(jour: Jour) -> R<Moment> {
    let m = socle::temps::secondes().a_la_seconde();
    if m.jour_de_paris() == jour {
        Ok(m)
    } else {
        a_l_heure(jour, 12, 0)
    }
}

/// UN PAS JOUE : son geste, a son instant (`pas`). Rend ses phrases.
fn jouer(d: &HorlogeFictive, p: &Pas) -> Vec<String> {
    let (quand, quoi) = (p.quand, p.quoi.as_str());
    match &p.geste {
        Geste::Valider { mois, jusqu_au } => valider_les_engagements(d, *mois, *jusqu_au, quoi, quand),
        Geste::Contrats { mois, debut } => {
            // Ceux qui ont un courriel partent a la signature ; un contrat sans courriel dans la fiche s'imprime et se
            // remet en main propre : l'impression est son depart.
            let mut imprimes: Vec<Signable> = Vec::new();
            let mut fait = pas(d, quoi, quand, |ctx| {
                let (par_courriel, sans): (Vec<Signable>, Vec<Signable>) =
                    signables(ctx, *mois)?.into_iter().filter(|c| c.vise.debut == *debut && c.a_envoyer).partition(|c| c.courriel);
                imprimes = sans;
                if par_courriel.is_empty() {
                    return Ok(Joue::Rien);
                }
                // LA BOITE MUETTE, et jamais une autre : le passe joue n'ecrit rien dans `courriels-simules/`.
                let mut ouvrir = || -> R<Box<dyn Courriel>> { Ok(Box::new(BoiteMuette::nouvelle())) };
                let contrats = par_courriel.into_iter().map(|c| c.vise).collect();
                crate::contrats::demander_signature(ctx, ChargeSignature::des_contrats(*mois, contrats), None, &mut ouvrir)?;
                Ok(Joue::Fait)
            });
            if !imprimes.is_empty() {
                let noms = imprimes.iter().map(|c| c.nom.as_str()).collect::<Vec<_>>().join(", ");
                let quoi = format!("contrats de l'embauche du {} imprimés pour {} (pas de courriel dans sa fiche), remis en main propre", debut.jj_mm_aaaa(), noms);
                fait.extend(pas(d, &quoi, quand, |ctx| {
                    let contrats = imprimes.iter().map(|c| c.vise.clone()).collect();
                    crate::contrats::contrats_imprimer(ctx, ChargeSignature::des_contrats(*mois, contrats), QUI_HORLOGE)?;
                    Ok(Joue::Fait)
                }));
            }
            fait
        }
        Geste::Dpae { mois, debut } => pas(d, quoi, quand, |ctx| {
            let a_declarer = Moteur::nouveau(ctx).embauches_a_declarer(Some(*mois))?;
            let attente: Vec<_> = a_declarer.iter().filter(|e| e.debut == *debut).collect();
            if attente.is_empty() {
                return Ok(Joue::Rien);
            }
            let engagements: Vec<crate::depots::EmbaucheDesignee> = attente.iter().map(|e| crate::depots::EmbaucheDesignee { salarie: e.salarie.clone(), numero: e.numero.to_string() }).collect();
            // Une DPAE que rien n'ecrit n'a rien a deposer.
            let crate::depots::Dpae::Ecrite(r) = crate::depots::dpae(ctx, *mois, Some(&engagements))? else {
                return Ok(Joue::Rien);
            };
            let fichier = r.chemin;
            // La DPAE emporte la demande de taux de PAS de ses embauches (`depot_acte::deposer_dpae`).
            for reel in [false, true] {
                deposer_dpae_joue(d, ctx, &fichier, reel)?;
            }
            Ok(Joue::Fait)
        }),
        // UN CONTRAT REVIENT SIGNE S'IL EST PARTI (par courriel ou imprime) : jamais un retour sans son depart.
        Geste::Signes { mois, debut } => pas(d, quoi, quand, |ctx| {
            let manquent: Vec<Signable> = signables(ctx, *mois)?.into_iter().filter(|c| c.vise.debut == *debut && !c.a_envoyer).collect();
            if manquent.is_empty() {
                return Ok(Joue::Rien);
            }
            for c in manquent {
                let ContratVise { salarie, debut, numero } = c.vise;
                crate::contrats::contrat_recu(ctx, &ChargeRecu { mois: *mois, salarie, debut, numero, avenant: None, recu: true }, QUI_HORLOGE, None)?;
            }
            Ok(Joue::Fait)
        }),
        Geste::Avenant { mois, salarie, debut, nom } => {
            let du_contrat = |ctx: &Contexte| -> R<Option<ContratASigner>> {
                Ok(crate::contrats::signature_apercu(ctx, *mois)?.contrats.into_iter().find(|x| x.salarie == *salarie && x.debut == *debut))
            };
            // La ligne de l'horloge dit ce que le journal dit : parti par courriel, ou imprime et remis en main propre.
            let imprime = std::cell::Cell::new(false);
            let mut fait = pas(d, &format!("avenant du contrat de {} envoyé à la signature", nom), quand, |ctx| {
                // Aucun avenant en attente de depart.
                let Some(x) = du_contrat(ctx)?.filter(|x| x.en_avenant) else {
                    return Ok(Joue::Rien);
                };
                let numero = x.numero.clone();
                let voulu = AvenantVoulu { salarie: salarie.clone(), debut: *debut, numero, mois: Some(*mois), effet: None };
                // LA BOITE MUETTE, et jamais une autre ; sans courriel dans la fiche, l'avenant s'imprime et se remet en
                // main propre : l'impression est son depart.
                let mut ouvrir = || -> R<Box<dyn Courriel>> { Ok(Box::new(BoiteMuette::nouvelle())) };
                if x.destinataire.is_some() {
                    let etats = crate::contrats::signature_apercu(ctx, *mois)?.contrats;
                    crate::contrats::envoyer_avenants(ctx, std::slice::from_ref(&voulu), &etats, &Preuve::default(), &mut ouvrir)?;
                } else {
                    crate::contrats::contrats_imprimer(ctx, ChargeSignature::des_avenants(*mois, vec![voulu]), QUI_HORLOGE)?;
                    imprime.set(true);
                }
                Ok(Joue::Fait)
            });
            if imprime.get() {
                fait = fait.into_iter().map(|l| l.replace("envoyé à la signature", "imprimé, remis en main propre")).collect();
            }
            let signe = if imprime.get() { "signé, remis en main propre" } else { "revenu signé" };
            fait.extend(pas(d, &format!("avenant du contrat de {} {}", nom, signe), quand, |ctx| {
                let Some(x) = du_contrat(ctx)?.filter(|x| x.avenant_attendu && x.en_attente) else {
                    return Ok(Joue::Rien);
                };
                let charge = ChargeRecu {
                    mois: *mois,
                    salarie: salarie.clone(),
                    debut: *debut,
                    numero: x.numero.clone(),
                    avenant: x.avenant.as_ref().and_then(|a| a.numero),
                    recu: true,
                };
                crate::contrats::contrat_recu(ctx, &charge, QUI_HORLOGE, None)?;
                Ok(Joue::Fait)
            }));
            fait
        }
        Geste::Arret { mois, salarie, dernier, cle, nature } => pas(d, quoi, quand, |ctx| {
            // ENCORE DU au jour du pas ? Le visiteur a pu le faire.
            let a = crate::metier::imprevus::tous_les_arrets(ctx)?.into_iter().find(|x| x.salarie == *salarie && x.dernier_jour_travaille == *dernier);
            let Some(a) = a else { return Ok(Joue::Rien) };
            let obligations = crate::obligations::de_l_arret(ctx, &a, &crate::obligations::Suivi::lire(ctx)?, ctx.aujourdhui)?;
            if !une_due(&obligations, *cle, ctx.aujourdhui) {
                return Ok(Joue::Rien);
            }
            d.reverifier()?;
            crate::imprevus::signaler_l_arret(ctx, &crate::imprevus::ChargeArretSignaler::de(salarie.clone(), *dernier, *nature), None)?;
            deposer_les_dsn(d, ctx, *mois, |x| x.nature_declaration == Some(nature.nature()), &mut Vec::new())?;
            Ok(Joue::Fait)
        }),
        Geste::Fin { mois, salarie, fin } => pas(d, quoi, quand, |ctx| {
            let mut encore = false;
            for x in crate::imprevus::fins_a_signaler(ctx, *mois, ctx.aujourdhui)? {
                encore |= x.salarie.as_str() == salarie && x.fin == *fin && due(&x.obligation, ctx.aujourdhui).is_some();
            }
            if !encore {
                return Ok(Joue::Rien);
            }
            d.reverifier()?;
            crate::imprevus::signaler_la_fin(ctx, &crate::imprevus::ChargeFinContratSignaler::de(*mois, salarie.clone()), false)?;
            deposer_les_dsn(d, ctx, *mois, |x| x.nature_declaration == Some(declarations::fctu::NATURE), &mut Vec::new())?;
            Ok(Joue::Fait)
        }),
        Geste::Dsn { mois, echeance } => {
            let mut a_faire: Vec<String> = Vec::new();
            let mut fait = pas(d, quoi, quand, |ctx| {
                if compagnie::etat_dsn::du_mois(ctx, *mois, None)?.partie() {
                    return Ok(Joue::Rien);
                }
                // LA DSN SE FABRIQUE A SON DEPOT, depuis les engagements valides du mois, DATEE DE SON ECHEANCE quel que
                // soit le jour ou elle part : la DSN d'un mois refuse que personne n'a corrige se refait octet pour octet,
                // et ne repart pas (`deposer_les_dsn`).
                d.reverifier()?;
                let a_l_echeance = HorlogeFictive { racine: d.racine, jour: (*echeance).min(d.jour) };
                let ctx_e = a_l_echeance.contexte()?;
                let fabriquer = || crate::gestes::depart::fabriquer_dsn(&ctx_e, *mois, QUI_HORLOGE);
                let t = a_l_heure(*echeance, 10, 0)?;
                socle::temps::pendant(t, fabriquer)?;
                // TOUTES LES DSN DU MOIS QUI NE SONT PAS PARTIES, dans l'ordre de `crate::etapes::ordre_de_depot` :
                // la mensuelle, puis les signalements.
                deposer_les_dsn(d, ctx, *mois, |_| true, &mut a_faire).map(Joue::si)
            });
            // Ce que le rangement de l'AER jouee laisse a faire se dit, a la date du depot, comme le dit la releve.
            fait.extend(a_faire.into_iter().map(|p| format!("Le {} : {}", quand.jour_de_paris().jj_mm_aaaa(), p)));
            fait
        }
        Geste::Documents { mois } => {
            // Ceux qui n'ont pas de courriel : leurs documents s'impriment et se remettent en main propre (l'impression
            // est leur depart) ; ils n'empechent pas les autres de partir.
            let mut sans: Vec<(String, String)> = Vec::new();
            let mut fait = pas(d, quoi, quand, |ctx| {
                // LES DOCUMENTS D'UN MOIS DONT LA DSN N'EST PAS ACCEPTEE ne partent pas : ils suivent sa DSN, jamais avant.
                if !compagnie::etat_dsn::du_mois(ctx, *mois, None)?.partie() {
                    return Ok(Joue::Rien);
                }
                let bilan = Moteur::nouveau(ctx).bilan_du_mois(*mois)?;
                use crate::etapes::{EtatCase, QuelleEtape};
                let a_envoyer = bilan.etapes.iter().filter(|e| e.cle == QuelleEtape::Documents).flat_map(|e| &e.groupes).any(|g| g.etat == EtatCase::AFaire);
                if !a_envoyer {
                    return Ok(Joue::Rien);
                }
                sans = crate::documents::sans_courriel(ctx, *mois)?;
                // LA BOITE MUETTE, et jamais une autre : le passe joue n'ecrit rien dans `courriels-simules/`.
                let mut ouvrir = || -> R<Box<dyn Courriel>> { Ok(Box::new(BoiteMuette::nouvelle())) };
                match crate::documents::envoyer_documents_de(ctx, *mois, &Preuve::default(), QUI_HORLOGE, &mut ouvrir) {
                    Ok(_) => Ok(Joue::Fait),
                    // Un mois sans documents a remettre par courriel.
                    Err(e) if est_une_erreur_d_operation(&e) => Ok(Joue::Rien),
                    Err(e) => Err(e),
                }
            });
            if !sans.is_empty() {
                let noms = sans.iter().map(|(_, n)| n.as_str()).collect::<Vec<_>>().join(", ");
                let salaries: Vec<String> = sans.iter().map(|(c, _)| c.clone()).collect();
                let quoi = format!("documents de {} imprimés pour {} (pas de courriel dans sa fiche), remis en main propre", mois_fr(*mois), noms);
                fait.extend(pas(d, &quoi, quand, |ctx| {
                    let charge = crate::documents::ChargeDocumentsImprimer { mois: *mois, salaries: Some(salaries.clone()) };
                    crate::documents::documents_imprimer(ctx, charge, QUI_HORLOGE)?;
                    Ok(Joue::Fait)
                }));
            }
            fait
        }
        Geste::Cloture { annee } => pas(d, quoi, quand, |ctx| {
            let annee = Annee::nouvelle(i64::from(*annee))?;
            let a_des_paies = engagement::stock::mois_saisis(&ctx.racine)?.iter().any(|m| m.annee_civile() == annee);
            if Moteur::nouveau(ctx).exercice_clos(annee)? || !a_des_paies {
                return Ok(Joue::Rien);
            }
            crate::gestes::exercice_ecrit::cloturer(ctx, annee, false, QUI_HORLOGE)?;
            Ok(Joue::Fait)
        }),
        Geste::Empeche { refus } => non_joue(quand, quoi, refus),
        Geste::Export { annee } => pas(d, quoi, quand, |ctx| {
            let annee = Annee::nouvelle(i64::from(*annee))?;
            if !Moteur::nouveau(ctx).exercice_clos(annee)? || crate::gestes::exercice_ecrit::export_compta_a_jour(&ctx.racine, annee)? {
                return Ok(Joue::Rien);
            }
            crate::gestes::exercice_ecrit::export_compta(ctx, annee, 1, 12, true)?;
            Ok(Joue::Fait)
        }),
    }
}

// ------------------------------------------------------------------ le mois et ses embauches

/// LES ENGAGEMENTS DU MOIS SE VALIDENT, comme une compagnie bien tenue les valide : ceux qui commencent au plus tard
/// `jusqu_au`, a `quand`, par le geste de l'ecran (la route `valider-engagement` : son verrou, ses refus), chacun
/// designe par son identite et tel qu'il est ecrit. Rien ne part d'un engagement en saisie ; un engagement auquel il
/// manque un fait attend qu'on le complete, et un refus du moteur arrete le pas, qui le dit.
fn valider_les_engagements(d: &HorlogeFictive, le_mois: Mois, jusqu_au: Jour, quoi: &str, quand: Moment) -> Vec<String> {
    pas(d, quoi, quand, |ctx| {
        let objets = engagement::stock::objets_du_mois(&ctx.racine, le_mois)?;
        let ids = engagement::stock::identites(&objets);
        // Un mois sans saisie n'a rien a valider.
        let Some(vue) = engagement::stock::vue_du_mois(&ctx.racine, le_mois)? else { return Ok(Joue::Rien) };
        let manques = crate::mois::manques_de_la_saisie(ctx, &vue);
        let mut fait = false;
        for ((o, id), manque) in objets.iter().zip(ids).zip(manques) {
            if o.est_valide() || o.importe_seul() || manque.is_some() || engagement::debut_brut(&o.brut) > Some(jusqu_au) {
                continue;
            }
            let charge = crate::engagement::ChargeValider { mois: le_mois, engagement: id };
            crate::engagement::valider_engagement(ctx, charge)?;
            fait = true;
        }
        Ok(Joue::si(fait))
    })
}

/// Un contrat de spectacle du mois qui n'est pas revenu signe (`signature-apercu`).
struct Signable {
    /// Le contrat, tel que les routes des contrats le recoivent.
    vise: ContratVise,
    /// « NOM Prenom ».
    nom: String,
    /// Sa fiche a un courriel : il part a la signature par courriel ; sinon il s'imprime.
    courriel: bool,
    /// Jamais parti, ou parti puis rendu faux par l'engagement avant d'etre revenu signe : il part dans sa version presente.
    a_envoyer: bool,
}

/// Les contrats de spectacle du mois qui ne sont pas revenus signes.
fn signables(ctx: &Contexte, mois: Mois) -> R<Vec<Signable>> {
    Ok(crate::contrats::signature_apercu(ctx, mois)?
        .contrats
        .into_iter()
        .filter(|x| x.recu_le.is_none())
        .map(|x| Signable {
            courriel: x.destinataire.is_some(),
            a_envoyer: x.a_envoyer && !x.en_avenant,
            vise: ContratVise { numero: x.numero.clone(), salarie: x.salarie, debut: x.debut },
            nom: x.nom,
        })
        .collect())
}

// ------------------------------------------------------------------ les avenants

/// L'instant ou l'engagement de ce contrat a ete valide en dernier (`valide.le`).
fn revalide_le(ctx: &Contexte, mois: Mois, salarie: &str, debut: Jour) -> R<Option<Moment>> {
    Ok(engagement::stock::objets_du_mois(&ctx.racine, mois)?.into_iter().find_map(|o| {
        let b = &*o.brut;
        let sien = b.saisie.carte.salarie.as_deref() == Some(salarie) && engagement::debut_brut(b) == Some(debut);
        sien.then(|| o.brut.valide.as_ref().map(|v| v.le))?
    }))
}

// ------------------------------------------------------------------ les signalements

/// Une obligation que le moteur dit a faire (ou en retard) : sa date butoir, qu'elle soit passee ou non.
fn a_faire(o: &crate::obligations::Obligation) -> Option<Jour> {
    if !o.etat.du() {
        return None;
    }
    o.butoir
}

/// Une obligation a faire (`a_faire`) dont la date butoir est passee ou d'aujourd'hui : sa butoir.
fn due(o: &crate::obligations::Obligation, jour: Jour) -> Option<Jour> {
    a_faire(o).filter(|b| *b <= jour)
}

/// Une obligation de cette cle est-elle due ce jour (`due`) ?
fn une_due(obligations: &[crate::obligations::Obligation], cle: crate::obligations::CleDObligation, jour: Jour) -> bool {
    obligations.iter().filter(|o| o.cle == cle).any(|o| due(o, jour).is_some())
}

/// LES SIGNALEMENTS DU MOIS que le moteur dit dus au jour `jour` (`obligations::de_l_arret`, `imprevus::fins_a_signaler`) :
/// l'arret (04) et la reprise anticipee (05) d'un arret dont le dernier jour travaille est du mois, la fin de contrat (07)
/// d'un contrat qui finit dans le mois. Chacun s'ecrit et part (test puis reel) le jour de sa date butoir, a 9 h 30 --
/// avant la DSN du mois quand c'est le jour de son echeance (un arret subroge) ; un signalement fait n'est plus du.
/// Une fin dont la carte n'a pas encore ce que le signalement exige (`a_saisir`) en est : son pas dit pourquoi il n'est
/// pas joue.
///
/// `cloture` : ce qui garde le mois ouvert. Tout signalement du mois a faire que l'horloge peut ecrire, sa date butoir
/// passee ou non : le delai de transmission d'un signalement (cinq jours ouvres, arrete du 2 juillet 2013, art. 1er, pris
/// pour CSS R. 133-14, II ; norme DSN, chapitre 2, § 2.5.3) peut finir apres l'echeance de la DSN du mois (une fin le
/// 30/01, signalee jusqu'au 06/02, quand la DSN de janvier est due le 05/02), et un mois fait ne se relit plus. Pas celui
/// qui attend la saisie : il est a la personne.
fn signalements_dus(ctx: &Contexte, le_mois: Mois, jour: Jour, cloture: bool) -> R<Vec<Pas>> {
    let du = |o: &crate::obligations::Obligation| -> Option<Jour> { if cloture { a_faire(o) } else { due(o, jour) } };
    let mut dus = Vec::new();
    use crate::imprevus::NatureSignalement;
    let natures = [(crate::obligations::CleDObligation::SignalementArret, NatureSignalement::Arret, "d'arrêt"), (crate::obligations::CleDObligation::SignalementReprise, NatureSignalement::Reprise, "de reprise")];
    let suivi = crate::obligations::Suivi::lire(ctx)?;
    for a in crate::metier::imprevus::tous_les_arrets(ctx)? {
        let dernier = a.dernier_jour_travaille;
        if dernier.mois_civil() != le_mois {
            continue;
        }
        let obligations = crate::obligations::de_l_arret(ctx, &a, &suivi, jour)?;
        for (cle, nature, dit) in natures {
            let Some(o) = obligations.iter().find(|o| o.cle == cle) else { continue };
            let Some(butoir) = du(o) else { continue };
            dus.push(Pas {
                quand: a_l_heure(butoir, 9, 30)?,
                quoi: format!("signalement {} de {} (dernier jour travaillé le {}) déposé", dit, a.salarie, dernier.jj_mm_aaaa()),
                geste: Geste::Arret { mois: le_mois, salarie: a.salarie.to_string(), dernier, cle, nature },
            });
        }
    }
    for f in crate::imprevus::fins_a_signaler(ctx, le_mois, jour)? {
        let Some(butoir) = du(&f.obligation) else { continue };
        if cloture && !f.a_saisir.is_empty() {
            continue;
        }
        dus.push(Pas {
            quand: a_l_heure(butoir, 9, 30)?,
            quoi: format!("signalement de fin du contrat de {} (fin le {}) déposé", f.salarie, f.fin.jj_mm_aaaa()),
            geste: Geste::Fin { mois: le_mois, salarie: f.salarie.to_string(), fin: f.fin },
        });
    }
    Ok(dus)
}

// ------------------------------------------------------------------ ce qui fait un mois

/// L'empreinte de ce qui fait un mois : sa saisie (`stock::empreinte_de_la_saisie_du_mois`) et ses imprevus. Un mois
/// fait dont l'empreinte change se rejoue.
fn empreinte_du_mois(racine: &Path, mois: Mois) -> R<String> {
    // Un mois sans saisie, ou sans imprevus, ou sans fiche de suivi, a une empreinte : celle de ce qu'il n'a pas.
    let saisie = format!("{:?}", engagement::stock::empreinte_de_la_saisie_du_mois(racine, mois, None)?);
    let mut tout = saisie.into_bytes();
    for f in [engagement::imprevus::dossier(racine).join(format!("{}.json", mois)), engagement::imprevus::dossier(racine).join("obligations.json")] {
        let octets = if socle::fichiers::existe(&f) { Some(std::fs::read(&f).map_err(|e| socle::echec::du_disque(&f, &e).reclasser(Classe::du_disque(&e)))?) } else { None };
        tout.extend(format!("{:?}", octets.map(|o| socle::fichiers::sha256_hex(&o))).into_bytes());
    }
    Ok(socle::fichiers::sha256_hex(&tout)[..16].to_string())
}

/// UN MOIS FAIT : toutes ses etapes sont vertes (`etapes_restantes`, ce que l'ecran montre) et aucun signalement n'est a
/// faire que l'horloge puisse ecrire, sa date butoir passee ou non (`signalements_dus`).
fn mois_fait(ctx: &Contexte, mois: socle::mois::Mois, jour: Jour) -> R<bool> {
    Ok(Moteur::nouveau(ctx).etapes_restantes(mois)?.is_empty() && signalements_dus(ctx, mois, jour, true)?.is_empty())
}

/// L'annee `annee` est-elle finie pour l'horloge : passee fin janvier de l'annee d'apres, et close et exportee, ou sans
/// aucun mois dans le logiciel.
fn annee_finie(d: &HorlogeFictive, annee: i32) -> R<bool> {
    if Jour::nouveau(annee + 1, 1, 31).is_none_or(|f| d.jour < f) {
        return Ok(false);
    }
    let ctx = d.contexte()?;
    let annee = Annee::nouvelle(i64::from(annee))?;
    if !engagement::stock::mois_saisis(&ctx.racine)?.iter().any(|m| m.annee_civile() == annee) {
        return Ok(true);
    }
    Ok(Moteur::nouveau(&ctx).exercice_clos(annee)? && crate::gestes::exercice_ecrit::export_compta_a_jour(&ctx.racine, annee)?)
}

// ------------------------------------------------------------------ les routes

/// La charge de `horloge-rattraper` : le jour joue (absent : aujourd'hui) ; `retenir: false` ne garde pas ce qui a ete
/// fait pour le bandeau (absent : il le garde).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeRattraper {
    aujourdhui: Option<Jour>,
    retenir: Option<bool>,
}

/// Ce que rend `horloge-rattraper` : ce que l'horloge a joue, une phrase par pas.
#[derive(Debug, Serialize)]
struct Rattrape {
    phrase: String,
    phrases: Vec<String>,
}

impl Dite for Rattrape {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

/// La route de l'horloge, `horloge-rattraper`.
pub fn route(ctx: &Contexte, charge: &Charge) -> R<Rendu> {
    let c: ChargeRattraper = charge.lire()?;
    let phrases = rattraper(&ctx.racine, c.aujourdhui, c.retenir != Some(false))?;
    en_json(Rattrape { phrase: format!("L'horloge a joué {} pas.", phrases.len()), phrases })
}

// ------------------------------------------------------------------ les tests

/// Une racine d'essai (dans le dossier temporaire, jamais `prive/` ni `fictives/`) : les trois fiches vides de quoi ouvrir un
/// `Contexte`, et, si `fictive`, le marqueur d'une compagnie fictive.
#[cfg(test)]
pub(crate) fn racine_d_essai(fictive: bool) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    let r = std::env::temp_dir().join(format!("paie-horloge-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
    let _ = std::fs::remove_dir_all(&r);
    std::fs::create_dir_all(r.join(socle::fichiers::CONFIG)).unwrap();
    for f in ["entreprise.json", "salaries.json", "spectacles.json"] {
        std::fs::write(r.join(socle::fichiers::CONFIG).join(f), "{}").unwrap();
    }
    if fictive {
        std::fs::write(r.join("COPIE-DE-DEVELOPPEMENT.json"), r#"{"copie_de": "compagnie fictive essai", "quoi": "essai"}"#).unwrap();
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tout ce que la racine porte : chemins relatifs et octets.
    fn photo(racine: &Path) -> Vec<(String, Vec<u8>)> {
        fn marche(d: &Path, racine: &Path, out: &mut Vec<(String, Vec<u8>)>) {
            for e in std::fs::read_dir(d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    marche(&p, racine, out);
                } else {
                    out.push((p.strip_prefix(racine).unwrap().to_string_lossy().into_owned(), std::fs::read(&p).unwrap()));
                }
            }
        }
        let mut v = Vec::new();
        marche(racine, racine, &mut v);
        v.sort();
        v
    }

    // ---------------------------------------------------------------- la garde

    #[test]
    fn la_garde_hors_compagnie_fictive_ne_joue_rien_et_ne_touche_a_rien() {
        let r = racine_d_essai(false);
        std::fs::create_dir_all(r.join("engagements")).unwrap();
        std::fs::write(r.join("engagements").join("2026-10.json"), "{}").unwrap();
        let avant = photo(&r);
        assert!(HorlogeFictive::verifier(&r).unwrap().is_none());
        assert_eq!(rattraper(&r, Jour::nouveau(2027, 3, 1), true).unwrap(), Vec::<String>::new());
        assert_eq!(rattraper(&r, Jour::nouveau(2027, 3, 1), false).unwrap(), Vec::<String>::new());
        assert_eq!(photo(&r), avant, "pas meme config/horloge.json");
        assert!(!r.join(FICHIER_HORLOGE).exists());
        // Un marqueur de copie de developpement qui n'est PAS une compagnie fictive ne suffit pas.
        std::fs::write(r.join("COPIE-DE-DEVELOPPEMENT.json"), r#"{"copie_de": "prive", "quoi": "copie"}"#).unwrap();
        let avant = photo(&r);
        assert!(HorlogeFictive::verifier(&r).unwrap().is_none());
        assert_eq!(rattraper(&r, Jour::nouveau(2027, 3, 1), true).unwrap(), Vec::<String>::new());
        assert_eq!(photo(&r), avant);
        // Un marqueur illisible se refuse : il ne dit pas si les envois sont joues.
        std::fs::write(r.join("COPIE-DE-DEVELOPPEMENT.json"), "{ pas du json").unwrap();
        assert!(HorlogeFictive::verifier(&r).is_err());
    }
}
