//! LES MODELES DE CONTRAT DE LA COMPAGNIE : ce que Configuration › Modeles de contrat montre et fait, et le modele que
//! chaque contrat suit.
//!
//! Chaque nature de contrat a son MODELE DE BASE, livre (`contrat_ecrit.modele` de `contrats-base.json`, dans
//! `donnees/modeles/`) : il suit les mises a jour du logiciel et ne se modifie pas. La compagnie en copie des MODELES
//! SPECIAUX, chacun un nom et le texte de la base dont il part (le logiciel dit ce qui a change dans la base depuis) ;
//! ils sont ranges avec elle (`gabarits/contrats/modeles.json`, la forme `ModelesDeContrat`) et sauvegardes avec elle.
//! Elle choisit le modele par defaut de chaque nature (la base sinon) ; un engagement en choisit un sous « Avance » (sa
//! case `modele_contrat`, le defaut sinon).
//!
//! UN SPECIAL S'ECRIT PARAGRAPHE PAR PARAGRAPHE (ses `ARTICLE:`) : une route par geste (ecrire, ajouter, deplacer,
//! supprimer un paragraphe ; renommer le modele). Chaque geste se fait sur le texte, qui se verifie ensuite
//! (`documents::contrat_ecrit::manques`) : un geste qui laisserait le modele faux est refuse, rien n'est ecrit, et la
//! phrase dit quelle donnee manque (le paragraphe d'une mention exigee ne se supprime pas tant qu'elle n'est pas
//! ailleurs).
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
use socle::date::Jour;
use socle::echec::{erreur, Classe, Echec, R};
use socle::ecran::Pastille;
use socle::fichiers::dossier_du_code;
use socle::mois::Mois;

const NATURE: Champ = Champ { nom: "nature", libelle: Affiche::new("Nature du contrat"), forme: Forme::Texte, sens: "la nature du contrat : cddu_spectacle, cdd, cdi…", avance: false };
const ID: Champ = Champ { nom: "id", libelle: Affiche::new("Modèle"), forme: Forme::Texte, sens: "l'id d'un modèle spécial (`modeles` de modeles-contrat)", avance: false };
const ID_OU_BASE: Champ = Champ { nom: "id", libelle: Affiche::new("Modèle"), forme: Forme::Texte, sens: "l'id d'un modèle spécial ; absent ou null : le modèle de base de la nature", avance: false };
const NOM: Champ = Champ { nom: "nom", libelle: Affiche::new("Nom"), forme: Forme::Texte, sens: "le nom du modèle spécial", avance: false };
const NUMERO: Champ = Champ { nom: "numero", libelle: Affiche::new("Paragraphe"), forme: Forme::Nombre, sens: "le numéro du paragraphe (`articles[].numero` de modele-contrat), à partir de 1", avance: false };
const TITRE: Champ = Champ { nom: "titre", libelle: Affiche::new("Titre"), forme: Forme::Texte, sens: "le titre du paragraphe", avance: false };
const TEXTE: Champ = Champ { nom: "texte", libelle: Affiche::new("Modèle"), forme: Forme::Texte, sens: "le modèle d'avenant entier (TITRE:, MODIFICATION:, ARTICLE:…)", avance: false };
const MODELE_REFUSE: Refus = Refus { classe: Classe::Refuse, corriger: "le modèle : ce que la phrase nomme, paragraphe par paragraphe ; rien n'est écrit" };

/// Ce que la ligne d'un modele special dit quand la base dont il est copie a change depuis (une mise a jour).
const BASE_CHANGEE: &str = "Le modèle de base a changé depuis la copie : ouvrir ce modèle pour voir quels paragraphes.";

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::ModelesContrat);

/// Une fiche de ce module qui ecrit un modele special et rend la liste, comme modeles-contrat.
const fn geste(route: Route, geste: &'static str, charge: &'static [Champ], exemple: &'static str) -> Fiche {
    Fiche { route, geste, charge, exemple, rend: "les modèles, comme modeles-contrat", refus: &[MODELE_REFUSE, MANQUE], acte: Acte::Ecrit, avance: true, phrase: 0, ecran: ECRAN }
}

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::ModelesContrat,
        geste: "Lister les modèles de contrat de chaque nature (la base et les spéciaux), et les modèles d'avenant.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "natures (chacune : ses modèles, la base d'abord, le défaut marqué, ce qui manque à chacun ; son avenant), clauses de la compagnie",
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
        rend: "articles (numero, titre, alinéas et leur condition), manques, en_francais, champs (ceux qu'un alinéa insère ou pose en condition, avec leur libellé), base_changee (ce qui a changé dans la base depuis la copie), engagements (ceux de l'aperçu, le plus récent d'abord)",
        refus: &[OPERATION, MANQUE],
        acte: Acte::Lit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::ApercuModeleContrat,
        geste: "Faire le PDF d'un modèle de contrat rempli avec un engagement de la compagnie.",
        charge: &[
            NATURE,
            ID_OU_BASE,
            Champ { nom: "engagement", libelle: Affiche::new("Engagement"), forme: Forme::Objet, sens: "{mois, salarie, debut}, un des `engagements` de modele-contrat ; absent ou null : le plus récent", avance: false },
        ],
        exemple: r#"{"nature":"cddu_spectacle","id":"m1","engagement":null}"#,
        rend: "phrase, chemin (le PDF de l'aperçu)",
        refus: &[MODELE_REFUSE, MANQUE],
        acte: Acte::Lit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    geste(Route::CopierModeleContrat, "Copier le modèle de base d'une nature en un modèle spécial de la compagnie.", &[NATURE, NOM], r#"{"nature":"cddu_spectacle","nom":"Tournée"}"#),
    geste(Route::RenommerModeleContrat, "Renommer un modèle spécial.", &[ID, NOM], r#"{"id":"m1","nom":"Tournée d'été"}"#),
    geste(
        Route::EcrireParagrapheContrat,
        "Écrire un paragraphe d'un modèle spécial : son titre, ses alinéas et leur condition ; le modèle est vérifié d'abord.",
        &[
            ID,
            NUMERO,
            TITRE,
            Champ { nom: "alineas", libelle: Affiche::new("Alinéas"), forme: Forme::Objet, sens: "[{si, texte}, …] dans l'ordre : texte, l'alinéa (une donnée s'y insère par {{son.champ}}, un des `champs` de modele-contrat) ; si, le champ qui doit avoir une valeur pour qu'il s'écrive, null : toujours", avance: false },
        ],
        r#"{"id":"m1","numero":3,"titre":"Lieu de travail","alineas":[{"si":null,"texte":"Le travail a lieu à {{contrat.lieu}}."}]}"#,
    ),
    geste(
        Route::AjouterParagrapheContrat,
        "Ajouter un paragraphe à un modèle spécial, à une place.",
        &[ID, Champ { nom: "numero", libelle: Affiche::new("Place"), forme: Forme::Nombre, sens: "sa place : 1 en tête, le nombre de paragraphes plus 1 à la fin", avance: false }, TITRE],
        r#"{"id":"m1","numero":4,"titre":"Déplacements"}"#,
    ),
    geste(
        Route::DeplacerParagrapheContrat,
        "Monter ou descendre un paragraphe d'un modèle spécial.",
        &[ID, NUMERO, Champ { nom: "sens", libelle: Affiche::new("Sens"), forme: Forme::Texte, sens: "« monter » ou « descendre »", avance: false }],
        r#"{"id":"m1","numero":4,"sens":"monter"}"#,
    ),
    geste(Route::SupprimerParagrapheContrat, "Supprimer un paragraphe d'un modèle spécial ; celui d'une mention exigée ne se supprime pas tant qu'elle n'est pas ailleurs.", &[ID, NUMERO], r#"{"id":"m1","numero":4}"#),
    Fiche {
        route: Route::SupprimerModeleContrat,
        geste: "Supprimer un modèle spécial ; les engagements qui l'avaient choisi suivent le défaut de leur nature, un contrat déjà parti garde son texte.",
        charge: &[ID],
        exemple: r#"{"id":"m1"}"#,
        rend: "les modèles, comme modeles-contrat ; la phrase nomme les engagements qui l'avaient choisi",
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
            en_json(Modeles { ecran: Some(ecran_des_modeles()), ..modeles(ctx, "")? })
        }
        Route::ModeleContrat => en_json(lire(ctx, charge.lire()?)?),
        Route::ApercuModeleContrat => en_json(apercu(ctx, charge.lire()?)?),
        Route::CopierModeleContrat => en_json(copier(ctx, charge.lire()?)?),
        Route::RenommerModeleContrat => en_json(renommer(ctx, charge.lire()?)?),
        Route::EcrireParagrapheContrat => en_json(ecrire_paragraphe(ctx, charge.lire()?)?),
        Route::AjouterParagrapheContrat => en_json(ajouter_paragraphe(ctx, charge.lire()?)?),
        Route::DeplacerParagrapheContrat => en_json(deplacer_paragraphe(ctx, charge.lire()?)?),
        Route::SupprimerParagrapheContrat => en_json(supprimer_paragraphe(ctx, charge.lire()?)?),
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
/// elle les a faits ; le modele par defaut de chaque nature qui n'est pas sa base (nature -> id d'un special) ; les ids
/// des speciaux qu'elle a supprimes : un id ne sert qu'une fois, et l'engagement qui en nomme un suit le defaut de sa
/// nature (`du_contrat`).
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelesDeContrat {
    speciaux: Vec<Special>,
    defauts: BTreeMap<String, String>,
    supprimes: Vec<String>,
}

/// UN MODELE SPECIAL : son id (celui que l'engagement et le defaut nomment), son nom, sa nature, le texte de la base dont
/// il a ete copie (ce qui a change dans la base depuis se dit en le comparant a celle du jour), et son texte.
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

fn aucun(id: &str) -> Echec {
    refus(format!("Aucun modèle de contrat {} : en choisir un de la liste (modeles-contrat).", id))
}

fn special<'a>(m: &'a ModelesDeContrat, id: &str) -> R<&'a Special> {
    m.speciaux.iter().find(|s| s.id == id).ok_or_else(|| aucun(id))
}

/// Le modele que nomme `{nature, id}` : son nom, s'il est la base, son texte ; et le special lui-meme.
fn designe<'a>(m: &'a ModelesDeContrat, nat: &Nature, id: Option<&str>) -> R<(String, Option<&'a Special>, String)> {
    match id {
        Some(id) => {
            let s = special(m, id)?;
            if s.nature != nat.cle {
                return Err(refus(format!("{} n'est pas un modèle de {}. Rien n'est écrit.", nom_special(s), nat.nom_du_contrat())));
            }
            Ok((nom_special(s), Some(s), s.texte.clone()))
        }
        None => Ok((nom_de_base(nat), None, texte_de_base(nat)?)),
    }
}

// ------------------------------------------------------------------ le modele d'un contrat

/// LE MODELE QUE SUIT CE CONTRAT, avant son depart : celui que son engagement choisit (`modele_contrat`), sinon le
/// defaut que la compagnie a choisi pour sa nature, sinon la base de sa nature. Un choix qui nomme un special supprime
/// (`supprimes`) revient au defaut : c'est ce que la suppression dit.
pub(crate) fn du_contrat(ctx: &Contexte, p: &Paie) -> R<ModeleDuContrat> {
    let nat = p.engagement.nat().map_err(Echec::from)?;
    let m = lus(ctx)?;
    let choix = p.engagement.ecrit.modele_contrat.as_deref().filter(|id| !m.supprimes.iter().any(|x| x == id));
    let choisi = match choix {
        Some(id) => Some(m.speciaux.iter().find(|s| s.id == id).ok_or_else(|| {
            refus(format!("Le contrat de {} suit le modèle {}, que la compagnie n'a pas : en choisir un autre dans son engagement (Avancé › Modèle de contrat). Rien n'est écrit.", p.salarie.id, id))
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

// ------------------------------------------------------------------ les paragraphes d'un special

/// UN MODELE DECOUPE EN PARAGRAPHES, ligne a ligne et sans rien perdre : ce qui precede le premier `ARTICLE:` (l'en-tete,
/// le preambule, les notes), chaque paragraphe (sa ligne `ARTICLE:` et les siennes), ce qui suit le dernier (les
/// signatures). Recolle, il redonne le texte.
struct Decoupe {
    tete: Vec<String>,
    articles: Vec<Vec<String>>,
    queue: Vec<String>,
}

impl Decoupe {
    fn de(texte: &str) -> R<Decoupe> {
        let mut d = Decoupe { tete: Vec::new(), articles: Vec::new(), queue: Vec::new() };
        for l in texte.split('\n') {
            let cle = fabrique_pdf::modele::CLES.iter().find(|k| l.starts_with(**k)).copied();
            match cle {
                Some("ARTICLE:") if d.queue.is_empty() => d.articles.push(vec![l.to_string()]),
                Some("ARTICLE:") => return Err(refus("Ce modèle a un paragraphe après ses signatures : il ne se découpe pas en paragraphes. Rien n'est écrit.")),
                _ if d.articles.is_empty() => d.tete.push(l.to_string()),
                Some(_) => d.queue.push(l.to_string()),
                None if d.queue.is_empty() => d.articles.last_mut().into_iter().for_each(|a| a.push(l.to_string())),
                None => d.queue.push(l.to_string()),
            }
        }
        Ok(d)
    }

    fn texte(&self) -> String {
        self.tete.iter().chain(self.articles.iter().flatten()).chain(self.queue.iter()).cloned().collect::<Vec<_>>().join("\n")
    }

    /// L'indice du paragraphe `numero` (a partir de 1).
    fn indice(&self, numero: usize) -> R<usize> {
        (1..=self.articles.len()).contains(&numero).then(|| numero - 1).ok_or_else(|| refus(format!("Ce modèle n'a pas de paragraphe {} : il en a {}. Rien n'est écrit.", numero, self.articles.len())))
    }

    /// Le titre du paragraphe d'indice `i`.
    fn titre(&self, i: usize) -> String {
        self.articles[i].first().map(|l| l["ARTICLE:".len()..].trim().to_string()).unwrap_or_default()
    }
}

/// Un alinea tel qu'un geste l'ecrit : sa condition (le champ qui doit avoir une valeur ; `None` : toujours) et son
/// texte.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AlineaEcrit {
    si: Option<String>,
    texte: String,
}

/// Un texte sur une ligne : un paragraphe du modele ne garde que ses mots.
fn une_ligne(t: &str) -> String {
    t.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// LES LIGNES D'UN PARAGRAPHE : `ARTICLE: titre`, puis chaque alinea (precede de `[si champ]` s'il en a une), une ligne
/// vide apres chacun. Un alinea vide ne s'ecrit pas ; un alinea qui commencerait par ce que la grammaire lit autrement
/// (une note, une condition, une cle) se refuse.
fn paragraphe(titre: &str, alineas: &[AlineaEcrit]) -> R<Vec<String>> {
    let titre = une_ligne(titre);
    if titre.is_empty() {
        return Err(erreur(Classe::Manque, "Le paragraphe a besoin d'un titre. Rien n'est écrit."));
    }
    let mut lignes = vec![format!("ARTICLE: {}", titre), String::new()];
    for (n, a) in alineas.iter().enumerate() {
        let t = une_ligne(&a.texte);
        if t.is_empty() {
            continue;
        }
        if t.starts_with('#') || t.starts_with('[') || fabrique_pdf::modele::CLES.iter().any(|k| t.starts_with(k)) {
            return Err(refus(format!("L'alinéa {} de « {} » commence par un signe que le modèle lit autrement (#, [ ou une clé comme ARTICLE:) : le reformuler ; sa condition se choisit à côté. Rien n'est écrit.", n + 1, titre)));
        }
        lignes.push(match a.si.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            Some(si) => format!("[si {}] {}", si, t),
            None => t,
        });
        lignes.push(String::new());
    }
    Ok(lignes)
}

/// UN GESTE SUR LES PARAGRAPHES D'UN SPECIAL : `faire` change sa decoupe et rend ce que la phrase dit du geste fait. Le
/// texte qui en sort se verifie (`documents::contrat_ecrit::manques`) : s'il lui manque quelque chose, rien n'est ecrit,
/// et le refus dit quoi.
fn sur_les_paragraphes(ctx: &Contexte, id: &str, faire: impl FnOnce(&mut Decoupe) -> R<String>) -> R<Modeles> {
    let mut m = lus(ctx)?;
    let i = m.speciaux.iter().position(|s| s.id == id).ok_or_else(|| aucun(id))?;
    let nat = nature_de(&m.speciaux[i].nature)?;
    let mut d = Decoupe::de(&m.speciaux[i].texte)?;
    let fait = faire(&mut d)?;
    let texte = d.texte();
    let dits = manques(&nat, &texte)?;
    if !dits.is_empty() {
        return Err(refus(format!("{} ne peut pas faire ce geste : il en sortirait faux. {} Rien n'est écrit.", nom_special(&m.speciaux[i]), dits.join(" "))));
    }
    m.speciaux[i].texte = texte;
    let devant = format!("{} : {} ", m.speciaux[i].nom, fait);
    ecrire(ctx, &m)?;
    modeles(ctx, &devant)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeEcrireParagraphe {
    id: String,
    numero: usize,
    titre: String,
    alineas: Vec<AlineaEcrit>,
}

/// ECRIRE UN PARAGRAPHE : `{id, numero, titre, alineas}` remplace le paragraphe `numero` (sa condition comprise).
fn ecrire_paragraphe(ctx: &Contexte, c: ChargeEcrireParagraphe) -> R<Modeles> {
    sur_les_paragraphes(ctx, &c.id, |d| {
        let i = d.indice(c.numero)?;
        d.articles[i] = paragraphe(&c.titre, &c.alineas)?;
        Ok(format!("paragraphe {} « {} » écrit.", c.numero, d.titre(i)))
    })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeAjouter {
    id: String,
    numero: usize,
    titre: String,
}

/// AJOUTER UN PARAGRAPHE : `{id, numero, titre}`, un paragraphe sans alinea a la place `numero` (1 en tete, n + 1 a la
/// fin) ; ses alineas s'ecrivent ensuite.
fn ajouter_paragraphe(ctx: &Contexte, c: ChargeAjouter) -> R<Modeles> {
    sur_les_paragraphes(ctx, &c.id, |d| {
        let n = d.articles.len();
        if !(1..=n + 1).contains(&c.numero) {
            return Err(refus(format!("Un paragraphe s'ajoute à une place de 1 à {}. Rien n'est écrit.", n + 1)));
        }
        d.articles.insert(c.numero - 1, paragraphe(&c.titre, &[])?);
        Ok(format!("paragraphe « {} » ajouté à la place {}.", d.titre(c.numero - 1), c.numero))
    })
}

/// Le sens d'un deplacement.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Sens {
    Monter,
    Descendre,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeDeplacer {
    id: String,
    numero: usize,
    sens: Sens,
}

/// DEPLACER UN PARAGRAPHE : `{id, numero, sens}`, d'une place vers le haut ou vers le bas.
fn deplacer_paragraphe(ctx: &Contexte, c: ChargeDeplacer) -> R<Modeles> {
    sur_les_paragraphes(ctx, &c.id, |d| {
        let i = d.indice(c.numero)?;
        let j = match c.sens {
            Sens::Monter if i == 0 => return Err(refus("Ce paragraphe est déjà le premier. Rien n'est écrit.")),
            Sens::Descendre if i + 1 == d.articles.len() => return Err(refus("Ce paragraphe est déjà le dernier. Rien n'est écrit.")),
            Sens::Monter => i - 1,
            Sens::Descendre => i + 1,
        };
        d.articles.swap(i, j);
        Ok(format!("paragraphe « {} » à la place {}.", d.titre(j), j + 1))
    })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeParagraphe {
    id: String,
    numero: usize,
}

/// SUPPRIMER UN PARAGRAPHE : `{id, numero}`. Celui qui porte une mention exigee ne se supprime pas tant qu'elle n'est
/// pas ailleurs : le verificateur le refuse, et dit laquelle.
fn supprimer_paragraphe(ctx: &Contexte, c: ChargeParagraphe) -> R<Modeles> {
    sur_les_paragraphes(ctx, &c.id, |d| {
        let i = d.indice(c.numero)?;
        let titre = d.titre(i);
        d.articles.remove(i);
        Ok(format!("paragraphe {} « {} » supprimé.", c.numero, titre))
    })
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
struct ChargeRenommer {
    id: String,
    nom: String,
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

/// L'engagement d'un apercu : le mois de sa carte, la cle de son salarie, son debut.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EngagementVise {
    mois: Mois,
    salarie: String,
    debut: Jour,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeApercu {
    nature: String,
    id: Option<String>,
    engagement: Option<EngagementVise>,
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

/// Une nature, ses modeles de contrat et son avenant.
#[derive(Debug, Serialize)]
struct NatureDite {
    nature: String,
    libelle: String,
    nom: String,
    modeles: Vec<ModeleDit>,
    avenant: AvenantDit,
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

/// Un engagement que l'apercu peut remplir : ce qui le designe (`engagement` de apercu-modele-contrat) et son nom.
#[derive(Debug, Serialize)]
struct EngagementAuChoix {
    engagement: EngagementVise,
    nom: String,
}

/// Ce que rend `modele-contrat`.
#[derive(Debug, Serialize)]
struct ModeleRendu {
    phrase: String,
    nature: String,
    id: Option<String>,
    nom: String,
    base: bool,
    articles: Vec<ArticleRendu>,
    manques: Vec<String>,
    en_francais: &'static str,
    champs: Vec<ChampDeModele>,
    base_changee: Vec<String>,
    engagements: Vec<EngagementAuChoix>,
}

/// Ce que rend `apercu-modele-contrat`.
#[derive(Debug, Serialize)]
struct Apercu {
    phrase: String,
    chemin: String,
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

impl Dite for Apercu {
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
/// ce qui manque a chacun, si sa base a change depuis la copie) et son modele d'avenant ; et les clauses particulieres
/// de la compagnie. `devant` : ce que la phrase dit avant (le geste qui vient d'etre fait).
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
            pieces.push(ModeleDit { id: Some(s.id.clone()), nom: s.nom.clone(), base: false, defaut: est_defaut, base_changee: (s.base != base).then_some(BASE_CHANGEE), pastille: pastille(&dits, est_defaut, false), manques: dits });
        }
        sortie.push(NatureDite { nature: nat.cle.clone(), libelle: nat.libelle.clone(), nom: nat.nom_du_contrat().to_string(), modeles: pieces, avenant: avenant_dit(ctx, &nat)? });
    }
    Ok(Modeles {
        phrase: format!("{}Modèles de contrat spéciaux de la compagnie : {}.", devant, m.speciaux.len()),
        natures: sortie,
        clauses_particulieres: ctx.entreprise.clauses_particulieres.portee().map(String::from),
        ecran: None,
    })
}

/// CE QUI A CHANGE DANS LA BASE depuis la copie d'un special (`avant` : la base copiee, `maintenant` : celle du jour),
/// paragraphe par paragraphe (par leur titre), sans rien fusionner : la compagnie reprend ce qu'elle veut.
fn ce_qui_a_change(avant: &str, maintenant: &str) -> Vec<String> {
    if avant == maintenant {
        return Vec::new();
    }
    let (Ok(a), Ok(b)) = (fabrique_pdf::modele::lire_modele(avant, "la base copiée"), fabrique_pdf::modele::lire_modele(maintenant, "la base")) else {
        return vec![BASE_CHANGEE.to_string()];
    };
    let mut dits = Vec::new();
    if (&a.titre, &a.soustitre, &a.preambule) != (&b.titre, &b.soustitre, &b.preambule) {
        dits.push("Dans la base, l'en-tête (titre, préambule) a changé.".to_string());
    }
    for (titre, alineas) in &b.articles {
        match a.articles.iter().find(|(t, _)| t == titre) {
            None => dits.push(format!("Dans la base, le paragraphe « {} » est nouveau.", titre)),
            Some((_, avant)) if avant != alineas => dits.push(format!("Dans la base, le paragraphe « {} » a changé.", titre)),
            Some(_) => {}
        }
    }
    for (titre, _) in a.articles.iter().filter(|(t, _)| !b.articles.iter().any(|(x, _)| x == t)) {
        dits.push(format!("La base n'a plus le paragraphe « {} ».", titre));
    }
    if (&a.signatures, &a.annule) != (&b.signatures, &b.annule) {
        dits.push("Dans la base, les signatures ou la phrase d'annulation ont changé.".to_string());
    }
    if dits.is_empty() {
        dits.push("Dans la base, seules les notes ont changé.".to_string());
    }
    dits
}

/// LES ENGAGEMENTS QUE L'APERCU D'UN MODELE DE CETTE NATURE PEUT REMPLIR : ceux de la compagnie, valides, actifs, saisis
/// dans le logiciel, de cette nature, le plus recent d'abord.
fn engagements_de(ctx: &Contexte, nature: &str) -> R<Vec<EngagementAuChoix>> {
    let mut v: Vec<EngagementAuChoix> = Vec::new();
    for o in engagement::stock::objets(&ctx.racine).map_err(Echec::from)? {
        if !o.est_valide() || o.est_importe() || o.est_non_actif() || o.carte().nature.as_deref() != Some(nature) {
            continue;
        }
        let (Some(salarie), Some(debut)) = (o.carte().salarie.as_ref(), Jour::du_texte(&engagement::debut_brut(&o.brut))) else { continue };
        let mois = Mois::lire(o.premier_mois()).map_err(Echec::from)?;
        v.push(EngagementAuChoix { engagement: EngagementVise { mois, salarie: salarie.to_string(), debut }, nom: engagement::etats::qui_de(ctx, &o) });
    }
    v.sort_by(|a, b| b.engagement.debut.cmp(&a.engagement.debut));
    Ok(v)
}

/// LIRE UN MODELE : `{nature, id}` (sans id : la base), ses articles et leurs alineas, ce qui lui manque, les champs
/// qu'un alinea insere ou pose en condition, ce qui a change dans sa base, les engagements de l'apercu.
fn lire(ctx: &Contexte, c: ChargeLire) -> R<ModeleRendu> {
    let nat = nature_de(&c.nature)?;
    let m = lus(ctx)?;
    let (nom, s, texte) = designe(&m, &nat, c.id.as_deref())?;
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
    let base_changee = match s {
        Some(s) => ce_qui_a_change(&s.base, &texte_de_base(&nat)?),
        None => Vec::new(),
    };
    let champs = documents::contrat_ecrit::champs_des_modeles(&nat).map_err(|m| erreur(Classe::Illisible, m))?;
    // Le nom qu'un special porte (celui que l'on renomme) ; la base, comme la liste la nomme.
    let nom = s.map_or(nom, |s| s.nom.clone());
    Ok(ModeleRendu { phrase, nature: nat.cle.clone(), id: c.id, nom, base: s.is_none(), articles, manques: dits, en_francais: documents::contrat_ecrit::EN_FRANCAIS, champs, base_changee, engagements: engagements_de(ctx, &nat.cle)? })
}

/// L'APERCU D'UN MODELE : `{nature, id, engagement}`, le PDF du contrat de cet engagement (le plus recent sans lui) fait
/// avec ce modele, dans `sorties/apercus/` ; rien d'autre ne s'ecrit, rien ne part. Un modele qui ne fait pas ce
/// contrat se refuse comme au depart, et dit pourquoi.
fn apercu(ctx: &Contexte, c: ChargeApercu) -> R<Apercu> {
    let nat = nature_de(&c.nature)?;
    let m = lus(ctx)?;
    let (nom, s, texte) = designe(&m, &nat, c.id.as_deref())?;
    let vise = match c.engagement {
        Some(v) => v,
        None => engagements_de(ctx, &nat.cle)?
            .into_iter()
            .next()
            .map(|e| e.engagement)
            .ok_or_else(|| refus(format!("La compagnie n'a encore aucun engagement validé de {} : l'aperçu remplit le modèle avec l'un d'eux.", nat.nom_du_contrat())))?,
    };
    let mois = vise.mois.to_string();
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs).map_err(Echec::from)?;
    let (_, paies) = documents::ecrire_documents::paies_aux_numeros_declares(ctx, &mois, None, &departs)?;
    let p = paies
        .into_iter()
        .find(|p| p.salarie.id.to_string() == vise.salarie && p.engagement.debut == vise.debut)
        .ok_or_else(|| refus(format!("Aucun engagement validé de {} du {} en {} : en choisir un de la liste (`engagements` de modele-contrat).", vise.salarie, vise.debut.jj_mm_aaaa(), vise.mois.etiquette())))?;
    let modele = modele_du_texte(nom.clone(), s.is_none(), &texte)?;
    let ent = documents::avenant::ent_du_mois(ctx, &mois)?;
    let d = ctx.racine.join("sorties").join("apercus");
    std::fs::create_dir_all(&d).map_err(|e| disque(&d, e))?;
    let chemin = d.join(format!("modele-{}-{}.pdf", nat.cle, c.id.as_deref().unwrap_or("base")));
    documents::contrat::ecrire_contrat(ctx, std::slice::from_ref(&p), &[modele], &ent, &chemin, &[])?;
    Ok(Apercu { phrase: format!("{}, rempli avec le contrat de {} du {} : l'aperçu est fait, rien n'est parti.", nom, p.engagement.nom, p.engagement.debut.jj_mm_aaaa()), chemin: chemin.to_string_lossy().into_owned() })
}

/// COPIER LA BASE D'UNE NATURE EN UN MODELE SPECIAL : `{nature, nom}` ; il garde le texte de la base dont il part.
fn copier(ctx: &Contexte, c: ChargeCopier) -> R<Modeles> {
    let nat = nature_de(&c.nature)?;
    let mut m = lus(ctx)?;
    let nom = nom_libre(&m, None, &c.nom)?;
    let texte = texte_de_base(&nat)?;
    // Un id ne sert qu'une fois : ni un special present, ni un supprime.
    let n = m.speciaux.iter().map(|s| &s.id).chain(m.supprimes.iter()).filter_map(|id| id.strip_prefix('m')?.parse::<u32>().ok()).max().map_or(1, |n| n + 1);
    m.speciaux.push(Special { id: format!("m{}", n), nom: nom.clone(), nature: nat.cle.clone(), base: texte.clone(), texte });
    ecrire(ctx, &m)?;
    modeles(ctx, &format!("Modèle « {} » copié de la base du {}. ", nom, nat.nom_du_contrat()))
}

/// Le nom d'un special : dit, et qu'aucun autre special (que `sauf`) ne porte.
fn nom_libre(m: &ModelesDeContrat, sauf: Option<&str>, nom: &str) -> R<String> {
    let nom = une_ligne(nom);
    if nom.is_empty() {
        return Err(erreur(Classe::Manque, "Le modèle spécial a besoin d'un nom. Rien n'est écrit."));
    }
    if m.speciaux.iter().any(|s| s.nom == nom && Some(s.id.as_str()) != sauf) {
        return Err(refus(format!("La compagnie a déjà un modèle « {} » : lui donner un autre nom. Rien n'est écrit.", nom)));
    }
    Ok(nom)
}

/// RENOMMER UN MODELE SPECIAL : `{id, nom}`.
fn renommer(ctx: &Contexte, c: ChargeRenommer) -> R<Modeles> {
    let mut m = lus(ctx)?;
    let nom = nom_libre(&m, Some(&c.id), &c.nom)?;
    let i = m.speciaux.iter().position(|s| s.id == c.id).ok_or_else(|| aucun(&c.id))?;
    let avant = std::mem::replace(&mut m.speciaux[i].nom, nom.clone());
    ecrire(ctx, &m)?;
    modeles(ctx, &format!("Modèle « {} » renommé « {} ». ", avant, nom))
}

/// Les engagements dont une carte choisit le special `id` (sa case `modele_contrat`, au premier mois ou a un avenant).
fn choisi_par(ctx: &Contexte, id: &str) -> R<Vec<String>> {
    Ok(engagement::stock::objets(&ctx.racine)
        .map_err(Echec::from)?
        .iter()
        .filter(|o| o.carte().modele_contrat.as_deref() == Some(id) || o.avenants().iter().any(|a| a.termes.modele_contrat.as_deref() == Some(id)))
        .map(|o| engagement::etats::qui_de(ctx, o))
        .collect())
}

/// SUPPRIMER UN MODELE SPECIAL : `{id}` ; s'il etait le defaut de sa nature, la base le redevient ; les engagements qui
/// l'avaient choisi suivent le defaut de leur nature (`supprimes`, `du_contrat`), et la phrase les nomme. Un contrat
/// deja parti garde son texte (`figer`).
fn supprimer(ctx: &Contexte, c: ChargeId) -> R<Modeles> {
    let mut m = lus(ctx)?;
    let nom = special(&m, &c.id)?.nom.clone();
    let qui = choisi_par(ctx, &c.id)?;
    m.speciaux.retain(|s| s.id != c.id);
    m.defauts.retain(|_, id| *id != c.id);
    m.supprimes.push(c.id);
    ecrire(ctx, &m)?;
    let suite = match qui.len() {
        0 => String::new(),
        n => format!("{} engagement(s) l'avaient choisi ({}) : leur contrat suit désormais le modèle par défaut de sa nature ; un contrat déjà parti garde son texte. ", n, qui.join(", ")),
    };
    modeles(ctx, &format!("Modèle « {} » supprimé. {}", nom, suite))
}

/// CHOISIR LE MODELE PAR DEFAUT D'UNE NATURE : `{nature, id}` ; sans id, la base.
fn par_defaut(ctx: &Contexte, c: ChargeLire) -> R<Modeles> {
    let nat = nature_de(&c.nature)?;
    let mut m = lus(ctx)?;
    let devant = match &c.id {
        Some(id) => {
            let (nom, _, _) = designe(&m, &nat, Some(id))?;
            m.defauts.insert(nat.cle.clone(), id.clone());
            format!("{} est le modèle par défaut du {}. ", nom, nat.nom_du_contrat())
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
fn ecran_des_modeles() -> EcranDesModeles {
    EcranDesModeles {
        intro: "Chaque contrat que le logiciel fait signer suit un modèle. Le modèle de base de chaque sorte de contrat porte déjà ce que la loi exige, rempli de lui-même depuis l'engagement et la configuration ; il suit les mises à jour et ne se modifie pas. La compagnie le copie en modèles spéciaux, choisit celui par défaut, et un engagement peut en choisir un autre (Avancé). Un contrat parti garde son texte.",
        clauses: "Ce qu'elle ajoute à chacun de ses contrats (confidentialité, image, déplacements…). Un engagement peut dire les siennes à la place, dans « Plus d'options ».",
        comment: [
            "« Copier la base » fait un modèle spécial, qui garde la base dont il part : si elle change avec une mise à jour, le modèle dit quels paragraphes ont changé.",
            "Un modèle spécial s'écrit paragraphe par paragraphe : un titre, des alinéas. Une donnée s'insère par la liste et se remplit d'elle-même ; un alinéa peut ne s'écrire que si une donnée est remplie ; **texte** met en gras.",
            "Ce que la loi exige doit rester : un geste qui l'ôterait est refusé, et le refus dit quelle donnée manque. L'aperçu montre le contrat rempli avec un engagement de la compagnie.",
        ],
        en_francais: documents::contrat_ecrit::EN_FRANCAIS,
        avenant: format!("Un modèle d'avenant cite les champs du contrat et : {}.", DE_L_AVENANT.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(", ")),
    }
}
