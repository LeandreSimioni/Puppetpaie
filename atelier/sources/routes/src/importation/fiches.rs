//! LES FICHES QUE L'IMPORT ECRIT, par le seul chemin des fiches (`routes::fiches` : la derivation, la verification,
//! l'ecriture datee) : la verification qu'en fera l'ecriture (`verifier_comme_a_l_ecriture`), une fiche creee
//! (`ecrire_fiche`), une fiche existante completee (`completer_fiche`). Ce que l'import en propose (une fiche typee par
//! fiche, lue des formes des pieces, et les fautes des cases qu'une piece portait hors de leur type) :
//! `import::proposee`.

use declarations::champs::{self, Anomalie};
use droit::contexte::Contexte;
use droit::fiche::{FauteDeCase, FicheEntrepriseDatee, FicheNommee, FicheSaisie, FicheSalarieDatee, FicheSpectacleDatee};
use import::proposee::{deux_sortes, FicheProposee};
use import::reponse::FicheVisee;
use socle::echec::{erreur, Classe, R};

use crate::fiches::{self, Forcer, SorteDeFiche};

/// LES ANOMALIES QUE L'ECRITURE TROUVERA : celles d'une fiche que l'import cree, derivee comme l'ecriture la derive
/// (`SorteDeFiche::deriver` force : la forme declaree se refait de la forme lisible), puis verifiee. L'apercu de l'import
/// les montre telles : verifier la fiche avant derivation ferait crier chaque champ que le bulletin ou la DSN imprime en
/// minuscules (« 3 quai de la Fosse ») « doit etre en MAJUSCULES » sous sa forme lisible, alors que l'ecriture le declare
/// bien en majuscules. Chaque anomalie porte le libelle de sa case (`libelle`), celui que la fiche montre.
///
/// LES FAUTES DES PIECES (`fautes` : une valeur qu'une piece portait hors du type de sa case) se disent avec, chacune
/// sur sa case, tant que la case reste vide : elle se redemande (`redemandees`).
pub(crate) fn verifier_comme_a_l_ecriture(donnees: &FicheProposee, fautes: &[FauteDeCase]) -> R<Vec<Anomalie>> {
    match donnees {
        FicheProposee::Entreprise(p) => verifier::<FicheEntrepriseDatee>(p, fautes),
        FicheProposee::Salarie(p) => verifier::<FicheSalarieDatee>(p, fautes),
        FicheProposee::Spectacle(p) => verifier::<FicheSpectacleDatee>(p, fautes),
    }
}

fn verifier<F: SorteDeFiche>(donnees: &F::Plate, fautes: &[FauteDeCase]) -> R<Vec<Anomalie>> {
    let mut lue = donnees.clone();
    let derivees = F::deriver(&mut lue, true, &|_| true)?;
    let mut anomalies = champs::verifier_fiche(F::a_juger(&lue))?;
    anomalies.extend(champs::des_fautes(F::NOM, &derivees)?);
    anomalies.extend(champs::des_fautes(F::NOM, &restent_vides(&lue, fautes))?);
    fiches::avec_libelles(F::NOM, &mut anomalies)?;
    Ok(anomalies)
}

/// Les fautes des pieces dont la case reste vide dans `fiche` : une case qu'une autre piece, ou la fiche, remplit ne se
/// redemande pas.
fn restent_vides<P: FicheSaisie>(fiche: &P, fautes: &[FauteDeCase]) -> Vec<FauteDeCase> {
    fautes.iter().filter(|f| !fiche.dite(f.case)).cloned().collect()
}

/// LES FAUTES DES PIECES QUI SE REDEMANDENT A UNE FICHE QUI EXISTE : celles dont la case y reste vide, en anomalies de sa
/// fiche, chacune sur sa case et avec son libelle.
pub(crate) fn redemandees<P: FicheSaisie>(fiche: &P, fautes: &[FauteDeCase]) -> R<Vec<Anomalie>> {
    let mut anomalies = champs::des_fautes(P::NOM, &restent_vides(fiche, fautes))?;
    fiches::avec_libelles(P::NOM, &mut anomalies)?;
    Ok(anomalies)
}

/// UNE FICHE QUE L'IMPORT CREE, ecrite comme celles de l'ecran (`fiches::ecrire_fiche`, `forcer` : une fiche importee
/// avec des anomalies est ecrite quand meme). Rend la fiche ecrite (nommee par sa cle ; l'entreprise, qui n'en a pas,
/// reste la fiche a creer qu'elle etait) et ses anomalies, chacune avec le libelle de sa case.
pub(crate) fn ecrire_fiche(ctx: &Contexte, donnees: &FicheProposee, note: &str) -> R<(FicheVisee, Vec<Anomalie>)> {
    match donnees {
        FicheProposee::Entreprise(p) => ecrire::<FicheEntrepriseDatee>(ctx, p.clone(), note),
        FicheProposee::Salarie(p) => ecrire::<FicheSalarieDatee>(ctx, p.clone(), note),
        FicheProposee::Spectacle(p) => ecrire::<FicheSpectacleDatee>(ctx, p.clone(), note),
    }
}

fn ecrire<F: SorteDeFiche>(ctx: &Contexte, lue: F::Plate, note: &str) -> R<(FicheVisee, Vec<Anomalie>)> {
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
/// (`fiches::poser`). Rend les cases et les notes retenues.
pub(crate) fn completer_fiche(ctx: &Contexte, fiche: &FicheNommee, champs: &FicheProposee) -> R<FicheProposee> {
    match (fiche, champs) {
        (FicheNommee::Entreprise, FicheProposee::Entreprise(c)) => completer::<FicheEntrepriseDatee>(ctx, None, c),
        (FicheNommee::Salarie(cle), FicheProposee::Salarie(c)) => completer::<FicheSalarieDatee>(ctx, Some(cle), c),
        (FicheNommee::Spectacle(cle), FicheProposee::Spectacle(c)) => completer::<FicheSpectacleDatee>(ctx, Some(cle), c),
        (f, c) => Err(deux_sortes(f.nom(), c.nom())),
    }
}

fn completer<F: SorteDeFiche>(ctx: &Contexte, cle: Option<&F::Cle>, champs: &F::Plate) -> R<FicheProposee> {
    let mut contenu = fiches::lire_fichier::<F>(ctx)?;
    let Some(brute) = F::existante(&mut contenu, cle) else {
        return Err(erreur(Classe::Refuse, format!("La fiche {} {} n'existe plus : rien n'est complete.", F::NOM, cle.map(F::cle_dite).into_iter().collect::<String>())));
    };
    let au = brute.au(&ctx.lue_au);
    let mut ecrits = F::Plate::default();
    for k in champs.cases_dites().into_iter().filter(|k| fiches::case_vide(au.valeur_ecrite(*k).as_ref())) {
        ecrits.prendre(champs, k);
    }
    let notes_de_la_fiche = au.notes_dites();
    let vide = |k: &str| notes_de_la_fiche.iter().find(|(n, _)| *n == k).is_none_or(|(_, t)| t.trim().is_empty());
    let notes_vides: Vec<&str> = champs.notes_dites().into_iter().map(|(k, _)| k).filter(|k| vide(k)).collect();
    for k in notes_vides {
        ecrits.prendre_la_note(champs, k);
    }
    if ecrits.est_vide() && ecrits.notes_dites().is_empty() {
        return Ok(F::proposee(ecrits));
    }
    let changements = F::differentes(&F::Plate::default(), &ecrits).into_iter().filter_map(|k| F::changement(&ecrits, k)).collect();
    fiches::poser::<F>(ctx, cle, changements, None, None)?;
    Ok(F::proposee(ecrits))
}
