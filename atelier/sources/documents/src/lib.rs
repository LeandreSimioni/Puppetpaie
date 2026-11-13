//! Les documents : ce qu'un engagement validé fait imprimer, signer ou envoyer. Chacun se fabrique au moment où il part,
//! depuis les paies écrites de l'engagement validé (`engagement::etats`) ; aucun ne recalcule une paie.

pub mod aer;
pub mod avenant;
pub mod bordereau;
pub mod bulletin;
pub mod cachet;
pub mod classeur;
pub mod conges_spectacles;
pub mod contrat;
pub mod contrat_ecrit;
pub mod ecritures_comptables;
pub mod etats_compta;
pub mod ecrire_documents;
pub mod etats_paie;
pub mod etats_personnel;
pub mod fabrique;
pub mod fin_de_contrat;
pub mod marqueurs;
pub mod fiche_questionnaire;
pub mod fiche_recapitulative;
pub mod libelles_courts;
pub mod plan_comptable;
pub mod prelevement;
pub mod tableaux_export;
pub mod taxe_sur_les_salaires;

/// La plus grande largeur, en points, à laquelle un document imprime l'image d'identité `quoi` (`logo`,
/// `signature`, `cachet`) : ce qu'il faut de pixels, et pas plus (`fabrique_pdf::alleger`). Le logo : en tête des
/// bulletins et des contrats (`bulletin::LOGO_LARGEUR`). La signature : dans le contrat
/// (`fabrique_pdf::style_contrat::SIGNATURE_LARGEUR`), le certificat Congés Spectacles (`conges_spectacles`, son cadre)
/// et l'AER (`aer::cadre_signature` dans la case du formulaire, `aer::CASE_SIGNATURE`). Le tampon : la case cachet de
/// l'AER (`aer::CASE_CACHET`).
pub fn largeur_imprimee(quoi: &str) -> Option<Mesure> {
    match quoi {
        "logo" => Some(bulletin::LOGO_LARGEUR),
        "signature" => [
            fabrique_pdf::style_contrat::SIGNATURE_LARGEUR,
            conges_spectacles::SIGNATURE_LARGEUR,
            { let r = aer::cadre_signature([Mesure::ZERO, Mesure::ZERO, aer::CASE_SIGNATURE, Mesure::UN]); r[2] - r[0] },
        ]
        .into_iter()
        .max(),
        "cachet" => Some(aer::CASE_CACHET),
        _ => None,
    }
}

pub use fabrique_pdf::forme::Mesure;

/// UNE SORTE DE PIECE DU DOSSIER D'UN SALARIE (`dossier-salarie`, `dossier-envoyer`) : une liste du logiciel, dans
/// l'ordre ou le dossier se lit (`TOUTES`, l'ordre du type). Elle s'ecrit par son mot (`bulletins`, `contrats-signes`…),
/// que la charge dit et que le journal des envois garde du dossier parti ; un autre mot se refuse a la lecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CategorieDeDocument {
    Bulletins,
    Contrats,
    ContratsSignes,
    CertificatsCs,
    Attestations,
    DocumentsFin,
    Recapitulatif,
}

impl CategorieDeDocument {
    /// Toutes, dans l'ordre ou le dossier se lit.
    pub const TOUTES: [CategorieDeDocument; 7] = [
        CategorieDeDocument::Bulletins,
        CategorieDeDocument::Contrats,
        CategorieDeDocument::ContratsSignes,
        CategorieDeDocument::CertificatsCs,
        CategorieDeDocument::Attestations,
        CategorieDeDocument::DocumentsFin,
        CategorieDeDocument::Recapitulatif,
    ];

    /// Son mot, tel que la charge et le journal l'ecrivent.
    pub fn texte(self) -> &'static str {
        match self {
            CategorieDeDocument::Bulletins => "bulletins",
            CategorieDeDocument::Contrats => "contrats",
            CategorieDeDocument::ContratsSignes => "contrats-signes",
            CategorieDeDocument::CertificatsCs => "certificats-cs",
            CategorieDeDocument::Attestations => "attestations",
            CategorieDeDocument::DocumentsFin => "documents-fin",
            CategorieDeDocument::Recapitulatif => "recapitulatif",
        }
    }

    /// Son libelle, tel que la page et le courriel du dossier le disent.
    pub fn libelle(self) -> &'static str {
        match self {
            CategorieDeDocument::Bulletins => "Bulletins de paie",
            CategorieDeDocument::Contrats => "Contrats",
            CategorieDeDocument::ContratsSignes => "Contrats signés",
            CategorieDeDocument::CertificatsCs => "Certificats Congés Spectacles",
            CategorieDeDocument::Attestations => "Attestations employeur (AER, AEM)",
            CategorieDeDocument::DocumentsFin => "Documents de fin de contrat",
            CategorieDeDocument::Recapitulatif => "Fiche récapitulative",
        }
    }
}

impl<'de> serde::Deserialize<'de> for CategorieDeDocument {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let mot = String::deserialize(d)?;
        match CategorieDeDocument::TOUTES.into_iter().find(|c| c.texte() == mot) {
            Some(c) => Ok(c),
            None => Err(serde::de::Error::custom(format!(
                "Catégorie inconnue : {}. Les catégories sont {}.",
                mot,
                CategorieDeDocument::TOUTES.map(CategorieDeDocument::texte).join(", ")
            ))),
        }
    }
}
