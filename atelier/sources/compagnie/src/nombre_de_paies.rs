//! LE NOMBRE DE PAIES DE L'ANNEE, ENVOYE A L'EDITEUR LE 16 JANVIER : le logiciel est gratuit, sans abonnement, et c'est
//! le seul moyen pour l'editeur de savoir combien il sert vraiment.
//!
//! CE QUI PART : une fois par an et par compagnie, `{"annee": N-1, "paies": <nombre>}`, et le numero de poste que portent
//! deja les incidents. Rien d'autre : ni nom, ni SIREN, ni NIR, ni montant. Zero part aussi (zero est une information).
//!
//! LE COMPTE : les paies VALIDEES dont le mois de paie est dans l'annee N-1, intermittents et permanents confondus : une
//! par mois que chaque engagement valide couvre (`engagements/`, `valide.paies`). Un mois tenu avant ce logiciel n'a
//! pas d'engagement : PuppetPaie n'a pas fait ces paies, elles ne comptent pas. Des engagements qui ne se
//! lisent pas font REFUSER le compte (regle 1 : rien ne se devine) : rien ne part, et le passage suivant recommence.
//!
//! QUAND : a partir du 16 janvier (l'horloge du moteur, `PUPPETPAIE_AUJOURDHUI` dans les essais), a l'ouverture ou au
//! passage de la veille (`ouverture::ouverture`, avec les incidents), le premier ce jour-la ou apres si le poste etait
//! eteint. Seule l'annee N-1 est due : une annee plus ancienne qui n'est jamais partie ne part plus.
//!
//! PAR OU : le canal des incidents (`incidents::canal`, l'identite du poste de `config/diagnostic.json`, la cle dans
//! l'en-tete), action `paies` du serveur de l'editeur ; SOUS LE MEME INTERRUPTEUR (`incidents::envoi`) : coupe, rien ne
//! part.
//!
//! CE QUI EST RETENU : `config/diagnostic.json`, cle `paies_annuelles`, une entree par annee partie (`paies`, `poste`,
//! `parti_le`) ; une annee qui y est ne repart pas. Ce fichier est propre au poste (`compagnies::PROPRES_AU_POSTE`) : une
//! remise a zero de la compagnie ne fait pas renvoyer l'annee. Un envoi qui echoue (hors ligne, serveur en panne) n'ecrit
//! rien : le passage suivant le refait. L'ecran « Ce qui est parti chez l'editeur » le montre avec les incidents
//! (`routes::filet::diagnostic_etat`).
//!
//! JAMAIS : depuis une compagnie fictive ni depuis la copie de developpement (le marqueur `COPIE-DE-DEVELOPPEMENT.json`,
//! `compagnies::est_copie`, que portent l'une et l'autre) ; depuis un logiciel bride (`bridage::est_bride`), ce qui couvre une
//! compagnie ajoutee sur un logiciel bride tant qu'il l'est. GARDEE AU DEBLOCAGE, elle est une compagnie reelle (son
//! marqueur est parti, `accueil::suppression::garder`) : son nombre part, et ses paies validees en demo comptent -- le
//! logiciel les a faites ; qu'elles ne soient pas parties ne change pas la regle du compte.

use std::collections::BTreeMap;
use std::path::Path;

use droit::contexte::Contexte;
use socle::date::Jour;
use socle::echec::R;
use socle::mois::Annee;

use crate::incidents::{self, AnneeRetenue, Canal, NumeroDePoste};

/// Le jour de janvier a partir duquel l'annee passee part.
pub const JOUR_DE_JANVIER: u32 = 16;

/// L'annee dont le nombre est du `aujourdhui` : l'annee passee, a partir du 16 janvier ; aucune avant. L'annee 1 n'a
/// pas d'annee passee : refus.
pub fn annee_due(aujourdhui: Jour) -> R<Option<Annee>> {
    if aujourdhui.mois > 1 || aujourdhui.jour >= JOUR_DE_JANVIER {
        Ok(Some(Annee::nouvelle(i64::from(aujourdhui.an) - 1)?))
    } else {
        Ok(None)
    }
}

/// Le nombre de paies validees dont le mois de paie est dans `annee` (voir l'en-tete) : celles que portent les fichiers
/// des paies de ses mois (`engagement::paies::nombre`).
pub fn compter(ctx: &Contexte, annee: Annee) -> R<usize> {
    let mut n = 0;
    for m in 1..=12 {
        n += engagement::paies::nombre(&ctx.racine, socle::mois::Mois::nouveau(annee.nombre(), m)?)?;
    }
    Ok(n)
}

/// Les annees deja parties, telles que `config/diagnostic.json` les retient (`paies_annuelles`) ; aucune tant qu'aucune
/// n'est partie.
fn parties(racine: &Path) -> R<Option<BTreeMap<Annee, AnneeRetenue>>> {
    Ok(incidents::lire_config(racine)?.paies_annuelles)
}

/// Retient que `annee` est partie, sans toucher au reste du fichier (l'identite du poste, l'interrupteur).
fn retenir(racine: &Path, annee: Annee, paies: usize, poste: &NumeroDePoste) -> R<()> {
    let mut c = incidents::lire_config(racine)?;
    let retenue = AnneeRetenue { paies: paies as u64, poste: poste.clone(), parti_le: socle::temps::secondes().a_la_seconde() };
    c.paies_annuelles.get_or_insert_with(BTreeMap::new).insert(annee, retenue);
    incidents::ecrire_config(racine, &c)
}

/// Pourquoi rien ne part de cette compagnie, ou `None` si elle peut envoyer.
fn empechement(racine: &Path) -> R<Option<&'static str>> {
    Ok(if crate::installation::est_fictive(racine)? {
        Some("compagnie fictive")
    } else if crate::installation::est_copie(racine) {
        Some("copie de developpement")
    } else if crate::bridage::est_bride() {
        Some("logiciel bride")
    } else if !incidents::envoi(racine)? {
        Some("envoi coupe")
    } else {
        None
    })
}

/// A l'ouverture et a chaque passage de la veille : envoie le nombre de paies de l'annee passee s'il est du et pas encore
/// parti. Ne leve jamais (l'ouverture n'echoue pas) : rend ce qui s'est passe, `{"annee", "envoye", "paies", "raison",
/// "erreur"}` -- `erreur` non vide quand le passage suivant recommencera.
pub fn envoyer_si_du(ctx: &Contexte) -> BilanDesPaies {
    let racine = ctx.racine.as_path();
    let bilan = |annee: Option<Annee>, envoye: bool, paies: Option<usize>, raison: &str, erreur: &str| BilanDesPaies {
        annee,
        envoye,
        paies,
        raison: (!raison.is_empty()).then(|| raison.to_string()),
        erreur: (!erreur.is_empty()).then(|| erreur.to_string()),
    };
    match empechement(racine) {
        Ok(Some(r)) => return bilan(None, false, None, r, ""),
        Ok(None) => {}
        Err(e) => return bilan(None, false, None, "", &e.phrase),
    }
    let annee = match annee_due(ctx.aujourdhui) {
        Ok(Some(a)) => a,
        Ok(None) => return bilan(None, false, None, "pas avant le 16 janvier", ""),
        Err(e) => return bilan(None, false, None, "", &e.phrase),
    };
    match parties(racine) {
        Ok(Some(p)) if p.contains_key(&annee) => return bilan(Some(annee), false, None, "deja parti", ""),
        Ok(_) => {}
        Err(e) => return bilan(Some(annee), false, None, "", &e.phrase),
    }
    let client = match incidents::canal(racine) {
        Ok(Canal::Pret(c)) => c,
        Ok(Canal::Panne(e)) => return bilan(Some(annee), false, None, "", &e.dite()),
        Err(e) => return bilan(Some(annee), false, None, "", &e.phrase),
    };
    let paies = match compter(ctx, annee) {
        Ok(n) => n,
        Err(e) => return bilan(Some(annee), false, None, "", &format!("compte refuse : {}", e.phrase)),
    };
    if let Err(e) = client.compter_paies(annee, paies) {
        return bilan(Some(annee), false, Some(paies), "", &e.dite());
    }
    match retenir(racine, annee, paies, &client.poste.0) {
        Ok(()) => bilan(Some(annee), true, Some(paies), "", ""),
        // Parti, mais pas retenu : le passage suivant le renverra, et le serveur remplace la meme annee.
        Err(e) => bilan(Some(annee), true, Some(paies), "", &e.phrase),
    }
}

/// Ce que rend un passage de l'envoi du nombre de paies (`envoyer_si_du`) : l'annee due, si elle est partie, combien,
/// pourquoi rien n'est parti, et l'erreur quand le passage suivant recommencera.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BilanDesPaies {
    #[serde(with = "socle::mois::en_nombre::option")]
    pub annee: Option<Annee>,
    pub envoye: bool,
    pub paies: Option<usize>,
    pub raison: Option<String>,
    pub erreur: Option<String>,
}

/// Ce que l'ecran « Ce qui est parti chez l'editeur » montre de chaque annee partie, la plus recente en tete : exactement
/// ce qui a quitte le poste (`contenu`), et quand.
pub fn annees_parties(racine: &Path) -> R<Vec<AnneePartie>> {
    let mut v: Vec<(Annee, AnneeRetenue)> = parties(racine)?.into_iter().flatten().collect();
    v.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(v.into_iter().map(|(an, e)| AnneePartie { annee: an, paies: e.paies, parti_le: Some(e.parti_le), contenu: ContenuParti { annee: an, paies: e.paies, poste: e.poste } }).collect())
}

/// Une annee partie, telle que l'ecran du filet la montre : son nombre de paies, quand il est parti, et ce qui est parti.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AnneePartie {
    #[serde(with = "socle::mois::en_nombre")]
    pub annee: Annee,
    pub paies: u64,
    #[serde(with = "socle::paris::ecrit::seconde::option")]
    pub parti_le: Option<socle::paris::Moment>,
    pub contenu: ContenuParti,
}

/// Exactement ce qui a quitte le poste pour une annee.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ContenuParti {
    #[serde(with = "socle::mois::en_nombre")]
    pub annee: Annee,
    pub paies: u64,
    pub poste: NumeroDePoste,
}
