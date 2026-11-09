//! LES FICHES QUE L'IMPORT PROPOSE : chacune une saisie de sa fiche (`declarations::champs::Saisie`), comme ce que
//! l'ecran saisit, case par case dans la forme que sa ligne de `champs-base.json` lui donne. L'import la compose depuis
//! les formes qu'il lit (la DSN, le bulletin, l'AEM, le certificat : `de`) ; il l'ecrit ensuite par le seul chemin des
//! fiches (`importation::fiches`).

use serde::Serialize;

use declarations::champs::Saisie;
use socle::echec::R;

/// Le titre que l'import donne a un spectacle dont il ne connait que le numero d'objet.
pub(crate) const A_COMPLETER: &str = "A COMPLETER";

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
