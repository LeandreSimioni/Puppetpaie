//! « Remplir ma fiche » : le formulaire que le moteur declare, le lien du courriel, la reponse typee et sa lecture.
//!
//! UNE SEULE DECLARATION (`Question`, `Question::declaree`) sert a tout : au formulaire publie sur puppetpaie.fr
//! (`formulaire_publie`, un fichier `.js` par forme, nomme par son empreinte, que la page charge et dessine), au lien du
//! courriel (`LienDeFiche`, qui nomme les questions a poser, jamais ce que la compagnie sait deja), aux lignes a recopier
//! sans la page (`a_recopier`), et a la lecture de la reponse (`lire_reponse`, puis `fiche_depuis_reponses`). La page ne
//! connait aucune question a l'avance ; ce qu'elle verifie pendant la saisie, le moteur le revérifie ici (regle de fond 2).
//!
//! LA REPONSE est une ligne par question, `cle : valeur`, et `aucun` pour « je n'en ai pas » ; le code de la demande
//! en derniere ligne (`code : FIC-…`). « aucun » est une reponse ; une ligne absente (ou sans valeur) n'en est pas une ;
//! une ligne mal formee, ou une valeur que son type refuse, est illisible et se redemande : vide n'est pas illisible.
//!
//! Les deux tables (nationalites) ne portent que le code ISO : la codification UE se deduit dans `droit::nationalite`.
//! Les pays se lisent dans la liste livree (`pays.rs`, COG de l'INSEE). Le departement et le pays de naissance se
//! lisent dans le NIR.

use serde::{Deserialize, Serialize};

use droit::fiche::{AccordDfs, CaseSalarie, ModeleBulletin, Nir, NumeroCongesSpectacles};
use socle::date::Jour;
use socle::echec::R;
use socle::fichiers::{Empreinte, EmpreinteCourte};
use socle::texte::{blanc as espace, strip};

pub const ALPHABET_CODE: &str = "ABCDEFGHJKMNPQRSTUVWXYZ23456789";

/// Ce qu'une demande demande, dit par le prefixe de son code : une signature (`SIG-`), une fiche (`FIC-`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SorteDeDemande {
    Signature,
    Fiche,
}

impl SorteDeDemande {
    /// Le prefixe de son code.
    pub fn prefixe(self) -> &'static str {
        match self {
            SorteDeDemande::Signature => "SIG-",
            SorteDeDemande::Fiche => "FIC-",
        }
    }
}

/// LE CODE D'UNE DEMANDE : ce qu'une demande de signature ou de fiche porte, et qu'on cherche dans la reponse. Son
/// prefixe dit la sorte de demande (`SorteDeDemande`), puis six signes de `ALPHABET_CODE` (sans 0/O ni 1/I/L, pour
/// qu'il se relise sans faute). Seul `code_au_hasard` le fait ; il se lit strictement par son texte (`lire`). Une
/// reponse recue est le texte d'un tiers : on y cherche `texte()`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CodeDeDemande(String);

impl CodeDeDemande {
    /// Le code ecrit `texte` ; refus s'il n'en a pas la forme.
    pub fn lire(texte: &str) -> R<CodeDeDemande> {
        let signes = [SorteDeDemande::Signature, SorteDeDemande::Fiche].into_iter().find_map(|s| texte.strip_prefix(s.prefixe()));
        match signes {
            Some(s) if s.chars().count() == 6 && s.chars().all(|c| ALPHABET_CODE.contains(c)) => Ok(CodeDeDemande(texte.to_string())),
            _ => Err(socle::refus!(
                "Code de demande {} : il s'écrit « SIG- » ou « FIC- » puis six signes parmi {}.",
                socle::fichiers::repr(texte),
                ALPHABET_CODE
            )),
        }
    }

    /// Le code tel qu'ecrit.
    pub fn texte(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CodeDeDemande {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for CodeDeDemande {
    type Error = socle::echec::Echec;
    fn try_from(s: String) -> R<CodeDeDemande> {
        CodeDeDemande::lire(&s)
    }
}

impl From<CodeDeDemande> for String {
    fn from(c: CodeDeDemande) -> String {
        c.0
    }
}

/// Un code a six signes derriere le prefixe de sa sorte (`SIG-` d'une signature, `FIC-` d'une fiche demandee a la
/// personne). Le systeme qui ne donne pas d'aleas fait refuser : un code devinable ne protege rien.
pub fn code_au_hasard(sorte: SorteDeDemande) -> R<CodeDeDemande> {
    let prefixe = sorte.prefixe();
    let alphabet = ALPHABET_CODE.as_bytes();
    let limite = 256 - 256 % alphabet.len();
    let mut code = String::from(prefixe);
    // UN TIRAGE PAR CODE, jamais un par signe : un octet rejete ne decale pas la suite des aleas qui suivent (au rejeu,
    // `socle::temps::aleas` est une suite fixe, que chaque tirage fait avancer).
    while code.len() < prefixe.len() + 6 {
        let mut b = [0u8; 32];
        socle::temps::aleas(&mut b)?;
        for x in b.iter().filter(|x| (**x as usize) < limite).take(prefixe.len() + 6 - code.len()) {
            code.push(alphabet[*x as usize % alphabet.len()] as char);
        }
    }
    Ok(CodeDeDemande(code))
}

// ---------------------------------------------------------------- le formulaire declare

/// UNE QUESTION DU FORMULAIRE « Remplir ma fiche », dans l'ordre ou la page les montre. Ce qu'elle est (cle, libelle,
/// aide, forme de la reponse, reponse « aucun », condition, case de la fiche qu'elle remplit) : `declaree`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Question {
    Prenom,
    Nom,
    NomUsage,
    Pseudonyme,
    Naissance,
    LieuNaissance,
    PaysNaissance,
    Nationalite,
    Nir,
    Sexe,
    DepartementNaissance,
    Adresse,
    CodePostal,
    Commune,
    Iban,
    Bic,
    CongesSpectacles,
    Anniversaire,
    AccordDfs,
    ModeleBulletin,
}

/// LA FORME D'UNE REPONSE : ce que la page verifie pendant la saisie (une politesse) et ce que la releve lit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormeDeReponse {
    /// Un texte libre.
    Texte,
    /// Une date, `JJ/MM/AAAA`.
    Date,
    /// Le numero de securite sociale : 15 chiffres, la cle (97 moins le reste du NIR par 97) comprise.
    Nir,
    /// Cinq chiffres.
    CodePostal,
    /// Deux caracteres : `01` a `98`, `2A`, `2B` (S21.G00.30.014).
    Departement,
    /// Un IBAN, sa cle MOD 97-10 juste.
    Iban,
    /// Un BIC, 8 ou 11 caracteres.
    Bic,
    /// Un choix parmi (valeur ecrite dans la reponse, ce que la page en dit).
    Choix(Vec<(&'static str, &'static str)>),
}

impl FormeDeReponse {
    /// Son nom dans le formulaire publie, que la page lit pour savoir quoi verifier.
    fn nom(&self) -> &'static str {
        match self {
            FormeDeReponse::Texte => "texte",
            FormeDeReponse::Date => "date",
            FormeDeReponse::Nir => "nir",
            FormeDeReponse::CodePostal => "code_postal",
            FormeDeReponse::Departement => "departement",
            FormeDeReponse::Iban => "iban",
            FormeDeReponse::Bic => "bic",
            FormeDeReponse::Choix(_) => "choix",
        }
    }
}

/// CE QU'UNE QUESTION EST : tout ce que la page dessine et que la releve lit.
#[derive(Debug, Clone)]
pub struct QuestionDeclaree {
    /// La cle de sa ligne dans la reponse (`cle : valeur`) et dans le lien.
    pub cle: &'static str,
    pub libelle: &'static str,
    /// L'aide courte, sous la case (`""` : aucune).
    pub aide: &'static str,
    pub forme: FormeDeReponse,
    /// La reponse « je n'en ai pas » et ce que la page en dit : elle s'ecrit `aucun`. `None` : la question n'en a pas.
    pub aucun: Option<&'static str>,
    /// Elle ne s'ouvre que si cette autre question a pour reponse « aucun » (le sexe sans numero de securite sociale).
    pub si_aucun: Option<Question>,
    /// Elle peut rester sans reponse, et la page le dit.
    pub facultative: bool,
    /// L'EXPLICATION LONGUE (`""` : aucune) : la page la montre derriere « En savoir plus », le courriel la porte entiere
    /// sous la ligne de la question.
    pub explication: &'static str,
    /// La case de la fiche du salarie qu'elle remplit : elle ne se pose que si cette case manque a sa fiche. C'est le
    /// seul endroit qui dit qu'une case se demande au salarie.
    pub case: CaseSalarie,
    /// Elle ne se pose qu'a un salarie dont la fiche dit deja cette case (l'accord a la deduction forfaitaire, a qui
    /// exerce une profession qui y ouvre droit).
    pourvu: Option<CaseSalarie>,
}

/// La cle de la ligne du code, la derniere de la reponse.
pub const CLE_DU_CODE: &str = "code";

/// Ce qui s'ecrit pour « je n'en ai pas ».
pub const AUCUN: &str = "aucun";

/// Ce qui separe la cle de la valeur dans une ligne de la reponse.
pub const SEPARATEUR: &str = " : ";

/// La page de puppetpaie.fr ou le salarie remplit sa fiche (le site l'heberge, `atelier/site/fiche/`).
pub const PAGE: &str = "https://puppetpaie.fr/fiche/";

/// Le nom de la fonction de la page qu'appelle le formulaire publie quand il se charge (`atelier/site/fiche/fiche.js`).
pub const RAPPEL_DE_LA_PAGE: &str = "ficheFormulaire";

impl Question {
    /// Toutes, dans l'ordre de la page.
    pub const TOUTES: [Question; 20] = [
        Question::Prenom,
        Question::Nom,
        Question::NomUsage,
        Question::Pseudonyme,
        Question::Naissance,
        Question::LieuNaissance,
        Question::PaysNaissance,
        Question::Nationalite,
        Question::Nir,
        Question::Sexe,
        Question::DepartementNaissance,
        Question::Adresse,
        Question::CodePostal,
        Question::Commune,
        Question::Iban,
        Question::Bic,
        Question::CongesSpectacles,
        Question::Anniversaire,
        Question::AccordDfs,
        Question::ModeleBulletin,
    ];

    /// CE QU'ELLE EST : la declaration, la seule.
    pub fn declaree(self) -> QuestionDeclaree {
        use FormeDeReponse as F;
        let q = |cle, libelle, aide, forme, case| QuestionDeclaree { cle, libelle, aide, forme, aucun: None, si_aucun: None, facultative: false, explication: "", case, pourvu: None };
        match self {
            Question::Prenom => q("prenom", "Prénom", "Le premier prénom de ton état civil, avec ses accents.", F::Texte, CaseSalarie::PrenomAffichage),
            Question::Nom => q("nom", "Nom de famille", "Ton nom de naissance, comme sur ta pièce d'identité.", F::Texte, CaseSalarie::NomAffichage),
            Question::NomUsage => QuestionDeclaree {
                aucun: Some("Je n'en ai pas"),
                ..q("nom_usage", "Nom d'usage", "Seulement si tu portes un autre nom que ton nom de naissance (celui d'un conjoint, par exemple).", F::Texte, CaseSalarie::NomUsage)
            },
            Question::Pseudonyme => QuestionDeclaree {
                aucun: Some("Je n'en ai pas"),
                ..q("pseudonyme", "Nom de scène", "Le nom sous lequel tu joues, s'il n'est pas le tien.", F::Texte, CaseSalarie::Pseudonyme)
            },
            Question::Naissance => q("naissance", "Date de naissance", "", F::Date, CaseSalarie::DateNaissance),
            Question::LieuNaissance => {
                q("lieu_naissance", "Commune de naissance", "Ou la ville, si tu es né ou née hors de France, comme sur ta pièce d'identité.", F::Texte, CaseSalarie::LieuNaissanceAffichage)
            }
            Question::PaysNaissance => q("pays_naissance", "Pays de naissance", "", F::Texte, CaseSalarie::PaysNaissance),
            Question::Nationalite => q("nationalite", "Nationalité", "Française, italienne, marocaine…", F::Texte, CaseSalarie::CodeNationaliteDsn),
            Question::Nir => QuestionDeclaree {
                aucun: Some("Je n'ai pas de numéro de sécurité sociale"),
                ..q("nir", "Numéro de sécurité sociale", "Les 15 chiffres de ta carte Vitale ou de ton attestation de droits.", F::Nir, CaseSalarie::Nir)
            },
            // Sans NIR, la DPAE porte le sexe (C_SEXE_SAL, obligatoire : `atelier/reference/norme-dpae/v5.3_FormatDPAE_EMAIL.txt`,
            // l. 415-421) et la DSN aussi, « si le NIR est absent en cas d'identification par un NTT » (S21.G00.30.005 :
            // `atelier/reference/norme-dsn/texte/bloc-S21.G00.30.txt`, l. 154-158) ; avec un NIR, son premier chiffre le dit.
            Question::Sexe => QuestionDeclaree { si_aucun: Some(Question::Nir), ..q("sexe", "Femme ou homme", "", F::Choix(vec![("femme", "Femme"), ("homme", "Homme")]), CaseSalarie::Sexe) },
            // Sans NIR, rien ne le lit : la DPAE l'exige (C_DEPT_NAISS_SAL, obligatoire, l. 427-433 du meme format) et la
            // DSN aussi (S21.G00.30.014, « Pour les personnes nées à l'étranger, renseigner 99 », l. 279-294 du bloc 30) :
            // « aucun » (ne hors de France) s'ecrit 99.
            Question::DepartementNaissance => QuestionDeclaree {
                aucun: Some("Je suis né ou née hors de France"),
                si_aucun: Some(Question::Nir),
                ..q("departement_naissance", "Département de naissance", "Deux caractères, comme 75 ou 2A.", F::Departement, CaseSalarie::DepartementNaissance)
            },
            Question::Adresse => q("adresse", "Adresse", "Numéro et rue, comme sur une enveloppe.", F::Texte, CaseSalarie::AdresseAffichage),
            Question::CodePostal => q("code_postal", "Code postal", "", F::CodePostal, CaseSalarie::CodePostal),
            Question::Commune => q("commune", "Commune", "", F::Texte, CaseSalarie::CommuneAffichage),
            Question::Iban => q("iban", "IBAN", "Le compte où ta compagnie verse ton salaire : sur ton relevé d'identité bancaire (RIB).", F::Iban, CaseSalarie::Banque),
            Question::Bic => q("bic", "BIC", "Sur le même RIB, 8 ou 11 caractères.", F::Bic, CaseSalarie::Banque),
            Question::CongesSpectacles => QuestionDeclaree {
                aucun: Some("Je n'ai pas de numéro Congés Spectacles"),
                ..q("conges_spectacles", "Numéro Congés Spectacles", "Le numéro que la caisse des Congés Spectacles (Audiens) t'a donné.", F::Texte, CaseSalarie::NumCongesSpectacles)
            },
            Question::Anniversaire => QuestionDeclaree {
                facultative: true,
                aucun: Some("Je n'en ai pas"),
                ..q(
                    "anniversaire",
                    "Date anniversaire de tes droits France Travail",
                    "Sur ta notification France Travail. Avec elle, ta compagnie peut te montrer tes heures et tes salaires chez elle depuis cette date.",
                    F::Date,
                    CaseSalarie::AnniversaireDroits,
                )
            },
            // L'information et le consentement annuel du salarie (arrete du 4 septembre 2025, art. 9, I, al. 3 :
            // `atelier/reference/sources/dfs/arrete-2025-frais-professionnels/articles-20261110.md` ; le BOSS, frais
            // professionnels, § 2190 : l'employeur informe chaque salarie « de ce dispositif et de ses conséquences sur la
            // validation de ses droits aux assurances sociales », « par tout moyen donnant date certaine », « Une réponse
            // d'accord ou de refus doit être retourné par le salarié » : `atelier/reference/sources/boss/boss-frais-professionnels-20261013.txt`,
            // l. 2730-2737). L'aide n'en dit que cela : la deduction se fait sur l'assiette des cotisations (§ 2210, « la
            // réintégration dans l'assiette des cotisations […] du montant de la déduction », l. 2742-2744), et elle a des
            // « conséquences sur la validation de ses droits aux assurances sociales » (§ 2190). Le courriel de la
            // demande, qui porte cette aide (`a_recopier`), et celui de la reponse, dates, portent la date certaine. Ne se pose qu'a un salarie dont la
            // profession y ouvre droit, quand la compagnie l'exerce salarie par salarie (`si_entreprise` de sa ligne).
            // Trois cas : il accepte, il refuse, ou sa reponse revient sans cette ligne : « son silence vaut accord »
            // (§ 2190, l. 2737), que la releve ecrit en disant d'ou il vient. Elle n'est donc jamais a remplir de force.
            //
            // L'EXPLICATION, phrase par phrase, chacune d'une ligne du BOSS, frais professionnels
            // (`atelier/reference/sources/boss/boss-frais-professionnels-20261013.txt`) :
            // - les metiers aux frais plus eleves, dont le spectacle vivant : § 2120, l. 2619-2622 et 2628-2631 ;
            // - un pourcentage retire avant le calcul des cotisations : l. 2316 (« avant application de la déduction ») et
            //   § 2210, l. 2742-2744 (« l'assiette des cotisations ») ; 20 % ou 25 % selon le metier : § 2300, l. 2878 ;
            // - il baisse chaque annee et disparait le 1er janvier 2032 : l. 2888-2893 ;
            // - 7 600 euros par an : l. 2622-2623 ; chez chaque employeur : § 2160, l. 2694-2696 ;
            // - seulement avec de vrais frais, pas si tout est rembourse : § 2130, l. 2651-2657 ;
            // - des consequences sur les droits aux assurances sociales : § 2190, l. 2732-2733 ;
            // - une reponse d'accord ou de refus : l. 2733-2734 ; le silence vaut accord : l. 2737 ;
            // - renoncer a tout moment, pour l'annee suivante : l. 2951-2952.
            Question::AccordDfs => QuestionDeclaree {
                pourvu: Some(CaseSalarie::DeductionForfaitaire),
                facultative: true,
                explication: "La déduction forfaitaire spécifique est prévue pour les métiers qui ont des frais professionnels plus élevés que d'ordinaire, dont ceux du spectacle vivant. Elle retire de ton salaire un pourcentage avant le calcul de tes cotisations, au départ 20 % ou 25 % selon ton métier. Ce pourcentage baisse chaque année et disparaît le 1er janvier 2032. La déduction est plafonnée à 7 600 euros par an, et ce plafond peut se compter chez chaque employeur. Elle ne s'applique que si tu as vraiment des frais pour ton travail, et pas si ta compagnie te rembourse tous tes frais. Elle a des conséquences sur les droits que tes cotisations te valent aux assurances sociales. Ta compagnie doit te demander ton accord, et tu lui réponds oui ou non. Si tu ne réponds pas, ton silence vaut accord. Tu peux à tout moment demander à y renoncer, et ta décision vaut alors à partir de l'année suivante.",
                ..q(
                    "accord_dfs",
                    "Déduction forfaitaire spécifique",
                    "Elle change le calcul de tes cotisations : l'explication est juste en dessous. Sans réponse de ta part, ton silence vaut accord.",
                    F::Choix(vec![("oui", "J'accepte la déduction"), ("non", "Je refuse la déduction")]),
                    CaseSalarie::DeductionForfaitaireAccord,
                )
            },
            Question::ModeleBulletin => QuestionDeclaree {
                aucun: Some("Celui que la compagnie donne d'ordinaire"),
                ..q(
                    "modele_bulletin",
                    "Ton bulletin de paie",
                    "Simplifié : les cotisations regroupées, le modèle réglementaire. Complet : le même, suivi d'une page qui détaille chaque cotisation.",
                    F::Choix(ModeleBulletin::TOUS.iter().map(|m| (m.as_str(), m.nom())).collect()),
                    CaseSalarie::ModeleBulletin,
                )
            },
        }
    }

    /// La question dont la cle est `cle`.
    pub fn de_la_cle(cle: &str) -> Option<Question> {
        Question::TOUTES.into_iter().find(|q| q.declaree().cle == cle)
    }
}

/// LES QUESTIONS A POSER a un salarie : celles dont la case manque a sa fiche (`manque` : sa condition est remplie et
/// elle est vide, `declarations::champs::a_demander_au_salarie`), pourvu que la fiche dise ce
/// que la question suppose (`dite`) et que la question dont elle depend soit posee.
pub fn a_poser(manque: &dyn Fn(CaseSalarie) -> bool, dite: &dyn Fn(CaseSalarie) -> bool) -> Vec<Question> {
    let mut posees: Vec<Question> = Vec::new();
    for q in Question::TOUTES {
        let d = q.declaree();
        if manque(d.case) && d.pourvu.is_none_or(dite) && d.si_aucun.is_none_or(|p| posees.contains(&p)) {
            posees.push(q);
        }
    }
    posees
}

// ---------------------------------------------------------------- le formulaire publie

/// Un choix, tel que le formulaire publie le dit.
#[derive(Debug, Serialize)]
struct ChoixPublie {
    valeur: &'static str,
    libelle: &'static str,
}

/// Une question, telle que le formulaire publie la dit a la page.
#[derive(Debug, Serialize)]
struct QuestionPubliee {
    cle: &'static str,
    libelle: &'static str,
    #[serde(skip_serializing_if = "str::is_empty")]
    aide: &'static str,
    forme: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    choix: Option<Vec<ChoixPublie>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    aucun: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    si_aucun: Option<&'static str>,
    facultative: bool,
    #[serde(skip_serializing_if = "str::is_empty")]
    explication: &'static str,
}

/// Comment la reponse s'ecrit : la page la compose ainsi, la releve la lit ainsi (`lire_reponse`).
#[derive(Debug, Serialize)]
struct GrammaireDeLaReponse {
    separateur: &'static str,
    aucun: &'static str,
    code: &'static str,
}

/// LE FORMULAIRE que la page dessine : toutes les questions, et la grammaire de la reponse.
#[derive(Debug, Serialize)]
struct Formulaire {
    questions: Vec<QuestionPubliee>,
    reponse: GrammaireDeLaReponse,
}

/// LE FORMULAIRE PUBLIE : son empreinte (le nom de son fichier, `formulaires/<empreinte>.js`, et ce que le lien nomme)
/// et son contenu, un script qui le remet a la page (la politique de securite de puppetpaie.fr n'admet que les scripts
/// du site : ni JSON charge, ni requete).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormulairePublie {
    pub empreinte: EmpreinteCourte,
    pub contenu: String,
}

impl FormulairePublie {
    /// Le nom de son fichier, sous `atelier/site/fiche/formulaires/`.
    pub fn nom_du_fichier(&self) -> String {
        format!("{}.js", self.empreinte)
    }
}

/// Le formulaire de cette version du moteur, tel qu'il se publie.
pub fn formulaire_publie() -> FormulairePublie {
    let questions = Question::TOUTES
        .iter()
        .map(|q| {
            let d = q.declaree();
            QuestionPubliee {
                cle: d.cle,
                libelle: d.libelle,
                aide: d.aide,
                forme: d.forme.nom(),
                choix: match &d.forme {
                    FormeDeReponse::Choix(c) => Some(c.iter().map(|&(valeur, libelle)| ChoixPublie { valeur, libelle }).collect()),
                    _ => None,
                },
                aucun: d.aucun,
                si_aucun: d.si_aucun.map(|p| p.declaree().cle),
                facultative: d.facultative,
                explication: d.explication,
            }
        })
        .collect();
    let f = Formulaire { questions, reponse: GrammaireDeLaReponse { separateur: SEPARATEUR, aucun: AUCUN, code: CLE_DU_CODE } };
    let json = socle::json_ascii::dumps(&f, None, true, false);
    FormulairePublie { empreinte: Empreinte::des_octets(json.as_bytes()).courte(), contenu: format!("{}({});\n", RAPPEL_DE_LA_PAGE, json) }
}

// ---------------------------------------------------------------- le lien du courriel

/// LE LIEN DU COURRIEL : la page, et dans le fragment (que le navigateur n'envoie jamais au serveur) l'empreinte du
/// formulaire, les questions a poser, le code de la demande, l'adresse de la compagnie et l'objet de la reponse. Il ne
/// porte rien de ce que la compagnie sait du salarie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LienDeFiche {
    pub formulaire: EmpreinteCourte,
    pub questions: Vec<Question>,
    pub code: CodeDeDemande,
    pub adresse: droit::fiche::Courriel,
    pub objet: String,
}

impl LienDeFiche {
    /// Le lien ecrit : `https://puppetpaie.fr/fiche/#f=…&q=…&code=…&a=…&s=…`.
    pub fn ecrit(&self) -> String {
        let pc = socle::html::pourcent;
        let q: Vec<&str> = self.questions.iter().map(|q| q.declaree().cle).collect();
        format!("{}#f={}&q={}&code={}&a={}&s={}", PAGE, self.formulaire, q.join(","), self.code, pc(&self.adresse, "@"), pc(&self.objet, ""))
    }

    /// Le lien `texte`, relu (la personne fictive le lit dans le courriel de la demande) ; refus s'il n'en a pas la forme.
    pub fn lire(texte: &str) -> R<LienDeFiche> {
        let faux = |pourquoi: &str| socle::refus!("Le lien {} n'est pas celui d'une fiche à remplir : {}.", socle::fichiers::repr(texte), pourquoi);
        let fragment = texte.strip_prefix(PAGE).and_then(|r| r.strip_prefix('#')).ok_or_else(|| faux("il ne mène pas à la page"))?;
        let mut parties: std::collections::BTreeMap<&str, String> = std::collections::BTreeMap::new();
        for paire in fragment.split('&') {
            let (k, v) = paire.split_once('=').ok_or_else(|| faux("une partie n'a pas de signe égal"))?;
            let v = depourcent(v).ok_or_else(|| faux("une valeur est mal encodée"))?;
            if parties.insert(k, v).is_some() {
                return Err(faux("une partie y est deux fois"));
            }
        }
        let mut partie = |k: &str| parties.remove(k).ok_or_else(|| faux(&format!("il n'a pas de « {} »", k)));
        let formulaire = EmpreinteCourte::lire(&partie("f")?)?;
        let questions = partie("q")?.split(',').filter(|c| !c.is_empty()).map(|c| Question::de_la_cle(c).ok_or_else(|| faux(&format!("« {} » n'est pas une question", c)))).collect::<R<Vec<_>>>()?;
        let code = CodeDeDemande::lire(&partie("code")?)?;
        let adresse = droit::fiche::Courriel::lire(&partie("a")?)?;
        let objet = partie("s")?;
        if let Some(k) = parties.keys().next() {
            return Err(faux(&format!("« {} » n'en est pas une partie", k)));
        }
        Ok(LienDeFiche { formulaire, questions, code, adresse, objet })
    }
}

/// Le texte encode `%XX` (UTF-8) relu ; `None` s'il est mal encode.
fn depourcent(t: &str) -> Option<String> {
    let o = t.as_bytes();
    let mut sortie: Vec<u8> = Vec::with_capacity(o.len());
    let mut i = 0;
    while i < o.len() {
        if o[i] == b'%' {
            let h = std::str::from_utf8(o.get(i + 1..i + 3)?).ok()?;
            sortie.push(u8::from_str_radix(h, 16).ok()?);
            i += 3;
        } else {
            sortie.push(o[i]);
            i += 1;
        }
    }
    String::from_utf8(sortie).ok()
}

// ---------------------------------------------------------------- le courriel de la demande

pub const SUJET: &str = "Ta fiche pour {compagnie}";

pub const CORPS: &str = "Bonjour{virgule_prenom},\n\nPour préparer ton contrat, {compagnie} a besoin de quelques informations sur toi.\n\nLe plus simple : ouvre ce lien, remplis la page, puis appuie sur « Envoyer » : ta réponse part de ta propre messagerie. La page ne garde rien.\n{lien}\n\nSi tu ne peux pas ouvrir la page, réponds simplement à ce message en recopiant les lignes à remplir ci-dessous, chacune suivie de ta réponse : ce que chacune demande est dit juste au-dessus.\n\n{questionnaire}\n\nCes informations servent à tes contrats, à tes bulletins et aux déclarations obligatoires (Urssaf, France Travail, Audiens). Elles restent chez {compagnie}.\n\nBien à toi,\n{signataire}\n{compagnie}\n";

pub const HTML: &str = "<div style=\"font-family:Arial,Helvetica,sans-serif;font-size:15px;line-height:1.5;color:#2C3E50\">\n<p>Bonjour{virgule_prenom},</p>\n<p>Pour préparer ton contrat, {compagnie} a besoin de quelques informations sur toi.</p>\n<p>Appuie sur le bouton, remplis la page, puis appuie sur « Envoyer » : ta réponse part de ta propre messagerie. La page ne garde rien.</p>\n<p style=\"margin:26px 0\">\n<a href=\"{lien}\" style=\"display:inline-block;background:#B22222;color:#ffffff;padding:14px 26px;border-radius:6px;text-decoration:none;font-weight:bold;font-size:16px\">Remplir ma fiche</a>\n</p>\n<p style=\"font-size:13px;color:#555\">Si tu ne peux pas ouvrir la page, réponds simplement à ce message en recopiant les lignes à remplir, chacune suivie de ta réponse : ce que chacune demande est dit juste au-dessus.</p>\n<pre style=\"font-family:Arial,Helvetica,sans-serif;font-size:14px;background:#f4f4f4;padding:12px;white-space:pre-wrap\">{questionnaire}</pre>\n<p>Ces informations servent à tes contrats, à tes bulletins et aux déclarations obligatoires (Urssaf, France Travail, Audiens). Elles restent chez {compagnie}.</p>\n<p>Bien à toi,<br>{signataire}<br>{compagnie}</p>\n</div>\n";

/// CE QUI SE RECOPIE SANS LA PAGE : d'abord ce que chaque ligne demande, avec l'aide de sa question, mot pour mot celle
/// de la page (une ligne qui commence par « cle — » : rien ne s'y prend pour une reponse, `lire_reponse`), puis les
/// lignes `cle : ` a remplir, puis le code, en dernier.
///
/// Le courriel porte ainsi l'information sur la deduction forfaitaire, et c'est lui qui la prouve : le BOSS veut le
/// salarie informe « par tout moyen donnant date certaine […] de ce dispositif et de ses conséquences sur la validation
/// de ses droits aux assurances sociales » (§ 2190), l'employeur qui ne justifie pas l'avoir informe « ne peut se
/// prévaloir d'un accord de ceux-ci, même tacite » (§ 2200), et c'est a lui de le prouver (§ 2210) :
/// `atelier/reference/sources/boss/boss-frais-professionnels-20261013.txt`, l. 2730-2745. La page ne garde rien ; le
/// courriel parti, date, reste dans la boite de la compagnie.
pub fn a_recopier(code: &CodeDeDemande, questions: &[Question]) -> String {
    let mut lignes: Vec<String> = Vec::new();
    for q in questions {
        let d = q.declaree();
        let mut dit = format!("{} — {}", d.cle, d.libelle);
        if let FormeDeReponse::Choix(c) = &d.forme {
            dit.push_str(&format!(" ({})", c.iter().map(|(v, _)| *v).collect::<Vec<_>>().join(" ou ")));
        }
        dit.push('.');
        if !d.aide.is_empty() {
            dit.push_str(&format!(" {}", d.aide));
        }
        if let Some(a) = d.aucun {
            dit.push_str(&format!(" « {} » : {}.", AUCUN, a.to_lowercase()));
        }
        if let Some(p) = d.si_aucun {
            dit.push_str(&format!(" Seulement si {} est « {} ».", p.declaree().cle, AUCUN));
        }
        if d.facultative {
            dit.push_str(" Facultatif.");
        }
        lignes.push(dit);
        // L'explication entiere, sous sa ligne : le courriel date prouve l'information (§ 2190 a 2210).
        if !d.explication.is_empty() {
            lignes.push(format!("   En savoir plus — {}", d.explication));
        }
    }
    lignes.push(String::new());
    lignes.extend(questions.iter().map(|q| format!("{}{}", q.declaree().cle, SEPARATEUR)));
    lignes.push(String::new());
    lignes.push(format!("{}{}{}", CLE_DU_CODE, SEPARATEUR, code));
    lignes.join("\n")
}

// ---------------------------------------------------------------- la reponse

/// UNE REPONSE A UNE QUESTION : « je n'en ai pas » (`aucun`), ou ce que la personne a ecrit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reponse {
    Aucun,
    Dite(String),
}

/// La reponse composee : une ligne par question repondue, `cle : valeur` ou `cle : aucun`, puis le code. La page la
/// compose de meme, depuis le formulaire publie ; la personne fictive, par cette fonction.
pub fn reponse(code: &CodeDeDemande, dites: &[(Question, Reponse)]) -> String {
    let mut lignes: Vec<String> = dites
        .iter()
        .map(|(q, r)| {
            let v = match r {
                Reponse::Aucun => AUCUN,
                Reponse::Dite(t) => t.as_str(),
            };
            format!("{}{}{}", q.declaree().cle, SEPARATEUR, v)
        })
        .collect();
    lignes.push(String::new());
    lignes.push(format!("{}{}{}", CLE_DU_CODE, SEPARATEUR, code));
    lignes.join("\n")
}

/// CE QUE LA REPONSE DIT : chaque question repondue, une fois, et ce qui ne s'y lit pas.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReponseLue {
    pub dites: Vec<(Question, Reponse)>,
    pub illisibles: Vec<String>,
    /// Les questions dont la reponse porte une ligne, lisible ou non.
    vues: Vec<Question>,
}

impl ReponseLue {
    /// La reponse ne porte aucune ligne pour `q` : la personne n'y a rien repondu (ni « aucun », ni une valeur, meme
    /// illisible).
    pub fn sans_ligne(&self, q: Question) -> bool {
        !self.vues.contains(&q)
    }

    /// NE GARDE QUE LES REPONSES AUX QUESTIONS `posees` (celles qu'on pose a cette personne) ; rend les autres questions
    /// repondues, qui ne s'ecrivent pas.
    pub fn ne_garder_que(&mut self, posees: &[Question]) -> Vec<Question> {
        let (gardees, autres): (Vec<_>, Vec<_>) = std::mem::take(&mut self.dites).into_iter().partition(|(q, _)| posees.contains(q));
        self.dites = gardees;
        autres.into_iter().map(|(q, _)| q).collect()
    }

    /// La reponse a `q`, si elle est dite.
    fn de(&self, q: Question) -> Option<&Reponse> {
        self.dites.iter().find(|(x, _)| *x == q).map(|(_, r)| r)
    }

    /// Ce que la personne a ecrit pour `q`, si elle a ecrit autre chose que « aucun ».
    fn ecrite(&self, q: Question) -> Option<&str> {
        match self.de(q) {
            Some(Reponse::Dite(t)) => Some(t.as_str()),
            Some(Reponse::Aucun) | None => None,
        }
    }
}

/// LA LECTURE DE LA REPONSE, le texte que la personne a ecrit (la citation deja otee, `envois::courriel_lecture`).
///
/// UNE LIGNE EST UNE REPONSE quand ce qui precede son premier deux-points est un seul mot (lettres, chiffres, `_`) :
/// `prenom : Marie`. Toute autre ligne n'est visiblement pas une reponse et ne dit rien : un bonjour, une signature,
/// « Le 22 nov. à 10:30, … a écrit : », les lignes « cle — … » du courriel. Le mot se lit sans tenir compte des
/// capitales (`Prenom`, `NIR`) ; la ligne du code n'est pas une question ; un autre mot qui n'est la cle d'aucune
/// question est illisible, et se redemande. Une valeur vide n'est pas une reponse ; `aucun` (ou `aucune`) en est une,
/// pour une question qui l'admet, et illisible pour une autre ; une question repondue deux fois de deux facons est
/// illisible.
pub fn lire_reponse(texte: &str) -> ReponseLue {
    let mut lue = ReponseLue::default();
    let mut en_double: Vec<Question> = Vec::new();
    for ligne in socle::texte::lignes_du_texte(texte) {
        let Some((tete, valeur)) = ligne.split_once(':') else { continue };
        let mot = strip(tete).to_lowercase();
        if mot.is_empty() || !mot.chars().all(|c| c.is_alphanumeric() || c == '_') || mot == CLE_DU_CODE {
            continue;
        }
        let valeur = strip(valeur);
        if valeur.is_empty() {
            continue;
        }
        let Some(q) = Question::de_la_cle(&mot) else {
            lue.illisibles.push(format!("« {} » : « {} » (ce n'est pas une question de la fiche)", strip(tete), valeur));
            continue;
        };
        let d = q.declaree();
        if !lue.vues.contains(&q) {
            lue.vues.push(q);
        }
        let r = if [AUCUN, "aucune"].contains(&valeur.to_lowercase().as_str()) {
            if d.aucun.is_none() {
                lue.illisibles.push(format!("{} : « {} » (cette question n'admet pas « {} »)", en_minuscule(d.libelle), valeur, AUCUN));
                continue;
            }
            Reponse::Aucun
        } else {
            Reponse::Dite(valeur.to_string())
        };
        match lue.de(q) {
            None => lue.dites.push((q, r)),
            Some(deja) if *deja == r => {}
            Some(_) => {
                if !en_double.contains(&q) {
                    en_double.push(q);
                    lue.illisibles.push(format!("{} : deux réponses différentes", en_minuscule(d.libelle)));
                }
            }
        }
    }
    lue.dites.retain(|(q, _)| !en_double.contains(q));
    lue
}

/// Le libelle d'une question, sa premiere lettre en minuscule, pour la note de la fiche.
pub fn en_minuscule(t: &str) -> String {
    let mut c = t.chars();
    match c.next() {
        Some(p) => p.to_lowercase().chain(c).collect(),
        None => String::new(),
    }
}

// ---------------------------------------------------------------- la lecture des valeurs

/// Sans accents, en minuscules, sans blancs aux bords.
fn sans_accents(t: &str) -> String {
    strip(&socle::texte::sans_accent(t).to_lowercase()).to_string()
}

/// Les nationalites (adjectifs) : (mot sans accent, code ISO du pays). La fiche garde le pays (`pays_nationalite`) ;
/// la codification UE s'en deduit (`droit::nationalite::code_du_pays`).
const NATIONALITES: [(&str, &str); 60] = [
    ("francaise", "FR"),
    ("francais", "FR"),
    ("italienne", "IT"),
    ("italien", "IT"),
    ("espagnole", "ES"),
    ("espagnol", "ES"),
    ("belge", "BE"),
    ("allemande", "DE"),
    ("allemand", "DE"),
    ("portugaise", "PT"),
    ("portugais", "PT"),
    ("polonaise", "PL"),
    ("polonais", "PL"),
    ("roumaine", "RO"),
    ("roumain", "RO"),
    ("grecque", "GR"),
    ("grec", "GR"),
    ("neerlandaise", "NL"),
    ("neerlandais", "NL"),
    ("irlandaise", "IE"),
    ("irlandais", "IE"),
    ("luxembourgeoise", "LU"),
    ("luxembourgeois", "LU"),
    ("autrichienne", "AT"),
    ("autrichien", "AT"),
    ("suedoise", "SE"),
    ("suedois", "SE"),
    ("danoise", "DK"),
    ("danois", "DK"),
    ("finlandaise", "FI"),
    ("finlandais", "FI"),
    ("norvegienne", "NO"),
    ("norvegien", "NO"),
    ("islandaise", "IS"),
    ("islandais", "IS"),
    ("britannique", "GB"),
    ("anglaise", "GB"),
    ("anglais", "GB"),
    ("suisse", "CH"),
    ("americaine", "US"),
    ("americain", "US"),
    ("canadienne", "CA"),
    ("canadien", "CA"),
    ("algerienne", "DZ"),
    ("algerien", "DZ"),
    ("marocaine", "MA"),
    ("marocain", "MA"),
    ("tunisienne", "TN"),
    ("tunisien", "TN"),
    ("bresilienne", "BR"),
    ("bresilien", "BR"),
    ("argentine", "AR"),
    ("argentin", "AR"),
    ("chilienne", "CL"),
    ("chilien", "CL"),
    ("ukrainienne", "UA"),
    ("ukrainien", "UA"),
    ("russe", "RU"),
    ("liechtensteinoise", "LI"),
    ("liechtensteinois", "LI"),
];

/// Les nationalites d'un pays que la liste livree n'a pas (`pays-base.json`, COG de l'INSEE : le Kosovo n'y a pas de
/// code ISO) : (mot sans accent, nom du pays). Le nom va dans `pays_nationalite_hors_liste` ; la zone
/// (`code_nationalite_dsn`) ne s'en deduit pas, l'administrateur la choisit.
const NATIONALITES_HORS_LISTE: [(&str, &str); 3] = [("kosovare", "Kosovo"), ("kosovar", "Kosovo"), ("kosovo", "Kosovo")];

/// Le premier chiffre (le zero) de chaque ecriture decimale d'Unicode (categorie `Nd`). Chaque ecriture compte dix
/// chiffres consecutifs.
const ZEROS: [u32; 68] = [
    0x30, 0x660, 0x6F0, 0x7C0, 0x966, 0x9E6, 0xA66, 0xAE6, 0xB66, 0xBE6, 0xC66, 0xCE6, 0xD66, 0xDE6, 0xE50, 0xED0, 0xF20, 0x1040, 0x1090, 0x17E0,
    0x1810, 0x1946, 0x19D0, 0x1A80, 0x1A90, 0x1B50, 0x1BB0, 0x1C40, 0x1C50, 0xA620, 0xA8D0, 0xA900, 0xA9D0, 0xA9F0, 0xAA50, 0xABF0, 0xFF10,
    0x104A0, 0x10D30, 0x11066, 0x110F0, 0x11136, 0x111D0, 0x112F0, 0x11450, 0x114D0, 0x11650, 0x116C0, 0x11730, 0x118E0, 0x11950, 0x11C50,
    0x11D50, 0x11DA0, 0x11F50, 0x16A60, 0x16AC0, 0x16B50, 0x1D7CE, 0x1D7D8, 0x1D7E2, 0x1D7EC, 0x1D7F6, 0x1E140, 0x1E2F0, 0x1E4F0, 0x1E950,
    0x1FBF0,
];

/// La valeur d'un chiffre decimal d'Unicode (`unicodedata.decimal`), ou `None`.
fn valeur_chiffre(c: char) -> Option<u32> {
    let n = c as u32;
    ZEROS.iter().find(|z| (**z..**z + 10).contains(&n)).map(|z| n - z)
}

/// Un chiffre pour `\d` : la categorie `Nd` d'Unicode.
fn chiffre(c: char) -> bool {
    valeur_chiffre(c).is_some()
}

/// Une date lue `AAAA-M-J` ou `J/M/AAAA` (separateurs `/`, `.`, `-`, espace), en ISO ; `None` si elle ne se
/// lit pas ou n'existe pas.
pub fn date_lue(texte: &str) -> Option<Jour> {
    let t = strip(texte);
    let c: Vec<char> = t.chars().collect();
    // Les trois groupes de chiffres, separes par `sep`, de largeurs permises.
    let groupes = |sep: &dyn Fn(char) -> bool, largeurs: [(usize, usize); 3]| -> Option<[String; 3]> {
        let mut morceaux: Vec<String> = vec![String::new()];
        for x in &c {
            if sep(*x) {
                morceaux.push(String::new());
            } else if chiffre(*x) {
                morceaux.last_mut()?.push(*x);
            } else {
                return None;
            }
        }
        if morceaux.len() != 3 || morceaux.iter().zip(largeurs).any(|(m, (a, b))| !(a..=b).contains(&m.chars().count())) {
            return None;
        }
        let mut it = morceaux.into_iter();
        Some([it.next()?, it.next()?, it.next()?])
    };
    let (a, mo, j) = if let Some([a, mo, j]) = groupes(&|x| x == '-', [(4, 4), (1, 2), (1, 2)]) {
        (a, mo, j)
    } else {
        let [j, mo, a] = groupes(&|x| matches!(x, '/' | '.' | '-' | ' '), [(1, 2), (1, 2), (4, 4)])?;
        (a, mo, j)
    };
    // Chaque groupe, ses chiffres ramenes a l'ASCII, lu par sa zone (`socle::piece`).
    use socle::piece::{format, Zone};
    let ascii = |t: &str| -> Option<String> { t.chars().map(|c| char::from_digit(valeur_chiffre(c)?, 10)).collect() };
    let annee = Zone::lue(&ascii(&a)?, format::AnneeEnChiffres).valeur()?.ok()?;
    let mois = Zone::lue(&ascii(&mo)?, format::MoisEnChiffres).valeur()?;
    Zone::lue(&ascii(&j)?, format::JourDuMois(annee.mois(mois))).valeur()
}

// LES PAYS QUE LA RELEVE SAIT NOMMER : ceux de la liste livree (`pays.rs`, Code officiel geographique de l'INSEE),
// par leur nom (celui du COG ou le nom officiel, sans accents) ou leur code ISO a deux lettres. Un nom ou un
// code que la liste ne connait pas laisse la case vide et la note le dit.

/// (code ISO, codification UE) d'un pays de la liste nomme en francais, ou de son code a deux lettres ;
/// `None` pour ce que la liste ne connait pas.
pub fn pays_lu(texte: &str) -> R<Option<(socle::pays::CodePays, droit::fiche::CodeNationalite)>> {
    let t = sans_accents(texte).replace('\u{2019}', "'");
    let lu = |p: socle::pays::Pays| (p.code.clone(), droit::nationalite::codification(&p));
    if t.chars().count() == 2 && t.chars().all(|c| c.is_ascii_lowercase()) {
        return Ok(socle::pays::du_code(&t)?.map(lu));
    }
    match socle::pays::du_nom(&t)? {
        Some(p) => Ok(Some(lu(p))),
        None => Ok(socle::pays::du_nom(&t.replace(' ', "-"))?.map(lu)),
    }
}

/// CE QUE LES REPONSES DISENT DE LA FICHE DU SALARIE : chaque case lue ; une case que les reponses ne disent pas, ou
/// pas lisiblement, ne s'ecrit pas. Ses champs portent les noms des cases de la fiche (`droit::fiche::ChampsSalarie`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FicheLue {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prenom_affichage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nom_affichage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nom_usage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pseudonyme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lieu_naissance_affichage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adresse_affichage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commune_affichage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_conges_spectacles: Option<NumeroCongesSpectacles>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_naissance: Option<Jour>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anniversaire_droits: Option<Jour>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_postal: Option<droit::fiche::CodePostal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nir: Option<droit::fiche::Nir>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nir_cle: Option<droit::fiche::CleNir>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sexe: Option<droit::fiche::Sexe>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pays_naissance: Option<socle::pays::CodePays>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_nationalite_dsn: Option<droit::fiche::CodeNationalite>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pays_nationalite: Option<socle::pays::CodePays>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pays_nationalite_hors_liste: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub banque: Option<BanqueLue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub departement_naissance: Option<droit::fiche::DepartementNaissance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deduction_forfaitaire_accord: Option<AccordDfs>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modele_bulletin: Option<ModeleBulletin>,
}

/// Le compte bancaire que les reponses donnent : l'IBAN et le BIC, chacun s'il est lisible.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BanqueLue {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iban: Option<droit::fiche::Iban>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bic: Option<droit::fiche::Bic>,
}

/// CE QUE LA RELEVE TIRE D'UNE REPONSE : les cases lues, et ce qui ne s'est pas lu (et se redemande).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FicheDesReponses {
    pub fiche: FicheLue,
    pub illisible: Vec<String>,
}

/// `fiche_depuis_reponses` : les cases que la reponse remplit. Rien n'est suppose. Une liste livree (les pays) qui ne
/// se lit pas refuse.
pub fn fiche_depuis_reponses(rep: &ReponseLue) -> R<FicheDesReponses> {
    let mut d = FicheLue::default();
    let mut illisible: Vec<String> = rep.illisibles.clone();
    let faute = |q: Question, r: &str| format!("{} : « {} »", en_minuscule(q.declaree().libelle), r);
    let texte = |q: Question| rep.ecrite(q).map(|r| strip(r).to_string()).filter(|t| !t.is_empty());
    d.prenom_affichage = texte(Question::Prenom);
    d.nom_affichage = texte(Question::Nom);
    d.nom_usage = texte(Question::NomUsage);
    d.pseudonyme = texte(Question::Pseudonyme);
    d.lieu_naissance_affichage = texte(Question::LieuNaissance);
    d.adresse_affichage = texte(Question::Adresse);
    d.commune_affichage = texte(Question::Commune);
    // LE NUMERO CONGES SPECTACLES, trois cas : un numero ; « aucun », la fiche retient qu'il n'en a pas (et les rappels
    // disent la demande a faire) ; rien, rien ne s'ecrit.
    d.num_conges_spectacles = match rep.de(Question::CongesSpectacles) {
        Some(Reponse::Dite(r)) => Some(NumeroCongesSpectacles::Numero(r.replace(' ', ""))).filter(|n| n.numero().is_some()),
        Some(Reponse::Aucun) => Some(NumeroCongesSpectacles::SansNumero),
        None => None,
    };
    for q in [Question::Naissance, Question::Anniversaire] {
        if let Some(r) = rep.ecrite(q) {
            let v = date_lue(r);
            if v.is_none() {
                illisible.push(faute(q, r));
            }
            match q {
                Question::Naissance => d.date_naissance = v,
                _ => d.anniversaire_droits = v,
            }
        }
    }
    if let Some(r) = rep.ecrite(Question::CodePostal) {
        let cp: String = r.chars().filter(|c| !espace(*c)).collect();
        if cp.chars().count() == 5 && cp.chars().all(chiffre) {
            match droit::fiche::CodePostal::lire(&cp) {
                Ok(cp) => d.code_postal = Some(cp),
                Err(e) => illisible.push(format!("{} ({})", faute(Question::CodePostal, r), e.phrase)),
            }
        } else {
            illisible.push(faute(Question::CodePostal, r));
        }
    }
    // LE NIR, revérifié comme la page le vérifie : treize caracteres d'un NIR (`forme_d_un_nir`, la Corse comprise), et
    // sa cle, si elle est dite, celle de ses treize caracteres (`droit::fiche::CleNir::du_nir`). Une cle fausse rend la
    // reponse entiere illisible : rien ne s'ecrit, elle se redemande.
    if let Some(r) = rep.ecrite(Question::Nir) {
        let n: String = Nir::rangee(r).chars().filter(|c| !matches!(c, '.' | '-')).collect();
        match Nir::et_sa_cle(&n).filter(|(treize, _)| forme_d_un_nir(treize)) {
            Some((treize, cle)) => {
                let nir = Nir::lire(treize)?;
                match cle.map(|c| (droit::fiche::CleNir::lire(c), droit::fiche::CleNir::du_nir(&nir))) {
                    None => d.nir = Some(nir),
                    Some((Ok(dite), Some(juste))) if dite == juste => {
                        d.nir = Some(nir);
                        d.nir_cle = Some(dite);
                    }
                    Some((Ok(_), _)) => illisible.push(format!("{} (sa clé n'est pas celle de ses 13 premiers chiffres)", faute(Question::Nir, r))),
                    Some((Err(e), _)) => illisible.push(format!("{} ({})", faute(Question::Nir, r), e.phrase)),
                }
            }
            None => illisible.push(faute(Question::Nir, r)),
        }
    }
    // Le sexe ne se lit que sans NIR : avec un NIR, son premier chiffre le dit.
    if let Some(r) = rep.ecrite(Question::Sexe).filter(|_| d.nir.is_none()) {
        match sans_accents(r).as_str() {
            "femme" => d.sexe = Some(droit::fiche::Sexe::Feminin),
            "homme" => d.sexe = Some(droit::fiche::Sexe::Masculin),
            _ => illisible.push(faute(Question::Sexe, r)),
        }
    }
    match rep.de(Question::DepartementNaissance) {
        Some(Reponse::Dite(r)) => {
            let t = strip(r).to_uppercase();
            if departement_de_la_dsn(&t) {
                d.departement_naissance = Some(droit::fiche::DepartementNaissance::lire(&t)?);
            } else {
                illisible.push(faute(Question::DepartementNaissance, r));
            }
        }
        // Ne hors de France : 99 (S21.G00.30.014, « Pour les personnes nées à l'étranger, renseigner 99 »).
        Some(Reponse::Aucun) => d.departement_naissance = Some(droit::fiche::DepartementNaissance::lire("99")?),
        None => {}
    }
    if let Some(r) = rep.ecrite(Question::PaysNaissance) {
        // Le pays de naissance se lit aussi dans la table « Pays » de la DSN (`pays::naissance_du_nom`) : le
        // Kosovo, l'URSS, que le COG n'a pas.
        let lu = match pays_lu(r)? {
            Some((code, _)) => Some(code),
            None => {
                let t = sans_accents(r).replace('\u{2019}', "'");
                if t.chars().count() == 2 && t.chars().all(|c| c.is_ascii_lowercase()) {
                    socle::pays::de_la_table_dsn(&t)?.map(|p| p.code)
                } else {
                    socle::pays::naissance_du_nom(&t)?
                }
            }
        };
        match lu {
            Some(code) => d.pays_naissance = Some(code),
            None => illisible.push(faute(Question::PaysNaissance, r)),
        }
    }
    // LA NATIONALITE : le pays se garde (`pays_nationalite`), la codification UE s'en deduit.
    if let Some(r) = rep.ecrite(Question::Nationalite) {
        let t = sans_accents(r);
        let pays = match NATIONALITES.iter().find(|(n, _)| *n == t) {
            Some((_, c)) => Some(c.to_string()),
            None => pays_lu(&t)?.map(|(c, _)| c.to_string()),
        };
        match pays {
            Some(pays) => {
                // La codification se lit dans la liste (un territoire suit son pays de rattachement), `pays.rs`.
                match socle::pays::du_code(&pays)? {
                    Some(p) => {
                        d.code_nationalite_dsn = Some(droit::nationalite::codification(&p));
                        d.pays_nationalite = Some(p.code);
                    }
                    None => illisible.push(faute(Question::Nationalite, r)),
                }
            }
            None => match NATIONALITES_HORS_LISTE.iter().find(|(n, _)| *n == t) {
                // Un pays hors de la liste se garde dans sa case ; la zone reste a choisir (la fiche le dit).
                Some((_, nom)) => d.pays_nationalite_hors_liste = Some(nom.to_string()),
                None => illisible.push(faute(Question::Nationalite, r)),
            },
        }
    }
    // LE COMPTE BANCAIRE : l'IBAN compact (sans blancs, en majuscules), lu par son type (`droit::fiche::Iban`) ; un IBAN
    // dont la forme ou la cle est fausse ne s'ecrit pas, il se dit a completer avec sa raison ; le BIC de meme (`Bic`).
    let mut banque = BanqueLue::default();
    if let Some(brut) = rep.ecrite(Question::Iban).map(strip).filter(|b| !b.is_empty()) {
        match droit::fiche::Iban::lire(&droit::fiche::Iban::rangee(brut)) {
            Ok(iban) => banque.iban = Some(iban),
            Err(e) => illisible.push(format!("{} ({})", faute(Question::Iban, brut), e.phrase)),
        }
    }
    if let Some(brut) = rep.ecrite(Question::Bic).map(strip).filter(|b| !b.is_empty()) {
        match droit::fiche::Bic::lire(&droit::fiche::Bic::rangee(brut)) {
            Ok(bic) => banque.bic = Some(bic),
            Err(e) => illisible.push(format!("{} ({})", faute(Question::Bic, brut), e.phrase)),
        }
    }
    if banque != BanqueLue::default() {
        d.banque = Some(banque);
    }
    // L'ACCORD A LA DEDUCTION FORFAITAIRE : l'accord dit par le courriel, date, est le consentement de l'annee ; le
    // refus, un refus (les choix de la case, `droit::fiche::AccordDfs`).
    if let Some(r) = rep.ecrite(Question::AccordDfs) {
        match sans_accents(r).as_str() {
            "oui" => d.deduction_forfaitaire_accord = Some(AccordDfs::ConsentementAnnuel),
            "non" => d.deduction_forfaitaire_accord = Some(AccordDfs::Refus),
            _ => illisible.push(faute(Question::AccordDfs, r)),
        }
    }
    // LE MODELE DE BULLETIN : un des choix de la case ; « aucun », celui de la compagnie, ne s'ecrit pas.
    if let Some(r) = rep.ecrite(Question::ModeleBulletin) {
        match ModeleBulletin::TOUS.into_iter().find(|m| sans_accents(m.as_str()) == sans_accents(r)) {
            Some(m) => d.modele_bulletin = Some(m),
            None => illisible.push(faute(Question::ModeleBulletin, r)),
        }
    }
    // Ce que le NIR ecrit de la naissance complete ce que les reponses ne disent pas.
    if let Some((departement, pays)) = d.nir.as_ref().map(naissance_depuis_nir).transpose()?.flatten() {
        d.departement_naissance = Some(departement);
        if d.pays_naissance.is_none() {
            d.pays_naissance = pays;
        }
    }
    Ok(FicheDesReponses { fiche: d, illisible })
}

/// TREIZE CARACTERES QUI ONT LA FORME D'UN NIR, deja ranges (`Nir::rangee`) : des chiffres, et aux 6e et 7e rangs deux
/// chiffres ou le departement corse, 2A ou 2B (INSEE, schema NIR : `atelier/reference/sources/insee-nir-siret/`).
fn forme_d_un_nir(t: &str) -> bool {
    let c: Vec<char> = t.chars().collect();
    let rang: String = c.iter().skip(5).take(2).collect();
    c.len() == 13 && c.iter().enumerate().all(|(i, x)| x.is_ascii_digit() || (i == 5 || i == 6) && (rang == "2A" || rang == "2B"))
}

/// UN DEPARTEMENT DE NAISSANCE QUE LA DSN ADMET : « (0[1-9]|[1-9][0-9])|2A|2B » (S21.G00.30.014, CSL 00 :
/// `atelier/reference/norme-dsn/texte/bloc-S21.G00.30.txt`, l. 295) ; 99, ne a l'etranger, s'ecrit par « aucun ».
fn departement_de_la_dsn(t: &str) -> bool {
    let c: Vec<char> = t.chars().collect();
    match c.as_slice() {
        ['0', b] => ('1'..='9').contains(b),
        [a, b] => ('1'..='9').contains(a) && b.is_ascii_digit() || t == "2A" || t == "2B",
        _ => false,
    }
}

/// Le departement de naissance que le NIR ECRIT (les 6e et 7e signes ; 99 pour une naissance a l'etranger), et le pays
/// de naissance qu'il en deduit (la France, hors 99). Deux cas se taisent : la Corse d'avant 1976 (« 20 ») et un NIR
/// incomplet.
fn naissance_depuis_nir(nir: &droit::fiche::Nir) -> R<Option<(droit::fiche::DepartementNaissance, Option<socle::pays::CodePays>)>> {
    let c: Vec<char> = nir.chars().collect();
    if !forme_d_un_nir(nir) || !matches!(c[0], '1' | '2') {
        return Ok(None);
    }
    let dep = droit::fiche::DepartementNaissance::lire(&c[5..7].iter().collect::<String>())?;
    if dep.est_l_ancienne_corse() {
        return Ok(None);
    }
    let pays = (!dep.est_etranger()).then(socle::pays::CodePays::france);
    Ok(Some((dep, pays)))
}
