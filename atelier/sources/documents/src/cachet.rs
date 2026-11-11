//! Le cachet de l'entreprise, DESSINE depuis ses propres donnees -- en SVG.
//!
//! Une attestation a deux cases : << Signature de l'employeur >> et le cachet. Le
//! fichier `gabarits/identite/signature.png` est un CACHET (<< un tampon a
//! l'italienne >>), pas une signature : il va dans la case cachet, jamais dans la
//! case signature.
//!
//! POURQUOI LE DESSINER PLUTOT QUE DEMANDER UNE IMAGE. C'est la regle 0 : ce
//! logiciel n'est ecrit pour aucune compagnie en particulier. Un PNG ne vaut que
//! pour celle qui l'a fourni -- une compagnie qui installe PuppetPaie n'aurait,
//! sans cela, aucun cachet, ou pire, celui de la precedente. Tout ce qu'un cachet
//! porte est deja dans `entreprise.json`, verifie et date : la raison sociale,
//! l'adresse, le SIRET, l'APE, la licence d'entrepreneur de spectacles. Le
//! dessiner, c'est n'avoir rien a demander a personne.
//!
//! POURQUOI EN SVG. Un tampon est un DESSIN, pas une photo : il doit rester net
//! dans la case d'un formulaire comme a l'ecran, et la MEME image sert aux deux.
//! Le SVG se met a l'echelle sans baver, pese quelques centaines d'octets, et
//! c'est le meme texte qu'on envoie a la page de Configuration pour que la
//! compagnie VOIE son cachet avant de se demander s'il lui en faut un autre --
//! la demande de l'editeur, mot pour mot : << que les gens le trouvent beau et ne
//! passent pas trop de temps a faire le leur >>.
//!
//! ET LA COMPAGNIE GARDE LE SIEN SI ELLE EN A UN. Le dessin est le DEFAUT, pas
//! une obligation : celle qui a deja son tampon le pose dans Configuration, et
//! c'est lui qui part. Qu'elle le retire, et le dessin reprend la main -- rien a
//! reconfigurer, rien a se rappeler. C'est `image` qui arbitre, et il n'a qu'une
//! question : y a-t-il un fichier ?
//!
//! RIEN N'EST INVENTE. Une valeur absente d'`entreprise.json` ne s'ecrit pas :
//! la ligne saute. Un cachet qui afficherait un SIRET vide ou << None >> serait
//! pire que pas de cachet, et la case n'est exigee par aucun formulaire.
//!
//! Le dessin se mesure en `Mesure` (des millioniemes exacts) : chaque coordonnee s'ecrit ensuite a six chiffres
//! significatifs (`format_g`).

use droit::fiche::Lue;
use fabrique_pdf::mesure::{mesure, Mesure};
use rust_decimal::RoundingStrategy;
use socle::num::Decimal;

/// Une coordonnee du dessin, ecrite a six chiffres significatifs (au pair a egalite), sans zero inutile : `12.3457`,
/// `300`, `0`. Le dessin reste entre 1e-4 et 1e6 : la notation positionnelle suffit.
fn format_g(x: Mesure) -> String {
    let d = Decimal::new(x.en_millioniemes(), 6);
    match d.round_sf_with_strategy(6, RoundingStrategy::MidpointNearestEven) {
        Some(r) => r.normalize().to_string(),
        None => unreachable!("une mesure tient dans un decimal : {}", x),
    }
}

/// L'encre d'un tampon, et la geometrie du dessin. Le rapport 2:1 est celui
/// d'un tampon rectangulaire courant ; il tient dans la case cachet de l'AER
/// (160 x 79 points) sans rogner.
pub const ENCRE: &str = "#1a2e73";
pub const LARGEUR: Mesure = Mesure::entier(300);
pub const HAUTEUR: Mesure = Mesure::entier(150);
/// Le tampon ne touche pas le bord de la case.
pub const MARGE: Mesure = Mesure::entier(3);

/// Ce que le cachet porte : le nom, le lieu, les numeros.
#[derive(Debug, Clone, PartialEq)]
pub struct Lignes {
    pub nom: Option<String>,
    pub lieu: Vec<String>,
    pub numeros: Vec<String>,
}

fn texte_de<T: AsRef<str>>(c: &Option<T>) -> Option<String> {
    c.remplie().map(str::to_string)
}

/// `lignes` : rendu en donnees et non dessine, pour que cela se lise sans
/// ouvrir un fichier. Les trois groupes n'ont pas le meme poids a l'oeil : le
/// nom est ce qu'on lit, le reste est ce qu'on verifie. `ent` est la fiche a la
/// date du document (`Contexte::entreprise`) : la licence y est le numero du jour.
pub fn lignes(ent: &droit::fiche::ChampsEntreprise) -> Lignes {
    let nom = texte_de(&ent.nom_affichage)
        .or_else(|| texte_de(&ent.raison_sociale));
    let mut lieu = Vec::new();
    if let Some(adresse) = texte_de(&ent.adresse_affichage).or_else(|| texte_de(&ent.adresse)) {
        lieu.push(adresse);
    }
    if let Some(complement) = texte_de(&ent.complement_adresse_affichage).or_else(|| texte_de(&ent.complement_adresse)) {
        lieu.push(complement);
    }
    let ou: Vec<String> = [texte_de(&ent.code_postal), texte_de(&ent.commune_affichage).or_else(|| texte_de(&ent.commune))]
        .into_iter()
        .flatten()
        .collect();
    if !ou.is_empty() {
        lieu.push(ou.join(" "));
    }
    let mut numeros = Vec::new();
    if let Some(s) = texte_de(&ent.siret) {
        numeros.push(format!("SIRET {}", s));
    }
    if let Some(s) = texte_de(&ent.naf) {
        numeros.push(format!("APE {}", s));
    }
    if let Some(l) = texte_de(&ent.licence_spectacle) {
        numeros.push(format!("Licence {}", l));
    }
    Lignes { nom, lieu, numeros }
}

/// Le corps le plus grand auquel `texte` tient dans `large`, et 5 au moins.
///
/// Sans mesure de police reelle : une largeur moyenne par caractere (`etroit`, une fraction du corps) suffit
/// pour du texte court, et le SVG n'a pas d'unite de mesure a nous rendre.
/// Mieux vaut un tampon un rien plus petit qu'un tampon qui deborde.
fn corps_qui_tient(texte: &str, large: Mesure, depart: Mesure, etroit: Mesure) -> Mesure {
    let n = texte.chars().count() as i64;
    if n == 0 {
        return depart;
    }
    let tient = match large.sur(etroit * n) {
        Some(t) => t,
        None => unreachable!("un caractere a une largeur"),
    };
    depart.min(tient).max(Mesure::entier(5))
}

/// `&`, `<` et `>`, et rien d'autre.
fn echapper(t: &str) -> String {
    t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Le cachet, en SVG. None s'il n'y a rien a dessiner.
///
/// Un cadre double -- le trait epais dehors, le fin dedans --, le nom en
/// capitales, un filet, puis l'adresse et les numeros. C'est la mise en page
/// d'un tampon encreur, et elle est tenue ICI pour que la page de
/// Configuration et l'attestation montrent exactement la meme chose.
pub fn svg(ent: &droit::fiche::ChampsEntreprise) -> Option<String> {
    let t = lignes(ent);
    if t.nom.is_none() && t.lieu.is_empty() && t.numeros.is_empty() {
        return None;
    }

    let dedans = Mesure::entier(16);
    let utile = LARGEUR - dedans * 2;
    let milieu = LARGEUR / 2;
    let police = "Helvetica,Arial,sans-serif";

    // LA MISE EN PAGE SE CALCULE AVANT DE S'ECRIRE, et c'est ce qui evite le
    // defaut trouve au premier rendu : la ligne << Licence ... >> sortait par
    // le bas du cadre. Une compagnie qui porte un nom long, trois lignes
    // d'adresse ou une licence a rallonge n'est pas un cas rare -- c'est le
    // cas courant des que l'on quitte celle qui a servi a dessiner.
    //
    // On monte donc la liste de ce qu'il y a a ecrire (texte, corps, graisse,
    // blanc au-dessus), on mesure, et on comprime si besoin. Le resultat est
    // ensuite CENTRE verticalement : un tampon dont le texte pend vers le haut
    // ou vers le bas se voit tout de suite.
    let mut items: Vec<(Option<String>, Mesure, u32, Mesure)> = Vec::new();
    if let Some(nom) = &t.nom {
        items.push((Some(nom.to_uppercase()), corps_qui_tient(nom, utile, Mesure::entier(22), mesure("0.62")), 700, Mesure::ZERO));
        items.push((None, Mesure::ZERO, 0, Mesure::entier(9))); // le filet
    }
    for (i, ligne) in t.lieu.iter().enumerate() {
        let blanc = if i == 0 { Mesure::entier(7) } else { Mesure::entier(3) };
        items.push((Some(ligne.clone()), corps_qui_tient(ligne, utile, mesure("11.5"), mesure("0.52")), 400, blanc));
    }
    for (i, ligne) in t.numeros.iter().enumerate() {
        let blanc = if i == 0 { Mesure::entier(6) } else { mesure("2.5") };
        items.push((Some(ligne.clone()), corps_qui_tient(ligne, utile, Mesure::entier(10), mesure("0.52")), 400, blanc));
    }

    let haut_utile = HAUTEUR - dedans * 2;
    let mut total: Mesure = items.iter().map(|(_, corps, _, blanc)| *blanc + *corps).sum();
    if total > haut_utile {
        // Tout retrecit du meme rapport : les proportions du tampon tiennent,
        // et rien ne sort du cadre.
        let reduit = |m: Mesure| match m.fois_rapport(haut_utile.en_millioniemes(), total.en_millioniemes()) {
            Some(r) => r,
            None => unreachable!("le total depasse la hauteur utile : il n'est pas nul"),
        };
        items = items.into_iter().map(|(txt, corps, gras, blanc)| (txt, reduit(corps), gras, reduit(blanc))).collect();
        total = haut_utile;
    }

    let mut out = vec![
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" width=\"{}\" height=\"{}\">",
            format_g(LARGEUR),
            format_g(HAUTEUR),
            format_g(LARGEUR),
            format_g(HAUTEUR)
        ),
        format!(
            "<rect x=\"4\" y=\"4\" width=\"{}\" height=\"{}\" rx=\"12\" fill=\"none\" stroke=\"{}\" stroke-width=\"4\"/>",
            format_g(LARGEUR - Mesure::entier(8)),
            format_g(HAUTEUR - Mesure::entier(8)),
            ENCRE
        ),
        format!(
            "<rect x=\"11\" y=\"11\" width=\"{}\" height=\"{}\" rx=\"7\" fill=\"none\" stroke=\"{}\" stroke-width=\"1.2\"/>",
            format_g(LARGEUR - Mesure::entier(22)),
            format_g(HAUTEUR - Mesure::entier(22)),
            ENCRE
        ),
    ];
    let mut y = dedans + (haut_utile - total) / 2;
    for (texte, corps, gras, blanc) in &items {
        y += *blanc + *corps;
        match texte {
            None => out.push(format!(
                "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"1\"/>",
                format_g(milieu - utile / 3),
                format_g(y),
                format_g(milieu + utile / 3),
                format_g(y),
                ENCRE
            )),
            Some(texte) => out.push(format!(
                "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" fill=\"{}\" font-family=\"{}\" font-size=\"{}\" font-weight=\"{}\">{}</text>",
                format_g(milieu),
                format_g(y),
                ENCRE,
                police,
                format_g(*corps),
                gras,
                echapper(texte)
            )),
        }
    }
    out.push("</svg>".into());
    Some(out.concat())
}
