//! Les mesures verticales des polices standard (ISO 32000-1, 9.6.2.2 : les 14 polices que tout lecteur connaît sans
//! qu'elles soient incorporées) : `Ascender` et `Descender` de leurs fichiers de métriques d'Adobe (« Core 14 AFM
//! Files »), déposés tels que servis dans `atelier/reference/sources/adobe-polices/adobe-core14-afm-20261111/` et lus
//! tels quels.

use crate::description::{defaut, Refus};
use crate::mesure::Mesure;

const DOSSIER: [(&str, &str); 12] = [
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
];

/// L'`Ascender` et le `Descender` (en millièmes d'em, le descendant négatif) de la police standard `nom` ; `None` pour
/// un nom qui n'est pas celui d'une police standard dont le fichier les dit (Symbol et ZapfDingbats n'en ont pas).
pub(crate) fn ascendant_descendant(nom: &str) -> Result<Option<(Mesure, Mesure)>, Refus> {
    let Some((_, afm)) = DOSSIER.iter().find(|(n, _)| *n == nom) else {
        return Ok(None);
    };
    let valeur = |cle: &str| -> Result<Mesure, Refus> {
        let ligne = afm.lines().find_map(|l| l.strip_prefix(cle).and_then(|r| r.strip_prefix(' ')));
        match ligne.map(|v| v.trim().parse::<i64>()) {
            Some(Ok(v)) => Ok(Mesure::entier(v)),
            Some(Err(e)) => defaut(format!("le fichier de métriques déposé de {} a un {} illisible ({})", nom, cle, e)),
            None => defaut(format!("le fichier de métriques déposé de {} n'a pas de {}", nom, cle)),
        }
    };
    Ok(Some((valeur("Ascender")?, valeur("Descender")?)))
}
