//! La contribution CPF-CDD (code du travail L. 6331-6) : un contrat l'ouvre-t-il ? La meme regle pour le bulletin, le
//! nominatif (cotisation 129), le bordereau (CTP 987) et le bloc S21.G00.44 (013/014). Le droit la juge sur ce que la
//! DSN ecrit d'un contrat : nature (40.007), dispositif (40.008), motif de recours (40.021).

use socle::echec::R;

use crate::natures::{MotifDeRecours, Nature, NatureDeContrat};

/// Le contrat d'un salarie dans un message : « le contrat VA5.26.01 de VARENNE », « le contrat de VARENNE » tant qu'il n'a
/// pas de numero.
pub fn le_contrat(numero: Option<&str>, salarie: &crate::contexte::IdSalarie) -> String {
    match numero.filter(|n| !n.is_empty()) {
        Some(n) => format!("le contrat {} de {}", n, salarie),
        None => format!("le contrat de {}", salarie),
    }
}

/// Le contrat de nature `nature` ouvre-t-il la contribution CPF-CDD ? Il se juge sur ce que la DSN en ecrit
/// (`codes_de` : le code de la nature, son dispositif, le motif de recours de la nature, sinon celui de l'engagement).
/// C'est la ou le calcul lit le motif de recours saisi : un CDD sans motif refuse (le motif dit s'il est saisonnier), un
/// motif que la nature n'a pas aussi (`Nature::motif_recours_refuse`). `qui` nomme le contrat dans un refus
/// (`le_contrat`).
pub fn ouvre_cpf_cdd(nature: NatureDeContrat, motif_recours: Option<MotifDeRecours>, qui: &str) -> R<bool> {
    let nat = nature.proprietes()?;
    if let Some(refus) = nat.motif_recours_fixe().is_none().then(|| motif_recours.and_then(|m| nat.motif_recours_refuse(m, qui))).flatten() {
        return Err(socle::echec::refus(refus));
    }
    let (nature, dispositif, motif) = codes_de(&nat, motif_recours);
    let Some(nature) = nature else {
        return Err(socle::refus!("La nature de {} ne dit pas son code (S21.G00.40.007) : le signaler au développeur.", qui));
    };
    // « titulaires d'un contrat à durée déterminée » : la nature 02 (CDD de droit prive) seule.
    if nature != "02" {
        return Ok(false);
    }
    match motif {
        // L. 6331-55 : la participation unique des employeurs d'intermittents du spectacle, « par dérogation [...] à
        // l'article L. 6331-6 ». INFERENCE : le CDD d'usage (05) d'une compagnie est celui d'un intermittent.
        Some(MotifDeRecours::ContratDUsage) => return Ok(false),
        // L. 6331-6 : « ceux visant les salariés occupant un emploi à caractère saisonnier défini au 3° de l'article
        // L. 1242-2 ». Le contrat vendanges (04) est un emploi saisonnier (INFERENCE).
        Some(MotifDeRecours::Saisonnier | MotifDeRecours::Vendanges) => return Ok(false),
        None => {
            return Err(socle::refus!(
                "Dites pourquoi ce CDD est conclu (« Motif du CDD », sur la carte de l'engagement) : {} n'en dit pas le motif.\n\
Le motif dit si le contrat ouvre la contribution CPF-CDD (code du travail, L. 6331-6 : pas un CDD saisonnier) ; la DSN \
le déclare (S21.G00.40.021).",
                qui
            ))
        }
        _ => {}
    }
    // D. 6331-72 : contrats d'accompagnement dans l'emploi, d'apprentissage, de professionnalisation, et ceux de
    // L. 6321-9 -- tous portent un dispositif de politique publique (40.008), et le moteur n'en ecrit aucun.
    if let Some(dispositif) = dispositif.filter(|d| d != "99") {
        return Err(socle::refus!(
            "Un CDD sous le dispositif de politique publique {1} (S21.G00.40.008), {0}. D. 6331-72 du code du travail en \
exclut certains de la contribution CPF-CDD (contrats d'accompagnement dans l'emploi, d'apprentissage, de \
professionnalisation, ceux de L. 6321-9) : le moteur ne sait pas lequel c'est, et ne declare pas le bloc \
S21.G00.44 sur une supposition.",
            qui, dispositif
        ));
    }
    Ok(true)
}

/// Ce que la DSN ecrit d'un contrat de nature `nat` : nature, dispositif, motif (celui de la nature, sinon celui de
/// l'engagement).
/// Un code que la nature ni l'engagement ne disent est `None`.
pub fn codes_de(nat: &Nature, motif_recours: Option<MotifDeRecours>) -> (Option<String>, Option<String>, Option<MotifDeRecours>) {
    use crate::natures::RubriqueDeLaNorme;
    const NATURE: RubriqueDeLaNorme = RubriqueDeLaNorme::du_moteur("S21.G00.40.007");
    const DISPOSITIF: RubriqueDeLaNorme = RubriqueDeLaNorme::du_moteur("S21.G00.40.008");
    (nat.code(NATURE), nat.code(DISPOSITIF), nat.motif_recours_fixe().or(motif_recours))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn le_cpf_cdd_suit_la_nature_et_le_motif() {
        assert!(!ouvre_cpf_cdd(NatureDeContrat::Cdi, None, "x").unwrap(), "un CDI");
        assert!(!ouvre_cpf_cdd(NatureDeContrat::CdduSpectacle, None, "x").unwrap(), "un CDD d'usage : L. 6331-55");
        assert!(!ouvre_cpf_cdd(NatureDeContrat::Cdd, Some(MotifDeRecours::Saisonnier), "x").unwrap(), "un saisonnier : L. 6331-6");
        assert!(ouvre_cpf_cdd(NatureDeContrat::Cdd, Some(MotifDeRecours::Accroissement), "x").unwrap(), "un accroissement d'activite");
        assert!(ouvre_cpf_cdd(NatureDeContrat::Cdd, None, "x").is_err(), "sans motif, on ne sait pas");
    }
}
