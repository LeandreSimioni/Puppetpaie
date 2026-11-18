//! `importer` : ecrit ce que `importation-apercu` annonce.
//!
//! `{"dossier": ...}` : la boite aux lettres `import/` entiere, comme l'apercu (`importation::bulletins`). Dans l'ordre : ce
//! qui est repris (DSN, AEM, PDF dont une page au moins est reprise) est COPIE dans `historique/bulletins/`, sous le nom
//! de sa copie (`<empreinte>.<pdf|dsn|txt>` : rien de personnel ; `importation::bulletins::nom_copie`) -- rien ne s'ecrase ;
//! l'apercu est relu DANS ces copies, et c'est lui qui s'ecrit ; les fiches
//! a creer (entreprise, salaries, spectacles), ecrites comme celles de l'ecran (`crate::fiches::ecrire_fiche`,
//! `forcer`) ; les ENGAGEMENTS IMPORTES (`engagement::importe`), chacun dans `engagements/AAAA-MM.json`, le fichier de son
//! mois d'entree (`engagement::importe::ecrire_tous`) : le contrat qu'une AEM ou un bulletin atteste, et ce qui a ete verse, mois par mois (la DSN
//! deposee, completee par le bulletin) ; puis l'alerte CASC-SVP (`alerte_apres_import`). Un import se rejoue sans dommage. La reponse est celle de l'apercu, completee de
//! ce qui a ete ecrit (`ecrit`, `fichiers`, `copie`) ; elle garde le compte rendu (`tri`) du dossier lu, pour l'ecran ;
//! un document accepte et pas lu part en incident, masque (`importation::incident`). Les pieces copiees restent sous
//! `historique/` : le dossier du salarie montre les pages des bulletins, l'alerte CASC-SVP relit les DSN.
//!
//! UNE PAIE, DEUX SOURCES, UNE LIGNE (les deux chemins) : une ligne de DSN qui arrive sur une ligne que des bulletins
//! ont deja completee garde ce que le bulletin a ecrit (le bulletin fait foi) ; une ligne qu'un bulletin seul declare
//! s'efface devant la ligne de DSN des memes salarie, debut et fin, qu'elle complete (`rapprocher`) -- l'ordre DSN
//! puis bulletins, ou bulletins puis DSN, donne les memes engagements.

use droit::fiche::Lue as _;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use droit::contexte::{Contexte, IdSalarie};
use droit::fiche::NomDeFiche;
use droit::fiche::FicheNommee;
use engagement::importe::{Attestation, CleDePaieVersee, ContratLu, ContratRelu, PaieLue, PaieVersee};
use engagement::numeros::NumeroContrat;
use socle::date::PeriodeOuverte;
use socle::fichiers::{CheminRelatif, NomDeFichier};
use socle::mois::Mois;
use socle::echec::{Classe, Echec, R};
use import::attestes::BlocDImport;
use import::reponse::{Copie, EtatFiche, Importation};

/// Le fichier de `config/` ou s'ecrit la fiche `nom`.
fn fichier_de(nom: NomDeFiche) -> &'static str {
    match nom {
        NomDeFiche::Entreprise => "entreprise.json",
        NomDeFiche::Salarie => "salaries.json",
        NomDeFiche::Spectacle => "spectacles.json",
    }
}

fn en_os(e: &std::io::Error, p: &Path) -> Echec {
    socle::echec::du_disque(p, e).reclasser(Classe::Disque)
}

// ------------------------------------------------------------------ l'alerte CASC-SVP

/// La DSN qu'un mois importe a deposee hors de PuppetPaie : le fichier que l'import a retenu pour ce mois (la paie
/// versee qu'une DSN declare, d'un engagement importe ; celle d'un bulletin seul nomme son PDF), sa copie sous
/// `historique/bulletins/`, ou qu'est la compagnie. `None` : un mois que l'import n'a pas retenu ; un fichier retenu
/// introuvable refuse.
pub(crate) fn dsn_importee(ctx: &Contexte, mois: Mois) -> R<Option<PathBuf>> {
    let fichiers: Vec<socle::fichiers::NomDeFichier> = engagement::importe::paies_lues(&ctx.racine)?
        .into_iter()
        .filter(|x| x.mois == mois && !x.paie.contrat.d_un_bulletin())
        .filter_map(|x| x.paie.fichier)
        .collect();
    let Some(nom) = fichiers.last() else { return Ok(None) };
    let chemin = dossier_bulletins(ctx).join(nom);
    if chemin.exists() {
        return Ok(Some(chemin));
    }
    let a_part = socle::echec::Lieu { fichier: Some(nom.clone().into()), ..Default::default() };
    Err(socle::echec::refus_avec(format!("La DSN de {} a été importée, mais son fichier n'est plus dans l'historique de la compagnie.", mois), a_part))
}

/// `lire_dsn_casc` : si la DSN deposee porte la CASC-SVP (un contrat rattache, S21.G00.70.013, a l'adhesion que la
/// DSN nomme `contrat` en bloc 15). `None` : illisible.
pub(crate) fn lire_dsn_casc(chemin: &Path, contrat: &droit::organismes::ReferenceDeContrat) -> R<Option<bool>> {
    let Some(d) = declarations::dsn_lue::lire_une_dsn(chemin)? else { return Ok(None) };
    let porte = d
        .adhesions
        .iter()
        .filter(|a| a.libelle == contrat.as_str())
        .filter_map(|a| a.reference.as_ref())
        .any(|r| d.rattachements.contains(r));
    Ok(Some(porte))
}

/// L'alerte apres l'import, lue avec ce que l'import a ecrit (`ctx`) : une compagnie de la 3090 qui arrive d'un autre
/// logiciel avec des DSN de 2025 ou apres, d'ou la CASC-SVP manque peut-etre. Elle dit ce que la regularisation de
/// toutes les annees ferait partir (`importation::casc::regularisation`, calculee sans rien ecrire) : ses mois, sa
/// periode, son total. Rien si ce n'est pas la 3090, ou s'il n'y a rien a regulariser ; une regularisation qui refuse
/// (une annee dont le taux n'est pas releve, une DSN importee dont le fichier manque, une fiche sans NIR) dit son refus,
/// qui est ce que l'export dirait.
fn alerte_apres_import(ctx: &Contexte) -> R<Option<String>> {
    let base = droit::adhesions::casc_svp_base()?;
    if ctx.entreprise.idcc.dite() != Some(&base.idcc) {
        return Ok(None);
    }
    let demande = crate::importation::casc::Demande { mois: None, annee: None, toutes_les_annees: true, jusqu_a: None, annees: Vec::new(), vire_le: Vec::new(), boite_liee: false };
    let r = match crate::importation::casc::regularisation(ctx, &demande) {
        Ok(crate::importation::casc::ARegulariser::Dossier(r)) => r,
        Ok(crate::importation::casc::ARegulariser::Rien(_)) => return Ok(None),
        Err(e) if e.classe == Some(Classe::Refuse) => return Ok(Some(format!("CASC-SVP : {}", e.phrase))),
        Err(e) => return Err(e),
    };
    Ok(Some(format!(
        "CASC-SVP : {} DSN déposée(s) depuis janvier 2025 ({}) ne portent peut-être pas la cotisation CASC-SVP -- un logiciel qui n'était pas paramétré l'a omise. Si Audiens ne l'a pas encaissée, {} sont à régulariser : Exports > « regularisation-casc-svp » fait le dossier (virements, courriels, liste nominative).",
        r.mois.len(),
        r.periode,
        socle::texte::euros(r.total)
    )))
}

// ------------------------------------------------------------------ une paie, deux sources

/// Une paie que des bulletins ont deja completee (ou ecrite).
fn porte_un_bulletin(x: &PaieLue) -> bool {
    x.paie.bulletin.is_some()
}

fn declaree_par_bulletin(x: &PaieLue) -> bool {
    x.paie.contrat.d_un_bulletin()
}

/// Complete une paie par ce qu'un bulletin a ecrit d'elle (`b`), comme `importation::bulletins::completer` : sur le
/// brut, le net imposable, le net a payer et le PAS, le bulletin l'emporte ; le numero d'objet et le jour du versement
/// remplissent ce qui est vide ; les colonnes du bulletin s'ajoutent (les heures, cachets et jours sont ceux du mois
/// d'un permanent).
fn completer_par(ligne: &mut PaieVersee, b: &PaieVersee) {
    fn gagne<T: Clone>(l: &mut Option<T>, b: &Option<T>) {
        if b.is_some() {
            l.clone_from(b);
        }
    }
    for (l, x) in [(&mut ligne.brut, &b.brut), (&mut ligne.nets.net_imposable, &b.nets.net_imposable), (&mut ligne.net_a_payer, &b.net_a_payer), (&mut ligne.nets.pas, &b.nets.pas)] {
        gagne(l, x);
    }
    if ligne.numero_objet.is_none() {
        ligne.numero_objet.clone_from(&b.numero_objet);
    }
    if ligne.verse_le.is_none() {
        ligne.verse_le = b.verse_le;
    }
    gagne(&mut ligne.heures, &b.heures);
    gagne(&mut ligne.duree_du_travail, &b.duree_du_travail);
    gagne(&mut ligne.cachets, &b.cachets);
    gagne(&mut ligne.jours, &b.jours);
    for (l, x) in [(&mut ligne.spectacle, &b.spectacle), (&mut ligne.analytique, &b.analytique)] {
        gagne(l, x);
    }
    for (l, x) in [
        (&mut ligne.cotisations_salariales, &b.cotisations_salariales),
        (&mut ligne.cotisations_patronales, &b.cotisations_patronales),
        (&mut ligne.cout_employeur, &b.cout_employeur),
        (&mut ligne.net_social, &b.net_social),
        (&mut ligne.net_avant_impot, &b.net_avant_impot),
    ] {
        gagne(l, x);
    }
    if !b.lignes.is_empty() {
        ligne.lignes.clone_from(&b.lignes);
    }
    if !b.dates.is_empty() {
        ligne.dates.clone_from(&b.dates);
    }
    gagne(&mut ligne.cotisations, &b.cotisations);
    gagne(&mut ligne.emploi_imprime, &b.emploi_imprime);
    gagne(&mut ligne.bulletin, &b.bulletin);
}

/// Une paie qu'un bulletin seul declare (`CleDePaieVersee::Bulletin`, `Copie`) s'efface devant la paie declaree des memes
/// salarie, debut et fin, qu'elle complete (une seule ; plusieurs : celle du meme mois, sinon rien ne se rapproche).
/// Rend le nombre de paies de bulletin laissees a cote de plusieurs paies declarees.
fn rapprocher(par_cle: &mut BTreeMap<(socle::mois::Mois, Option<IdSalarie>, CleDePaieVersee), PaieLue>) -> usize {
    let de_bulletin: Vec<(socle::mois::Mois, Option<IdSalarie>, CleDePaieVersee)> = par_cle.iter().filter(|(_, x)| declaree_par_bulletin(x)).map(|(k, _)| k.clone()).collect();
    let mut ambigues = 0usize;
    for kb in de_bulletin {
        let b = par_cle[&kb].clone();
        // Une paie dont la fiche est encore a creer ne se rapproche de rien.
        let (Some(periode), Some(_)) = (b.paie.periode.and_then(|o| o.fermee()), &b.salarie) else { continue };
        let salarie = b.salarie.clone();
        let mut cibles: Vec<(socle::mois::Mois, Option<IdSalarie>, CleDePaieVersee)> = par_cle
            .iter()
            .filter(|(_, x)| !declaree_par_bulletin(x) && x.salarie == salarie && x.paie.periode == Some(periode.into()))
            .map(|(k, _)| k.clone())
            .collect();
        if cibles.len() > 1 {
            cibles.retain(|k| k.0 == kb.0);
        }
        // UN CONTRAT A CHEVAL SUR DEUX MOIS : le bulletin imprime la periode du mois, la DSN le contrat entier
        // (S21.G00.40.001 et .010, `norme-dsn/texte/bloc-S21.G00.40.txt`). Le bulletin complete la paie declaree du meme
        // mois dont le contrat contient sa periode, quand il n'y en a qu'une.
        if cibles.is_empty() {
            cibles = par_cle
                .iter()
                .filter(|(k, x)| {
                    !declaree_par_bulletin(x) && k.0 == kb.0 && x.salarie == salarie && x.paie.periode.is_some_and(|o| o.croise(&periode) == Some(periode))
                })
                .map(|(k, _)| k.clone())
                .collect();
        }
        match cibles.as_slice() {
            [] => {}
            [kd] => {
                if let Some(d) = par_cle.get_mut(kd) {
                    if !porte_un_bulletin(d) {
                        completer_par(&mut d.paie, &b.paie);
                    }
                }
                par_cle.remove(&kb);
            }
            _ => ambigues += 1,
        }
    }
    ambigues
}


/// LES ENGAGEMENTS IMPORTES, tires des contrats attestes (`contrats`, une AEM ou un bulletin chacun) et des paies
/// versees (`paies`, une par mois et par contrat). Un engagement, c'est un salarie, un debut et une fin : le contrat
/// atteste qui les porte (plusieurs : celui du meme numero d'objet que la paie), et ses paies, une par mois. Une paie
/// qu'aucun contrat atteste ne porte fait son engagement avec celles des memes salarie, debut, fin et numero de contrat. Les mois du
/// contrat d'un permanent font un seul engagement (plus bas).
/// Son numero est celui que la DSN deposee a declare (S21.G00.40.009), sinon celui que le bulletin imprime ; jamais un
/// numero tire.
fn engagements_importes(contrats: Vec<ContratLu>, paies: Vec<PaieLue>, aujourdhui: socle::date::Jour) -> R<Vec<engagement::importe::Neuf>> {
    use engagement::importe::{Neuf, Pieces};
    // Un salarie, et les dates de son engagement (une fin absente : il court encore ; aucune quand ses pieces ne disent
    // pas son debut).
    struct Groupe {
        salarie: Option<IdSalarie>,
        periode: Option<PeriodeOuverte>,
        contrat: Option<ContratLu>,
        paies: Vec<PaieLue>,
    }
    // Le contrat d'un permanent : sa nature, payee au mois, est dite, ou reste a dire (`import::nature`).
    let mut permanents: HashSet<droit::natures::NatureDeContrat> = HashSet::new();
    for c in &contrats {
        if let Some(n) = c.contrat.nature {
            if n.proprietes()?.mensuelle() {
                permanents.insert(n);
            }
        }
    }
    let permanent = |c: &ContratLu| c.contrat.nature.is_none_or(|n| permanents.contains(&n));
    let mut groupes: Vec<Groupe> = Vec::new();
    for c in contrats {
        groupes.push(Groupe { salarie: c.salarie.clone(), periode: c.periode, contrat: Some(c), paies: Vec::new() });
    }
    for p in paies {
        let (s, dates, o) = (p.salarie.clone(), p.paie.periode, p.paie.numero_objet.clone());
        let mut candidats: Vec<usize> = (0..groupes.len()).filter(|&i| groupes[i].contrat.is_some() && groupes[i].salarie == s && groupes[i].periode == dates).collect();
        // Le contrat qu'un bulletin atteste porte la periode du mois ; la paie declaree, le contrat entier : la periode
        // du bulletin dans le contrat declare, et l'engagement prend les dates du contrat.
        if let (true, Some(declare), false) = (candidats.is_empty(), dates, declaree_par_bulletin(&p)) {
            candidats = (0..groupes.len())
                .filter(|&i| {
                    let g = &groupes[i];
                    g.paies.is_empty()
                        && g.contrat.as_ref().is_some_and(|c| c.contrat.atteste_par == Attestation::Bulletin)
                        && g.salarie == s
                        && g.periode.and_then(|x| x.fermee()).is_some_and(|du_mois| declare.croise(&du_mois) == Some(du_mois))
                })
                .collect();
            if let [i] = candidats.as_slice() {
                groupes[*i].periode = dates;
            }
        }
        // LE MOIS D'UN CONTRAT DE PERMANENT : la paie va au contrat de permanent du salarie qui couvre ce qu'elle paie.
        // Une paie de bulletin paie sa periode ; une paie declaree porte le contrat entier (S21.G00.40.001 et .010,
        // `norme-dsn/texte/bloc-S21.G00.40.txt`) et paie ce qu'il a de son mois. Le contrat de permanent que des bulletins
        // deja repris attestent commence au premier d'entre eux, que la DSN ajoutee ensuite peut preceder : c'est le mois
        // paye qu'il couvre, comme les bulletins de ce contrat lus avec elle (plus haut, le contrat a cheval).
        // Une paie declaree qui porte un numero d'objet est celle d'un contrat d'usage (regle 6 : le numero d'objet n'existe
        // que sur les natures 02 et 10) : elle ne va pas au contrat d'un permanent, meme si elle paie un de ses mois.
        let d_usage_declaree = !declaree_par_bulletin(&p) && o.is_some();
        if candidats.is_empty() && !d_usage_declaree {
            let payee: Option<PeriodeOuverte> = if declaree_par_bulletin(&p) {
                dates
            } else {
                dates.and_then(|d| d.croise(&socle::date::Periode::du_mois(p.mois))).map(PeriodeOuverte::from)
            };
            // Le contrat couvre la paie : il commence au plus tard avec elle, et finit au plus tot avec elle ou ne finit pas.
            let couvre = |contrat: Option<PeriodeOuverte>| match (contrat, payee) {
                (None, _) => true,
                (Some(_), None) => false,
                (Some(c), Some(x)) => {
                    c.contient(&x.debut())
                        && match c.fermee() {
                            None => true,
                            Some(cf) => x.fermee().is_some_and(|xf| cf.croise(&xf) == Some(xf)),
                        }
                }
            };
            candidats = (0..groupes.len())
                .filter(|&i| {
                    let g = &groupes[i];
                    g.salarie == s && g.contrat.as_ref().is_some_and(permanent) && couvre(g.periode)
                })
                .collect();
        }
        let atteste = match candidats.as_slice() {
            [i] => Some(*i),
            [] => None,
            plusieurs => {
                let meme_objet = |c: &ContratLu| match (&c.contrat.numero_objet, &o) {
                    (Some(a), Some(b)) => a.meme(b),
                    (a, b) => a.is_none() && b.is_none(),
                };
                let memes: Vec<usize> = plusieurs.iter().copied().filter(|&i| groupes[i].contrat.as_ref().is_some_and(meme_objet)).collect();
                (memes.len() == 1).then(|| memes[0])
            }
        };
        let libre = |g: &Groupe| !g.paies.iter().any(|x| x.mois == p.mois);
        let i = match atteste.filter(|&i| libre(&groupes[i])) {
            Some(i) => i,
            None => match (0..groupes.len()).find(|&i| {
                let g = &groupes[i];
                g.contrat.is_none() && g.salarie == s && g.periode == dates && g.paies.first().map(|x| &x.paie.contrat) == Some(&p.paie.contrat) && libre(g)
            }) {
                Some(i) => i,
                None => {
                    groupes.push(Groupe { salarie: s, periode: dates, contrat: None, paies: Vec::new() });
                    groupes.len() - 1
                }
            },
        };
        groupes[i].paies.push(p);
    }
    // UN CONTRAT DE PERMANENT, UN ENGAGEMENT : les bulletins d'un permanent attestent chacun le mois qu'ils paient, tous
    // du meme contrat, et la DSN le declare chaque mois sous le meme numero (S21.G00.40.009, reporte a l'identique d'une
    // DSN a la suivante : `norme-dsn/texte/chapitre-2-principes-de-constitution.txt`, § 2.1.2). Les mois du contrat d'un
    // permanent (et les paies declarees de ces mois qu'aucun contrat ne porte) font un engagement par numero ; deux
    // numeros, deux contrats, comme la DSN les a declares. Un mois sans numero lu va au contrat declare a partir de lui,
    // sinon au dernier (inference : le numero du contrat en cours ce mois-la).
    let mois_de = |g: &Groupe| -> Vec<Mois> { g.paies.iter().map(|p| p.mois).collect() };
    let mois_du_debut = |g: &Groupe| g.periode.map(|o| o.debut().mois_civil());
    let du_permanent: HashSet<(Option<IdSalarie>, Mois)> = groupes
        .iter()
        .filter(|g| g.contrat.as_ref().is_some_and(permanent))
        .flat_map(|g| mois_de(g).into_iter().chain(mois_du_debut(g)).map(|m| (g.salarie.clone(), m)).collect::<Vec<_>>())
        .collect();
    let (mut a_fondre, mut autres): (Vec<Groupe>, Vec<Groupe>) = groupes.into_iter().partition(|g| match &g.contrat {
        Some(c) => permanent(c),
        None => !g.paies.is_empty() && mois_de(g).into_iter().all(|m| du_permanent.contains(&(g.salarie.clone(), m))),
    });
    // Le premier mois d'un groupe : celui de sa premiere paie, sinon celui de son debut.
    let premier_mois = |g: &Groupe| mois_de(g).into_iter().min().or_else(|| mois_du_debut(g));
    a_fondre.sort_by_key(|g| (g.salarie.clone(), premier_mois(g)));
    let declare = |g: &Groupe| g.paies.iter().find_map(|p| p.paie.contrat.numero_declare().cloned());
    let imprime = |g: &Groupe| g.contrat.as_ref().and_then(|c| c.contrat.numero_contrat.clone());
    // Le numero que la DSN a declare ; sinon celui que le bulletin imprime.
    let numero_de = |g: &Groupe| -> Option<NumeroContrat> { declare(g).or_else(|| imprime(g)) };
    // (salarie, numero, ses groupes)
    let mut paquets: Vec<(Option<IdSalarie>, Option<NumeroContrat>, Vec<Groupe>)> = Vec::new();
    let (mut numerotes, mut muets): (Vec<(NumeroContrat, Groupe)>, Vec<Groupe>) = (Vec::new(), Vec::new());
    for g in a_fondre {
        match numero_de(&g) {
            Some(n) => numerotes.push((n, g)),
            None => muets.push(g),
        }
    }
    for (n, g) in numerotes {
        match paquets.iter_mut().find(|(s, x, _)| *s == g.salarie && x.as_ref() == Some(&n)) {
            Some(p) => p.2.push(g),
            None => paquets.push((g.salarie.clone(), Some(n), vec![g])),
        }
    }
    for g in muets {
        let m = premier_mois(&g);
        let siens: Vec<usize> = (0..paquets.len()).filter(|&k| paquets[k].0 == g.salarie).collect();
        let a_partir = siens.iter().copied().filter(|&k| paquets[k].2.iter().map(premier_mois).min().is_some_and(|x| x >= m)).min_by_key(|&k| paquets[k].2.iter().map(premier_mois).min());
        match a_partir.or_else(|| siens.last().copied()) {
            Some(k) => paquets[k].2.push(g),
            None => paquets.push((g.salarie.clone(), None, vec![g])),
        }
    }
    for (salarie, _, mut gs) in paquets {
        gs.sort_by_key(premier_mois);
        let paies: Vec<PaieLue> = gs.iter().flat_map(|g| g.paies.iter().cloned()).collect();
        let declarees: Vec<&PaieLue> = paies.iter().filter(|p| !declaree_par_bulletin(p)).collect();
        // Son debut est celui que la DSN declare, sinon le plus tot de ses groupes.
        let premiere = declarees.iter().find_map(|p| p.paie.periode).or_else(|| gs.iter().filter_map(|g| g.periode).min());
        // Sa fin est celle que la DSN declare ; un bulletin ne paie que son mois, il ne dit pas quand le contrat finit.
        let fin = declarees.last().and_then(|p| p.paie.periode).and_then(|o| o.fin());
        let periode = match (premiere, fin) {
            (Some(o), Some(f)) => Some(o.sans_fin().fermee_au(f)?.into()),
            (Some(o), None) => Some(o.sans_fin()),
            (None, _) => None,
        };
        // Le contrat entier : ce qu'un bulletin atteste de son seul mois reste a la paie de ce mois.
        let mut contrat = gs.iter().rev().find_map(|g| g.contrat.clone());
        if let Some(c) = contrat.as_mut().filter(|_| gs.len() > 1) {
            let a = &mut c.contrat;
            (a.fichier, a.page, a.numero_bulletin) = (None, None, None);
            (a.heures, a.cachets, a.jours, a.brut) = (None, None, None, None);
        }
        autres.push(Groupe { salarie, periode, contrat, paies });
    }
    let mut out: Vec<Neuf> = autres
        .into_iter()
        .map(|g| -> R<Neuf> {
            let dates = g.contrat.iter().filter_map(|c| c.contrat.importe_le).chain(g.paies.iter().filter_map(|p| p.paie.importe_le));
            // Le jour ou ses pieces ont ete reprises ; des pieces que rien ne date sont reprises par cet import, aujourd'hui.
            let le = match dates.min() {
                Some(l) => l,
                None => aujourdhui,
            };
            let numero = numero_de(&g);
            let paies = g.paies.iter().map(|p| (p.mois.clone(), p.paie.clone())).collect();
            let pieces = Pieces { le, contrat: g.contrat.map(|c| c.contrat), paies };
            let carte = engagement::saisie::Carte {
                // Les lignes que l'ecriture garde portent la fiche de leur NIR (`ecrire` : `par_nir`) ; une ligne sans elle
                // ne fait pas d'engagement.
                salarie: Some(g.salarie.clone().ok_or_else(|| socle::fichiers::absent("salarie"))?),
                debut: g.periode.map(|o| o.debut()),
                fin: g.periode.and_then(|o| o.fin()),
                numero,
                ..Default::default()
            };
            Ok(Neuf { carte, pieces })
        })
        .collect::<R<_>>()?;
    out.sort_by(|a, b| {
        let k = |e: &Neuf| ((e.carte.debut.clone(), e.carte.fin.clone()), e.carte.salarie.clone());
        k(a).cmp(&k(b))
    });
    Ok(out)
}

// ------------------------------------------------------------------ l'ecriture

/// Ce qu'une ecriture a fait.
struct Ecrit {
    fichiers: Vec<PathBuf>,
    contrats_aem: usize,
    contrats_bulletins: usize,
    contrats_certificats: usize,
    contrats_aem_pdf: usize,
    paies: usize,
    restent_a_creer: usize,
    /// Les fiches existantes completees.
    completees: usize,
    avertissements: Vec<String>,
    /// Les paies declarees que cet import apporte : (mois, salarie, numero declare).
    arrivees: HashSet<(Mois, IdSalarie, NumeroContrat)>,
}

impl Ecrit {
    fn rien(&self) -> bool {
        self.restent_a_creer == 0 && self.completees == 0 && self.contrats_aem == 0 && self.contrats_bulletins == 0 && self.contrats_certificats == 0 && self.contrats_aem_pdf == 0 && self.paies == 0
    }
}

/// Ecrit ce que l'apercu `imp` annonce : les fiches (et leur place dans `reponse`), puis les engagements importes
/// (`engagements_importes` : les contrats des AEM et des bulletins, les paies versees).
fn ecrire(ctx: &Contexte, imp: &mut crate::import::Apercu, note: &str, matricules: &crate::importation::matricules::Matricules) -> R<Ecrit> {
    // LES MATRICULES QUE LA DSN QUI ARRIVE DECLARE, en une fois, avant les fiches neuves : celle que la DSN cree avec le
    // matricule qu'une fiche cede le trouve libre (`importation::matricules`). Les fiches s'ecrivent face a ce qui est
    // alors sur le disque ; chaque fiche neuve recoit le matricule que `prevoir` lui a decide.
    let d_office = matricules.poser(&mut imp.a_creer)?;
    let relu;
    let ctx = if matricules.ecrit() {
        matricules.ecrire(ctx, note)?;
        relu = Contexte::nouveau(&ctx.racine, Some(ctx.date))?.au(&ctx.lue_au)?;
        &relu
    } else {
        ctx
    };
    let aujourdhui = ctx.date;
    let reponse = &mut imp.reponse;
    // Les fiches, comme celles de l'ecran (`forcer` : une fiche importee avec des anomalies est ecrite quand meme).
    let mut ecrits: BTreeSet<PathBuf> = BTreeSet::new();
    let mut restent_a_creer = 0usize;
    for (a, d_office) in imp.a_creer.iter().zip(d_office) {
        let (fiche, anomalies) = crate::importation::fiches::ecrire_fiche(ctx, &a.donnees, &a.fautes, note, d_office)?;
        let emplacement = match (a.donnees.nom(), a.rang) {
            (NomDeFiche::Entreprise, _) => reponse.entreprise.as_mut(),
            (NomDeFiche::Salarie, Some(i)) => reponse.salaries.get_mut(i),
            (NomDeFiche::Spectacle, Some(i)) => reponse.spectacles.get_mut(i),
            (NomDeFiche::Salarie | NomDeFiche::Spectacle, None) => None,
        };
        let Some(emplacement) = emplacement else {
            restent_a_creer += 1;
            continue;
        };
        emplacement.fiche = fiche;
        emplacement.etat = EtatFiche::Creee;
        emplacement.anomalies = anomalies;
        ecrits.insert(ctx.racine.join(socle::fichiers::CONFIG).join(fichier_de(a.donnees.nom())));
    }
    // Les fiches qui existent : ce qui y est vide ou « A COMPLETER » (`import::completer`), rien d'autre.
    let mut completees = 0usize;
    for a in &imp.a_completer {
        let faits = crate::importation::fiches::completer_fiche(ctx, &a.fiche, &a.champs)?;
        if !faits.est_dite() {
            continue;
        }
        let emplacement = match &a.fiche {
            FicheNommee::Entreprise => reponse.entreprise.as_mut(),
            FicheNommee::Salarie(_) => reponse.salaries.iter_mut().find(|e| e.fiche.nommee() == Some(&a.fiche)),
            FicheNommee::Spectacle(_) => reponse.spectacles.iter_mut().find(|e| e.fiche.nommee() == Some(&a.fiche)),
        };
        if let Some(e) = emplacement {
            e.etat = EtatFiche::Completee;
            e.completer.retain(|c| faits.valeur_ecrite(c.champ).is_some());
        }
        ecrits.insert(ctx.racine.join(socle::fichiers::CONFIG).join(fichier_de(a.fiche.nom())));
        completees += 1;
    }
    crate::fiches::dater_licences_suivantes(ctx, &imp.licences, Some(note))?;
    let mut fichiers: Vec<PathBuf> = ecrits.into_iter().collect();
    let mut avertissements: Vec<String> = Vec::new();

    // Les engagements importes, avec les fiches telles qu'elles sont MAINTENANT.
    let frais = Contexte::nouveau(&ctx.racine, None)?;
    let mut par_nir: HashMap<droit::fiche::Nir, IdSalarie> = HashMap::new();
    for (cle, s) in &frais.salaries {
        if let Some(n) = s.nir.dite() {
            par_nir.insert(n.clone(), cle.clone());
        }
    }
    // CE QUE L'IMPORT A DEJA ECRIT, relu de ses engagements importes : les contrats attestes, les paies versees.
    let importes_avant = engagement::importe::tous(&ctx.racine)?;
    let mut engagements: Vec<ContratLu> = engagement::importe::contrats_lus(&ctx.racine)?.into_iter().map(ContratRelu::a_l_apercu).collect();
    let le = aujourdhui;
    // LES CONTRATS QUE L'IMPORT RETIENT (`import::attestes` en a decide), rattaches par NIR a la fiche qui existe
    // maintenant, comptes par bloc du compte rendu.
    // Ce que le registre change aux contrats de l'historique : reduit, ou retire.
    for (avant, apres) in &imp.contrats_changes {
        if let Some(i) = engagements.iter().position(|e| e == avant) {
            match apres {
                Some(l) => engagements[i] = l.clone(),
                None => {
                    engagements.remove(i);
                }
            }
        }
    }
    let mut comptes: HashMap<BlocDImport, usize> = HashMap::new();
    for (nir, ligne, bloc) in &imp.contrats_attestes {
        let Some(cle) = par_nir.get(nir) else { continue };
        let mut e = ligne.clone();
        e.salarie = Some(cle.clone());
        e.contrat.importe_le = Some(le);
        engagements.push(e);
        *comptes.entry(*bloc).or_default() += 1;
    }
    let compte = |bloc: BlocDImport| -> usize { comptes.iter().filter(|(b, _)| **b == bloc).map(|(_, n)| *n).sum() };
    let (contrats_aem, contrats_bulletins, contrats_certificats, contrats_aem_pdf) =
        (compte(BlocDImport::Declarations), compte(BlocDImport::Bulletins), compte(BlocDImport::Certificats), compte(BlocDImport::AemPdf));

    // LES PAIES : on remplace ligne a ligne par la cle (mois, salarie, contrat). Une ligne de DSN qui arrive sur une ligne
    // completee par un bulletin garde ce que le bulletin a ecrit ; une paie de bulletin seul s'efface devant la paie
    // declaree qu'elle complete (`rapprocher`).
    let mut par_cle: BTreeMap<(socle::mois::Mois, Option<IdSalarie>, CleDePaieVersee), PaieLue> = BTreeMap::new();
    for x in crate::import::paies_historique(ctx)? {
        let x = x.a_l_apercu();
        par_cle.insert(x.cle(), x);
    }
    let avant = par_cle.clone();
    for x in &imp.paies {
        let Some(cle) = x.paie.nir.as_ref().and_then(|n| par_nir.get(n)) else { continue };
        let mut x = x.clone();
        x.salarie = Some(cle.clone());
        x.paie.importe_le = Some(le);
        let k = x.cle();
        if let Some(a) = par_cle.get(&k).filter(|a| porte_un_bulletin(a) && !porte_un_bulletin(&x)) {
            completer_par(&mut x.paie, &a.paie.clone());
        }
        par_cle.insert(k, x);
    }
    // LES PAIES QU'UNE DSN DECLARE ET QUE CET IMPORT APPORTE : celles d'une DSN qui arrive (`nature::la_dsn_fait_foi`) ;
    // l'import rejoue sur le meme dossier n'en apporte aucune.
    let arrivees: HashSet<(Mois, IdSalarie, NumeroContrat)> =
        par_cle.keys().filter(|k| !avant.contains_key(*k)).filter_map(|(m, s, c)| Some((*m, s.clone()?, c.numero_declare()?.clone()))).collect();
    let ambigues = rapprocher(&mut par_cle);
    if ambigues > 0 {
        avertissements.push(format!(
            "{} paie(s) écrite(s) d'un bulletin seul ont les dates de plusieurs paies déclarées en DSN : aucune n'est rapprochée, les engagements importés restent côte à côte, à regarder.",
            ambigues
        ));
    }
    let mut neuves = avant.keys().filter(|k| !par_cle.contains_key(*k)).count();
    for (k, v) in &par_cle {
        if avant.get(k).map(PaieLue::sans_date_d_import) != Some(v.sans_date_d_import()) {
            neuves += 1;
        }
    }
    if contrats_aem + contrats_bulletins + contrats_certificats + contrats_aem_pdf + neuves > 0 || importes_avant.is_empty() {
        let importes = engagements_importes(engagements, par_cle.into_values().collect(), le)?;
        for p in engagement::importe::ecrire_tous(&ctx.racine, importes)? {
            fichiers.push(p);
        }
    }
    Ok(Ecrit { fichiers, contrats_aem, contrats_bulletins, contrats_certificats, contrats_aem_pdf, paies: neuves, restent_a_creer, completees, avertissements, arrivees })
}

// ------------------------------------------------------------------ la route

/// LES MOIS IMPORTES SONT DECLARES S'ILS L'ONT ETE : les depots DSN que les retours deja releves attestent
/// entrent au journal avec l'historique, pour qu'un mois tenu avant ne se dise pas « en retard »
/// (`reprise_journal::reprendre_les_depots`). Rend le nombre de depots repris.
fn depots_attestes_au_journal(ctx: &Contexte) -> R<usize> {
    let qui = compagnie::INTERFACE.to_string();
    compagnie::incidents::noter_ceux_des_retours(&ctx.racine, envois::reprise_journal::reprendre_les_depots(ctx, &qui))
}


/// LES ECARTS DE CALCUL PARTENT A CHAQUE IMPORT (`import::ecarts`), lus avec les fiches telles que l'import les a
/// ecrites ; rien ne s'en dit a l'ecran.
fn rapporter_les_ecarts(frais: &Contexte) -> R<usize> {
    import::ecarts::rapporter(frais)
}
fn avertissement_differents(differents: &[String], ou: &str) -> String {
    format!(
        "{} fichier(s) de {} portent le nom d'un fichier déjà copié avec un autre contenu : la copie est gardée telle quelle, à comparer à la main ({}).",
        differents.len(),
        ou,
        differents.iter().take(5).cloned().collect::<Vec<_>>().join(", ")
    )
}

/// Ou l'import du dossier copie ce qu'il reprend.
fn dossier_bulletins(ctx: &Contexte) -> PathBuf {
    import::mois::dossier_des_bulletins(&ctx.racine)
}

/// La copie de ce que le dossier lu reprend, dans `historique/bulletins/`, sous le nom de sa copie. RIEN NE S'ECRASE :
/// un fichier deja copie et identique est laisse ; un autre contenu sous le meme nom est laisse aussi, et SIGNALE (il
/// n'est pas relu). Rend le compte, et les noms a relire.
fn copier_repris(ctx: &Contexte, lu: &crate::importation::bulletins::DossierLu) -> R<(Copie, Vec<NomDeFichier>)> {
    let dossier = dossier_bulletins(ctx);
    let (mut copies, mut identiques) = (0usize, 0usize);
    let (mut differents, mut noms): (Vec<String>, Vec<NomDeFichier>) = (Vec::new(), Vec::new());
    for (i, nom) in &lu.copies {
        let source = &lu.tri.fichiers[*i].disque;
        let cible = dossier.join(nom);
        if cible.exists() {
            let a = std::fs::read(source).map_err(|e| en_os(&e, source))?;
            let b = std::fs::read(&cible).map_err(|e| en_os(&e, &cible))?;
            if a == b {
                identiques += 1;
                noms.push(nom.clone());
            } else {
                differents.push(nom.to_string());
            }
            continue;
        }
        std::fs::create_dir_all(&dossier).map_err(|e| en_os(&e, &dossier))?;
        std::fs::copy(source, &cible).map_err(|e| en_os(&e, source))?;
        // La copie garde la date de la piece.
        let m = std::fs::metadata(source).and_then(|m| m.modified()).map_err(|e| en_os(&e, source))?;
        let f = std::fs::OpenOptions::new().write(true).open(&cible).map_err(|e| en_os(&e, &cible))?;
        f.set_modified(m).map_err(|e| en_os(&e, &cible))?;
        copies += 1;
        noms.push(nom.clone());
    }
    Ok((Copie { copies, identiques, differents, dossier }, noms))
}

/// `importer` : `{"dossier": ...}`, la boite aux lettres `import/` (vide ou absent : celle de la compagnie) -- la copie
/// de ce qui est repris, l'apercu relu dans la copie, l'ecriture.
pub(crate) fn importer(ctx: &Contexte, charge: &crate::import::ChargeDossier) -> R<Importation> {
    let aujourdhui = ctx.date;
    let dossier = crate::import::dossier_import(ctx, charge)?;
    // CE QUI EST PARTI D'ICI, que le journal dit, lu une fois pour le geste : il garde les matricules et les valeurs des
    // contrats qu'il a declares. Les depots que l'import reprend au journal (`depots_attestes_au_journal`) n'y changent
    // rien : leurs lignes ne portent aucune version d'engagement (`envois::reprise_journal`), et rien n'est parti d'eux
    // pour un engagement (`envois::journal::departs_des_lignes`).
    let departs = envois::journal::lu(&ctx.racine)?.departs;
    // 1. Le dossier tel qu'il est : son compte rendu (pour l'ecran), ce que l'import fera des matricules, et la copie de
    // ce qu'il reprend.
    let (tri, copie, noms, deposes, matricules) = crate::importation::bulletins::lire_dossier(ctx, &dossier, &HashMap::new(), |lu| {
        let matricules = crate::importation::matricules::prevoir(ctx, &lu.ap, &departs)?;
        let (copie, noms) = copier_repris(ctx, &lu)?;
        crate::importation::incident::noter_les_non_lus(ctx, &lu)?;
        let tri = lu.ap.reponse.tri;
        // La copie se relit sous le nom du fichier depose.
        let deposes: HashMap<CheminRelatif, CheminRelatif> = lu.copies.iter().map(|(i, nom)| (CheminRelatif::from(nom.clone()), lu.tri.fichiers[*i].chemin.clone())).collect();
        Ok((tri, copie, noms, deposes, matricules))
    })?;

    // 2. D'ABORD LA COPIE, ENSUITE LA LECTURE : l'apercu relu dans les copies de ce dossier (et d'elles seules), prises
    // dans `historique/bulletins/` ; c'est lui qui s'ecrit.
    let copies = dossier_bulletins(ctx);
    let relu = PathBuf::from(format!("{}-copie", crate::import::dossier_de_travail().display()));
    let relire = || -> R<crate::import::Apercu> {
        std::fs::create_dir_all(&relu).map_err(|e| en_os(&e, &relu))?;
        for nom in &noms {
            std::fs::copy(copies.join(nom), relu.join(nom)).map_err(|e| en_os(&e, &copies.join(nom)))?;
        }
        let frais_avant = Contexte::nouveau(&ctx.racine, None)?;
        crate::importation::bulletins::lire_dossier(&frais_avant, &relu, &deposes, |lu| Ok(lu.ap))
    };
    let mut imp = match relire() {
        Ok(a) => {
            socle::fichiers::effacer(&relu)?;
            a
        }
        Err(e) => return Err(socle::fichiers::effacer_apres(e, &relu)),
    };

    // Le compte rendu est celui du dossier lu (`tri`) : ses chemins, ses fichiers non repris (la relecture ne les voit pas).
    if !copie.differents.is_empty() {
        imp.reponse.avertissements.push(avertissement_differents(&copie.differents, "ce dossier"));
    }
    imp.reponse.tri = tri;
    imp.reponse.source = dossier.display().to_string();
    let note = format!("Importe du dossier import/ le {}", aujourdhui.jj_mm_aaaa());
    let e = ecrire(ctx, &mut imp, &note, &matricules)?;
    let mut reponse = imp.reponse;
    reponse.avertissements.extend(e.avertissements.iter().cloned());
    reponse.contrats_nouveaux = e.contrats_aem;
    reponse.bulletins.contrats_nouveaux = e.contrats_bulletins;
    if let Some(c) = reponse.certificats.as_mut() {
        c.contrats_nouveaux = e.contrats_certificats;
    }
    if let Some(c) = reponse.aem_pdf.as_mut() {
        c.contrats_nouveaux = e.contrats_aem_pdf;
    }
    reponse.rien_a_faire = e.rien();
    reponse.ecrit = true;
    reponse.fichiers = e.fichiers;
    reponse.copie = Some(copie);
    reponse.depots_repris = Some(depots_attestes_au_journal(ctx)?);
    // Ce que les DSN reprises disent de la configuration hors de la fiche (le taux AT/MP, les organismes), lu avec les
    // fiches telles que l'import les a ecrites.
    let frais = Contexte::nouveau(&ctx.racine, Some(ctx.date))?;
    let mut faits = crate::importation::parametrage::parametrer(&frais);
    // La nature des contrats repris des permanents, quand leur DSN reprise la dit.
    faits.extend(crate::importation::nature::dire_depuis_les_dsn(&ctx.racine, &ctx.date)?);
    reponse.parametrage = faits;
    // Ce qu'une DSN qui arrive declare d'un contrat de permanent autrement que sa carte : elle fait foi, tant que rien
    // n'est parti d'ici.
    reponse.dsn_fait_foi = crate::importation::nature::la_dsn_fait_foi(&ctx.racine, &e.arrivees, &departs)?;
    let compte = |s: import::reponse::SortDeLaValeur| reponse.dsn_fait_foi.iter().filter(|v| v.sort == s).count();
    let matricules_dits = matricules.dire(true);
    let remplacees = match compte(import::reponse::SortDeLaValeur::Remplacee) {
        0 => String::new(),
        1 => " La DSN reprise fait foi : une valeur d'un contrat repris est remplacée par ce qu'elle déclare.".to_string(),
        n => format!(" La DSN reprise fait foi : {} valeurs de contrats repris sont remplacées par ce qu'elle déclare.", n),
    };
    let gardees = match compte(import::reponse::SortDeLaValeur::DejaDeclaree) {
        0 => String::new(),
        1 => " Une valeur que la DSN reprise déclare autrement est gardée : l'engagement est déjà validé ou déclaré d'ici.".to_string(),
        n => format!(" {} valeurs que la DSN reprise déclare autrement sont gardées : leurs engagements sont déjà validés ou déclarés d'ici.", n),
    };
    let remplacees = remplacees + &gardees + &matricules_dits;
    // Les matricules (`importation::matricules`), tels que l'import les a poses.
    matricules.montrer(&mut reponse, &imp.a_creer);
    // Ce que l'ecran dit de l'import fait, et sa couleur : l'ecriture a eu lieu, c'est fait (ce qui reste a dire a ses
    // propres lignes : la nature des contrats repris, « Pour demarrer »).
    let ecrite = |f: &import::reponse::FicheImportee| matches!(f.etat, EtatFiche::Creee | EtatFiche::Completee);
    let de = |l: &[import::reponse::FicheImportee]| l.iter().filter(|f| ecrite(f)).count();
    // Les fiches ecrites, par sorte : l'entreprise, les salaries, les spectacles.
    let mut fiches: Vec<String> = Vec::new();
    if reponse.entreprise.as_ref().is_some_and(ecrite) {
        fiches.push("l'entreprise".into());
    }
    if de(&reponse.salaries) > 0 {
        fiches.push(socle::ecran::compte(de(&reponse.salaries), "salarié", "salariés"));
    }
    if de(&reponse.spectacles) > 0 {
        fiches.push(socle::ecran::compte(de(&reponse.spectacles), "spectacle", "spectacles"));
    }
    // UN SPECTACLE SANS TITRE (`completer::A_COMPLETER`) : les bulletins en proposent un a valider
    // (la note `titre_propose`), ou les pieces ne portent que son numero d'objet ; le contrat imprime le titre, la phrase
    // le dit. Lu dans les fiches telles que l'import les a ecrites.
    let (mut proposes, mut sans_titre) = (0, 0);
    let frais = Contexte::nouveau(&ctx.racine, Some(ctx.date))?;
    {
        for sp in frais.spectacles.values().filter(|sp| !sp.archivee()) {
            if sp.titre.portee().is_some_and(|t| t.trim().eq_ignore_ascii_case(import::proposee::A_COMPLETER)) {
                if sp.titre_propose.as_deref().is_some_and(|t| !t.is_empty()) {
                    proposes += 1;
                } else {
                    sans_titre += 1;
                }
            }
        }
    }
    let titres = match (proposes, sans_titre) {
        (0, 0) => String::new(),
        (p, 0) => format!(" Titre à valider pour {} spectacle(s), proposé d'après les bulletins (Pour démarrer).", p),
        (0, s) => format!(" Titre à dire pour {} spectacle(s) : les pièces ne portent que le numéro d'objet (Pour démarrer).", s),
        (p, s) => format!(" Titre à dire pour {} spectacle(s), dont {} proposé(s) d'après les bulletins (Pour démarrer).", p + s, p),
    };
    // LES INDIVIDUS MIS DE COTE (`importation::identite`) : rien d'eux n'est repris, la phrase le dit, et chacun sa
    // question.
    let en_question = reponse.dire_les_identites();
    reponse.dire();
    let b = &reponse.bulletins;
    reponse.phrase = socle::phrase!(
        "Repris : {}, {} ; fiches écrites : {}.{}{}{}",
        socle::ecran::compte(reponse.contrats_nouveaux + b.contrats_nouveaux, "contrat", "contrats"),
        socle::ecran::compte(b.paies_nouvelles + b.paies_completees, "bulletin", "bulletins"),
        if fiches.is_empty() { "aucune".to_string() } else { fiches.join(", ") },
        titres,
        en_question,
        remplacees
    );
    reponse.fait();
    rapporter_les_ecarts(&frais)?;
    // L'alerte CASC-SVP lit les engagements et les fiches que l'import vient d'ecrire, comme la regularisation ; en
    // dernier, pour que son erreur ne saute aucune etape de l'import.
    if let Some(alerte) = alerte_apres_import(&frais)? {
        reponse.avertissements.push(alerte);
    }
    Ok(reponse)
}
