//! Ce qui est du, et quand : `echeances` est la seule fonction qui compose les echeances de la compagnie (les DPAE, les
//! obligations de `obligations.rs`, la DSN de chaque mois, les echeances annuelles de `donnees/echeancier-base.json` --
//! la cloture de l'exercice apres la DSN de decembre, l'export compta, la revue des spectacles, le taux AT/MP, les
//! contributions de branche). Chacune porte sa date et dit si elle est due maintenant (`due`). Les deux routes la lisent :
//! `echeancier` (tout, date, ce mois-ci et l'annee qui vient) et `rappels` (ce qui est du maintenant, avec son geste).
//! Le panneau et l'horloge lisent les mois dont la DSN est due (`mois_a_declarer`, `mois_a_parcourir`), qu'elle compose.
//! Aucun calcul de paie : les engagements se lisent sans chiffrer, et leur etat (`engagement::etats`), le journal par
//! `etats`.
//!
//! Chaque charge, chaque reponse, l'echeancier livre (`donnees/echeancier-base.json`) et ce que la compagnie a coche
//! (`config/echeancier.json`) sont une forme declaree ici, lue une fois (`socle::forme::lire`). Un refus porte sa `classe`.

use droit::fiche::Lue as _;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use droit::contexte::Contexte;
use socle::date::Jour;
use socle::mois::{Annee, Graphie, Mois};
use engagement;
use compagnie::etat_dsn;
use crate::catalogue::{Acte, Affiche, Champ, Forme, Fiche, Refus as RefusFiche, FORME, OPERATION};
use crate::catalogue::Route;
use crate::enveloppe::{en_json, route_inconnue, Charge, Dite, Porte, Rendu};
use crate::imprevus::{FinASignaler, ResumeDArret};
use crate::SansCharge;
use socle::ecran::Pastille;
use declarations::demarrage::{Manque, Pose};
use socle::echec::{erreur, Classe};
use crate::catalogue::{Ecran, Place};

// ------------------------------------------------------------------ les refus

use socle::echec::R;

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::Dessine(Place::Paie);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::Rappels,
        geste: "Dire ce que la compagnie a à faire ce jour : DSN, fins de contrat, DPAE…",
        charge: &[],
        exemple: r#"{}"#,
        rend: "rappels : les échéances dues maintenant (chacune : quoi, titre, question, mois ou année, état et sa pastille, sa date ; action : la route que son geste ouvre, sa charge, puis la route de l'acte, le mot du bouton)",
        refus: &[OPERATION, FORME],
        acte: Acte::Lit,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::Echeancier,
        geste: "Lire les échéances du mois et de l'année.",
        charge: &[],
        exemple: "{}",
        rend: "ce_mois et a_venir : les échéances, chacune avec sa date, son état et si un rappel la dit",
        refus: &[OPERATION, FORME],
        acte: Acte::Lit,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::RevueSpectacles,
        geste: "Archiver les spectacles qui ne continuent pas, et noter la revue faite.",
        charge: &[ANNEE, Champ { nom: "continuent", libelle: Affiche::new("Spectacles qui continuent"), forme: Forme::Liste, sens: "les spectacles qui continuent (leurs clés) ; les autres s'archivent", avance: false }],
        exemple: r#"{"annee":2026,"continuent":["le-banc-des-menteurs"]}"#,
        rend: "phrase, archives, continuent, configuration",
        refus: &[RefusFiche { classe: Classe::Refuse, corriger: "les spectacles qui continuent : des spectacles actifs, par leur clé" }, FORME],
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: Ecran::Dessine(Place::Paie),
    },
    Fiche {
        route: Route::PourDemarrer,
        geste: "Dire ce que la compagnie peut remplir d'avance pour ses premiers actes.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "titre, note, manques (chacun sa case, sa phrase, où il vit ; une ligne d'un salarié dit qui), poses (ce que le moteur a posé, en information)",
        refus: &[OPERATION, FORME],
        acte: Acte::Lit,
        phrase: 0,
        ecran: Ecran::Dessine(Place::Paie),
    },
    Fiche {
        route: Route::EcheanceFaite,
        geste: "Cocher une échéance annuelle que seule la compagnie peut dire faite.",
        charge: &[
            Champ { nom: "cle", libelle: Affiche::new("Échéance"), forme: Forme::Texte, sens: "l'échéance et son année, « quoi:AAAA » (celle que rappels ou echeancier rendent)", avance: false },
            Champ { nom: "fait", libelle: Affiche::new("Faite"), forme: Forme::OuiNon, sens: "faux : la décocher", avance: false },
        ],
        exemple: r#"{"cle":"revue-spectacles:2026"}"#,
        rend: "cle, fait",
        refus: &[RefusFiche { classe: Classe::Refuse, corriger: "une échéance qui se coche, avec son année" }, FORME],
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: Ecran::Dessine(Place::Paie),
    },
];

const ANNEE: Champ = Champ { nom: "annee", libelle: Affiche::new("Année"), forme: Forme::Nombre, sens: "l'année, AAAA", avance: false };

// ------------------------------------------------------------------ les charges

/// La charge de `revue-spectacles` : l'annee de la revue, et les spectacles qui continuent (leurs cles ; les autres
/// s'archivent). Le geste qu'offre son rappel l'ecrit aussi.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeRevue {
    annee: i32,
    continuent: Vec<droit::contexte::IdSpectacle>,
}

/// La charge de `echeance-faite` : l'echeance « quoi:AAAA », cochee sauf `fait: false`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChargeFaite {
    cle: CleDEcheance,
    fait: Option<bool>,
}

/// LA CLE D'UNE ECHEANCE : ce qui la nomme d'un rappel a l'autre, que la page renvoie pour la cocher
/// (`echeance-faite`), que le tableau de l'administrateur garde. Elle s'ecrit par son texte, se lit strictement par lui,
/// et se trie comme lui.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub(crate) enum CleDEcheance {
    /// `quoi:AAAA` : une echeance de l'annee (celles de l'echeancier livre, l'export comptable).
    DeLAnnee { quoi: CleDAnnuelle, annee: Annee },
    /// `dsn:AAAA-MM` : la DSN du mois.
    Dsn(Mois),
    /// `dpae:AAAA-MM-JJ` : les DPAE des embauches du jour.
    Dpae(Jour),
    /// `<quoi>:<salarie>:<jour>` : une obligation qui vise un salarie, nee le jour dit (la fin d'un contrat, ce qu'un
    /// arret fait naitre).
    DuSalarie { quoi: QuoiDuSalarie, salarie: droit::contexte::IdSalarie, jour: Jour },
}

impl CleDEcheance {
    /// La cle d'une echeance de l'annee `annee`.
    fn de_l_annee(quoi: &CleDAnnuelle, annee: i32) -> R<CleDEcheance> {
        Ok(CleDEcheance::DeLAnnee { quoi: quoi.clone(), annee: Annee::nouvelle(i64::from(annee))? })
    }
}

impl std::fmt::Display for CleDEcheance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CleDEcheance::DeLAnnee { quoi, annee } => write!(f, "{}:{}", quoi, annee),
            CleDEcheance::Dsn(mois) => write!(f, "{}:{}", DSN, mois),
            CleDEcheance::Dpae(jour) => write!(f, "{}:{}", DPAE, jour.iso()),
            CleDEcheance::DuSalarie { quoi, salarie, jour } => write!(f, "{}:{}:{}", quoi, salarie, jour.iso()),
        }
    }
}

impl From<CleDEcheance> for String {
    fn from(c: CleDEcheance) -> String {
        c.to_string()
    }
}

impl TryFrom<String> for CleDEcheance {
    type Error = socle::echec::Echec;
    /// `quoi` va jusqu'au premier deux-points, le jour d'une obligation apres le dernier ; le salarie est entre les deux.
    fn try_from(texte: String) -> R<CleDEcheance> {
        use serde::de::IntoDeserializer;
        let inconnue = || erreur(Classe::Refuse, format!("Echeance inconnue : {}.", socle::fichiers::repr(&texte)));
        let (quoi, reste) = texte.split_once(':').ok_or_else(inconnue)?;
        fn lu<'de, T: Deserialize<'de>>(t: &'de str) -> Option<T> {
            T::deserialize(IntoDeserializer::<serde::de::value::Error>::into_deserializer(t)).ok()
        }
        let cle = match (quoi, reste.rsplit_once(':')) {
            (DSN, None) => lu::<Mois>(reste).map(CleDEcheance::Dsn),
            (DPAE, None) => lu::<Jour>(reste).map(CleDEcheance::Dpae),
            (_, None) => match (CleDAnnuelle::lire(quoi).ok(), lu::<Annee>(reste)) {
                (Some(quoi), Some(annee)) => Some(CleDEcheance::DeLAnnee { quoi, annee }),
                _ => None,
            },
            (_, Some((salarie, jour))) => match (QuoiDuSalarie::lire(quoi), droit::contexte::IdSalarie::lire(salarie).ok(), lu::<Jour>(jour)) {
                (Some(quoi), Some(salarie), Some(jour)) => Some(CleDEcheance::DuSalarie { quoi, salarie, jour }),
                _ => None,
            },
        };
        match cle {
            Some(c) if !quoi.is_empty() => Ok(c),
            _ => Err(inconnue()),
        }
    }
}

impl PartialOrd for CleDEcheance {
    fn partial_cmp(&self, autre: &CleDEcheance) -> Option<std::cmp::Ordering> {
        Some(self.cmp(autre))
    }
}

impl Ord for CleDEcheance {
    fn cmp(&self, autre: &CleDEcheance) -> std::cmp::Ordering {
        self.to_string().cmp(&autre.to_string())
    }
}

// ------------------------------------------------------------------ les reponses

/// Ce que rend `rappels`.
#[derive(Debug, Serialize)]
pub(crate) struct Rappels {
    phrase: String,
    pub(crate) rappels: Vec<EcheanceDite>,
    /// La boite des rappels se ferme « plus tard » quand il y a quelque chose a faire.
    fermer_dit: &'static str,
    phrases: Vec<String>,
}

/// Ce que rend `echeancier`.
#[derive(Debug, Serialize)]
struct Echeancier {
    phrase: String,
    aujourdhui: Jour,
    /// Aujourd'hui en clair, « lundi 2 novembre 2026 ».
    #[serde(rename = "date")]
    date_dite: String,
    date_dit: String,
    bienvenue: &'static str,
    /// Le nom de la compagnie ; `null` : la fiche ne le dit pas encore.
    compagnie: Option<String>,
    /// La convention de la fiche de l'entreprise ; `null` : la fiche ne la dit pas.
    idcc: Option<droit::fiche::Idcc>,
    ce_mois: Vec<EcheanceDite>,
    a_venir: Vec<EcheanceDite>,
    a_faire: usize,
    phrases: Vec<String>,
}

/// Ce que rend `revue-spectacles`.
#[derive(Debug, Serialize)]
struct Revue {
    phrase: String,
    annee: i32,
    archives: Vec<droit::contexte::IdSpectacle>,
    continuent: Vec<droit::contexte::IdSpectacle>,
    /// La configuration relue (`ouverture`, route `configuration`).
    configuration: crate::ouverture::ConfigurationDite,
}

/// Ce que rend `echeance-faite`.
#[derive(Debug, Serialize)]
struct Faite {
    phrase: &'static str,
    cle: CleDEcheance,
    fait: bool,
}

/// Ce que chaque route de ce module rend porte sa phrase.
macro_rules! dites {
    ($($t:ty),*) => {$(
        impl Dite for $t {
            fn phrase(&self) -> &str {
                &self.phrase
            }
        }
    )*};
}
dites!(Rappels, Echeancier, Revue, PourDemarrerVu);

impl Dite for Faite {
    fn phrase(&self) -> &str {
        self.phrase
    }
}

/// Une route de ce module.
pub fn route(porte: &Porte, charge: &Charge) -> R<Rendu> {
    let ctx = porte.ctx;
    match porte.route {
        Route::Rappels => {
            let SansCharge {} = charge.lire()?;
            en_json(rappels(ctx)?)
        }
        Route::Echeancier => {
            let SansCharge {} = charge.lire()?;
            en_json(echeancier(ctx)?)
        }
        Route::RevueSpectacles => en_json(revue_spectacles(ctx, charge.lire()?)?),
        Route::EcheanceFaite => en_json(echeance_faite(ctx, charge.lire()?)?),
        // Ce que la compagnie peut remplir d'avance, d'un coup ; ce que le moteur a pose de lui-meme se dit en information
        // (gris) ; les acces aux organismes et le Drive se proposent (`acces_proposes`).
        Route::PourDemarrer => {
            let SansCharge {} = charge.lire()?;
            let (proposes, simules) = acces_proposes(ctx)?;
            let (simule, pastille_simulee) = match simules {
                Some((pose, pastille)) => (Some(pose), Some(pastille)),
                None => (None, None),
            };
            let d = declarations::demarrage::route(ctx, proposes, simule)?;
            // Chaque pose a sa pastille : celle de la compagnie dont rien ne part (la derniere), « posé pour vous » sinon.
            let n = d.poses.len();
            let poses = d
                .poses
                .into_iter()
                .enumerate()
                .map(|(i, pose)| {
                    let pastille = match (&pastille_simulee, i + 1 == n) {
                        (Some(p), true) => p.clone(),
                        _ => Pastille::de(socle::ecran::EtatDEcran::Information, "posé pour vous"),
                    };
                    PoseVue { pose, pastille }
                })
                .collect();
            en_json(PourDemarrerVu { phrase: d.phrase, phrases: d.phrases, titre: d.titre, note: d.note, manques: d.manques, poses })
        }
        autre => Err(route_inconnue(autre.nom())),
    }
}

/// Archive les spectacles actifs qui ne sont pas dans
/// `continuent`, note la revue de l'annee faite, rend la configuration.
fn revue_spectacles(ctx: &Contexte, charge: ChargeRevue) -> socle::echec::R<Revue> {
    let ChargeRevue { annee, continuent } = charge;
    // L'ordre ne compte pas, les doublons non plus.
    let garder: BTreeSet<droit::contexte::IdSpectacle> = continuent.into_iter().collect();
    let actifs = ctx.spectacles_actifs();
    let inconnus: Vec<&str> = garder.iter().filter(|g| !actifs.contains(g)).map(AsRef::as_ref).collect();
    if !inconnus.is_empty() {
        return Err(erreur(Classe::Refuse, format!("Spectacle actif inconnu : {}.", inconnus.join(", "))));
    }
    let archives: Vec<droit::contexte::IdSpectacle> = actifs.into_iter().filter(|c| !garder.contains(*c)).cloned().collect();
    for cle in &archives {
        crate::fiches::activer_fiche(ctx, &crate::fiches::ChargeActiver::Spectacle { cle: cle.clone(), actif: false })?;
    }
    marquer_echeance(ctx, &CleDEcheance::de_l_annee(&REVUE_SPECTACLES, annee)?, true, "interface")?;
    let continuent: Vec<droit::contexte::IdSpectacle> = garder.into_iter().collect();
    let neuf = Contexte::nouveau(&ctx.racine, None)?;
    let configuration = crate::ouverture::configuration_dite(&neuf)?;
    let phrase = format!("Spectacles qui continuent : {} ; archivés : {}.", continuent.len(), archives.len());
    Ok(Revue { phrase, annee, archives, continuent, configuration })
}

/// `echeance-faite` : coche (`fait`) ou decoche l'echeance `cle`, au nom de la fenetre (`compagnie::INTERFACE`).
fn echeance_faite(ctx: &Contexte, charge: ChargeFaite) -> socle::echec::R<Faite> {
    let ChargeFaite { cle, fait } = charge;
    let fait = fait != Some(false);
    marquer_echeance(ctx, &cle, fait, compagnie::INTERFACE)?;
    Ok(Faite { phrase: if fait { "Échéance cochée : faite." } else { "Échéance décochée." }, cle, fait })
}



// ============================================================ les echeances

/// Les mots de la DSN et de la DPAE, dans une cle d'echeance et comme genre d'une echeance.
const DSN: &str = "dsn";
const DPAE: &str = "dpae";
/// Les echeances de l'annee que le moteur nomme : trois de l'echeancier livre, et l'export comptable, une cle du
/// logiciel qui n'y est pas (le test `les_echeances_du_moteur_sont_celles_de_l_echeancier` le tient).
const CLOTURE: CleDAnnuelle = CleDAnnuelle::du_moteur("cloture");
const EXPORT_COMPTA: CleDAnnuelle = CleDAnnuelle::du_moteur("export-compta");
const REVUE_SPECTACLES: CleDAnnuelle = CleDAnnuelle::du_moteur("revue-spectacles");
const AT_MP: CleDAnnuelle = CleDAnnuelle::du_moteur("at-mp");

/// UNE ECHEANCE DE L'ANNEE : la `cle` d'une echeance d'`echeancier-base.json`, ou l'export comptable. INFERENCE (aucun
/// texte n'en fixe la forme : c'est une cle du logiciel) : jamais vide, sans deux-points (elle est le `quoi` d'une cle
/// d'echeance `quoi:AAAA`). Celles que le moteur lit sont des constantes ; celle de l'echeancier se lit. Elle s'ecrit
/// comme son texte.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub(crate) struct CleDAnnuelle(std::borrow::Cow<'static, str>);

impl CleDAnnuelle {
    /// La cle ecrite `texte` ; vide, ou avec un deux-points, elle se refuse.
    fn lire(texte: &str) -> R<CleDAnnuelle> {
        if texte.is_empty() || texte.contains(':') {
            return Err(erreur(Classe::Refuse, format!("Echeance inconnue : {}.", socle::fichiers::repr(texte))));
        }
        Ok(CleDAnnuelle(std::borrow::Cow::Owned(texte.to_string())))
    }

    /// LA CLE QUE LE MOTEUR NOMME, ecrite en Rust : une constante, dont le compilateur refuse une cle vide ou a
    /// deux-points.
    const fn du_moteur(cle: &'static str) -> CleDAnnuelle {
        assert!(!cle.is_empty(), "echeance de l'annee vide");
        let octets = cle.as_bytes();
        let mut i = 0;
        while i < octets.len() {
            assert!(octets[i] != b':', "echeance de l'annee a deux-points");
            i += 1;
        }
        CleDAnnuelle(std::borrow::Cow::Borrowed(cle))
    }
}

impl std::fmt::Display for CleDAnnuelle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for CleDAnnuelle {
    type Error = socle::echec::Echec;
    fn try_from(s: String) -> R<CleDAnnuelle> {
        CleDAnnuelle::lire(&s)
    }
}

/// LE GENRE D'UNE ECHEANCE DES RAPPELS : la DSN d'un mois, les DPAE d'un jour, une obligation due (`obligations.rs` : la
/// fin d'un contrat qui se signale, ce qu'un arret fait naitre), une echeance de l'annee. Il s'ecrit `dsn`, `dpae`,
/// `obligation`, ou la cle de l'echeance de l'annee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GenreDEcheance {
    Dsn,
    Dpae,
    Obligation,
    DeLAnnee(CleDAnnuelle),
}

impl std::fmt::Display for GenreDEcheance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GenreDEcheance::Dsn => f.write_str(DSN),
            GenreDEcheance::Dpae => f.write_str(DPAE),
            GenreDEcheance::Obligation => f.write_str("obligation"),
            GenreDEcheance::DeLAnnee(cle) => cle.fmt(f),
        }
    }
}

impl Serialize for GenreDEcheance {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}
/// Ce que nomme la cle du rappel d'une fin de contrat a signaler.
const FIN_CONTRAT: &str = "fin-contrat";

/// CE QUE VISE UNE ECHEANCE D'UN SALARIE : la fin de son contrat, a signaler (`fin-contrat`), ou une obligation qu'un
/// arret fait naitre (son nom, `CleDObligation`). C'est le `quoi` d'une cle `<quoi>:<salarie>:<jour>`, ecrit ainsi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum QuoiDuSalarie {
    FinContrat,
    Obligation(crate::obligations::CleDObligation),
}

impl QuoiDuSalarie {
    /// Ce que `texte` nomme ; `None` s'il ne nomme ni la fin d'un contrat ni une obligation.
    fn lire(texte: &str) -> Option<QuoiDuSalarie> {
        if texte == FIN_CONTRAT {
            return Some(QuoiDuSalarie::FinContrat);
        }
        crate::obligations::CleDObligation::de_son_nom(texte).map(QuoiDuSalarie::Obligation)
    }
}

impl std::fmt::Display for QuoiDuSalarie {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QuoiDuSalarie::FinContrat => f.write_str(FIN_CONTRAT),
            QuoiDuSalarie::Obligation(c) => c.fmt(f),
        }
    }
}

/// L'etat d'une echeance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EtatEcheance {
    AFaire,
    Fait,
    EnRetard,
}
use EtatEcheance::{AFaire, EnRetard, Fait};

impl EtatEcheance {
    /// L'etat que la couleur lit (`socle::ecran::couleur_de_l_etat`).
    fn a_l_ecran(self) -> socle::ecran::EtatDEcran {
        match self {
            AFaire => socle::ecran::EtatDEcran::AFaire,
            Fait => socle::ecran::EtatDEcran::Fait,
            EnRetard => socle::ecran::EtatDEcran::EnRetard,
        }
    }

    /// Ses mots, sur sa pastille.
    fn mots(self) -> &'static str {
        match self {
            EnRetard => "en retard",
            AFaire => "à faire",
            Fait => "fait",
        }
    }
}

/// Ou en est le mois dont on rappelle la DSN (`etape`) : un mois sans saisie se cree, un
/// mois saisi se produit, un mois produit se depose, une DSN refusee se corrige.
const A_SAISIR: &str = "a_saisir";
const A_PRODUIRE: &str = "a_produire";
const A_DEPOSER: &str = "a_deposer";
const A_CORRIGER: &str = "a_corriger";

/// L'echeancier va jusqu'a douze mois devant.
const MOIS_A_VENIR: i32 = 12;

/// UNE ECHEANCE est une DONNEE : quoi, quand, ou en est-on, et ce qu'il y a a faire. Elle ne fait rien : « verifier et
/// envoyer » passe par les operations existantes, avec leurs garde-fous et leur mot de passe.
pub(crate) struct Echeance {
    quoi: GenreDEcheance,
    cle: CleDEcheance,
    titre: String,
    question: String,
    detail: String,
    source: String,
    /// Le mois ou elle tombe : l'echeancier la range sous ce mois (aucun : une obligation sans date butoir).
    tombe: Option<Mois>,
    /// Son dernier jour, quand elle en a un.
    date: Option<Jour>,
    /// Quand, en clair (« vendredi 15/01/2027 à midi », « après la DSN de décembre… »).
    quand_dit: String,
    etat: EtatEcheance,
    /// DUE MAINTENANT : son moment est venu et elle n'est pas faite. Les rappels sont exactement celles-ci.
    due: bool,
    /// Ou en est la chose : le mois d'une DSN (`a_saisir`, `a_produire`, `a_deposer`, `a_corriger`), l'etat d'une obligation.
    etape: String,
    /// Seule la compagnie peut la dire faite (`echeance-faite`).
    cochable: bool,
    /// Le mois sur lequel elle porte (la DSN de ce mois, la DPAE d'un engagement de ce mois).
    mois: Option<Mois>,
    annee: Option<i32>,
    fichier: Option<std::path::PathBuf>,
    engagements: i64,
    validees: i64,
    spectacles: Vec<SpectacleARevoir>,
    salarie: Option<droit::contexte::IdSalarie>,
    a_saisir: Vec<String>,
    /// Une DSN : son echeance de depot (`echeance_dsn::Echeance::dite`).
    echeance_depot: Option<declarations::echeance_dsn::EcheanceDite>,
    /// Une obligation due : l'obligation telle que `obligations.rs` l'a calculee.
    obligation: Option<crate::obligations::Obligation>,
    /// Ce sur quoi elle porte (la fin de contrat, l'arret resume), que la page repasse au geste qui la fait.
    objet: Option<Objet>,
}

/// CE SUR QUOI PORTE UNE OBLIGATION DUE : la fin de contrat telle que la route `fins-de-contrat` la rend, ou l'arret
/// resume tel que la route `imprevus` le rend.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub(crate) enum Objet {
    Fin(Box<FinASignaler>),
    Arret(Box<ResumeDArret>),
}

impl Objet {
    /// Le salarie qu'il nomme (la cle de sa fiche).
    fn salarie(&self) -> droit::contexte::IdSalarie {
        match self {
            Objet::Fin(f) => f.salarie.clone(),
            Objet::Arret(a) => a.salarie().clone(),
        }
    }
}

impl Echeance {
    fn nouveau(quoi: GenreDEcheance, cle: CleDEcheance, titre: String, question: String, tombe: Option<Mois>) -> Echeance {
        Echeance {
            quoi,
            cle,
            titre,
            question,
            detail: String::new(),
            source: String::new(),
            tombe,
            date: None,
            quand_dit: String::new(),
            etat: AFaire,
            due: true,
            etape: String::new(),
            cochable: false,
            mois: None,
            annee: None,
            fichier: None,
            engagements: 0,
            validees: 0,
            spectacles: Vec::new(),
            salarie: None,
            a_saisir: Vec::new(),
            echeance_depot: None,
            obligation: None,
            objet: None,
        }
    }

    /// Son dernier jour, et ce jour en clair.
    fn le(&mut self, jour: &Jour) {
        self.date = Some(*jour);
        self.quand_dit = format!("le {}", jour.jj_mm_aaaa());
    }

    /// Ce sur quoi l'echeancier la range : son dernier jour, ou le mois ou elle tombe (en texte : « 2026-11-05 » se range
    /// apres « 2026-11 »).
    fn rang(&self) -> Option<String> {
        match (self.date, self.tombe) {
            (Some(j), _) => Some(j.iso()),
            (None, t) => t.map(String::from),
        }
    }

    /// LE GESTE QUI LA FAIT, en routes du catalogue (`etapes::action` : la route qu'il ouvre, sa charge, la route de
    /// l'acte qui suit, le mot du bouton), celles que l'ecran et le LLM appellent. Une obligation, lue sur ses champs : la
    /// fin d'un contrat se signale (ou, s'il manque une case, se complete dans sa carte : la saisie du mois, puis
    /// l'engagement modifie) ; un signalement DSN d'arret ou de reprise se signale ; une obligation hors DSN qu'une route
    /// dit faite (`accident-declare`, `attestation-declare`) se dit faite ; le reste s'ouvre dans son mois. Une DSN, une
    /// DPAE : l'apercu de son depot, puis le depot. La cloture, l'export compta, la revue des spectacles : leur route.
    /// Le geste d'une route qui a sa structure de charge (le mois, les signalements des imprevus, la revue des
    /// spectacles) la passe ; un objet qui ne la remplit pas (un arret sans son salarie ou son dernier jour travaille)
    /// n'offre pas de geste.
    fn action(&self) -> R<Option<crate::etapes::Action>> {
        use crate::etapes::{action, deposer_dsn, ChargeAnnee, ChargeMois, ChargeObligationDArret};
        use crate::imprevus::{ChargeArretSignaler, ChargeFinContratSignaler, NatureSignalement};
        // L'arret sur lequel porte l'obligation : son salarie et son dernier jour travaille.
        let arret = || match &self.objet {
            Some(Objet::Arret(a)) => Some((a.salarie().clone(), a.dernier_jour_travaille())),
            _ => None,
        };
        let salarie = || self.objet.as_ref().map(Objet::salarie);
        Ok(Some(match &self.quoi {
            GenreDEcheance::Obligation => {
                let Some(o) = self.obligation.as_ref() else { return Ok(None) };
                let hors_dsn = matches!(o.ou, Some(crate::obligations::Ou::Lieux(_)));
                use crate::obligations::CleDObligation as C;
                let (Some(mois), cle) = (self.mois, o.cle) else { return Ok(None) };
                match cle {
                    C::SignalementFinContrat if !self.a_saisir.is_empty() => {
                        action(Route::Saisie, ChargeMois { mois }, Some(Route::ModifierEngagement), "Compléter le contrat".into())
                    }
                    C::SignalementFinContrat => {
                        let Some(s) = salarie() else { return Ok(None) };
                        action(Route::FinContratSignaler, ChargeFinContratSignaler::de(mois, s), Some(Route::Deposer), "Signaler…".into())
                    }
                    cle @ (C::SignalementArret | C::SignalementArretRectificatif | C::SignalementReprise) => {
                        let Some((s, dernier_jour)) = arret() else { return Ok(None) };
                        let nature = if cle == C::SignalementReprise { NatureSignalement::Reprise } else { NatureSignalement::Arret };
                        action(Route::ArretSignaler, ChargeArretSignaler::de(s, dernier_jour, nature), Some(Route::Deposer), "Signaler…".into())
                    }
                    C::DeclarationAccident | C::AttestationSalaire if hors_dsn => {
                        let Some((salarie, dernier_jour)) = arret() else { return Ok(None) };
                        let route = if cle == C::DeclarationAccident { Route::AccidentDeclare } else { Route::AttestationDeclare };
                        action(route, ChargeObligationDArret { salarie, dernier_jour }, None, "Faite le".into())
                    }
                    _ => action(Route::EtatDuMois, ChargeMois { mois }, None, "Ouvrir le mois".into()),
                }
            }
            GenreDEcheance::Dsn => {
                let Some(mois) = self.mois else { return Ok(None) };
                deposer_dsn(mois, None, "Vérifier et envoyer".into())
            }
            GenreDEcheance::Dpae => {
                let Some(mois) = self.mois else { return Ok(None) };
                action(Route::DepotApercu, crate::depots::Designation::du_mois(crate::depots::Quoi::Dpae, mois), Some(Route::Deposer), "Préparer la DPAE".into())
            }
            GenreDEcheance::DeLAnnee(cle) if *cle == CLOTURE || *cle == EXPORT_COMPTA || *cle == REVUE_SPECTACLES => {
                let Some(annee) = self.annee else { return Ok(None) };
                if *cle == CLOTURE {
                    action(Route::Cloture, ChargeAnnee { annee: Annee::nouvelle(i64::from(annee))? }, None, format!("Clôturer {}", annee))
                } else if *cle == EXPORT_COMPTA {
                    action(Route::ExportCompta, ChargeAnnee { annee: Annee::nouvelle(i64::from(annee))? }, None, "Exporter pour la compta".into())
                } else {
                    // Tous continuent, tant qu'on n'en a pas décoché un : la boîte s'ouvre ainsi.
                    let charge = ChargeRevue { annee, continuent: self.spectacles.iter().map(|s| s.cle.clone()).collect() };
                    action(Route::RevueSpectacles, charge, None, "Choisir les spectacles".into())
                }
            }
            GenreDEcheance::DeLAnnee(_) => return Ok(None),
        }))
    }

    /// L'échéance telle que les routes la rendent.
    fn dite(&self) -> R<EcheanceDite> {
        Ok(EcheanceDite {
            quoi: self.quoi.clone(),
            cle: self.cle.clone(),
            titre: self.titre.clone(),
            question: self.question.clone(),
            detail: self.detail.clone(),
            source: self.source.clone(),
            tombe: self.tombe,
            date: self.date,
            quand_dit: self.quand_dit.clone(),
            etat: self.etat,
            en_retard: self.etat == EnRetard,
            rappel: self.due,
            etape: self.etape.clone(),
            cochable: self.cochable,
            pastille: Pastille::de(self.etat.a_l_ecran(), self.etat.mots()),
            mois: self.mois,
            annee: self.annee.map(|a| Annee::nouvelle(i64::from(a))).transpose()?,
            fichier: self.fichier.clone(),
            engagements: self.engagements,
            validees: self.validees,
            spectacles: self.spectacles.clone(),
            salarie: self.salarie.clone(),
            a_saisir: self.a_saisir.clone(),
            echeance_depot: self.echeance_depot.clone(),
            obligation: self.obligation.clone(),
            objet: self.objet.clone(),
            action: self.action()?,
            bouton_coche: if self.etat == Fait { "Pas encore fait" } else { "C'est fait" },
            titre_boite: self.annee.map(|a| format!("Les spectacles de {}", a)),
        })
    }
}

/// UNE ECHEANCE TELLE QUE LES ROUTES LA RENDENT (`rappels`, `echeancier`), champ par champ.
#[derive(Debug, Serialize)]
pub(crate) struct EcheanceDite {
    pub(crate) quoi: GenreDEcheance,
    pub(crate) cle: CleDEcheance,
    pub(crate) titre: String,
    pub(crate) question: String,
    pub(crate) detail: String,
    pub(crate) source: String,
    /// Le mois ou elle tombe ; `null` sans date butoir.
    pub(crate) tombe: Option<Mois>,
    pub(crate) date: Option<Jour>,
    /// Quand, en clair.
    #[serde(rename = "quand")]
    pub(crate) quand_dit: String,
    pub(crate) etat: EtatEcheance,
    pub(crate) en_retard: bool,
    /// Due maintenant : un rappel la dit.
    pub(crate) rappel: bool,
    pub(crate) etape: String,
    pub(crate) cochable: bool,
    /// Sa pastille : la couleur et les mots de son etat.
    pub(crate) pastille: Pastille,
    pub(crate) mois: Option<Mois>,
    pub(crate) annee: Option<Annee>,
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    pub(crate) fichier: Option<std::path::PathBuf>,
    pub(crate) engagements: i64,
    pub(crate) validees: i64,
    pub(crate) spectacles: Vec<SpectacleARevoir>,
    pub(crate) salarie: Option<droit::contexte::IdSalarie>,
    pub(crate) a_saisir: Vec<String>,
    /// Une DSN : son echeance de depot, telle que `declarations::echeance_dsn` l'ecrit.
    pub(crate) echeance_depot: Option<declarations::echeance_dsn::EcheanceDite>,
    /// Une obligation due, telle que `obligations.rs` l'a calculee.
    pub(crate) obligation: Option<crate::obligations::Obligation>,
    /// Ce sur quoi elle porte (la fin de contrat de `imprevus`, l'arret resume).
    pub(crate) objet: Option<Objet>,
    /// Le geste qui la fait (`etapes::action` : une route du catalogue et son bouton).
    pub(crate) action: Option<crate::etapes::Action>,
    /// La case « C'est fait ».
    pub(crate) bouton_coche: &'static str,
    /// Le titre de sa boite.
    pub(crate) titre_boite: Option<String>,
}

/// Un spectacle actif que la revue de l'annee demande : sa cle, son titre et son numero d'objet tels que sa fiche les
/// porte (vides quand elle ne les dit pas), et son nom a l'ecran (`nomme` : son titre, sinon sa cle).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct SpectacleARevoir {
    cle: droit::contexte::IdSpectacle,
    titre: Option<String>,
    numero_objet: Option<droit::fiche::NumeroObjetSpectacle>,
    nomme: String,
}

/// Ce qu'une echeance lit : la compagnie, le jour, et le journal des depots (lu une fois).
pub(crate) struct Etat<'a> {
    pub(crate) ctx: &'a Contexte,
    pub(crate) aujourdhui: Jour,
    pub(crate) journal: std::sync::Arc<Vec<envois::journal::Ligne>>,
}

impl Etat<'_> {
    fn racine(&self) -> &Path {
        &self.ctx.racine
    }

    fn de(ctx: &Contexte) -> socle::echec::R<Etat<'_>> {
        Ok(Etat { ctx, aujourdhui: ctx.aujourdhui, journal: envois::journal::lu(&ctx.racine).map(|j| j.lignes)? })
    }
}

/// `rappels` : les echeances dues maintenant (`echeances`, `due`), dans l'ordre ou elles se font, au jour de la compagnie.
pub(crate) fn rappels(ctx: &Contexte) -> socle::echec::R<Rappels> {
    let st = Etat::de(ctx)?;
    let rappels: Vec<EcheanceDite> = echeances(&st)?.iter().filter(|e| e.due).map(Echeance::dite).collect::<R<_>>()?;
    let phrases: Vec<String> = rappels
        .iter()
        .map(|r| {
            let quand = if r.quand_dit.is_empty() { String::new() } else { format!(" Échéance : {}.", r.quand_dit) };
            format!("{} ({}) : {} {}{}", r.titre, r.pastille.mots, r.question, r.detail, quand).replace("  ", " ")
        })
        .collect();
    Ok(Rappels {
        phrase: format!("Rappels : {}.", rappels.len()),
        fermer_dit: if rappels.is_empty() { "Fermer" } else { "Plus tard" },
        rappels,
        phrases,
    })
}

/// `echeancier` : les memes echeances, rangees. « Ce mois-ci » : celles qui sont dues (un rappel les dit, avec leur
/// geste) et celles qui tombent ce mois ; « a venir » : les autres, sur douze mois.
fn echeancier(ctx: &Contexte) -> socle::echec::R<Echeancier> {
    let st = Etat::de(ctx)?;
    let courant = st.aujourdhui.mois_civil();
    let fin = courant.plus(MOIS_A_VENIR)?;
    let (mut ce_mois, mut a_venir) = (Vec::new(), Vec::new());
    for e in echeances(&st)? {
        if e.due || e.tombe == Some(courant) {
            ce_mois.push(e);
        } else if e.tombe.is_some_and(|t| courant < t && t <= fin) {
            a_venir.push(e);
        }
    }
    let ordre = |etat: EtatEcheance| match etat {
        EnRetard => 0,
        AFaire => 1,
        Fait => 2,
    };
    ce_mois.sort_by_cached_key(|e| (ordre(e.etat), e.rang()));
    a_venir.sort_by_key(|e| (e.tombe, e.date));
    let compagnie = ctx.entreprise.nommee().map(str::to_string);
    let combien = format!("{} ce mois-ci, {} à venir.", socle::ecran::compte(ce_mois.len(), "échéance", "échéances"), a_venir.len());
    let phrase = match &compagnie {
        Some(c) => format!("{} : {}", c, combien),
        None => combien,
    };
    let a_faire = ce_mois.iter().filter(|x| !x.due && matches!(x.etat, AFaire | EnRetard)).count();
    let ce_mois: Vec<EcheanceDite> = ce_mois.iter().map(Echeance::dite).collect::<R<_>>()?;
    let a_venir: Vec<EcheanceDite> = a_venir.iter().map(Echeance::dite).collect::<R<_>>()?;
    let ligne = |x: &EcheanceDite| format!("{} ({}{})", x.titre, x.pastille.mots, if x.quand_dit.is_empty() { String::new() } else { format!(", {}", x.quand_dit) });
    let phrases: Vec<String> = ce_mois.iter().chain(&a_venir).map(ligne).collect();
    Ok(Echeancier {
        phrase,
        aujourdhui: st.aujourdhui,
        date_dite: date_en_clair(&st.aujourdhui),
        date_dit: format!("Nous sommes le {}", date_en_clair(&st.aujourdhui)),
        bienvenue: "Bienvenue sur PuppetPaie",
        compagnie,
        idcc: ctx.entreprise.idcc.dite().cloned(),
        ce_mois,
        a_venir,
        a_faire,
        phrases,
    })
}

/// TOUT CE QUI EST DU, ET QUAND : la seule fonction qui compose les echeances, dans l'ordre ou elles se font. D'abord
/// les DPAE (elles se comptent en heures, avant la premiere heure de travail), puis les obligations nees d'un evenement
/// (quarante-huit heures pour un accident, cinq jours ouvres pour un signalement), la DSN des mois passes qui n'en ont
/// pas, les echeances annuelles (la cloture apres la DSN de decembre, l'export compta, la revue des spectacles, le taux
/// AT/MP, les contributions de branche), et la DSN des douze mois qui viennent. Une echeance se tait des qu'elle est
/// faite ; aucune ne se tait parce qu'on a clique « plus tard ».
pub(crate) fn echeances(st: &Etat) -> R<Vec<Echeance>> {
    let base = echeancier_base()?;
    let mut out = rappels_dpae(st)?;
    out.extend(rappels_obligations(st)?);
    let reclames = mois_a_declarer(st)?;
    for m in &reclames {
        out.push(rappel_dsn(st, *m, &base.dsn)?);
    }
    out.extend(annuelles(st, &base)?);
    // La DSN du mois passe (quand elle n'est plus reclamee) et des douze qui viennent ; rien avant la premiere embauche.
    let premier = premier_mois(st.ctx)?;
    for n in -1..MOIS_A_VENIR {
        let m = st.aujourdhui.mois_civil().plus(n)?;
        if reclames.contains(&m) || premier.is_some_and(|p| m < p) {
            continue;
        }
        out.push(dsn_a_venir(st, m, &base.dsn)?);
    }
    Ok(out)
}

/// Le premier mois de la compagnie (`etat_dsn::premier_mois_du`), s'il y en a un.
fn premier_mois(ctx: &Contexte) -> R<Option<Mois>> {
    etat_dsn::premier_mois_du(ctx)
}

/// La DSN d'un mois que les rappels ne reclament pas : faite, ou a venir.
fn dsn_a_venir(st: &Etat, mois: Mois, modele: &ModeleDsn) -> R<Echeance> {
    let echeance = declarations::echeance_dsn::de_la_compagnie(st.ctx, mois)?;
    let titre = formater(&modele.titre, &[("de_mois", socle::texte::de(&mois.libelle(Graphie::Accentuee)))])?;
    let mut e = Echeance::nouveau(GenreDEcheance::Dsn, CleDEcheance::Dsn(mois), titre, String::new(), Some(mois.suivant()));
    e.etat = if !etat_dsn::due(st.ctx, mois, None)? { Fait } else if echeance.passee_au(&st.aujourdhui) { EnRetard } else { AFaire };
    e.due = false;
    e.mois = Some(mois);
    e.detail = dsn_detail(modele, &echeance)?;
    e.source = formater(&modele.source, &[])?;
    e.date = Some(echeance.jour);
    e.quand_dit = echeance.phrase();
    e.echeance_depot = Some(echeance.dite()?);
    Ok(e)
}

fn dsn_detail(modele: &ModeleDsn, echeance: &declarations::echeance_dsn::Echeance) -> R<String> {
    formater(
        &modele.detail,
        &[
            ("jour", echeance.jour.jour.to_string()),
            ("mois_suivant", echeance.jour.mois_civil().libelle(Graphie::Accentuee)),
            // Une echeance qui n'est pas reportee n'a rien a dire de son report.
            ("report", echeance.pourquoi().map(|p| format!(" {}", p)).into_iter().collect()),
        ],
    )
}

/// LES ECHEANCES ANNUELLES (`echeancier-base.json`, celles de la convention de la fiche), de l'annee en cours et de la
/// suivante. La cloture de l'annee passee vient APRES sa DSN de decembre, et l'export compta avec elle.
fn annuelles(st: &Etat, base: &EcheancierBase) -> R<Vec<Echeance>> {
    let ctx = st.ctx;
    let faites = cochees(ctx)?;
    let derniere = st.aujourdhui.mois_civil().plus(MOIS_A_VENIR)?.annee();
    let premier = premier_mois(ctx)?;
    let mut out = Vec::new();
    for annee in st.aujourdhui.an..=derniere {
        for e in annuelles_de(base, ctx.entreprise.idcc.dite()) {
            if let Some(x) = annuelle(st, e, annee, &faites, premier)? {
                let export = x.quoi == GenreDEcheance::DeLAnnee(CLOTURE) && x.etat != Fait;
                out.push(x);
                if export {
                    out.extend(export_compta(st, annee - 1)?);
                }
            }
        }
    }
    Ok(out)
}

/// L'export compta de l'annee passee : du des janvier, tant que l'annee n'est pas close et que l'export n'est pas a jour.
fn export_compta(st: &Etat, annee: i32) -> R<Option<Echeance>> {
    if annee != st.aujourdhui.an - 1 || crate::gestes::exercice_ecrit::export_compta_a_jour(&st.ctx.racine, Annee::nouvelle(i64::from(annee))?)? {
        return Ok(None);
    }
    let mut r = Echeance::nouveau(
        GenreDEcheance::DeLAnnee(EXPORT_COMPTA),
        CleDEcheance::de_l_annee(&EXPORT_COMPTA, annee)?,
        format!("Exporter les chiffres de {} pour la comptabilité", annee),
        "Le journal de paie de l'année, pour le logiciel de compta.".into(),
        Some(Mois::nouveau(annee + 1, 1)?),
    );
    r.annee = Some(annee);
    r.detail = "Une écriture par mois, équilibrée : salaires bruts et charges au débit, organismes et net à payer au crédit.".into();
    Ok(Some(r))
}

/// Une echeance annuelle de l'annee `annee` ; `None` quand elle ne concerne pas ce dossier.
fn annuelle(st: &Etat, e: &Annuelle, annee: i32, faites: &Cochees, premier: Option<Mois>) -> R<Option<Echeance>> {
    let ctx = st.ctx;
    let cle = &e.cle;
    let precedente = annee - 1;
    // La cloture d'une annee dont aucun mois n'est dans PuppetPaie (tenue ailleurs) ne concerne pas ce dossier.
    if e.fait_par == FaitPar::ExerciceClos && mois_de_l_annee(ctx, Annee::nouvelle(i64::from(precedente))?)?.is_empty() {
        return Ok(None);
    }
    // Une revue des spectacles sans spectacle n'a rien a revoir.
    if *cle == REVUE_SPECTACLES && ctx.spectacles.is_empty() {
        return Ok(None);
    }
    let trop_courte = || erreur(Classe::Manque, socle::fichiers::trop_courte("les mois de l'échéance"));
    let debut = Mois::nouveau(annee, *e.mois.first().ok_or_else(trop_courte)?)?;
    let dernier = Mois::nouveau(annee, *e.mois.last().ok_or_else(trop_courte)?)?;
    // Ce qui tombait avant l'arrivee de la compagnie ne la concerne pas -- sauf le taux AT/MP de l'annee, sans lequel
    // rien ne se calcule.
    let arrivee_apres = premier.is_some_and(|b| dernier < b);
    if arrivee_apres && *cle != AT_MP {
        return Ok(None);
    }
    // `{jour_dsn}` : le jour de janvier ou la DSN de decembre de l'annee d'avant est due, reporte (`echeance_dsn`).
    let decembre = Mois::nouveau(precedente, 12)?;
    let dsn_decembre = declarations::echeance_dsn::de_la_compagnie(ctx, decembre)?;
    let valeurs = [("annee", annee.to_string()), ("annee_precedente", precedente.to_string()), ("jour_dsn", dsn_decembre.jour.jour.to_string())];
    let courant = st.aujourdhui.mois_civil();
    let mut x = Echeance::nouveau(GenreDEcheance::DeLAnnee(cle.clone()), CleDEcheance::de_l_annee(cle, annee)?, formater(&e.titre, &valeurs)?, String::new(), Some(debut));
    x.detail = formater(&e.detail, &valeurs)?;
    x.source = e.source.clone();
    x.cochable = e.fait_par == FaitPar::Coche;
    x.annee = Some(annee);
    x.etat = match fait_annuel(ctx, e, annee, faites)? {
        Some(true) => Fait,
        // Une compagnie arrivee apres l'echeance n'est pas « en retard » : elle a a le faire.
        Some(false) if courant > dernier && !arrivee_apres => EnRetard,
        _ => AFaire,
    };
    // Due : son premier mois est venu, et elle n'est pas faite.
    x.due = matches!(x.etat, AFaire | EnRetard) && debut <= courant;
    if *cle == CLOTURE {
        x.annee = Some(precedente);
        x.cle = CleDEcheance::de_l_annee(&CLOTURE, precedente)?;
        // LA CLOTURE VIENT APRES LA DSN DE DECEMBRE : tant qu'elle n'est pas partie, c'est son rappel qui parle.
        x.quand_dit = format!("après la DSN de décembre, due {}", dsn_decembre.phrase());
        let partie = etat_dsn::du_mois(ctx, decembre, None)?;
        x.due = x.due && (partie.partie() || partie.refusee());
        x.etat = if x.etat == EnRetard && !x.due { AFaire } else { x.etat };
        if x.due {
            x.titre = format!("Clôturer l'exercice {}", precedente);
            x.question = format!("La DSN de décembre est passée. Figer l'année {} ?", precedente);
        }
    }
    if *cle == REVUE_SPECTACLES && x.due {
        x.spectacles = spectacles_a_revoir(ctx, annee, faites)?;
        x.question = format!("Lesquels de vos spectacles se jouent encore en {} ?", annee);
    }
    Ok(Some(x))
}

/// Les echeances annuelles de la convention de la fiche (`idcc`) : celles qui n'en nomment aucune, et celles qui la
/// nomment.
fn annuelles_de<'a>(base: &'a EcheancierBase, idcc: Option<&'a droit::fiche::Idcc>) -> impl Iterator<Item = &'a Annuelle> {
    base.annuelles.iter().filter(move |e| e.idcc.as_ref().is_none_or(|l| idcc.is_some_and(|i| l.iter().any(|x| x == i))))
}

/// Vrai, faux, ou `None` quand on ne peut pas le savoir.
fn fait_annuel(ctx: &Contexte, e: &Annuelle, annee: i32, faites: &Cochees) -> R<Option<bool>> {
    if e.fait_par == FaitPar::ExerciceClos {
        return Ok(Some(exercice_clos(ctx, Annee::nouvelle(i64::from(annee) - 1)?)?.is_some()));
    }
    // Le taux AT/MP se saisit dans la page : pose pour l'annee (ou un taux collectif que l'annee reprend d'elle-meme,
    // `Contexte::taux_atmp_de`), il vaut releve.
    if e.cle == AT_MP && matches!(ctx.taux_atmp_de(annee), Some(Ok(t)) if t.valeur.is_some()) {
        return Ok(Some(true));
    }
    if e.fait_par == FaitPar::Coche {
        return Ok(Some(faites.fait.contains_key(&CleDEcheance::de_l_annee(&e.cle, annee)?)));
    }
    Ok(None)
}

// ------------------------------------------------------ l'echeancier livre

/// L'ECHEANCIER LIVRE (`donnees/echeancier-base.json`) : les phrases de la DSN mensuelle, et les echeances de
/// calendrier, chacune avec son texte. La seule lecture du fichier.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EcheancierBase {
    /// Ce qu'est le fichier.
    #[serde(deserialize_with = "socle::forme::non_lue")]
    quoi: socle::forme::NonLue,
    dsn: ModeleDsn,
    annuelles: Vec<Annuelle>,
}

/// La DSN mensuelle : son titre (`{de_mois}`), son detail (`{jour}`, `{mois_suivant}`, `{report}`), ses textes.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModeleDsn {
    titre: String,
    detail: String,
    source: String,
}

/// Une echeance de calendrier : ses mois (le premier ou elle est due, le dernier apres lequel elle est en retard), son
/// titre et son detail (`{annee}`, `{annee_precedente}`, `{jour_dsn}`), son texte, qui la dit faite ; les conventions
/// qu'elle concerne (aucune : toutes).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Annuelle {
    cle: CleDAnnuelle,
    idcc: Option<Vec<droit::fiche::Idcc>>,
    mois: Vec<u32>,
    titre: String,
    detail: String,
    source: String,
    fait_par: FaitPar,
}

/// Qui dit une echeance annuelle faite : la cloture de l'exercice, ou la compagnie qui la coche. Une echeance n'existe
/// que s'il y a quelque chose a faire : ce qui n'est qu'une information n'en est pas une.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FaitPar {
    ExerciceClos,
    Coche,
}

fn echeancier_base() -> socle::echec::R<EcheancierBase> {
    socle::forme::lire_fichier(&socle::fichiers::dossier_du_code().join("echeancier-base.json"), "L'échéancier livré avec le logiciel ne se lit pas : le signaler au développeur.").map_err(|m| m.reclasser(Classe::Illisible))
}

// ------------------------------------------------------ ce que la compagnie a coche

/// Ce que l'utilisateur a coche dans l'echeancier, sous la racine de la compagnie.
pub(crate) const ECHEANCIER: &str = "config/echeancier.json";

/// `config/echeancier.json` : les echeances que la compagnie a dites faites (« quoi:AAAA »), quand et par qui.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Cochees {
    quoi: String,
    fait: BTreeMap<CleDEcheance, Coche>,
}

/// Ce qu'est `config/echeancier.json`.
const QUOI_COCHEES: &str = "Les echeances que l'utilisateur a dites faites.";

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Coche {
    le: Jour,
    qui: String,
}

/// `config/echeancier.json`, lu : rien de coche s'il n'existe pas. La seule lecture du fichier ; `marquer_echeance` est
/// la seule ecriture.
fn cochees(ctx: &Contexte) -> socle::echec::R<Cochees> {
    let p = ctx.racine.join(ECHEANCIER);
    if !p.exists() {
        return Ok(Cochees { quoi: QUOI_COCHEES.to_string(), fait: BTreeMap::new() });
    }
    socle::forme::lire_fichier(&p, "Les échéances cochées de la compagnie ne se lisent pas.").map_err(|m| m.reclasser(Classe::Illisible))
}

/// Coche (ou decoche) une echeance que seule la compagnie peut dire faite : « quoi:AAAA », `quoi` une echeance annuelle
/// cochable de l'echeancier livre.
fn marquer_echeance(ctx: &Contexte, cle: &CleDEcheance, fait: bool, qui: &str) -> socle::echec::R<()> {
    let inconnue = || erreur(Classe::Refuse, format!("Echeance inconnue : {}.", socle::fichiers::repr(&cle.to_string())));
    let CleDEcheance::DeLAnnee { quoi, annee } = cle else { return Err(inconnue()) };
    let base = echeancier_base()?;
    let Some(e) = base.annuelles.iter().find(|x| x.cle == *quoi) else {
        return Err(inconnue());
    };
    if e.fait_par != FaitPar::Coche {
        let titre = formater(&e.titre, &[("annee", annee.to_string()), ("annee_precedente", (annee.nombre() - 1).to_string()), ("jour_dsn", String::new())])?;
        return Err(erreur(Classe::Refuse, format!("« {} » ne se coche pas : le logiciel le voit fait quand c'est fait.", titre)));
    }
    let mut c = cochees(ctx)?;
    if fait {
        c.fait.insert(cle.clone(), Coche { le: ctx.aujourdhui, qui: qui.to_string() });
    } else {
        c.fait.remove(cle);
    }
    c.quoi = QUOI_COCHEES.to_string();
    let p = ctx.racine.join(ECHEANCIER);
    if let Some(parent) = p.parent() {
        crate::fiches::creer_dossier(parent)?;
    }
    socle::ecriture::ecrire_json(&p, &c, 2).map_err(|e| e.reclasser(Classe::Disque))
}

/// `{nom}` remplace, `{{` et `}}` s'ecrivent `{` et `}`.
fn formater(modele: &str, valeurs: &[(&str, String)]) -> R<String> {
    let mut out = String::new();
    let mut it = modele.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '{' if it.peek() == Some(&'{') => {
                it.next();
                out.push('{');
            }
            '}' if it.peek() == Some(&'}') => {
                it.next();
                out.push('}');
            }
            '{' => {
                let mut nom = String::new();
                loop {
                    match it.next() {
                        Some('}') => break,
                        Some(x) => nom.push(x),
                        None => return Err(erreur(Classe::Illisible, "une accolade ouverte n'est pas fermée avant la fin du texte")),
                    }
                }
                let v = valeurs.iter().find(|(k, _)| *k == nom).ok_or_else(|| socle::fichiers::absent(&nom).reclasser(Classe::Manque))?;
                out.push_str(&v.1);
            }
            '}' => return Err(erreur(Classe::Illisible, "une accolade fermée n'a pas été ouverte")),
            c => out.push(c),
        }
    }
    Ok(out)
}

/// « mercredi 23 septembre 2026 ».
fn date_en_clair(j: &Jour) -> String {
    const JOURS: [&str; 7] = ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"];
    let mois = j.mois_civil().nom(socle::mois::Graphie::Accentuee);
    format!("{} {} {} {}", JOURS[j.jour_de_la_semaine() as usize], j.jour, mois, j.an)
}

// -------------------------------------------------------------- l'exercice

/// La cloture de l'annee (ou `None`).
pub(crate) fn exercice_clos(ctx: &Contexte, annee: Annee) -> R<Option<crate::gestes::exercice_ecrit::ExerciceNote>> {
    Ok(crate::gestes::exercice_ecrit::lire_exercices(&ctx.racine)?.de(annee).cloned())
}

/// Les mois de l'annee connus du dossier.
pub(crate) fn mois_de_l_annee(ctx: &Contexte, annee: Annee) -> R<Vec<Mois>> {
    let mut v: Vec<Mois> = super::ouverture::mois_connus(&ctx.racine)?.into_iter().map(|m| m.0).filter(|m| m.annee_civile() == annee).collect();
    v.sort();
    Ok(v)
}

/// `spectacles_a_revoir` : les spectacles actifs, tant que la revue de l'annee
/// n'est pas faite.
fn spectacles_a_revoir(ctx: &Contexte, annee: i32, faites: &Cochees) -> R<Vec<SpectacleARevoir>> {
    if faites.fait.contains_key(&CleDEcheance::de_l_annee(&REVUE_SPECTACLES, annee)?) {
        return Ok(Vec::new());
    }
    Ok(ctx.spectacles
        .iter()
        .filter(|(_, s)| !s.archivee())
        .map(|(cle, s)| {
            let titre = s.titre.remplie().map(str::to_string);
            let nomme = match &titre {
                Some(t) => t.clone(),
                None => cle.to_string(),
            };
            SpectacleARevoir { cle: cle.clone(), titre, numero_objet: s.numero_objet.dite().cloned(), nomme }
        })
        .collect())
}

// ------------------------------------------------------- la DSN des mois

/// Le mois le plus recent dont la DSN reelle est partie et n'a pas ete
/// refusee -- passee, ou en attente de son bilan.
fn derniere_dsn_passee(st: &Etat) -> R<Option<Mois>> {
    let mois: BTreeSet<Mois> = envois::journal::filtrer(&st.journal, Some(envois::journal::Acte::Dsn), None, true).filter(|l| l.accepte() && !l.signalement()).filter_map(|l| l.mois()).collect();
    let mut passes = Vec::new();
    for m in mois {
        if !etat_dsn::du_mois(st.ctx, m, None)?.a_faire() {
            passes.push(m);
        }
    }
    Ok(passes.into_iter().max())
}

/// Les mois dont la DSN manque -- ou a ete refusee --, du plus ancien au plus
/// recent : depuis le mois qui suit la derniere DSN passee jusqu'au mois d'avant
/// aujourd'hui. Sans aucune DSN au journal : le seul mois d'avant -- on ne va pas
/// reclamer cinq ans d'histoire a une compagnie qui commence.
fn mois_a_declarer(st: &Etat) -> R<Vec<Mois>> {
    let dernier = st.aujourdhui.mois_civil().precedent();
    let depuis = derniere_dsn_passee(st)?;
    let mut premier = match depuis {
        Some(d) => d.suivant(),
        None => dernier,
    };
    if let (Some(borne), None) = (premier_mois(st.ctx)?, &depuis) {
        premier = premier.max(borne);
    }
    let mut out = Vec::new();
    for m in premier.jusqu_a(dernier) {
        if etat_dsn::due(st.ctx, m, None)? {
            out.push(m);
        }
    }
    Ok(out)
}

/// LES MOIS QUI ONT QUELQUE CHOSE A FAIRE au jour `aujourdhui`, du plus ancien au plus recent : ceux que la saisie
/// touche (`stock::mois_saisis`), et ceux dont la DSN est reclamee sans engagement (`mois_a_declarer` : une DSN sans
/// individu, cahier technique DSN 2026.1, § 2.4). Le mois a ouvrir et l'horloge de la compagnie fictive les parcourent.
pub(crate) fn mois_a_parcourir(ctx: &Contexte, aujourdhui: Jour) -> socle::echec::R<Vec<Mois>> {
    let mut mois = engagement::stock::mois_saisis(&ctx.racine)?;
    let st = Etat { ctx, aujourdhui, journal: envois::journal::lu(&ctx.racine)?.lignes };
    mois.extend(mois_a_declarer(&st)?);
    mois.sort();
    mois.dedup();
    Ok(mois)
}

/// `(nom, chemin)` de la DSN mensuelle du dossier du mois, la premiere dans l'ordre de
/// `crate::etapes::ordre_de_depot` ; une DSN qui ne se lit pas se refuse.
fn dsn_deposable(st: &Etat, mois: Mois) -> R<Option<(String, std::path::PathBuf)>> {
    // Le mois n'est ecrit que si le moteur a note sa DSN mensuelle (`depart::mois_ecrit`, le critere de l'horloge
    // et de l'accueil) : un fichier DSN qu'il n'a pas note ne fait pas « le mois est ecrit ».
    let dossier = st.racine().join("sorties").join(mois.to_string());
    if !crate::gestes::depart::mois_ecrit(st.racine(), mois)? || !dossier.is_dir() {
        return Ok(None);
    }
    let disque = |e: std::io::Error| socle::echec::du_disque(&dossier, &e).reclasser(Classe::du_disque(&e));
    let mut fichiers = Vec::new();
    for e in std::fs::read_dir(&dossier).map_err(disque)? {
        let e = e.map_err(disque)?;
        let (nom, c) = (e.file_name().to_string_lossy().into_owned(), e.path());
        if !c.is_file() || !nom.to_lowercase().ends_with(".dsn") {
            continue;
        }
        // LA MENSUELLE, PAS UN SIGNALEMENT : un signalement d'arret ou de fin ecrit apres la DSN du mois, dans le meme
        // dossier, n'est pas « la DSN a deposer » du rappel.
        let ordre = crate::etapes::ordre_de_depot(&nom, &c)?;
        if ordre.0 == 0 {
            fichiers.push((ordre, nom, c));
        }
    }
    fichiers.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(fichiers.into_iter().next().map(|(_, nom, c)| (nom, c)))
}

/// Combien d'engagements le mois montre, et combien sont valides.
fn engagements_valides(ctx: &Contexte, mois: Mois) -> R<(i64, i64)> {
    let valides = engagement::etats::valides_du_mois(ctx, mois)?;
    Ok((valides.len() as i64, valides.iter().filter(|v| **v).count() as i64))
}

fn rappel_dsn(st: &Etat, le_mois: Mois, modele: &ModeleDsn) -> R<Echeance> {
    let mois = le_mois;
    let lib = le_mois.libelle(Graphie::Accentuee);
    let mut r = Echeance::nouveau(
        GenreDEcheance::Dsn,
        CleDEcheance::Dsn(mois),
        format!("Il est temps d'envoyer la DSN {}", socle::texte::de(&lib)),
        format!("Avez-vous bien fini les paies {} ?", socle::texte::de(&lib)),
        Some(le_mois.suivant()),
    );
    r.mois = Some(le_mois);
    // L'echeance : `echeance_dsn`, le 5 ou le 15 du mois suivant selon l'effectif, reporte au jour ouvre suivant un
    // samedi, un dimanche ou un jour ferie (R. 133-14, I), a midi. Passee, ce n'est plus un rappel, c'est un retard --
    // et on le dit.
    let echeance = declarations::echeance_dsn::de_la_compagnie(st.ctx, le_mois)?;
    r.etat = if echeance.passee_au(&st.aujourdhui) { EnRetard } else { AFaire };
    r.date = Some(echeance.jour);
    r.quand_dit = echeance.phrase();
    r.source = formater(&modele.source, &[])?;
    r.echeance_depot = Some(echeance.dite()?);

    if !engagement::stock::a_une_saisie(&st.ctx.racine, mois)? {
        r.etape = A_SAISIR.into();
        r.detail = format!(
            "Aucun engagement saisi pour {}. S'il n'y a eu aucun cachet, une DSN « néant » se dépose quand même : le mois s'écrit vide, puis se dépose.",
            lib
        );
        return Ok(r);
    }
    let (engagements, validees) = engagements_valides(st.ctx, mois)?;
    r.engagements = engagements;
    r.validees = validees;

    let etat = etat_dsn::du_mois(st.ctx, mois, None)?;
    if etat.refusee() {
        r.etape = A_CORRIGER.into();
        let motif = match etat.anomalies.as_deref() {
            Some(a) if !a.is_empty() => a.join(" ; "),
            _ => "motif non lu".to_string(),
        };
        r.detail = format!("La DSN de {} a été REFUSÉE : {}. À corriger et à renvoyer.", lib, motif);
        return Ok(r);
    }

    let s = |n: i64| if n == 1 { "" } else { "s" };
    if let Some((nom, chemin)) = dsn_deposable(st, mois)? {
        r.etape = A_DEPOSER.into();
        r.detail = format!(
            "{} engagement{}, {} validé{}. Le mois est écrit ({}) ; la DSN n'est pas partie.",
            r.engagements,
            s(r.engagements),
            r.validees,
            s(r.validees),
            nom
        );
        r.fichier = Some(chemin);
    } else {
        r.etape = A_PRODUIRE.into();
        r.detail = format!(
            "{} engagement{} saisi{}, {} validé{}. Le mois n'est pas encore écrit : bulletins, DSN et le reste s'écrivent d'abord, puis la DSN se dépose.",
            r.engagements,
            s(r.engagements),
            s(r.engagements),
            r.validees,
            s(r.validees)
        );
    }
    if r.engagements != 0 && r.validees < r.engagements {
        let reste = r.engagements - r.validees;
        r.detail.push_str(&format!(" Il reste {} engagement{} non validé{}.", reste, s(reste), s(reste)));
    }
    ajouter_les_mois_de_l_ancien_logiciel(st, mois, &mut r)?;
    Ok(r)
}

/// Ce que la DSN de `mois` declare en plus pour les mois dont l'ancien logiciel a depose la DSN, dit comme la page Paie
/// le dit (`import::mois::dsn_de_l_ancien_logiciel`).
fn ajouter_les_mois_de_l_ancien_logiciel(st: &Etat, mois: Mois, r: &mut Echeance) -> R<()> {
    for m in engagement::stock::mois_saisis(st.racine())?.into_iter().filter(|&m| m < mois) {
        let Some(d) = import::mois::dsn_de_l_ancien_logiciel(st.ctx, m)? else { continue };
        if mois == d.porteur && d.engagements > 0 {
            let lib = m.libelle(Graphie::Accentuee);
            r.detail.push_str(&format!(" {} : {}", socle::texte::capitaliser(&lib), d.phrase));
        }
    }
    Ok(())
}

// ------------------------------------------------------ les obligations dues

/// CE QUI EST DU, A L'OUVERTURE : toute obligation que `obligations.rs` dit a faire ou en retard -- la
/// fin d'un contrat qui se signale, et ce qu'un arret fait naitre (le signalement d'arret ou de reprise anticipee, la
/// declaration d'accident du travail ou de trajet sous quarante-huit heures, l'attestation de salaire du deuil d'enfant).
/// UN SEUL MECANISME : chaque obligation devient un rappel par `rappel_d_obligation`, qui porte l'obligation telle que le
/// moteur l'a calculee (sa date butoir, son etat, sa phrase, ses textes) et ce sur quoi elle porte (`objet` : la fin de
/// contrat de la route `fins-de-contrat`, ou l'arret resume comme la route `imprevus` le resume) -- la page en tire le meme
/// geste que dans la colonne du mois. Le plus presse d'abord : en retard, puis par date butoir.
///
/// Ils se taisent des que la chose est faite -- le depot REEL du signalement note au journal, la declaration ou
/// l'attestation dite faite (`accident-declare`, `attestation-declare`) --, jamais parce qu'on a clique « plus tard ».
/// Ce qui n'est pas encore du (`a_venir`), ce qui est a la charge du salarie (la maladie professionnelle) et ce dont le
/// delai ne se compte pas (`indetermine` : il n'a pas de date butoir ; la colonne du mois le montre) ne s'y rappellent pas.
fn rappels_obligations(st: &Etat) -> R<Vec<Echeance>> {
    let mut out = Vec::new();

    // LES FINS DE CONTRAT : celles du mois passe et du mois en cours, deja finies (`crate::imprevus::fins_a_signaler`, la
    // meme liste que la colonne du mois), et celles d'avant dont le signalement est en retard (`fins_en_retard_avant`). Un CDD
    // d'usage du spectacle arrive a son terme (031) n'en fait pas partie.
    let mois = st.aujourdhui.mois_civil();
    let precedent = mois.precedent();
    let mut fins = crate::imprevus::fins_en_retard_avant(st.ctx, precedent, st.aujourdhui)?;
    for m in [precedent, mois] {
        fins.extend(crate::imprevus::fins_a_signaler(st.ctx, m, st.aujourdhui)?);
    }
    for f in fins {
        if f.fin > st.aujourdhui {
            continue; // il n'a pas encore pris fin
        }
        if !du(&f.obligation) {
            continue;
        }
        let salarie = f.salarie.to_string();
        let numero = match &f.numero {
            Some(n) => format!("Le contrat {}", n),
            None => "Le contrat sans numéro".to_string(),
        };
        let a_saisir: Vec<String> = f.a_saisir.iter().map(crate::imprevus::phrase_du_manque).collect();
        let mut r = rappel_d_obligation(
            &f.obligation,
            CleDEcheance::DuSalarie { quoi: QuoiDuSalarie::FinContrat, salarie: f.salarie.clone(), jour: f.fin },
            format!("Signaler la fin du contrat de {}", salarie),
            format!("{} s'est terminé le {}.", numero, f.fin.jj_mm_aaaa()),
            Objet::Fin(Box::new(f.clone())),
        );
        r.mois = Some(f.mois);
        r.salarie = Some(f.salarie.clone());
        r.etape = if a_saisir.is_empty() { A_PRODUIRE } else { A_SAISIR }.into();
        if !a_saisir.is_empty() {
            r.detail.push_str(&format!(" À saisir d'abord, dans la carte du contrat : {}", a_saisir.join(" ")));
        }
        r.a_saisir = a_saisir;
        out.push(r);
    }

    // CE QU'UN ARRET FAIT NAITRE : tous les arrets du magasin des imprevus (`crate::metier::imprevus::tous_les_arrets`), leurs
    // obligations comptees par `obligations::de_l_arret` au jour du rappel.
    let arrets = crate::metier::imprevus::tous_les_arrets(st.ctx)?;
    if !arrets.is_empty() {
        let n = crate::metier::imprevus::nomenclatures()?;
        let suivi = crate::obligations::Suivi::lire(st.ctx)?;
        for a in &arrets {
            let obligations = crate::obligations::de_l_arret(st.ctx, a, &suivi, st.aujourdhui)?;
            if !obligations.iter().any(du) {
                continue;
            }
            let resume = crate::imprevus::resume_d_arret_complet(st.ctx, &n, a, &suivi, st.aujourdhui)?;
            let nom = st.ctx.nomme(&a.salarie);
            let dernier = a.dernier_jour_travaille;
            for o in obligations.iter().filter(|o| du(o)) {
                let mut r = rappel_d_obligation(
                    o,
                    CleDEcheance::DuSalarie { quoi: QuoiDuSalarie::Obligation(o.cle), salarie: a.salarie.clone(), jour: dernier },
                    format!("{} — {}", o.libelle, nom),
                    format!("{}.", resume.en_clair()),
                    Objet::Arret(Box::new(resume.clone())),
                );
                r.mois = Some(dernier.mois_civil());
                r.salarie = Some(a.salarie.clone());
                out.push(r);
            }
        }
    }

    // Le plus presse d'abord : ce qui est en retard, puis par date butoir (le tri est stable : a butoir egale, l'ordre
    // de lecture reste).
    out.sort_by_key(|r| (r.etat != EnRetard, r.date));
    Ok(out)
}

/// Une obligation due : a faire, ou en retard.
fn du(o: &crate::obligations::Obligation) -> bool {
    o.etat.du()
}

/// Le rappel d'une obligation : son titre, sa question, et ce que le moteur en a calcule -- la date butoir (`jour`), le
/// retard, la phrase -- ; `objet` : ce sur quoi elle porte, que la page repasse au geste qui la fait.
fn rappel_d_obligation(o: &crate::obligations::Obligation, cle: CleDEcheance, titre: String, question: String, objet: Objet) -> Echeance {
    // La date butoir, quand le delai se compte ; une obligation sans date butoir ne tombe dans aucun mois.
    let butoir = o.butoir;
    let mut r = Echeance::nouveau(GenreDEcheance::Obligation, cle, titre, question, butoir.map(|j| j.mois_civil()));
    r.etat = if o.etat == crate::obligations::EtatDObligation::EnRetard { EnRetard } else { AFaire };
    r.etape = o.etat.nom().to_string();
    if let Some(j) = butoir {
        r.le(&j);
    }
    r.detail = match o.delai.as_deref() {
        Some(delai) if !delai.is_empty() => format!("Délai : {}. {}", delai, o.phrase),
        _ => o.phrase.clone(),
    };
    r.obligation = Some(o.clone());
    r.objet = Some(objet);
    r
}

// ------------------------------------------------------------- les DPAE


/// Les DPAE a envoyer : celles des engagements valides (`etapes::Moteur::embauches_a_declarer`), a toute distance de
/// l'embauche, celles en retard comprises. Une par jour d'embauche, avec la charge de
/// son depot si elle est testee -- le bouton ouvre alors directement ce depot. Se tait
/// des que la DPAE reelle est au journal.
fn rappels_dpae(st: &Etat) -> R<Vec<Echeance>> {
    let aujourdhui = st.aujourdhui;
    let mut jours: BTreeMap<(Mois, Jour), Vec<crate::etapes::DpaeDue>> = BTreeMap::new();
    for e in crate::etapes::Moteur::nouveau(st.ctx).embauches_a_declarer(None)? {
        jours.entry((e.mois, e.debut)).or_default().push(e);
    }

    let mut out = Vec::new();
    for ((mois, jour), groupe) in jours {
        let retard = jour < aujourdhui;
        let noms = groupe
            .iter()
            .map(|e| match &e.spectacle {
                Some(s) => format!("{} ({})", e.nom, s),
                None => e.nom.clone(),
            })
            .collect::<Vec<_>>()
            .join(", ");
        let quand = if retard {
            format!("L'embauche a commencé le {} : la DPAE est EN RETARD, à envoyer tout de suite.", jour.jj_mm_aaaa())
        } else if jour == aujourdhui {
            "L'embauche est aujourd'hui : la DPAE doit partir avant la première heure de travail.".to_string()
        } else {
            let reste = jour.moins(&aujourdhui);
            format!(
                "Elle doit partir avant la première heure de travail — il reste {} jour{}.",
                reste,
                if reste == 1 { "" } else { "s" }
            )
        };
        let mut r = Echeance::nouveau(
            GenreDEcheance::Dpae,
            CleDEcheance::Dpae(jour),
            format!("DPAE à envoyer — embauche du {}", jour.jj_mm_aaaa()),
            format!("Pas de trace d'envoi pour : {}.", noms),
            Some(jour.mois_civil()),
        );
        r.mois = Some(mois);
        r.le(&jour);
        r.quand_dit = format!("avant la première heure de travail, le {}", jour.jj_mm_aaaa());
        r.detail = quand;
        r.etat = if retard { EnRetard } else { AFaire };
        r.engagements = groupe.len() as i64;
        out.push(r);
    }
    Ok(out)
}

/// LES ACCES AUX ORGANISMES (les mots de passe et les comptes du coffre, `coffre::coffre_etat_de_la_fiche`) : aucun ne bloque.
/// On configure, on saisit, on fabrique la DPAE et la DSN sans eux ; ils servent a envoyer depuis le logiciel, et chacun
/// est un choix. « Pour demarrer » propose ceux qui ne sont pas poses, avec ce qu'ils permettent (`sinon` : ce qui se
/// fait sans eux) ; une compagnie dont rien ne part (envois simules, ou la demo : `bridage::branchement`) n'en propose
/// aucun et le dit, une fois.
fn acces_proposes(ctx: &Contexte) -> socle::echec::R<(Vec<Manque>, Option<(Pose, Pastille)>)> {
    use envois::branchement::Branchement;
    let (texte, mots) = match compagnie::bridage::branchement(&ctx.racine)? {
        Branchement::Monde => ("", ""),
        Branchement::Simulateur => ("Les envois de cette compagnie sont simulés : rien ne part, ses mots de passe et ses comptes ne se demandent pas.", "simulé"),
        Branchement::Message(_) => ("Dans la démo, rien ne part de cette compagnie : ses mots de passe et ses comptes ne se demandent pas.", "démo"),
    };
    if !mots.is_empty() {
        let sans_objet = Pose { cle: "acces".into(), libelle: "Les accès aux organismes".into(), texte: texte.into(), ou: OU_DES_ACCES.into(), onglet: "coffre".into(), champ: None };
        return Ok((Vec::new(), Some((sans_objet, Pastille::de(socle::ecran::EtatDEcran::Information, mots)))));
    }
    let etat = super::coffre::coffre_etat_de_la_fiche(&ctx.racine, &ctx.entreprise)?;
    let mut out = Vec::new();
    for s in etat.services {
        if (s.pose || s.environnement) && s.identifiant.manque.is_empty() {
            continue;
        }
        out.push(acces_propose(format!("acces.{}", s.service), s.libelle.to_string(), format!("{} depuis le logiciel", s.sert_a), "vous le faites vous-même, hors du logiciel"));
    }
    // Le Drive de la sauvegarde : se lie au meme endroit (« Lier le Drive »), sans reseau pour le savoir.
    if super::coffre::compte_autorise(&ctx.racine).is_none() {
        out.push(acces_propose("acces.google".into(), "Google Drive".into(), "sauvegarder la compagnie dans votre Drive".into(), "rien ne se sauvegarde hors de ce poste"));
    }
    Ok((out, None))
}

/// UN ACCES PROPOSE, une ligne de « Pour demarrer » (`declarations::demarrage::route`, qui y ajoute sa phrase) : ce qu'il
/// permet (`utile`), ce qui se fait sans lui (`sinon`), ou il se pose (l'onglet du coffre, aucune case).
fn acces_propose(cle: String, libelle: String, utile: String, sinon: &str) -> Manque {
    Manque::utile(cle, libelle, utile, sinon, (OU_DES_ACCES, "coffre"), None, String::new())
}

/// Une pose de « Pour demarrer » et sa pastille.
#[derive(Debug, Serialize)]
struct PoseVue {
    #[serde(flatten)]
    pose: Pose,
    pastille: Pastille,
}

/// `pour-demarrer` (`declarations::demarrage::PourDemarrer`), chaque pose avec sa pastille.
#[derive(Debug, Serialize)]
struct PourDemarrerVu {
    phrase: String,
    phrases: Vec<String>,
    titre: String,
    note: String,
    manques: Vec<Manque>,
    poses: Vec<PoseVue>,
}

const OU_DES_ACCES: &str = "Configuration › Mots de passe";

#[cfg(test)]
mod tests {
    use super::*;

    /// Les echeances de l'annee que le moteur nomme : la cloture, la revue des spectacles et le taux AT/MP sont des
    /// echeances de l'echeancier livre ; l'export comptable n'en est pas une (deux echeances porteraient sa cle).
    #[test]
    fn les_echeances_du_moteur_sont_celles_de_l_echeancier() -> R<()> {
        let base = echeancier_base()?;
        let livree = |c: &CleDAnnuelle| base.annuelles.iter().any(|e| e.cle == *c);
        assert!([CLOTURE, REVUE_SPECTACLES, AT_MP].iter().all(livree));
        assert!(!livree(&EXPORT_COMPTA));
        Ok(())
    }
}
