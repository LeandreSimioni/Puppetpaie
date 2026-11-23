//! Configuration › Signature & tampon : la signature, le cachet et le logo de la compagnie, poses ou dessines. La route
//! `identite` rend l'ecran tel qu'on le voit (`ecran::EcranRendu`) : un rang par image, ce qui s'y regarde (l'image que
//! rend `identite-image`) et ses gestes (`poser-identite`, `retirer-identite`).

use std::path::{Path, PathBuf};

use crate::catalogue::{Acte, Affiche, Champ, Forme, Ecran, Fiche, Place, Refus, FORME, MANQUE, OPERATION};
use crate::catalogue::Route;
use crate::ecran::{EcranRendu, RangDeLEcran};
use crate::Encart;
use droit::fiche::CaseDeFiche;
use crate::enveloppe::{en_json, Charge, Dite, Phrase, Porte, Rendu};
use crate::fiches::{aujourd_hui, creer_dossier, ecrire, refus};
use crate::imprevus::GesteDeRoute;
use crate::ouverture::{DocumentMontre, Identite, ImageIdentite};
use droit::contexte::Contexte;
use socle::echec::R;
use socle::grandeurs::Octets;
use documents::SorteDImage;
use fabrique_pdf::alleger::FormatDImage;

const QUOI_IDENTITE: Champ = Champ { nom: "quoi", libelle: Affiche::new("Signature, cachet ou logo"), forme: Forme::Texte, sens: "signature, cachet ou logo", avance: false };

/// Les types de media d'une image qui se pose : ceux que le logiciel reconnait (`FormatDImage`).
const IMAGES_ADMISES: &[&str] = &[FormatDImage::Png.mime(), FormatDImage::Jpeg.mime(), FormatDImage::Svg.mime()];

const IDENTITE_REND: &str = "phrase, encarts (ce qui se pose, et comment), rangs : la signature, le tampon et le logo, chacun son titre, ce qu'il dit, sa pastille et son état, l'image qui s'y regarde (montre : la route et la charge qui la rendent, l'adresse de la fenêtre) et ses gestes";

/// Ou ces routes se voient a l'ecran (une place par module).
const PLACE: Place = Place::Identite;

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::Identite,
        geste: "Dire la signature, le cachet et le logo de la compagnie.",
        charge: &[],
        exemple: r#"{}"#,
        rend: IDENTITE_REND,
        refus: &[OPERATION],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Lit,
        phrase: 0,
        ecran: Ecran::Rendu(PLACE),
    },
    Fiche {
        route: Route::IdentiteImage,
        geste: "Montrer la signature, le cachet ou le logo : l'image posée, ou le tampon que le logiciel dessine.",
        charge: &[QUOI_IDENTITE],
        exemple: r#"{"quoi":"cachet"}"#,
        rend: "phrase, media (son type), image (en base64)",
        refus: &[OPERATION, FORME],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Lit,
        phrase: 0,
        ecran: Ecran::Lecture,
    },
    Fiche {
        route: Route::PoserIdentite,
        geste: "Poser la signature, le cachet ou le logo (une image PNG, JPEG ou SVG).",
        charge: &[
            QUOI_IDENTITE,
            Champ { nom: "contenu", libelle: Affiche::new("Image"), forme: Forme::Fichier(IMAGES_ADMISES), sens: "l'image en base64 (une adresse data: aussi)", avance: false },
            Champ { nom: "poids", libelle: Affiche::new("Poids visé en Ko"), forme: Forme::Nombre, sens: "le poids visé de l'image dans chaque document, en Ko (le geste de la route identite en propose un)", avance: false },
        ],
        exemple: r#"{"quoi":"logo","contenu":"PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIxIiBoZWlnaHQ9IjEiLz4=","poids":20}"#,
        rend: IDENTITE_REND,
        refus: &[Refus { classe: socle::echec::Classe::Refuse, corriger: "une image lisible (PNG, JPEG, SVG), de 2 Mo au plus" }, FORME, MANQUE],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: Ecran::Geste(PLACE),
    },
    Fiche {
        route: Route::RetirerIdentite,
        geste: "Retirer la signature, le cachet ou le logo posé.",
        charge: &[QUOI_IDENTITE],
        exemple: r#"{"quoi":"logo"}"#,
        rend: IDENTITE_REND,
        refus: &[OPERATION, MANQUE],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: Ecran::Geste(PLACE),
    },
];

/// Une route de ce module, branchee sur l'enveloppe.
pub fn route(porte: &Porte, charge: &Charge) -> R<Rendu> {
    let ctx = porte.ctx;
    match porte.route {
        Route::Identite => {
            let super::SansCharge {} = charge.lire()?;
            let identite = identite(ctx)?;
            en_json(IdentiteDite { phrase: dire_l_identite(&identite), ecran: ecran_de_l_identite(ctx, &identite) })
        }
        Route::IdentiteImage => en_json(image(ctx, charge.lire()?)?),
        Route::PoserIdentite => {
            let (identite, poids) = poser_identite(ctx, charge.lire()?)?;
            en_json(IdentiteDite { phrase: socle::phrase!("Posé. {} {}", poids, dire_l_identite(&identite)), ecran: ecran_de_l_identite(ctx, &identite) })
        }
        Route::RetirerIdentite => {
            let identite = retirer_identite(ctx, charge.lire()?)?;
            en_json(IdentiteDite { phrase: socle::phrase!("Retiré. {}", dire_l_identite(&identite)), ecran: ecran_de_l_identite(ctx, &identite) })
        }
        autre => Err(super::enveloppe::route_inconnue(autre.nom())),
    }
}

impl Dite for IdentiteDite {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

impl Dite for ImageDeLIdentite {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

/// L'IDENTITE DE LA COMPAGNIE telle que ses routes la rendent : sa phrase, et l'ecran Signature & tampon.
#[derive(Debug, serde::Serialize)]
struct IdentiteDite {
    phrase: Phrase,
    #[serde(flatten)]
    ecran: EcranDeLIdentite,
}

/// L'ecran Signature & tampon : un rang par image, que designe son nom (`ChargeQuoi`), sans case ; les cases du geste
/// qui pose montrent d'abord le poids vise (`ValeursDeLaPose`) et n'offrent aucun choix.
type EcranDeLIdentite = EcranRendu<ChargeQuoi, CaseDeFiche, ValeursDeLaPose>;

/// Ce que montrent d'abord les cases du geste qui pose une image : le poids vise, que le moteur propose.
#[derive(Debug, Clone, serde::Serialize)]
struct ValeursDeLaPose {
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "Octets::ecrite_en_ko_opt")]
    poids: Option<Octets>,
}

/// La charge de `poser-identite` : quoi, l'image en base64 (une adresse data: aussi), et le poids vise, lu en Ko tapes
/// (« 20,5 »).
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargePoser {
    quoi: String,
    contenu: String,
    #[serde(deserialize_with = "Octets::ko")]
    poids: Octets,
}

/// La charge de `retirer-identite` et de `identite-image`, et ce qui designe une image aux gestes de l'ecran : quoi. La
/// fenetre la lit aussi, pour servir l'image a la page (`interface::Fenetre::identite`).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChargeQuoi {
    pub quoi: String,
}

/// Ce que rend `identite-image` : la phrase, le type de media de l'image et ses octets en base64. La fenetre le lit
/// aussi (`interface::Fenetre::identite`).
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageDeLIdentite {
    phrase: Phrase,
    pub media: String,
    pub image: String,
}

// ------------------------------------------------------------------ l'identite

/// Le poids d'une image reçue, et le poids visé, au plus : 2 Mo.
const OCTETS_MAX: Octets = Octets::en_ko(2048);
/// Le poids visé d'une image dans chaque document, que le geste qui la pose propose.
const POIDS_VISE: Octets = Octets::en_ko(20);

/// L'image que la charge nomme (`quoi`), ou ce qui la refuse.
#[track_caller]
fn sorte_d_image(quoi: &str) -> R<SorteDImage> {
    match SorteDImage::du_nom(quoi) {
        Some(s) => Ok(s),
        None => Err(refus(format!("Identite inconnue : {}. Connues : cachet, logo, signature.", socle::fichiers::repr(quoi)))),
    }
}

fn dossier_identite(ctx: &Contexte) -> PathBuf {
    ctx.gabarits().join("identite")
}

/// Le fichier pose pour `quoi`, quelle que soit son extension, et son format.
fn chemin_identite(dossier: &Path, quoi: SorteDImage) -> Option<(PathBuf, FormatDImage)> {
    FormatDImage::TOUS.iter().map(|f| (dossier.join(format!("{}{}", quoi.nom(), f.extension())), *f)).find(|(c, _)| c.exists())
}

/// L'adresse ou la fenetre sert l'image `quoi` a la page (`interface::Fenetre::identite`, par `identite-image`).
fn adresse_de_l_image(quoi: SorteDImage) -> String {
    format!("/identite/{}", quoi.nom())
}

/// `identite-image` : l'image posee pour `quoi`, ou, pour le cachet sans image posee, celui que le logiciel dessine
/// depuis la fiche Entreprise.
fn image(ctx: &Contexte, c: ChargeQuoi) -> R<ImageDeLIdentite> {
    let quoi = sorte_d_image(&c.quoi)?;
    let nom = nom_de(quoi);
    if let Some((f, format)) = chemin_identite(&dossier_identite(ctx), quoi) {
        let octets = std::fs::read(&f).map_err(|e| socle::echec::du_disque(&f, &e).reclasser(socle::echec::Classe::Disque))?;
        return Ok(ImageDeLIdentite { phrase: socle::phrase!("{} posé, en {}.", nom, format.nom().to_uppercase()), media: format.mime().to_string(), image: socle::base64::ecrire(&octets) });
    }
    match (quoi, documents::cachet::svg(&ctx.entreprise)) {
        (SorteDImage::Cachet, Some(d)) => {
            Ok(ImageDeLIdentite { phrase: socle::phrase!("Le tampon que le logiciel dessine depuis la fiche Entreprise, en SVG."), media: FormatDImage::Svg.mime().to_string(), image: socle::base64::ecrire(d.as_bytes()) })
        }
        (SorteDImage::Cachet, None) => Err(refus("Aucun tampon n'est posé, et la fiche Entreprise ne dit rien que le logiciel puisse y dessiner.")),
        (SorteDImage::Signature | SorteDImage::Logo, _) => Err(refus(format!("{} : rien n'est posé.", nom))),
    }
}

fn avant(f: &Path) -> R<()> {
    let mut nom = f.as_os_str().to_owned();
    nom.push(".avant");
    std::fs::rename(f, PathBuf::from(nom)).map_err(|e| socle::echec::du_disque(&f, &e).reclasser(socle::echec::Classe::Disque))
}

fn poser_identite(ctx: &Contexte, charge: ChargePoser) -> R<(Identite, String)> {
    let quoi = sorte_d_image(&charge.quoi)?;
    // Une adresse data: porte son en-tete avant la virgule ; le base64 seul n'en a pas.
    let utile = match charge.contenu.rsplit_once(',') {
        Some((_, b)) => b,
        None => charge.contenu.as_str(),
    };
    let octets = compagnie::problemes::base64_strict(utile).ok_or_else(|| refus("Le fichier n'est pas lisible : base64 invalide."))?;
    if octets.is_empty() {
        return Err(refus("Le fichier est vide."));
    }
    let recu = Octets::de(&octets);
    if recu > OCTETS_MAX {
        return Err(refus(format!("Le fichier fait {} ; la limite est de {}.", recu, OCTETS_MAX)));
    }
    FormatDImage::des_octets(&octets).ok_or_else(|| {
        refus("Ce fichier n'est ni un PNG, ni un JPEG, ni un SVG (ses premiers octets ne sont ceux d'aucun des trois). Rien n'a ete ecrit.")
    })?;
    // Le poids visé est dit par la charge : le geste le propose (`ValeursDeLaPose`), la personne le garde ou le change.
    let objectif = charge.poids;
    if objectif < Octets::en_ko(1) || objectif > OCTETS_MAX {
        return Err(refus(format!("Le poids visé est un nombre de Ko, de 1 Ko à {} : {} ne l'est pas.", OCTETS_MAX, objectif)));
    }
    // L'image part dans chaque document qui la porte : elle est allégée avant d'être posée, à la largeur que les
    // documents impriment (`documents::largeur_imprimee`) et au poids visé.
    let largeur = quoi.largeur_imprimee();
    let allegee = fabrique_pdf::alleger::alleger(&octets, largeur, objectif.en_usize())
        .map_err(|r| documents::fabrique::echec(r).reformuler(|p| format!("Le fichier n'est pas lisible : {}. Rien n'a ete ecrit.", p)))?;
    let poids = dire_le_poids(quoi, &allegee, objectif);
    let (octets, format) = (allegee.octets, allegee.format);
    let dossier = dossier_identite(ctx);
    creer_dossier(&dossier)?;
    // Les autres extensions s'en vont : une identite, un fichier.
    if let Some((ancien, _)) = chemin_identite(&dossier, quoi) {
        avant(&ancien)?;
    }
    let cible = dossier.join(format!("{}{}", quoi.nom(), format.extension()));
    std::fs::write(&cible, &octets).map_err(|e| socle::echec::du_disque(&cible, &e).reclasser(socle::echec::Classe::Disque))?;
    if quoi == SorteDImage::Signature {
        // Pour qui elle est posee : la personne qui signe CE JOUR-LA.
        let mut registre = crate::ouverture::registre_identite(&dossier)?;
        let signataire = crate::ouverture::signataire_de(&ctx.entreprise);
        registre.signature = Some(crate::ouverture::PoseePour { de: signataire.nom, le: aujourd_hui() });
        ecrire(&dossier.join("identite.json"), &registre)?;
    }
    Ok((identite(ctx)?, poids))
}

/// Le nom d'une image, en tete de phrase.
fn nom_de(quoi: SorteDImage) -> &'static str {
    match quoi {
        SorteDImage::Signature => "La signature",
        SorteDImage::Cachet => "Le tampon",
        SorteDImage::Logo => "Le logo",
    }
}

/// Ce que l'image posée pèsera dans chaque document, et pourquoi on l'a allégée.
fn dire_le_poids(quoi: SorteDImage, a: &fabrique_pdf::alleger::Allegee, objectif: Octets) -> String {
    let nom = nom_de(quoi);
    let pourquoi = "Chaque document part par courriel et se garde : une image lourde l'alourdit chaque fois.";
    let allegee = Octets::de(&a.octets);
    let poids = if a.format == FormatDImage::Svg { format!("environ {}", allegee) } else { allegee.to_string() };
    let mut dit = format!("{} pèsera {} dans chaque document qui le porte", nom, poids);
    if quoi == SorteDImage::Signature {
        dit = dit.replace("qui le porte", "qui la porte");
    }
    if !a.telle_quelle {
        dit.push_str(&format!(" (l'image reçue : {} ; ramenée à {} pixels de large, ce que les documents impriment)", Octets::de_longueur(a.avant), a.largeur));
    }
    dit.push('.');
    if allegee > objectif {
        dit.push_str(&format!(" C'est plus que les {} visés : une image plus simple (moins de détails, moins de couleurs) serait plus légère.", objectif));
    }
    format!("{} {}", dit, pourquoi)
}

fn retirer_identite(ctx: &Contexte, charge: ChargeQuoi) -> R<Identite> {
    let quoi = sorte_d_image(&charge.quoi)?;
    if let Some((f, _)) = chemin_identite(&dossier_identite(ctx), quoi) {
        avant(&f)?;
    }
    identite(ctx)
}

fn identite(ctx: &Contexte) -> R<Identite> {
    crate::ouverture::identite(ctx, &ctx.gabarits())
}

/// La pastille d'une image : la sienne, posee, ou celle que le logiciel dessine, faites ; rien de pose, a faire.
fn pastille_de(e: &ImageIdentite) -> socle::ecran::Pastille {
    socle::ecran::Pastille::de(if e.pose || e.automatique { socle::ecran::EtatDEcran::Fait } else { socle::ecran::EtatDEcran::AFaire }, mots_de(e))
}

/// Ce que la pastille d'une image dit : la sienne, posee, celle que le logiciel dessine, ou rien de pose.
fn mots_de(e: &ImageIdentite) -> &'static str {
    if e.pose {
        "le vôtre"
    } else if e.automatique {
        "celui du logiciel"
    } else {
        "rien de posé"
    }
}

/// L'identite de la compagnie en une phrase : la signature, le tampon et le logo, chacun avec ce que l'ecran en dit.
fn dire_l_identite(v: &Identite) -> Phrase {
    socle::phrase!("Signature : {} ; tampon : {} ; logo : {}.", mots_de(&v.signature), mots_de(&v.cachet), mots_de(&v.logo))
}

// Les mots de l'ecran, en constantes : un texte fautif ne compile pas (`Affiche::new`).
const INTRO: &str = "Ce qui se pose sur les documents de la compagnie : PNG, JPEG ou SVG, 2 Mo au plus. Une image posée part dans chaque document qui la porte : le logiciel l'allège au poids visé.";
const IMPORTER: Affiche = Affiche::new("Importer…");
const MODIFIER: Affiche = Affiche::new("Modifier…");
const RETIRER: Affiche = Affiche::new("Retirer");
const TAMPON_DU_LOGICIEL: Affiche = Affiche::new("Tampon du logiciel");

/// L'ECRAN DE LA SIGNATURE ET DU TAMPON, dans l'ordre signature, tampon, logo (`Identite::images`) : pour chaque image
/// son titre et ce qu'elle dit (ou elle va, ce qu'elle engage, et ce qui la contredit dans la fiche Entreprise), sa
/// pastille et ce qui la suit (l'image posee, ou le tampon dessine), l'image qui se regarde, et ses gestes : la poser
/// (ou la changer), et la retirer quand elle est posee (le tampon revient alors au dessin du logiciel).
fn ecran_de_l_identite(ctx: &Contexte, v: &Identite) -> EcranDeLIdentite {
    let ou = |i: &ImageIdentite| if i.documents.is_empty() { String::new() } else { format!("Sur {}", i.documents.join(", ")) };
    let sig = &v.signature;
    let (nom, fonction) = match &sig.signataire {
        Some(s) => (s.nom.clone(), s.fonction.clone()),
        None => (None, None),
    };
    let titre_signature = match (&nom, &fonction) {
        (Some(n), Some(f)) => format!("La signature de {}, {}", n, f),
        (Some(n), None) => format!("La signature de {}", n),
        (None, _) => "La signature".to_string(),
    };
    let mut dit = format!("{}. Elle engage la personne qui signe au nom de la compagnie.", ou(sig));
    if nom.is_none() {
        dit.push_str(" La fiche Entreprise ne dit pas qui signe.");
    }
    if let (Some(true), Some(pour), Some(n)) = (sig.autre_personne, &sig.posee_pour, &nom) {
        dit.push_str(&format!(" Cette image a été posée pour {} ; la personne qui signe est maintenant {}. Importer sa signature à elle, ou corriger la fiche Entreprise.", pour, n));
    }
    let dits = [
        (titre_signature, dit),
        ("Le tampon".to_string(), format!("{}. Il identifie la compagnie, il n'engage personne.", ou(&v.cachet))),
        ("Le logo".to_string(), format!("{}, en haut à droite ; sans lui, rien n'y est imprimé.", ou(&v.logo))),
    ];
    // Le tampon que le logiciel dessine ne se montre que si la fiche Entreprise lui donne de quoi le dessiner.
    let dessine = documents::cachet::svg(&ctx.entreprise).is_some();
    let rangs = v.images().into_iter().zip(dits).map(|((quoi, e), (titre, dit))| {
        let designe = || ChargeQuoi { quoi: quoi.nom().to_string() };
        // Une image posee dit sa forme, son poids et son jour (`ouverture::identite`) ; le tampon dessine, d'ou il vient.
        let etat = match (e.pose, &e.format, e.octets, &e.quand) {
            (true, Some(format), Some(octets), Some(quand)) => Some(format!("{}, {} dans chaque document, posé le {}", format.nom().to_uppercase(), octets, quand.dit())),
            (true, ..) => None,
            (false, ..) if e.automatique => Some("dessiné depuis la fiche Entreprise, il la suit : rien à faire".to_string()),
            (false, ..) => None,
        };
        let montre = if e.pose {
            Some(DocumentMontre::nomme("l'image posée", Route::IdentiteImage, designe(), adresse_de_l_image(quoi)))
        } else if e.automatique && dessine {
            Some(DocumentMontre::nomme("le tampon du logiciel", Route::IdentiteImage, designe(), adresse_de_l_image(quoi)))
        } else {
            None
        };
        let sans_valeur = || ValeursDeLaPose { poids: None };
        let mut gestes = Vec::new();
        if e.pose {
            let retirer = if e.automatique { TAMPON_DU_LOGICIEL } else { RETIRER };
            gestes.push(GesteDeRoute::avec_cases(Route::RetirerIdentite, designe(), retirer, Vec::new(), sans_valeur(), super::SansCharge {}));
        }
        gestes.push(
            GesteDeRoute::avec_cases(Route::PoserIdentite, designe(), if e.pose { MODIFIER } else { IMPORTER }, vec!["contenu", "poids"], ValeursDeLaPose { poids: Some(POIDS_VISE) }, super::SansCharge {})
                .disant(vec![INTRO.to_string()]),
        );
        RangDeLEcran { titre, dit: vec![dit], pastille: pastille_de(e), etat, montre, gestes }
    });
    let intro = Encart::de(socle::ecran::EtatDEcran::Information, "", INTRO);
    EcranRendu { encarts: vec![intro], rangs: rangs.collect(), cases: Vec::new(), historique: Vec::new(), enregistrer: None }
}
