//! LES AUTRES ETABLISSEMENTS DE LA COMPAGNIE (`etablissements` de la fiche entreprise) : ce que l'ecran en dit
//! (Configuration › Entreprise › Avance, « Autres etablissements ») et comment la fiche les date.
//!
//! La fiche porte une liste datee des etablissements ouverts : une version a chaque ouverture, a chaque modification et a
//! chaque fermeture. Toute la configuration est datee : une ligne qu'on modifie vaut a partir de la date d'effet que la
//! boite « Ce qui change » demande, et ce qui precede garde l'ancienne valeur ; une ligne qu'on retire ferme
//! l'etablissement a partir de cette date (une date au jour de son ouverture, ou avant, l'efface : il n'a jamais ouvert).
//! Une ligne neuve se date a son ouverture, et corriger une ouverture ne demande pas de date.
//!
//! Chacun dit son SIRET (du SIREN de la compagnie), son adresse, son ouverture et son code risque AT/MP : le taux est
//! determine par etablissement et le classement suit l'activite qui y est exercee (D. 242-6-1,
//! `atelier/reference/sources/code-de-la-securite-sociale/xml/D242-6-1.xml` ; arrete du 17/10/1995, art. 1, I, 1°,
//! `tarification-atmp/`). Un code laisse vide prend celui que la compagnie a pour l'annee de l'ouverture (INFERENCE :
//! l'activite de l'etablissement est celle de la compagnie tant que la notification de la CARSAT ne dit pas autre chose),
//! et l'ecran le dit (`taux-atmp`, `atmp::route`). Le taux AT/MP est une ligne comme celle du siege (`routes::atmp`) : il
//! vaut a partir de sa date (`depuis`, celle de la notification), et la ligne qui le change se date a ce jour-la
//! (`jour_du_changement`) ; la paie le lit par la meme regle que celui du siege (`droit::bareme::surcharger`).

use std::collections::BTreeMap;

use droit::contexte::Contexte;
use droit::fiche::{ChampsEntreprise, Datee, Etablissement, FicheEntrepriseDatee, Lue as _};
use socle::date::Jour;
use socle::echec::R;

use crate::routes::fiches::{refus, ChangementDit};

fn nic(e: &Etablissement) -> Option<String> {
    e.nic.en_texte()
}

/// Son nom : son libelle, sinon sa commune, sinon son NIC.
fn nom(e: &Etablissement) -> String {
    let commune = e.commune_affichage.remplie().or(e.commune.portee());
    match (e.libelle.remplie(), commune, e.nic.portee()) {
        (Some(l), _, _) => l.to_string(),
        (None, Some(c), _) => c.to_string(),
        (None, None, Some(n)) => n.to_string(),
        (None, None, None) => "Établissement sans nom".to_string(),
    }
}

/// Ce qu'une ligne dit, sans ce qui ne se date pas : son ouverture, son NIC qui se lit dans le SIRET, la forme lisible de
/// son adresse, d'ou vient son taux AT/MP ; une structure vide n'y est pas.
fn comparable(e: &Etablissement) -> Etablissement {
    let taux_atmp = e.taux_atmp.clone().map(|t| droit::fiche::TauxAtmp { provenance: None, notification: None, ..t }).filter(|t| !t.est_vide());
    let versement_mobilite = e.versement_mobilite.clone().filter(|v| !v.est_vide());
    Etablissement {
        ouverture: None,
        nic: None,
        adresse_affichage: None,
        complement_adresse_affichage: None,
        commune_affichage: None,
        taux_atmp,
        versement_mobilite,
        ..e.clone()
    }
}

/// Une ligne lisible : ce que la boite « Ce qui change » montre.
fn lisible(e: &Etablissement) -> String {
    let adresse = [
        e.adresse_affichage.remplie().or(e.adresse.portee()),
        e.code_postal.portee(),
        e.commune_affichage.remplie().or(e.commune.portee()),
    ]
    .into_iter()
    .flatten()
    .filter(|s| !s.trim().is_empty())
    .collect::<Vec<_>>()
    .join(" ");
    let mut morceaux = vec![nom(e), format!("SIRET {}", e.siret.remplie().map(str::to_string).into_iter().collect::<String>()), adresse];
    if let Some(t) = &e.taux_atmp {
        if let Some(code) = t.code_risque.remplie() {
            let taux = t.valeur.as_ref().map(|v| format!(", taux {} %", v.ecrit().replace('.', ","))).into_iter().collect::<String>();
            morceaux.push(format!("code risque {}{}", code, taux));
        }
    }
    if let Some(vm) = &e.versement_mobilite {
        let taux = vm.taux.as_ref().map(|t| t.ecrit().replace('.', ",")).into_iter().collect::<String>();
        morceaux.push(format!("versement mobilité {} %", taux));
    }
    morceaux.into_iter().filter(|s| !s.trim().is_empty()).collect::<Vec<_>>().join(" · ")
}

/// UN CHANGEMENT QUI SE DATE : un etablissement deja ouvert dont la ligne change (`apres`), ou qu'on retire (`apres` :
/// `None`, il ferme).
pub(crate) struct Changement {
    pub nom: String,
    pub avant: Etablissement,
    pub apres: Option<Etablissement>,
}

impl Changement {
    /// L'entree de la boite « Ce qui change » (`fiches::changements_dits`).
    pub(crate) fn dit(&self) -> ChangementDit {
        let apres = match &self.apres {
            Some(e) => lisible(e),
            None => "fermé".to_string(),
        };
        ChangementDit::en_mots("etablissements", format!("Établissement « {} »", self.nom), lisible(&self.avant), apres)
    }
}

/// CE QUI SE DATE entre la liste `anciens` (les etablissements ouverts, tels que la fiche les dit) et la liste `nouveaux`
/// (telle que l'ecran l'envoie, derivee : `fiches::deriver`). Une ligne neuve ne se date pas (son ouverture la date), une
/// ouverture corrigee non plus.
pub(crate) fn a_dater(anciens: &[Etablissement], nouveaux: &[Etablissement]) -> Vec<Changement> {
    let mut out = Vec::new();
    for a in anciens {
        match nouveaux.iter().find(|n| nic(n) == nic(a)) {
            None => out.push(Changement { nom: nom(a), avant: a.clone(), apres: None }),
            Some(n) if comparable(n) != comparable(a) => out.push(Changement { nom: nom(a), avant: a.clone(), apres: Some(n.clone()) }),
            Some(_) => {}
        }
    }
    out
}

/// La frise d'un etablissement : a partir de chaque jour, sa ligne (`None` : ferme).
type Frise = BTreeMap<Jour, Option<Etablissement>>;

fn au(frise: &Frise, jour: &Jour) -> Option<Etablissement> {
    frise.range(..=*jour).next_back().and_then(|(_, v)| v.clone())
}

/// LA LIGNE DU TAUX AT/MP D'UN ETABLISSEMENT, telle que l'ecran la renvoie (sans `provenance` ni `notification`, que la
/// liste ne garde pas) : la meme que celle de la fiche (`avant`), elle reste ce qu'elle etait ; autre, elle est celle de
/// la personne, sauf celle que le moteur pose (l'arrete), qui dit deja la sienne.
fn provenance_de(ligne: &mut Etablissement, avant: Option<&Etablissement>) {
    let Some(t) = ligne.taux_atmp.as_mut() else { return };
    match avant.and_then(|a| a.taux_atmp.as_ref()) {
        Some(a) if droit::bareme::meme_taux_atmp(a, t) => *t = a.clone(),
        _ if t.provenance.is_none() && !t.est_vide() => {
            t.provenance = Some(droit::fiche::ProvenanceAtmp::Saisie);
            t.notification = None;
        }
        _ => {}
    }
}

/// Le jour ou une ligne qui change vaut : celui de son nouveau taux AT/MP (la date de la notification) quand c'est lui qui
/// change, sinon la date d'effet de la boite (`d`).
fn jour_du_changement(avant: &Etablissement, apres: &Etablissement, d: Jour) -> Jour {
    match (comparable(avant).taux_atmp != comparable(apres).taux_atmp, apres.taux_atmp.as_ref().and_then(|t| t.depuis)) {
        (true, Some(j)) => j,
        _ => d,
    }
}

/// ECRIRE LA LISTE QUE L'ECRAN DONNE (`liste_ecran`) dans la fiche entreprise `contenu`, datee : voir l'en-tete. `siege`
/// est la fiche entreprise ecrite (son SIREN, son NIC) ; `depuis` est la date d'effet de la boite « Ce qui change » ; sans
/// elle, un changement qui se date se refuse.
pub(crate) fn ecrire(ctx: &Contexte, contenu: &mut FicheEntrepriseDatee, siege: &ChampsEntreprise, liste_ecran: &[Etablissement], depuis: Option<Jour>, note: Option<&str>) -> R<()> {
    if contenu.etablissements.is_none() && liste_ecran.is_empty() {
        // Une compagnie qui n'en a jamais eu : rien a dater.
        return Ok(());
    }
    let siren = siege.siren.remplie().map(str::trim);
    let mut nouvelles: Vec<(Jour, Etablissement)> = Vec::new();
    let mut griefs: Vec<String> = Vec::new();
    for (i, e) in liste_ecran.iter().enumerate() {
        let mut o = e.clone();
        let nom = match o.libelle.remplie().filter(|l| !l.trim().is_empty()) {
            Some(l) => l.to_string(),
            None => format!("Établissement {}", i + 1),
        };
        match (o.siret.remplie(), siren) {
            (Some(s), _) if s.chars().count() != 14 || !s.chars().all(|c| c.is_ascii_digit()) => {
                griefs.push(format!("{} : son SIRET (quatorze chiffres, sur l'avis de situation Sirene)", nom))
            }
            (None, _) => griefs.push(format!("{} : son SIRET (quatorze chiffres, sur l'avis de situation Sirene)", nom)),
            (Some(s), Some(siren)) if s[..9] != *siren => griefs.push(format!("{} : le SIRET {} n'est pas de la compagnie (son SIREN est {})", nom, s, siren)),
            _ => {}
        }
        for (dite, mot) in [(o.adresse.portee().is_some(), "son adresse"), (o.code_postal.portee().is_some(), "son code postal"), (o.commune.portee().is_some(), "sa commune")] {
            if !dite {
                griefs.push(format!("{} : {}", nom, mot));
            }
        }
        if let Some(vm) = &o.versement_mobilite {
            let paye = vm.taux.as_ref().map(droit::fiche::Montant::decimal).transpose().map_err(refus)?.is_some_and(|d| !d.is_zero());
            if paye && vm.code_insee.remplie().map(|c| c.trim().chars().count()) != Some(5) {
                griefs.push(format!("{} : le code INSEE de sa commune (cinq caractères), que la DSN déclare avec le versement mobilité", nom));
            }
        }
        let Some(ouverture) = o.ouverture else {
            griefs.push(format!("{} : sa date d'ouverture", nom));
            continue;
        };
        if o.taux_atmp.as_ref().and_then(|t| t.code_risque.remplie()).is_none() {
            let du_siege = ctx.entreprise_au(&ouverture).taux_atmp.and_then(|t| t.code_risque).filter(|c| !c.is_empty());
            match du_siege {
                Some(code) => o.taux_atmp.get_or_insert_with(droit::fiche::TauxAtmp::default).code_risque = Some(code),
                None => griefs.push(format!("{} : son code risque AT/MP, tel que la CARSAT le notifie", nom)),
            }
        }
        if let Some(m) = o.taux_atmp.as_ref().map(droit::bareme::taux_atmp_refuse).transpose().map_err(refus)?.flatten() {
            griefs.push(format!("{} : {}", nom, m));
        }
        nouvelles.push((ouverture, o));
    }
    if !griefs.is_empty() {
        return Err(refus(format!("Autres établissements : il manque {}. Rien n'a été enregistré.", griefs.join(" ; "))));
    }

    // L'histoire telle que la fiche la dit : une frise par etablissement.
    let versions: Vec<(Option<Jour>, Vec<Etablissement>)> =
        contenu.etablissements.iter().flat_map(|l| droit::fiche::valeurs_datees(l)).map(|e| (e.depuis, e.valeur.iter().flatten().cloned().collect())).collect();
    let ouverts: Vec<Etablissement> = versions.last().map(|(_, l)| l.clone()).into_iter().flatten().collect();
    let mut frises: BTreeMap<Option<String>, Frise> = BTreeMap::new();
    for (i, (j, l)) in versions.iter().enumerate() {
        let Some(j) = j.or_else(|| versions.get(i + 1).and_then(|(x, _)| *x)) else { continue };
        for f in frises.values_mut() {
            f.insert(j, None);
        }
        for e in l {
            frises.entry(nic(e)).or_default().insert(j, Some(e.clone()));
        }
    }
    for f in frises.values_mut() {
        // Les fermetures repetees ne disent rien de plus.
        let mut avant: Option<Option<Etablissement>> = None;
        f.retain(|_, v| {
            let garde = avant.as_ref() != Some(v);
            avant = Some(v.clone());
            garde
        });
    }

    for (_, o) in nouvelles.iter_mut() {
        let n = nic(o);
        provenance_de(o, ouverts.iter().find(|a| nic(a) == n));
    }
    let lignes: Vec<Etablissement> = nouvelles.iter().map(|(_, e)| e.clone()).collect();
    for c in &a_dater(&ouverts, &lignes) {
        let d = depuis.ok_or_else(|| {
            refus(format!(
                "Changer l'établissement « {} » demande sa date d'effet (« à partir du ») : ce qui précède garde l'ancienne \
valeur, et la date ne se devine pas. Rien n'a été enregistré.",
                c.nom
            ))
        })?;
        let n = nic(&c.avant);
        let f = frises.entry(n.clone()).or_default();
        match &c.apres {
            None => {
                let ouvert_le = f.iter().find(|(_, v)| v.is_some()).map(|(j, _)| *j);
                if ouvert_le.is_some_and(|o| d <= o) {
                    // Retire avant d'avoir ouvert : il n'a jamais ete.
                    frises.remove(&n);
                } else {
                    f.retain(|j, _| *j < d);
                    f.insert(d, None);
                }
            }
            Some(apres) => {
                f.insert(jour_du_changement(&c.avant, apres, d), Some(apres.clone()));
            }
        }
    }
    for (ouverture, ligne) in &nouvelles {
        let n = nic(ligne);
        let deja = ouverts.iter().any(|a| nic(a) == n);
        let f = frises.entry(n.clone()).or_default();
        if f.is_empty() {
            // Un etablissement neuf : il vaut a son ouverture.
            f.insert(*ouverture, Some(ligne.clone()));
            continue;
        }
        if !deja {
            // Ferme, il rouvre a la date d'effet.
            let d = depuis.ok_or_else(|| refus(format!("Rouvrir l'établissement « {} » demande sa date (« à partir du »). Rien n'a été enregistré.", nom(ligne))))?;
            f.retain(|j, _| *j < d);
            f.insert(d, Some(ligne.clone()));
            continue;
        }
        // L'ouverture se corrige : la ligne vaut a partir d'elle, avec ce qu'elle disait alors.
        let premiere = f.iter().find(|(_, v)| v.is_some()).map(|(j, _)| *j);
        if premiere != Some(*ouverture) {
            let a_l_ouverture = au(f, ouverture).or_else(|| f.values().find_map(|v| v.clone()));
            f.retain(|j, _| j > ouverture);
            f.insert(*ouverture, a_l_ouverture);
        }
        for v in f.values_mut().flatten() {
            v.ouverture = Some(*ouverture);
        }
    }

    // La liste datee : a chaque jour ou une frise change, les etablissements ouverts.
    let mut jours: Vec<Jour> = frises.values().flat_map(|f| f.keys().copied()).collect();
    jours.sort();
    jours.dedup();
    let mut datee = vec![Datee { depuis: None, valeur: Some(Vec::new()), note: None }];
    let mut precedente: Vec<Etablissement> = Vec::new();
    for j in jours {
        let mut ouverts_ce_jour: Vec<(Option<Jour>, Option<String>, Etablissement)> = frises.iter().filter_map(|(n, f)| au(f, &j).map(|v| (v.ouverture, n.clone(), v))).collect();
        ouverts_ce_jour.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
        let valeur: Vec<Etablissement> = ouverts_ce_jour.into_iter().map(|(_, _, v)| v).collect();
        if valeur == precedente {
            continue;
        }
        let note = note.map(str::trim).filter(|n| !n.is_empty() && Some(j) == depuis).map(str::to_string);
        datee.push(Datee { depuis: Some(j), valeur: Some(valeur.clone()), note });
        precedente = valeur;
    }

    // Le controle que toute lecture de la configuration fait (`droit::contexte`), sur ce qui est ouvert, avant d'ecrire :
    // contre le bareme de cette annee et de l'annee de chaque ouverture (une annee sans socle livre ne se controle pas).
    let fiche = ChampsEntreprise { siren: siege.siren.clone(), nic: siege.nic.clone(), etablissements: Some(lignes), ..ChampsEntreprise::default() };
    let mut annees: Vec<i32> = std::iter::once(ctx.aujourdhui.an).chain(nouvelles.iter().map(|(o, _)| o.an)).collect();
    annees.sort();
    annees.dedup();
    for an in annees {
        let Some(premier) = Jour::nouveau(an, 1, 1) else { continue };
        if droit::contexte::socle_livre_lu(an).map_err(refus)?.is_some() {
            let bar = ctx.bareme_fondu(&premier).map_err(refus)?;
            droit::contexte::controler_etablissements(&fiche, &bar).map_err(refus)?;
        }
    }
    contenu.etablissements = Some(datee);
    Ok(())
}

/// LA FICHE ENTREPRISE TELLE QUE L'ECRAN LA MONTRE : ses autres etablissements sont la derniere liste de la fiche
/// (`Contexte::etablissements_dits`), ceux qui ouvrent plus tard compris ; c'est contre elle qu'un enregistrement se
/// compare.
pub(crate) fn entreprise_montree(ctx: &Contexte) -> ChampsEntreprise {
    let mut e = ctx.entreprise.clone();
    if e.etablissements.is_some() || ctx.entreprise_brute().etablissements.is_some() {
        e.etablissements = Some(ctx.etablissements_dits());
    }
    e
}
