//! LES FICHES QUE L'IMPORT ECRIT, par le seul chemin des fiches (`routes::fiches` : la derivation, la verification,
//! l'ecriture datee) : la verification qu'en fera l'ecriture (`verifier_comme_a_l_ecriture`), une fiche creee
//! (`ecrire_fiche`), une fiche existante completee (`completer_fiche`). Ce que l'import en propose (une saisie par fiche,
//! composee des formes lues) : `import::proposee`.

use declarations::champs::{self, Anomalie, Saisie};
use droit::contexte::Contexte;
use droit::fiche::{FicheEntrepriseDatee, FicheNommee, FicheSalarieDatee, FicheSpectacleDatee, NomDeFiche};
use import::reponse::FicheVisee;
use socle::echec::{erreur, Classe, R};

use crate::fiches::{self, Forcer, SorteDeFiche};

/// LES ANOMALIES QUE L'ECRITURE TROUVERA : celles d'une fiche que l'import cree, derivee comme l'ecriture la derive
/// (`fiches::deriver` force : la forme declaree se refait de la forme lisible), puis verifiee. L'apercu de l'import les
/// montre telles : verifier la fiche avant derivation ferait crier chaque champ que le bulletin ou la DSN imprime en
/// minuscules (« 3 quai de la Fosse ») « doit etre en MAJUSCULES » sous sa forme lisible, alors que l'ecriture le declare
/// bien en majuscules. Chaque anomalie porte le libelle de sa case (`libelle`), celui que la fiche montre.
///
/// LE PONT, tant que l'import compose ses fiches en saisies : la saisie se lit en fiche typee (`droit::fiche::lire_saisie`,
/// comme a l'ecriture) ; une case que son type refuse refuse l'apercu entier.
pub(crate) fn verifier_comme_a_l_ecriture(nom: NomDeFiche, donnees: &Saisie) -> R<Vec<Anomalie>> {
    match nom {
        NomDeFiche::Entreprise => verifier::<FicheEntrepriseDatee>(donnees),
        NomDeFiche::Salarie => verifier::<FicheSalarieDatee>(donnees),
        NomDeFiche::Spectacle => verifier::<FicheSpectacleDatee>(donnees),
    }
}

fn verifier<F: SorteDeFiche>(donnees: &Saisie) -> R<Vec<Anomalie>> {
    let mut lue: F::Plate = droit::fiche::lire_saisie(donnees, F::FICHIER).map_err(|m| m.reclasser(Classe::Refuse))?;
    let fautes = F::deriver(&mut lue, true, &|_| true)?;
    let mut anomalies = champs::verifier_fiche(F::a_juger(&lue))?;
    anomalies.extend(champs::des_fautes(F::NOM, &fautes)?);
    fiches::avec_libelles(F::NOM, &mut anomalies)?;
    Ok(anomalies)
}

/// UNE FICHE QUE L'IMPORT CREE, ecrite comme celles de l'ecran (`fiches::ecrire_fiche`, `forcer` : une fiche importee
/// avec des anomalies est ecrite quand meme). Rend la fiche ecrite (nommee par sa cle ; l'entreprise, qui n'en a pas,
/// reste la fiche a creer qu'elle etait) et ses anomalies, chacune avec le libelle de sa case.
pub(crate) fn ecrire_fiche(ctx: &Contexte, nom: NomDeFiche, donnees: &Saisie, note: &str) -> R<(FicheVisee, Vec<Anomalie>)> {
    match nom {
        NomDeFiche::Entreprise => ecrire::<FicheEntrepriseDatee>(ctx, donnees, note),
        NomDeFiche::Salarie => ecrire::<FicheSalarieDatee>(ctx, donnees, note),
        NomDeFiche::Spectacle => ecrire::<FicheSpectacleDatee>(ctx, donnees, note),
    }
}

fn ecrire<F: SorteDeFiche>(ctx: &Contexte, donnees: &Saisie, note: &str) -> R<(FicheVisee, Vec<Anomalie>)> {
    let lue: F::Plate = droit::fiche::lire_saisie(donnees, F::FICHIER).map_err(|m| m.reclasser(Classe::Refuse))?;
    let p = fiches::preparer::<F>(ctx, None, |plate| poser_les_cases::<F>(plate, lue))?;
    let e = fiches::ecrire_fiche::<F>(ctx, p, Forcer::Tout, None, Some(note))?;
    let mut anomalies = e.anomalies;
    fiches::avec_libelles(F::NOM, &mut anomalies)?;
    let fiche = match e.cle {
        Some(k) => FicheVisee::Nommee(F::nommee(k)),
        None => FicheVisee::ACreer(F::NOM),
    };
    Ok((fiche, anomalies))
}

/// Les cases que `lue` dit, posees dans `plate` : une fiche vide la prend entiere, ses notes comprises. Rend les cases
/// posees.
fn poser_les_cases<F: SorteDeFiche>(plate: &mut F::Plate, lue: F::Plate) -> Vec<droit::fiche::CaseDeFiche> {
    let dites = F::differentes(&F::Plate::default(), &lue);
    if F::differentes(&F::Plate::default(), plate).is_empty() {
        *plate = lue;
        return dites;
    }
    for k in &dites {
        if let Some(ch) = F::changement(&lue, *k) {
            F::appliquer(plate, ch);
        }
    }
    dites
}

/// L'IMPORT COMPLETE UNE FICHE QUI EXISTE (`importation::completer`) : chaque case de `champs` va dans la fiche telle
/// qu'elle est sur le disque, si elle y est vide ou « A COMPLETER » -- et seulement alors : une case remplie depuis
/// l'apercu reste ce qu'elle est. Une case vide n'avait rien declare : la valeur qui la remplit vaut de tout temps
/// (`fiches::poser`). Rend les cases ecrites.
pub(crate) fn completer_fiche(ctx: &Contexte, fiche: &FicheNommee, champs: &Saisie) -> R<Saisie> {
    match fiche {
        FicheNommee::Entreprise => completer::<FicheEntrepriseDatee>(ctx, None, champs),
        FicheNommee::Salarie(cle) => completer::<FicheSalarieDatee>(ctx, Some(cle), champs),
        FicheNommee::Spectacle(cle) => completer::<FicheSpectacleDatee>(ctx, Some(cle), champs),
    }
}

fn completer<F: SorteDeFiche>(ctx: &Contexte, cle: Option<&F::Cle>, champs: &Saisie) -> R<Saisie> {
    let mut contenu = fiches::lire_fichier::<F>(ctx)?;
    let Some(brute) = F::existante(&mut contenu, cle) else {
        return Err(erreur(Classe::Refuse, format!("La fiche {} {} n'existe plus : rien n'est complete.", F::NOM, cle.map(F::cle_dite).into_iter().collect::<String>())));
    };
    let au = fiches::vue::<F>(&brute.au(&ctx.lue_au))?;
    let mut ecrits = Saisie::default();
    for (k, v) in champs.cases().iter().filter(|(k, _)| fiches::vide(au.valeur(*k))) {
        ecrits.poser(*k, v.clone());
    }
    for (k, t) in champs.notes().iter().filter(|(k, _)| au.note(k).is_none_or(|n| n.trim().is_empty())) {
        ecrits.poser_note(k, t.clone());
    }
    if !ecrits.est_dite() {
        return Ok(ecrits);
    }
    let lue: F::Plate = droit::fiche::lire_saisie(&ecrits, F::FICHIER).map_err(|m| m.reclasser(Classe::Refuse))?;
    let changements = F::differentes(&F::Plate::default(), &lue).into_iter().filter_map(|k| F::changement(&lue, k)).collect();
    fiches::poser::<F>(ctx, cle, changements, None, None)?;
    Ok(ecrits)
}
