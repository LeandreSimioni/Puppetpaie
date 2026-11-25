//! Les routes de l'engagement, celles de la carte : la valider, la modifier, la retirer. Chacune rend la phrase que
//! l'on lit ; un refus dit quoi corriger et n'ecrit rien.

use serde::{Deserialize, Serialize};

use droit::contexte::Contexte;
use engagement::carte::{Brouillon, Changements};
use engagement::saisie::RangDansLeMois;
use engagement::stock::Identite;
use socle::echec::{erreur, Classe, Echec, R};
use socle::mois::Mois;

use crate::catalogue::{Acte, Forme, Fiche, Refus as RefusDit, CASE_EXIGEE, FORME};
use crate::declaration::{charge, dit, legendes, ChampDuType, ChargeDuType, Legende, ParSaForme, ValeurDeLaCharge};
use crate::catalogue::Route;
use crate::catalogue::{Ecran, Place};
use crate::etapes::{action, Action, ChargeMois};
use crate::enveloppe::{en_json, Charge, Dite, Phrase, Porte, Rendu};

/// Dans une charge, l'identite d'une carte se dit d'un objet (`{salarie, debut, rang}`).
impl ValeurDeLaCharge<ParSaForme> for Identite {
    const FORME: Forme = Forme::Objet;
}

pub(crate) const IDENTITE: Legende = dit("Carte", "l'identité de la carte, telle que le moteur l'a rendue : {salarie, debut, rang}");
/// Les changements des cases du brouillon d'une carte, dans l'ordre.
pub(crate) const CHANGEMENTS: Legende = dit(
    "Cases qui changent",
    "[{case, valeur}, …], au moins un, appliqués dans l'ordre, tout ou rien (un refus nomme sa case et rien n'est écrit) : chacun une case de la nature de la carte (`natures` de cette fiche : chacune obligatoire ou facultative à la validation, et les motifs d'un CDD ; une case qu'elle n'a pas se refuse), valeur null pour la vider ; {\"case\": \"nature\", \"valeur\": \"cdi\"} change sa nature (les cases des deux natures gardent ce qu'elles disent, les autres naissent vides). Les cases : salarie (la clé de sa fiche), spectacle, debut, fin, motif_recours (un CDD), lignes [{rubrique, et exactement les clés que sa rubrique demande, `rubriques[].cles[].cle` de la route etat, null pour une case vide : quantite (dans l'unité de sa rubrique, {\"cachets\": 2} : la clé `rubriques[].quantite` de la route etat), taux, heures (par unité : 4 pour des services de 4 h), lieu, dates (tous ses jours : le moteur range ceux des mois suivants au planning ; un jour du mois s'écrit seul, ou {jour, quantite, lieu}, sa quantité dans l'unité de la ligne et son lieu, chacun facultatif), planning (facultatifs : absents, ils sont vides) ; base et taux_majoration (un pourcentage) ; montant, genre ; territoire et cases (un repas, une nuitée ; facultatifs ; une autre ligne ne les a pas) ; une ligne qui se compte jour par jour : quand aucun jour ne dit sa quantité, celle de la ligne se répartit sur ses jours du mois, le reste sur les premiers (10 heures sur 3 jours : 4, 3, 3) ; quand des jours disent la leur et que sa quantité est absente ou leur somme, sa quantité est cette somme, un jour qui n'en dit pas restant vide (5, 3, 2 avec 10 restent 5, 3, 2) ; quand sa quantité en diffère, elle a été tapée et se répartit sur tous ses jours (5, 3, 2 avec 12 : 4, 4, 4) ; une ligne lue et renvoyée telle quelle ne change donc pas}] ; les cases que la carte ouvre, et leur place (dont celles de la fin, celles que son contrat écrit appelle, et numero, le numéro déjà déclaré en DSN d'un contrat commencé avant PuppetPaie) : `carte.cases` de sa paie dans la route apercu (leur sens : `aides` de la route etat) ; le préavis : {type, debut, fin}, ses parties `parties.preavis` de la route etat, son type parmi `choix.preavis`",
);
pub(crate) const SAISIE_INCOMPLETE: RefusDit = RefusDit { classe: Classe::SaisieIncomplete, corriger: "le fait que la phrase nomme, sur la carte ou la fiche du salarié" };
const OPERATION: RefusDit = RefusDit {
    classe: Classe::Refuse,
    corriger: "ce que la phrase dit : un mois importé ou clos, une carte que l'écran ne montre pas, un contrat au mois déjà engagé (sa carte le modifie), une saisie hors de ses bornes",
};

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Paie);

/// Les routes de ce module, et leurs fiches. Toutes ecrivent.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::ValiderEngagement,
        geste: "Valider l'engagement d'une carte, tel qu'il est écrit : calculer ses paies.",
        charge: &ChargeValider::CHARGE,
        exemple: r#"{"mois":"2026-11","engagement":{"salarie":"FULLER","debut":"2026-11-27","rang":0}}"#,
        rend: "engagement (l'identité de la carte), phrase ; un engagement déjà validé le reste, rien n'est écrit",
        refus: &[SAISIE_INCOMPLETE, OPERATION, CASE_EXIGEE, FORME],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Ecrit,
        phrase: 320,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::ModifierEngagement,
        geste: "Modifier un engagement validé : changer des cases de sa carte, et le revalider.",
        charge: &CHARGE_MODIFIER,
        exemple: r#"{"mois":"2026-11","engagement":{"salarie":"FULLER","debut":"2026-11-27","rang":0},"changements":[{"case":"lignes","valeur":[{"rubrique":"cachet","quantite":{"cachets":1},"taux":160,"lieu":null,"dates":["2026-11-27"],"planning":[]}]}],"confirme":false,"jour":null}"#,
        rend: "rien n'a changé : change faux ; ce qui est parti le demande : avertissement (à montrer, puis renvoyer avec confirme : son titre, annuler, confirmer, et la case du jour, jour_libelle et jour_propose, quand un terme vaut d'un autre jour), rien n'est écrit ; écrit : change, phrase (un terme vaut à partir du mois de la carte)",
        refus: &[
            SAISIE_INCOMPLETE,
            OPERATION,
            CASE_EXIGEE,
            RefusDit { classe: Classe::ModificationNonDeclarable, corriger: "rien : la norme ne permet plus de déclarer ce changement ; la phrase dit pourquoi" },
            FORME,
        ],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Ecrit,
        phrase: 320,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::RetirerEngagement,
        geste: "Retirer l'engagement d'une carte, ou le rendre non actif si un envoi est parti.",
        charge: &ChargeRetirer::CHARGE,
        exemple: r#"{"mois":"2026-11","engagement":{"salarie":"FULLER","debut":"2026-11-27","rang":0},"confirme":true}"#,
        rend: "phrase : retiré, ou non actif et ce que cela rend à faire ; un envoi est parti : avertissement (sa phrase, son titre, garder, confirmer : à montrer, puis renvoyer avec confirme), rien n'est écrit",
        refus: &[OPERATION, FORME],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Ecrit,
        phrase: 200,
        ecran: ECRAN,
    },
];

/// Une route de ce module, branchee sur l'enveloppe.
pub fn route(porte: &Porte, charge: &Charge) -> R<Rendu> {
    let ctx = porte.ctx;
    match porte.route {
        Route::ValiderEngagement => en_json(valider_engagement(ctx, charge.lire()?)?),
        Route::ModifierEngagement => en_json(modifier_engagement(ctx, charge.lire_avec_le_jour("jour", |c: &ChargeModifier| c.mois)?)?),
        Route::RetirerEngagement => en_json(retirer_engagement(ctx, charge.lire()?)?),
        autre => Err(crate::enveloppe::route_inconnue(autre.nom())),
    }
}

impl Dite for Validation {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

impl Dite for Modification {
    fn phrase(&self) -> &Phrase {
        match self {
            Modification::Faite { phrase, .. } => phrase,
            Modification::Avertie { phrase, .. } => phrase,
        }
    }
}

impl Dite for Retrait {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

/// La charge de « Modifier l'engagement » : le mois de la carte, son identite, ce qu'on y change (`changements`, comme
/// `enregistrer-saisie`) ; l'avertissement lu (`confirme`). Le jour du mois d'ou vaut un terme change (`jour`, `null` :
/// le 1er) se lit a cote, dans le mois de la carte (`Charge::lire_avec_le_jour`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeModifier {
    mois: Mois,
    engagement: Identite,
    changements: Changements,
    confirme: bool,
}

// Sa charge, champ par champ : `jour` se lit hors de la structure (`Charge::lire_avec_le_jour`), ce que `charge!` ne dit pas.
const CHARGE_MODIFIER: ChargeDuType = ChargeDuType {
    champs: &[
        ChampDuType::du_type::<_, Mois>("mois", legendes::MOIS),
        ChampDuType::du_type::<_, Identite>("engagement", IDENTITE),
        ChampDuType::du_type::<_, Changements>("changements", CHANGEMENTS),
        ChampDuType::du_type::<_, bool>("confirme", dit("Avertissement lu", "vrai : l'avertissement rendu a été lu et accepté")),
        ChampDuType::du_type::<_, Option<u32>>("jour", dit("Jour d'effet", "le jour du mois (un nombre, de 1 à 31) où un terme changé prend effet, quand ce n'est pas le 1er").avancee()),
    ],
};

charge! {
    /// La charge de « Retirer cet engagement » : le mois de la carte, son identite, l'avertissement lu (`confirme`).
    #[derive(Debug, Clone, Serialize)]
    pub(crate) struct ChargeRetirer {
        mois: Mois = legendes::MOIS,
        engagement: Identite = IDENTITE,
        confirme: bool = dit("Avertissement lu", "faux d'abord ; vrai : l'avertissement rendu a été lu, et l'engagement devient non actif"),
    }
}

/// LES GESTES DE LA CARTE D'UN ENGAGEMENT, tels que la ligne de l'engagement les rend (`etat-par-engagement`) : en
/// brouillon, le valider ou le retirer ; valide, ouvert pour etre modifie, le modifier (ses changements s'ajoutent a la
/// charge rendue), relire le mois sans rien ecrire, ou le retirer. Un engagement valide n'a pas de geste qui le valide.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum GestesDeLaCarte {
    Brouillon { retirer: Action, valider: Action },
    Valide { retirer: Action, annuler: Action, modifier: Action },
}

impl GestesDeLaCarte {
    /// Les gestes de la carte de l'engagement `engagement` de l'ecran de `mois`, valide ou non.
    pub(crate) fn de(mois: Mois, engagement: &Identite, valide: bool) -> GestesDeLaCarte {
        let retirer = action(Route::RetirerEngagement, ChargeRetirer { mois, engagement: engagement.clone(), confirme: false }, None, "Retirer cet engagement".into());
        if !valide {
            let valider = action(Route::ValiderEngagement, ChargeValider { mois, engagement: engagement.clone() }, None, "Valider l'engagement".into()).principale();
            return GestesDeLaCarte::Brouillon { retirer, valider };
        }
        let charge = ChargeModifier { mois, engagement: engagement.clone(), changements: Changements::aucun(), confirme: false };
        GestesDeLaCarte::Valide {
            retirer,
            annuler: action(Route::Saisie, ChargeMois { mois }, None, "Annuler".into()),
            modifier: action(Route::ModifierEngagement, charge, None, "Modifier l'engagement".into()).principale(),
        }
    }
}

/// Ce que rend « Valider l'engagement » : l'identite de la carte ecrite, et la phrase.
#[derive(Debug, Serialize)]
pub(crate) struct Validation {
    pub(crate) engagement: Identite,
    pub(crate) phrase: Phrase,
}

/// Ce que rend « Modifier l'engagement » : fait (`change` dit si quelque chose s'est ecrit), ou l'avertissement a
/// confirmer, rien d'ecrit.
#[derive(Debug, Serialize)]
#[serde(untagged)]
enum Modification {
    Faite { change: bool, phrase: Phrase },
    Avertie { avertissement: Avertissement, phrase: Phrase, phrases: Vec<String> },
}

/// L'avertissement d'une modification : chaque engagement touche, ce que le changement entraine pour lui, et d'ou vaut
/// un terme change (`jour`, `null` quand il vaut du debut).
/// La boite qui le montre : son titre, ses deux boutons (`annuler`, `confirmer` : renvoyer avec `confirme`) ; quand un
/// terme vaut d'un autre jour que le 1er, la case du jour (`jour_libelle`) et ce qu'elle montre d'abord (`jour_propose`,
/// le jour d'ou il vaut, `jour_dit`).
#[derive(Debug, Serialize)]
struct Avertissement {
    engagements: Vec<Touche>,
    /// D'ou vaut ce qui change, en une phrase.
    #[serde(rename = "jour")]
    jour_dit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    jour_libelle: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    jour_propose: Option<String>,
    titre: &'static str,
    annuler: &'static str,
    confirmer: &'static str,
}

impl Avertissement {
    /// L'avertissement de `engagements`, dont un terme change vaut du jour `depuis`, quand il en a un.
    fn de(engagements: Vec<Touche>, depuis: Option<socle::date::Jour>) -> Avertissement {
        Avertissement {
            engagements,
            jour_dit: depuis.map(|j| vaut_a_partir_du(&j)),
            jour_libelle: depuis.map(|_| "le"),
            jour_propose: depuis.map(|j| j.quantieme_ecrit()),
            titre: "Modifier l'engagement",
            annuler: "Annuler",
            confirmer: "Modifier l'engagement",
        }
    }
}

/// L'AVERTISSEMENT D'UN RETRAIT (`retirer-engagement`, un envoi parti) : ce qui est parti (`phrase`), et la boite qui le
/// montre : son titre, garder l'engagement (`garder`), ou le rendre non actif (`confirmer` : renvoyer avec `confirme`).
#[derive(Debug, Serialize)]
struct AvertissementDuRetrait {
    phrase: String,
    titre: &'static str,
    garder: &'static str,
    confirmer: &'static str,
}

/// Un engagement que la modification touche : nomme (`qui`), une ligne par consequence.
#[derive(Debug, Serialize)]
struct Touche {
    qui: String,
    consequences: Vec<String>,
}

/// Ce que rend « Retirer cet engagement » : la phrase ; un envoi parti, l'avertissement a confirmer.
#[derive(Debug, Serialize)]
struct Retrait {
    phrase: Phrase,
    #[serde(skip_serializing_if = "Option::is_none")]
    avertissement: Option<AvertissementDuRetrait>,
}

// ------------------------------------------------------------------ outils

#[track_caller]
pub(super) fn refus(m: impl Into<String>) -> Echec {
    erreur(Classe::Refuse, m)
}

/// L'ENGAGEMENT QU'UNE CARTE MONTRE : celui que designe son identite, que le moteur lui a rendue
/// (`engagement::stock::identites`). Refus : une identite que l'ecran de `mois` ne montre pas.
fn designe(ctx: &Contexte, mois: Mois, id: &Identite) -> R<(RangDansLeMois, engagement::stock::Objet)> {
    engagement::stock::designe(&ctx.racine, mois, id).map_err(|e| e.reclasser(Classe::Refuse))
}


// ------------------------------------------------------------------ valider-engagement


charge! {
    /// La charge de « Valider l'engagement » : le mois de la carte, et son identite (son brouillon est ecrit, case par
    /// case, par `enregistrer-saisie`).
    #[derive(Debug, Clone, Serialize)]
    pub(crate) struct ChargeValider {
        pub(crate) mois: Mois = legendes::MOIS,
        pub(crate) engagement: Identite = IDENTITE,
    }
}

/// « VALIDER L'ENGAGEMENT » : le brouillon ecrit de la carte (`engagement`, son identite) se valide : il se lit en carte
/// valide (`engagement::carte::CarteValide::depuis`, dans le calcul) et ecrit ses paies, une par mois qu'il couvre
/// (`engagement::etats::valider`). Le tout se fait dans un brouillon de fichiers (`engagement::stock::Brouillon`) : il
/// lui manque un fait, la validation refuse en disant lequel, et rien n'est ecrit. Valider ne se defait pas : un
/// engagement valide se modifie (`modifier-engagement`) ou se retire. L'horloge de la compagnie fictive valide par ce
/// geste aussi.
pub(crate) fn valider_engagement(ctx: &Contexte, carte: ChargeValider) -> R<Validation> {
    let ChargeValider { mois, engagement: id } = carte;
    let _verrou = crate::mois::avant_d_ecrire(ctx, mois)?;
    let brouillon = engagement::stock::Brouillon::ouvrir(&ctx.racine);
    // Ce qui se valide est le brouillon ecrit, lu en sa forme (`engagement::carte::Brouillon`).
    crate::mois::brouillon_ecrit(ctx, mois, &id)?;
    let (i, o) = engagement::stock::designe(&ctx.racine, mois, &id)?;
    // VALIDER CE QUI L'EST DEJA ne change rien et n'est pas une faute : rien ne s'ecrit (AGENTS, « Enregistrer sans rien
    // changer n'est jamais une faute »).
    if o.est_valide() {
        return Ok(Validation { engagement: id, phrase: Phrase::fixe("Engagement validé.") });
    }
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let data = engagement::stock::vue_du_mois(&ctx.racine, mois)
        ?
        .ok_or_else(|| refus(format!("{} : l'écran ne montre pas cet engagement ; rechargez le mois. Rien n'est écrit.", engagement::etats::qui_de(ctx, &o))))?;
    if let Some(manque) = crate::mois::manque_de(ctx, &data, i) {
        return Err(crate::mois::refus_du_calcul(ctx, &data, manque));
    }
    engagement::etats::valider(ctx, mois, i, &departs).map_err(|m| crate::mois::refus_du_calcul(ctx, &data, m))?;
    brouillon.ecrire()?;
    if let Some(e) = i.dans(&data.engagements) {
        if let Some(cle) = &e.salarie {
            crate::fiches::poser_anciennete_a_l_embauche(ctx, cle, e.debut_lu())?;
        }
    }
    Ok(Validation { engagement: id, phrase: Phrase::fixe("Engagement validé.") })
}

// ------------------------------------------------------------------ modifier-engagement

/// « MODIFIER L'ENGAGEMENT » : un engagement valide ouvre le brouillon de sa carte de `mois` (`Brouillon::de_la_carte`
/// de sa vue), qui prend les changements comme `enregistrer-saisie` les prend (`Changements::appliquer`), puis il se
/// revalide, d'un geste. La carte se designe par son identite (`engagement`, que le moteur lui a rendue) ; elle modifie
/// son engagement, et lui seul. LE MOIS DE LA CARTE
/// EST LE « A PARTIR DE » : un terme du contrat qu'elle change (le salaire, la duree du travail, l'emploi...) vaut de ce
/// mois jusqu'au changement suivant (`engagement::stock::poser_la_saisie`), du 1er, ou du `jour` dit.
///
/// - Rien n'a change : rien ne s'ecrit, la reponse le dit (`change` faux).
/// - Quelque chose a change, et rien de ce qui est parti ne le demande : la modification s'ecrit, l'engagement reste valide.
/// - Ce qui est parti le demande : la reponse porte l'avertissement, et rien n'est ecrit ; la page le montre et renvoie
///   la demande avec `confirme` (et `jour`, quand le changement ne vaut pas du 1er). `avertissement` : `engagements`,
///   l'engagement touche nomme (`qui`) avec une ligne par ce que le changement entraine pour un envoi parti sur la
///   version d'avant (`consequences`, `engagement::etats::a_refaire`) : un avenant a faire signer, une DPAE a deposer
///   pour une embauche avancee, un bulletin rectificatif, la DSN qui se corrige ; et, pour un contrat au mois dont un
///   terme change dans un mois d'apres son premier, `jour` : la phrase qui dit d'ou il vaut (`a_partir_de`).
///
/// Refus, et rien n'est ecrit : un engagement qui n'est pas valide (il se saisit sur sa carte), importe, d'un mois tenu
/// avant PuppetPaie, d'un exercice clos ; un jour qui n'est pas un jour du contrat dans ce mois ; un changement que la
/// norme ne permet plus de declarer (`crate::regularisation::refus_non_declarable`) ; une saisie qui ne se calcule pas.
fn modifier_engagement(ctx: &Contexte, (charge, jour): (ChargeModifier, Option<socle::date::Jour>)) -> R<Modification> {
    let ChargeModifier { mois, engagement: id, changements, confirme } = charge;
    let _verrou = crate::mois::avant_d_ecrire(ctx, mois)?;
    let (i, o) = designe(ctx, mois, &id)?;
    if !o.est_valide() {
        return Err(refus(format!(
            "{} : cet engagement n'est pas validé ; il se saisit sur sa carte, puis « Valider l'engagement ». Rien n'est écrit.",
            engagement::etats::qui_de(ctx, &o)
        )));
    }
    // LA MODIFICATION SE FAIT DANS UN BROUILLON (`engagement::stock::Brouillon`) : calculee en memoire, elle ne s'ecrit
    // sur le disque que quand elle est faite ; rien n'a change, un avertissement a confirmer, un refus : rien n'est ecrit.
    let brouillon = engagement::stock::Brouillon::ouvrir(&ctx.racine);
    let depuis = match modifier_puis_revalider(ctx, mois, i, &o, changements, confirme, jour)? {
        Issue::Ecrite(depuis) => depuis,
        Issue::Rien => return Ok(Modification::Faite { change: false, phrase: Phrase::fixe("Rien n'a changé : rien n'est écrit.") }),
        Issue::Avertir(a) => {
            let phrases = avertissement_en_phrases(&a);
            return Ok(Modification::Avertie { avertissement: a, phrase: Phrase::fixe("Ce qui est parti le demande : rien n'est écrit sans confirme."), phrases });
        }
    };
    brouillon.ecrire()?;
    let vue = engagement::stock::vue_du_mois(&ctx.racine, mois)?;
    if let Some(e) = vue.as_ref().and_then(|d| i.dans(&d.engagements)) {
        if let Some(cle) = &e.salarie {
            crate::fiches::poser_anciennete_a_l_embauche(ctx, cle, e.debut_lu())?;
        }
    }
    crate::accueil::suppression::sortir_de_la_demo(&ctx.racine, mois).map_err(|m| m.reclasser(Classe::Disque))?;
    let phrase = match depuis {
        Some(j) => socle::phrase!("Engagement modifié. {}", vaut_a_partir_du(&j)),
        None => Phrase::fixe("Engagement modifié."),
    };
    Ok(Modification::Faite { change: true, phrase })
}

/// Ce que la modification a donne.
enum Issue {
    /// Rien n'a change.
    Rien,
    /// Ce qui est parti le demande : l'avertissement, a confirmer.
    Avertir(Avertissement),
    /// Ecrite : l'engagement revalide ; le jour d'ou vaut un terme qu'elle change dans un mois d'apres le premier.
    Ecrite(Option<socle::date::Jour>),
}

/// « Ce qui change vaut à partir du 01/09/2026… » : un terme change du jour `j` au changement suivant ; un montant d'un
/// seul mois est un fait du mois.
fn vaut_a_partir_du(j: &socle::date::Jour) -> String {
    format!(
        "Ce qui change vaut à partir du {}, jusqu'au prochain changement ; un montant d'un seul mois se dit en prime.",
        j.jj_mm_aaaa()
    )
}

/// LE JOUR D'OU VAUT UN TERME que la carte de `mois` change dans un contrat paye au mois, `o` avant et `devenu` apres : le
/// dernier changement date de ce mois, s'il n'etait pas deja ecrit. `None` : la carte ne change aucun terme a ce mois
/// (le premier mois du contrat dit ses termes de son debut).
fn a_partir_de(o: &engagement::stock::Objet, devenu: &engagement::stock::Objet, mois: Mois) -> R<Option<socle::date::Jour>> {
    if !devenu.mensuel()? {
        return Ok(None);
    }
    let du_mois = |x: &engagement::stock::Objet| -> Vec<engagement::stock::Avenant> {
        x.avenants().iter().filter(|a| a.depuis.mois_civil() == mois).cloned().collect()
    };
    let apres = du_mois(devenu);
    Ok((apres != du_mois(o)).then(|| apres.iter().map(|a| a.depuis).max()).flatten())
}

/// La modification elle-meme, dans le brouillon de fichiers ouvert : la carte de `mois` de l'engagement `o`, de rang
/// `i`, prend ses changements ; l'engagement se rouvre, prend la carte changee et se revalide. Ce qu'elle rend dit si
/// elle s'ecrit (`Issue::Ecrite`).
fn modifier_puis_revalider(ctx: &Contexte, lu: Mois, i: RangDansLeMois, o: &engagement::stock::Objet, changements: Changements, confirme: bool, jour: Option<socle::date::Jour>) -> R<Issue> {
    let qui = engagement::etats::qui_de(ctx, o);
    let vue = o.vue(lu)?.ok_or_else(|| refus(format!("{} : l'écran ne montre pas cet engagement ; rechargez le mois. Rien n'est écrit.", qui)))?;
    let mut carte = Brouillon::de_la_carte(&vue, &qui).map_err(|e| e.reclasser(Classe::Refuse))?;
    changements.appliquer(ctx, lu, &mut carte, &crate::mois::metiers_de_la_compagnie(ctx)?, |_| qui.clone()).map_err(|e| e.reclasser(Classe::Refuse))?;
    engagement::etats::modifier(ctx, lu, i)?;
    engagement::stock::poser_la_saisie(ctx, lu, i, &carte.en_carte(), jour).map_err(|e| e.reclasser(Classe::Refuse))?;
    if !engagement::etats::a_revalider_rouvert(ctx, lu, i)? {
        return Ok(Issue::Rien);
    }
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let devenu = engagement::stock::objets(&ctx.racine)?.into_iter().find(|x| x.fichier == o.fichier && x.rang == o.rang);
    let depuis = match devenu.as_ref() {
        Some(d) => a_partir_de(o, d, lu)?,
        None => None,
    };
    if let Some(devenu) = devenu {
        let mut m = crate::etapes::Moteur::nouveau(ctx);
        if let Some(texte) = crate::regularisation::refus_non_declarable(&mut m, ctx, o, &devenu, &departs)? {
            return Err(erreur(Classe::ModificationNonDeclarable, texte));
        }
    }
    let v = engagement::etats::valider(ctx, lu, i, &departs).map_err(|m| {
        match engagement::stock::vue_du_mois(&ctx.racine, lu) {
            Ok(Some(data)) => crate::mois::refus_du_calcul(ctx, &data, m),
            Ok(None) => m,
            Err(lu) => lu,
        }
    })?;
    // L'AVERTISSEMENT : l'engagement touche, nomme (`qui`), une ligne par ce que le changement entraine pour lui, et le
    // jour d'ou vaut un terme change, qui se dit quand ce n'est pas le 1er.
    let lignes = ce_que_le_changement_entraine(ctx, &v, &departs)?;
    if !confirme && !lignes.is_empty() {
        let touche = Touche { qui: engagement::etats::qui_de(ctx, v.objet()), consequences: lignes };
        return Ok(Issue::Avertir(Avertissement::de(vec![touche], depuis)));
    }
    Ok(Issue::Ecrite(depuis))
}

/// L'avertissement d'une modification en phrases : ce que le changement entraine pour chaque engagement touche, puis
/// d'ou vaut un terme change (`jour`).
fn avertissement_en_phrases(a: &Avertissement) -> Vec<String> {
    let mut lignes: Vec<String> = a.engagements.iter().flat_map(|e| e.consequences.iter().map(move |c| format!("{} : {}", e.qui, c))).collect();
    if let Some(j) = &a.jour_dit {
        lignes.push(format!("{} (jour : un autre jour du mois)", j));
    }
    lignes
}

/// Ce qu'un contrat parti que le changement modifie entraine.
const UN_AVENANT: &str = "un avenant à faire signer";

/// CE QUE LE CHANGEMENT ENTRAINE, une ligne par consequence, pour l'engagement revalide `v` : chaque envoi parti sur une
/// version qui n'est plus la sienne (`engagement::etats::a_refaire`). Une embauche reportee ne refait pas la DPAE
/// (`crate::regularisation::dpae_d_une_date_changee`).
pub(crate) fn ce_que_le_changement_entraine(ctx: &Contexte, v: &engagement::etats::Valide, departs: &engagement::departs::Departs) -> R<Vec<String>> {
    use engagement::etats::ARefaire;
    let presente = v
        .objet()
        .brut
        .valide
        .as_ref()
        .and_then(|x| x.saisie.as_ref())
        .and_then(|s| s.carte.debut_lu())
        .ok_or_else(|| erreur(Classe::DefautDuLogiciel, "Défaut du logiciel : l'engagement revalidé n'a pas sa saisie."))?;

    let mut m = crate::etapes::Moteur::nouveau(ctx);
    let mut out = Vec::new();
    for a in engagement::etats::a_refaire(ctx, v, departs)? {
        match a {
            ARefaire::Avenant { .. } => out.push(UN_AVENANT.to_string()),
            ARefaire::Dpae { debut_declare, .. } => {
                let avant = debut_declare.ok_or_else(|| erreur(Classe::DefautDuLogiciel, "Défaut du logiciel : la DPAE partie n'a pas sa date d'embauche."))?;
                let (rien_a_refaire, phrase) = crate::regularisation::dpae_d_une_date_changee(avant, presente)?;
                if !rien_a_refaire {
                    out.push(phrase);
                }
            }
            ARefaire::Bulletin { mois, .. } => out.push(un_bulletin_rectificatif(mois)),
            ARefaire::Dsn { mois, .. } => out.push(la_dsn_qui_change(&mut m, ctx, mois)?),
        }
    }
    Ok(out)
}

/// « un bulletin rectificatif de janvier 2026 » : la paie partie de `mois` se refait.
fn un_bulletin_rectificatif(mois: Mois) -> String {
    format!("un bulletin rectificatif {}", crate::regularisation::de_mois_en_clair(mois))
}

/// Ce qui arrive a la DSN deposee de `mois` quand sa paie change, comme toute modification d'un mois declare
/// (`crate::regularisation::modification_de_l_engagement`) : un annule-et-remplace jusqu'a la limite, ensuite la correction
/// dans la DSN qui porte (`compagnie::etat_dsn::dsn_qui_porte`).
fn la_dsn_qui_change(m: &mut crate::etapes::Moteur, ctx: &Contexte, mois: Mois) -> R<String> {
    use crate::regularisation::{de_mois_en_clair, DsnDeposee};
    let de = de_mois_en_clair(mois);
    Ok(match crate::regularisation::modification_de_l_engagement(m, ctx, mois)? {
        Some(DsnDeposee::Corrigee { porteur }) => {
            format!("la correction dans la DSN {} (celle {}, déposée, ne se remplace plus)", de_mois_en_clair(porteur), de)
        }
        Some(DsnDeposee::Remplacee { limite }) => {
            format!("un annule-et-remplace de la DSN {}, admis jusqu'au {} {}", de, limite.jj_mm_aaaa(), socle::paris::HeureDuJour::MINUIT.dite())
        }
        None => format!("la DSN {} à corriger", de),
    })
}

// ------------------------------------------------------------------ retirer-engagement

/// « Retirer cet engagement » : la carte se designe par son identite (`engagement`, que le moteur lui a rendue).
/// Tant que rien n'est parti de lui, l'engagement se supprime, meme valide, et rend son numero
/// (`engagement::etats::supprimer`). Des qu'un envoi est parti, la route avertit sans rien ecrire (`avertissement` : ce
/// qui est parti) ; renvoyee avec `confirme`, l'engagement devient non actif (`engagement::etats::desactiver`) et la
/// phrase dit ce que cela rend a faire (la DSN d'un mois a corriger). Rien ne part de lui-meme.
fn retirer_engagement(ctx: &Contexte, charge: ChargeRetirer) -> R<Retrait> {
    let mois = charge.mois;
    let _verrou = crate::mois::avant_d_ecrire(ctx, mois)?;
    let (i, o) = designe(ctx, mois, &charge.engagement)?;
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    if !charge.confirme {
        let parti = !engagement::etats::ce_qui_est_parti(&o, &departs).is_empty();
        return match engagement::etats::supprimer(ctx, &o, &departs) {
            Ok(()) => Ok(Retrait { phrase: Phrase::fixe("Engagement retiré."), avertissement: None }),
            Err(m) if parti => Ok(Retrait {
                phrase: socle::phrase!("{}", m.phrase),
                avertissement: Some(AvertissementDuRetrait {
                    phrase: m.phrase,
                    titre: "Retirer cet engagement ?",
                    garder: "Garder l'engagement",
                    confirmer: "Le rendre non actif",
                }),
            }),
            Err(m) => Err(m),
        };
    }
    let n = engagement::etats::desactiver(ctx, charge.mois, i, &departs)?;
    let a_faire = a_faire_en_phrases(&n.a_refaire(&departs));
    let phrase = if a_faire.is_empty() { Phrase::fixe("Engagement non actif.") } else { socle::phrase!("Engagement non actif. À faire : {}.", a_faire.join(", ")) };
    Ok(Retrait { phrase, avertissement: None })
}

/// Ce qu'un engagement non actif rend a faire, en phrases pour l'ecran.
pub(crate) fn a_faire_en_phrases(a: &[engagement::etats::ARefaire]) -> Vec<String> {
    use engagement::etats::ARefaire;
    a.iter()
        .filter_map(|x| match x {
            ARefaire::Dsn { mois, .. } => Some(format!("DSN de {} à corriger", mois.libelle(socle::mois::Graphie::Accentuee))),
            _ => None,
        })
        .collect()
}
