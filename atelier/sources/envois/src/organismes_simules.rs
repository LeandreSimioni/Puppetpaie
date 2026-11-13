//! CE QUE LES ORGANISMES REPONDENT A UNE COMPAGNIE FICTIVE, au-dela de l'accuse et du bilan conformes : l'effectif que
//! l'Urssaf calcule depuis les DSN deposees (compte rendu de nature 118), le taux de prelevement a la source que la DGFiP
//! communique (nature 94), et le bilan non conforme d'une DSN (nature 11, rapport `BAN`). Chaque piece est fabriquee dans la
//! forme ou l'organisme la publie, archivee dans `retours/` comme une vraie, et lue par le meme lecteur.
//!
//! LE MONDE N'EST PAS LA CONFIGURATION DE LA COMPAGNIE. Ce que la DGFiP sait d'un salarie (son taux) et l'anomalie que
//! net-entreprises relevera, une compagnie ne l'ecrit nulle part : c'est le scenario de la fictive qui l'annonce, a sa
//! date (`annoncer`), et le simulateur le garde de son cote, dans `organismes-simules/`, a cote de `courriels-simules/`.
//! Rien de ce dossier n'est lu par le calcul, ni par une route. Un salarie dont le scenario n'annonce aucun taux en recoit
//! un de simulation (`taux_de_simulation`) : la DGFiP repond a chaque declaration, comme la vraie.

use droit::fiche::Lue as _;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use droit::contexte::Contexte;
use droit::nature_declaration::NatureDeLaDeclaration;
use socle::date::Jour;
use socle::echec::{erreur, Classe, R};
use socle::num::Decimal;

use crate::journal::Ligne;
use declarations::dsn_lue::DsnLue;
use socle::mois::Mois;
use socle::paris::Moment;
use crate::branchement::Branchement;

const DOSSIER: &str = "organismes-simules";
const FICHIER: &str = "annonces.json";

/// Le compte rendu de l'Urssaf qui porte les effectifs (table des types de parametres NEOReS).
pub const NATURE_EFFECTIFS: &str = "118";

fn chemin(racine: &Path) -> PathBuf {
    racine.join(DOSSIER).join(FICHIER)
}

// ------------------------------------------------------------------ ce que le scenario annonce

/// CE QUE LE SCENARIO ANNONCE AUX ORGANISMES, une sorte par annonce : `{"taux_pas": …}`, `{"taux_atmp": …}` ou
/// `{"refus": …}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Annonce {
    TauxPas(TauxPasAnnonce),
    TauxAtmp(TauxAtmpAnnonce),
    Refus(RefusAnnonce),
}

/// La DGFiP connait ce taux pour ce salarie (la cle de sa fiche).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TauxPasAnnonce {
    pub salarie: String,
    pub taux: Decimal,
}

/// La CARSAT notifie ce taux a l'etablissement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TauxAtmpAnnonce {
    pub code_risque: String,
    pub taux: Decimal,
    pub date_effet: Jour,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bureau: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_ctn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section: Option<String>,
}

/// net-entreprises refusera la prochaine DSN reelle (du mois dit, s'il l'est) par cette anomalie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RefusAnnonce {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rubrique: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mois: Option<Mois>,
}

/// UNE ANNONCE GARDEE : son jour, ce qu'elle annonce (une seule de ses trois cases), et, une fois servie, le flux du
/// depot qu'un refus a vise (`servi_par`) ou de la DSN qui a porte la notification AT/MP (`rendue`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AnnonceGardee {
    le: Jour,
    #[serde(skip_serializing_if = "Option::is_none")]
    taux_pas: Option<TauxPasAnnonce>,
    #[serde(skip_serializing_if = "Option::is_none")]
    taux_atmp: Option<TauxAtmpAnnonce>,
    #[serde(skip_serializing_if = "Option::is_none")]
    refus: Option<RefusAnnonce>,
    #[serde(skip_serializing_if = "Option::is_none")]
    servi_par: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rendue: Option<String>,
}


/// LA MEMOIRE DU SIMULATEUR (`organismes-simules/annonces.json`) : les annonces du scenario, et les annees dont
/// l'effectif est deja rendu. Aucun fichier : rien n'est encore annonce ni rendu.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Memoire {
    annonces: Vec<AnnonceGardee>,
    effectifs_rendus: Vec<String>,
}

fn charger(racine: &Path) -> R<Memoire> {
    let p = chemin(racine);
    if !socle::fichiers::existe(&p) {
        return Ok(Memoire::default());
    }
    Ok(socle::forme::lire_fichier(&p, "La mémoire des organismes simulés ne se lit pas.")?)
}

fn ecrire(racine: &Path, m: &Memoire) -> R<()> {
    socle::ecriture::ecrire_json(&chemin(racine), m, 2).map_err(|m| m.reclasser(Classe::Disque))
}

/// Un nombre a deux decimales ecrites, le demi au pair.
fn deux_decimales(x: Decimal) -> Decimal {
    let mut q = x.round_dp_with_strategy(2, rust_decimal::RoundingStrategy::MidpointNearestEven);
    q.rescale(2);
    q
}

/// Un texte facultatif d'une annonce : vide, il n'est pas dit.
fn dit(v: Option<String>) -> Option<String> {
    v.filter(|t| !t.is_empty())
}

/// Ce que le scenario annonce aux organismes de la compagnie fictive, a la date ou il le joue (le jour de l'horloge
/// figee). Trois annonces :
/// - `{"taux_pas": {"salarie": "<cle de la fiche>", "taux": "7.50"}}` : la DGFiP connait ce taux pour ce salarie a
///   partir de ce jour ; elle le communique en retour de chaque DSN mensuelle reelle, et de chaque demande de taux
///   (signalement d'amorcage), qui le declare ensuite. Un salarie que le scenario n'annonce pas recoit un taux de
///   simulation (`taux_de_simulation`) ;
/// - `{"taux_atmp": {"code_risque": "923AD", "taux": "1.60", "date_effet": "AAAA-MM-JJ", "bureau": "B"?, "code_ctn": ..?,
///   "section": ..?}}` : la CARSAT notifie ce taux a l'etablissement ; la notification (nature 34) suit une fois la
///   prochaine DSN mensuelle reelle deposee a partir de ce jour (`crm_atmp`) ;
/// - `{"refus": {"code": "...", "message": "...", "rubrique": "S21.G00.xx.xxx"?, "mois": "AAAA-MM"?}}` : la prochaine DSN
///   reelle deposee a partir de ce jour (du mois dit, s'il l'est) recoit un bilan non conforme portant cette anomalie.
/// Hors d'une compagnie fictive, refus : le monde ne s'annonce pas.
pub fn annoncer(racine: &Path, branchement: Branchement, annonce: &Annonce) -> R<()> {
    if !branchement.joue() {
        return Err(erreur(Classe::Refuse, "Seuls les organismes d'une compagnie fictive s'annoncent : ceux d'une vraie repondent d'eux-memes."));
    }
    let mut gardee = AnnonceGardee {
        le: socle::temps::secondes().a_la_seconde().jour_de_paris(),
        taux_pas: None,
        taux_atmp: None,
        refus: None,
        servi_par: None,
        rendue: None,
    };
    match annonce.clone() {
        Annonce::TauxPas(t) => {
            let n = t.taux;
            if t.salarie.is_empty() || !(Decimal::ZERO..=Decimal::ONE_HUNDRED).contains(&n) {
                return Err(socle::echec::cle("Le taux de prélèvement annoncé dit le salarié et un taux entre 0 et 100.", "taux_pas").reclasser(Classe::Illisible));
            }
            gardee.taux_pas = Some(TauxPasAnnonce { salarie: t.salarie, taux: deux_decimales(n) });
        }
        Annonce::TauxAtmp(t) => {
            let code = t.code_risque.replace(['.', ' '], "").to_uppercase();
            let n = t.taux;
            if code.len() != 5 || !(Decimal::ZERO..Decimal::from(20)).contains(&n) || n.is_zero() {
                return Err(socle::echec::cle("Le taux AT/MP annoncé dit un code risque de 5 caractères, un taux en pourcentage et une date d'effet AAAA-MM-JJ.", "taux_atmp").reclasser(Classe::Illisible));
            }
            gardee.taux_atmp = Some(TauxAtmpAnnonce {
                code_risque: code,
                taux: deux_decimales(n),
                date_effet: t.date_effet,
                bureau: dit(t.bureau),
                code_ctn: dit(t.code_ctn),
                section: dit(t.section),
            });
        }
        Annonce::Refus(r) => {
            if r.code.is_empty() || r.message.is_empty() {
                return Err(erreur(Classe::Illisible, "refus : le code et le message de l'anomalie sont attendus."));
            }
            gardee.refus = Some(RefusAnnonce { code: r.code, message: r.message, rubrique: dit(r.rubrique), mois: r.mois });
        }
    }
    let mut m = charger(racine)?;
    m.annonces.push(gardee);
    ecrire(racine, &m)
}

// ------------------------------------------------------------------ l'effectif

/// L'effectif d'un mois, d'apres les DSN de ce mois (une par etablissement), comme R. 130-1 du code de la securite
/// sociale le compte (`atelier/reference/sources/code-de-la-securite-sociale/articles-20261029.md`, « L'effectif au sens
/// de la securite sociale ») : chaque personne titulaire d'un contrat compte a due proportion des jours du mois ou elle
/// est employee (II, quatrieme alinea) ; a temps plein, entierement ; a temps partiel (S21.G00.40.014 = 20), au rapport
/// de sa duree contractuelle a la duree de reference (dernier alinea de L. 1111-2 du code du travail : la quotite du
/// contrat, S21.G00.40.013, sur celle de la categorie, S21.G00.40.012) ; un CDD qui remplace un salarie absent
/// (motif de recours 01, S21.G00.40.021) n'est pas compte (II, premier alinea, qui renvoie a la deuxieme phrase du 2° de
/// L. 1111-2). Une personne (son NIR, sinon son NTT) compte au plus une fois par jour, au plus fort de ses contrats de ce
/// jour. Un contrat sans fin court jusqu'au dernier jour du mois.
fn effectif_du_mois(an: i32, mois: u32, dsn: &[DsnLue]) -> R<Decimal> {
    let n = socle::date::jours_du_mois(an, mois);
    let premier = Jour::nouveau(an, mois, 1).expect("mois");
    let dernier = Jour::nouveau(an, mois, n).expect("mois");
    let mut par_jour: BTreeMap<String, Vec<Decimal>> = BTreeMap::new();
    for d in dsn {
        for k in &d.contrats {
            let ind = &d.individus[k.individu];
            let qui = match (&ind.nir, &ind.ntt) {
                (Some(nir), _) => nir.clone(),
                (None, Some(ntt)) => format!("NTT {}", ntt),
                (None, None) => return Err(socle::echec::cle("Un salarié de la DSN n'a ni NIR ni NTT.", "S21.G00.30.020")),
            };
            let jours = par_jour.entry(qui).or_insert_with(|| vec![Decimal::ZERO; n as usize]);
            if k.motif_recours == Some(droit::natures::MotifDeRecours::Remplacement) {
                continue;
            }
            let Some(debut) = k.debut else { continue };
            let fin = match k.fin_declaree.or(k.fin) {
                Some(f) => f,
                None => dernier,
            };
            if debut > dernier || fin < premier {
                continue;
            }
            let poids = if k.modalite == Some(droit::natures::ModaliteDuTempsDeTravail::TempsPartiel) {
                let (Some(reference), Some(quotite)) = (k.quotite_reference, k.quotite) else {
                    return Err(socle::echec::cle("Un contrat à temps partiel de la DSN ne dit pas sa quotité.", "S21.G00.40.012, S21.G00.40.013"));
                };
                if reference > Decimal::ZERO {
                    socle::num::div(quotite, reference).min(Decimal::ONE)
                } else {
                    Decimal::ONE
                }
            } else {
                Decimal::ONE
            };
            let (a, b) = (if debut < premier { 1 } else { debut.jour }, if fin > dernier { n } else { fin.jour });
            for j in a..=b {
                let p = &mut jours[(j - 1) as usize];
                *p = (*p).max(poids);
            }
        }
    }
    Ok(par_jour.values().map(|jours| socle::num::div(jours.iter().sum::<Decimal>(), Decimal::from(n))).sum())
}

/// L'effectif salarie annuel de `an` : la moyenne des effectifs des mois de l'annee, les mois sans salarie ecartes, le
/// resultat tronque au centieme (L. 130-1, I, et R. 130-1, I, du code de la securite sociale, memes releves). `None` quand
/// aucun mois n'a de salarie.
fn effectif_annuel(an: i32, dsn_par_mois: &BTreeMap<u32, Vec<DsnLue>>) -> R<Option<Decimal>> {
    let mut mois: Vec<Decimal> = Vec::new();
    for (m, d) in dsn_par_mois {
        let e = effectif_du_mois(an, *m, d)?;
        if e > Decimal::ZERO {
            mois.push(e);
        }
    }
    if mois.is_empty() {
        return Ok(None);
    }
    let moyenne = socle::num::div(mois.iter().sum::<Decimal>(), Decimal::from(mois.len() as i64));
    // Le milliardieme ajoute avant de tronquer : un quotient qui tombe juste sur un centieme (un tiers fois trois)
    // s'ecrit en decimal une unite du dernier chiffre en dessous, et ne doit pas perdre ce centieme.
    Ok(Some((moyenne + Decimal::new(1, 9)).round_dp_with_strategy(2, rust_decimal::RoundingStrategy::ToNegativeInfinity)))
}

// ------------------------------------------------------------------ ce que le simulateur rend

/// Une piece fabriquee : le retour tel que la liste des retours le decrit, et ses octets.
pub struct Piece {
    pub retour: crate::retours::Annonce,
    pub contenu: Vec<u8>,
}

/// UN RETOUR TEL QUE LA LISTE DES RETOURS LE DECRIT : son flux, sa nature, sa publication, son statut (`fictive`), son
/// libelle, et pour un compte rendu du PAS, le mois de la declaration (`production`).
fn retour(nature: &str, flux: &str, publication: Moment, libelle: &str, production: Option<String>) -> crate::retours::Annonce {
    crate::retours::Annonce {
        publication: Some(publication),
        production,
        ops: None,
        delegataire: None,
        nature_retour: nature.to_string(),
        statut: Some("fictive".to_string()),
        id: None,
        url: None,
        libelle: Some(libelle.to_string()),
        flux: Some(flux.to_string()),
    }
}

fn echappe(t: &str) -> String {
    t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Le texte d'un element XML : la valeur, echappee, ou rien.
fn element(v: Option<&str>) -> String {
    v.into_iter().map(echappe).collect()
}

/// Le mois de la ligne : une DSN du journal le dit toujours.
fn mois_de(l: &Ligne) -> R<Mois> {
    l.mois().ok_or_else(|| socle::echec::cle("Une DSN du journal ne dit pas son mois.", "mois"))
}

/// La DSN de la ligne du journal, relue la ou le depot l'a prise (`sorties/<mois>/<fichier>`) ; un signalement est range
/// sous le mois de son fait. `None` : le fichier n'est plus la.
fn dsn_deposee(racine: &Path, l: &Ligne) -> R<Option<DsnLue>> {
    let Some(fichier) = l.fichier() else { return Ok(None) };
    let chemin = racine.join("sorties").join(mois_de(l)?.to_string()).join(fichier);
    let octets = match std::fs::read(&chemin) {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(socle::echec::du_disque(&chemin, &e)),
    };
    declarations::dsn_lue::lire(&octets, Some(fichier)).map(Some)
}

fn reelle(l: &Ligne) -> bool {
    l.reel()
}

/// Le refus annonce qui attend ce depot : le premier non servi, annonce au plus tard le jour du depot, du mois du depot
/// s'il en dit un. Il est servi par ce depot (son flux).
fn refus_pour(memoire: &mut Memoire, l: &Ligne, flux: &str) -> R<Option<RefusAnnonce>> {
    if !reelle(l) {
        return Ok(None);
    }
    let depot = l.quand().jour();
    let mois = mois_de(l)?;
    for a in memoire.annonces.iter_mut() {
        let Some(r) = a.refus.clone() else { continue };
        if a.servi_par.is_some() {
            continue;
        }
        if a.le <= depot && r.mois.is_none_or(|m| m == mois) {
            a.servi_par = Some(flux.to_string());
            return Ok(Some(r));
        }
    }
    Ok(None)
}

/// L'accuse de reception (nature 10, rapport `AEE`) et le bilan non conforme (nature 11, rapport `BAN`) d'une DSN que
/// net-entreprises refuse, dans le schema des bilans (`atelier/reference/sources/gip-mds-api-dsn/
/// gip-mds-specifications-schema-bilans-vt2.2-20201118.txt` : `envoi`, `idflux`, `essai_reel`, `envoi_etat` ;
/// `declaration`, sa `declaration_identification`, son bilan `etat` et `mois`, ses anomalies `code`, `categorie`,
/// `message`, `identifiant` de la rubrique).
fn bilans_refuses(flux: &str, publication: Moment, l: &Ligne, nature: NatureDeLaDeclaration, refus: &RefusAnnonce) -> R<Vec<Piece>> {
    let mois = mois_de(l)?;
    let mois_dsn = format!("01{:02}{}", mois.numero(), mois.annee());
    let envoi = format!("<envoi><idflux>{}</idflux><essai_reel>02</essai_reel><envoi_etat>OK</envoi_etat></envoi>", echappe(flux));
    let rapport = |type_rapport: &str, corps: &str| {
        format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?><ns2:rapport xmlns:ns2=\"http://www.gip-mds.fr/\" type=\"{}\">{}</ns2:rapport>", type_rapport, corps)
            .into_bytes()
    };
    let ban = format!(
        "{}<declaration><declaration_identification><rang>1</rang><nature>{}</nature></declaration_identification>\
         <declaration_bilan><etat>KO</etat><mois>{}</mois><declaration_anomalie><code>{}</code><categorie>Bloquante</categorie>\
         <message>{}</message><identifiant>{}</identifiant></declaration_anomalie></declaration_bilan></declaration>",
        envoi,
        nature,
        mois_dsn,
        echappe(&refus.code),
        echappe(&refus.message),
        element(refus.rubrique.as_deref())
    );
    Ok(vec![
        Piece { retour: retour("10", flux, publication, "Accuse de reception (compagnie fictive)", None), contenu: rapport("AEE", &envoi) },
        Piece { retour: retour("11", flux, publication, "Bilan d'anomalies (compagnie fictive)", None), contenu: rapport("BAN", &ban) },
    ])
}

/// LE TAUX DE SIMULATION d'un salarie dont le scenario n'annonce aucun taux : la DGFiP simulee le connait comme la vraie
/// connait celui de tout contribuable identifie. Il ne vient d'aucun texte ni d'aucun avis d'imposition : c'est un
/// nombre tire du NIR (FNV-1a), stable d'un compte rendu a l'autre, de 0,00 % a 12,00 % par dixieme.
fn taux_de_simulation(nir: &str) -> Decimal {
    let h = nir.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3));
    deux_decimales(Decimal::new((h % 121) as i64, 1))
}

/// Le compte rendu DGFiP de nature 94 d'une DSN mensuelle ou d'un signalement d'amorcage reels : le taux est « mis à
/// disposition du collecteur par le biais d'un compte-rendu établi par l'administration fiscale en retour de chaque
/// déclaration sociale nominative », et qui « verse[…] pour la première fois un revenu » peut le demander « par
/// anticipation » (BOI-IR-PAS-30-10-20, § 20, `atelier/reference/sources/bofip/BOI-IR-PAS-30-10-20-20181203.html`).
/// Pour chaque individu qu'elle declare : son NIR, son matricule et son taux (la derniere annonce `taux_pas` faite au
/// plus tard le jour du depot, sinon `taux_de_simulation`), dans la forme du CRM nominatif recu
/// (`envois/tests/envois/retours.rs`, `CRM_REEL` : `rapport/@identifiant`, `salarie`, `NIR`, `matricule`,
/// `taux_imposition_PAS`). `None` quand la declaration ne porte aucun salarie de la compagnie.
fn crm_pas(ctx: &Contexte, memoire: &Memoire, l: &Ligne, d: &DsnLue, flux: &str, publication: Moment) -> R<Option<Piece>> {
    let depot = l.quand().jour();
    let mut taux: BTreeMap<String, Decimal> = BTreeMap::new();
    for a in &memoire.annonces {
        let Some(t) = &a.taux_pas else { continue };
        if a.le <= depot {
            taux.insert(t.salarie.clone(), t.taux);
        }
    }
    let mut salaries = String::new();
    let mut vus = BTreeSet::new();
    for (ind, nir) in d.individus.iter().filter_map(|i| i.nir.clone().map(|n| (i, n))) {
        let Some((cle, _)) = ctx.salaries.iter().find(|(_, s)| s.nir.portee() == Some(nir.as_str())) else { continue };
        if !vus.insert(nir.clone()) {
            continue;
        }
        // Le taux annonce ; sans annonce, celui de la simulation.
        let t = match taux.get(cle.as_str()) {
            Some(t) => *t,
            None => taux_de_simulation(&nir),
        };
        salaries.push_str(&format!(
            "<salarie><NIR>{}</NIR><matricule>{}</matricule><taux_imposition_PAS>{}</taux_imposition_PAS></salarie>",
            echappe(&nir),
            element(ind.code_salarie.as_deref()),
            t
        ));
    }
    if salaries.is_empty() {
        return Ok(None);
    }
    let mut o = [0u8; 4];
    socle::temps::aleas(&mut o).map_err(|e| e.reformuler(|m| format!("pas d'alea du systeme : {}", m)).reclasser(Classe::Disque))?;
    let identifiant = 100_000_000 + u32::from_le_bytes(o) % 900_000_000;
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ns2:rapport xmlns:ns2=\"http://www.gip-mds.fr/\" identifiant=\"{}\" type=\"CRM nominatif\" \
         profil=\"DSN\" version=\"v02r01\"><envoi><envoi_identification><idflux>{}</idflux></envoi_identification></envoi><declaration>\
         <declaration_bilan><etat>OK</etat>{}</declaration_bilan></declaration></ns2:rapport>",
        identifiant,
        echappe(flux),
        salaries
    );
    // Le mois de la declaration (`AAAAMM`), meme pour un signalement date du jour de son fait.
    let production = Some(mois_de(l)?.to_string().replace('-', ""));
    let r = retour(crate::retours::NATURE_TAUX_PAS, flux, publication, "Compte-rendu metier DGFiP, taux du PAS (compagnie fictive)", production);
    Ok(Some(Piece { retour: r, contenu: xml.into_bytes() }))
}

/// La notification du taux AT/MP de nature 34 (la CNAM) d'une DSN mensuelle reelle : chaque annonce `taux_atmp` faite au
/// plus tard le jour du depot et pas encore rendue, rattachee a cette declaration (« la dernière déclaration mensuelle
/// disponible », guide de l'API DSN v6.8, §5.26.1), dans la forme du schema des bilans : `declaration_identification`
/// (`nature`, `type`, `mois`, `SIREN`, `nic_affectation`), puis `declaration_bilan` et un `taux_AT` par annonce
/// (`code_ctn`, `section`, `code_risque`, `temoin_bureau`, `taux`, `date_effet`, `date_calcul` ;
/// `atelier/reference/sources/gip-mds-api-dsn/`, LISEZ-MOI, « La notification du taux AT/MP »). Le `type` du rapport
/// n'est ecrit par aucun texte depose : la piece n'en porte pas. `None` quand la CARSAT n'a rien a notifier.
fn crm_atmp(memoire: &mut Memoire, l: &Ligne, d: &DsnLue, flux: &str, publication: Moment) -> R<Option<Piece>> {
    let depot = l.quand().jour();
    let mois = element(d.mois.map(|m| m.premier_jour().jjmmaaaa()).as_deref());
    let calcul = socle::temps::secondes().jour_de_paris().aaaammjj();
    // Un element facultatif ne s'ecrit que s'il est dit.
    let facultatif = |v: &Option<String>, balise: &str| v.iter().map(|x| format!("<{}>{}</{}>", balise, echappe(x), balise)).collect::<String>();
    let mut taux = String::new();
    for a in memoire.annonces.iter_mut() {
        let Some(t) = a.taux_atmp.clone() else { continue };
        if a.rendue.is_some() || a.le > depot {
            continue;
        }
        taux.push_str(&format!(
            "<taux_AT>{}{}<code_risque>{}</code_risque>{}<taux>{}</taux><date_effet>{}</date_effet><date_calcul>{}</date_calcul></taux_AT>",
            facultatif(&t.code_ctn, "code_ctn"),
            facultatif(&t.section, "section"),
            echappe(&t.code_risque),
            facultatif(&t.bureau, "temoin_bureau"),
            t.taux,
            t.date_effet.aaaammjj(),
            calcul
        ));
        a.rendue = Some(flux.to_string());
    }
    if taux.is_empty() {
        return Ok(None);
    }
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><ns2:rapport xmlns:ns2=\"http://www.gip-mds.fr/\" profil=\"DSN\" version=\"v02r02\">\
         <envoi><envoi_identification><idflux>{}</idflux></envoi_identification></envoi><declaration><declaration_identification>\
         <rang>1</rang><nature>{}</nature><type>{}</type><mois>{}</mois><SIREN>{}</SIREN><nic_affectation>{}</nic_affectation>\
         </declaration_identification><declaration_bilan><etat>OK</etat>{}</declaration_bilan></declaration></ns2:rapport>",
        echappe(flux),
        element(d.nature_declaration.map(NatureDeLaDeclaration::code)),
        element(d.type_declaration.map(droit::type_declaration::TypeDeLaDeclaration::code)),
        mois,
        element(d.entreprise.siren.as_deref()),
        element(d.etablissement()),
        taux
    );
    let r = retour(crate::retours::NATURE_TAUX_ATMP, flux, publication, "Compte-rendu metier taux AT/MP, notification de la Carsat (compagnie fictive)", None);
    Ok(Some(Piece { retour: r, contenu: xml.into_bytes() }))
}

/// Les DSN mensuelles reelles de l'annee `an` que l'Urssaf a recues et acceptees, par mois : pour chaque mois et chaque
/// etablissement, la derniere deposee ; une DSN dont le bilan est non conforme n'en est pas une, ni une ligne qui ne dit
/// pas sa nature.
fn dsn_de_l_annee(racine: &Path, lignes: &[Ligne], an: i32) -> R<BTreeMap<u32, Vec<DsnLue>>> {
    let mut dernieres: BTreeMap<(Mois, Option<&str>), &Ligne> = BTreeMap::new();
    for l in lignes {
        if l.acte() != crate::journal::Acte::Dsn || !reelle(l) || !l.accepte() || l.nature_dsn() != Some(NatureDeLaDeclaration::Mensuelle) {
            continue;
        }
        let Some(mois) = l.mois().filter(|m| m.annee() == an) else { continue };
        dernieres.insert((mois, l.etablissement()), l);
    }
    let mut out: BTreeMap<u32, Vec<DsnLue>> = BTreeMap::new();
    for ((mois, _), l) in dernieres {
        if l.retours_organisme().iter().any(|r| r.ko()) {
            continue;
        }
        if let Some(d) = dsn_deposee(racine, l)? {
            out.entry(mois.numero()).or_default().push(d);
        }
    }
    Ok(out)
}

/// Le compte rendu NEOReS de nature 118 de l'Urssaf : l'effectif moyen annuel de l'annee `an` (type de parametre A01,
/// `atelier/reference/sources/neores/gip-mds-neores-table-types-de-parametres-20230703.csv`), dans le profil « parametre
/// utile en paie » du cahier technique NEOReS 2026.1.2 (`atelier/reference/sources/neores/`, p. 27 : `RO.B001`, `RO.B002`,
/// `RO.B004` de type 04, `RO.B008` ; dates AAAA-MM-JJ, date de calcul AAAA-MM-JJThh:mm:ss). La date d'effet et la date de
/// fin bornent l'annee mesuree, comme `releve_retours::reconstruire_effectifs` les lit (inference : le cahier ne dit pas
/// quelle annee les dates portent ; c'est la forme des 118 recus que le lecteur range).
fn crm_effectif(an: i32, valeur: Decimal, flux: &str, publication: Moment) -> Piece {
    let calcule_le = socle::paris::ecrit::seconde::texte(socle::temps::secondes());
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><RO><RO_R001>P26V01</RO_R001><RO_B000><RO_B000_R001>{}</RO_B000_R001></RO_B000>\
         <RO_B001><RO_B001_R001>04</RO_B001_R001><RO_B002><RO_B002_R002>{}</RO_B002_R002><RO_B002_R003>Effectifs</RO_B002_R003>\
         <RO_B002_R005>URSSAF</RO_B002_R005></RO_B002><RO_B004><RO_B004_R003>04</RO_B004_R003><RO_B008><RO_B008_R001>A01</RO_B008_R001>\
         <RO_B008_R002>{}</RO_B008_R002><RO_B008_R003>{}-01-01</RO_B008_R003><RO_B008_R004>{}-12-31</RO_B008_R004>\
         <RO_B008_R006>{}</RO_B008_R006></RO_B008></RO_B004></RO_B001></RO>",
        echappe(flux),
        NATURE_EFFECTIFS,
        deux_decimales(valeur),
        an,
        an,
        calcule_le
    );
    Piece { retour: retour(NATURE_EFFECTIFS, flux, publication, "Effectifs (compagnie fictive)", None), contenu: xml.into_bytes() }
}

/// Ce que les organismes rendent a la DSN que porte la ligne `l` du journal (deja acceptee au transport), au-dela de
/// l'accuse et du bilan conformes : `Err(pieces)` quand net-entreprises la refuse (ses deux bilans, rien d'autre), sinon
/// `Ok(pieces)` a ajouter (le taux du PAS, a une DSN mensuelle comme a un signalement d'amorcage ; l'effectif de l'annee
/// precedente a la premiere DSN mensuelle d'une annee). Une DSN du journal dit sa nature.
pub fn reponses_dsn(ctx: &Contexte, memoire: &mut Memoire, lignes: &[Ligne], l: &Ligne, flux: &str, publication: Moment) -> R<Result<Vec<Piece>, Vec<Piece>>> {
    let nature = l.nature_dsn().ok_or_else(|| socle::echec::cle("Une DSN du journal ne dit pas sa nature.", "nature"))?;
    if let Some(r) = refus_pour(memoire, l, flux)? {
        return Ok(Err(bilans_refuses(flux, publication, l, nature, &r)?));
    }
    let mut pieces = Vec::new();
    if !reelle(l) {
        return Ok(Ok(pieces));
    }
    // La demande de taux par anticipation (BOI-IR-PAS-30-10-20, § 20) : le compte rendu du taux, rien d'autre.
    if nature == declarations::amorcage::NATURE {
        if let Some(d) = dsn_deposee(&ctx.racine, l)? {
            pieces.extend(crm_pas(ctx, memoire, l, &d, flux, publication)?);
        }
        return Ok(Ok(pieces));
    }
    if nature != NatureDeLaDeclaration::Mensuelle {
        return Ok(Ok(pieces));
    }
    if let Some(d) = dsn_deposee(&ctx.racine, l)? {
        pieces.extend(crm_pas(ctx, memoire, l, &d, flux, publication)?);
        pieces.extend(crm_atmp(memoire, l, &d, flux, publication)?);
    }
    if let Some(an) = l.mois().map(Mois::annee) {
        let precedente = (an - 1).to_string();
        if !memoire.effectifs_rendus.contains(&precedente) {
            if let Some(e) = effectif_annuel(an - 1, &dsn_de_l_annee(&ctx.racine, lignes, an - 1)?)? {
                pieces.push(crm_effectif(an - 1, e, flux, publication));
                memoire.effectifs_rendus.push(precedente);
            }
        }
    }
    Ok(Ok(pieces))
}

/// La memoire du simulateur, lue pour une releve jouee ; `garder` la reecrit si elle a change.
pub fn memoire(racine: &Path) -> R<Memoire> {
    charger(racine)
}

pub fn garder(racine: &Path, avant: &Memoire, apres: &Memoire) -> R<()> {
    if avant != apres {
        ecrire(racine, apres)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une DSN reduite a ce que le decompte lit, lue : un individu et ses contrats `(debut, fin, modalite, reference, quotite,
    /// motif de recours)`, dates JJMMAAAA.
    fn dsn(individus: &[(&str, &[(&str, &str, &str, &str, &str, &str)])]) -> DsnLue {
        let mut t = String::from("S20.G00.05.001,'01'\n");
        for (nir, contrats) in individus {
            t.push_str(&format!("S21.G00.30.001,'{}'\n", nir));
            for (debut, fin, modalite, reference, quotite, motif) in contrats.iter() {
                t.push_str(&format!("S21.G00.40.001,'{}'\n", debut));
                if !fin.is_empty() {
                    t.push_str(&format!("S21.G00.40.010,'{}'\n", fin));
                }
                t.push_str(&format!("S21.G00.40.012,'{}'\nS21.G00.40.013,'{}'\nS21.G00.40.014,'{}'\n", reference, quotite, modalite));
                if !motif.is_empty() {
                    t.push_str(&format!("S21.G00.40.021,'{}'\n", motif));
                }
            }
        }
        declarations::dsn_lue::lire(t.as_bytes(), None).unwrap()
    }

    /// R. 130-1 du code de la securite sociale (`atelier/reference/sources/code-de-la-securite-sociale/articles-20261029.md`) :
    /// chacun a due proportion des jours du mois ou il est employe (II), un temps partiel au rapport de ses horaires a la
    /// duree de reference (dernier alinea de L. 1111-2), le CDD de remplacement ecarte (2° de L. 1111-2), les mois sans
    /// salarie hors de la moyenne et le resultat tronque au centieme (I).
    #[test]
    fn l_effectif_annuel_se_compte_comme_r_130_1_le_dit() {
        let mut par_mois: BTreeMap<u32, Vec<DsnLue>> = BTreeMap::new();
        // Janvier (31 jours) : un CDI a temps plein tout le mois (1), un mi-temps (75,84 h sur 151,67 : 0,5) du 16 au 31
        // (16 jours : 0,258...), un CDDU de deux jours (2/31), un CDD de remplacement (ecarte).
        par_mois.insert(
            1,
            vec![dsn(&[
                ("1800101999991", &[("01012020", "", "10", "151.67", "151.67", "")]),
                ("2800101999992", &[("16012025", "", "20", "151.67", "75.84", "")]),
                ("1800101999993", &[("20012025", "21012025", "99", "2.00", "2.00", "05")]),
                ("2800101999994", &[("01012025", "31012025", "10", "151.67", "151.67", "01")]),
            ])],
        );
        // Fevrier (28 jours) : le seul CDI ; mars a decembre sans salarie, hors de la moyenne.
        par_mois.insert(2, vec![dsn(&[("1800101999991", &[("01012020", "", "10", "151.67", "151.67", "")])])]);
        // Janvier : 1 + 0,5 x 16/31 + 2/31 ; la moyenne avec fevrier (1), tronquee au centieme : 1,16.
        assert_eq!(effectif_annuel(2025, &par_mois).unwrap(), Some(Decimal::new(116, 2)));
        assert_eq!(effectif_annuel(2025, &BTreeMap::new()).unwrap(), None);
    }

    /// Le compte rendu 118 que le simulateur rend se lit par le lecteur des vrais (`releve_retours::reconstruire_effectifs`,
    /// cahier technique NEOReS 2026.1.2) : `config/effectifs.json` porte l'effectif A01 de l'annee mesuree, a la place de celui
    /// que la compagnie avait dit.
    #[test]
    fn le_118_rendu_s_ecrit_dans_les_effectifs_de_la_compagnie() {
        let racine = std::env::temp_dir().join(format!("envois-118-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&racine);
        std::fs::create_dir_all(racine.join(crate::retours::RETOURS)).unwrap();
        std::fs::create_dir_all(racine.join(socle::fichiers::CONFIG)).unwrap();
        // Ce que la compagnie a dit : l'annee que le 118 mesure le remplace, l'autre reste.
        let j = |a, m, d| socle::date::Jour::nouveau(a, m, d).unwrap();
        assert!(crate::releve_retours::poser_effectif(&racine, 2026, Decimal::from(3), j(2027, 1, 5)).unwrap());
        assert!(crate::releve_retours::poser_effectif(&racine, 2025, Decimal::from(2), j(2027, 1, 5)).unwrap());
        let p = crm_effectif(2026, Decimal::new(1142, 2),"FICTIF-000000000001", Moment::a_paris(j(2027, 2, 15), 10, 0, 0).unwrap());
        let chemin = crate::releve_retours::archiver_retour(&racine, "", &p.retour, &p.contenu, Some("application/xml"), "20270215-100000").unwrap();
        assert!(chemin.file_name().unwrap().to_string_lossy().contains("_nature118_"));
        crate::releve_retours::reconstruire_effectifs(&racine).unwrap();
        let fiche: droit::fiche::ChampsEntreprise = socle::forme::lire_texte("{}", "la fiche").unwrap();
        let e = droit::contexte::effectif_de(&racine, &fiche).unwrap();
        assert_eq!(e.annees.get(&2026), Some(&socle::num::Decimal::new(1142, 2)));
        assert_eq!(e.annees.get(&2025), Some(&socle::num::Decimal::from(2)));
        assert!(!crate::releve_retours::poser_effectif(&racine, 2026, Decimal::from(3), j(2027, 3, 1)).unwrap());
        let _ = std::fs::remove_dir_all(&racine);
    }
}
