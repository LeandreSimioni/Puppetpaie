//! LES FICHES QUE L'IMPORT PROPOSE, ENCORE EN JSON (une `Map` par fiche) : lues a son bord dans la saisie de leur fiche
//! (`champs::lecture`) ou dans sa forme (`droit::fiche::lire_saisie`), puis passees au seul chemin des fiches
//! (`routes::fiches` : la derivation, la verification, l'ecriture datee). Ce module tombe avec le JSON de l'import.

use serde_json::{Map, Value};

use declarations::champs::{self, Saisie};
use droit::contexte::Contexte;
use droit::fiche::{FicheEntrepriseDatee, FicheSalarieDatee, FicheSpectacleDatee};
use socle::echec::{erreur, Classe, R};

use crate::routes::fiches::{self, Forcer, SorteDeFiche};

/// La saisie de la fiche `nom` que l'import propose, lue par la table de ses cases : une case qui n'a pas sa forme se
/// refuse, avec son chemin.
fn saisie(nom: &str, donnees: &Map<String, Value>) -> R<Saisie> {
    Ok(socle::forme::lire_selon(&Value::Object(donnees.clone()), champs::lecture(nom)?, "Une case de la fiche n'a pas la forme que sa ligne décrit : la corriger.")?)
}

/// LES ANOMALIES QUE L'ECRITURE TROUVERA : celles d'une fiche que l'import cree, derivee comme l'ecriture la derive
/// (`fiches::deriver` force : la forme declaree se refait de la forme lisible), puis verifiee. L'apercu de l'import les
/// montre telles : verifier la fiche avant derivation ferait crier chaque champ que le bulletin ou la DSN imprime en
/// minuscules (« 3 quai de la Fosse ») « doit etre en MAJUSCULES » sous sa forme lisible, alors que l'ecriture le declare
/// bien en majuscules. Chaque anomalie porte le libelle de sa case (`libelle`), celui que la fiche montre.
pub(crate) fn verifier_comme_a_l_ecriture(nom: &str, donnees: &Map<String, Value>) -> R<Vec<Value>> {
    let mut s = saisie(nom, donnees)?;
    fiches::deriver(nom, &mut s, true, &|_| true)?;
    let mut anomalies = champs::verifier_fiche(nom, &s)?;
    fiches::avec_libelles(nom, &mut anomalies)?;
    Ok(anomalies.iter().map(socle::ecriture::en_valeur).collect())
}

/// LA FORME LISIBLE DE DEPART des fiches qui n'en ont pas (`champs::a_combler`), posee sur une copie de `donnees` : une
/// vue, pas une migration.
pub(crate) fn combler_affichage(nom: &str, donnees: &Map<String, Value>) -> R<Map<String, Value>> {
    let mut sortie = donnees.clone();
    for c in champs::a_combler(nom, &saisie(nom, donnees)?)? {
        match c.bloc {
            None => {
                sortie.insert(c.cle, Value::String(c.texte));
            }
            Some(b) => {
                if let Some(Value::Object(p)) = sortie.get_mut(b) {
                    p.insert(c.cle, Value::String(c.texte));
                }
            }
        }
    }
    Ok(sortie)
}

/// Les formes declarees de `donnees` fabriquees depuis leurs formes lisibles (`fiches::deriver`), posees dans `donnees`.
pub(crate) fn deriver(nom: &str, donnees: &mut Map<String, Value>, forcer: bool) -> R<()> {
    let mut s = saisie(nom, donnees)?;
    fiches::deriver(nom, &mut s, forcer, &|_| true)?;
    if let Value::Object(o) = socle::ecriture::en_valeur(&s) {
        donnees.extend(o);
    }
    Ok(())
}

/// UNE FICHE QUE L'IMPORT CREE, ecrite comme celles de l'ecran (`fiches::ecrire_fiche`, `forcer` : une fiche importee
/// avec des anomalies est ecrite quand meme). Rend sa cle et ses anomalies, chacune avec le libelle de sa case.
pub(crate) fn ecrire_fiche(ctx: &Contexte, nom: &str, donnees: &Map<String, Value>, note: Option<&str>) -> R<(Option<String>, Vec<Value>)> {
    match nom {
        "entreprise" => ecrire::<FicheEntrepriseDatee>(ctx, donnees, note),
        "salarie" => ecrire::<FicheSalarieDatee>(ctx, donnees, note),
        "spectacle" => ecrire::<FicheSpectacleDatee>(ctx, donnees, note),
        autre => Err(erreur(Classe::Refuse, format!("Fiche inconnue : {}", socle::fichiers::repr(autre)))),
    }
}

fn ecrire<F: SorteDeFiche>(ctx: &Contexte, donnees: &Map<String, Value>, note: Option<&str>) -> R<(Option<String>, Vec<Value>)> {
    let lue: F::Plate = droit::fiche::lire_saisie(donnees, F::FICHIER).map_err(|m| erreur(Classe::Refuse, m))?;
    let p = fiches::preparer::<F>(ctx, None, |plate| poser_les_cases::<F>(plate, lue))?;
    let e = fiches::ecrire_fiche::<F>(ctx, p, Forcer::Tout, None, note)?;
    let mut anomalies = e.anomalies;
    fiches::avec_libelles(F::NOM, &mut anomalies)?;
    Ok((e.cle, anomalies.iter().map(socle::ecriture::en_valeur).collect()))
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

/// L'IMPORT COMPLETE UNE FICHE QUI EXISTE (`import::completer`) : chaque champ de `champs` va dans la fiche telle qu'elle
/// est sur le disque, s'il y est vide ou « A COMPLETER » -- et seulement alors : un champ rempli depuis l'apercu reste ce
/// qu'il est. Une case vide n'avait rien declare : la valeur qui la remplit vaut de tout temps (`fiches::poser`). Rend
/// les champs ecrits.
pub(crate) fn completer_fiche(ctx: &Contexte, nom: &str, cle: Option<&str>, champs: &Map<String, Value>) -> R<Map<String, Value>> {
    match nom {
        "entreprise" => completer::<FicheEntrepriseDatee>(ctx, cle, champs),
        "salarie" => completer::<FicheSalarieDatee>(ctx, cle, champs),
        "spectacle" => completer::<FicheSpectacleDatee>(ctx, cle, champs),
        autre => Err(erreur(Classe::Refuse, format!("Fiche inconnue : {}", socle::fichiers::repr(autre)))),
    }
}

fn completer<F: SorteDeFiche>(ctx: &Contexte, cle: Option<&str>, champs: &Map<String, Value>) -> R<Map<String, Value>> {
    let mut contenu = fiches::lire_fichier::<F>(ctx)?;
    let Some(brute) = F::existante(&mut contenu, cle) else {
        return Err(erreur(Classe::Refuse, format!("La fiche {} {} n'existe plus : rien n'est complete.", F::NOM, cle.into_iter().collect::<String>())));
    };
    let au = fiches::vue::<F>(&brute.au(&ctx.lue_au))?;
    let ecrits: Map<String, Value> = champs.iter().filter(|(k, _)| fiches::vide(au.case(k))).map(|(k, v)| (k.clone(), v.clone())).collect();
    if ecrits.is_empty() {
        return Ok(ecrits);
    }
    let lue: F::Plate = droit::fiche::lire_saisie(&ecrits, F::FICHIER).map_err(|m| erreur(Classe::Refuse, m))?;
    let changements = F::differentes(&F::Plate::default(), &lue).into_iter().filter_map(|k| F::changement(&lue, k)).collect();
    fiches::poser::<F>(ctx, cle, changements, None, None)?;
    Ok(ecrits)
}
