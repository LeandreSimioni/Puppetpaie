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
use crate::catalogue::{Acte, Affiche, Ecran, Fiche, Forme, Place, Refus, MANQUE, OPERATION};
use crate::fiches::{CaseDeLaFiche, PartieDeLaFiche};
use crate::ouverture::Choix;
use droit::fiche::ValeurDeCase;
use crate::declaration::{charge, cle, dit, ChargeDuType, Legende, ParSaForme, ValeurDeLaCharge};
use crate::enveloppe::{en_json, Charge, Dite, Phrase, Porte, Rendu};
use crate::imprevus::GesteDeRoute;
use crate::SansCharge;
use documents::contrat_ecrit::{ChampDeModele, ModeleDuContrat};
use documents::marqueurs::ModeleLu;
use droit::contexte::{Contexte, IdSalarie};
use droit::natures::{Nature, NatureDeContrat};
use engagement::calcul::Paie;
use engagement::carte::IdDeModele;
use socle::date::Jour;
use socle::echec::{erreur, Classe, Echec, R};
use socle::ecran::Pastille;
use socle::fichiers::{Empreinte, EmpreinteCourte};
use socle::mois::Mois;

const NATURE: Legende = dit("Nature du contrat", "la nature du contrat : cddu_spectacle, cdd, cdi…");
const ID: Legende = dit("Modèle", "l'id d'un modèle spécial (`modeles` de modeles-contrat)");
const ID_OU_BASE: Legende = dit("Modèle", "l'id d'un modèle spécial ; absent ou null : le modèle de base de la nature");
const PARTIE: Legende = dit("Partie", "le texte du modèle que le geste lit ou écrit");
const NOM: Legende = dit("Nom", "le nom du modèle spécial");
const NUMERO: Legende = dit("Paragraphe", "le numéro du paragraphe (`articles[].numero` de modele-contrat), à partir de 1");
const TITRE: Legende = dit("Titre", "le titre du paragraphe");
const MODELE_REFUSE: Refus = Refus { classe: Classe::Refuse, corriger: "le modèle : ce que la phrase nomme, paragraphe par paragraphe ; rien n'est écrit" };

/// Ce que la ligne d'un modele special dit quand une base dont il est copie a change depuis (une mise a jour).
const BASE_CHANGEE: &str = "Le modèle de base a changé depuis la copie : ouvrir ce modèle pour voir quels paragraphes.";

/// Ou se voient les deux lectures de ce module : l'ecran des modeles de contrat les appelle, sous « Avance ».
const DESSINE: Ecran = Ecran::DessineAvance(Place::ModelesContrat);
/// Ou se voient ses gestes : les deux lectures les rendent (`GesteDuModele`), sous « Avance ».
const GESTE: Ecran = Ecran::GesteAvance(Place::ModelesContrat);

/// Une fiche de ce module.
const fn fiche(route: Route, geste: &'static str, charge: &'static ChargeDuType, exemple: &'static str, rend: &'static str, refus: &'static [Refus], acte: Acte, ecran: Ecran) -> Fiche {
    Fiche { route, geste, charge, exemple, rend, refus, acte, phrase: 0, ecran, pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee }
}

/// Une fiche d'un geste qui ecrit un modele et rend la liste, comme modeles-contrat.
const fn geste(route: Route, geste: &'static str, charge: &'static ChargeDuType, exemple: &'static str) -> Fiche {
    fiche(route, geste, charge, exemple, "les modèles, comme modeles-contrat", &[MODELE_REFUSE, MANQUE], Acte::Ecrit, GESTE)
}

/// Les routes de ce module, et leurs fiches, dans l'ordre ou leurs exemples se jouent : la copie fait `m1`, que les
/// gestes suivants ecrivent, et que le dernier supprime.
pub const ROUTES: &[Fiche] = &[
    fiche(
        Route::ModelesContrat,
        "Lister les modèles de contrat de chaque nature, la base et les spéciaux.",
        &super::SansCharge::CHARGE,
        r#"{}"#,
        "natures (chacune : son titre, ses gestes (copier sa base), ses modèles, la base d'abord, le défaut marqué, ce qui manque à chacun, à son contrat ou à son avenant, ouvrir (le geste qui lit le modèle) et ses gestes) : chaque geste a sa route, sa charge, son libellé, les cases que sa boîte fait saisir et ce qu'elles montrent d'abord",
        &[OPERATION],
        Acte::Lit,
        DESSINE,
    ),
    geste(Route::CopierModeleContrat, "Copier en modèle spécial le modèle de base d'une nature, contrat et avenant.", &ChargeCopier::CHARGE, r#"{"nature":"cddu_spectacle","nom":"Tournée"}"#),
    fiche(
        Route::ModeleContrat,
        "Lire le contrat ou l'avenant d'un modèle, par paragraphe, et ses manques.",
        &ChargeLire::CHARGE,
        r#"{"nature":"cddu_spectacle","id":"m1","partie":"contrat"}"#,
        "articles (numero, titre, alinéas et leur condition ; ecrire, le geste qui l'enregistre, que les alinéas tapés complètent ; gestes, ceux qu'on peut y faire ; un article sans geste se lit seulement), manques, en_francais, champs (ceux qu'un alinéa insère ou pose en condition, avec leur libellé), base_changee (ce qui a changé dans la base depuis la copie ; null pour la base), engagements (ceux de l'aperçu, le plus récent d'abord), parties (le geste qui lit le contrat, celui qui lit l'avenant), gestes (ceux qu'on peut faire sur le modèle), apercu (le geste qui fait l'aperçu, que l'engagement choisi complète) : chaque geste a sa route, sa charge, son libellé, les cases que sa boîte fait saisir et ce qu'elles montrent d'abord",
        &[OPERATION, MANQUE],
        Acte::Lit,
        DESSINE,
    ),
    geste(Route::RenommerModeleContrat, "Renommer un modèle spécial.", &ChargeRenommer::CHARGE, r#"{"id":"m1","nom":"Tournée d'été"}"#),
    geste(
        Route::EcrireParagrapheContrat,
        "Écrire un paragraphe d'un modèle spécial : titre, alinéas et leur condition.",
        &ChargeEcrireParagraphe::CHARGE,
        r#"{"id":"m1","partie":"contrat","numero":3,"titre":"Lieu de travail","alineas":[{"si":null,"texte":"Le travail a lieu {{contrat.lieu}}."}]}"#,
    ),
    geste(
        Route::AjouterParagrapheContrat,
        "Ajouter un paragraphe à un modèle spécial, à une place.",
        &ChargeAjouter::CHARGE,
        r#"{"id":"m1","partie":"contrat","numero":4,"titre":"Déplacements"}"#,
    ),
    geste(
        Route::DeplacerParagrapheContrat,
        "Monter ou descendre un paragraphe d'un modèle spécial.",
        &ChargeDeplacer::CHARGE,
        r#"{"id":"m1","partie":"contrat","numero":4,"sens":"monter"}"#,
    ),
    geste(
        Route::SupprimerParagrapheContrat,
        "Supprimer un paragraphe d'un modèle spécial, sauf une mention exigée seule.",
        &ChargeParagraphe::CHARGE,
        r#"{"id":"m1","partie":"contrat","numero":3}"#,
    ),
    fiche(
        Route::ApercuModeleContrat,
        "Faire le PDF du contrat ou de l'avenant d'un modèle, rempli d'un engagement.",
        &ChargeApercu::CHARGE,
        r#"{"nature":"cddu_spectacle","id":"m1","partie":"avenant","engagement":null}"#,
        "phrase, chemin (le PDF de l'aperçu)",
        &[MODELE_REFUSE, MANQUE],
        Acte::Lit,
        GESTE,
    ),
    fiche(Route::ModeleContratParDefaut, "Choisir le modèle par défaut d'une nature.", &ChargeDefaut::CHARGE, r#"{"nature":"cddu_spectacle","id":"m1"}"#, "les modèles, comme modeles-contrat", &[OPERATION, MANQUE], Acte::Ecrit, GESTE),
    fiche(
        Route::SupprimerModeleContrat,
        "Supprimer un modèle spécial ; ses engagements suivent le défaut.",
        &ChargeId::CHARGE,
        r#"{"id":"m1"}"#,
        "les modèles, comme modeles-contrat ; la phrase nomme les engagements qui l'avaient choisi",
        &[OPERATION, MANQUE],
        Acte::Ecrit,
        GESTE,
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
    defauts: BTreeMap<NatureDeContrat, IdDeModele>,
    supprimes: Vec<IdDeModele>,
}

/// UN MODELE SPECIAL : son id (celui que l'engagement et le defaut nomment), son nom, sa nature, son contrat et son
/// avenant.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Special {
    id: IdDeModele,
    nom: String,
    nature: NatureDeContrat,
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

impl socle::liste::ListeFermee for Partie {
    const TOUTES: &'static [Partie] = &[Partie::Contrat, Partie::Avenant];

    fn code(&self) -> R<String> {
        socle::liste::code_ecrit(self)
    }

    fn libelle(&self) -> R<&'static str> {
        Ok(self.bouton().texte())
    }
}

impl Partie {
    /// Le libelle de la partie, celui du geste qui la lit (une constante : un texte fautif ne compile pas).
    const fn bouton(self) -> Affiche {
        match self {
            Partie::Contrat => const { Affiche::new("Contrat") },
            Partie::Avenant => const { Affiche::new("Avenant") },
        }
    }
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
    socle::echec::du_disque(&chemin, &e).reclasser(Classe::Disque)
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
    socle::forme::lire_fichier(&chemin, "les modèles de contrat de la compagnie").map_err(|m| m.reclasser(Classe::Illisible))
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

/// Le texte livre d'une partie du modele de base de cette nature.
fn texte_de_base(nat: &Nature, partie: Partie) -> R<String> {
    let nom = match partie {
        Partie::Contrat => nat.modele_contrat(),
        Partie::Avenant => nat.modele_avenant(),
    };
    let nom = nom.ok_or_else(|| refus(format!("La nature {} n'a pas de contrat écrit.", nat.libelle)))?;
    lire_texte(&socle::fichiers::dossier_du_code().join("modeles").join(nom))
}

/// Un nom en tete de ligne : sa premiere lettre en capitale, le reste tel quel (« Contrat de travail à durée
/// déterminée »).
fn capitale(t: &str) -> String {
    let mut c = t.chars();
    c.next().into_iter().flat_map(char::to_uppercase).chain(c).collect()
}

fn nom_de_base(nat: &Nature) -> String {
    format!("Le modèle de base du {}", nat.nom_du_contrat())
}

/// Ce qui manque a une partie d'un modele de cette nature (le verificateur de sa partie).
fn manques(nat: &Nature, partie: Partie, texte: &str) -> R<Vec<String>> {
    match partie {
        Partie::Contrat => documents::contrat_ecrit::manques(nat, texte, None).map_err(|m| m.reclasser(Classe::Illisible)),
        Partie::Avenant => Ok(documents::contrat_ecrit::manques_de_l_avenant(texte)),
    }
}

/// Un modele lu pour un contrat, et son avenant ; un texte qui ne se lit pas est un refus.
fn modele_lu(nom: String, base: bool, contrat: &str, avenant: &str) -> R<ModeleDuContrat> {
    let lire = |t: &str| ModeleLu::lire(t, &nom).map_err(|m| m.reclasser(Classe::ModeleIncomplet));
    Ok(ModeleDuContrat { lu: lire(contrat)?, avenant: lire(avenant)?, nom, base })
}

fn aucun(id: &IdDeModele) -> Echec {
    IdDeModele::aucun(id)
}

/// Le special que nomme `id` (l'id d'une charge, ou celui d'un defaut).
fn special<'a>(m: &'a ModelesDeContrat, id: &IdDeModele) -> R<&'a Special> {
    m.speciaux.iter().find(|s| s.id == *id).ok_or_else(|| aucun(id))
}

/// Le modele que nomme `{nature, id}` : son nom, et le special (aucun : la base).
fn designe<'a>(m: &'a ModelesDeContrat, nat: &Nature, id: Option<&IdDeModele>) -> R<(String, Option<&'a Special>)> {
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
    let nat = p.engagement.nat()?;
    let m = lus(ctx)?;
    let choix = p.engagement.ecrit.modele_contrat.as_ref().filter(|id| !m.supprimes.contains(id));
    let choisi = match choix {
        Some(id) => Some(m.speciaux.iter().find(|s| s.id == *id).ok_or_else(|| {
            refus(format!("Le contrat de {} suit le modèle {}, que la compagnie n'a pas : en choisir un autre dans son engagement (Avancé › Modèle de contrat). Rien n'est écrit.", p.salarie.id, id))
        })?),
        None => m.defauts.get(&nat.cle).map(|id| special(&m, id)).transpose()?,
    };
    match choisi {
        Some(s) if s.nature != nat.cle => Err(refus(format!("{} est un modèle de {}, pas de {} : en choisir un autre. Rien n'est écrit.", s.nom_dit(), s.nature.proprietes()?.nom_du_contrat(), nat.nom_du_contrat()))),
        Some(s) => modele_lu(s.nom_dit(), false, &s.contrat.texte, &s.avenant.texte),
        None => modele_lu(nom_de_base(&nat), true, &texte_de_base(&nat, Partie::Contrat)?, &texte_de_base(&nat, Partie::Avenant)?),
    }
}

/// Les fichiers ou un contrat parti garde son texte et celui de son avenant.
fn figes(ctx: &Contexte, empreinte: &EmpreinteCourte) -> (PathBuf, PathBuf) {
    let d = dossier(ctx).join("partis");
    (d.join(format!("{}.modele", empreinte)), d.join(format!("{}.avenant.modele", empreinte)))
}

/// FIGE LE TEXTE DE CE MODELE ET CELUI DE SON AVENANT, au depart d'un contrat qui le suit (s'ils ne le sont pas deja) ;
/// rend leur empreinte, que la ligne du journal note.
pub(crate) fn figer(ctx: &Contexte, m: &ModeleDuContrat) -> R<EmpreinteCourte> {
    let e = Empreinte::des_octets(format!("{}\u{0}{}", m.lu.texte, m.avenant.texte).as_bytes()).courte();
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
pub(crate) fn du_contrat_parti(ctx: &Contexte, empreinte: Option<&EmpreinteCourte>, p: &Paie) -> R<ModeleDuContrat> {
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
    articles: Vec<Paragraphe>,
    queue: Vec<String>,
}

/// Un paragraphe de la decoupe : son titre (ce que dit sa ligne `ARTICLE:`), lu une fois quand il se decoupe ou
/// s'ecrit, et ses lignes, la ligne `ARTICLE:` en tete.
struct Paragraphe {
    titre: String,
    lignes: Vec<String>,
}

impl Decoupe {
    fn de(texte: &str) -> R<Decoupe> {
        let mut d = Decoupe { tete: Vec::new(), articles: Vec::new(), queue: Vec::new() };
        for l in texte.split('\n') {
            let cle = fabrique_pdf::modele::CLES.iter().find(|k| l.starts_with(**k)).copied();
            match cle {
                Some(k @ "ARTICLE:") if d.queue.is_empty() => d.articles.push(Paragraphe { titre: l[k.len()..].trim().to_string(), lignes: vec![l.to_string()] }),
                Some("ARTICLE:") => return Err(refus("Ce modèle a un paragraphe après ses signatures : il ne se découpe pas en paragraphes. Rien n'est écrit.")),
                _ if d.articles.is_empty() => d.tete.push(l.to_string()),
                Some(_) => d.queue.push(l.to_string()),
                None if d.queue.is_empty() => d.articles.last_mut().into_iter().for_each(|a| a.lignes.push(l.to_string())),
                None => d.queue.push(l.to_string()),
            }
        }
        Ok(d)
    }

    fn texte(&self) -> String {
        self.tete.iter().chain(self.articles.iter().flat_map(|p| p.lignes.iter())).chain(self.queue.iter()).cloned().collect::<Vec<_>>().join("\n")
    }

    /// L'indice du paragraphe `numero` (a partir de 1).
    fn indice(&self, numero: usize) -> R<usize> {
        (1..=self.articles.len()).contains(&numero).then(|| numero - 1).ok_or_else(|| refus(format!("Ce modèle n'a pas de paragraphe {} : il en a {}. Rien n'est écrit.", numero, self.articles.len())))
    }

    /// Le titre du paragraphe d'indice `i`.
    fn titre(&self, i: usize) -> String {
        self.articles[i].titre.clone()
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
fn paragraphe(titre: &str, alineas: &[Alinea]) -> R<Paragraphe> {
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
    Ok(Paragraphe { titre, lignes })
}

/// UN GESTE SUR LES PARAGRAPHES D'UNE PARTIE D'UN SPECIAL : `faire` change sa decoupe et rend ce que la phrase dit du
/// geste fait. Le texte qui en sort se verifie : s'il lui manque quelque chose que le texte d'avant avait, rien n'est
/// ecrit, et le refus dit quoi. Un manque deja la (la base a change) ne bloque pas : il se corrige un geste a la fois.
fn sur_les_paragraphes(ctx: &Contexte, id: &IdDeModele, partie: Partie, faire: impl FnOnce(&mut Decoupe) -> R<String>) -> R<Modeles> {
    let mut m = lus(ctx)?;
    let i = m.speciaux.iter().position(|s| s.id == *id).ok_or_else(|| aucun(id))?;
    let nat = m.speciaux[i].nature.proprietes()?;
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

/// LES PARTIES D'UN ALINEA : sa condition, au choix parmi les champs que `modele-contrat` rend (`champs`), vide :
/// toujours ; son texte, sur plusieurs lignes.
const PARTIES_D_UN_ALINEA: &[crate::catalogue::Partie] = &[
    crate::catalogue::Partie { cle: "si", libelle: Affiche::new("Condition"), forme: Forme::ChoixRendu, toujours: true },
    crate::catalogue::Partie { cle: "texte", libelle: Affiche::new("Texte"), forme: Forme::TexteLong, toujours: true },
];

/// Dans une charge, un alinea est une valeur faite de parties (`{si, texte}`) : les alineas d'un paragraphe, des rangees.
impl ValeurDeLaCharge<ParSaForme> for Alinea {
    const FORME: Forme = Forme::Parties(PARTIES_D_UN_ALINEA);
}

charge! {
    #[derive(Debug)]
    struct ChargeEcrireParagraphe {
        #[serde(deserialize_with = "engagement::carte::id_de_la_charge::deserialize")]
        id: IdDeModele = ID,
        partie: Partie = PARTIE,
        numero: usize = NUMERO,
        titre: String = TITRE,
        alineas: Vec<Alinea> = dit("Alinéas", "[{si, texte}, …] dans l'ordre : texte, l'alinéa (une donnée s'y insère par {{son.champ}}, un des `champs` de modele-contrat) ; si, le champ qui doit avoir une valeur pour qu'il s'écrive, null : toujours"),
    }
}

/// ECRIRE UN PARAGRAPHE : `{id, partie, numero, titre, alineas}` remplace le paragraphe `numero` (sa condition comprise).
fn ecrire_paragraphe(ctx: &Contexte, c: ChargeEcrireParagraphe) -> R<Modeles> {
    sur_les_paragraphes(ctx, &c.id, c.partie, |d| {
        let i = d.indice(c.numero)?;
        d.articles[i] = paragraphe(&c.titre, &c.alineas)?;
        Ok(format!("paragraphe {} « {} » écrit.", c.numero, d.titre(i)))
    })
}

charge! {
    #[derive(Debug)]
    struct ChargeAjouter {
        #[serde(deserialize_with = "engagement::carte::id_de_la_charge::deserialize")]
        id: IdDeModele = ID,
        partie: Partie = PARTIE,
        numero: usize = dit("Place", "sa place : 1 en tête, le nombre de paragraphes plus 1 à la fin"),
        titre: String = TITRE,
    }
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
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Sens {
    Monter,
    Descendre,
}

impl socle::liste::ListeFermee for Sens {
    const TOUTES: &'static [Sens] = &[Sens::Monter, Sens::Descendre];

    fn code(&self) -> R<String> {
        socle::liste::code_ecrit(self)
    }

    fn libelle(&self) -> R<&'static str> {
        Ok(self.bouton().texte())
    }
}

impl Sens {
    /// Le libelle du sens, celui du geste qui deplace un paragraphe (une constante : un texte fautif ne compile pas).
    const fn bouton(self) -> Affiche {
        match self {
            Sens::Monter => const { Affiche::new("Monter") },
            Sens::Descendre => const { Affiche::new("Descendre") },
        }
    }
}

charge! {
    #[derive(Debug)]
    struct ChargeDeplacer {
        #[serde(deserialize_with = "engagement::carte::id_de_la_charge::deserialize")]
        id: IdDeModele = ID,
        partie: Partie = PARTIE,
        numero: usize = NUMERO,
        sens: Sens = dit("Sens", "où le paragraphe se déplace"),
    }
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

charge! {
    #[derive(Debug)]
    struct ChargeParagraphe {
        #[serde(deserialize_with = "engagement::carte::id_de_la_charge::deserialize")]
        id: IdDeModele = ID,
        partie: Partie = PARTIE,
        numero: usize = NUMERO,
    }
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

charge! {
    #[derive(Debug)]
    struct ChargeLire {
        nature: NatureDeContrat = NATURE,
        #[serde(default, deserialize_with = "engagement::carte::id_de_la_charge::option::deserialize")]
        id: Option<IdDeModele> = ID_OU_BASE,
        partie: Partie = PARTIE,
    }
}

charge! {
    #[derive(Debug)]
    struct ChargeDefaut {
        nature: NatureDeContrat = NATURE,
        #[serde(default, deserialize_with = "engagement::carte::id_de_la_charge::option::deserialize")]
        id: Option<IdDeModele> = ID_OU_BASE,
    }
}

charge! {
    #[derive(Debug)]
    struct ChargeCopier {
        nature: NatureDeContrat = NATURE,
        nom: String = NOM,
    }
}

charge! {
    #[derive(Debug)]
    struct ChargeRenommer {
        #[serde(deserialize_with = "engagement::carte::id_de_la_charge::deserialize")]
        id: IdDeModele = ID,
        nom: String = NOM,
    }
}

charge! {
    #[derive(Debug)]
    struct ChargeId {
        #[serde(deserialize_with = "engagement::carte::id_de_la_charge::deserialize")]
        id: IdDeModele = ID,
    }
}

/// L'engagement d'un apercu : le mois de sa carte, la cle de son salarie, son debut.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EngagementVise {
    mois: Mois,
    salarie: IdSalarie,
    debut: Jour,
}

/// Dans une charge, l'engagement d'un apercu se dit d'un objet (`{mois, salarie, debut}`).
impl ValeurDeLaCharge<ParSaForme> for EngagementVise {
    const FORME: Forme = Forme::Objet;
}

charge! {
    #[derive(Debug)]
    struct ChargeApercu {
        nature: NatureDeContrat = NATURE,
        #[serde(default, deserialize_with = "engagement::carte::id_de_la_charge::option::deserialize")]
        id: Option<IdDeModele> = ID_OU_BASE,
        partie: Partie = PARTIE,
        engagement: Option<EngagementVise> = dit("Engagement", "{mois, salarie, debut}, un des `engagements` de modele-contrat ; absent ou null : le plus récent"),
    }
}

/// Un modele de contrat d'une nature, tel que la liste le dit : la base (sans id) ou un special.
#[derive(Debug, Serialize)]
struct ModeleDit {
    id: Option<IdDeModele>,
    nom: String,
    base: bool,
    defaut: bool,
    /// Ce que la ligne dit d'une base qui a change depuis la copie ; `None` : rien.
    base_changee: Option<&'static str>,
    /// Ce qui manque a son contrat, puis a son avenant.
    manques: Vec<String>,
    pastille: Pastille,
    /// Le geste qui l'ouvre : la lecture de son contrat (`modele-contrat`).
    ouvrir: GesteDuModele,
    /// Ce qu'on peut faire sur lui : le choisir par defaut (sauf s'il l'est), le supprimer (un special).
    gestes: Vec<GesteDuModele>,
}

/// Une nature et ses modeles de contrat : son titre (le nom de son contrat, en tete de ligne), ce qu'on peut y faire
/// (copier sa base).
#[derive(Debug, Serialize)]
struct NatureDite {
    nature: NatureDeContrat,
    titre: String,
    libelle: String,
    nom: String,
    gestes: Vec<GesteDuModele>,
    modeles: Vec<ModeleDit>,
}

/// UN GESTE DE L'ECRAN DES MODELES DE CONTRAT (`imprevus::GesteDeRoute`) : sa charge designe le modele, sa partie ou son
/// paragraphe (`SurLeModele`) ; ses cases montrent d'abord le nom du modele (`ValeursDuModele`) et n'offrent aucun choix.
type GesteDuModele = GesteDeRoute<SurLeModele, ValeursDuModele, SansCharge>;

/// CE QUI DESIGNE, AUX GESTES DE L'ECRAN DES MODELES DE CONTRAT, la chose sur laquelle ils se font : la nature (copier sa
/// base, choisir le defaut), le modele special (aucun : la base), sa partie, un paragraphe et le sens ou il se deplace.
/// Ce qui ne la designe pas ne s'ecrit pas : chaque geste a les champs de la charge de sa route, rien d'autre.
#[derive(Debug, Clone, Default, Serialize)]
struct SurLeModele {
    #[serde(skip_serializing_if = "Option::is_none")]
    nature: Option<NatureDeContrat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<IdDeModele>,
    #[serde(skip_serializing_if = "Option::is_none")]
    partie: Option<Partie>,
    #[serde(skip_serializing_if = "Option::is_none")]
    numero: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sens: Option<Sens>,
}

/// Ce que montrent d'abord les cases d'un geste de l'ecran des modeles : le nom du special qu'on renomme.
#[derive(Debug, Clone, Default, Serialize)]
struct ValeursDuModele {
    #[serde(skip_serializing_if = "Option::is_none")]
    nom: Option<String>,
}

// Les mots de l'ecran, en constantes : un texte fautif ne compile pas (`Affiche::new`).
const OUVRIR: Affiche = Affiche::new("Ouvrir");
const PAR_DEFAUT: Affiche = Affiche::new("Choisir par défaut");
const SUPPRIMER: Affiche = Affiche::new("Supprimer");
const COPIER: Affiche = Affiche::new("Copier la base…");
const RENOMMER: Affiche = Affiche::new("Renommer…");
const AJOUTER: Affiche = Affiche::new("Ajouter un paragraphe…");
const ENREGISTRER: Affiche = Affiche::new("Enregistrer");
const APERCU: Affiche = Affiche::new("Aperçu PDF");
const TITRE_DU_PARAGRAPHE: Affiche = Affiche::new("Titre");
const ALINEAS: Affiche = Affiche::new("Alinéas");
const AJOUTER_UN_ALINEA: Affiche = Affiche::new("Ajouter un alinéa");
const TOUJOURS: Affiche = Affiche::new("Toujours");

/// Un geste de l'ecran sans case : il part avec sa charge.
fn sans_case(route: Route, sur: SurLeModele, libelle: Affiche) -> GesteDuModele {
    GesteDeRoute::avec_cases(route, sur, libelle, Vec::new(), ValeursDuModele::default(), SansCharge {})
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
    phrase: Phrase,
    natures: Vec<NatureDite>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ecran: Option<EcranDesModeles>,
}

/// Un article d'un modele : son numero, son titre, ce qu'il est (lu ou a ecrire : `CorpsDeLArticle`) et ce qu'on peut y
/// faire (le monter, sauf le premier ; le descendre, sauf le dernier ; le supprimer).
#[derive(Debug, Serialize)]
struct ArticleRendu {
    numero: usize,
    titre: String,
    #[serde(flatten)]
    corps: CorpsDeLArticle,
    gestes: Vec<GesteDuModele>,
}

/// CE QU'EST UN ARTICLE : d'un modele de base, ses alineas, qui se lisent seulement ; d'un special, ses cases (son titre,
/// ses alineas en rangees : chacun sa condition, au choix parmi les champs, et son texte), dessinees comme celles d'une
/// fiche, et le geste qui les enregistre (sa charge designe l'article, ce que disent les cases la complete, chacune sous
/// sa cle).
#[derive(Debug, Serialize)]
#[serde(untagged)]
enum CorpsDeLArticle {
    Lu { alineas: Vec<Alinea> },
    AEcrire { cases: Vec<CaseDeLaFiche<&'static str>>, ecrire: GesteDuModele },
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
    phrase: Phrase,
    nature: NatureDeContrat,
    id: Option<IdDeModele>,
    partie: Partie,
    nom: String,
    base: bool,
    articles: Vec<ArticleRendu>,
    manques: Vec<String>,
    en_francais: &'static str,
    champs: Vec<ChampDeModele>,
    base_changee: Option<Vec<String>>,
    engagements: Vec<EngagementAuChoix>,
    /// Les gestes qui lisent l'une et l'autre partie du modele : son contrat, son avenant.
    parties: Vec<GesteDuModele>,
    /// Ce qu'on peut faire sur un special : le renommer, lui ajouter un paragraphe a la fin.
    gestes: Vec<GesteDuModele>,
    /// Le geste qui fait l'apercu de cette partie ; l'engagement choisi parmi `engagements` le complete (aucun : le plus
    /// recent).
    apercu: GesteDuModele,
}

/// Ce que rend `apercu-modele-contrat`.
#[derive(Debug, Serialize)]
struct Apercu {
    phrase: Phrase,
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    chemin: std::path::PathBuf,
}

macro_rules! dites {
    ($($t:ty),*) => {$(
        impl Dite for $t {
            fn phrase(&self) -> &Phrase {
                &self.phrase
            }
        }
    )*};
}
dites!(Modeles, ModeleRendu, Apercu);

// ------------------------------------------------------------------ les gestes

/// Les natures dont la compagnie fait les contrats au jour de l'ecran : le contrat d'usage toujours ; celles d'un
/// permanent quand elle en propose ce jour-la, ou qu'un permanent de cette nature est du (son avenant :
/// `engagement::permanents::Permanents::natures`).
fn natures_de(ctx: &Contexte) -> R<Vec<std::sync::Arc<Nature>>> {
    let jour = ctx.aujourdhui;
    let p = engagement::permanents::Permanents::de(ctx, &socle::date::Periode::du_jour(jour))?;
    Ok(p.natures()?.into_iter().filter(|n| n.modele_contrat().is_some()).collect())
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
        let dit = |id: Option<&IdDeModele>, nom: String, manques: Vec<String>, base_changee: bool| -> R<ModeleDit> {
            let est_defaut = defaut == id;
            let pastille = match (manques.is_empty(), est_defaut) {
                (false, _) => Pastille::de(socle::ecran::EtatDEcran::Refuse, "incomplet"),
                (true, true) => Pastille::de(socle::ecran::EtatDEcran::Fait, "par défaut"),
                (true, false) if id.is_none() => Pastille::de(socle::ecran::EtatDEcran::Information, "le modèle du logiciel"),
                (true, false) => Pastille::de(socle::ecran::EtatDEcran::Information, "modèle spécial"),
            };
            let du_modele = || SurLeModele { nature: Some(nat.cle.clone()), id: id.cloned(), ..SurLeModele::default() };
            let ouvrir = sans_case(Route::ModeleContrat, SurLeModele { partie: Some(Partie::Contrat), ..du_modele() }, OUVRIR);
            let mut gestes = Vec::new();
            if !est_defaut {
                gestes.push(sans_case(Route::ModeleContratParDefaut, du_modele(), PAR_DEFAUT));
            }
            if let Some(id) = id {
                let disparait = ce_que_supprimer_dit(ctx, &nom, id, est_defaut)?;
                gestes.push(sans_case(Route::SupprimerModeleContrat, SurLeModele { id: Some(id.clone()), ..SurLeModele::default() }, SUPPRIMER).disant(disparait));
            }
            Ok(ModeleDit { id: id.cloned(), nom, base: id.is_none(), defaut: est_defaut, base_changee: base_changee.then_some(BASE_CHANGEE), manques, pastille, ouvrir, gestes })
        };
        let mut pieces = vec![dit(None, nom_de_base(&nat), manques_des_deux(&nat, &contrat, &avenant)?, false)?];
        for s in m.speciaux.iter().filter(|s| s.nature == nat.cle) {
            let change = s.contrat.base != contrat || s.avenant.base != avenant;
            pieces.push(dit(Some(&s.id), s.nom.clone(), manques_des_deux(&nat, &s.contrat.texte, &s.avenant.texte)?, change)?);
        }
        let nom = nat.nom_du_contrat();
        let copier = GesteDeRoute::avec_cases(Route::CopierModeleContrat, SurLeModele { nature: Some(nat.cle.clone()), ..SurLeModele::default() }, COPIER, vec![cle!(ChargeCopier, nom)], ValeursDuModele::default(), SansCharge {});
        sortie.push(NatureDite { nature: nat.cle.clone(), titre: capitale(&nom), libelle: nat.libelle.clone(), nom, gestes: vec![copier], modeles: pieces });
    }
    Ok(Modeles { phrase: socle::phrase!("{}Modèles de contrat spéciaux de la compagnie : {}.", devant, m.speciaux.len()), natures: sortie, ecran: None })
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
fn engagements_de(ctx: &Contexte, nature: NatureDeContrat) -> R<Vec<EngagementAuChoix>> {
    let mut v: Vec<EngagementAuChoix> = Vec::new();
    for o in engagement::stock::objets(&ctx.racine)? {
        if !o.est_valide() || o.est_importe() || o.est_non_actif() || o.carte().nature != Some(nature) {
            continue;
        }
        let (Some(salarie), Some(debut)) = (o.carte().salarie.as_ref(), engagement::debut_brut(&o.brut)) else { continue };
        let mois = o.premier_mois();
        v.push(EngagementAuChoix { engagement: EngagementVise { mois, salarie: salarie.clone(), debut }, nom: engagement::etats::qui_de(ctx, &o) });
    }
    v.sort_by(|a, b| b.engagement.debut.cmp(&a.engagement.debut));
    Ok(v)
}

/// LIRE UNE PARTIE D'UN MODELE : `{nature, id, partie}` (sans id : la base), ses articles et leurs alineas, ce qui lui
/// manque, les champs qu'un alinea insere ou pose en condition, ce qui a change dans sa base, les engagements de
/// l'apercu.
fn lire(ctx: &Contexte, c: ChargeLire) -> R<ModeleRendu> {
    let nat = c.nature.proprietes()?;
    let m = lus(ctx)?;
    let (nom, s) = designe(&m, &nat, c.id.as_ref())?;
    let texte = texte_de(&nat, s, c.partie)?;
    // Un texte sans paragraphe ne se lit pas : aucun geste n'en laisse (chacun verifie le modele), il ne vient que d'un
    // fichier ecrit a la main, et se refuse.
    let lu = fabrique_pdf::modele::lire_modele(&texte, &nom).map_err(|r| documents::fabrique::echec(r).reformuler(|p| format!("{} : le supprimer et copier la base à nouveau.", p)))?;
    // Ce qui designe le modele lu, et cette partie de lui, a ses gestes.
    let du_modele = || SurLeModele { nature: Some(nat.cle.clone()), id: c.id.clone(), ..SurLeModele::default() };
    let de_la_partie = || SurLeModele { partie: Some(c.partie), ..du_modele() };
    // Ce qui designe cette partie d'un special a un geste qui l'ecrit ; la base ne s'ecrit pas.
    let ecrite = |numero: Option<usize>, sens: Option<Sens>| s.map(|s| SurLeModele { id: Some(s.id.clone()), partie: Some(c.partie), numero, sens, ..SurLeModele::default() });
    let dernier = lu.articles.len();
    let champs = documents::contrat_ecrit::champs_des_modeles(&nat, c.partie == Partie::Avenant).map_err(|m| m.reclasser(Classe::Illisible))?;
    // La condition d'un alinea se choisit parmi les champs ; vide, il s'ecrit toujours.
    let conditions: Vec<Choix> = std::iter::once(Choix::de("", TOUJOURS.texte())).chain(champs.iter().map(|ch| Choix::de(&ch.marqueur, ch.quoi))).collect();
    let articles = lu
        .articles
        .iter()
        .enumerate()
        .map(|(n, (titre, alineas))| {
            let numero = n + 1;
            let alineas: Vec<Alinea> = alineas
                .iter()
                .map(|a| match fabrique_pdf::modele::condition(a) {
                    Some((si, t)) => Alinea { si: Some(si.to_string()), texte: t.to_string() },
                    None => Alinea { si: None, texte: a.clone() },
                })
                .collect();
            let corps = match ecrite(Some(numero), None) {
                None => CorpsDeLArticle::Lu { alineas },
                Some(sur) => CorpsDeLArticle::AEcrire {
                    cases: vec![
                        CaseDeLaFiche::simple(cle!(ChargeEcrireParagraphe, titre), TITRE_DU_PARAGRAPHE, Forme::Texte, ValeurDeCase::de(titre)),
                        CaseDeLaFiche::de_rangees(
                            cle!(ChargeEcrireParagraphe, alineas),
                            ALINEAS,
                            ValeurDeCase::de(&alineas),
                            PARTIES_D_UN_ALINEA.iter().map(|p| PartieDeLaFiche::de(p, (p.forme == Forme::ChoixRendu).then(|| conditions.clone()))).collect(),
                            AJOUTER_UN_ALINEA,
                        ),
                    ],
                    ecrire: GesteDeRoute::avec_cases(Route::EcrireParagrapheContrat, sur, ENREGISTRER, vec![cle!(ChargeEcrireParagraphe, titre), cle!(ChargeEcrireParagraphe, alineas)], ValeursDuModele::default(), SansCharge {}),
                },
            };
            let monter = ecrite(Some(numero), Some(Sens::Monter)).filter(|_| numero > 1).map(|sur| sans_case(Route::DeplacerParagrapheContrat, sur, Sens::Monter.bouton()));
            let descendre = ecrite(Some(numero), Some(Sens::Descendre)).filter(|_| numero < dernier).map(|sur| sans_case(Route::DeplacerParagrapheContrat, sur, Sens::Descendre.bouton()));
            let supprimer = ecrite(Some(numero), None).map(|sur| sans_case(Route::SupprimerParagrapheContrat, sur, SUPPRIMER));
            ArticleRendu { numero, titre: titre.clone(), corps, gestes: [monter, descendre, supprimer].into_iter().flatten().collect() }
        })
        .collect();
    let parties = <Partie as socle::liste::ListeFermee>::TOUTES.iter().map(|p| sans_case(Route::ModeleContrat, SurLeModele { partie: Some(*p), ..du_modele() }, p.bouton())).collect();
    let mut gestes = Vec::new();
    if let Some(s) = s {
        let renommer = SurLeModele { id: Some(s.id.clone()), ..SurLeModele::default() };
        gestes.push(GesteDeRoute::avec_cases(Route::RenommerModeleContrat, renommer, RENOMMER, vec![cle!(ChargeRenommer, nom)], ValeursDuModele { nom: Some(s.nom.clone()) }, SansCharge {}));
    }
    // Un paragraphe s'ajoute a la fin ; il se deplace ensuite.
    if let Some(sur) = ecrite(Some(dernier + 1), None) {
        gestes.push(GesteDeRoute::avec_cases(Route::AjouterParagrapheContrat, sur, AJOUTER, vec![cle!(ChargeAjouter, titre)], ValeursDuModele::default(), SansCharge {}));
    }
    let apercu = sans_case(Route::ApercuModeleContrat, de_la_partie(), APERCU);
    let dits = manques(&nat, c.partie, &texte)?;
    let quoi = if c.partie == Partie::Avenant { format!("{}, son avenant", nom) } else { nom };
    let phrase = if dits.is_empty() { socle::phrase!("{} : complet.", quoi) } else { socle::phrase!("{} : {}", quoi, dits.join(" ")) };
    // La base n'est copiee de rien : rien ne se compare, `None`.
    let base_changee = s.map(|s| texte_de_base(&nat, c.partie).map(|b| ce_qui_a_change(&s.partie(c.partie).base, &b))).transpose()?;
    let engagements = engagements_de(ctx, nat.cle)?;
    Ok(ModeleRendu {
        phrase,
        nature: nat.cle.clone(),
        id: c.id,
        partie: c.partie,
        // Le nom qu'un special porte (celui que l'on renomme) ; la base, comme la liste la nomme.
        nom: match s {
            Some(s) => s.nom.clone(),
            None => nom_de_base(&nat),
        },
        base: s.is_none(),
        articles,
        manques: dits,
        en_francais: documents::contrat_ecrit::EN_FRANCAIS,
        champs,
        base_changee,
        engagements,
        parties,
        gestes,
        apercu,
    })
}

/// L'APERCU D'UN MODELE : `{nature, id, partie, engagement}`, le PDF du contrat de cet engagement (le plus recent sans
/// lui) fait avec ce modele, ou d'un avenant n° 1 a ce contrat, date du jour, qui cite son premier article sans le
/// changer ; dans `sorties/apercus/`, rien d'autre ne s'ecrit, rien ne part. Un modele qui ne fait pas ce contrat se
/// refuse comme au depart, et dit pourquoi.
fn apercu(ctx: &Contexte, c: ChargeApercu) -> R<Apercu> {
    let nat = c.nature.proprietes()?;
    let m = lus(ctx)?;
    let (nom, s) = designe(&m, &nat, c.id.as_ref())?;
    let vise = match c.engagement {
        Some(v) => v,
        None => engagements_de(ctx, nat.cle)?
            .into_iter()
            .next()
            .map(|e| e.engagement)
            .ok_or_else(|| refus(format!("La compagnie n'a encore aucun engagement validé de {} : l'aperçu remplit le modèle avec l'un d'eux.", nat.nom_du_contrat())))?,
    };
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let (_, paies) = documents::ecrire_documents::paies_aux_numeros_declares(ctx, vise.mois, None, &departs)?;
    let p = paies
        .into_iter()
        .find(|p| p.salarie.id == vise.salarie && p.engagement.contrat().debut() == vise.debut)
        .ok_or_else(|| refus(format!("Aucun engagement validé de {} du {} en {} : en choisir un de la liste.", vise.salarie, vise.debut.jj_mm_aaaa(), vise.mois.etiquette())))?;
    let modele = modele_lu(nom.clone(), s.is_none(), &texte_de(&nat, s, Partie::Contrat)?, &texte_de(&nat, s, Partie::Avenant)?)?;
    let ent = documents::avenant::ent_du_mois(ctx, vise.mois);
    let d = documents::DossierDeTravail::Apercus.sous(&ctx.racine);
    std::fs::create_dir_all(&d).map_err(|e| disque(&d, e))?;
    let (sorte, quoi) = match c.partie {
        Partie::Contrat => ("contrat", "le contrat"),
        Partie::Avenant => ("avenant", "un avenant au contrat"),
    };
    // Le fichier porte l'id du special, ou dit la base.
    let modele_du_fichier = match &c.id {
        Some(id) => id.texte(),
        None => "base",
    };
    let chemin = d.join(format!("modele-{}-{}-{}.pdf", nat.cle, modele_du_fichier, sorte));
    match c.partie {
        Partie::Contrat => documents::contrat::ecrire_contrat(ctx, std::slice::from_ref(&p), &[modele], &ent, &chemin, &[])?,
        Partie::Avenant => {
            // Rien ne change : l'apercu cite le premier article du contrat tel qu'il est (`modification_d_apercu`).
            let v = documents::contrat::valeurs_de_paie(ctx, &ent, &p)?;
            let contrat = modele.lu.decoupe(&nom).map_err(|m| m.reclasser(Classe::Illisible))?;
            let x = documents::avenant::modification_d_apercu(&contrat, &v).map_err(|m| m.reclasser(Classe::Refuse))?;
            let modifs = [fabrique_pdf::forme::Modification { numero: Some(x.numero), article: x.article, avant: x.avant, apres: x.apres, phrase: None }];
            documents::contrat::ecrire_avenant(ctx, &p, &ent, &chemin, &modifs, engagement::numeros::NumeroDAvenant::premier(), p.engagement.contrat().debut(), ctx.aujourdhui, ctx.aujourdhui, &modele, false)?
        }
    }
    Ok(Apercu { phrase: socle::phrase!("{}, {} de {} du {} : l'aperçu est fait, rien n'est parti.", nom, quoi, p.engagement.nom, p.engagement.contrat().debut().jj_mm_aaaa()), chemin })
}

/// Le nom d'un special : dit, et qu'aucun autre special (que `sauf`) ne porte.
fn nom_libre(m: &ModelesDeContrat, sauf: Option<&IdDeModele>, nom: &str) -> R<String> {
    let nom = une_ligne(nom);
    if nom.is_empty() {
        return Err(erreur(Classe::Manque, "Le modèle spécial a besoin d'un nom. Rien n'est écrit."));
    }
    if m.speciaux.iter().any(|s| s.nom == nom && Some(&s.id) != sauf) {
        return Err(refus(format!("La compagnie a déjà un modèle « {} » : lui donner un autre nom. Rien n'est écrit.", nom)));
    }
    Ok(nom)
}

/// COPIER LA BASE D'UNE NATURE EN UN MODELE SPECIAL : `{nature, nom}`, son contrat et son avenant ; chacun garde le texte
/// de la base dont il part.
fn copier(ctx: &Contexte, c: ChargeCopier) -> R<Modeles> {
    let nat = c.nature.proprietes()?;
    let mut m = lus(ctx)?;
    let nom = nom_libre(&m, None, &c.nom)?;
    let partie = |p| texte_de_base(&nat, p).map(|t| Texte { base: t.clone(), texte: t });
    let (contrat, avenant) = (partie(Partie::Contrat)?, partie(Partie::Avenant)?);
    // Un id ne sert qu'une fois : ni un special present, ni un supprime.
    let n = m.speciaux.iter().map(|s| &s.id).chain(m.supprimes.iter()).fold(0, |n, id| n.max(id.numero())) + 1;
    m.speciaux.push(Special { id: IdDeModele::de(n), nom: nom.clone(), nature: nat.cle.clone(), contrat, avenant });
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
fn choisi_par(ctx: &Contexte, id: &IdDeModele) -> R<Vec<String>> {
    Ok(engagement::stock::objets(&ctx.racine)?
        .iter()
        .filter(|o| o.carte().modele_contrat.as_ref() == Some(id) || o.avenants().iter().any(|a| a.termes.modele_contrat.as_ref() == Some(id)))
        .map(|o| engagement::etats::qui_de(ctx, o))
        .collect())
}

/// CE QUE LA BOITE DE `supprimer-modele-contrat` DIT avant qu'on supprime le special `id` (`nom`) : ce qui disparait, ce
/// que devient le defaut de sa nature s'il l'est (`est_defaut`), et les engagements qui l'ont choisi (`choisi_par`).
fn ce_que_supprimer_dit(ctx: &Contexte, nom: &str, id: &IdDeModele, est_defaut: bool) -> R<Vec<String>> {
    let mut dit = vec![format!("Le modèle « {} » disparaît, son contrat et son avenant.", nom)];
    if est_defaut {
        dit.push("C'est le modèle par défaut de sa nature : le modèle du logiciel le redevient.".to_string());
    }
    let qui = choisi_par(ctx, id)?;
    if !qui.is_empty() {
        dit.push(format!("{} engagement(s) l'ont choisi ({}) : leur contrat suit alors le modèle par défaut de sa nature ; un contrat déjà parti garde son texte.", qui.len(), qui.join(", ")));
    }
    Ok(dit)
}

/// SUPPRIMER UN MODELE SPECIAL : `{id}` ; s'il etait le defaut de sa nature, la base le redevient ; les engagements qui
/// l'avaient choisi suivent le defaut de leur nature (`supprimes`, `du_contrat`), et la phrase les nomme. Un contrat
/// deja parti garde son texte (`figer`).
fn supprimer(ctx: &Contexte, c: ChargeId) -> R<Modeles> {
    let mut m = lus(ctx)?;
    let s = special(&m, &c.id)?;
    let (nom, id) = (s.nom.clone(), s.id.clone());
    let qui = choisi_par(ctx, &id)?;
    m.speciaux.retain(|s| s.id != id);
    m.defauts.retain(|_, d| *d != id);
    m.supprimes.push(id);
    ecrire(ctx, &m)?;
    let suite = match qui.len() {
        0 => String::new(),
        n => format!("{} engagement(s) l'avaient choisi ({}) : leur contrat suit désormais le modèle par défaut de sa nature ; un contrat déjà parti garde son texte. ", n, qui.join(", ")),
    };
    modeles(ctx, &format!("Modèle « {} » supprimé. {}", nom, suite))
}

/// CHOISIR LE MODELE PAR DEFAUT D'UNE NATURE : `{nature, id}` ; sans id, la base.
fn par_defaut(ctx: &Contexte, c: ChargeDefaut) -> R<Modeles> {
    let nat = c.nature.proprietes()?;
    let mut m = lus(ctx)?;
    let devant = match &c.id {
        Some(id) => {
            let (nom, _) = designe(&m, &nat, Some(id))?;
            let id = special(&m, id)?.id.clone();
            m.defauts.insert(nat.cle.clone(), id);
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
pub(crate) fn au_choix(ctx: &Contexte, nature: NatureDeContrat) -> R<Vec<ModeleAuChoix>> {
    let m = lus(ctx)?;
    let siens: Vec<&Special> = m.speciaux.iter().filter(|s| s.nature == nature).collect();
    if siens.is_empty() {
        return Ok(Vec::new());
    }
    let defaut = match m.defauts.get(&nature).map(|id| special(&m, id)).transpose()? {
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
    id: Option<IdDeModele>,
    nom: String,
}

impl ModeleAuChoix {
    /// Le modele tel que la case `modele_contrat` d'une carte l'offre : son id, le code vide pour le defaut de la nature.
    pub(crate) fn en_choix(&self) -> crate::ouverture::Choix {
        match &self.id {
            Some(id) => crate::ouverture::Choix::de(id.texte(), &self.nom),
            // Le defaut de la nature : le choix vide, qui n'ecrit rien sur la carte.
            None => crate::ouverture::Choix::de("", &self.nom),
        }
    }
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
