//! Configuration › Signature & tampon : la signature, le cachet et le logo de la compagnie, poses ou dessines. La route
//! `identite` rend l'ecran tel qu'on le voit (`ecran::EcranRendu`) : un rang par image, ce qui s'y regarde (l'image que
//! rend `identite-image`) et ses gestes (`poser-identite`, `retirer-identite`). La boite qui pose une image montre,
//! a chaque case changee, ce que l'image pesera gardee ou allegee (`identite-allegement`) : alleger est un choix.

use std::path::{Path, PathBuf};

use crate::catalogue::{Acte, Affiche, Ecran, Fiche, Place, Refus, FORME, MANQUE, OPERATION};
use crate::declaration::{charge, cle, dit, Legende};
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

const QUOI_IDENTITE: Legende = dit("Signature, cachet ou logo", "l'image de l'identité de la compagnie");

/// Les types de media d'une image qui se pose : ceux que le logiciel reconnait (`FormatDImage`).
const IMAGES_ADMISES: &[&str] = &[FormatDImage::Png.mime(), FormatDImage::Jpeg.mime(), FormatDImage::Svg.mime()];

const IDENTITE_REND: &str = "phrase, encarts (ce qui se pose, et comment), rangs : la signature, le tampon et le logo, chacun son titre, ce qu'il dit (et, posée mais que les documents n'impriment pas, pourquoi : à reposer), sa pastille et son état, l'image qui s'y regarde (montre : la route et la charge qui la rendent, l'adresse de la fenêtre) et ses gestes";

/// Le refus d'une image qui ne se pose pas : ce qu'il faut a la place.
const POSABLE: Refus = Refus { classe: socle::echec::Classe::Refuse, corriger: "une image lisible (PNG, JPEG, SVG), de 2 Mo au plus, et le poids visé quand on l'allège" };

/// Ou ces routes se voient a l'ecran (une place par module).
const PLACE: Place = Place::Identite;

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::Identite,
        geste: "Dire la signature, le cachet et le logo de la compagnie.",
        charge: &crate::SansCharge::CHARGE,
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
        geste: "Rendre l'image de la signature, du tampon ou du logo.",
        charge: &ChargeQuoi::CHARGE,
        exemple: r#"{"quoi":"cachet"}"#,
        rend: "phrase, media (son type), image (en base64)",
        refus: &[OPERATION, FORME],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Lit,
        phrase: 0,
        ecran: Ecran::Lecture,
    },
    Fiche {
        route: Route::IdentiteAllegement,
        geste: "Montrer ce qu'une image pèserait dans les documents, gardée ou allégée.",
        charge: &ChargePoser::CHARGE,
        exemple: r#"{"quoi":"logo","contenu":"PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIxIiBoZWlnaHQ9IjEiLz4=","allegement":"alleger","poids":20}"#,
        rend: "phrase (le texte d'un SVG dessiné en tracés et les polices qui manquent au poste, le poids de l'image reçue, ce qu'elle pèserait telle quelle dans chaque document, et allégée au poids visé quand on l'allège), media (son type), image (celle qui se poserait, en base64)",
        refus: &[POSABLE, FORME, MANQUE],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Lit,
        phrase: 0,
        ecran: Ecran::Lecture,
    },
    Fiche {
        route: Route::PoserIdentite,
        geste: "Poser la signature, le cachet ou le logo (PNG, JPEG, SVG), gardé ou allégé.",
        charge: &ChargePoser::CHARGE,
        exemple: r#"{"quoi":"logo","contenu":"PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHdpZHRoPSIxIiBoZWlnaHQ9IjEiLz4=","allegement":"alleger","poids":20}"#,
        rend: IDENTITE_REND,
        refus: &[POSABLE, FORME, MANQUE],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: Ecran::Geste(PLACE),
    },
    Fiche {
        route: Route::RetirerIdentite,
        geste: "Retirer la signature, le cachet ou le logo posé.",
        charge: &ChargeQuoi::CHARGE,
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
            en_json(IdentiteDite { phrase: dire_l_identite(&identite), ecran: ecran_de_l_identite(ctx, &identite)? })
        }
        Route::IdentiteImage => en_json(image(ctx, charge.lire()?)?),
        Route::IdentiteAllegement => {
            let a = a_poser(&charge.lire()?)?;
            en_json(ImageDeLIdentite { phrase: socle::phrase!("{}", a.dit), media: a.format.mime().to_string(), image: socle::base64::ecrire(&a.octets) })
        }
        Route::PoserIdentite => {
            let (identite, poids) = poser_identite(ctx, charge.lire()?)?;
            en_json(IdentiteDite { phrase: socle::phrase!("Posé. {} {}", poids, dire_l_identite(&identite)), ecran: ecran_de_l_identite(ctx, &identite)? })
        }
        Route::RetirerIdentite => {
            let identite = retirer_identite(ctx, charge.lire()?)?;
            en_json(IdentiteDite { phrase: socle::phrase!("Retiré. {}", dire_l_identite(&identite)), ecran: ecran_de_l_identite(ctx, &identite)? })
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
/// qui pose montrent d'abord ce que le moteur propose (`ValeursDeLaPose`) et offrent l'allegement (`ChoixDeLaPose`).
type EcranDeLIdentite = EcranRendu<ChargeQuoi, CaseDeFiche, ValeursDeLaPose, ChoixDeLaPose>;

/// Ce que montrent d'abord les cases du geste qui pose une image : l'allegement et le poids vise, que le moteur propose.
#[derive(Debug, Clone, serde::Serialize)]
struct ValeursDeLaPose {
    #[serde(skip_serializing_if = "Option::is_none")]
    allegement: Option<Allegement>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "Octets::ecrite_en_ko_opt")]
    poids: Option<Octets>,
}

/// Ce qu'offre la case de l'allegement du geste qui pose une image : chacun de ses choix, alleger ouvrant le poids vise.
#[derive(Debug, Clone, serde::Serialize)]
struct ChoixDeLaPose {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    allegement: Vec<crate::ouverture::Choix>,
}

socle::noms_fermes! {
    /// L'ALLEGEMENT D'UNE IMAGE A SA POSE : la garder telle qu'elle est recue, ou l'alleger au poids vise
    /// (`fabrique_pdf::alleger::alleger`). Chaque document qui la porte part par courriel et se garde : c'est a la
    /// personne de choisir, en voyant ce que chacun pese (`identite-allegement`).
    Allegement, "Allègement" {
        Garder = "garder" : "Garder l'original",
        Alleger = "alleger" : "Alléger au poids visé",
    }
}

charge! {
    /// La charge de `poser-identite` et de `identite-allegement` : quoi, l'image en base64 (une adresse data: aussi),
    /// l'allegement, et le poids vise, lu en Ko tapes (« 20,5 »), quand on l'allege et seulement alors.
    #[derive(Debug, Clone)]
    struct ChargePoser {
        quoi: SorteDImage = QUOI_IDENTITE,
        contenu: String = dit("Image", "l'image en base64 (une adresse data: aussi)").fichier(IMAGES_ADMISES),
        allegement: Allegement = dit("Allègement", "garder l'image telle quelle, ou l'alléger au poids visé (le logiciel propose d'alléger)"),
        #[serde(default, deserialize_with = "Octets::ko_opt")]
        poids: Option<Octets> = dit("Poids visé en Ko", "le poids visé de l'image dans chaque document, en Ko, quand on l'allège (le logiciel en propose un)"),
    }
}

charge! {
    /// La charge de `retirer-identite` et de `identite-image`, et ce qui designe une image aux gestes de l'ecran : quoi. La
    /// fenetre la lit aussi, pour servir l'image a la page (`interface::Fenetre::identite`).
    #[derive(Debug, Clone, serde::Serialize)]
    pub struct ChargeQuoi {
        pub quoi: SorteDImage = QUOI_IDENTITE,
    }
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


fn dossier_identite(ctx: &Contexte) -> PathBuf {
    ctx.gabarits().join("identite")
}

/// Le fichier pose pour `quoi`, quelle que soit son extension, et son format ; un disque qui ne repond pas est une
/// erreur, pas une absence.
fn chemin_identite(dossier: &Path, quoi: SorteDImage) -> R<Option<(PathBuf, FormatDImage)>> {
    for f in FormatDImage::TOUS {
        let c = dossier.join(format!("{}{}", quoi.nom(), f.extension()));
        if c.try_exists().map_err(|e| socle::echec::du_disque(&c, &e).reclasser(socle::echec::Classe::Disque))? {
            return Ok(Some((c, f)));
        }
    }
    Ok(None)
}

/// UNE IMAGE POSEE QUE LES DOCUMENTS N'IMPRIMENT PAS (un SVG au texte posé avant que la pose ne le dessine en tracés) :
/// ce qui l'en empeche, lu comme la fabrique la lit (`poids_dans_un_document`) ; elle se repose, rien ne la reecrit
/// d'office. Un defaut du logiciel remonte.
fn ne_s_imprime_pas(ctx: &Contexte, quoi: SorteDImage) -> R<Option<String>> {
    let Some((f, _)) = chemin_identite(&dossier_identite(ctx), quoi)? else {
        return Ok(None);
    };
    let octets = std::fs::read(&f).map_err(|e| socle::echec::du_disque(&f, &e).reclasser(socle::echec::Classe::Disque))?;
    match fabrique_pdf::alleger::poids_dans_un_document(&octets, quoi.largeur_imprimee()) {
        Ok(_) => Ok(None),
        Err(r) if r.classe == fabrique_pdf::Classe::DefautDuLogiciel => Err(documents::fabrique::echec(r)),
        Err(r) => Ok(Some(r.phrase)),
    }
}

/// L'adresse ou la fenetre sert l'image `quoi` a la page (`interface::Fenetre::identite`, par `identite-image`).
fn adresse_de_l_image(quoi: SorteDImage) -> String {
    format!("/identite/{}", quoi.nom())
}

/// `identite-image` : l'image posee pour `quoi`, ou, pour le cachet sans image posee, celui que le logiciel dessine
/// depuis la fiche Entreprise.
fn image(ctx: &Contexte, c: ChargeQuoi) -> R<ImageDeLIdentite> {
    let quoi = c.quoi;
    let nom = nom_de(quoi);
    if let Some((f, format)) = chemin_identite(&dossier_identite(ctx), quoi)? {
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

/// UNE IMAGE RECUE, TELLE QU'ELLE SE POSERAIT : ses octets (l'original, ou l'image allegee), leur format, et ce que la
/// phrase en dit (`dire_le_poids`).
struct ImageAPoser {
    octets: Vec<u8>,
    format: FormatDImage,
    dit: String,
}

/// L'image que la charge pose, gardee ou allegee comme elle le dit, et ce qu'elle pese : ce que `identite-allegement`
/// montre et que `poser-identite` ecrit, le meme calcul pour les deux. Rien ne s'ecrit.
fn a_poser(charge: &ChargePoser) -> R<ImageAPoser> {
    let quoi = charge.quoi;
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
    let format = FormatDImage::des_octets(&octets).ok_or_else(|| {
        refus("Ce fichier n'est ni un PNG, ni un JPEG, ni un SVG (ses premiers octets ne sont ceux d'aucun des trois).")
    })?;
    // LE TEXTE D'UN SVG se dessine ici en tracés : les documents ne l'impriment pas (`fabrique_pdf::alleger::textes_en_traces`).
    let (octets, en_traces) = match format {
        FormatDImage::Svg => {
            let traces = fabrique_pdf::alleger::textes_en_traces(&octets).map_err(|r| documents::fabrique::echec(r).reformuler(|p| format!("L'image ne se pose pas : {}.", p)))?;
            match traces {
                Some(fabrique_pdf::alleger::TexteEnTraces { octets, textes, polices_absentes }) => (octets, Some(dire_les_traces(&textes, &polices_absentes))),
                None => (octets, None),
            }
        }
        FormatDImage::Png | FormatDImage::Jpeg => (octets, None),
    };
    let dit = |poids: String| match &en_traces {
        Some(t) => format!("{} {}", t, poids),
        None => poids,
    };
    let illisible = |r| documents::fabrique::echec(r).reformuler(|p| format!("Le fichier n'est pas lisible : {}.", p));
    // Ce que chaque document en ecrit, a la largeur que les documents impriment (`documents::largeur_imprimee`) : la
    // fabrique le ramene a la resolution d'impression, et le comprime.
    let largeur = quoi.largeur_imprimee();
    let dans_un_document = |o: &[u8]| fabrique_pdf::alleger::poids_dans_un_document(o, largeur).map(Octets::de_longueur).map_err(illisible);
    let tel_quel = dans_un_document(&octets)?;
    match (charge.allegement, charge.poids) {
        (Allegement::Garder, None) => Ok(ImageAPoser { dit: dit(dire_le_poids(recu, tel_quel, None)), octets, format }),
        (Allegement::Garder, Some(_)) => Err(refus("Le poids visé ne vaut que pour alléger l'image : gardée telle quelle, elle n'en a pas.")),
        (Allegement::Alleger, None) => Err(refus("Alléger l'image demande le poids visé, en Ko.")),
        (Allegement::Alleger, Some(objectif)) => {
            if objectif < Octets::en_ko(1) || objectif > OCTETS_MAX {
                return Err(refus(format!("Le poids visé est un nombre de Ko, de 1 Ko à {} : {} ne l'est pas.", OCTETS_MAX, objectif)));
            }
            let a = fabrique_pdf::alleger::alleger(&octets, largeur, objectif.en_usize()).map_err(illisible)?;
            let allegee = Allege { poids: dans_un_document(&a.octets)?, largeur: (!a.telle_quelle).then_some(a.largeur), objectif };
            Ok(ImageAPoser { dit: dit(dire_le_poids(recu, tel_quel, Some(allegee))), octets: a.octets, format: a.format })
        }
    }
}

/// Ce que l'allegement a donne : le poids de l'image allegee dans chaque document, sa largeur en pixels quand elle a ete
/// ramenee (aucune : elle reste telle quelle, deja assez fine et assez legere), et le poids vise.
struct Allege {
    poids: Octets,
    largeur: Option<u32>,
    objectif: Octets,
}

fn poser_identite(ctx: &Contexte, charge: ChargePoser) -> R<(Identite, String)> {
    let quoi = charge.quoi;
    let ImageAPoser { octets, format, dit: poids } = a_poser(&charge).map_err(|e| e.reformuler(|p| format!("{} Rien n'a été écrit.", p)))?;
    let dossier = dossier_identite(ctx);
    creer_dossier(&dossier)?;
    // Les autres extensions s'en vont : une identite, un fichier.
    if let Some((ancien, _)) = chemin_identite(&dossier, quoi)? {
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

/// Ce que l'image reçue pèse, ce qu'elle pèse telle quelle dans chaque document, et, allégée, ce qu'elle y pèse.
fn dire_le_poids(recu: Octets, tel_quel: Octets, allegee: Option<Allege>) -> String {
    let mut dit = format!("L'image reçue : {} ; telle quelle, {} dans chaque document qui la porte.", recu, tel_quel);
    if let Some(Allege { poids, largeur, objectif }) = allegee {
        match largeur {
            None => dit.push_str(&format!(" Allégée au poids visé ({}) : rien ne change, l'alléger ne la rend pas plus légère.", objectif)),
            Some(l) => dit.push_str(&format!(" Allégée au poids visé ({}) : {} dans chaque document, ramenée à {} pixels de large, ce que les documents impriment.", objectif, poids, l)),
        }
        if poids > objectif {
            dit.push_str(&format!(" C'est plus que les {} visés : une image plus simple (moins de détails, moins de couleurs) serait plus légère.", objectif));
        }
    }
    format!("{} Chaque document part par courriel et se garde : une image lourde l'alourdit chaque fois.", dit)
}

/// Ce que la pose a fait du texte d'un SVG : ce qu'il disait, dessiné en tracés avec les polices du poste, et celles
/// qu'il nomme et que le poste n'a pas.
fn dire_les_traces(textes: &[String], absentes: &[String]) -> String {
    let guillemets = |l: &[String]| l.iter().map(|t| format!("« {} »", t)).collect::<Vec<_>>().join(", ");
    let mut dit = if textes.is_empty() {
        "Son texte est dessiné en tracés avec les polices de ce poste : les documents n'impriment pas le texte d'un SVG.".to_string()
    } else {
        format!("Son texte ({}) est dessiné en tracés avec les polices de ce poste : les documents n'impriment pas le texte d'un SVG.", guillemets(textes))
    };
    match absentes {
        [] => {}
        [une] => dit.push_str(&format!(" La police « {} » n'est pas sur ce poste : une autre la remplace.", une)),
        plusieurs => dit.push_str(&format!(" Les polices {} ne sont pas sur ce poste : d'autres les remplacent.", guillemets(plusieurs))),
    }
    format!("{} Regardez l'image : c'est elle qui se pose.", dit)
}

fn retirer_identite(ctx: &Contexte, charge: ChargeQuoi) -> R<Identite> {
    let quoi = charge.quoi;
    if let Some((f, _)) = chemin_identite(&dossier_identite(ctx), quoi)? {
        avant(&f)?;
    }
    identite(ctx)
}

fn identite(ctx: &Contexte) -> R<Identite> {
    crate::ouverture::identite(ctx, &ctx.gabarits())
}

/// La pastille d'une image : la sienne, posee, ou celle que le logiciel dessine, faites ; rien de pose, a faire, sauf le
/// logo, facultatif : sans lui, rien n'est imprime a sa place. Posee mais que les documents n'impriment pas
/// (`ne_s_imprime_pas`) : a reposer.
fn pastille_de(quoi: SorteDImage, e: &ImageIdentite, a_reposer: bool) -> socle::ecran::Pastille {
    if a_reposer {
        return socle::ecran::Pastille::de(socle::ecran::EtatDEcran::AFaire, "à reposer");
    }
    let etat = if e.pose || e.automatique {
        socle::ecran::EtatDEcran::Fait
    } else if quoi == SorteDImage::Logo {
        socle::ecran::EtatDEcran::Information
    } else {
        socle::ecran::EtatDEcran::AFaire
    };
    socle::ecran::Pastille::de(etat, mots_de(e))
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

/// Les cases du geste qui pose une image : l'image, l'allegement, et le poids vise, qu'ouvre « alleger ».
fn cases_de_la_pose() -> Vec<&'static str> {
    vec![cle!(ChargePoser, contenu), cle!(ChargePoser, allegement), cle!(ChargePoser, poids)]
}

/// Ce que le geste qui pose une image propose : l'alleger, au poids vise ; la personne voit ce que chacun pese
/// (`identite-allegement`) et choisit.
const PROPOSE: ValeursDeLaPose = ValeursDeLaPose { allegement: Some(Allegement::Alleger), poids: Some(POIDS_VISE) };

/// Les choix de l'allegement, ceux du type (`Allegement`), « alleger » ouvrant le poids vise.
fn choix_de_la_pose() -> R<ChoixDeLaPose> {
    let allegement = Allegement::TOUTES
        .iter()
        .map(|a| {
            let c = crate::ouverture::Choix::de(a.nom(), socle::liste::ListeFermee::libelle(a)?);
            Ok(match a {
                Allegement::Garder => c,
                Allegement::Alleger => c.ouvrant(vec![cle!(ChargePoser, poids)]),
            })
        })
        .collect::<R<Vec<_>>>()?;
    Ok(ChoixDeLaPose { allegement })
}

// Les mots de l'ecran, en constantes : un texte fautif ne compile pas (`Affiche::new`).
const INTRO: &str = "Ce qui se pose sur les documents de la compagnie : PNG, JPEG ou SVG, 2 Mo au plus. Une image posée part dans chaque document qui la porte : la garder telle quelle, ou l'alléger au poids visé ; la boîte montre ce que chacune pèse.";
const IMPORTER: Affiche = Affiche::new("Importer…");
const MODIFIER: Affiche = Affiche::new("Modifier…");
const RETIRER: Affiche = Affiche::new("Retirer");
const TAMPON_DU_LOGICIEL: Affiche = Affiche::new("Tampon du logiciel");

/// L'ECRAN DE LA SIGNATURE ET DU TAMPON, dans l'ordre signature, tampon, logo (`Identite::images`) : pour chaque image
/// son titre et ce qu'elle dit (ou elle va, ce qu'elle engage, et ce qui la contredit dans la fiche Entreprise), sa
/// pastille et ce qui la suit (l'image posee, ou le tampon dessine), l'image qui se regarde, et ses gestes : la poser
/// (ou la changer), et la retirer quand elle est posee (le tampon revient alors au dessin du logiciel).
fn ecran_de_l_identite(ctx: &Contexte, v: &Identite) -> R<EcranDeLIdentite> {
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
    let mut dit = format!("{}. Elle engage la personne qui signe au nom de la compagnie ; sans elle, aucun contrat ne part se faire signer.", ou(sig));
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
    let choix = choix_de_la_pose()?;
    // Une image posee que les documents n'impriment pas se dit a reposer, sur son rang : les documents qui la portent
    // sont refuses tant qu'elle reste.
    let reposer = v.images().into_iter().map(|(quoi, e)| if e.pose { ne_s_imprime_pas(ctx, quoi) } else { Ok(None) }).collect::<R<Vec<_>>>()?;
    let rangs = v.images().into_iter().zip(dits).zip(reposer).map(|(((quoi, e), (titre, dit)), reposer)| {
        let mut dit = vec![dit];
        if let Some(pourquoi) = &reposer {
            dit.push(format!(
                "L'image posée ne s'imprime pas : {}. La reposer (« {} ») : à la pose, le texte d'un SVG se dessine en tracés. Tant qu'elle reste, chaque document qui la porte est refusé.",
                pourquoi,
                MODIFIER.texte()
            ));
        }
        let designe = || ChargeQuoi { quoi };
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
        let mut gestes = Vec::new();
        if e.pose {
            let retirer = if e.automatique { TAMPON_DU_LOGICIEL } else { RETIRER };
            let rien = ValeursDeLaPose { allegement: None, poids: None };
            gestes.push(GesteDeRoute::avec_cases(Route::RetirerIdentite, designe(), retirer, Vec::new(), rien, ChoixDeLaPose { allegement: Vec::new() }));
        }
        gestes.push(
            GesteDeRoute::avec_cases(Route::PoserIdentite, designe(), if e.pose { MODIFIER } else { IMPORTER }, cases_de_la_pose(), PROPOSE, choix.clone())
                .disant(vec![INTRO.to_string()])
                .montrant(Route::IdentiteAllegement),
        );
        RangDeLEcran { titre, dit, pastille: pastille_de(quoi, e, reposer.is_some()), etat, montre, gestes }
    });
    let intro = Encart::de(socle::ecran::EtatDEcran::Information, "", INTRO);
    Ok(EcranRendu { encarts: vec![intro], rangs: rangs.collect(), cases: Vec::new(), historique: Vec::new(), enregistrer: None })
}
