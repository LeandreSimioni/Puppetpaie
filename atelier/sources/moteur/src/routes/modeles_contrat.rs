//! LES MODELES DE CONTRAT DE LA COMPAGNIE : ce que Configuration › Modeles de contrat montre et fait.
//!
//! Chaque nature de contrat a son modele livre (`contrat_ecrit` de `contrats-base.json`) ; une compagnie qui a le sien le
//! pose dans son dossier `gabarits/`, sous le meme nom : il est pris a la place du livre (`socle::fichiers::modele_de`),
//! et sauvegarde avec elle. Un modele propre se verifie avant d'etre pose : sa grammaire, ses champs (un champ que le
//! logiciel ne connait pas ne ferait aucun contrat), et les mentions que la loi exige d'un contrat ecrit (code du travail
//! L1242-12 et R1221-34, `documents::contrat_ecrit::refuser_les_mentions_absentes`) ; s'il manque quelque chose, rien n'est
//! pose, et le refus dit quoi. Les clauses particulieres de la compagnie (`clauses_particulieres` de sa fiche) s'ecrivent
//! ici une fois et se reprennent dans chaque contrat.

use droit::fiche::Lue as _;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::routes::catalogue::{Acte, Affiche, Champ, Forme, Fiche, Refus, MANQUE, OPERATION};
use crate::routes::catalogue::Route;
use crate::routes::enveloppe::{en_json, Charge, Dite, Porte, Rendu};
use documents::contrat_ecrit::ChampDeModele;
use droit::contexte::Contexte;
use droit::natures::Nature;
use socle::echec::{erreur, Classe, Echec, R};
use socle::fichiers::dossier_du_code;
use socle::ecran::Pastille;
use documents::marqueurs::{Marqueur, ModeleLu, Valeurs, DE_L_AVENANT, DU_REMPLACEMENT};
use crate::routes::catalogue::{Ecran, Place};

const NATURE: Champ = Champ { nom: "nature", libelle: Affiche::new("Nature du contrat"), forme: Forme::Texte, sens: "la nature du contrat : cddu_spectacle, cdd, cdi…", avance: false };
const QUOI: Champ = Champ { nom: "quoi", libelle: Affiche::new("Contrat ou avenant"), forme: Forme::Texte, sens: "contrat ou avenant", avance: false };

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::ModelesContrat);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::ModelesContrat,
        geste: "Lister les modèles de contrat et d'avenant, livrés ou propres.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "natures (chacune : ses modèles, livrés ou propres), champs permis, clauses de la compagnie",
        refus: &[OPERATION],
        acte: Acte::Lit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::ModeleContratLivre,
        geste: "Lire le texte d'un modèle livré, pour en partir.",
        charge: &[NATURE, QUOI],
        exemple: r#"{"nature":"cddu_spectacle","quoi":"contrat"}"#,
        rend: "texte",
        refus: &[OPERATION, MANQUE],
        acte: Acte::Lit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::PoserModeleContrat,
        geste: "Poser le modèle de contrat ou d'avenant propre à la compagnie.",
        charge: &[NATURE, QUOI, Champ { nom: "texte", libelle: Affiche::new("Modèle"), forme: Forme::Texte, sens: "le modèle entier (le format des modèles livrés : TITRE:, ARTICLE:…)", avance: false }],
        exemple: r#"{"nature":"cddu_spectacle","quoi":"avenant","texte":"TITRE: Avenant n° {{avenant.numero}}\n\nMODIFICATION:\nL'article {{modification.numero}} est désormais rédigé ainsi : « {{modification.apres}} »\n\nARTICLE: Le reste\nLe reste du contrat est inchangé.\n"}"#,
        rend: "les modèles, comme modeles-contrat",
        refus: &[Refus { classe: Classe::Refuse, corriger: "le modèle : la mention obligatoire ou le champ que la phrase nomme ; rien n'est posé" }, MANQUE],
        acte: Acte::Ecrit,
        avance: true,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::RetirerModeleContrat,
        geste: "Remettre le modèle livré à la place de celui de la compagnie.",
        charge: &[NATURE, QUOI],
        exemple: r#"{"nature":"cddu_spectacle","quoi":"avenant"}"#,
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
        Route::ModeleContratLivre => en_json(livre(charge.lire()?)?),
        Route::PoserModeleContrat => en_json(poser(ctx, charge.lire()?)?),
        Route::RetirerModeleContrat => en_json(retirer(ctx, charge.lire()?)?),
        Route::ClausesParticulieres => en_json(clauses(ctx, charge.lire()?)?),
        autre => Err(super::enveloppe::route_inconnue(autre.nom())),
    }
}

/// Un modele de contrat, ou d'avenant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum QuoiModele {
    Contrat,
    Avenant,
}

impl QuoiModele {
    const LES_DEUX: [QuoiModele; 2] = [QuoiModele::Contrat, QuoiModele::Avenant];

    fn mot(self) -> &'static str {
        match self {
            QuoiModele::Contrat => "contrat",
            QuoiModele::Avenant => "avenant",
        }
    }
}

/// La charge qui designe un modele : la nature, et contrat ou avenant.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeModele {
    nature: String,
    quoi: QuoiModele,
}

/// La charge de `poser-modele-contrat` : le modele designe, et son texte entier.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargePoser {
    nature: String,
    quoi: QuoiModele,
    texte: String,
}

/// La charge de `clauses-particulieres` : le texte ; vide, aucune.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeClauses {
    texte: String,
}

/// Un modele d'une nature : le livre ou celui de la compagnie, son refus s'il en a un, son texte s'il est le sien.
#[derive(Debug, Serialize)]
struct ModeleDit {
    quoi: QuoiModele,
    nom: String,
    propre: bool,
    refus: Option<String>,
    texte: Option<String>,
    libelle: &'static str,
    bouton: &'static str,
    pastille: Pastille,
}

/// Une nature et ses modeles, et les champs qu'ils peuvent citer.
#[derive(Debug, Serialize)]
struct NatureDite {
    nature: String,
    libelle: String,
    nom: String,
    modeles: Vec<ModeleDit>,
    champs: Vec<ChampDeModele>,
    champs_titre: String,
}

/// Ce que l'ecran des modeles de contrat dit (`ecran` de `modeles-contrat`).
#[derive(Debug, Serialize)]
struct EcranDesModeles {
    intro: &'static str,
    clauses: &'static str,
    comment: [String; 4],
    avenant: String,
}

/// Ce que rendent `modeles-contrat`, `poser-modele-contrat` et `retirer-modele-contrat`.
#[derive(Debug, Serialize)]
struct Modeles {
    phrase: String,
    natures: Vec<NatureDite>,
    dossier: String,
    champs_de_l_avenant: Vec<String>,
    clauses_particulieres: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ecran: Option<EcranDesModeles>,
}

/// Ce que rend `modele-contrat-livre`.
#[derive(Debug, Serialize)]
struct ModeleLivre {
    phrase: String,
    nom: String,
    texte: String,
    libelle: String,
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

impl Dite for ModeleLivre {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

impl Dite for ClausesEcrites {
    fn phrase(&self) -> &str {
        self.phrase
    }
}

fn refus(m: impl Into<String>) -> Echec {
    erreur(Classe::Refuse, m)
}

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

/// Le nom du modele livre de `quoi` (« contrat » ou « avenant ») pour la nature `cle`.
fn nom_du_modele(cle: &str, quoi: QuoiModele) -> R<(std::sync::Arc<Nature>, String)> {
    let nat = droit::natures::nature(cle).map_err(Echec::from)?;
    let nom = match quoi {
        QuoiModele::Contrat => nat.modele_contrat(),
        QuoiModele::Avenant => nat.modele_avenant(),
    };
    let nom = nom.ok_or_else(|| refus(format!("La nature {} n'a pas de modèle de {}.", nat.libelle, quoi.mot())))?;
    Ok((nat, nom))
}

/// Le modele de la compagnie, s'il est pose dans son dossier `gabarits/`.
fn propre(ctx: &Contexte, nom: &str) -> Option<PathBuf> {
    Some(ctx.gabarits().join(nom)).filter(|p| p.is_file())
}

/// CE QUE LA PAGE MONTRE : pour chaque nature, le modele du contrat et celui de l'avenant (le livre, ou celui de la
/// compagnie, et s'il est bon), les champs qu'un modele peut citer, ceux que la loi exige, le dossier ou poser le sien ;
/// et les clauses particulieres de la compagnie.
/// `devant` : ce que la phrase dit avant les modeles (le geste qui vient d'etre fait).
fn modeles(ctx: &Contexte, devant: &str) -> R<Modeles> {
    let mut sortie = Vec::new();
    for nat in natures_de(ctx)? {
        let mut pieces = Vec::new();
        for quoi in QuoiModele::LES_DEUX {
            let (_, nom) = nom_du_modele(&nat.cle, quoi)?;
            let a_soi = propre(ctx, &nom);
            let (verdict, texte) = match &a_soi {
                Some(p) => match std::fs::read_to_string(p) {
                    Ok(t) => (verifier(&nat, quoi, &nom, &t).err(), Some(t)),
                    Err(e) => (Some(format!("Ce modèle ne se lit pas ({}) : le poser de nouveau.", e)), None),
                },
                None => (None, None),
            };
            // CE QUE LA PASTILLE DIT, et sa couleur (`couleur_de_l_etat`) : le modele de la compagnie, bon ou refuse ; le
            // modele du logiciel, une information.
            let (etat, mots) = match (&a_soi, &verdict) {
                (Some(_), Some(_)) => ("refuse", "le modèle de la compagnie, refusé"),
                (Some(_), None) => ("fait", "le modèle de la compagnie"),
                (None, _) => ("info", "le modèle du logiciel"),
            };
            pieces.push(ModeleDit {
                quoi,
                nom,
                propre: a_soi.is_some(),
                refus: verdict,
                texte,
                libelle: match quoi {
                    QuoiModele::Contrat => "Le contrat",
                    QuoiModele::Avenant => "L'avenant",
                },
                bouton: if a_soi.is_some() { "Modifier le modèle" } else { "Écrire le modèle" },
                pastille: Pastille::de(etat, mots),
            });
        }
        let mut champs = documents::contrat_ecrit::champs_des_modeles(&nat).map_err(|m| erreur(Classe::Illisible, m))?;
        champs.extend(DU_REMPLACEMENT.iter().map(|c| ChampDeModele { marqueur: c.to_string(), quoi: "le contrat remplacé (bloc ANNULE: seulement)", obligatoire: false, texte: None, si_dite: false }));
        let champs_titre = format!("Ses champs ({})", champs.len());
        sortie.push(NatureDite { nature: nat.cle.clone(), libelle: nat.libelle.clone(), nom: nat.nom_du_contrat().to_string(), modeles: pieces, champs, champs_titre });
    }
    Ok(Modeles {
        phrase: format!("{}{}", devant, dire_les_modeles(&sortie)),
        natures: sortie,
        dossier: ctx.gabarits().to_string_lossy().into_owned(),
        champs_de_l_avenant: DE_L_AVENANT.iter().map(|c| c.to_string()).collect(),
        clauses_particulieres: ctx.entreprise.clauses_particulieres.portee().map(String::from),
        ecran: None,
    })
}

/// Le texte du modele livre, pour partir de lui.
fn livre(c: ChargeModele) -> R<ModeleLivre> {
    let (nat, nom) = nom_du_modele(&c.nature, c.quoi)?;
    let chemin = dossier_du_code().join("modeles").join(&nom);
    let texte = std::fs::read_to_string(&chemin).map_err(|e| erreur(Classe::Disque, socle::echec::du_disque(&chemin, &e)))?;
    let de = match c.quoi {
        QuoiModele::Contrat => "du contrat",
        QuoiModele::Avenant => "de l'avenant",
    };
    let libelle = format!("{} {}", de, nat.libelle);
    Ok(ModeleLivre { phrase: format!("Modèle livré {}.", libelle), nom, texte, libelle })
}

/// UN MODELE PROPRE SE VERIFIE AVANT D'ETRE POSE : sa grammaire (`fabrique_pdf::modele::lire_modele`), chacun de ses
/// champs connu du logiciel (un champ inconnu arreterait chaque contrat), et, pour un contrat, chaque mention que la loi
/// exige de sa nature.
fn verifier(nat: &Nature, quoi: QuoiModele, nom: &str, texte: &str) -> Result<(), String> {
    let lu = ModeleLu::lire(texte, nom)?;
    let m = lu.decoupe(nom).map_err(|r| format!("Ce modèle ne se lit pas : {}", r))?;
    let avenant = quoi == QuoiModele::Avenant;
    if avenant && m.modification.is_empty() {
        return Err("Ce modèle d'avenant n'a pas de bloc MODIFICATION: : c'est lui qui dit, article par article, ce qui change.".into());
    }
    let permis = |c: &Marqueur| {
        documents::contrat_ecrit::CHAMPS.iter().any(|(k, _, _)| k == c) || DU_REMPLACEMENT.contains(c) || (avenant && DE_L_AVENANT.contains(c))
    };
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
        return Err(format!("Ce modèle cite des champs qu'un modèle {} ne porte pas : {}. Les corriger d'après la liste des champs, ou les écrire en texte.", if avenant { "d'avenant" } else { "de contrat" }, hors.join(", ")));
    }
    if !avenant {
        // La verification ne sait pas encore ce que chaque contrat dira : une mention due « si elle est dite » (la
        // personne remplacee) se demande comme si elle l'etait.
        let toutes: Valeurs = documents::contrat_ecrit::mentions(nat)?.into_iter().map(|(c, _, _)| (c, "dite".to_string())).collect();
        documents::contrat_ecrit::refuser_les_mentions_absentes(&std::path::Path::new("gabarits").join(nom), &lu, nat, &toutes)?;
    }
    Ok(())
}

/// POSER LE MODELE DE LA COMPAGNIE : `{nature, quoi, texte}`. Verifie d'abord ; l'ancien modele propre, s'il y en a un,
/// est garde a cote (`.avant`).
fn poser(ctx: &Contexte, c: ChargePoser) -> R<Modeles> {
    let (nat, nom) = nom_du_modele(&c.nature, c.quoi)?;
    let texte = c.texte.as_str();
    verifier(&nat, c.quoi, &nom, texte).map_err(|m| refus(format!("{} Rien n'est posé.", m.trim_end_matches(" Rien n'est écrit."))))?;
    let dossier = ctx.gabarits();
    std::fs::create_dir_all(&dossier).map_err(|e| erreur(Classe::Disque, socle::echec::du_disque(&dossier, &e)))?;
    let cible = dossier.join(&nom);
    if cible.is_file() {
        let ancien = dossier.join(format!("{}.avant", nom));
        std::fs::rename(&cible, &ancien).map_err(|e| erreur(Classe::Disque, socle::echec::du_disque(&cible, &e)))?;
    }
    std::fs::write(&cible, texte).map_err(|e| erreur(Classe::Disque, socle::echec::du_disque(&cible, &e)))?;
    modeles(ctx, "Modèle de la compagnie posé. ")
}

/// RETIRER LE MODELE DE LA COMPAGNIE : le livre reprend sa place ; le sien est garde a cote (`.avant`).
fn retirer(ctx: &Contexte, c: ChargeModele) -> R<Modeles> {
    let (_, nom) = nom_du_modele(&c.nature, c.quoi)?;
    if let Some(p) = propre(ctx, &nom) {
        let ancien = ctx.gabarits().join(format!("{}.avant", nom));
        std::fs::rename(&p, &ancien).map_err(|e| erreur(Classe::Disque, socle::echec::du_disque(&p, &e)))?;
    }
    modeles(ctx, "Modèle de la compagnie retiré : le modèle livré revient. ")
}

/// LES CLAUSES PARTICULIERES DE LA COMPAGNIE : `{texte}`, reprises dans chaque contrat a partir d'aujourd'hui ;
/// l'engagement peut dire les siennes (« Plus d'options »). Vide : aucune.
fn clauses(ctx: &Contexte, c: ChargeClauses) -> R<ClausesEcrites> {
    let texte = c.texte.trim();
    let clauses = if texte.is_empty() { None } else { Some(texte) };
    crate::routes::fiches::poser_entreprise(ctx, droit::fiche::ChangementEntreprise::clauses_particulieres(clauses.map(str::to_string)), Some(ctx.aujourdhui))?;
    Ok(ClausesEcrites { phrase: "Clauses particulières enregistrées." })
}

/// Ce que `modeles` rend, en une phrase : les modeles de chaque nature, et ceux que la compagnie a poses.
fn dire_les_modeles(natures: &[NatureDite]) -> String {
    let modeles: Vec<&ModeleDit> = natures.iter().flat_map(|n| &n.modeles).collect();
    let propres = modeles.iter().filter(|m| m.propre).count();
    format!("Modèles de contrat et d'avenant : {} ; propres à la compagnie : {}.", modeles.len(), propres)
}

/// CE QUE L'ECRAN DES MODELES DE CONTRAT DIT (`ecran` de `modeles-contrat`) : a quoi sert un modele, les clauses
/// particulieres, comment ecrire celui de la compagnie, et ce que cite un modele d'avenant.
fn ecran_des_modeles(m: &Modeles) -> EcranDesModeles {
    EcranDesModeles {
        intro: "Chaque contrat que le logiciel fait signer suit un modèle. Celui du logiciel porte déjà ce que la loi exige d'un contrat écrit, rempli de lui-même depuis l'engagement et la configuration. Si la compagnie a son propre contrat, elle le pose ici : le logiciel le prend à la place du sien, pour tous les contrats de cette sorte.",
        clauses: "Ce qu'elle ajoute à chacun de ses contrats (confidentialité, image, déplacements…). Un engagement peut dire les siennes à la place, dans « Plus d'options ».",
        comment: [
            "Partir du modèle du logiciel (« Écrire le modèle »), ou poser un fichier texte déjà écrit.".to_string(),
            "Changer le texte comme on veut. Chaque article commence par une ligne « ARTICLE: son titre » ; une ligne vide sépare deux paragraphes ; **texte** met en gras ; une ligne qui commence par # est une note, qui ne s'imprime pas.".to_string(),
            "Les champs entre accolades ({{salarie.nom_complet}}…) se remplissent d'eux-mêmes : leur liste est sous chaque contrat. Ceux que la loi exige doivent rester ; sans l'un d'eux, le modèle est refusé, et le refus dit lequel.".to_string(),
            format!("« Poser ce modèle » : il est vérifié, puis rangé dans le dossier de la compagnie ({}), et sauvegardé avec elle. L'ancien est gardé à côté.", m.dossier),
        ],
        avenant: format!("Un modèle d'avenant cite aussi : {}.", m.champs_de_l_avenant.join(", ")),
    }
}
