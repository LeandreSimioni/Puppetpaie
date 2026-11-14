//! CE QUE L'IMPORT MET DANS UNE FICHE QUI EXISTE DEJA : tout ce qu'il peut tirer des imports, sans que la fiche fasse loi
//! contre un bulletin qui a les bonnes informations.
//!
//! Une fiche que l'import CREE recoit tout ce que la DSN et les bulletins disent. Une fiche qui EXISTE recoit ce qui y
//! est vide ou « A COMPLETER » (sans casse : le titre que l'import des DSN donne a un spectacle dont la DSN ne porte que
//! le numero d'objet, `import::proposee::A_COMPLETER`), et garde tout champ rempli : la fiche fait foi, l'ecart reste
//! seulement signale (`ecarts`). L'apercu dit, fiche par fiche, ce qui serait complete et d'ou ca vient (`completer`) ;
//! l'import l'ecrit (`importation::fiches::completer_fiche`), sans rien dater : une case vide n'avait rien declare.
//!
//! Un champ que la piece ne porte pas ne s'invente pas (regle 1) : on ne complete que de ce que la DSN ou le bulletin
//! imprime, par le meme chemin que la fiche qu'il creerait. La fiche qui existe et ce que l'import en ferait se
//! confrontent case par case, chacune la valeur ecrite de sa case (`droit::fiche::ValeurDeCase`).

use declarations::champs::Champ;
use droit::fiche::{CaseDeFiche, CaseSalarie, FicheSaisie};
use import::proposee::FicheProposee;
use socle::echec::R;

use crate::fiches::{case_vide, SorteDeFiche};

/// LA FORME DECLAREE D'UNE FORME LISIBLE (`nom_affichage` : `nom`) : le champ de la table qui s'en deduit (`derive_de`) ;
/// rien pour une case qui n'est pas une forme lisible.
pub(crate) fn forme_declaree_de(champs: &[Champ], cle: CaseDeFiche) -> Option<CaseDeFiche> {
    champs.iter().find(|c| c.derive_de == Some(cle)).map(|c| c.cle)
}

/// Une case de `f` qui ne dit rien (`fiches::case_vide`) : absente, un texte blanc ou « A COMPLETER », une liste ou une
/// structure vides.
pub(crate) fn vide<P: FicheSaisie>(f: &P, k: CaseDeFiche) -> bool {
    case_vide(f.valeur_ecrite(k).as_ref())
}

/// Les cases de `proposees` (la fiche que l'import creerait de cette piece) qu'une fiche existante recoit : une case de
/// la fiche, vide dans `fiche`, pleine dans `proposees`. Un champ officiel dont la forme lisible est remplie se derive
/// d'elle a l'ecriture : il n'est pas complete. Ce que l'import ne met jamais dans une fiche qui existe : son etat
/// (`actif` : une fiche archivee l'est exprès). La forme lisible (`*_affichage`) d'un champ complete suit, quand elle
/// est vide : celle que la piece donne, sinon celle que l'ecran en ferait (`fiches::comblee`).
pub(crate) fn champs_a_completer<F: SorteDeFiche>(fiche: &F::Plate, proposees: &F::Plate) -> R<F::Plate> {
    let champs = declarations::champs::fiche(F::NOM)?;
    let actif: CaseDeFiche = CaseSalarie::Actif.into();
    let mut out = F::Plate::default();
    for c in &champs {
        if forme_declaree_de(&champs, c.cle).is_some() || c.cle == actif {
            continue;
        }
        if vide(proposees, c.cle) || !vide(fiche, c.cle) {
            continue;
        }
        if c.derive_de.is_some_and(|src| !vide(fiche, src)) {
            continue;
        }
        out.prendre(proposees, c.cle);
    }
    if out.est_vide() {
        return Ok(out);
    }
    // La fiche, case par case, et ce qui la complete : ce que l'ecran en ferait (`comblee`).
    let mut fusion = fiche.clone();
    fusion.fondre(&out);
    let comble = crate::fiches::comblee::<F>(&fusion)?;
    for c in &champs {
        let Some(src) = c.derive_de else { continue };
        if vide(&out, c.cle) || !vide(fiche, src) {
            continue;
        }
        if !vide(proposees, src) {
            out.prendre(proposees, src);
        } else if !vide(&comble, src) {
            out.prendre(&comble, src);
        }
    }
    Ok(out)
}

/// Une fiche existante que l'import completerait : laquelle, et ses cases.
pub(crate) struct ACompleter {
    pub(crate) fiche: droit::fiche::FicheNommee,
    pub(crate) champs: FicheProposee,
}

/// Ajoute des cases a completer. LA PREMIERE PIECE QUI DONNE UNE CASE LA GARDE : les DSN passent avant les bulletins,
/// comme pour une fiche que l'import cree (le bulletin complete la fiche que la DSN cree, il ne la reecrit pas).
pub(crate) fn ajouter(liste: &mut Vec<ACompleter>, fiche: droit::fiche::FicheNommee, champs: &FicheProposee) -> R<()> {
    if !champs.est_dite() {
        return Ok(());
    }
    match liste.iter_mut().find(|a| a.fiche == fiche) {
        Some(a) => a.champs.completer_de(champs)?,
        None => liste.push(ACompleter { fiche, champs: champs.clone() }),
    }
    Ok(())
}
