//! LES FICHES QUE L'IMPORT PROPOSE : chacune une saisie de sa fiche (`declarations::champs::Saisie`), comme ce que
//! l'ecran saisit, case par case dans la forme que sa ligne de `champs-base.json` lui donne. L'import la compose depuis
//! les formes qu'il lit (la DSN, le bulletin, l'AEM, le certificat : `de`), puis la passe au seul chemin des fiches
//! (`routes::fiches` : la derivation, la verification, l'ecriture datee).

use serde::Serialize;

use declarations::champs::{self, Anomalie, Saisie};
use droit::contexte::Contexte;
use droit::fiche::{FicheEntrepriseDatee, FicheSalarieDatee, FicheSpectacleDatee};
use socle::echec::{erreur, Classe, R};

use crate::routes::fiches::{self, Forcer, SorteDeFiche};

/// LA FICHE `nom` QUE DIT UNE FORME LUE (`t` : ses champs nommes comme ceux de la fiche), relue par la table de ses
/// cases : une case vide n'y est pas ; une case qui n'a pas sa forme se refuse, avec son chemin.
pub(crate) fn de<T: Serialize + ?Sized>(nom: &str, t: &T) -> R<Saisie> {
    Saisie::de_la_fiche(nom, t)
}

/// Les cases et les notes de `autre` posees dans `s`, a la place de celles qu'il avait.
pub(crate) fn fondre(s: &mut Saisie, autre: &Saisie) {
    for (k, v) in autre.cases() {
        s.poser(k, v.clone());
    }
    for (k, t) in autre.notes() {
        s.poser_note(k, t.clone());
    }
}

/// Les cases et les notes de `autre` que `s` ne dit pas encore, posees dans `s` : la premiere piece qui en dit une la
/// garde.
pub(crate) fn completer_de(s: &mut Saisie, autre: &Saisie) {
    for (k, v) in autre.cases() {
        if s.case(k).is_none() {
            s.poser(k, v.clone());
        }
    }
    for (k, t) in autre.notes() {
        if s.note(k).is_none() {
            s.poser_note(k, t.clone());
        }
    }
}

/// LES ANOMALIES QUE L'ECRITURE TROUVERA : celles d'une fiche que l'import cree, derivee comme l'ecriture la derive
/// (`fiches::deriver` force : la forme declaree se refait de la forme lisible), puis verifiee. L'apercu de l'import les
/// montre telles : verifier la fiche avant derivation ferait crier chaque champ que le bulletin ou la DSN imprime en
/// minuscules (« 3 quai de la Fosse ») « doit etre en MAJUSCULES » sous sa forme lisible, alors que l'ecriture le declare
/// bien en majuscules. Chaque anomalie porte le libelle de sa case (`libelle`), celui que la fiche montre.
pub(crate) fn verifier_comme_a_l_ecriture(nom: &str, donnees: &Saisie) -> R<Vec<Anomalie>> {
    let mut s = donnees.clone();
    fiches::deriver(nom, &mut s, true, &|_| true)?;
    let mut anomalies = champs::verifier_fiche(nom, &s)?;
    fiches::avec_libelles(nom, &mut anomalies)?;
    Ok(anomalies)
}

/// UNE FICHE QUE L'IMPORT CREE, ecrite comme celles de l'ecran (`fiches::ecrire_fiche`, `forcer` : une fiche importee
/// avec des anomalies est ecrite quand meme). Rend sa cle et ses anomalies, chacune avec le libelle de sa case.
pub(crate) fn ecrire_fiche(ctx: &Contexte, nom: &str, donnees: &Saisie, note: &str) -> R<(Option<String>, Vec<Anomalie>)> {
    match nom {
        "entreprise" => ecrire::<FicheEntrepriseDatee>(ctx, donnees, note),
        "salarie" => ecrire::<FicheSalarieDatee>(ctx, donnees, note),
        "spectacle" => ecrire::<FicheSpectacleDatee>(ctx, donnees, note),
        autre => Err(erreur(Classe::Refuse, format!("Fiche inconnue : {}", socle::fichiers::repr(autre)))),
    }
}

fn ecrire<F: SorteDeFiche>(ctx: &Contexte, donnees: &Saisie, note: &str) -> R<(Option<String>, Vec<Anomalie>)> {
    let lue: F::Plate = droit::fiche::lire_saisie(donnees, F::FICHIER).map_err(|m| erreur(Classe::Refuse, m))?;
    let p = fiches::preparer::<F>(ctx, None, |plate| poser_les_cases::<F>(plate, lue))?;
    let e = fiches::ecrire_fiche::<F>(ctx, p, Forcer::Tout, None, Some(note))?;
    let mut anomalies = e.anomalies;
    fiches::avec_libelles(F::NOM, &mut anomalies)?;
    Ok((e.cle, anomalies))
}

/// Les cases que `lue` dit, posees dans `plate` : une fiche vide la prend entiere, ses notes comprises. Rend les cases
/// posees.
fn poser_les_cases<F: SorteDeFiche>(plate: &mut F::Plate, lue: F::Plate) -> Vec<&'static str> {
    let dites = F::differentes(&F::Plate::default(), &lue);
    if F::differentes(&F::Plate::default(), plate).is_empty() {
        *plate = lue;
        return dites;
    }
    for k in &dites {
        if let Some(ch) = F::changement(&lue, k) {
            F::appliquer(plate, ch);
        }
    }
    dites
}

/// L'IMPORT COMPLETE UNE FICHE QUI EXISTE (`import::completer`) : chaque case de `champs` va dans la fiche telle qu'elle
/// est sur le disque, si elle y est vide ou « A COMPLETER » -- et seulement alors : une case remplie depuis l'apercu
/// reste ce qu'elle est. Une case vide n'avait rien declare : la valeur qui la remplit vaut de tout temps
/// (`fiches::poser`). Rend les cases ecrites.
pub(crate) fn completer_fiche(ctx: &Contexte, nom: &str, cle: Option<&str>, champs: &Saisie) -> R<Saisie> {
    match nom {
        "entreprise" => completer::<FicheEntrepriseDatee>(ctx, cle, champs),
        "salarie" => completer::<FicheSalarieDatee>(ctx, cle, champs),
        "spectacle" => completer::<FicheSpectacleDatee>(ctx, cle, champs),
        autre => Err(erreur(Classe::Refuse, format!("Fiche inconnue : {}", socle::fichiers::repr(autre)))),
    }
}

fn completer<F: SorteDeFiche>(ctx: &Contexte, cle: Option<&str>, champs: &Saisie) -> R<Saisie> {
    let mut contenu = fiches::lire_fichier::<F>(ctx)?;
    let Some(brute) = F::existante(&mut contenu, cle) else {
        return Err(erreur(Classe::Refuse, format!("La fiche {} {} n'existe plus : rien n'est complete.", F::NOM, cle.into_iter().collect::<String>())));
    };
    let au = fiches::vue::<F>(&brute.au(&ctx.lue_au))?;
    let mut ecrits = Saisie::default();
    for (k, v) in champs.cases().iter().filter(|(k, _)| fiches::vide(au.case(k))) {
        ecrits.poser(k, v.clone());
    }
    for (k, t) in champs.notes().iter().filter(|(k, _)| au.note(k).is_none_or(|n| n.trim().is_empty())) {
        ecrits.poser_note(k, t.clone());
    }
    if !ecrits.est_dite() {
        return Ok(ecrits);
    }
    let lue: F::Plate = droit::fiche::lire_saisie(&ecrits, F::FICHIER).map_err(|m| erreur(Classe::Refuse, m))?;
    let changements = F::differentes(&F::Plate::default(), &lue).into_iter().filter_map(|k| F::changement(&lue, k)).collect();
    fiches::poser::<F>(ctx, cle, changements, None, None)?;
    Ok(ecrits)
}
