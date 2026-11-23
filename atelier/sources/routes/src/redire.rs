//! UNE VALEUR D'UNE FICHE QUI NE SE LIT PLUS, ET LA COMPAGNIE QUI LA SAISIT DE NOUVEAU. Une valeur datee du fichier d'une
//! fiche que le type de sa case ne lit plus (`droit::fiche::NonLu::Valeur`) ne ferme pas la compagnie et ne se devine pas :
//! le logiciel la montre et demande de la saisir de nouveau, comme un cuisinier tend le bocal a l'etiquette illisible.
//!
//! - LA DEMANDE est la route `valeurs-illisibles` : chaque valeur, sa fiche, le libelle de sa case, ce qui est ecrit et
//!   ne se lit plus, depuis quand, et la case elle-meme, dessinee comme dans sa fiche (`fiches::case_a_redire`) ; la
//!   reponse est `enregistrer-fiche`, avec la charge que la demande donne : la case saisie, ou vide expres (`null`). Redite,
//!   la valeur illisible est remplacee a son `depuis`, et elle seule (`droit::fiche::premiere_illisible`,
//!   `fiches::ecriture`).
//! - LE BARRAGE (`barrage`), le seul : tant qu'une valeur attend, une route que sa fiche ne dit pas servie
//!   (`catalogue::PendantLaDemande`) refuse en renvoyant a la demande (`Classe::ValeurARedire`) : rien ne se calcule, ne
//!   s'ecrit de soi-meme ni ne part. Il se tient a l'entree des routes, la ou une route ouvre la compagnie
//!   (`service::contexte_de_la_route`).
//! - Une cle inconnue ne demande rien et ne bloque rien : elle se garde, et seul l'editeur l'apprend (`non_lus`).

use droit::contexte::Contexte;
use droit::fiche::{FicheNommee, NomDeFiche, NonLu};
use socle::date::Jour;
use socle::echec::{erreur, Classe, R};

use crate::catalogue::{Acte, Ecran, Fiche, PendantLaDemande, Route, OPERATION};
use crate::enveloppe::{en_json, Charge, Dite, Phrase, Porte, Rendu};
use crate::fiches::CaseDeLaFiche;

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[Fiche {
    route: Route::ValeursIllisibles,
    geste: "Dire les valeurs des fiches qui ne se lisent plus, à saisir de nouveau.",
    charge: &crate::SansCharge::CHARGE,
    exemple: r#"{}"#,
    rend: "valeurs (chacune : la fiche, le libellé de la case, ce qui est écrit, depuis quand, la case à saisir telle que la fiche la dessine, et la charge d'enregistrer-fiche qui la redit, la case saisie ou null pour la dire vide) ; titre, vide, enregistrer, confirmer (les boutons) ; phrase",
    refus: &[OPERATION],
    acte: Acte::Lit,
    phrase: 0,
    ecran: Ecran::Lecture,
    pendant_la_demande: PendantLaDemande::Servie,
}];

/// `valeurs-illisibles`.
pub fn route(porte: &Porte, charge: &Charge) -> R<Rendu> {
    match porte.route {
        Route::ValeursIllisibles => {
            let crate::SansCharge {} = charge.lire()?;
            en_json(demande(porte.ctx)?)
        }
        autre => Err(crate::enveloppe::route_inconnue(autre.nom())),
    }
}

/// UNE VALEUR QUI ATTEND D'ETRE SAISIE DE NOUVEAU : sa fiche (`fiche`, `cle`) et sa case, ce qui est ecrit, son `depuis`.
struct AttenteDUneValeur {
    fiche: FicheNommee,
    case: droit::fiche::CaseDeFiche,
    depuis: Option<Jour>,
    valeur: socle::forme::ValeurIllisible,
}

/// Les valeurs des fiches de la compagnie qui ne se lisent plus, dans l'ordre des fichiers (`Contexte::non_lus`).
fn en_attente_toutes(ctx: &Contexte) -> Vec<AttenteDUneValeur> {
    ctx.non_lus()
        .into_iter()
        .filter_map(|n| match n {
            NonLu::Valeur { fiche, case, depuis, valeur, .. } => Some(AttenteDUneValeur { fiche, case, depuis, valeur }),
            NonLu::Cle { .. } => None,
        })
        .collect()
}

/// CE QUI EST ECRIT ET NE SE LIT PLUS, tel que la compagnie le lit, court : un texte, un nombre, un oui-non tels
/// qu'ecrits ; une liste par son nombre (« une liste de 3 établissements ») ; la ligne du taux AT/MP par ce qu'elle est ;
/// d'autres cases ecrites ensemble par leurs valeurs (`ValeurIllisible::ecrite`), ou leur nombre si elles sont longues.
fn ecrite_dite(case: droit::fiche::CaseDeFiche, v: &socle::forme::ValeurIllisible) -> String {
    use droit::fiche::CaseEntreprise;
    const LONGUE: usize = 60;
    let (un, plusieurs) = if case == CaseEntreprise::Etablissements.into() { ("établissement", "établissements") } else { ("valeur", "valeurs") };
    if let Some(n) = v.elements() {
        return match n {
            0 => "une liste vide".to_string(),
            1 => format!("une liste d'un{} {}", if un == "valeur" { "e" } else { "" }, un),
            n => format!("une liste de {} {}", n, plusieurs),
        };
    }
    if v.est_un_objet() && case == CaseEntreprise::TauxAtmp.into() {
        return "une ligne du taux AT/MP".to_string();
    }
    let ecrite = v.ecrite();
    if v.est_un_objet() && ecrite.chars().count() > LONGUE {
        return format!("{} valeurs écrites ensemble", ecrite.split(" · ").count());
    }
    ecrite
}

/// LA FICHE EN MOTS : « l'entreprise », le nom du salarie (sa cle, si sa fiche ne le nomme pas), le titre du spectacle.
fn fiche_dite(ctx: &Contexte, f: &FicheNommee) -> String {
    match f {
        FicheNommee::Entreprise => "l'entreprise".to_string(),
        FicheNommee::Salarie(k) => ctx.nomme(k),
        FicheNommee::Spectacle(k) => match ctx.spectacles.get(k).and_then(|s| s.titre.clone()) {
            Some(t) => format!("« {} »", t),
            None => k.to_string(),
        },
    }
}

/// « la fiche de l'entreprise », « la fiche de Solène Marchetot », « la fiche du spectacle « Titre » ».
fn la_fiche(ctx: &Contexte, f: &FicheNommee) -> String {
    match f {
        FicheNommee::Spectacle(_) => format!("la fiche du spectacle {}", fiche_dite(ctx, f)),
        FicheNommee::Entreprise | FicheNommee::Salarie(_) => format!("la fiche de {}", fiche_dite(ctx, f)),
    }
}

/// Ce que dit une case que la table ne decrit pas : elle n'a pas de libelle, et sa cle est un nom interne.
const SANS_LIBELLE: &str = "une case que la fiche ne montre pas";

/// Le libelle de la case, tel que la fiche le montre ; `SANS_LIBELLE`, si la table ne la decrit pas.
fn libelle(nom: NomDeFiche, case: droit::fiche::CaseDeFiche) -> R<String> {
    Ok(match declarations::champs::champ(nom, case)?.map(|c| c.libelle).filter(|l| !l.is_empty()) {
        Some(l) => l,
        None => SANS_LIBELLE.to_string(),
    })
}

/// La case dans une phrase : son libelle entre guillemets (« Code APE »), ou `SANS_LIBELLE`.
pub(crate) fn libelle_dit(nom: NomDeFiche, case: droit::fiche::CaseDeFiche) -> R<String> {
    Ok(match declarations::champs::champ(nom, case)?.map(|c| c.libelle).filter(|l| !l.is_empty()) {
        Some(l) => format!("« {} »", l),
        None => SANS_LIBELLE.to_string(),
    })
}

/// La phrase qui ouvre une phrase : sa premiere lettre en capitale.
fn capitale(t: &str) -> String {
    let mut c = t.chars();
    c.next().into_iter().flat_map(char::to_uppercase).chain(c).collect()
}

/// LA PHRASE DE LA DEMANDE, quand une valeur au moins attend : la premiere nommee (sa fiche, sa case), et combien d'autres.
fn phrase_d_attente(ctx: &Contexte, a: &[AttenteDUneValeur], suite: &str) -> R<Option<String>> {
    let Some(p) = a.first() else { return Ok(None) };
    let autres = match a.len() - 1 {
        0 => String::new(),
        1 => ", et une autre".to_string(),
        n => format!(", et {} autres", n),
    };
    Ok(Some(format!("Une valeur de {} ne se lit plus ({}){} : {}", la_fiche(ctx, &p.fiche), libelle(p.fiche.nom(), p.case)?, autres, suite)))
}

/// LA COMPAGNIE ATTEND-ELLE UNE VALEUR A SAISIR DE NOUVEAU ? La phrase que l'ouverture en dit (elle rend la main sans
/// rien relever, inscrire ni sauvegarder), ou aucune.
pub(crate) fn en_attente(ctx: &Contexte) -> R<Option<String>> {
    phrase_d_attente(ctx, &en_attente_toutes(ctx), "la saisir de nouveau ; rien n'est relevé ni sauvegardé d'ici là.")
}

/// LE BARRAGE, le seul : la route `route` sur la compagnie `ctx`. Sa fiche la dit servie pendant la demande
/// (`PendantLaDemande::Servie`), ou aucune valeur n'attend : elle passe. Sinon elle refuse, en renvoyant a la demande ; une
/// route sans fiche (celles de l'atelier) refuse comme une route que sa fiche ne dit pas servie.
pub(crate) fn barrage(ctx: &Contexte, route: Route) -> R<()> {
    if crate::catalogue::catalogue().iter().any(|f| f.route == route && f.pendant_la_demande == PendantLaDemande::Servie) {
        return Ok(());
    }
    match refus_de_la_demande(ctx)? {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// LE REFUS QUI RENVOIE A LA DEMANDE, le meme partout (le barrage ; pendant la demande, une saisie d'`enregistrer-fiche`
/// qui n'y repond pas : `fiches::ecriture`) ; aucun si rien n'attend.
pub(crate) fn refus_de_la_demande(ctx: &Contexte) -> R<Option<socle::echec::Echec>> {
    Ok(phrase_d_attente(ctx, &en_attente_toutes(ctx), "la saisir de nouveau, puis recommencer.")?.map(|phrase| erreur(Classe::ValeurARedire, phrase)))
}

/// LA CHARGE D'`enregistrer-fiche` QUI REDIT UNE VALEUR, sans ses `changements` (la case saisie, ou `null`) : la fiche et sa
/// cle.
#[derive(Debug, serde::Serialize)]
struct ChargeDeLaReponse {
    nom: NomDeFiche,
    #[serde(skip_serializing_if = "Option::is_none")]
    cle: Option<String>,
}

/// UNE VALEUR A SAISIR DE NOUVEAU, telle que la demande la rend : sa phrase, sa fiche et le libelle de sa case en mots, ce
/// qui est ecrit, depuis quand (aucun : de tout temps), la case a saisir (aucune : la table ne la decrit pas, elle ne peut
/// que se dire vide), et la charge de la reponse ; `cle` : la case, telle que les `changements` de la reponse la nomment.
#[derive(Debug, serde::Serialize)]
struct ValeurDemandee {
    phrase: String,
    fiche: String,
    cle: droit::fiche::CaseDeFiche,
    libelle: String,
    ecrite: String,
    depuis: Option<Jour>,
    case: Option<CaseDeLaFiche>,
    enregistrer: ChargeDeLaReponse,
}

/// CE QUE REND `valeurs-illisibles` : la phrase, chaque valeur (dans l'ordre des fichiers ; la page les demande une par
/// une), et ce que disent les boutons de la boite.
#[derive(Debug, serde::Serialize)]
struct ValeursIllisibles {
    phrase: Phrase,
    phrases: Vec<String>,
    titre: &'static str,
    vide: &'static str,
    enregistrer: &'static str,
    confirmer: &'static str,
    valeurs: Vec<ValeurDemandee>,
}

impl Dite for ValeursIllisibles {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

/// La demande : chaque valeur qui ne se lit plus.
fn demande(ctx: &Contexte) -> R<ValeursIllisibles> {
    let mut valeurs = Vec::new();
    for a in en_attente_toutes(ctx) {
        let nom = a.fiche.nom();
        let ecrite = ecrite_dite(a.case, &a.valeur);
        let depuis_dit = match a.depuis {
            Some(j) => format!(" (à partir du {})", j.jj_mm_aaaa()),
            None => String::new(),
        };
        let phrase = format!(
            "{}, {}{} : « {} » ne se lit plus. La saisir de nouveau, ou dire qu'elle est vide.",
            capitale(&la_fiche(ctx, &a.fiche)),
            libelle_dit(nom, a.case)?,
            depuis_dit,
            ecrite
        );
        let cle = match &a.fiche {
            FicheNommee::Entreprise => None,
            FicheNommee::Salarie(k) => Some(k.to_string()),
            FicheNommee::Spectacle(k) => Some(k.to_string()),
        };
        valeurs.push(ValeurDemandee {
            phrase,
            fiche: fiche_dite(ctx, &a.fiche),
            cle: a.case,
            libelle: libelle(nom, a.case)?,
            ecrite,
            depuis: a.depuis,
            case: crate::fiches::case_a_redire(ctx, nom, a.case)?,
            enregistrer: ChargeDeLaReponse { nom, cle },
        });
    }
    let phrase = match valeurs.len() {
        0 => Phrase::fixe("Toutes les valeurs des fiches se lisent."),
        1 => Phrase::fixe("Une valeur ne se lit plus : la saisir de nouveau."),
        n => socle::phrase!("{} valeurs ne se lisent plus : les saisir de nouveau, une par une.", n),
    };
    Ok(ValeursIllisibles {
        phrase,
        phrases: valeurs.iter().map(|v| v.phrase.clone()).collect(),
        titre: "Une valeur ne se lit plus",
        vide: "Elle est vide",
        enregistrer: "Enregistrer",
        confirmer: "Confirmer",
        valeurs,
    })
}
