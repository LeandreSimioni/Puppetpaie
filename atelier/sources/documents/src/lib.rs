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

/// UNE IMAGE DE L'IDENTITE DE LA COMPAGNIE : sa signature, son cachet (le tampon), son logo. Une liste du logiciel,
/// dans l'ordre ou l'ecran les montre (`TOUTES`) ; elle s'ecrit par son nom (`signature`, `cachet`, `logo`), celui du
/// fichier pose (`gabarits/identite/<nom>.png`) et de la charge qui la designe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SorteDImage {
    Signature,
    Cachet,
    Logo,
}

impl SorteDImage {
    pub const TOUTES: [SorteDImage; 3] = [SorteDImage::Signature, SorteDImage::Cachet, SorteDImage::Logo];

    /// Son nom.
    pub fn nom(self) -> &'static str {
        match self {
            SorteDImage::Signature => "signature",
            SorteDImage::Cachet => "cachet",
            SorteDImage::Logo => "logo",
        }
    }

    /// Celle de ce nom, si c'en est une.
    pub fn du_nom(nom: &str) -> Option<SorteDImage> {
        SorteDImage::TOUTES.into_iter().find(|s| s.nom() == nom)
    }

    /// La plus grande largeur, en points, à laquelle un document l'imprime : ce qu'il faut de pixels, et pas plus
    /// (`fabrique_pdf::alleger`). Le logo : en tête des bulletins et des contrats (`bulletin::LOGO_LARGEUR`). La
    /// signature : dans le contrat (`fabrique_pdf::style_contrat::SIGNATURE_LARGEUR`), le certificat Congés Spectacles
    /// (`conges_spectacles`, son cadre) et l'AER (`aer::cadre_signature` dans la case du formulaire, `aer::CASE_SIGNATURE`).
    /// Le tampon : la case cachet de l'AER (`aer::CASE_CACHET`).
    pub fn largeur_imprimee(self) -> Mesure {
        match self {
            SorteDImage::Logo => bulletin::LOGO_LARGEUR,
            SorteDImage::Signature => {
                let aer = { let r = aer::cadre_signature([Mesure::ZERO, Mesure::ZERO, aer::CASE_SIGNATURE, Mesure::UN]); r[2] - r[0] };
                fabrique_pdf::style_contrat::SIGNATURE_LARGEUR.max(conges_spectacles::SIGNATURE_LARGEUR).max(aer)
            }
            SorteDImage::Cachet => aer::CASE_CACHET,
        }
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
