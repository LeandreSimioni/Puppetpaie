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

/// LES SORTES D'UNE PIECE PRODUITE, DECLAREES AVEC LEUR PREFIXE ET LEUR RANGEMENT ENSEMBLE, une ligne chacune : chaque
/// variante est dans `TOUTES`, une fois, et a son prefixe et son rangement, par construction. Une piece d'un mois a son
/// type (`PieceDuMois`, son dossier dit sans condition), une piece gardee aussi (une sorte d'un mois rangee ailleurs).
macro_rules! sortes_de_pieces_produites {
    (
        du mois { $( $(#[$dm:meta])* $m:ident = $pm:literal, $dossier:ident; )* }
        hors du mois { $( $(#[$dh:meta])* $h:ident = $ph:literal, $hors:ident; )* }
        gardees { $( $(#[$dg:meta])* $g:ident = $sg:ident, $dossier_g:ident; )* }
    ) => {
        /// UNE SORTE DE PIECE PRODUITE : ce que le moteur ecrit dans les sorties d'un mois (`sorties/<mois>/`, son
        /// `envoi/`), dans celles des exercices (`sorties/exercices/`) ou dans les dossiers de salaries
        /// (`exports/dossiers/`), dit au debut du nom de son fichier. Une liste du logiciel, declaree avec son prefixe et
        /// son rangement (`sortes_de_pieces_produites!`) ; une seule fonction ecrit ce debut (`prefixe`), une seule le
        /// relit (`du_nom`), une seule dit ou elle se range (`rangement` ; le dossier d'une piece d'un mois,
        /// `PieceDuMois::dossier`). Aucun prefixe n'en commence un autre : un nom n'est que d'une sorte. Ce n'est pas une
        /// categorie du dossier d'un salarie (`CategorieDeDocument`) : la DPAE, la DSN, l'amorcage n'en sont pas. Un
        /// fichier de travail (un PDF temporaire) n'en est pas une, ni une piece recue d'un organisme (l'AER de France
        /// Travail, dont le nom commence aussi par `AER_` : une entree de l'archive de son retour, dans `retours/`, ou,
        /// qu'aucune fiche ne porte, un fichier de `retours/aer-a-ranger/` ; jamais dans les sorties, ou se lisent les
        /// pieces produites), ni un fichier de l'ancien logiciel (`AEM_`).
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum SorteDePieceProduite {
            $( $(#[$dm])* $m, )*
            $( $(#[$dh])* $h, )*
        }

        /// UNE PIECE D'UN MOIS : une sorte qui se range dans un dossier d'un mois (ses sorties ou leur envoi), ou une
        /// piece gardee (une sorte d'un mois qui se range ailleurs que les siennes : le bulletin d'origine garde a cote de
        /// son rectificatif). Son dossier se dit sans condition (`dossier`) ; une sorte hors d'un mois n'en est pas une.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum PieceDuMois {
            $( $(#[$dm])* $m, )*
            $( $(#[$dg])* $g, )*
        }

        impl SorteDePieceProduite {
            /// Toutes, chacune une fois.
            pub const TOUTES: &'static [SorteDePieceProduite] = &[ $( SorteDePieceProduite::$m, )* $( SorteDePieceProduite::$h, )* ];

            /// Le debut du nom de son fichier.
            pub const fn prefixe(self) -> &'static str {
                match self {
                    $( SorteDePieceProduite::$m => $pm, )*
                    $( SorteDePieceProduite::$h => $ph, )*
                }
            }

            /// OU ELLE SE RANGE : dans un dossier d'un mois (`DossierDuMois`), ou hors d'un mois (`DossierHorsDuMois`).
            pub const fn rangement(self) -> Rangement {
                match self {
                    $( SorteDePieceProduite::$m => Rangement::DuMois(DossierDuMois::$dossier), )*
                    $( SorteDePieceProduite::$h => Rangement::HorsDuMois(DossierHorsDuMois::$hors), )*
                }
            }

            /// Sa piece d'un mois ; aucune pour une sorte qui se range hors d'un mois.
            pub const fn du_mois(self) -> Option<PieceDuMois> {
                match self {
                    $( SorteDePieceProduite::$m => Some(PieceDuMois::$m), )*
                    $( SorteDePieceProduite::$h => None, )*
                }
            }
        }

        impl PieceDuMois {
            /// Sa sorte : celle de son nom.
            pub const fn sorte(self) -> SorteDePieceProduite {
                match self {
                    $( PieceDuMois::$m => SorteDePieceProduite::$m, )*
                    $( PieceDuMois::$g => SorteDePieceProduite::$sg, )*
                }
            }

            /// LE DOSSIER DU MOIS ou elle se range, que son ecrivain et ses lecteurs prennent ici.
            pub const fn dossier(self) -> DossierDuMois {
                match self {
                    $( PieceDuMois::$m => DossierDuMois::$dossier, )*
                    $( PieceDuMois::$g => DossierDuMois::$dossier_g, )*
                }
            }
        }
    };
}

sortes_de_pieces_produites! {
    // Dans l'envoi du mois (`sorties/<mois>/envoi/`) : ce que le moteur prepare pour les salaries (leurs pieces, le PDF
    // qui les reunit pour l'imprimer, la preuve d'une signature) ; dans les sorties du mois (`sorties/<mois>/`) : les
    // archives du mois, les declarations, les documents de fin de contrat.
    du mois {
        /// Les bulletins d'un salarie (`Bulletin_<cle>_<Mois>.pdf`).
        Bulletin = "Bulletin_", Envoi;
        /// L'archive des bulletins du mois (`Bulletins_<Mois>.pdf`).
        Bulletins = "Bulletins_", Sorties;
        /// Les certificats Conges Spectacles d'un salarie (`CertificatCS_<cle>_<Mois>.pdf`).
        CertificatCs = "CertificatCS_", Envoi;
        /// L'archive des certificats du mois et son recapitulatif (`CertificatsCS_<Mois>.pdf`, `…_recapitulatif.txt`).
        CertificatsCs = "CertificatsCS_", Sorties;
        /// L'attestation employeur France Travail d'un salarie, rangee pour l'envoi (`AER_<cle>_<Mois>.pdf`).
        Aer = "AER_", Envoi;
        /// Le contrat, ou les contrats, d'un salarie (`Contrat_…`, `Contrats_…`).
        Contrat = "Contrat", Envoi;
        /// Un avenant (`Avenant<n>_…`).
        Avenant = "Avenant", Envoi;
        /// La DPAE d'un mois, entiere ou a la minute, et son recapitulatif (`DPAE_<Mois>….txt`).
        Dpae = "DPAE_", Sorties;
        /// La DSN mensuelle (`DSN_<Mois>….dsn`).
        Dsn = "DSN_", Sorties;
        /// Un signalement d'amorcage, et la note du lot (`Amorcage<motif>_…`).
        Amorcage = "Amorcage", Sorties;
        /// Le certificat de travail (`CertificatTravail_<cle>_<AAAAMMJJ>.pdf`).
        CertificatDeTravail = "CertificatTravail_", Sorties;
        /// Le recu pour solde de tout compte (`RecuSoldeToutCompte_<cle>_<AAAAMMJJ>.pdf`).
        RecuSoldeToutCompte = "RecuSoldeToutCompte_", Sorties;
        /// Le PDF a imprimer, qui reunit les pieces parties d'un mois (`Impression_<quoi>_<Mois>.pdf`) : la piece de personne.
        Impression = "Impression_", Envoi;
        /// La preuve d'une signature, le courriel tel qu'il est arrive (`Signature_<cle>_<code>.eml`).
        Signature = "Signature_", Envoi;
        /// Le signalement d'un arret de travail (`SignalementArret_<cle>_<AAAAMMJJ>….dsn`).
        SignalementArret = "SignalementArret_", Sorties;
        /// Le signalement d'une reprise apres un arret (`SignalementReprise_<cle>_<AAAAMMJJ>….dsn`).
        SignalementReprise = "SignalementReprise_", Sorties;
        /// Le signalement d'une fin de contrat (`SignalementFinContrat_<cle>_<AAAAMMJJ>….dsn`).
        SignalementFinContrat = "SignalementFinContrat_", Sorties;
    }
    hors du mois {
        /// Le dossier d'un salarie (`Dossier_<nom>_<periode>…`).
        Dossier = "Dossier_", DossiersDeSalaries;
        /// Le dossier qui reunit tous les salaries (`Dossiers_<periode>…`).
        Dossiers = "Dossiers_", DossiersDeSalaries;
        /// L'export comptable d'une annee, ou d'une partie (`Compta_<annee>….csv`).
        Compta = "Compta_", Exercices;
        /// L'archive d'un exercice clos (`Exercice_<annee>.zip`).
        Exercice = "Exercice_", Exercices;
        /// La reponse a une demande de fiche, le courriel tel qu'il est arrive (`Fiche_<code>.eml`).
        FicheDeSalarie = "Fiche_", FichesDeSalaries;
    }
    gardees {
        /// LE BULLETIN D'ORIGINE GARDE A COTE DE SON RECTIFICATIF (`Bulletin_<cle>_<Mois>_<N-AAAA>.pdf`) : de la sorte
        /// `Bulletin` par son nom, mais il ne part a personne (ce qui est parti se garde) : il se range dans les sorties
        /// du mois, pas dans l'envoi.
        BulletinGarde = Bulletin, Sorties;
    }
}

impl SorteDePieceProduite {
    /// La sorte du fichier `nom`, et ce qui suit son prefixe ; aucune si le nom ne commence par aucun.
    pub fn du_nom(nom: &str) -> Option<(SorteDePieceProduite, &str)> {
        SorteDePieceProduite::TOUTES.iter().find_map(|s| nom.strip_prefix(s.prefixe()).map(|reste| (*s, reste)))
    }
}

/// LE FICHIER D'UNE DECLARATION DEPOSEE (le nom qu'une ligne du journal donne : une DSN, une DPAE, un signalement), sous
/// les sorties de son mois `sorties` (`sorties/<mois>/`), dans le dossier ou sa sorte se range (`du_nom`, `du_mois`). Un
/// nom d'aucune sorte d'un mois n'est pas un fichier que le moteur ecrit : il se refuse, comme une faute du logiciel.
pub fn fichier_depose(sorties: &std::path::Path, nom: &str) -> socle::echec::R<std::path::PathBuf> {
    match SorteDePieceProduite::du_nom(nom).and_then(|(s, _)| s.du_mois()) {
        Some(p) => Ok(p.dossier().sous(sorties).join(nom)),
        None => Err(socle::echec::erreur(
            socle::echec::Classe::DefautDuLogiciel,
            format!("« {} » : le journal nomme un fichier déposé qui n'est d'aucune pièce d'un mois : le signaler au développeur.", nom),
        )),
    }
}

/// OU SE RANGE UNE PIECE PRODUITE (`SorteDePieceProduite::rangement`) : dans un dossier d'un mois, ou hors d'un mois.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rangement {
    DuMois(DossierDuMois),
    HorsDuMois(DossierHorsDuMois),
}

/// LE DOSSIER HORS D'UN MOIS ou se range une piece produite : les exercices (`sorties/exercices/`), les dossiers de
/// salaries (`exports/dossiers/`), les reponses aux demandes de fiche (`sorties/fiches-salaries/`). Leur chemin n'est
/// ecrit qu'ici (`sous`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DossierHorsDuMois {
    Exercices,
    DossiersDeSalaries,
    FichesDeSalaries,
}

impl DossierHorsDuMois {
    /// Ce dossier, sous la compagnie `racine`.
    pub fn sous(self, racine: &std::path::Path) -> std::path::PathBuf {
        match self {
            DossierHorsDuMois::Exercices => racine.join("sorties").join("exercices"),
            DossierHorsDuMois::DossiersDeSalaries => racine.join("exports").join("dossiers"),
            DossierHorsDuMois::FichesDeSalaries => racine.join("sorties").join("fiches-salaries"),
        }
    }
}

/// LE DOSSIER D'UN MOIS ou se range une piece produite (`PieceDuMois::dossier`) : ses sorties (`sorties/<mois>/`), ou
/// leur envoi (`sorties/<mois>/envoi/`), ou le moteur prepare ce qui part aux salaries (leurs pieces, le PDF qui les
/// reunit pour l'imprimer, la preuve d'une signature). Le nom `envoi` n'est ecrit qu'ici.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DossierDuMois {
    Sorties,
    Envoi,
}

impl DossierDuMois {
    /// Les deux, l'envoi d'abord : la ou se cherche une piece d'un mois dont on ne sait que le nom (un nom de sorte
    /// `Bulletin` est dans l'envoi, ou, garde a cote de son rectificatif, dans les sorties : `PieceDuMois::BulletinGarde`).
    pub const TOUS: [DossierDuMois; 2] = [DossierDuMois::Envoi, DossierDuMois::Sorties];

    /// Son nom sous les sorties du mois ; aucun : ce sont elles.
    const fn nom(self) -> Option<socle::fichiers::NomDeFichier> {
        match self {
            DossierDuMois::Sorties => None,
            DossierDuMois::Envoi => Some(socle::fichiers::NomDeFichier::du_moteur("envoi")),
        }
    }

    /// Ce dossier, sous les sorties du mois `sorties` (`sorties/<mois>/`).
    pub fn sous(self, sorties: &std::path::Path) -> std::path::PathBuf {
        match self.nom() {
            Some(n) => sorties.join(n.texte()),
            None => sorties.to_path_buf(),
        }
    }

    /// Ce dossier, en chemin relatif a la compagnie, sous les sorties du mois `sorties` (`CheminRelatif::sorties`).
    pub fn sous_le_chemin(self, sorties: &socle::fichiers::CheminRelatif) -> socle::fichiers::CheminRelatif {
        match self.nom() {
            Some(n) => sorties.joint(&n),
            None => sorties.clone(),
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
