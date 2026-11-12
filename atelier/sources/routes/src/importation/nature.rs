//! LA NATURE DES CONTRATS QUE LES BULLETINS N'ONT PAS DITE. L'import d'un bulletin de paie atteste un contrat
//! (un engagement importe, `importation::bulletins::engagement`) ; aucun gabarit n'imprime sa nature. Un bulletin
//! qui porte une marque du spectacle (numero d'objet, cotisation Congés Spectacles) atteste un CDDU
//! (`engagement::importe::Importe::nature`). Un bulletin qui n'en porte aucune est celui d'un permanent : CDI ou CDD de
//! droit commun, que rien ne dit. Son contrat n'a pas de `nature` ; ce module :
//!
//! - `a_dire` : les salaries dont un contrat repris attend sa nature, avec les natures qu'on peut lui donner
//!   (celles d'un permanent : `contrats-base.json`, paie « mensuelle ») et, pour chacune, ce qu'il faut dire avec elle
//!   (`demande`) -- `importation-etat` les rend, la page les demande apres l'import (Configuration, « Reprendre
//!   l'historique ») ;
//! - la route `importation-nature` `{"salarie", "nature", "debut"?, "fin"?, "numero"?}` : la nature dite s'ecrit sur
//!   l'engagement de chacun de ses contrats a dire, et la fiche Entreprise dit que la compagnie emploie des
//!   permanents. Une nature qui n'est pas d'un permanent, un salarie sans contrat a dire, ce que `demande` exige et qui
//!   manque : refus, rien n'est ecrit.
//!
//! LE CONTRAT QUI CONTINUE. Un permanent paye le dernier mois repris (le plus recent de l'historique) a un contrat qui
//! court encore, sauf s'il finissait ce mois-la. Le mois suivant le continue, et la DSN le declare sous le MEME numero
//! (S21.G00.40.009, « un des identifiants du contrat » ; CCH-12) et la meme date de debut (S21.G00.40.001) que
//! l'ancien logiciel : `atelier/reference/norme-dsn/texte/bloc-S21.G00.40.txt`. Aucun bulletin ne les imprime ; un
//! numero ne se recalcule pas, il se lit (regle 4) : sur la DSN deja deposee. Ils se demandent donc avec la nature
//! (`demande` : `debut`, `numero`, le salaire mensuel et la duree du travail du contrat, `fin` s'il a un terme), et
//! s'ecrivent sur l'engagement repris lui-meme (`debut`, `fin`, `numero`, `salaire_mensuel`, `quotite_contrat`) : il
//! continue ici, sous la meme identite (`engagement::importe`, « un contrat de permanent est un engagement »).

use droit::fiche::Lue as _;
use std::collections::BTreeMap;
use std::path::Path;


use droit::contexte::Contexte;
use engagement::importe::Importe;
use socle::date::Jour;
use socle::echec::{erreur, Classe, R};

/// Les natures qu'un permanent peut recevoir.
fn natures_permanentes() -> R<Vec<std::sync::Arc<droit::natures::Nature>>> {
    let toutes = droit::natures::natures().map_err(|m| m.reclasser(Classe::Refuse))?;
    Ok(toutes.iter().filter(|n| n.mensuelle()).cloned().collect())
}

/// Ce qu'une nature demande d'un contrat qui court encore : une case de la charge d'`importation-nature`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Demande {
    Debut,
    Fin,
    Numero,
    Salaire,
    Quotite,
}

/// Ce qu'une nature demande avec elle pour un contrat qui court encore : son debut, son numero, son salaire et
/// sa duree du travail, et sa fin s'il a un terme. Rien pour un contrat fini.
fn demande(n: &droit::natures::Nature, en_cours: bool) -> Vec<Demande> {
    use Demande::*;
    match (en_cours, n.duree_indeterminee) {
        (false, true) | (false, false) => vec![],
        (true, true) => vec![Debut, Numero, Salaire, Quotite],
        (true, false) => vec![Debut, Fin, Numero, Salaire, Quotite],
    }
}


/// Les engagements importes.
fn importes(racine: &Path) -> R<Vec<Importe>> {
    engagement::importe::tous(racine)
}

/// Le dernier mois repris : le plus recent que l'historique a verse (`AAAA-MM`) ; aucun sans paie reprise.
fn dernier_mois(importes: &[Importe]) -> Option<socle::mois::Mois> {
    importes.iter().flat_map(|i| i.paies().keys().copied()).max()
}

/// Le premier et le dernier jour que les paies d'un engagement importe paient : ceux du bulletin dans son mois, sinon le
/// mois entier. Rien pour un engagement sans paie.
fn jours_payes(i: &Importe) -> R<Option<(Jour, Jour)>> {
    let mut bornes = Vec::new();
    for (&mois, p) in i.paies() {
        let dans = |j: Option<Jour>| j.filter(|j| mois.contient(j));
        let debut = match dans(p.debut) {
            Some(j) => j,
            None => mois.premier_jour(),
        };
        let fin = match dans(p.fin) {
            Some(j) => j,
            None => mois.dernier_jour(),
        };
        bornes.push((debut, fin));
    }
    Ok(bornes.iter().map(|b| b.0).min().zip(bornes.iter().map(|b| b.1).max()))
}

/// Le nom d'un salarié à l'écran, tel que sa fiche le porte aujourd'hui ; sa clé quand aucune fiche ne le nomme.
fn nom_du_salarie(racine: &Path, cle: &str) -> R<String> {
    let lues = droit::fiche::lire_fichier::<droit::fiche::Salaries>(&racine.join(droit::contexte::SALARIES), droit::contexte::SALARIES)?;
    Ok(match lues.fiches.get(cle).and_then(|f| f.au(&socle::temps::jour_de_paris()).nomme()) {
        Some(n) => n,
        None => cle.to_string(),
    })
}

/// Les contrats a dire d'un salarie : combien, les jours que leurs paies paient, payes le dernier mois repris.
struct ContratsADire {
    nombre: usize,
    payes: Option<(Jour, Jour)>,
    en_cours: bool,
}

/// Les contrats a dire de chaque salarie.
fn a_dire_par_salarie(importes: &[Importe]) -> R<BTreeMap<String, ContratsADire>> {
    let dernier = dernier_mois(importes);
    let mut par: BTreeMap<String, ContratsADire> = BTreeMap::new();
    for i in importes.iter().filter(|i| i.a_dire()) {
        // Un engagement importe porte toujours son salarie.
        let Some(salarie) = i.objet().carte().salarie.as_ref().map(ToString::to_string) else { continue };
        let payes = jours_payes(i)?;
        let x = par.entry(salarie).or_insert(ContratsADire { nombre: 0, payes: None, en_cours: false });
        x.nombre += 1;
        x.payes = match (x.payes, payes) {
            (Some((a, b)), Some((c, d))) => Some((a.min(c), b.max(d))),
            (a, b) => a.or(b),
        };
        x.en_cours |= dernier.as_ref().is_some_and(|d| i.paies().contains_key(d));
    }
    Ok(par)
}

/// Les salaries dont des contrats repris attendent leur nature : `[{salarie, nom, contrats, du, au, en_cours, suite,
/// natures: [{cle, libelle, demande}]}]`, dans l'ordre des cles. `en_cours` : il est paye le dernier mois repris, son
/// contrat court peut-etre encore (`suite` : le mois qui suit).
pub(crate) fn a_dire(racine: &Path) -> R<Vec<ADire>> {
    let importes = importes(racine)?;
    let par = a_dire_par_salarie(&importes)?;
    if par.is_empty() {
        return Ok(Vec::new());
    }
    let suite = dernier_mois(&importes).map(socle::mois::Mois::suivant);
    let natures = natures_permanentes()?;
    let mut out = Vec::new();
    for (salarie, c) in par {
        let en_cours = c.en_cours;
        let natures = natures.iter().map(|x| NatureProposee { cle: x.cle, libelle: x.libelle.clone(), demande: demande(x, en_cours) }).collect();
        // Le mois de la derniere DSN qui l'a declare, tel que l'ecran le lit (« de novembre 2026 »).
        let mois_au = c.payes.map(|(_, au)| au.mois_civil());
        let au_dit: Option<String> = mois_au.map(crate::regularisation::de_mois_en_clair);
        let nom = nom_du_salarie(racine, &salarie)?;
        let periode: String = c.payes.iter().map(|(du, au)| format!(", du {} au {}", du.jj_mm_aaaa(), au.jj_mm_aaaa())).collect();
        let dit = format!(
            "{} contrat{} repris{} : ses bulletins n'ont pas de Congés Spectacles ni de numéro d'objet, c'est un contrat de permanent.",
            c.nombre,
            if c.nombre > 1 { "s" } else { "" },
            periode
        );
        let en_cours_dit = match (en_cours, mois_au, suite) {
            (true, Some(au), Some(suite)) => Some(format!(
                "Payé jusqu'en {} : son contrat continue en {}, et sa DSN le déclare avec la date de début et le numéro que votre ancien logiciel lui a donnés. Ils sont sur sa dernière DSN (bloc Contrat), pas sur le bulletin.",
                au.libelle(socle::mois::Graphie::Accentuee),
                suite.libelle(socle::mois::Graphie::Accentuee)
            )),
            _ => None,
        };
        out.push(ADire {
            numero_placeholder: match &au_dit {
                Some(a) => format!("sur la DSN {}", a),
                None => "sur sa dernière DSN".to_string(),
            },
            choix_dit: format!("Nature du contrat de {}", nom),
            salarie,
            nom,
            contrats: c.nombre,
            du: c.payes.map(|p| p.0),
            au: c.payes.map(|p| p.1),
            au_dit,
            en_cours,
            suite: suite.filter(|_| en_cours).map(|m| m.to_string()),
            natures,
            dit,
            en_cours_dit,
        });
    }
    Ok(out)
}

/// Les contrats d'un salarie dont la nature reste a dire, et ce que la page en dit.
#[derive(Debug, serde::Serialize)]
pub(crate) struct ADire {
    pub(crate) salarie: String,
    pub(crate) nom: String,
    pub(crate) contrats: usize,
    /// Le premier et le dernier jour que ses paies reprises paient ; rien sans paie reprise.
    pub(crate) du: Option<Jour>,
    pub(crate) au: Option<Jour>,
    pub(crate) au_dit: Option<String>,
    /// Paye le dernier mois repris : son contrat court peut-etre encore (`suite` : le mois qui suit).
    pub(crate) en_cours: bool,
    pub(crate) suite: Option<String>,
    pub(crate) natures: Vec<NatureProposee>,
    pub(crate) numero_placeholder: String,
    pub(crate) choix_dit: String,
    pub(crate) dit: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) en_cours_dit: Option<String>,
}

/// Une nature de permanent qui peut se dire, et ce qu'elle demande d'un contrat qui court encore.
#[derive(Debug, serde::Serialize)]
pub(crate) struct NatureProposee {
    pub(crate) cle: droit::natures::NatureDeContrat,
    pub(crate) libelle: String,
    pub(crate) demande: Vec<Demande>,
}

fn refus(m: impl Into<String>) -> socle::echec::Echec {
    erreur(Classe::Refuse, m)
}

/// La charge d'`importation-nature` : le salarie, la nature, et ce que `demande` exige d'un contrat qui court encore.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeNature {
    pub(crate) salarie: String,
    pub(crate) nature: droit::natures::NatureDeContrat,
    pub(crate) debut: Option<Jour>,
    pub(crate) fin: Option<Jour>,
    pub(crate) numero: Option<String>,
    pub(crate) salaire: Option<MontantSaisi>,
    pub(crate) quotite: Option<MontantSaisi>,
}

/// Un montant tel qu'il se saisit : un texte (« 1 850,00 ») ou un nombre.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(untagged)]
pub(crate) enum MontantSaisi {
    Texte(String),
    Nombre(socle::forme::NombreLu),
}

impl MontantSaisi {
    fn texte(&self) -> String {
        match self {
            MontantSaisi::Texte(t) => t.clone(),
            MontantSaisi::Nombre(n) => n.to_string(),
        }
    }
}

impl ChargeNature {
    /// Une case de texte demandee (le numero, le salaire, la quotite), telle qu'elle est dite, sans ses blancs ; rien
    /// pour une case vide. Un jour n'est pas un texte : il se lit dans sa case (`debut`, `fin`).
    fn lu(&self, champ: Demande) -> Option<String> {
        let v = match champ {
            Demande::Debut | Demande::Fin => None,
            Demande::Numero => self.numero.clone(),
            Demande::Salaire => self.salaire.as_ref().map(MontantSaisi::texte),
            Demande::Quotite => self.quotite.as_ref().map(MontantSaisi::texte),
        };
        v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
    }
}

/// Ce qu'`importation-nature` rend : les contrats ecrits, ce qui reste a dire, et ce que l'ecran en dit.
#[derive(Debug, serde::Serialize)]
pub(crate) struct NatureDite {
    pub(crate) phrase: String,
    pub(crate) ecrits: usize,
    pub(crate) a_dire: Vec<ADire>,
    /// La fiche Entreprise vient de dire que la compagnie emploie des permanents.
    pub(crate) permanents_declares: bool,
    /// Les contrats de prevoyance et de sante des permanents que les DSN reprises declarent, poses.
    pub(crate) parametrage: Vec<String>,
    pub(crate) couleur: &'static str,
}

/// `importation-nature` : la nature dite pour les contrats a dire d'un salarie, et, s'il court encore, le debut, la fin
/// et le numero du contrat tels que la DSN de l'ancien logiciel les a declares.
pub(crate) fn dire(ctx: &Contexte, charge: &ChargeNature) -> R<NatureDite> {
    let plein = |s: &str| Some(s.trim().to_string()).filter(|s| !s.is_empty());
    let salarie = plein(&charge.salarie).ok_or_else(|| refus("Le salarié manque : rien n'a été écrit."))?;
    let nature = charge.nature;
    let permises = natures_permanentes()?;
    let Some(nat) = permises.iter().find(|n| n.cle == nature) else {
        let noms: Vec<&str> = permises.iter().map(|n| n.libelle.as_str()).collect();
        return Err(refus(format!("« {} » n'est pas la nature d'un contrat de permanent ({}) : rien n'a été écrit.", nature, noms.join(", "))));
    };
    let importes = importes(&ctx.racine)?;
    let qui = nom_du_salarie(&ctx.racine, &salarie)?;
    let Some(c) = a_dire_par_salarie(&importes)?.remove(&salarie) else {
        return Err(refus(format!("Aucun contrat repris de {} n'attend sa nature : rien n'a été écrit.", qui)));
    };
    let en_cours = c.en_cours;
    // Les jours que ses bulletins repris paient : un contrat a dire en a toujours (il vient d'un bulletin).
    let Some((du, au)) = c.payes else {
        return Err(refus(format!("Aucune paie reprise de {} ne dit quand ses contrats ont été payés : rien n'a été écrit.", qui)));
    };
    let dernier = dernier_mois(&importes);
    // Le contrat qui court encore : son debut, sa fin s'il a un terme, son numero -- lus sur la DSN deja deposee ; ce
    // qui s'en ecrit sur l'engagement repris.
    let mut contrat = engagement::saisie::Carte::default();
    let mut debut_dit: Option<Jour> = None;
    for champ in demande(nat, en_cours) {
        let manque = || {
            refus(match champ {
                Demande::Debut => format!("La date de début du contrat de {} manque (celle que la DSN déclare) : rien n'a été écrit.", qui),
                Demande::Fin => format!("La date de fin prévue du contrat de {} manque : rien n'a été écrit.", qui),
                Demande::Salaire => format!("Le salaire mensuel brut du contrat de {} manque (celui du contrat signé) : rien n'a été écrit.", qui),
                Demande::Quotite => format!(
                    "La durée du travail du contrat de {} manque (en heures par mois, celle du contrat signé) : rien n'a été écrit.",
                    qui
                ),
                Demande::Numero => format!("Le numéro du contrat de {} manque (celui que la DSN déclare) : rien n'a été écrit.", qui),
            })
        };
        match champ {
            Demande::Debut | Demande::Fin => {
                let jour = (if champ == Demande::Debut { charge.debut } else { charge.fin }).ok_or_else(manque)?;
                if champ == Demande::Debut && jour > du {
                    return Err(refus(format!(
                        "Le contrat de {} ne peut pas commencer le {} : un bulletin le paie dès le {}. Rien n'a été écrit.",
                        qui,
                        jour.iso(),
                        du.iso()
                    )));
                }
                if champ == Demande::Fin && jour < au {
                    return Err(refus(format!("Le contrat de {} ne peut pas finir le {} : un bulletin le paie jusqu'au {}. Rien n'a été écrit.", qui, jour.iso(), au.iso())));
                }
                if champ == Demande::Debut {
                    debut_dit = Some(jour);
                }
                *(if champ == Demande::Debut { &mut contrat.debut } else { &mut contrat.fin }) = Some(jour);
            }
            Demande::Salaire | Demande::Quotite => {
                let v = charge.lu(champ).ok_or_else(manque)?;
                let (cle, quoi) = if champ == Demande::Salaire {
                    (&mut contrat.salaire_mensuel, "un salaire mensuel brut")
                } else {
                    (&mut contrat.quotite_contrat, "une durée du travail en heures par mois")
                };
                match socle::num::decimal_a_virgule(&v).filter(|d| *d > socle::num::Decimal::ZERO).map(socle::forme::NombreLu::de_decimal) {
                    Some(n) => *cle = Some(n),
                    None => return Err(refus(format!("« {} » n'est pas {} : rien n'a été écrit.", v, quoi))),
                };
            }
            Demande::Numero => {
                let v = charge.lu(champ).ok_or_else(manque)?;
                let n = engagement::numeros::NumeroContrat::lire(&v).map_err(|m| m.reformuler(|p| format!("{} Rien n'a été écrit.", p)).reclasser(Classe::Refuse))?;
                contrat.numero = Some(n);
            }
        }
    }
    // La nature va sur l'engagement de chaque contrat a dire ; ce qui definit le contrat qui court encore, sur le sien.
    let ecrits = engagement::importe::completer(&ctx.racine, |e, pieces| {
        let a_dire = pieces.contrat.as_ref().is_some_and(|c| c.nature.is_none());
        if e.salarie.as_deref() != Some(salarie.as_str()) || !a_dire || e.nature.is_some() {
            return false;
        }
        e.nature = Some(nature);
        if dernier.as_ref().is_some_and(|d| pieces.paies.contains_key(d)) {
            for k in engagement::saisie::Champ::TOUS.iter().copied().filter(|k| contrat.dit(*k)) {
                if k != engagement::saisie::Champ::Numero || e.numero.is_none() {
                    e.prendre(&contrat, k);
                }
            }
        }
        true
    })?;
    // Son anciennete dans l'entreprise part du debut de son contrat, s'il a commence avant le premier bulletin repris.
    if let Some(debut) = debut_dit {
        crate::fiches::reprendre_anciennete(ctx, &salarie, du, debut)?;
    }
    // LA COMPAGNIE EMPLOIE UN PERMANENT : c'est ce qu'on vient de dire (une nature payee au mois). La case « La
    // compagnie emploie aussi des salaries au mois » de la fiche Entreprise (`Contexte::emploie_des_permanents_pendant`)
    // se coche donc ici, sans la redemander, a partir du debut du contrat (sinon du premier jour que les bulletins repris
    // paient) : un contrat paye au mois qui commence sans elle se refuse (`engagement::engagements`), et ce qu'elle
    // ouvre (les contrats de prevoyance et de sante des permanents) se demande alors dans « Pour demarrer », comme pour
    // toute compagnie qui la coche.
    let depuis = match debut_dit {
        Some(debut) => debut,
        None => du,
    };
    let permanents_declares = !ctx.emploie_des_permanents_pendant(&depuis, &depuis);
    if permanents_declares {
        crate::fiches::poser_entreprise(ctx, droit::fiche::ChangementEntreprise::EmploieDesPermanents(Some(true)), Some(depuis))?;
    }
    // Les contrats de prevoyance et de sante des permanents que l'ecran attend maintenant : ceux que les DSN reprises
    // declarent (S21.G00.15) se posent, lus sous la fiche telle qu'elle vient d'etre ecrite.
    let parametrage = if permanents_declares {
        let frais = Contexte::nouveau(&ctx.racine, Some(ctx.date))?;
        crate::importation::parametrage::poser_organismes_repris(&frais)?
    } else {
        Vec::new()
    };
    // Ce que l'ecran dit du geste, et sa couleur : la nature est ecrite, c'est fait ; ce qui reste a dire pour un
    // permanent est une ligne de « Pour demarrer », qui a la sienne.
    let mut phrase = format!("Nature enregistrée sur {} contrat{} repris.", ecrits, if ecrits > 1 { "s" } else { "" });
    if permanents_declares {
        phrase.push_str(" La compagnie emploie donc des salariés au mois : « Pour démarrer » dit ce que cela demande encore.");
    }
    for p in &parametrage {
        phrase.push(' ');
        phrase.push_str(p);
    }
    Ok(NatureDite { phrase, ecrits, a_dire: a_dire(&ctx.racine)?, permanents_declares, parametrage, couleur: socle::ecran::couleur_de_l_etat(socle::ecran::EtatDEcran::Fait) })
}

impl crate::enveloppe::Dite for NatureDite {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

/// CE QUE LA DSN REPRISE DIT DEJA NE SE DEMANDE PAS. Pour chaque salarie dont des contrats repris attendent leur nature
/// (`a_dire`), le contrat que sa derniere DSN reprise declare la donne (S21.G00.40.007, la nature du contrat ;
/// `norme-dsn/texte/bloc-S21.G00.40.txt`), avec ce que `demande` exige d'un contrat qui court encore : le debut (.001),
/// le numero (.009), la duree du travail (.013), la fin s'il a un terme (.010) et le salaire du contrat, « la remuneration
/// brute habituelle du salarie » (S21.G00.51, type 010 ; `bloc-S21.G00.51.txt`). Tout y est : la nature se dit comme a
/// l'ecran (`dire`). Il en manque, ou la nature declaree n'est pas d'un permanent : rien ne se choisit, l'ecran le
/// demande. Rend ce que chaque nature dite a fait, en phrases ; une nature lue que l'ecriture refuse se dit aussi.
pub(crate) fn dire_depuis_les_dsn(racine: &Path, quand: &Jour) -> R<Vec<String>> {
    let lecture = import::dsn_aem::lire_dsn(&racine.join("historique").join("bulletins"))?;
    let permises = natures_permanentes()?;
    let mut faits = Vec::new();
    for x in a_dire(racine)? {
        let salarie = x.salarie.clone();
        let ctx = Contexte::nouveau(racine, Some(*quand))?;
        let Some(nir) = ctx.salaries.get(salarie.as_str()).and_then(|f| f.nir.en_texte()) else { continue };
        // Sa derniere paie declaree : le contrat tel que la derniere DSN le dit.
        let Some(((_, _, numero), p)) = lecture.paies.iter().filter(|((_, n, _), _)| n.as_deref() == Some(nir.as_str())).last() else { continue };
        let c = p.contrat.as_ref();
        let Some(code) = c.and_then(|c| c.nature_dsn.clone()) else { continue };
        let Some(nat) = permises.iter().find(|n| n.code("S21.G00.40.007").as_deref() == Some(code.as_str())) else { continue };
        let en_cours = x.en_cours;
        let mut charge = ChargeNature { salarie: salarie.clone(), nature: nat.cle, debut: None, fin: None, numero: None, salaire: None, quotite: None };
        let mut complet = true;
        for champ in demande(nat, en_cours) {
            let pose = match champ {
                Demande::Numero => Some(charge.numero = Some(numero.to_string())),
                Demande::Salaire => p.paie.mesures.salaire_habituel.map(|m| charge.salaire = Some(MontantSaisi::Texte(m.to_string()))),
                Demande::Debut => c.and_then(|c| c.debut).map(|j| charge.debut = Some(j)),
                Demande::Fin => c.and_then(|c| c.fin).map(|j| charge.fin = Some(j)),
                Demande::Quotite => c.and_then(|c| c.quotite).map(|q| charge.quotite = Some(MontantSaisi::Texte(q.to_string()))),
            };
            complet &= pose.is_some();
        }
        if !complet {
            continue;
        }
        let nom = nom_du_salarie(racine, &salarie)?;
        match dire(&ctx, &charge) {
            Ok(r) => faits.push(format!("{} : {}, lu dans sa DSN reprise. {}", nom, nat.libelle, r.phrase)),
            Err(e) => faits.push(format!("{} : {}, lu dans sa DSN reprise, ne s'écrit pas. {}", nom, nat.libelle, e.phrase)),
        }
    }
    Ok(faits)
}
