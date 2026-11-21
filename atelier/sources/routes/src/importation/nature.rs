//! LA NATURE DES CONTRATS QUE LES BULLETINS N'ONT PAS DITE. L'import d'un bulletin de paie atteste un contrat
//! (un engagement importe, `importation::bulletins::engagement`) ; aucun gabarit n'imprime sa nature. Un bulletin
//! qui porte une marque du spectacle (numero d'objet, cotisation Congés Spectacles) atteste un CDDU
//! (`engagement::importe::Importe::nature`). Un bulletin qui n'en porte aucune est celui d'un permanent : CDI ou CDD de
//! droit commun, que rien ne dit. Un contrat que ses seules DSN declarent, sans le motif de recours qui designerait sa
//! nature (S21.G00.40.021), reste aussi a dire. Son contrat n'a pas de `nature` ; ce module :
//!
//! - `a_dire` : les salaries dont un contrat repris attend sa nature, avec les natures que ses pieces permettent
//!   (`natures_permises` : celles d'un permanent pour un bulletin sans marque du spectacle, celles qui ecrivent ce que
//!   la DSN declare) et, pour chacune, ce qu'il faut dire avec elle (`demande`) -- `importation-etat` les rend, la page
//!   les demande apres l'import (Configuration, « Reprendre l'historique ») ;
//! - la route `importation-nature` `{"salarie", "nature", "debut"?, "fin"?, "numero"?}` : la nature dite s'ecrit sur
//!   l'engagement de chacun de ses contrats a dire, et, payee au mois, la fiche Entreprise dit que la compagnie emploie
//!   des permanents. Ce que la charge tait se prend sur la derniere DSN reprise du salarie (`DeclareParLaDsn`). Une
//!   nature que ses pieces ne permettent pas, un salarie sans contrat a dire, ce que `demande` exige et que ni la charge
//!   ni la DSN ne disent : refus, rien n'est ecrit.
//!
//! LE CONTRAT QUI CONTINUE. Un permanent paye le dernier mois repris (le plus recent de l'historique) a un contrat qui
//! court encore, sauf s'il finissait ce mois-la. Le mois suivant le continue, et la DSN le declare sous le MEME numero
//! (S21.G00.40.009, « un des identifiants du contrat » ; CCH-12) et la meme date de debut (S21.G00.40.001) que
//! l'ancien logiciel : `atelier/reference/norme-dsn/texte/bloc-S21.G00.40.txt`. Aucun bulletin ne les imprime ; un
//! numero ne se recalcule pas, il se lit (regle 4) : sur la DSN deja deposee, et ce qu'elle declare ne se demande pas.
//! Le reste se demande avec la nature (`demande` : `debut`, `numero`, le salaire mensuel et la duree du travail du
//! contrat, `fin` s'il a un terme), et
//! s'ecrivent sur l'engagement repris lui-meme (`debut`, `fin`, `numero`, `salaire_mensuel`, `quotite_contrat`) : il
//! continue ici, sous la meme identite (`engagement::importe`, « un contrat de permanent est un engagement »).

use droit::fiche::Lue as _;
use std::collections::BTreeMap;
use std::path::Path;


use droit::contexte::{Contexte, IdSalarie};
use engagement::importe::Importe;
use socle::date::{Jour, Periode};
use socle::echec::{erreur, Classe, R};

/// Les natures qu'un permanent peut recevoir.
fn natures_permanentes() -> R<Vec<std::sync::Arc<droit::natures::Nature>>> {
    let toutes = droit::natures::natures().map_err(|m| m.reclasser(Classe::Refuse))?;
    Ok(toutes.iter().filter(|n| n.mensuelle()).cloned().collect())
}

/// LES NATURES QUE LES CONTRATS A DIRE DU SALARIE `salarie` PEUVENT RECEVOIR : celles que tout ce que leurs pieces en
/// disent permet, pour chacun d'eux. Une nature se dit, elle ne se devine pas (regle 6) : on ne propose que celles que les
/// pieces ne contredisent pas, et la compagnie dit la sienne.
/// - un bulletin sans marque du spectacle (ni numero d'objet ni Congés Spectacles) atteste le contrat d'un permanent
///   (`ContratAtteste::nature` vide) : une nature payee au mois ;
/// - la DSN deposee declare sa nature du contrat (S21.G00.40.007) : une nature qui ecrit ce code ; avec son motif de
///   recours (.021) quand elle le declare : une nature qu'elle designe avec lui (`Nature::declaree_par` ;
///   `norme-dsn/texte/bloc-S21.G00.40.txt`, `contrats-base.json`) ;
/// - la DSN deposee declare, ou le bulletin imprime, un numero d'objet (S21.G00.40.050, `PaieVersee::numero_objet`) :
///   une nature qui ne l'interdit pas (`contrats-base.json`, `interdits`).
///
/// Une DSN reprise qui ne se relit plus ne dit pas son code : seul ce que la paie reprise garde (le numero d'objet) trie.
fn natures_permises(ctx: &Contexte, importes: &[Importe], salarie: &IdSalarie, dsn: &DsnReprises) -> R<Vec<std::sync::Arc<droit::natures::Nature>>> {
    let toutes = droit::natures::natures().map_err(|m| m.reclasser(Classe::Refuse))?;
    let nir = ctx.salaries.get(salarie).and_then(|f| f.nir.dite().cloned());
    let mut permises: Vec<std::sync::Arc<droit::natures::Nature>> = toutes.iter().cloned().collect();
    for i in importes.iter().filter(|i| i.a_dire() && i.objet().carte().salarie.as_ref() == Some(salarie)) {
        if i.contrat().is_some() {
            permises.retain(|n| n.mensuelle());
        }
        for (mois, p) in i.paies() {
            if p.numero_objet.is_some() {
                permises.retain(|n| n.interdit(droit::rubrique!("S21.G00.40.050")).is_none());
            }
            let (Some(nir), Some(numero)) = (&nir, p.contrat.numero_declare()) else { continue };
            let Ok(lecture) = dsn.lue()? else { continue };
            let Some(c) = lecture.paies.get(&(*mois, Some(nir.clone()), numero.clone())).and_then(|d| d.contrat.as_ref()) else { continue };
            if let Some(code) = c.nature_dsn.as_deref() {
                permises.retain(|n| n.code(droit::rubrique!("S21.G00.40.007")).as_deref() == Some(code) && (c.motif_recours.is_none() || n.declaree_par(code, c.motif_recours)));
            }
        }
    }
    Ok(permises)
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
/// sa duree du travail, et sa fin s'il a un terme. Rien pour un contrat fini, ni pour un contrat qui n'est pas paye au
/// mois : un contrat d'usage que la DSN declare porte deja ses dates et son numero (S21.G00.40.001, .010, .009), et
/// n'a ni salaire mensuel ni duree du travail par mois.
fn demande(n: &droit::natures::Nature, en_cours: bool) -> Vec<Demande> {
    use Demande::*;
    match (en_cours && n.mensuelle(), n.duree_indeterminee) {
        (false, true) | (false, false) => vec![],
        (true, true) => vec![Debut, Numero, Salaire, Quotite],
        (true, false) => vec![Debut, Fin, Numero, Salaire, Quotite],
    }
}

/// Les DSN reprises (le dossier de l'import), lues une fois par geste, a la premiere qui en a besoin.
struct DsnReprises<'a> {
    racine: &'a Path,
    /// Lues, ou la phrase qui dit pourquoi elles ne se relisent pas (un manque, pas une panne).
    lue: std::cell::OnceCell<Result<import::dsn_aem::LectureDsn, socle::texte::Phrase>>,
}

impl<'a> DsnReprises<'a> {
    fn de(racine: &'a Path) -> Self {
        DsnReprises { racine, lue: std::cell::OnceCell::new() }
    }

    /// LA LECTURE REPOND (« Le logiciel est doux », comme `importation::identite::en_attente`) : une DSN reprise qui ne
    /// se relit plus ne declare rien, et ce qu'elle declarait se demande ; seule l'erreur qui dit un manque
    /// (`Echec::dit_un_manque`) le devient, une panne (le disque) remonte.
    fn lue(&self) -> R<&Result<import::dsn_aem::LectureDsn, socle::texte::Phrase>> {
        if let Some(l) = self.lue.get() {
            return Ok(l);
        }
        let l = match import::dsn_aem::lire_dsn(&import::mois::dossier_des_bulletins(self.racine)) {
            Ok(l) => Ok(l),
            Err(e) if e.dit_un_manque() => Err(socle::phrase!("Les DSN reprises ne se relisent pas : {} Ce qu'elles déclarent du contrat se demande donc.", e.phrase)),
            Err(e) => return Err(e),
        };
        Ok(self.lue.get_or_init(|| l))
    }
}

/// LE CONTRAT A DIRE QUE LA DSN DECLARE, retrouve par ce qui l'identifie, comme `la_dsn_fait_foi` : le numero sous lequel
/// une DSN deposee a declare une paie d'un contrat a dire du salarie (`CleDePaieVersee::numero_declare`, la paie que
/// l'import a rattachee a sa DSN), au dernier mois ou elle en declare une. Ce mois-la, deux numeros (deux contrats a dire
/// declares), ou aucun (aucune paie rattachee) : rien n'identifie le contrat, la DSN n'en declare rien.
fn contrat_identifie<'i>(importes: &'i [Importe], salarie: &IdSalarie) -> Option<(socle::mois::Mois, &'i engagement::numeros::NumeroContrat)> {
    let declares: Vec<(socle::mois::Mois, &engagement::numeros::NumeroContrat)> = importes
        .iter()
        .filter(|i| i.a_dire() && i.objet().carte().salarie.as_ref() == Some(salarie))
        .flat_map(|i| i.paies().iter().filter_map(|(m, p)| Some((*m, p.contrat.numero_declare()?))))
        .collect();
    let dernier = declares.iter().map(|(m, _)| *m).max()?;
    let mut du_dernier = declares.iter().filter(|(m, _)| *m == dernier).map(|(_, n)| *n);
    let numero = du_dernier.next()?;
    du_dernier.all(|n| n == numero).then_some((dernier, numero))
}

/// CE QUE LA DERNIERE DSN REPRISE D'UN SALARIE DECLARE DU CONTRAT A DIRE (`contrat_identifie`) : le contrat tel qu'elle
/// le declare (`ContratDeclare`, `norme-dsn/texte/bloc-S21.G00.40.txt` : le debut .001, la fin s'il a un terme .010, le
/// numero .009, la duree du travail .013), et ce qui en est propre ici : la nature (S21.G00.40.007, celle du seul
/// permanent qui porte ce code) et le salaire du contrat, « la remuneration brute habituelle du salarie » (S21.G00.51,
/// type 010 ; `bloc-S21.G00.51.txt`), d'un mois que la paie et le contrat couvrent entier
/// (`Mesures::salaire_habituel_du_mois`, `couvre_le_mois`) ; sinon, pourquoi il ne s'y lit pas.
struct DeclareParLaDsn {
    contrat: declarations::dsn_lue::ContratDeclare,
    nature: Option<std::sync::Arc<droit::natures::Nature>>,
    salaire: Result<socle::grandeurs::MontantUnitaire, &'static str>,
}

impl DeclareParLaDsn {
    /// Ce que la derniere DSN reprise du salarie `salarie` declare du contrat a dire ; rien (`Ok(None)`) quand sa fiche
    /// n'a pas de NIR (un salarie ne se rattache a sa DSN que par lui), que rien n'identifie le contrat, ou qu'aucune DSN
    /// reprise ne le declare ; la phrase qui le dit (`Err`) quand les DSN reprises ne se relisent pas.
    fn de(
        dsn: &DsnReprises,
        ctx: &Contexte,
        importes: &[Importe],
        salarie: &IdSalarie,
        permises: &[std::sync::Arc<droit::natures::Nature>],
    ) -> R<Result<Option<Self>, socle::texte::Phrase>> {
        let Some(nir) = ctx.salaries.get(salarie).and_then(|f| f.nir.dite().cloned()) else { return Ok(Ok(None)) };
        let Some((mois, numero)) = contrat_identifie(importes, salarie) else { return Ok(Ok(None)) };
        let lecture = match dsn.lue()? {
            Ok(l) => l,
            Err(p) => return Ok(Err(p.clone())),
        };
        let Some(p) = lecture.paies.get(&(mois, Some(nir), numero.clone())) else { return Ok(Ok(None)) };
        let Some(c) = p.contrat.as_ref() else { return Ok(Ok(None)) };
        // La nature et le motif de recours que la DSN declare ensemble (`Nature::declaree_par`) : un contrat d'usage, de
        // nature 02 comme un CDD, n'en recoit pas la nature.
        let du_code: Vec<&std::sync::Arc<droit::natures::Nature>> = match c.nature_dsn.as_deref() {
            Some(code) => permises.iter().filter(|n| n.declaree_par(code, c.motif_recours)).collect(),
            None => Vec::new(),
        };
        // Le salaire de base d'un mois que la paie couvre entier, sous un contrat qui le couvre entier : le montant d'une
        // unite, le mois. D'une partie du mois, il n'en est que la part : il se demande, et chaque cause se dit.
        let salaire = if !couvre_le_mois(c, mois) {
            Err(" Son salaire mensuel ne s'y lit pas : le contrat déclaré ne couvre pas tout le mois de sa dernière paie déclarée.")
        } else {
            match p.paie.mesures.salaire_habituel_du_mois(mois) {
                Some(m) => Ok(m.pour_une_unite()),
                None => Err(" Son salaire mensuel ne s'y lit pas : sa dernière paie déclarée ne le paie pas sur un mois entier, d'une seule ligne de salaire de base."),
            }
        };
        Ok(Ok(Some(DeclareParLaDsn {
            contrat: c.clone(),
            nature: match du_code.as_slice() {
                [n] => Some(std::sync::Arc::clone(n)),
                _ => None,
            },
            salaire,
        })))
    }

    fn debut(&self) -> Jour {
        self.contrat.periode.debut()
    }

    fn fin(&self) -> Option<Jour> {
        self.contrat.periode.fin()
    }

    fn numero(&self) -> Option<&engagement::numeros::NumeroContrat> {
        self.contrat.numero.as_ref()
    }

    fn salaire(&self) -> Option<socle::grandeurs::MontantUnitaire> {
        self.salaire.ok()
    }

    /// La duree d'un contrat au mois : la quotite que la DSN declare en heures.
    fn quotite(&self) -> Option<socle::grandeurs::Heures> {
        match self.contrat.quotite {
            Some(droit::natures::QuotiteDuContrat::Heures(h)) => Some(h),
            Some(droit::natures::QuotiteDuContrat::Jours(_)) | None => None,
        }
    }

    /// La DSN declare-t-elle ce que la nature demande ?
    fn declare(&self, champ: Demande) -> bool {
        match champ {
            Demande::Debut => true,
            Demande::Fin => self.fin().is_some(),
            Demande::Numero => self.numero().is_some(),
            Demande::Salaire => self.salaire().is_some(),
            Demande::Quotite => self.quotite().is_some(),
        }
    }

    /// Ce qu'elle declare, en clair : « CDI, debut le 01/03/2022, numero CH1.22.01, 151,67 heures par mois ».
    fn en_clair(&self) -> String {
        let mut dit: Vec<String> = Vec::new();
        if let Some(n) = &self.nature {
            dit.push(n.libelle.clone());
        }
        dit.push(format!("début le {}", self.debut().jj_mm_aaaa()));
        if let Some(f) = self.fin() {
            dit.push(format!("fin le {}", f.jj_mm_aaaa()));
        }
        if let Some(n) = self.numero() {
            dit.push(format!("numéro {}", n));
        }
        if let Some(h) = self.quotite() {
            dit.push(format!("{} heures par mois", socle::grandeurs::ecrit::sans_zero::dit(h)));
        }
        if let Some(s) = self.salaire() {
            dit.push(format!("salaire mensuel de {} €", socle::grandeurs::ecrit::sans_zero::dit(s)));
        }
        dit.join(", ")
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
fn jours_payes(i: &Importe) -> R<Option<Periode>> {
    let mut toutes: Option<Periode> = None;
    for (&mois, p) in i.paies() {
        let le_mois = Periode::du_mois(mois);
        let payee = match p.periode.and_then(|o| o.croise(&le_mois)) {
            Some(x) => x,
            None => le_mois,
        };
        toutes = Some(match toutes {
            Some(t) => reunies(t, payee)?,
            None => payee,
        });
    }
    Ok(toutes)
}

/// Les jours que deux periodes payees couvrent, du premier au dernier.
fn reunies(a: Periode, b: Periode) -> R<Periode> {
    Periode::du_au(a.debut().min(b.debut()), a.fin().max(b.fin()))
}

/// Le nom d'un salarié à l'écran, tel que sa fiche le porte aujourd'hui ; sa clé quand aucune fiche ne le nomme.
fn nom_du_salarie(racine: &Path, cle: &IdSalarie) -> R<String> {
    let lues = droit::fiche::lire_fiche_posee::<droit::fiche::Salaries>(&droit::contexte::SALARIES.dans(racine), droit::fiche::NomDeFiche::Salarie)?;
    Ok(match lues.fiches.get(cle).and_then(|f| f.au(&socle::temps::jour_de_paris()).nomme()) {
        Some(n) => n,
        None => cle.to_string(),
    })
}

/// Les contrats a dire d'un salarie : combien, les jours que leurs paies paient, payes le dernier mois repris.
struct ContratsADire {
    nombre: usize,
    /// Ceux qu'un bulletin atteste (sans marque du spectacle).
    de_permanent: usize,
    /// Ceux qu'aucune piece n'atteste et dont aucune DSN ne declare une paie : leurs paies viennent de bulletins seuls.
    /// Les autres, leurs DSN les declarent.
    par_bulletin: usize,
    payes: Option<Periode>,
    en_cours: bool,
}

impl ContratsADire {
    /// CE QUE LA PAGE DIT DE SES CONTRATS A DIRE, d'ou ils viennent : un bulletin sans marque du spectacle, celui d'un
    /// permanent ; des bulletins dont aucun contrat n'a ete atteste ; une DSN dont ce qu'elle declare ne suffit pas a dire
    /// la nature (sans motif de recours, S21.G00.40.021).
    fn dit(&self) -> String {
        let pluriel = |n: usize| if n > 1 { "s" } else { "" };
        let periode: String = self.payes.iter().map(|p| format!(" ; payé{} du {} au {}", pluriel(self.nombre), p.debut().jj_mm_aaaa(), p.fin().jj_mm_aaaa())).collect();
        let par_dsn = self.nombre - self.de_permanent - self.par_bulletin;
        let mut dit = Vec::new();
        if self.de_permanent > 0 {
            dit.push(format!(
                "{} contrat{} repris : ses bulletins n'ont pas de Congés Spectacles ni de numéro d'objet, c'est un contrat de permanent",
                self.de_permanent,
                pluriel(self.de_permanent)
            ));
        }
        if self.par_bulletin > 0 {
            dit.push(format!("{} contrat{} repris de ses bulletins, que l'import n'a pas su rattacher à un contrat (son compte rendu dit pourquoi) : sa nature reste à dire", self.par_bulletin, pluriel(self.par_bulletin)));
        }
        if par_dsn > 0 {
            dit.push(format!("{} contrat{} repris de ses seules DSN : ce qu'elles en déclarent ne suffit pas à dire sa nature", par_dsn, pluriel(par_dsn)));
        }
        format!("{}{}.", dit.join(" ; "), periode)
    }
}

/// Les contrats a dire de chaque salarie.
fn a_dire_par_salarie(importes: &[Importe]) -> R<BTreeMap<IdSalarie, ContratsADire>> {
    let dernier = dernier_mois(importes);
    let mut par: BTreeMap<IdSalarie, ContratsADire> = BTreeMap::new();
    for i in importes.iter().filter(|i| i.a_dire()) {
        // Un engagement importe porte toujours son salarie.
        let Some(salarie) = i.objet().carte().salarie.clone() else { continue };
        let payes = jours_payes(i)?;
        let x = par.entry(salarie).or_insert(ContratsADire { nombre: 0, de_permanent: 0, par_bulletin: 0, payes: None, en_cours: false });
        x.nombre += 1;
        // D'ou vient ce qui en a ete repris : la piece qui atteste son contrat, sinon la source de ses paies.
        let de_permanent = i.contrat().is_some();
        x.de_permanent += usize::from(de_permanent);
        x.par_bulletin += usize::from(!de_permanent && i.paies().values().all(|p| p.contrat.d_un_bulletin()));
        x.payes = match (x.payes, payes) {
            (Some(a), Some(b)) => Some(reunies(a, b)?),
            (a, b) => a.or(b),
        };
        x.en_cours |= dernier.as_ref().is_some_and(|d| i.paies().contains_key(d));
    }
    Ok(par)
}

/// Les salaries dont des contrats repris (`importes`, le stock deja lu) attendent leur nature, dans l'ordre des cles :
/// ceux que « Pour demarrer » dit (`declarations::demarrage::CeQueLImportARepris`).
pub(crate) fn salaries_a_dire(importes: &[Importe]) -> R<Vec<IdSalarie>> {
    Ok(a_dire_par_salarie(importes)?.into_keys().collect())
}

/// Les salaries dont des contrats repris attendent leur nature : `[{salarie, nom, contrats, du, au, en_cours, suite,
/// natures: [{cle, libelle, demande}]}]`, dans l'ordre des cles. `en_cours` : il est paye le dernier mois repris, son
/// contrat court peut-etre encore (`suite` : le mois qui suit). CE QUE LA DERNIERE DSN REPRISE DECLARE DU CONTRAT NE SE
/// DEMANDE PAS (`DeclareParLaDsn`) : la nature qu'elle declare est la seule proposee, et chaque nature ne demande que ce
/// qu'elle ne declare pas ; `importation-nature` le prend sur elle.
pub(crate) fn a_dire(ctx: &Contexte) -> R<Vec<ADire>> {
    a_dire_lu(ctx, &DsnReprises::de(&ctx.racine))
}

fn a_dire_lu(ctx: &Contexte, dsn: &DsnReprises) -> R<Vec<ADire>> {
    let racine = ctx.racine.as_path();
    let importes = importes(racine)?;
    let par = a_dire_par_salarie(&importes)?;
    if par.is_empty() {
        return Ok(Vec::new());
    }
    let suite = dernier_mois(&importes).map(socle::mois::Mois::suivant);
    let mut out = Vec::new();
    for (salarie, c) in par {
        let en_cours = c.en_cours;
        let natures = natures_permises(ctx, &importes, &salarie, dsn)?;
        let (declare, dsn_non_relue) = match DeclareParLaDsn::de(dsn, ctx, &importes, &salarie, &natures)? {
            Ok(d) => (d, None),
            Err(p) => (None, Some(p)),
        };
        let nature_declaree = declare.as_ref().and_then(|d| d.nature.as_ref()).map(|n| n.cle);
        let natures = natures
            .iter()
            .filter(|x| nature_declaree.is_none_or(|n| n == x.cle))
            .map(|x| NatureProposee {
                cle: x.cle,
                libelle: x.libelle.clone(),
                demande: demande(x, en_cours).into_iter().filter(|champ| !declare.as_ref().is_some_and(|d| d.declare(*champ))).collect(),
            })
            .collect();
        // Le mois de la derniere DSN qui l'a declare, tel que l'ecran le lit (« de novembre 2026 »).
        let mois_au = c.payes.map(|p| p.fin().mois_civil());
        let au_dit: Option<String> = mois_au.map(crate::regularisation::de_mois_en_clair);
        let nom = nom_du_salarie(racine, &salarie)?;
        let dit = c.dit();
        let en_cours_dit = match (en_cours, mois_au, suite, &declare) {
            (true, Some(au), Some(suite), Some(d)) => Some(format!(
                "Payé jusqu'en {} : son contrat continue en {}, tel que sa dernière DSN reprise le déclare ({}).{}",
                au.libelle(socle::mois::Graphie::Accentuee),
                suite.libelle(socle::mois::Graphie::Accentuee),
                d.en_clair(),
                match d.salaire {
                    Ok(_) => "",
                    Err(pourquoi) => pourquoi,
                }
            )),
            (true, Some(au), Some(suite), None) => Some(format!(
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
            payes: c.payes,
            au_dit,
            en_cours,
            suite: suite.filter(|_| en_cours).map(|m| m.to_string()),
            natures,
            dit,
            en_cours_dit,
            dsn_non_relue,
        });
    }
    Ok(out)
}

/// Les contrats d'un salarie dont la nature reste a dire, et ce que la page en dit.
#[derive(Debug, serde::Serialize)]
pub(crate) struct ADire {
    pub(crate) salarie: IdSalarie,
    pub(crate) nom: String,
    pub(crate) contrats: usize,
    /// Du premier au dernier jour que ses paies reprises paient ; rien sans paie reprise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) payes: Option<Periode>,
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
    /// Les DSN reprises ne se relisent pas : ce qu'elles declarent du contrat se demande, et la phrase dit pourquoi.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) dsn_non_relue: Option<socle::texte::Phrase>,
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
    pub(crate) salarie: IdSalarie,
    pub(crate) nature: droit::natures::NatureDeContrat,
    pub(crate) debut: Option<Jour>,
    pub(crate) fin: Option<Jour>,
    pub(crate) numero: Option<engagement::numeros::NumeroContrat>,
    /// Le salaire, un nombre, ou tel que la personne l'a tape (« 1 850,00 »), converti a la porte (`Charge::lire`).
    pub(crate) salaire: Option<socle::grandeurs::MontantUnitaire>,
    pub(crate) quotite: Option<socle::grandeurs::Heures>,
}

/// Ce qu'`importation-nature` rend : les contrats ecrits, ce qui reste a dire, et ce que l'ecran en dit.
#[derive(Debug, serde::Serialize)]
pub(crate) struct NatureDite {
    pub(crate) phrase: socle::texte::Phrase,
    pub(crate) ecrits: usize,
    pub(crate) a_dire: Vec<ADire>,
    /// La fiche Entreprise vient de dire que la compagnie emploie des permanents.
    pub(crate) permanents_declares: bool,
    /// Les contrats de prevoyance et de sante des permanents que les DSN reprises declarent, poses.
    pub(crate) parametrage: Vec<String>,
    pub(crate) couleur: &'static str,
}

/// `importation-nature` : la nature dite pour les contrats a dire d'un salarie, et, s'il court encore, le debut, la fin
/// et le numero du contrat tels que la DSN de l'ancien logiciel les a declares. Ce que la charge ne dit pas se prend sur
/// la derniere DSN reprise du salarie quand elle le declare (`DeclareParLaDsn`) ; ce qu'aucune des deux ne dit se refuse.
pub(crate) fn dire(ctx: &Contexte, charge: &ChargeNature) -> R<NatureDite> {
    dire_lu(ctx, charge, &DsnReprises::de(&ctx.racine))
}

fn dire_lu(ctx: &Contexte, charge: &ChargeNature, dsn: &DsnReprises) -> R<NatureDite> {
    let salarie = &charge.salarie;
    let nature = charge.nature;
    let importes = importes(&ctx.racine)?;
    let qui = nom_du_salarie(&ctx.racine, salarie)?;
    let Some(c) = a_dire_par_salarie(&importes)?.remove(salarie) else {
        return Err(refus(format!("Aucun contrat repris de {} n'attend sa nature : rien n'a été écrit.", qui)));
    };
    let permises = natures_permises(ctx, &importes, salarie, dsn)?;
    let Some(nat) = permises.iter().find(|n| n.cle == nature) else {
        let noms: Vec<&str> = permises.iter().map(|n| n.libelle.as_str()).collect();
        return Err(refus(match noms.is_empty() {
            false => format!("« {} » n'est pas une nature que ses pièces reprises permettent pour le contrat de {} ({}) : rien n'a été écrit.", nature, qui, noms.join(", ")),
            true => format!("Les pièces reprises de {} ne permettent aucune nature à la fois pour tous ses contrats à dire : rien n'a été écrit.", qui),
        }));
    };
    let en_cours = c.en_cours;
    // Les jours que ses paies reprises paient : un contrat a dire en a toujours (un bulletin, ou sa DSN, l'a repris).
    let Some(payes) = c.payes else {
        return Err(refus(format!("Aucune paie reprise de {} ne dit quand ses contrats ont été payés : rien n'a été écrit.", qui)));
    };
    let (du, au) = (payes.debut(), payes.fin());
    let dernier = dernier_mois(&importes);
    // Le contrat qui court encore : son debut, sa fin s'il a un terme, son numero -- lus sur la DSN deja deposee ; ce
    // qui s'en ecrit sur l'engagement repris.
    let mut contrat = engagement::saisie::Carte::default();
    let mut debut_dit: Option<Jour> = None;
    let champs = demande(nat, en_cours);
    // Ce que la charge tait, la DSN reprise le dit peut-etre : elle ne se lit que s'il en manque.
    let tait = |champ: &Demande| match champ {
        Demande::Debut => charge.debut.is_none(),
        Demande::Fin => charge.fin.is_none(),
        Demande::Numero => charge.numero.is_none(),
        Demande::Salaire => charge.salaire.is_none(),
        Demande::Quotite => charge.quotite.is_none(),
    };
    // Des DSN reprises qui ne se relisent pas ne declarent rien : ce que la charge tait manque, et le refus dit pourquoi.
    let (declare, non_relue) = if champs.iter().any(tait) {
        match DeclareParLaDsn::de(dsn, ctx, &importes, salarie, &permises)? {
            Ok(d) => (d, None),
            Err(p) => (None, Some(p)),
        }
    } else {
        (None, None)
    };
    let d = declare.as_ref();
    let charge = ChargeNature {
        salarie: salarie.clone(),
        nature,
        debut: charge.debut.or(d.map(DeclareParLaDsn::debut)),
        fin: charge.fin.or(d.and_then(DeclareParLaDsn::fin)),
        numero: charge.numero.clone().or(d.and_then(|d| d.numero().cloned())),
        salaire: charge.salaire.or(d.and_then(DeclareParLaDsn::salaire)),
        quotite: charge.quotite.or(d.and_then(DeclareParLaDsn::quotite)),
    };
    let pourquoi: String = non_relue.iter().map(|p| format!(" {}", p)).collect();
    for champ in champs {
        let manque = || {
            refus(format!("{}{}", match champ {
                Demande::Debut => format!("La date de début du contrat de {} manque (celle que la DSN déclare) : rien n'a été écrit.", qui),
                Demande::Fin => format!("La date de fin prévue du contrat de {} manque : rien n'a été écrit.", qui),
                Demande::Salaire => format!("Le salaire mensuel brut du contrat de {} manque (celui du contrat signé) : rien n'a été écrit.", qui),
                Demande::Quotite => format!(
                    "La durée du travail du contrat de {} manque (en heures par mois, celle du contrat signé) : rien n'a été écrit.",
                    qui
                ),
                Demande::Numero => format!("Le numéro du contrat de {} manque (celui que la DSN déclare) : rien n'a été écrit.", qui),
            }, pourquoi))
        };
        match champ {
            Demande::Debut | Demande::Fin => {
                let jour = (if champ == Demande::Debut { charge.debut } else { charge.fin }).ok_or_else(manque)?;
                if champ == Demande::Debut && jour > du {
                    return Err(refus(format!(
                        "Le contrat de {} ne peut pas commencer le {} : ses paies reprises le paient dès le {}. Rien n'a été écrit.",
                        qui,
                        jour.iso(),
                        du.iso()
                    )));
                }
                if champ == Demande::Fin && jour < au {
                    return Err(refus(format!("Le contrat de {} ne peut pas finir le {} : ses paies reprises le paient jusqu'au {}. Rien n'a été écrit.", qui, jour.iso(), au.iso())));
                }
                if champ == Demande::Debut {
                    debut_dit = Some(jour);
                }
                *(if champ == Demande::Debut { &mut contrat.debut } else { &mut contrat.fin }) = Some(jour);
            }
            Demande::Salaire => {
                let salaire = charge.salaire.ok_or_else(manque)?;
                if salaire <= socle::grandeurs::MontantUnitaire::ZERO {
                    return Err(refus(format!("« {} » n'est pas un salaire mensuel brut : rien n'a été écrit.", socle::grandeurs::ecrit::sans_zero::dit(salaire))));
                }
                contrat.salaire_mensuel = Some(socle::grandeurs::courte(salaire));
            }
            Demande::Quotite => {
                let h = charge.quotite.ok_or_else(manque)?;
                if h <= socle::grandeurs::Heures::ZERO {
                    return Err(refus(format!(
                        "« {} » n'est pas une durée du travail en heures par mois : rien n'a été écrit.",
                        socle::grandeurs::ecrit::sans_zero::dit(h)
                    )));
                }
                contrat.quotite_contrat = Some(socle::grandeurs::courte(h));
            }
            Demande::Numero => contrat.numero = Some(charge.numero.clone().ok_or_else(manque)?),
        }
    }
    // La nature va sur l'engagement de chaque contrat a dire ; ce qui definit le contrat qui court encore, sur le sien.
    // Un contrat a dire est celui dont ni la carte, ni la piece du contrat, ni la DSN ne portent la nature
    // (`engagement::importe::nature_de`) : qu'un bulletin l'atteste, ou ses seules DSN.
    let ecrits = engagement::importe::completer(&ctx.racine, |e, pieces| {
        if e.salarie.as_ref() != Some(salarie) || engagement::importe::nature_de(e, pieces).is_some() {
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
        crate::fiches::reprendre_anciennete(ctx, salarie, du, debut)?;
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
    // Une nature qui n'est pas payee au mois (un contrat d'usage) ne dit rien d'un permanent.
    let permanents_declares = nat.mensuelle() && !ctx.emploie_des_permanents_pendant(&Periode::du_jour(depuis));
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
    use socle::texte::Phrase;
    let mut phrases = vec![socle::phrase!("Nature enregistrée sur {} contrat{} repris.", ecrits, if ecrits > 1 { "s" } else { "" })];
    if permanents_declares {
        phrases.push(Phrase::fixe("La compagnie emploie donc des salariés au mois : « Pour démarrer » dit ce que cela demande encore."));
    }
    // Ce que le parametrage repris dit, encore en texte (#419, le dernier chantier).
    phrases.extend(parametrage.iter().map(|p| socle::phrase!("{}", p)));
    Ok(NatureDite { phrase: Phrase::jointes(phrases, " "), ecrits, a_dire: a_dire_lu(ctx, dsn)?, permanents_declares, parametrage, couleur: socle::ecran::couleur_de_l_etat(socle::ecran::EtatDEcran::Fait) })
}

impl crate::enveloppe::Dite for NatureDite {
    fn phrase(&self) -> &socle::texte::Phrase {
        &self.phrase
    }
}

/// CE QUE LA DSN REPRISE DIT DEJA NE SE DEMANDE PAS. Pour chaque salarie dont des contrats repris attendent leur nature
/// (`a_dire`), le contrat que sa derniere DSN reprise declare (`DeclareParLaDsn`) dit la nature, avec tout ce que
/// `demande` exige d'un contrat qui court encore : la nature se dit comme a l'ecran (`dire`), qui prend sur la DSN ce
/// que la charge tait. Il en manque (le salaire d'un mois que la paie ne couvre pas entier), ou la nature declaree n'est
/// pas celle d'un seul permanent : rien ne s'ecrit, et l'ecran ne demande que ce qui manque (`a_dire`). Rend ce que
/// chaque nature dite a fait, en phrases ; une nature lue que l'ecriture refuse se dit aussi.
pub(crate) fn dire_depuis_les_dsn(racine: &Path, quand: &Jour) -> R<Vec<String>> {
    let dsn = DsnReprises::de(racine);
    let mut faits = Vec::new();
    let tous = importes(racine)?;
    for (salarie, x) in a_dire_par_salarie(&tous)? {
        let ctx = Contexte::nouveau(racine, Some(*quand))?;
        let permises = natures_permises(&ctx, &tous, &salarie, &dsn)?;
        // Des DSN reprises qui ne se relisent pas ne disent rien : rien ne s'ecrit, `importation-etat` dit pourquoi.
        let Ok(Some(d)) = DeclareParLaDsn::de(&dsn, &ctx, &tous, &salarie, &permises)? else { continue };
        let Some(nat) = d.nature.clone() else { continue };
        if !demande(&nat, x.en_cours).into_iter().all(|champ| d.declare(champ)) {
            continue;
        }
        let charge = ChargeNature { salarie: salarie.clone(), nature: nat.cle, debut: None, fin: None, numero: None, salaire: None, quotite: None };
        let nom = nom_du_salarie(racine, &salarie)?;
        match dire_lu(&ctx, &charge, &dsn) {
            Ok(r) => faits.push(format!("{} : {}, lu dans sa DSN reprise. {}", nom, nat.libelle, r.phrase)),
            Err(e) => faits.push(format!("{} : {}, lu dans sa DSN reprise, ne s'écrit pas. {}", nom, nat.libelle, e.phrase)),
        }
    }
    Ok(faits)
}

/// LA DSN FAIT FOI (Léandre, 18/11/2026 : « DSN fait foi » ; « qu'il dise ce qu'il a remplacé »). Un contrat de
/// permanent repris dont la nature a ete dite (`importation-nature`, a la main ou d'une DSN reprise avant) garde sa carte
/// d'un import a l'autre (`engagement::importe::ecrire_tous`) ; quand une DSN QUI ARRIVE avec cet import (`arrivees` :
/// les paies declarees que l'historique n'avait pas, par mois, salarie et numero) declare une de ses paies, la derniere
/// d'entre elles dit le contrat tel que la DSN le declare : son debut (S21.G00.40.001), sa fin s'il en a une (.010), son
/// numero (.009), sa nature (.007, celle d'un permanent qui porte ce code, s'il n'y en a qu'une ;
/// `norme-dsn/texte/bloc-S21.G00.40.txt`). Chaque valeur declaree que la carte porte autrement se pose sur elle et se
/// dit (`ValeurDeLaDsn`) ; une valeur identique, ou que la DSN ne declare pas, ne change pas et ne se dit pas. Un import
/// rejoue sur le meme dossier n'apporte aucune paie declaree : rien ne se remplace de nouveau.
///
/// TANT QUE RIEN N'EST PARTI D'ICI. Un engagement valide, ou dont quelque chose est parti (`departs`, ce que le journal
/// dit : `engagement::etats::ce_qui_est_parti`), a dit son contrat sous ce que porte sa carte : son numero ne se
/// recalcule pas (regle 4), et le debut et la nature, qui le designent avec lui dans la DSN, non plus ; rien ne s'y
/// remplace, l'ecart se dit (`SortDeLaValeur::DejaDeclaree`).
pub(crate) fn la_dsn_fait_foi(
    racine: &Path,
    arrivees: &std::collections::HashSet<(socle::mois::Mois, IdSalarie, engagement::numeros::NumeroContrat)>,
    departs: &engagement::departs::Departs,
) -> R<Vec<import::reponse::ValeurDeLaDsn>> {
    use import::reponse::{SortDeLaValeur, ValeurDeLaDsn, ValeurDeclaree, ValeurDuContrat};
    if arrivees.is_empty() {
        return Ok(Vec::new());
    }
    // Les engagements que la DSN qui arrive declare autrement : (salarie, mois et numero de sa derniere paie arrivee, ce
    // qui differe, et ce qui est deja parti de lui ou sa validation, qui garde sa carte).
    struct ARemplacer {
        salarie: IdSalarie,
        mois: socle::mois::Mois,
        numero: engagement::numeros::NumeroContrat,
        valeurs: Vec<ValeurDuContrat>,
        deja: Option<String>,
    }
    let mut a_remplacer: Vec<ARemplacer> = Vec::new();
    let mut lecture: Option<import::dsn_aem::LectureDsn> = None;
    let permises = natures_permanentes()?;
    for i in importes(racine)? {
        let carte = i.objet().carte();
        let (Some(salarie), Some(nature)) = (carte.salarie.clone(), carte.nature) else { continue };
        // Le contrat d'un permanent : sa nature est dite, aucune piece ne l'atteste (une AEM, un certificat, un bulletin
        // du spectacle n'attestent qu'un contrat d'usage). La nature dite se recopie sur sa piece a l'import suivant :
        // elle n'en fait pas une nature attestee.
        if !i.permanent() {
            continue;
        }
        let arrivee = i.paies().iter().rev().find_map(|(m, p)| {
            let n = p.contrat.numero_declare()?;
            arrivees.contains(&(*m, salarie.clone(), n.clone())).then(|| (*m, n.clone(), p.clone()))
        });
        let Some((mois, numero, paie)) = arrivee else { continue };
        let mut valeurs = Vec::new();
        if let Some(debut) = paie.periode.map(|o| o.debut()).filter(|d| carte.debut != Some(*d)) {
            valeurs.push(ValeurDuContrat::Debut { avant: carte.debut, declaree: debut });
        }
        if let Some(fin) = paie.periode.and_then(|o| o.fin()).filter(|f| carte.fin != Some(*f)) {
            valeurs.push(ValeurDuContrat::Fin { avant: carte.fin, declaree: fin });
        }
        if carte.numero.as_ref() != Some(&numero) {
            valeurs.push(ValeurDuContrat::Numero { avant: carte.numero.clone(), declaree: numero.clone() });
        }
        // La nature : le code que la DSN declare pour ce contrat, lu dans les DSN reprises (une fois pour l'import).
        if lecture.is_none() {
            lecture = Some(import::dsn_aem::lire_dsn(&import::mois::dossier_des_bulletins(racine))?);
        }
        let declare = lecture.as_ref().and_then(|l| l.paies.get(&(mois, paie.nir.clone(), numero.clone()))).and_then(|d| d.contrat.as_ref());
        // Le code et le motif de recours declares ensemble designent la nature (`Nature::declaree_par`).
        if let Some((code, motif)) = declare.and_then(|c| Some((c.nature_dsn.clone()?, c.motif_recours))) {
            let dite = permises.iter().find(|n| n.cle == nature);
            let du_code: Vec<&std::sync::Arc<droit::natures::Nature>> = permises.iter().filter(|n| n.declaree_par(&code, motif)).collect();
            // La nature dite est deja celle que la DSN declare : elle ne dit rien d'autre.
            let meme_code = dite.is_some_and(|n| n.declaree_par(&code, motif));
            if let (false, [declaree]) = (meme_code, du_code.as_slice()) {
                valeurs.push(ValeurDuContrat::Nature { avant: nature, declaree: declaree.cle });
            }
        }
        if valeurs.is_empty() {
            continue;
        }
        let partis = engagement::etats::ce_qui_est_parti(i.objet(), departs);
        let deja = match (partis.is_empty(), i.objet().est_valide()) {
            (false, _) => Some(format!("déjà parti ({})", partis.join(" ; "))),
            (true, true) => Some("validé".to_string()),
            (true, false) => None,
        };
        a_remplacer.push(ARemplacer { salarie, mois, numero, valeurs, deja });
    }
    if a_remplacer.is_empty() {
        return Ok(Vec::new());
    }
    // La carte prend les valeurs declarees : celle de l'engagement dont la paie arrivee est la sienne, s'il n'a rien dit
    // d'ici.
    if a_remplacer.iter().any(|x| x.deja.is_none()) {
        engagement::importe::completer(racine, |e, pieces| {
            let Some(x) = a_remplacer.iter().filter(|x| x.deja.is_none()).find(|x| {
                e.salarie.as_ref() == Some(&x.salarie) && pieces.paies.get(&x.mois).is_some_and(|p| p.contrat.numero_declare() == Some(&x.numero))
            }) else {
                return false;
            };
            for v in &x.valeurs {
                match v {
                    ValeurDuContrat::Nature { declaree, .. } => e.nature = Some(*declaree),
                    ValeurDuContrat::Debut { declaree, .. } => e.debut = Some(*declaree),
                    ValeurDuContrat::Fin { declaree, .. } => e.fin = Some(*declaree),
                    ValeurDuContrat::Numero { declaree, .. } => e.numero = Some(declaree.clone()),
                }
            }
            true
        })?;
    }
    let mut out = Vec::new();
    for x in a_remplacer {
        let nom = nom_du_salarie(racine, &x.salarie)?;
        for valeur in x.valeurs {
            let (quoi, avant, declaree): (&str, Option<String>, String) = match &valeur {
                ValeurDuContrat::Nature { avant, declaree } => ("la nature du contrat", Some(avant.proprietes()?.libelle.clone()), declaree.proprietes()?.libelle.clone()),
                ValeurDuContrat::Debut { avant, declaree } => ("le début du contrat", avant.map(|j| j.jj_mm_aaaa()), declaree.jj_mm_aaaa()),
                ValeurDuContrat::Fin { avant, declaree } => ("la fin du contrat", avant.map(|j| j.jj_mm_aaaa()), declaree.jj_mm_aaaa()),
                ValeurDuContrat::Numero { avant, declaree } => ("le numéro du contrat", avant.as_ref().map(ToString::to_string), declaree.to_string()),
            };
            // La carte ne portait rien : elle se complete. Elle portait autre chose : elle le remplace, ou, deja dit
            // d'ici, le garde, et l'ecart se dit.
            let (sort, phrase) = match (&x.deja, avant) {
                (None, None) => (SortDeLaValeur::Completee, socle::phrase!("{} : {} devient « {} », ce que la DSN reprise déclare.", nom, quoi, declaree)),
                (None, Some(avant)) => (
                    SortDeLaValeur::Remplacee,
                    socle::phrase!("{} : {} était « {} » ; la DSN reprise déclare « {} », qui le remplace.", nom, quoi, avant, declaree),
                ),
                (Some(deja), avant) => (
                    SortDeLaValeur::DejaDeclaree,
                    socle::phrase!(
                        "{} : la DSN reprise déclare {} « {} », l'engagement porte {} ; il est {} : rien n'est remplacé.",
                        nom,
                        quoi,
                        declaree,
                        match avant {
                            Some(a) => format!("« {} »", a),
                            None => "rien".to_string(),
                        },
                        deja
                    ),
                ),
            };
            out.push(ValeurDeLaDsn { salarie: x.salarie.clone(), nom: nom.clone(), valeur: ValeurDeclaree::Contrat(valeur), sort, phrase });
        }
    }
    Ok(out)
}

/// Le contrat que la DSN declare (`c` : ses dates, S21.G00.40.001 et .010, et la fin que son bloc 62 declare) couvre-t-il
/// le mois `mois` entier ?
fn couvre_le_mois(c: &declarations::dsn_lue::ContratDeclare, mois: socle::mois::Mois) -> bool {
    let du_mois = Periode::du_mois(mois);
    c.periode.croise(&du_mois) == Some(du_mois) && c.fin_declaree.is_none_or(|f| f >= mois.dernier_jour())
}
