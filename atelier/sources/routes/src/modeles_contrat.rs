//! LES MODELES DE CONTRAT DE LA COMPAGNIE : ce que Configuration › Modeles de contrat montre et fait, et le modele que
//! chaque contrat suit.
//!
//! Chaque nature de contrat a son MODELE DE BASE, livre (`contrat_ecrit` de `contrats-base.json`, dans
//! `donnees/modeles/`) : un contrat et son avenant, qui suivent les mises a jour du logiciel et ne se modifient pas. La
//! compagnie en copie des MODELES SPECIAUX : chacun un nom, son contrat et son avenant, et pour chacun le texte de la
//! base dont il part (le logiciel dit ce qui a change dans la base depuis) ; ils sont ranges avec elle
//! (`gabarits/contrats/modeles.json`, la forme `ModelesDeContrat`). Elle choisit le modele par defaut de chaque nature
//! (la base sinon) ; un engagement en choisit un sous « Avance » (sa case `modele_contrat`, le defaut sinon).
//!
//! UN SPECIAL S'ECRIT PARAGRAPHE PAR PARAGRAPHE (les `ARTICLE:` de son contrat ou de son avenant, sa `partie`) : une
//! route par geste (ecrire, ajouter, deplacer, supprimer un paragraphe ; renommer le modele). Chaque geste se fait sur le
//! texte, qui se verifie ensuite (`documents::contrat_ecrit::manques`, `manques_de_l_avenant`) : un geste qui laisserait
//! le modele faux est refuse, rien n'est ecrit, et la phrase dit quelle donnee manque.
//!
//! LE CONTRAT PARTI GARDE SON TEXTE, et celui de son avenant : au depart, les deux se figent sous une empreinte
//! (`gabarits/contrats/partis/<empreinte>.modele` et `.avenant.modele`, `figer`), que la ligne du journal note
//! (`ContratParti::modele`) ; refaire ce contrat, ou lui faire un avenant, relit ces textes-la (`du_contrat_parti`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::catalogue::Route;
use crate::catalogue::{Acte, Affiche, Champ, Ecran, Fiche, Forme, Place, Refus, MANQUE, OPERATION};
use crate::enveloppe::{en_json, Charge, Dite, Porte, Rendu};
use documents::contrat_ecrit::{ChampDeModele, ModeleDuContrat};
use documents::marqueurs::ModeleLu;
use droit::contexte::Contexte;
use droit::natures::Nature;
use engagement::calcul::Paie;
use socle::date::Jour;
use socle::echec::{erreur, Classe, Echec, R};
use socle::ecran::Pastille;
use socle::mois::Mois;

const fn champ(nom: &'static str, libelle: &'static str, forme: Forme, sens: &'static str) -> Champ {
    Champ { nom, libelle: Affiche::new(libelle), forme, sens, avance: false }
}

const NATURE: Champ = champ("nature", "Nature du contrat", Forme::Texte, "la nature du contrat : cddu_spectacle, cdd, cdi…");
const ID: Champ = champ("id", "Modèle", Forme::Texte, "l'id d'un modèle spécial (`modeles` de modeles-contrat)");
const ID_OU_BASE: Champ = champ("id", "Modèle", Forme::Texte, "l'id d'un modèle spécial ; absent ou null : le modèle de base de la nature");
const PARTIE: Champ = champ("partie", "Partie", Forme::Texte, "« contrat » ou « avenant » : le texte du modèle que le geste lit ou écrit");
const NOM: Champ = champ("nom", "Nom", Forme::Texte, "le nom du modèle spécial");
const NUMERO: Champ = champ("numero", "Paragraphe", Forme::Nombre, "le numéro du paragraphe (`articles[].numero` de modele-contrat), à partir de 1");
const TITRE: Champ = champ("titre", "Titre", Forme::Texte, "le titre du paragraphe");
const MODELE_REFUSE: Refus = Refus { classe: Classe::Refuse, corriger: "le modèle : ce que la phrase nomme, paragraphe par paragraphe ; rien n'est écrit" };

/// Ce que la ligne d'un modele special dit quand une base dont il est copie a change depuis (une mise a jour).
const BASE_CHANGEE: &str = "Le modèle de base a changé depuis la copie : ouvrir ce modèle pour voir quels paragraphes.";

/// Une fiche de ce module (elles se voient toutes sur l'ecran des modeles de contrat, sous « Avance »).
const fn fiche(route: Route, geste: &'static str, charge: &'static [Champ], exemple: &'static str, rend: &'static str, refus: &'static [Refus], acte: Acte) -> Fiche {
    Fiche { route, geste, charge, exemple, rend, refus, acte, avance: true, phrase: 0, ecran: Ecran::Dessine(Place::ModelesContrat) }
}

/// Une fiche d'un geste qui ecrit un modele et rend la liste, comme modeles-contrat.
const fn geste(route: Route, geste: &'static str, charge: &'static [Champ], exemple: &'static str) -> Fiche {
    fiche(route, geste, charge, exemple, "les modèles, comme modeles-contrat", &[MODELE_REFUSE, MANQUE], Acte::Ecrit)
}

/// Les routes de ce module, et leurs fiches, dans l'ordre ou leurs exemples se jouent : la copie fait `m1`, que les
/// gestes suivants ecrivent, et que le dernier supprime.
pub const ROUTES: &[Fiche] = &[
    fiche(
        Route::ModelesContrat,
        "Lister les modèles de contrat de chaque nature : la base et les spéciaux, chacun avec son avenant.",
        &[],
        r#"{}"#,
        "natures (chacune : ses modèles, la base d'abord, le défaut marqué, ce qui manque à chacun, à son contrat ou à son avenant)",
        &[OPERATION],
        Acte::Lit,
    ),
    geste(Route::CopierModeleContrat, "Copier le modèle de base d'une nature, contrat et avenant, en un modèle spécial de la compagnie.", &[NATURE, NOM], r#"{"nature":"cddu_spectacle","nom":"Tournée"}"#),
    fiche(
        Route::ModeleContrat,
        "Lire le contrat ou l'avenant d'un modèle, paragraphe par paragraphe, et ce qui lui manque.",
        &[NATURE, ID_OU_BASE, PARTIE],
        r#"{"nature":"cddu_spectacle","id":"m1","partie":"contrat"}"#,
        "articles (numero, titre, alinéas et leur condition), manques, en_francais, champs (ceux qu'un alinéa insère ou pose en condition, avec leur libellé), base_changee (ce qui a changé dans la base depuis la copie), engagements (ceux de l'aperçu, le plus récent d'abord)",
        &[OPERATION, MANQUE],
        Acte::Lit,
    ),
    geste(Route::RenommerModeleContrat, "Renommer un modèle spécial.", &[ID, NOM], r#"{"id":"m1","nom":"Tournée d'été"}"#),
    geste(
        Route::EcrireParagrapheContrat,
        "Écrire un paragraphe d'un modèle spécial : son titre, ses alinéas et leur condition ; le modèle est vérifié d'abord.",
        &[
            ID,
            PARTIE,
            NUMERO,
            TITRE,
            champ("alineas", "Alinéas", Forme::Objet, "[{si, texte}, …] dans l'ordre : texte, l'alinéa (une donnée s'y insère par {{son.champ}}, un des `champs` de modele-contrat) ; si, le champ qui doit avoir une valeur pour qu'il s'écrive, null : toujours"),
        ],
        r#"{"id":"m1","partie":"contrat","numero":3,"titre":"Lieu de travail","alineas":[{"si":null,"texte":"Le travail a lieu {{contrat.lieu}}."}]}"#,
    ),
    geste(
        Route::AjouterParagrapheContrat,
        "Ajouter un paragraphe à un modèle spécial, à une place.",
        &[ID, PARTIE, champ("numero", "Place", Forme::Nombre, "sa place : 1 en tête, le nombre de paragraphes plus 1 à la fin"), TITRE],
        r#"{"id":"m1","partie":"contrat","numero":4,"titre":"Déplacements"}"#,
    ),
    geste(
        Route::DeplacerParagrapheContrat,
        "Monter ou descendre un paragraphe d'un modèle spécial.",
        &[ID, PARTIE, NUMERO, champ("sens", "Sens", Forme::Texte, "« monter » ou « descendre »")],
        r#"{"id":"m1","partie":"contrat","numero":4,"sens":"monter"}"#,
    ),
    geste(
        Route::SupprimerParagrapheContrat,
        "Supprimer un paragraphe d'un modèle spécial ; celui d'une mention exigée ne se supprime pas tant qu'elle n'est pas ailleurs.",
        &[ID, PARTIE, NUMERO],
        r#"{"id":"m1","partie":"contrat","numero":3}"#,
    ),
    fiche(
        Route::ApercuModeleContrat,
        "Faire le PDF du contrat ou de l'avenant d'un modèle, rempli avec un engagement de la compagnie.",
        &[NATURE, ID_OU_BASE, PARTIE, champ("engagement", "Engagement", Forme::Objet, "{mois, salarie, debut}, un des `engagements` de modele-contrat ; absent ou null : le plus récent")],
        r#"{"nature":"cddu_spectacle","id":"m1","partie":"avenant","engagement":null}"#,
        "phrase, chemin (le PDF de l'aperçu)",
        &[MODELE_REFUSE, MANQUE],
        Acte::Lit,
    ),
    fiche(Route::ModeleContratParDefaut, "Choisir le modèle par défaut d'une nature.", &[NATURE, ID_OU_BASE], r#"{"nature":"cddu_spectacle","id":"m1"}"#, "les modèles, comme modeles-contrat", &[OPERATION, MANQUE], Acte::Ecrit),
    fiche(
        Route::SupprimerModeleContrat,
        "Supprimer un modèle spécial ; les engagements qui l'avaient choisi suivent le défaut de leur nature, un contrat déjà parti garde son texte.",
        &[ID],
        r#"{"id":"m1"}"#,
        "les modèles, comme modeles-contrat ; la phrase nomme les engagements qui l'avaient choisi",
        &[OPERATION, MANQUE],
        Acte::Ecrit,
    ),
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

/// UN MODELE SPECIAL : son id (celui que l'engagement et le defaut nomment), son nom, sa nature, son contrat et son
/// avenant.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Special {
    id: String,
    nom: String,
    nature: String,
    contrat: Texte,
    avenant: Texte,
}

/// Le texte d'une partie d'un special, et celui de la base dont elle a ete copiee (ce qui a change dans la base depuis
/// se dit en le comparant a celle du jour).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Texte {
    base: String,
    texte: String,
}

/// La partie d'un modele qu'un geste lit ou ecrit : le contrat, ou son avenant.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Partie {
    Contrat,
    Avenant,
}

impl Special {
    fn partie(&self, p: Partie) -> &Texte {
        match p {
            Partie::Contrat => &self.contrat,
            Partie::Avenant => &self.avenant,
        }
    }

    fn partie_mut(&mut self, p: Partie) -> &mut Texte {
        match p {
            Partie::Contrat => &mut self.contrat,
            Partie::Avenant => &mut self.avenant,
        }
    }

    fn nom_dit(&self) -> String {
        format!("Le modèle « {} »", self.nom)
    }
}

fn dossier(ctx: &Contexte) -> PathBuf {
    ctx.gabarits().join("contrats")
}

fn disque(chemin: &Path, e: std::io::Error) -> Echec {
    erreur(Classe::Disque, socle::echec::du_disque(&chemin, &e))
}

fn lire_texte(chemin: &Path) -> R<String> {
    std::fs::read_to_string(chemin).map_err(|e| disque(chemin, e))
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

fn refus(m: impl Into<String>) -> Echec {
    erreur(Classe::Refuse, m)
}

fn nature_de(cle: &str) -> R<std::sync::Arc<Nature>> {
    droit::natures::nature(cle).map_err(Echec::from)
}

/// Le texte livre d'une partie du modele de base de cette nature.
fn texte_de_base(nat: &Nature, partie: Partie) -> R<String> {
    let nom = match partie {
        Partie::Contrat => nat.modele_contrat(),
        Partie::Avenant => nat.modele_avenant(),
    };
    let nom = nom.ok_or_else(|| refus(format!("La nature {} n'a pas de contrat écrit.", nat.libelle)))?;
    lire_texte(&socle::fichiers::dossier_du_code().join("modeles").join(nom))
}

fn nom_de_base(nat: &Nature) -> String {
    format!("Le modèle de base du {}", nat.nom_du_contrat())
}

/// Ce qui manque a une partie d'un modele de cette nature (le verificateur de sa partie).
fn manques(nat: &Nature, partie: Partie, texte: &str) -> R<Vec<String>> {
    match partie {
        Partie::Contrat => documents::contrat_ecrit::manques(nat, texte, None).map_err(|m| erreur(Classe::Illisible, m)),
        Partie::Avenant => Ok(documents::contrat_ecrit::manques_de_l_avenant(texte)),
    }
}

/// Un modele lu pour un contrat, et son avenant ; un texte qui ne se lit pas est un refus.
fn modele_lu(nom: String, base: bool, contrat: &str, avenant: &str) -> R<ModeleDuContrat> {
    let lire = |t: &str| ModeleLu::lire(t, &nom).map_err(|m| erreur(Classe::ModeleIncomplet, m));
    Ok(ModeleDuContrat { lu: lire(contrat)?, avenant: lire(avenant)?, nom, base })
}

fn aucun(id: &str) -> Echec {
    refus(format!("Aucun modèle de contrat {} : en choisir un de la liste (modeles-contrat).", id))
}

fn special<'a>(m: &'a ModelesDeContrat, id: &str) -> R<&'a Special> {
    m.speciaux.iter().find(|s| s.id == id).ok_or_else(|| aucun(id))
}

/// Le modele que nomme `{nature, id}` : son nom, et le special (aucun : la base).
fn designe<'a>(m: &'a ModelesDeContrat, nat: &Nature, id: Option<&str>) -> R<(String, Option<&'a Special>)> {
    match id {
        Some(id) => {
            let s = special(m, id)?;
            if s.nature != nat.cle {
                return Err(refus(format!("{} n'est pas un modèle de {}. Rien n'est écrit.", s.nom_dit(), nat.nom_du_contrat())));
            }
            Ok((s.nom_dit(), Some(s)))
        }
        None => Ok((nom_de_base(nat), None)),
    }
}

/// Le texte d'une partie du modele que `designe` rend.
fn texte_de(nat: &Nature, s: Option<&Special>, partie: Partie) -> R<String> {
    match s {
        Some(s) => Ok(s.partie(partie).texte.clone()),
        None => texte_de_base(nat, partie),
    }
}

// ------------------------------------------------------------------ le modele d'un contrat

/// LE MODELE QUE SUIT CE CONTRAT, avant son depart, et son avenant : celui que son engagement choisit
/// (`modele_contrat`), sinon le defaut que la compagnie a choisi pour sa nature, sinon la base de sa nature. Un choix qui
/// nomme un special supprime (`supprimes`) revient au defaut : c'est ce que la suppression dit.
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
        Some(s) if s.nature != nat.cle => Err(refus(format!("{} est un modèle de {}, pas de {} : en choisir un autre. Rien n'est écrit.", s.nom_dit(), nature_de(&s.nature)?.nom_du_contrat(), nat.nom_du_contrat()))),
        Some(s) => modele_lu(s.nom_dit(), false, &s.contrat.texte, &s.avenant.texte),
        None => modele_lu(nom_de_base(&nat), true, &texte_de_base(&nat, Partie::Contrat)?, &texte_de_base(&nat, Partie::Avenant)?),
    }
}

/// Les fichiers ou un contrat parti garde son texte et celui de son avenant.
fn figes(ctx: &Contexte, empreinte: &str) -> (PathBuf, PathBuf) {
    let d = dossier(ctx).join("partis");
    (d.join(format!("{}.modele", empreinte)), d.join(format!("{}.avenant.modele", empreinte)))
}

/// FIGE LE TEXTE DE CE MODELE ET CELUI DE SON AVENANT, au depart d'un contrat qui le suit (s'ils ne le sont pas deja) ;
/// rend leur empreinte, que la ligne du journal note.
pub(crate) fn figer(ctx: &Contexte, m: &ModeleDuContrat) -> R<String> {
    let e: String = socle::fichiers::sha256_hex(format!("{}\u{0}{}", m.lu.texte, m.avenant.texte).as_bytes()).chars().take(16).collect();
    let (contrat, avenant) = figes(ctx, &e);
    for (chemin, texte) in [(contrat, &m.lu.texte), (avenant, &m.avenant.texte)] {
        if !chemin.is_file() {
            let d = dossier(ctx).join("partis");
            std::fs::create_dir_all(&d).map_err(|x| disque(&d, x))?;
            std::fs::write(&chemin, texte).map_err(|x| disque(&chemin, x))?;
        }
    }
    Ok(e)
}

/// LE MODELE AVEC LEQUEL CE CONTRAT EST PARTI, et son avenant : les textes figes sous l'empreinte que sa ligne du journal
/// note. Une ligne qui n'en note pas est celle d'un contrat parti avant que le journal note son modele : il se refait
/// avec le modele qu'il suit aujourd'hui (`du_contrat`).
pub(crate) fn du_contrat_parti(ctx: &Contexte, empreinte: Option<&str>, p: &Paie) -> R<ModeleDuContrat> {
    let Some(e) = empreinte else { return du_contrat(ctx, p) };
    let (contrat, avenant) = figes(ctx, e);
    modele_lu("Le modèle avec lequel ce contrat est parti".to_string(), false, &lire_texte(&contrat)?, &lire_texte(&avenant)?)
}

// ------------------------------------------------------------------ les paragraphes d'un special

/// UN MODELE DECOUPE EN PARAGRAPHES, ligne a ligne et sans rien perdre : ce qui precede le premier `ARTICLE:` (l'en-tete,
/// le preambule, le bloc de modification d'un avenant, les notes), chaque paragraphe (sa ligne `ARTICLE:` et les
/// siennes), ce qui suit le dernier (les signatures). Recolle, il redonne le texte.
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

/// Un alinea : sa condition (le champ qui doit avoir une valeur ; `None` : toujours) et son texte. Un geste l'ecrit,
/// `modele-contrat` le rend.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Alinea {
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
fn paragraphe(titre: &str, alineas: &[Alinea]) -> R<Vec<String>> {
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

/// UN GESTE SUR LES PARAGRAPHES D'UNE PARTIE D'UN SPECIAL : `faire` change sa decoupe et rend ce que la phrase dit du
/// geste fait. Le texte qui en sort se verifie : s'il lui manque quelque chose que le texte d'avant avait, rien n'est
/// ecrit, et le refus dit quoi. Un manque deja la (la base a change) ne bloque pas : il se corrige un geste a la fois.
fn sur_les_paragraphes(ctx: &Contexte, id: &str, partie: Partie, faire: impl FnOnce(&mut Decoupe) -> R<String>) -> R<Modeles> {
    let mut m = lus(ctx)?;
    let i = m.speciaux.iter().position(|s| s.id == id).ok_or_else(|| aucun(id))?;
    let nat = nature_de(&m.speciaux[i].nature)?;
    let avant = m.speciaux[i].partie(partie).texte.clone();
    let mut d = Decoupe::de(&avant)?;
    let fait = faire(&mut d)?;
    let texte = d.texte();
    let deja = manques(&nat, partie, &avant)?;
    let dits: Vec<String> = manques(&nat, partie, &texte)?.into_iter().filter(|x| !deja.contains(x)).collect();
    if !dits.is_empty() {
        return Err(refus(format!("{} ne peut pas faire ce geste : il en sortirait faux. {} Rien n'est écrit.", m.speciaux[i].nom_dit(), dits.join(" "))));
    }
    let s = &mut m.speciaux[i];
    s.partie_mut(partie).texte = texte;
    let devant = format!("{}{} : {} ", s.nom, if partie == Partie::Avenant { ", avenant" } else { "" }, fait);
    ecrire(ctx, &m)?;
    modeles(ctx, &devant)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeEcrireParagraphe {
    id: String,
    partie: Partie,
    numero: usize,
    titre: String,
    alineas: Vec<Alinea>,
}

/// ECRIRE UN PARAGRAPHE : `{id, partie, numero, titre, alineas}` remplace le paragraphe `numero` (sa condition comprise).
fn ecrire_paragraphe(ctx: &Contexte, c: ChargeEcrireParagraphe) -> R<Modeles> {
    sur_les_paragraphes(ctx, &c.id, c.partie, |d| {
        let i = d.indice(c.numero)?;
        d.articles[i] = paragraphe(&c.titre, &c.alineas)?;
        Ok(format!("paragraphe {} « {} » écrit.", c.numero, d.titre(i)))
    })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeAjouter {
    id: String,
    partie: Partie,
    numero: usize,
    titre: String,
}

/// AJOUTER UN PARAGRAPHE : `{id, partie, numero, titre}`, un paragraphe sans alinea a la place `numero` (1 en tete,
/// n + 1 a la fin) ; ses alineas s'ecrivent ensuite.
fn ajouter_paragraphe(ctx: &Contexte, c: ChargeAjouter) -> R<Modeles> {
    sur_les_paragraphes(ctx, &c.id, c.partie, |d| {
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
    partie: Partie,
    numero: usize,
    sens: Sens,
}

/// DEPLACER UN PARAGRAPHE : `{id, partie, numero, sens}`, d'une place vers le haut ou vers le bas.
fn deplacer_paragraphe(ctx: &Contexte, c: ChargeDeplacer) -> R<Modeles> {
    sur_les_paragraphes(ctx, &c.id, c.partie, |d| {
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
    partie: Partie,
    numero: usize,
}

/// SUPPRIMER UN PARAGRAPHE : `{id, partie, numero}`. Celui qui porte une mention exigee ne se supprime pas tant qu'elle
/// n'est pas ailleurs : le verificateur le refuse, et dit laquelle.
fn supprimer_paragraphe(ctx: &Contexte, c: ChargeParagraphe) -> R<Modeles> {
    sur_les_paragraphes(ctx, &c.id, c.partie, |d| {
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
    partie: Partie,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeDefaut {
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
    partie: Partie,
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
    /// Ce qui manque a son contrat, puis a son avenant.
    manques: Vec<String>,
    pastille: Pastille,
}

/// Une nature et ses modeles de contrat.
#[derive(Debug, Serialize)]
struct NatureDite {
    nature: String,
    libelle: String,
    nom: String,
    modeles: Vec<ModeleDit>,
}

/// Ce que l'ecran des modeles de contrat dit (`ecran` de `modeles-contrat`).
#[derive(Debug, Serialize)]
struct EcranDesModeles {
    intro: &'static str,
    comment: [&'static str; 3],
    en_francais: &'static str,
}

/// Ce que rendent `modeles-contrat` et chaque geste qui change un modele.
#[derive(Debug, Serialize)]
struct Modeles {
    phrase: String,
    natures: Vec<NatureDite>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ecran: Option<EcranDesModeles>,
}

/// Un article d'un modele : son numero, son titre, ses alineas.
#[derive(Debug, Serialize)]
struct ArticleRendu {
    numero: usize,
    titre: String,
    alineas: Vec<Alinea>,
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
    partie: Partie,
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

macro_rules! dites {
    ($($t:ty),*) => {$(
        impl Dite for $t {
            fn phrase(&self) -> &str {
                &self.phrase
            }
        }
    )*};
}
dites!(Modeles, ModeleRendu, Apercu);

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

/// Ce qui manque au contrat et a l'avenant d'un modele, l'avenant nomme.
fn manques_des_deux(nat: &Nature, contrat: &str, avenant: &str) -> R<Vec<String>> {
    let mut v = manques(nat, Partie::Contrat, contrat)?;
    v.extend(manques(nat, Partie::Avenant, avenant)?.into_iter().map(|x| format!("Avenant : {}", x)));
    Ok(v)
}

/// CE QUE LA PAGE MONTRE : pour chaque nature, ses modeles (la base, puis les speciaux, le defaut marque, ce qui manque
/// a chacun, si sa base a change depuis la copie). `devant` : ce que la phrase dit avant (le geste qui vient d'etre fait).
fn modeles(ctx: &Contexte, devant: &str) -> R<Modeles> {
    let m = lus(ctx)?;
    let mut sortie = Vec::new();
    for nat in natures_de(ctx)? {
        let (contrat, avenant) = (texte_de_base(&nat, Partie::Contrat)?, texte_de_base(&nat, Partie::Avenant)?);
        let defaut = m.defauts.get(&nat.cle);
        let dit = |id: Option<&String>, nom: String, manques: Vec<String>, base_changee: bool| {
            let est_defaut = defaut == id;
            let pastille = match (manques.is_empty(), est_defaut) {
                (false, _) => Pastille::de("refuse", "incomplet"),
                (true, true) => Pastille::de("fait", "par défaut"),
                (true, false) if id.is_none() => Pastille::de("info", "le modèle du logiciel"),
                (true, false) => Pastille::de("info", "modèle spécial"),
            };
            ModeleDit { id: id.cloned(), nom, base: id.is_none(), defaut: est_defaut, base_changee: base_changee.then_some(BASE_CHANGEE), manques, pastille }
        };
        let mut pieces = vec![dit(None, nom_de_base(&nat), manques_des_deux(&nat, &contrat, &avenant)?, false)];
        for s in m.speciaux.iter().filter(|s| s.nature == nat.cle) {
            let change = s.contrat.base != contrat || s.avenant.base != avenant;
            pieces.push(dit(Some(&s.id), s.nom.clone(), manques_des_deux(&nat, &s.contrat.texte, &s.avenant.texte)?, change));
        }
        sortie.push(NatureDite { nature: nat.cle.clone(), libelle: nat.libelle.clone(), nom: nat.nom_du_contrat().to_string(), modeles: pieces });
    }
    Ok(Modeles { phrase: format!("{}Modèles de contrat spéciaux de la compagnie : {}.", devant, m.speciaux.len()), natures: sortie, ecran: None })
}

/// CE QUI A CHANGE DANS LA BASE depuis la copie d'une partie d'un special (`avant` : la base copiee, `maintenant` :
/// celle du jour), paragraphe par paragraphe (par leur titre), sans rien fusionner : la compagnie reprend ce qu'elle veut.
fn ce_qui_a_change(avant: &str, maintenant: &str) -> Vec<String> {
    if avant == maintenant {
        return Vec::new();
    }
    let (Ok(a), Ok(b)) = (fabrique_pdf::modele::lire_modele(avant, "la base copiée"), fabrique_pdf::modele::lire_modele(maintenant, "la base")) else {
        return vec![BASE_CHANGEE.to_string()];
    };
    let mut dits = Vec::new();
    if (&a.titre, &a.soustitre, &a.preambule, &a.modification) != (&b.titre, &b.soustitre, &b.preambule, &b.modification) {
        dits.push("Dans la base, l'en-tête (titre, préambule, bloc de modification) a changé.".to_string());
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

/// LIRE UNE PARTIE D'UN MODELE : `{nature, id, partie}` (sans id : la base), ses articles et leurs alineas, ce qui lui
/// manque, les champs qu'un alinea insere ou pose en condition, ce qui a change dans sa base, les engagements de
/// l'apercu.
fn lire(ctx: &Contexte, c: ChargeLire) -> R<ModeleRendu> {
    let nat = nature_de(&c.nature)?;
    let m = lus(ctx)?;
    let (nom, s) = designe(&m, &nat, c.id.as_deref())?;
    let texte = texte_de(&nat, s, c.partie)?;
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
                        Some((si, t)) => Alinea { si: Some(si.to_string()), texte: t.to_string() },
                        None => Alinea { si: None, texte: a.clone() },
                    })
                    .collect(),
            })
            .collect(),
        Err(_) => Vec::new(),
    };
    let dits = manques(&nat, c.partie, &texte)?;
    let quoi = if c.partie == Partie::Avenant { format!("{}, son avenant", nom) } else { nom };
    let phrase = if dits.is_empty() { format!("{} : complet.", quoi) } else { format!("{} : {}", quoi, dits.join(" ")) };
    let base_changee = match s {
        Some(s) => ce_qui_a_change(&s.partie(c.partie).base, &texte_de_base(&nat, c.partie)?),
        None => Vec::new(),
    };
    let champs = documents::contrat_ecrit::champs_des_modeles(&nat, c.partie == Partie::Avenant).map_err(|m| erreur(Classe::Illisible, m))?;
    let engagements = engagements_de(ctx, &nat.cle)?;
    Ok(ModeleRendu {
        phrase,
        nature: nat.cle.clone(),
        id: c.id,
        partie: c.partie,
        // Le nom qu'un special porte (celui que l'on renomme) ; la base, comme la liste la nomme.
        nom: s.map_or_else(|| nom_de_base(&nat), |s| s.nom.clone()),
        base: s.is_none(),
        articles,
        manques: dits,
        en_francais: documents::contrat_ecrit::EN_FRANCAIS,
        champs,
        base_changee,
        engagements,
    })
}

/// L'APERCU D'UN MODELE : `{nature, id, partie, engagement}`, le PDF du contrat de cet engagement (le plus recent sans
/// lui) fait avec ce modele, ou d'un avenant n° 1 a ce contrat, date du jour, qui cite son premier article sans le
/// changer ; dans `sorties/apercus/`, rien d'autre ne s'ecrit, rien ne part. Un modele qui ne fait pas ce contrat se
/// refuse comme au depart, et dit pourquoi.
fn apercu(ctx: &Contexte, c: ChargeApercu) -> R<Apercu> {
    let nat = nature_de(&c.nature)?;
    let m = lus(ctx)?;
    let (nom, s) = designe(&m, &nat, c.id.as_deref())?;
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
    let modele = modele_lu(nom.clone(), s.is_none(), &texte_de(&nat, s, Partie::Contrat)?, &texte_de(&nat, s, Partie::Avenant)?)?;
    let ent = documents::avenant::ent_du_mois(ctx, &mois)?;
    let d = ctx.racine.join("sorties").join("apercus");
    std::fs::create_dir_all(&d).map_err(|e| disque(&d, e))?;
    let (sorte, quoi) = match c.partie {
        Partie::Contrat => ("contrat", "le contrat"),
        Partie::Avenant => ("avenant", "un avenant au contrat"),
    };
    let chemin = d.join(format!("modele-{}-{}-{}.pdf", nat.cle, c.id.as_deref().unwrap_or("base"), sorte));
    let (debut, jour) = (p.engagement.debut.jj_mm_aaaa(), ctx.aujourdhui.jj_mm_aaaa());
    match c.partie {
        Partie::Contrat => documents::contrat::ecrire_contrat(ctx, std::slice::from_ref(&p), &[modele], &ent, &chemin, &[])?,
        Partie::Avenant => {
            // Rien ne change : l'apercu cite le premier article du contrat tel qu'il est (`modification_d_apercu`).
            let v = documents::contrat::valeurs_de_paie(ctx, &ent, &p)?;
            let contrat = modele.lu.decoupe(&nom).map_err(|m| erreur(Classe::Illisible, m))?;
            let x = documents::avenant::modification_d_apercu(&contrat, &v).map_err(|m| refus(m))?;
            let modifs = [fabrique_pdf::forme::Modification { numero: Some(x.numero as i64), article: x.article, avant: x.avant, apres: x.apres, phrase: None }];
            documents::contrat::ecrire_avenant(ctx, &p, &ent, &chemin, &modifs, 1, &debut, &jour, &jour, &modele, false)?
        }
    }
    Ok(Apercu { phrase: format!("{}, {} de {} du {} : l'aperçu est fait, rien n'est parti.", nom, quoi, p.engagement.nom, debut), chemin: chemin.to_string_lossy().into_owned() })
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

/// COPIER LA BASE D'UNE NATURE EN UN MODELE SPECIAL : `{nature, nom}`, son contrat et son avenant ; chacun garde le texte
/// de la base dont il part.
fn copier(ctx: &Contexte, c: ChargeCopier) -> R<Modeles> {
    let nat = nature_de(&c.nature)?;
    let mut m = lus(ctx)?;
    let nom = nom_libre(&m, None, &c.nom)?;
    let partie = |p| texte_de_base(&nat, p).map(|t| Texte { base: t.clone(), texte: t });
    let (contrat, avenant) = (partie(Partie::Contrat)?, partie(Partie::Avenant)?);
    // Un id ne sert qu'une fois : ni un special present, ni un supprime.
    let n = m.speciaux.iter().map(|s| &s.id).chain(m.supprimes.iter()).filter_map(|id| id.strip_prefix('m')?.parse::<u32>().ok()).max().map_or(1, |n| n + 1);
    m.speciaux.push(Special { id: format!("m{}", n), nom: nom.clone(), nature: nat.cle.clone(), contrat, avenant });
    ecrire(ctx, &m)?;
    modeles(ctx, &format!("Modèle « {} » copié de la base du {}. ", nom, nat.nom_du_contrat()))
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
fn par_defaut(ctx: &Contexte, c: ChargeDefaut) -> R<Modeles> {
    let nat = nature_de(&c.nature)?;
    let mut m = lus(ctx)?;
    let devant = match &c.id {
        Some(id) => {
            let (nom, _) = designe(&m, &nat, Some(id))?;
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

/// CE QUE L'ECRAN DES MODELES DE CONTRAT DIT (`ecran` de `modeles-contrat`) : a quoi sert un modele, comment faire
/// celui de la compagnie, le francais.
fn ecran_des_modeles() -> EcranDesModeles {
    EcranDesModeles {
        intro: "Chaque contrat que le logiciel fait signer suit un modèle, et ses avenants l'avenant de ce modèle. Le modèle de base de chaque sorte de contrat porte déjà ce que la loi exige, rempli de lui-même depuis l'engagement et la configuration ; il suit les mises à jour et ne se modifie pas. La compagnie le copie en modèles spéciaux, choisit celui par défaut, et un engagement peut en choisir un autre (Avancé). Un contrat parti garde son texte.",
        comment: [
            "« Copier la base » fait un modèle spécial, contrat et avenant, qui garde la base dont il part : si elle change avec une mise à jour, le modèle dit quels paragraphes ont changé.",
            "Un modèle spécial s'écrit paragraphe par paragraphe : un titre, des alinéas. Une donnée s'insère par la liste et se remplit d'elle-même ; un alinéa peut ne s'écrire que si une donnée est remplie (les clauses particulières d'un engagement, par exemple) ; **texte** met en gras.",
            "Ce que la loi exige doit rester : un geste qui l'ôterait est refusé, et le refus dit quelle donnée manque. L'aperçu montre le contrat rempli avec un engagement de la compagnie.",
        ],
        en_francais: documents::contrat_ecrit::EN_FRANCAIS,
    }
}
