//! LES MODELES DE CONTRAT DE LA COMPAGNIE : ce que Configuration › Modeles de contrat montre et fait, et le modele que
//! chaque contrat suit.
//!
//! Chaque nature de contrat a son MODELE DE BASE, livre (`contrat_ecrit.modele` de `contrats-base.json`, dans
//! `donnees/modeles/`) : il suit les mises a jour du logiciel et ne se modifie pas. La compagnie en copie des MODELES
//! SPECIAUX, chacun un nom et la base dont il part (l'empreinte de son texte au jour de la copie : le logiciel dit si la
//! base a change depuis) ; ils sont ranges avec elle (`gabarits/contrats/modeles.json`, la forme `ModelesDeContrat`) et
//! sauvegardes avec elle. Elle choisit le modele par defaut de chaque nature (la base sinon) ; un engagement en choisit un
//! sous « Avance » (sa case `modele_contrat`, le defaut sinon). Un modele se verifie paragraphe par paragraphe
//! (`documents::contrat_ecrit::manques`) avant d'etre ecrit, et le refus dit quoi.
//!
//! LE CONTRAT PARTI GARDE SON TEXTE : au depart, le texte du modele qu'il suit se fige sous son empreinte
//! (`gabarits/contrats/partis/<empreinte>.modele`, `figer`), que la ligne du journal note (`ContratParti::modele`) ;
//! refaire ce contrat, ou lui faire un avenant, relit ce texte-la (`du_contrat_parti`).
//!
//! L'avenant garde ici son modele propre d'une nature, pose en texte entier dans `gabarits/` (`poser-modele-avenant`) ;
//! les clauses particulieres de la compagnie (`clauses_particulieres` de sa fiche) s'ecrivent ici une fois.

use droit::fiche::Lue as _;
use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::catalogue::Route;
use crate::catalogue::{Acte, Affiche, Champ, Ecran, Fiche, Forme, Place, Refus, MANQUE, OPERATION};
use crate::enveloppe::{en_json, Charge, Dite, Porte, Rendu};
use documents::contrat_ecrit::{ChampDeModele, ModeleDuContrat};
use documents::marqueurs::{Marqueur, ModeleLu, DE_L_AVENANT, DU_REMPLACEMENT};
use droit::contexte::Contexte;
use droit::natures::Nature;
use engagement::calcul::Paie;
use socle::echec::{erreur, Classe, Echec, R};
use socle::ecran::Pastille;
use socle::fichiers::dossier_du_code;

const NATURE: Champ = Champ { nom: "nature", libelle: Affiche::new("Nature du contrat"), forme: Forme::Texte, sens: "la nature du contrat : cddu_spectacle, cdd, cdi…", avance: false };
const ID: Champ = Champ { nom: "id", libelle: Affiche::new("Modèle"), forme: Forme::Texte, sens: "l'id d'un modèle spécial (`modeles` de modeles-contrat)", avance: false };
const ID_OU_BASE: Champ = Champ { nom: "id", libelle: Affiche::new("Modèle"), forme: Forme::Texte, sens: "l'id d'un modèle spécial ; absent ou null : le modèle de base de la nature", avance: false };
const TEXTE: Champ = Champ { nom: "texte", libelle: Affiche::new("Modèle"), forme: Forme::Texte, sens: "le modèle entier (TITRE:, ARTICLE:… ; un alinéa qui commence par [si un.champ] ne s'écrit que si ce champ a une valeur)", avance: false };
const MODELE_REFUSE: Refus = Refus { classe: Classe::Refuse, corriger: "le modèle : ce que la phrase nomme, paragraphe par paragraphe ; rien n'est écrit" };

/// Ce que la ligne d'un modele special dit quand la base dont il est copie a change depuis (une mise a jour).
const BASE_CHANGEE: &str = "Le modèle de base a changé depuis la copie : le relire pour reprendre ce qui a changé.";

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::ModelesContrat);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::ModelesContrat,
        geste: "Lister les modèles de contrat de chaque nature (la base et les spéciaux), et les modèles d'avenant.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "natures (chacune : ses modèles, la base d'abord, le défaut marqué, ce qui manque à chacun ; son avenant), champs permis, clauses de la compagnie",
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::ModeleContrat,
        geste: "Lire un modèle de contrat, paragraphe par paragraphe, et ce qui lui manque.",
        charge: &[NATURE, ID_OU_BASE],
        exemple: r#"{"nature":"cddu_spectacle"}"#,
        rend: "texte, articles (titre, alinéas et leur condition), manques, en_francais",
        refus: &[OPERATION, MANQUE],
        acte: Acte::Lit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::CopierModeleContrat,
        geste: "Copier le modèle de base d'une nature en un modèle spécial de la compagnie.",
        charge: &[NATURE, Champ { nom: "nom", libelle: Affiche::new("Nom"), forme: Forme::Texte, sens: "le nom du modèle spécial", avance: false }],
        exemple: r#"{"nature":"cddu_spectacle","nom":"Tournée"}"#,
        rend: "les modèles, comme modeles-contrat",
        refus: &[OPERATION, MANQUE],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::EcrireModeleContrat,
        geste: "Écrire le texte d'un modèle spécial ; il est vérifié d'abord.",
        charge: &[ID, TEXTE],
        exemple: r#"{"id":"m1","texte":"TITRE: Contrat…"}"#,
        rend: "les modèles, comme modeles-contrat",
        refus: &[MODELE_REFUSE, MANQUE],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::SupprimerModeleContrat,
        geste: "Supprimer un modèle spécial ; un contrat déjà parti garde son texte.",
        charge: &[ID],
        exemple: r#"{"id":"m1"}"#,
        rend: "les modèles, comme modeles-contrat",
        refus: &[OPERATION, MANQUE],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::ModeleContratParDefaut,
        geste: "Choisir le modèle par défaut d'une nature.",
        charge: &[NATURE, ID_OU_BASE],
        exemple: r#"{"nature":"cddu_spectacle","id":"m1"}"#,
        rend: "les modèles, comme modeles-contrat",
        refus: &[OPERATION, MANQUE],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::PoserModeleAvenant,
        geste: "Poser le modèle d'avenant propre à la compagnie.",
        charge: &[NATURE, TEXTE],
        exemple: r#"{"nature":"cddu_spectacle","texte":"TITRE: Avenant n° {{avenant.numero}}\n\nMODIFICATION:\nL'article {{modification.numero}} est désormais rédigé ainsi : « {{modification.apres}} »\n\nARTICLE: Le reste\nLe reste du contrat est inchangé.\n"}"#,
        rend: "les modèles, comme modeles-contrat",
        refus: &[MODELE_REFUSE, MANQUE],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::RetirerModeleAvenant,
        geste: "Remettre le modèle d'avenant livré à la place de celui de la compagnie.",
        charge: &[NATURE],
        exemple: r#"{"nature":"cddu_spectacle"}"#,
        rend: "les modèles, comme modeles-contrat",
        refus: &[OPERATION, MANQUE],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::ClausesParticulieres,
        geste: "Écrire les clauses particulières que chaque contrat reprend dès aujourd'hui.",
        charge: &[Champ { nom: "texte", libelle: Affiche::new("Clauses"), forme: Forme::Texte, sens: "les clauses ; vide : aucune", avance: false }],
        exemple: r#"{"texte":"Les répétitions ont lieu au théâtre."}"#,
        rend: "phrase",
        refus: &[OPERATION],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 60,
        ecran: ECRAN,
    },
];

/// Une route de ce module, branchee sur l'enveloppe.
pub fn route(porte: &Porte, charge: &Charge) -> R<Rendu> {
    let ctx = porte.ctx;
    match porte.route {
        Route::ModelesContrat => {
            let super::SansCharge {} = charge.lire()?;
            let mut m = modeles(ctx, "")?;
            m.ecran = Some(ecran_des_modeles(&m));
            en_json(m)
        }
        Route::ModeleContrat => en_json(lire(ctx, charge.lire()?)?),
        Route::CopierModeleContrat => en_json(copier(ctx, charge.lire()?)?),
        Route::EcrireModeleContrat => en_json(ecrire_texte(ctx, charge.lire()?)?),
        Route::SupprimerModeleContrat => en_json(supprimer(ctx, charge.lire()?)?),
        Route::ModeleContratParDefaut => en_json(par_defaut(ctx, charge.lire()?)?),
        Route::PoserModeleAvenant => en_json(poser_avenant(ctx, charge.lire()?)?),
        Route::RetirerModeleAvenant => en_json(retirer_avenant(ctx, charge.lire()?)?),
        Route::ClausesParticulieres => en_json(clauses(ctx, charge.lire()?)?),
        autre => Err(super::enveloppe::route_inconnue(autre.nom())),
    }
}

// ------------------------------------------------------------------ la forme

/// LES MODELES DE CONTRAT DE LA COMPAGNIE (`gabarits/contrats/modeles.json`) : ses modeles speciaux, dans l'ordre ou
/// elle les a faits, et le modele par defaut de chaque nature qui n'est pas sa base (nature -> id d'un special).
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelesDeContrat {
    speciaux: Vec<Special>,
    defauts: BTreeMap<String, String>,
}

/// UN MODELE SPECIAL : son id (celui que l'engagement et le defaut nomment), son nom, sa nature, l'empreinte du texte
/// de la base dont il a ete copie, et son texte.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Special {
    id: String,
    nom: String,
    nature: String,
    base: String,
    texte: String,
}

fn dossier(ctx: &Contexte) -> PathBuf {
    ctx.gabarits().join("contrats")
}

fn disque(chemin: &std::path::Path, e: std::io::Error) -> Echec {
    erreur(Classe::Disque, socle::echec::du_disque(&chemin, &e))
}

/// Les modeles de la compagnie ; aucun fichier : elle n'en a fait aucun.
fn lus(ctx: &Contexte) -> R<ModelesDeContrat> {
    let chemin = dossier(ctx).join("modeles.json");
    if !chemin.is_file() {
        return Ok(ModelesDeContrat::default());
    }
    socle::forme::lire_fichier(&chemin, "les modèles de contrat de la compagnie").map_err(|m| erreur(Classe::Illisible, m))
}

fn ecrire(ctx: &Contexte, m: &ModelesDeContrat) -> R<()> {
    let d = dossier(ctx);
    std::fs::create_dir_all(&d).map_err(|e| disque(&d, e))?;
    let (chemin, tmp) = (d.join("modeles.json"), d.join("modeles.json.tmp"));
    std::fs::write(&tmp, socle::ecriture::json_indente(m, 1)).map_err(|e| disque(&tmp, e))?;
    std::fs::rename(&tmp, &chemin).map_err(|e| disque(&chemin, e))
}

/// L'empreinte d'un texte de modele : ce qui le nomme une fois fige.
fn empreinte(texte: &str) -> String {
    socle::fichiers::sha256_hex(texte.as_bytes()).chars().take(16).collect()
}

fn refus(m: impl Into<String>) -> Echec {
    erreur(Classe::Refuse, m)
}

fn nature_de(cle: &str) -> R<std::sync::Arc<Nature>> {
    droit::natures::nature(cle).map_err(Echec::from)
}

/// Le texte du modele de base de cette nature.
fn texte_de_base(nat: &Nature) -> R<String> {
    let nom = nat.modele_contrat().ok_or_else(|| refus(format!("La nature {} n'a pas de contrat écrit.", nat.libelle)))?;
    let chemin = dossier_du_code().join("modeles").join(nom);
    std::fs::read_to_string(&chemin).map_err(|e| disque(&chemin, e))
}

fn nom_de_base(nat: &Nature) -> String {
    format!("Le modèle de base du {}", nat.nom_du_contrat())
}

fn nom_special(s: &Special) -> String {
    format!("Le modèle « {} »", s.nom)
}

/// Un modele lu pour un contrat ; un texte qui ne se lit pas est un refus.
fn modele_du_texte(nom: String, base: bool, texte: &str) -> R<ModeleDuContrat> {
    let lu = ModeleLu::lire(texte, &nom).map_err(|m| erreur(Classe::ModeleIncomplet, m))?;
    Ok(ModeleDuContrat { nom, base, lu })
}

fn special<'a>(m: &'a ModelesDeContrat, id: &str) -> R<&'a Special> {
    m.speciaux.iter().find(|s| s.id == id).ok_or_else(|| refus(format!("Aucun modèle de contrat {} : en choisir un de la liste (modeles-contrat).", id)))
}

// ------------------------------------------------------------------ le modele d'un contrat

/// LE MODELE QUE SUIT CE CONTRAT, avant son depart : celui que son engagement choisit (`modele_contrat`), sinon le
/// defaut que la compagnie a choisi pour sa nature, sinon la base de sa nature.
pub(crate) fn du_contrat(ctx: &Contexte, p: &Paie) -> R<ModeleDuContrat> {
    let nat = p.engagement.nat().map_err(Echec::from)?;
    let m = lus(ctx)?;
    let choisi = match &p.engagement.ecrit.modele_contrat {
        Some(id) => Some(m.speciaux.iter().find(|s| s.id == *id).ok_or_else(|| {
            refus(format!("Le contrat de {} suit le modèle {}, que la compagnie n'a plus : en choisir un autre dans son engagement (Avancé › Modèle de contrat). Rien n'est écrit.", p.salarie.id, id))
        })?),
        None => m.defauts.get(&nat.cle).map(|id| special(&m, id)).transpose()?,
    };
    match choisi {
        Some(s) if s.nature != nat.cle => Err(refus(format!("{} est un modèle de {}, pas de {} : en choisir un autre. Rien n'est écrit.", nom_special(s), nature_de(&s.nature)?.nom_du_contrat(), nat.nom_du_contrat()))),
        Some(s) => modele_du_texte(nom_special(s), false, &s.texte),
        None => modele_du_texte(nom_de_base(&nat), true, &texte_de_base(&nat)?),
    }
}

/// FIGE LE TEXTE DE CE MODELE, au depart d'un contrat qui le suit (`gabarits/contrats/partis/<empreinte>.modele`, s'il
/// ne l'est pas deja) ; rend son empreinte, que la ligne du journal note.
pub(crate) fn figer(ctx: &Contexte, m: &ModeleDuContrat) -> R<String> {
    let e = empreinte(&m.lu.texte);
    let d = dossier(ctx).join("partis");
    let chemin = d.join(format!("{}.modele", e));
    if !chemin.is_file() {
        std::fs::create_dir_all(&d).map_err(|x| disque(&d, x))?;
        std::fs::write(&chemin, &m.lu.texte).map_err(|x| disque(&chemin, x))?;
    }
    Ok(e)
}

/// LE MODELE AVEC LEQUEL CE CONTRAT EST PARTI : le texte fige sous l'empreinte que sa ligne du journal note. Une ligne
/// qui n'en note pas est celle d'un contrat parti avant que le journal note son modele : il se refait avec le modele
/// qu'il suit aujourd'hui (`du_contrat`).
pub(crate) fn du_contrat_parti(ctx: &Contexte, empreinte: Option<&str>, p: &Paie) -> R<ModeleDuContrat> {
    let Some(e) = empreinte else { return du_contrat(ctx, p) };
    let chemin = dossier(ctx).join("partis").join(format!("{}.modele", e));
    let texte = std::fs::read_to_string(&chemin).map_err(|x| disque(&chemin, x))?;
    modele_du_texte("Le modèle avec lequel ce contrat est parti".to_string(), false, &texte)
}

// ------------------------------------------------------------------ les charges et les reponses

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeLire {
    nature: String,
    id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeCopier {
    nature: String,
    nom: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeEcrire {
    id: String,
    texte: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeId {
    id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeNature {
    nature: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeAvenant {
    nature: String,
    texte: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeClauses {
    texte: String,
}

/// Un modele de contrat d'une nature, tel que la liste le dit : la base (sans id) ou un special.
#[derive(Debug, Serialize)]
struct ModeleDit {
    id: Option<String>,
    nom: String,
    base: bool,
    defaut: bool,
    /// Ce que la ligne dit d'une base qui a change depuis la copie ; `None` : rien.
    base_changee: Option<&'static str>,
    manques: Vec<String>,
    pastille: Pastille,
}

/// Le modele d'avenant d'une nature : le livre ou celui de la compagnie, son refus s'il en a un, son texte (le livre, ou le sien ; aucun s'il ne se lit pas).
#[derive(Debug, Serialize)]
struct AvenantDit {
    nom: String,
    propre: bool,
    refus: Option<String>,
    texte: Option<String>,
    bouton: &'static str,
    pastille: Pastille,
}

/// Une nature, ses modeles de contrat et son avenant, et les champs qu'ils peuvent citer.
#[derive(Debug, Serialize)]
struct NatureDite {
    nature: String,
    libelle: String,
    nom: String,
    modeles: Vec<ModeleDit>,
    avenant: AvenantDit,
    champs: Vec<ChampDeModele>,
    champs_titre: String,
}

/// Ce que l'ecran des modeles de contrat dit (`ecran` de `modeles-contrat`).
#[derive(Debug, Serialize)]
struct EcranDesModeles {
    intro: &'static str,
    clauses: &'static str,
    comment: [&'static str; 3],
    en_francais: &'static str,
    avenant: String,
}

/// Ce que rendent `modeles-contrat` et chaque geste qui change un modele.
#[derive(Debug, Serialize)]
struct Modeles {
    phrase: String,
    natures: Vec<NatureDite>,
    champs_de_l_avenant: Vec<String>,
    clauses_particulieres: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ecran: Option<EcranDesModeles>,
}

/// Un alinea d'un article : sa condition (le champ qui doit avoir une valeur) et son texte.
#[derive(Debug, Serialize)]
struct AlineaRendu {
    si: Option<String>,
    texte: String,
}

/// Un article d'un modele : son numero, son titre, ses alineas.
#[derive(Debug, Serialize)]
struct ArticleRendu {
    numero: usize,
    titre: String,
    alineas: Vec<AlineaRendu>,
}

/// Ce que rend `modele-contrat`.
#[derive(Debug, Serialize)]
struct ModeleRendu {
    phrase: String,
    nature: String,
    id: Option<String>,
    nom: String,
    base: bool,
    texte: String,
    articles: Vec<ArticleRendu>,
    manques: Vec<String>,
    en_francais: &'static str,
}

/// Ce que rend `clauses-particulieres`.
#[derive(Debug, Serialize)]
struct ClausesEcrites {
    phrase: &'static str,
}

impl Dite for Modeles {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

impl Dite for ModeleRendu {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

impl Dite for ClausesEcrites {
    fn phrase(&self) -> &str {
        self.phrase
    }
}

// ------------------------------------------------------------------ les gestes

/// Les natures dont la compagnie fait les contrats : le contrat d'usage toujours ; celles d'un permanent quand elle en
/// emploie (`emploie_des_permanents` de sa fiche).
fn natures_de(ctx: &Contexte) -> R<Vec<std::sync::Arc<Nature>>> {
    let permanents = ctx.emploie_des_permanents();
    Ok(droit::natures::natures()
        .map_err(Echec::from)?
        .iter()
        .filter(|n| n.modele_contrat().is_some() && (permanents || !n.permanent()))
        .cloned()
        .collect())
}

fn manques(nat: &Nature, texte: &str) -> R<Vec<String>> {
    documents::contrat_ecrit::manques(nat, texte, None).map_err(|m| erreur(Classe::Illisible, m))
}

/// CE QUE LA PAGE MONTRE : pour chaque nature, ses modeles de contrat (la base, puis les speciaux, le defaut marque,
/// ce qui manque a chacun, si sa base a change depuis la copie), son modele d'avenant, les champs qu'un modele peut
/// citer ; et les clauses particulieres de la compagnie. `devant` : ce que la phrase dit avant (le geste qui vient d'etre
/// fait).
fn modeles(ctx: &Contexte, devant: &str) -> R<Modeles> {
    let m = lus(ctx)?;
    let mut sortie = Vec::new();
    for nat in natures_de(ctx)? {
        let base = texte_de_base(&nat)?;
        let defaut = m.defauts.get(&nat.cle);
        let pastille = |manques: &[String], defaut: bool, base: bool| match (manques.is_empty(), defaut) {
            (false, _) => Pastille::de("refuse", "incomplet"),
            (true, true) => Pastille::de("fait", "par défaut"),
            (true, false) if base => Pastille::de("info", "le modèle du logiciel"),
            (true, false) => Pastille::de("info", "modèle spécial"),
        };
        let manques_base = manques(&nat, &base)?;
        let mut pieces = vec![ModeleDit { id: None, nom: nom_de_base(&nat), base: true, defaut: defaut.is_none(), base_changee: None, pastille: pastille(&manques_base, defaut.is_none(), true), manques: manques_base }];
        for s in m.speciaux.iter().filter(|s| s.nature == nat.cle) {
            let dits = manques(&nat, &s.texte)?;
            let est_defaut = defaut == Some(&s.id);
            pieces.push(ModeleDit { id: Some(s.id.clone()), nom: s.nom.clone(), base: false, defaut: est_defaut, base_changee: (s.base != empreinte(&base)).then_some(BASE_CHANGEE), pastille: pastille(&dits, est_defaut, false), manques: dits });
        }
        let mut champs = documents::contrat_ecrit::champs_des_modeles(&nat).map_err(|m| erreur(Classe::Illisible, m))?;
        champs.extend(DU_REMPLACEMENT.iter().map(|c| ChampDeModele { marqueur: c.to_string(), quoi: "le contrat remplacé (bloc ANNULE: seulement)", obligatoire: false, texte: None, si_dite: false }));
        let champs_titre = format!("Ses champs ({})", champs.len());
        sortie.push(NatureDite { nature: nat.cle.clone(), libelle: nat.libelle.clone(), nom: nat.nom_du_contrat().to_string(), modeles: pieces, avenant: avenant_dit(ctx, &nat)?, champs, champs_titre });
    }
    let speciaux = m.speciaux.len();
    Ok(Modeles {
        phrase: format!("{}Modèles de contrat spéciaux de la compagnie : {}.", devant, speciaux),
        natures: sortie,
        champs_de_l_avenant: DE_L_AVENANT.iter().map(|c| c.to_string()).collect(),
        clauses_particulieres: ctx.entreprise.clauses_particulieres.portee().map(String::from),
        ecran: None,
    })
}

/// LIRE UN MODELE : `{nature, id}` (sans id : la base), son texte, ses articles et leurs alineas, ce qui lui manque.
fn lire(ctx: &Contexte, c: ChargeLire) -> R<ModeleRendu> {
    let nat = nature_de(&c.nature)?;
    let m = lus(ctx)?;
    let (nom, base, texte) = match &c.id {
        Some(id) => {
            let s = special(&m, id)?;
            (s.nom.clone(), false, s.texte.clone())
        }
        None => (nom_de_base(&nat), true, texte_de_base(&nat)?),
    };
    let articles = match fabrique_pdf::modele::lire_modele(&texte, &nom) {
        Ok(lu) => lu
            .articles
            .iter()
            .enumerate()
            .map(|(n, (titre, alineas))| ArticleRendu {
                numero: n + 1,
                titre: titre.clone(),
                alineas: alineas
                    .iter()
                    .map(|a| match fabrique_pdf::modele::condition(a) {
                        Some((si, t)) => AlineaRendu { si: Some(si.to_string()), texte: t.to_string() },
                        None => AlineaRendu { si: None, texte: a.clone() },
                    })
                    .collect(),
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    let dits = manques(&nat, &texte)?;
    let phrase = if dits.is_empty() { format!("{} : complet.", nom) } else { format!("{} : {}", nom, dits.join(" ")) };
    Ok(ModeleRendu { phrase, nature: nat.cle.clone(), id: c.id, nom, base, texte, articles, manques: dits, en_francais: documents::contrat_ecrit::EN_FRANCAIS })
}

/// COPIER LA BASE D'UNE NATURE EN UN MODELE SPECIAL : `{nature, nom}` ; il sait de quelle base il part.
fn copier(ctx: &Contexte, c: ChargeCopier) -> R<Modeles> {
    let nat = nature_de(&c.nature)?;
    let nom = c.nom.trim();
    if nom.is_empty() {
        return Err(erreur(Classe::Manque, "Le modèle spécial a besoin d'un nom. Rien n'est écrit."));
    }
    let mut m = lus(ctx)?;
    if m.speciaux.iter().any(|s| s.nom == nom) {
        return Err(refus(format!("La compagnie a déjà un modèle « {} » : lui donner un autre nom. Rien n'est écrit.", nom)));
    }
    let texte = texte_de_base(&nat)?;
    let n = m.speciaux.iter().filter_map(|s| s.id.strip_prefix('m')?.parse::<u32>().ok()).max().map_or(1, |n| n + 1);
    m.speciaux.push(Special { id: format!("m{}", n), nom: nom.to_string(), nature: nat.cle.clone(), base: empreinte(&texte), texte });
    ecrire(ctx, &m)?;
    modeles(ctx, &format!("Modèle « {} » copié de la base du {}. ", nom, nat.nom_du_contrat()))
}

/// ECRIRE LE TEXTE D'UN MODELE SPECIAL : `{id, texte}`. Il se verifie d'abord (`documents::contrat_ecrit::manques`) ;
/// s'il lui manque quelque chose, rien n'est ecrit et le refus dit quoi.
fn ecrire_texte(ctx: &Contexte, c: ChargeEcrire) -> R<Modeles> {
    let mut m = lus(ctx)?;
    let i = m.speciaux.iter().position(|s| s.id == c.id).ok_or_else(|| refus(format!("Aucun modèle de contrat {} : en choisir un de la liste (modeles-contrat).", c.id)))?;
    let nat = nature_de(&m.speciaux[i].nature)?;
    let dits = manques(&nat, &c.texte)?;
    if !dits.is_empty() {
        return Err(refus(format!("{} {} Rien n'est écrit.", nom_special(&m.speciaux[i]), dits.join(" "))));
    }
    m.speciaux[i].texte = c.texte;
    let nom = m.speciaux[i].nom.clone();
    ecrire(ctx, &m)?;
    modeles(ctx, &format!("Modèle « {} » écrit. ", nom))
}

/// SUPPRIMER UN MODELE SPECIAL : `{id}` ; s'il etait le defaut de sa nature, la base le redevient. Un contrat deja parti
/// garde son texte (`figer`).
fn supprimer(ctx: &Contexte, c: ChargeId) -> R<Modeles> {
    let mut m = lus(ctx)?;
    let nom = special(&m, &c.id)?.nom.clone();
    m.speciaux.retain(|s| s.id != c.id);
    m.defauts.retain(|_, id| *id != c.id);
    ecrire(ctx, &m)?;
    modeles(ctx, &format!("Modèle « {} » supprimé. ", nom))
}

/// CHOISIR LE MODELE PAR DEFAUT D'UNE NATURE : `{nature, id}` ; sans id, la base.
fn par_defaut(ctx: &Contexte, c: ChargeLire) -> R<Modeles> {
    let nat = nature_de(&c.nature)?;
    let mut m = lus(ctx)?;
    let devant = match &c.id {
        Some(id) => {
            let s = special(&m, id)?;
            if s.nature != nat.cle {
                return Err(refus(format!("{} n'est pas un modèle de {}. Rien n'est écrit.", nom_special(s), nat.nom_du_contrat())));
            }
            let devant = format!("{} est le modèle par défaut du {}. ", nom_special(s), nat.nom_du_contrat());
            m.defauts.insert(nat.cle.clone(), id.clone());
            devant
        }
        None => {
            m.defauts.remove(&nat.cle);
            format!("La base est le modèle par défaut du {}. ", nat.nom_du_contrat())
        }
    };
    ecrire(ctx, &m)?;
    modeles(ctx, &devant)
}

/// LES MODELES QU'UN ENGAGEMENT DE CETTE NATURE PEUT CHOISIR (sa case `modele_contrat`) : le defaut d'abord (sans id),
/// puis chaque special ; vide quand la compagnie n'en a fait aucun.
pub(crate) fn au_choix(ctx: &Contexte, nature: &str) -> R<Vec<ModeleAuChoix>> {
    let m = lus(ctx)?;
    let siens: Vec<&Special> = m.speciaux.iter().filter(|s| s.nature == nature).collect();
    if siens.is_empty() {
        return Ok(Vec::new());
    }
    let defaut = match m.defauts.get(nature).map(|id| special(&m, id)).transpose()? {
        Some(s) => format!("Le modèle par défaut (« {} »)", s.nom),
        None => "Le modèle par défaut (la base)".to_string(),
    };
    let mut v = vec![ModeleAuChoix { id: None, nom: defaut }];
    v.extend(siens.into_iter().map(|s| ModeleAuChoix { id: Some(s.id.clone()), nom: s.nom.clone() }));
    Ok(v)
}

/// Un modele au choix d'un engagement : son id (aucun : le defaut de la nature) et son nom.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ModeleAuChoix {
    id: Option<String>,
    nom: String,
}

// ------------------------------------------------------------------ l'avenant

/// Le nom du modele livre de l'avenant de la nature `cle`.
fn nom_de_l_avenant(cle: &str) -> R<(std::sync::Arc<Nature>, String)> {
    let nat = nature_de(cle)?;
    let nom = nat.modele_avenant().ok_or_else(|| refus(format!("La nature {} n'a pas de modèle d'avenant.", nat.libelle)))?;
    Ok((nat, nom))
}

/// Le modele d'avenant de la compagnie, s'il est pose dans son dossier `gabarits/`.
fn propre(ctx: &Contexte, nom: &str) -> Option<PathBuf> {
    Some(ctx.gabarits().join(nom)).filter(|p| p.is_file())
}

fn avenant_dit(ctx: &Contexte, nat: &Nature) -> R<AvenantDit> {
    let (_, nom) = nom_de_l_avenant(&nat.cle)?;
    let a_soi = propre(ctx, &nom);
    let (verdict, texte) = match &a_soi {
        Some(p) => match std::fs::read_to_string(p) {
            Ok(t) => (verifier_avenant(&nom, &t).err(), Some(t)),
            Err(e) => (Some(format!("Ce modèle ne se lit pas ({}) : le poser de nouveau.", e)), None),
        },
        // Le modele livre, pour en partir.
        None => {
            let chemin = dossier_du_code().join("modeles").join(&nom);
            (None, Some(std::fs::read_to_string(&chemin).map_err(|e| disque(&chemin, e))?))
        }
    };
    let pastille = match (&a_soi, &verdict) {
        (Some(_), Some(_)) => Pastille::de("refuse", "le modèle de la compagnie, refusé"),
        (Some(_), None) => Pastille::de("fait", "le modèle de la compagnie"),
        (None, _) => Pastille::de("info", "le modèle du logiciel"),
    };
    Ok(AvenantDit { nom, propre: a_soi.is_some(), refus: verdict, texte, bouton: if a_soi.is_some() { "Modifier le modèle" } else { "Écrire le modèle" }, pastille })
}

/// UN MODELE D'AVENANT SE VERIFIE AVANT D'ETRE POSE : sa grammaire, son bloc MODIFICATION:, et chacun de ses champs
/// connu du logiciel.
fn verifier_avenant(nom: &str, texte: &str) -> Result<(), String> {
    let lu = ModeleLu::lire(texte, nom)?;
    let m = lu.decoupe(nom).map_err(|r| format!("Ce modèle ne se lit pas : {}", r))?;
    if m.modification.is_empty() {
        return Err("Ce modèle d'avenant n'a pas de bloc MODIFICATION: : c'est lui qui dit, article par article, ce qui change.".into());
    }
    let permis = |c: &Marqueur| documents::contrat_ecrit::CHAMPS.iter().any(|(k, _, _)| k == c) || DU_REMPLACEMENT.contains(c) || DE_L_AVENANT.contains(c);
    let mut hors: Vec<String> = Vec::new();
    for c in lu.morceaux.iter().filter_map(|x| match x {
        documents::marqueurs::Morceau::Marqueur(c) => Some(c),
        documents::marqueurs::Morceau::Texte(_) => None,
    }) {
        if !permis(c) && !hors.contains(&c.to_string()) {
            hors.push(c.to_string());
        }
    }
    if !hors.is_empty() {
        return Err(format!("Ce modèle cite des champs qu'un modèle d'avenant ne porte pas : {}. Les corriger d'après la liste des champs, ou les écrire en texte.", hors.join(", ")));
    }
    Ok(())
}

/// POSER LE MODELE D'AVENANT DE LA COMPAGNIE : `{nature, texte}`. Verifie d'abord ; l'ancien, s'il y en a un, est garde a
/// cote (`.avant`).
fn poser_avenant(ctx: &Contexte, c: ChargeAvenant) -> R<Modeles> {
    let (_, nom) = nom_de_l_avenant(&c.nature)?;
    verifier_avenant(&nom, &c.texte).map_err(|m| refus(format!("{} Rien n'est posé.", m)))?;
    let dossier = ctx.gabarits();
    std::fs::create_dir_all(&dossier).map_err(|e| disque(&dossier, e))?;
    let cible = dossier.join(&nom);
    if cible.is_file() {
        let ancien = dossier.join(format!("{}.avant", nom));
        std::fs::rename(&cible, &ancien).map_err(|e| disque(&cible, e))?;
    }
    std::fs::write(&cible, &c.texte).map_err(|e| disque(&cible, e))?;
    modeles(ctx, "Modèle d'avenant de la compagnie posé. ")
}

/// RETIRER LE MODELE D'AVENANT DE LA COMPAGNIE : le livre reprend sa place ; le sien est garde a cote (`.avant`).
fn retirer_avenant(ctx: &Contexte, c: ChargeNature) -> R<Modeles> {
    let (_, nom) = nom_de_l_avenant(&c.nature)?;
    if let Some(p) = propre(ctx, &nom) {
        let ancien = ctx.gabarits().join(format!("{}.avant", nom));
        std::fs::rename(&p, &ancien).map_err(|e| disque(&p, e))?;
    }
    modeles(ctx, "Modèle d'avenant de la compagnie retiré : le modèle livré revient. ")
}

/// LES CLAUSES PARTICULIERES DE LA COMPAGNIE : `{texte}`, reprises dans chaque contrat a partir d'aujourd'hui ;
/// l'engagement peut dire les siennes (« Plus d'options »). Vide : aucune.
fn clauses(ctx: &Contexte, c: ChargeClauses) -> R<ClausesEcrites> {
    let texte = c.texte.trim();
    let clauses = if texte.is_empty() { None } else { Some(texte) };
    crate::fiches::poser_entreprise(ctx, droit::fiche::ChangementEntreprise::clauses_particulieres(clauses.map(str::to_string)), Some(ctx.aujourdhui))?;
    Ok(ClausesEcrites { phrase: "Clauses particulières enregistrées." })
}

/// CE QUE L'ECRAN DES MODELES DE CONTRAT DIT (`ecran` de `modeles-contrat`) : a quoi sert un modele, les clauses
/// particulieres, comment faire celui de la compagnie, le francais, et ce que cite un modele d'avenant.
fn ecran_des_modeles(m: &Modeles) -> EcranDesModeles {
    EcranDesModeles {
        intro: "Chaque contrat que le logiciel fait signer suit un modèle. Le modèle de base de chaque sorte de contrat porte déjà ce que la loi exige, rempli de lui-même depuis l'engagement et la configuration ; il suit les mises à jour et ne se modifie pas. La compagnie le copie en modèles spéciaux, choisit celui par défaut, et un engagement peut en choisir un autre (Avancé). Un contrat parti garde son texte.",
        clauses: "Ce qu'elle ajoute à chacun de ses contrats (confidentialité, image, déplacements…). Un engagement peut dire les siennes à la place, dans « Plus d'options ».",
        comment: [
            "« Copier la base » fait un modèle spécial, qui sait de quelle base il part : si la base change avec une mise à jour, sa ligne le dit.",
            "Chaque article commence par une ligne « ARTICLE: son titre » ; une ligne vide sépare deux alinéas ; un alinéa qui commence par [si un.champ] ne s'écrit que si ce champ a une valeur ; **texte** met en gras ; une ligne qui commence par # est une note, qui ne s'imprime pas.",
            "Les champs entre accolades ({{salarie.nom_complet}}…) se remplissent d'eux-mêmes : leur liste est sous chaque nature. Ceux que la loi exige doivent rester ; sans l'un d'eux, le modèle est refusé, et le refus dit lequel et où.",
        ],
        en_francais: documents::contrat_ecrit::EN_FRANCAIS,
        avenant: format!("Un modèle d'avenant cite aussi : {}.", m.champs_de_l_avenant.join(", ")),
    }
}
