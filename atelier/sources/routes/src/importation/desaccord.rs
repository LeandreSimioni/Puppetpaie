//! UN BULLETIN DEJA REPRIS, ET LA DSN QUI ARRIVE ENSUITE ET DIT LE CONTRAT AUTREMENT (Léandre, 19/11/2026 : « si on a
//! déjà un bulletin et qu'on ajoute une DSN, il faut faire un message à l'utilisateur et c'est à l'utilisateur de
//! trancher » ; « il faut une page avec l'import pour qu'on puisse régler tous les conflits »). Ce que les pieces disent
//! du contrat : ses dates, son activite, sa nature ; ni ses montants, ni ses cotisations.
//!
//! - `en_attente` : chaque contrat repris d'un bulletin avant la DSN qui declare sa paie
//!   (`engagement::importe::bulletin_avant_la_dsn`), dont la compagnie n'a pas encore choisi la piece, et que la DSN dit
//!   autrement ; `importation-etat` les rend, chacun avec ses deux gestes. Tant qu'elle n'a rien choisi, ce qui etait la
//!   (le bulletin) est garde (`engagement::importe::la_dsn_prime`) ;
//! - la route `importation-desaccord` `{"salarie", "debut", "garder": "bulletin" | "dsn"}` ecrit son choix sur le
//!   contrat (`ContratAtteste::gardee`) ; « la DSN », l'engagement prend aussi les dates qu'elle declare. L'activite et la
//!   nature se lisent ensuite par la seule regle (`activite_du_mois`, `nature_de`).
//!
//! Une DSN deja la, puis une AEM ou un bulletin : la DSN prime, rien ne se demande.

use droit::contexte::{Contexte, IdSalarie};
use engagement::importe::{ContratAtteste, PaieVersee, PieceGardee};
use socle::date::{Jour, PeriodeOuverte};
use socle::echec::{erreur, Classe, R};
use socle::grandeurs::ecrit::sans_zero::dit;
use socle::texte::Phrase;

use crate::catalogue::{Affiche, Route};

/// Des dates, en mots.
fn dates(o: PeriodeOuverte) -> String {
    match o.fin() {
        Some(f) => format!("du {} au {}", o.debut().jj_mm_aaaa(), f.jj_mm_aaaa()),
        None => format!("du {}, sans fin dite", o.debut().jj_mm_aaaa()),
    }
}

/// CE QUE LE BULLETIN ET LA DSN DISENT AUTREMENT DU CONTRAT de `i` (le bulletin repris avant la DSN : `contrat`) : (ce
/// que dit le bulletin, ce que dit la DSN), une partie par chose qui differe ; vide : ils disent la meme chose. Les dates
/// et la nature, sur la derniere paie que la DSN declare ; l'activite, sur chaque mois ou le contrat tient
/// (`engagement::importe::activite_du_mois`), quand les deux pieces la disent.
fn ce_qui_differe(periode: Option<PeriodeOuverte>, contrat: &ContratAtteste, declarees: &[(socle::mois::Mois, &PaieVersee)]) -> R<(Vec<String>, Vec<String>)> {
    let (mut bulletin, mut dsn) = (Vec::new(), Vec::new());
    let Some((_, derniere)) = declarees.last() else { return Ok((bulletin, dsn)) };
    if let (Some(b), Some(d)) = (periode, derniere.periode) {
        if b != d {
            bulletin.push(dates(b));
            dsn.push(dates(d));
        }
    }
    if let (Some(b), Some(d)) = (contrat.nature, derniere.nature) {
        if b != d {
            bulletin.push(b.proprietes()?.libelle.clone());
            dsn.push(d.proprietes()?.libelle.clone());
        }
    }
    let tient = |m: socle::mois::Mois| periode.is_some_and(|o| o.debut().mois_civil() == m && o.fin().is_some_and(|f| f.mois_civil() == m));
    for (m, p) in declarees.iter().filter(|(m, _)| tient(*m)) {
        let mois = m.libelle(socle::mois::Graphie::Accentuee);
        let mut mesure = |b: Option<String>, d: Option<String>, mot: &str| {
            if let (Some(b), Some(d)) = (b, d) {
                if b != d {
                    bulletin.push(format!("{} {} en {}", b, mot, mois));
                    dsn.push(format!("{} {} en {}", d, mot, mois));
                }
            }
        };
        mesure(contrat.cachets.map(dit), p.cachets.map(dit), "cachet(s)");
        mesure(contrat.heures.map(dit), p.heures.map(dit), "heure(s)");
        mesure(contrat.jours.map(dit), p.jours.map(dit), "jour(s)");
    }
    Ok((bulletin, dsn))
}

/// UN DESACCORD, tel que la page le lit : le salarie, le debut du contrat, la phrase (« le bulletin dit …, la DSN
/// dit … ») et ses deux gestes (garder le bulletin, garder la DSN).
#[derive(Debug, serde::Serialize)]
pub(crate) struct DesaccordADire {
    salarie: IdSalarie,
    nom: String,
    debut: Jour,
    dit: Phrase,
    gestes: Vec<crate::imprevus::GesteDeRoute<ChargeDesaccord>>,
}

// Les libelles des deux gestes, en constantes : un texte fautif ne compile pas (`Affiche::new`).
const LE_BULLETIN: Affiche = Affiche::new("Garder le bulletin");
const LA_DSN: Affiche = Affiche::new("Garder la DSN");

/// LES DESACCORDS QUI ATTENDENT LA COMPAGNIE (voir le module), un par contrat. Un engagement valide, ou celui d'un
/// permanent (sa carte suit la DSN qui arrive : `nature::la_dsn_fait_foi`), n'en a pas.
pub(crate) fn en_attente(ctx: &Contexte) -> R<Vec<DesaccordADire>> {
    let mut out = Vec::new();
    for i in engagement::importe::tous(&ctx.racine)? {
        if i.objet().est_valide() || i.permanent() {
            continue;
        }
        let Some(lu) = i.contrat_lu()? else { continue };
        if lu.contrat.gardee.is_some() {
            continue;
        }
        let paies = i.paies_lues()?;
        let declarees: Vec<(socle::mois::Mois, &PaieVersee)> =
            paies.iter().filter(|x| engagement::importe::bulletin_avant_la_dsn(&lu.contrat, &x.paie)).map(|x| (x.mois, &x.paie)).collect();
        let (bulletin, dsn) = ce_qui_differe(lu.periode, &lu.contrat, &declarees)?;
        let (Some(periode), false) = (lu.periode, bulletin.is_empty()) else { continue };
        let nom = ctx.nomme(&lu.salarie);
        let debut = periode.debut();
        let dit = socle::phrase!(
            "{}, contrat du {} : le bulletin repris dit {}, la DSN ajoutée ensuite dit {}. Le bulletin est gardé tant que vous n'avez pas choisi.",
            nom,
            debut.jj_mm_aaaa(),
            bulletin.join(", "),
            dsn.join(", ")
        );
        let geste = |garder: PieceGardee, libelle: Affiche| {
            crate::imprevus::GesteDeRoute::sans_cases(Route::ImportationDesaccord, ChargeDesaccord { salarie: lu.salarie.clone(), debut, garder }, libelle)
        };
        let gestes = vec![geste(PieceGardee::Bulletin, LE_BULLETIN), geste(PieceGardee::Dsn, LA_DSN)];
        out.push(DesaccordADire { salarie: lu.salarie.clone(), nom, debut, dit, gestes });
    }
    Ok(out)
}

/// La charge d'`importation-desaccord` : le salarie, le debut du contrat repris, la piece a garder.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeDesaccord {
    salarie: IdSalarie,
    debut: Jour,
    garder: PieceGardee,
}

/// Ce qu'`importation-desaccord` rend : ce que l'ecran en dit.
#[derive(Debug, serde::Serialize)]
pub(crate) struct DesaccordTranche {
    phrase: Phrase,
}

impl crate::enveloppe::Dite for DesaccordTranche {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

/// `importation-desaccord` : la piece que la compagnie garde pour le contrat repris d'un bulletin du salarie qui commence
/// le jour dit. « La DSN » : l'engagement prend aussi les dates que la derniere paie qu'elle declare porte. Le meme
/// choix redit ne change rien. Refus, et rien n'est ecrit : aucun contrat repris d'un bulletin, non valide, de ce
/// salarie ne commence ce jour-la.
pub(crate) fn trancher(ctx: &Contexte, charge: &ChargeDesaccord) -> R<DesaccordTranche> {
    let nom = ctx.nomme(&charge.salarie);
    let mut trouve = false;
    engagement::importe::completer(&ctx.racine, |carte, pieces| {
        if carte.salarie.as_ref() != Some(&charge.salarie) || carte.debut != Some(charge.debut) {
            return false;
        }
        let Some(c) = pieces.contrat.as_mut().filter(|c| c.atteste_par == engagement::importe::Attestation::Bulletin) else { return false };
        trouve = true;
        c.gardee = Some(charge.garder);
        if charge.garder == PieceGardee::Dsn {
            if let Some(o) = pieces.paies.values().rev().filter(|p| !p.contrat.d_un_bulletin()).find_map(|p| p.periode) {
                (carte.debut, carte.fin) = (Some(o.debut()), o.fin());
            }
        }
        true
    })?;
    if !trouve {
        return Err(erreur(
            Classe::Refuse,
            format!("Aucun contrat repris d'un bulletin pour {} ne commence le {} : rien n'a été écrit.", nom, charge.debut.jj_mm_aaaa()),
        ));
    }
    let phrase = match charge.garder {
        PieceGardee::Bulletin => socle::phrase!("Contrat de {} du {} : le bulletin repris est gardé.", nom, charge.debut.jj_mm_aaaa()),
        PieceGardee::Dsn => socle::phrase!("Contrat de {} du {} : ce que la DSN déclare est gardé.", nom, charge.debut.jj_mm_aaaa()),
    };
    Ok(DesaccordTranche { phrase })
}
