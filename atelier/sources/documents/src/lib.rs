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

/// UNE SORTE DE PIECE PRODUITE : ce que le moteur ecrit dans les sorties d'un mois (`sorties/<mois>/`, son `envoi/`),
/// dans celles des exercices (`sorties/exercices/`) ou dans un dossier de salaries, dit au debut du nom de son fichier.
/// Une liste du logiciel ; une seule fonction ecrit ce debut (`prefixe`), une seule le relit (`du_nom`), une seule dit
/// si la piece est rangee dans `sorties/<mois>/envoi/` (`dans_l_envoi`). Aucun prefixe n'en commence un autre : un nom
/// n'est que d'une sorte. Ce n'est pas une categorie du dossier d'un salarie (`CategorieDeDocument`) : la DPAE, la DSN,
/// l'amorcage n'en sont pas. Un fichier de travail (un PDF temporaire) n'en est pas une, ni une piece recue d'un
/// organisme (l'AER de France Travail, archivee avec son retour), ni un fichier de l'ancien logiciel (`AEM_`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SorteDePieceProduite {
    /// Les bulletins d'un salarie (`Bulletin_<cle>_<Mois>.pdf`), et celui d'origine garde a cote de son rectificatif.
    Bulletin,
    /// L'archive des bulletins du mois (`Bulletins_<Mois>.pdf`).
    Bulletins,
    /// Les certificats Conges Spectacles d'un salarie (`CertificatCS_<cle>_<Mois>.pdf`).
    CertificatCs,
    /// L'archive des certificats du mois et son recapitulatif (`CertificatsCS_<Mois>.pdf`, `…_recapitulatif.txt`).
    CertificatsCs,
    /// L'attestation employeur France Travail d'un salarie, rangee pour l'envoi (`AER_<cle>_<Mois>.pdf`).
    Aer,
    /// Le contrat, ou les contrats, d'un salarie (`Contrat_…`, `Contrats_…`).
    Contrat,
    /// Un avenant (`Avenant<n>_…`).
    Avenant,
    /// La DPAE d'un mois, entiere ou a la minute, et son recapitulatif (`DPAE_<Mois>….txt`).
    Dpae,
    /// La DSN mensuelle (`DSN_<Mois>….dsn`).
    Dsn,
    /// Un signalement d'amorcage, et la note du lot (`Amorcage<motif>_…`).
    Amorcage,
    /// Le certificat de travail (`CertificatTravail_<cle>_<AAAAMMJJ>.pdf`).
    CertificatDeTravail,
    /// Le recu pour solde de tout compte (`RecuSoldeToutCompte_<cle>_<AAAAMMJJ>.pdf`).
    RecuSoldeToutCompte,
    /// Le dossier d'un salarie (`Dossier_<nom>_<periode>…`).
    Dossier,
    /// Le dossier qui reunit tous les salaries (`Dossiers_<periode>…`).
    Dossiers,
    /// Le PDF a imprimer, qui reunit les pieces parties d'un mois (`Impression_<quoi>_<Mois>.pdf`) : la piece de personne.
    Impression,
    /// La preuve d'une signature, le courriel tel qu'il est arrive (`Signature_<cle>_<code>.eml`).
    Signature,
    /// Le signalement d'un arret de travail (`SignalementArret_<cle>_<AAAAMMJJ>….dsn`).
    SignalementArret,
    /// Le signalement d'une reprise apres un arret (`SignalementReprise_<cle>_<AAAAMMJJ>….dsn`).
    SignalementReprise,
    /// Le signalement d'une fin de contrat (`SignalementFinContrat_<cle>_<AAAAMMJJ>….dsn`).
    SignalementFinContrat,
    /// L'export comptable d'une annee, ou d'une partie (`Compta_<annee>….csv`).
    Compta,
    /// L'archive d'un exercice clos (`Exercice_<annee>.zip`).
    Exercice,
}

impl SorteDePieceProduite {
    /// Toutes.
    pub const TOUTES: [SorteDePieceProduite; 21] = [
        SorteDePieceProduite::Bulletin,
        SorteDePieceProduite::Bulletins,
        SorteDePieceProduite::CertificatCs,
        SorteDePieceProduite::CertificatsCs,
        SorteDePieceProduite::Aer,
        SorteDePieceProduite::Contrat,
        SorteDePieceProduite::Avenant,
        SorteDePieceProduite::Dpae,
        SorteDePieceProduite::Dsn,
        SorteDePieceProduite::Amorcage,
        SorteDePieceProduite::CertificatDeTravail,
        SorteDePieceProduite::RecuSoldeToutCompte,
        SorteDePieceProduite::Dossier,
        SorteDePieceProduite::Dossiers,
        SorteDePieceProduite::Impression,
        SorteDePieceProduite::Signature,
        SorteDePieceProduite::SignalementArret,
        SorteDePieceProduite::SignalementReprise,
        SorteDePieceProduite::SignalementFinContrat,
        SorteDePieceProduite::Compta,
        SorteDePieceProduite::Exercice,
    ];

    /// Le debut du nom de son fichier.
    pub const fn prefixe(self) -> &'static str {
        match self {
            SorteDePieceProduite::Bulletin => "Bulletin_",
            SorteDePieceProduite::Bulletins => "Bulletins_",
            SorteDePieceProduite::CertificatCs => "CertificatCS_",
            SorteDePieceProduite::CertificatsCs => "CertificatsCS_",
            SorteDePieceProduite::Aer => "AER_",
            SorteDePieceProduite::Contrat => "Contrat",
            SorteDePieceProduite::Avenant => "Avenant",
            SorteDePieceProduite::Dpae => "DPAE_",
            SorteDePieceProduite::Dsn => "DSN_",
            SorteDePieceProduite::Amorcage => "Amorcage",
            SorteDePieceProduite::CertificatDeTravail => "CertificatTravail_",
            SorteDePieceProduite::RecuSoldeToutCompte => "RecuSoldeToutCompte_",
            SorteDePieceProduite::Dossier => "Dossier_",
            SorteDePieceProduite::Dossiers => "Dossiers_",
            SorteDePieceProduite::Impression => "Impression_",
            SorteDePieceProduite::Signature => "Signature_",
            SorteDePieceProduite::SignalementArret => "SignalementArret_",
            SorteDePieceProduite::SignalementReprise => "SignalementReprise_",
            SorteDePieceProduite::SignalementFinContrat => "SignalementFinContrat_",
            SorteDePieceProduite::Compta => "Compta_",
            SorteDePieceProduite::Exercice => "Exercice_",
        }
    }

    /// Rangee dans `sorties/<mois>/envoi/` : la piece qui part au salarie (la sienne, le PDF qui les reunit pour
    /// l'imprimer, la preuve de sa signature) ; les autres sont dans `sorties/<mois>/`, `sorties/exercices/` ou un dossier
    /// de salaries. Le bulletin d'origine garde a cote de son rectificatif est, lui, dans `sorties/<mois>/`.
    pub const fn dans_l_envoi(self) -> bool {
        match self {
            SorteDePieceProduite::Bulletin
            | SorteDePieceProduite::CertificatCs
            | SorteDePieceProduite::Aer
            | SorteDePieceProduite::Contrat
            | SorteDePieceProduite::Avenant
            | SorteDePieceProduite::Impression
            | SorteDePieceProduite::Signature => true,
            SorteDePieceProduite::Bulletins
            | SorteDePieceProduite::CertificatsCs
            | SorteDePieceProduite::Dpae
            | SorteDePieceProduite::Dsn
            | SorteDePieceProduite::Amorcage
            | SorteDePieceProduite::CertificatDeTravail
            | SorteDePieceProduite::RecuSoldeToutCompte
            | SorteDePieceProduite::Dossier
            | SorteDePieceProduite::Dossiers
            | SorteDePieceProduite::SignalementArret
            | SorteDePieceProduite::SignalementReprise
            | SorteDePieceProduite::SignalementFinContrat
            | SorteDePieceProduite::Compta
            | SorteDePieceProduite::Exercice => false,
        }
    }

    /// La sorte du fichier `nom`, et ce qui suit son prefixe ; aucune si le nom ne commence par aucun.
    pub fn du_nom(nom: &str) -> Option<(SorteDePieceProduite, &str)> {
        SorteDePieceProduite::TOUTES.into_iter().find_map(|s| nom.strip_prefix(s.prefixe()).map(|reste| (s, reste)))
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
