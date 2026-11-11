//! Les métriques des polices standard (ISO 32000-1, 9.6.2.2 : les 14 polices que tout lecteur connaît sans qu'elles
//! soient incorporées, « or their font metrics ») : leurs fichiers de métriques d'Adobe (« Core 14 AFM Files »),
//! déposés tels que servis dans `atelier/reference/sources/adobe-polices/adobe-core14-afm-20261111/` et lus tels quels,
//! selon la spécification AFM 4.1 (`adobe-afm-extraits-20261111.md`, Adobe Technical Note #5004). Une police n'y est que
//! par son nom exact : un autre nom, même approchant, n'est pas une police standard.

use std::collections::HashMap;

use crate::description::{defaut, Refus};
use crate::mesure::Mesure;

const DOSSIER: [(&str, &str); 14] = [
    ("Courier", include_str!("../../../reference/sources/adobe-polices/adobe-core14-afm-20261111/Courier.afm")),
    ("Courier-Bold", include_str!("../../../reference/sources/adobe-polices/adobe-core14-afm-20261111/Courier-Bold.afm")),
    ("Courier-Oblique", include_str!("../../../reference/sources/adobe-polices/adobe-core14-afm-20261111/Courier-Oblique.afm")),
    ("Courier-BoldOblique", include_str!("../../../reference/sources/adobe-polices/adobe-core14-afm-20261111/Courier-BoldOblique.afm")),
    ("Helvetica", include_str!("../../../reference/sources/adobe-polices/adobe-core14-afm-20261111/Helvetica.afm")),
    ("Helvetica-Bold", include_str!("../../../reference/sources/adobe-polices/adobe-core14-afm-20261111/Helvetica-Bold.afm")),
    ("Helvetica-Oblique", include_str!("../../../reference/sources/adobe-polices/adobe-core14-afm-20261111/Helvetica-Oblique.afm")),
    ("Helvetica-BoldOblique", include_str!("../../../reference/sources/adobe-polices/adobe-core14-afm-20261111/Helvetica-BoldOblique.afm")),
    ("Times-Roman", include_str!("../../../reference/sources/adobe-polices/adobe-core14-afm-20261111/Times-Roman.afm")),
    ("Times-Bold", include_str!("../../../reference/sources/adobe-polices/adobe-core14-afm-20261111/Times-Bold.afm")),
    ("Times-Italic", include_str!("../../../reference/sources/adobe-polices/adobe-core14-afm-20261111/Times-Italic.afm")),
    ("Times-BoldItalic", include_str!("../../../reference/sources/adobe-polices/adobe-core14-afm-20261111/Times-BoldItalic.afm")),
    ("Symbol", include_str!("../../../reference/sources/adobe-polices/adobe-core14-afm-20261111/Symbol.afm")),
    ("ZapfDingbats", include_str!("../../../reference/sources/adobe-polices/adobe-core14-afm-20261111/ZapfDingbats.afm")),
];

/// Le fichier de métriques de la police standard que `nom` nomme exactement.
fn fichier(nom: &str) -> Option<&'static str> {
    DOSSIER.iter().find(|(n, _)| *n == nom).map(|(_, afm)| *afm)
}

/// Un nombre d'un fichier de métriques déposé : son défaut est celui du logiciel, qui le livre.
fn nombre(nom: &str, cle: &str, v: &str) -> Result<Mesure, Refus> {
    match v.trim().parse::<i64>() {
        Ok(v) => Ok(Mesure::entier(v)),
        Err(e) => defaut(format!("le fichier de métriques déposé de {} a un {} illisible ({:?} : {})", nom, cle, v, e)),
    }
}

/// L'`Ascender` et le `Descender` (en millièmes d'em, le descendant négatif) de la police standard `nom` ; `None` pour
/// un nom qui n'est pas celui d'une police standard, ou dont le fichier ne les dit pas (« Optional », section 7 :
/// Symbol et ZapfDingbats).
pub(crate) fn ascendant_descendant(nom: &str) -> Result<Option<(Mesure, Mesure)>, Refus> {
    let Some(afm) = fichier(nom) else {
        return Ok(None);
    };
    let valeur = |cle: &str| -> Result<Option<Mesure>, Refus> {
        afm.lines().find_map(|l| l.strip_prefix(cle).and_then(|r| r.strip_prefix(' '))).map(|v| nombre(nom, cle, v)).transpose()
    };
    match (valeur("Ascender")?, valeur("Descender")?) {
        (Some(a), Some(d)) => Ok(Some((a, d))),
        (None, None) => Ok(None),
        (Some(_), None) | (None, Some(_)) => defaut(format!("le fichier de métriques déposé de {} ne dit que l'un de Ascender et Descender", nom)),
    }
}

/// La largeur (en millièmes d'em) de chaque glyphe de la police standard `nom`, par le nom du glyphe : la section 8
/// (« Individual Character Metrics ») de son fichier, chaque ligne entre `StartCharMetrics` et `EndCharMetrics`, `N`
/// (son nom) et `WX` (« Character width in x for writing direction 0 »). `None` pour un nom qui n'est pas celui d'une
/// police standard.
pub(crate) fn largeurs(nom: &str) -> Result<Option<HashMap<&'static str, Mesure>>, Refus> {
    let Some(afm) = fichier(nom) else {
        return Ok(None);
    };
    let mut lignes = afm.lines();
    let attendues = loop {
        match lignes.next() {
            Some(l) => match l.strip_prefix("StartCharMetrics ") {
                Some(n) => break nombre(nom, "StartCharMetrics", n)?,
                None => continue,
            },
            None => return defaut(format!("le fichier de métriques déposé de {} n'a pas de StartCharMetrics", nom)),
        }
    };
    let mut par_nom = HashMap::new();
    let mut vues = 0;
    for l in lignes {
        if l.trim() == "EndCharMetrics" {
            if Mesure::entier(vues) != attendues {
                return defaut(format!("le fichier de métriques déposé de {} annonce {} glyphes et en décrit {}", nom, attendues, vues));
            }
            return Ok(Some(par_nom));
        }
        let (mut glyphe, mut largeur) = (None, None);
        for champ in l.split(';').map(str::trim) {
            match champ.split_once(' ') {
                Some(("N", n)) => glyphe = Some(n.trim()),
                Some(("WX", w)) => largeur = Some(nombre(nom, "WX", w)?),
                Some(_) | None => {}
            }
        }
        match (glyphe, largeur) {
            (Some(g), Some(w)) => {
                par_nom.insert(g, w);
            }
            (None, _) | (_, None) => return defaut(format!("le fichier de métriques déposé de {} a la ligne « {} », sans N ou sans WX", nom, l)),
        }
        vues += 1;
    }
    defaut(format!("le fichier de métriques déposé de {} n'a pas de EndCharMetrics", nom))
}
