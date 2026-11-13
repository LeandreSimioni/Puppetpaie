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
//! confrontent case par case, chacune une saisie de sa fiche (`declarations::champs::Saisie`).

use declarations::champs::{Champ, Saisie, Valeur};
use droit::fiche::{CaseDeFiche, CaseSalarie, NomDeCase, NomDeFiche};
use socle::echec::R;

/// Une case qui ne dit rien : absente, un texte blanc ou « A COMPLETER » (sans casse), une liste ou une structure vides.
/// Un booleen ou un nombre disent quelque chose.
pub(crate) fn vide(v: Option<&Valeur>) -> bool {
    crate::fiches::vide(v)
}

/// LA FORME DECLAREE D'UNE FORME LISIBLE (`nom_affichage` : `nom`) : le champ de la table qui s'en deduit (`derive_de`) ;
/// rien pour une case qui n'est pas une forme lisible.
pub(crate) fn forme_declaree_de(champs: &[Champ], cle: CaseDeFiche) -> Option<CaseDeFiche> {
    champs.iter().find(|c| c.derive_de == Some(cle)).map(|c| c.cle)
}

/// Ce que l'import ne met jamais dans une fiche qui existe : son etat (`actif` : une fiche archivee l'est exprès), et son
/// historique.
fn jamais() -> [CaseDeFiche; 2] {
    [CaseSalarie::Actif.into(), NomDeCase::HISTORIQUE.into()]
}

/// Les cases de `proposees` (la fiche que l'import creerait de cette piece) qu'une fiche existante recoit : une case de
/// la fiche, vide dans `fiche`, pleine dans `proposees`. Un champ officiel dont la forme lisible est remplie se derive
/// d'elle a l'ecriture : il n'est pas complete. La forme lisible (`*_affichage`) d'un champ complete suit, quand elle
/// est vide : celle que la piece donne, sinon celle que l'ecran en ferait (`Saisie::comblee`).
pub(crate) fn champs_a_completer(nom: NomDeFiche, fiche: &Saisie, proposees: &Saisie) -> R<Saisie> {
    let champs = declarations::champs::fiche(nom)?;
    let mut out = Saisie::default();
    for c in &champs {
        if forme_declaree_de(&champs, c.cle).is_some() || jamais().contains(&c.cle) {
            continue;
        }
        let Some(v) = proposees.valeur(c.cle) else { continue };
        if vide(Some(v)) || !vide(fiche.valeur(c.cle)) {
            continue;
        }
        if c.derive_de.is_some_and(|src| !vide(fiche.valeur(src))) {
            continue;
        }
        out.poser(c.cle, v.clone());
    }
    if !out.est_dite() {
        return Ok(out);
    }
    // La fiche, case par case, et ce qui la complete : ce que l'ecran en ferait (`comblee`).
    let mut fusion = fiche.clone();
    import::proposee::fondre(&mut fusion, &out);
    let comble = fusion.comblee(nom)?;
    for c in &champs {
        let Some(src) = c.derive_de else { continue };
        if out.valeur(c.cle).is_none() || !vide(fiche.valeur(src)) {
            continue;
        }
        let lisible = proposees.valeur(src).filter(|v| !vide(Some(v))).or_else(|| comble.valeur(src).filter(|v| !vide(Some(v))));
        if let Some(v) = lisible {
            out.poser(src, v.clone());
        }
    }
    Ok(out)
}

/// Une fiche existante que l'import completerait : laquelle, et ses cases.
pub(crate) struct ACompleter {
    pub(crate) fiche: droit::fiche::FicheNommee,
    pub(crate) champs: Saisie,
}

/// Ajoute des cases a completer. LA PREMIERE PIECE QUI DONNE UNE CASE LA GARDE : les DSN passent avant les bulletins,
/// comme pour une fiche que l'import cree (le bulletin complete la fiche que la DSN cree, il ne la reecrit pas).
pub(crate) fn ajouter(liste: &mut Vec<ACompleter>, fiche: droit::fiche::FicheNommee, champs: &Saisie) {
    if !champs.est_dite() {
        return;
    }
    match liste.iter_mut().find(|a| a.fiche == fiche) {
        Some(a) => import::proposee::completer_de(&mut a.champs, champs),
        None => liste.push(ACompleter { fiche, champs: champs.clone() }),
    }
}
