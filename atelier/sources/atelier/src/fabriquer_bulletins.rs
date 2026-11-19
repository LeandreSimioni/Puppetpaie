//! `moteur-atelier fabriquer-bulletins <description.json> <dossier> [--gabarit coolpaie|spaiectacle]` : les
//! bulletins FICTIFS d'un logiciel precedent, un PDF par bulletin, pour qu'une compagnie fictive entre dans PuppetPaie
//! « D'un logiciel précédent » en important ses bulletins par la page.
//!
//! La page se dessine par la fabrique des tests (`bulletins_fictifs.rs`, a cote : un seul code), a la maniere du
//! logiciel choisi ; elle se relit par `bulletin_lu` comme un vrai bulletin. Ici, seulement ce qu'un
//! ancien logiciel aurait calcule et imprime, a partir d'une description courte (le format : `atelier/outils/banc/
//! LISEZ-MOI.md`, « Une compagnie qui vient d'un logiciel précédent ») :
//!
//! - le brut, somme des lignes soumises (quantite x taux, au centime) ;
//! - des cotisations VRAISEMBLABLES (`COTISATIONS` : des taux d'un faux bulletin, ni relevés ni recalcules par le
//!   moteur -- l'import ne recalcule rien, le bulletin fait foi) ; elles sont COHERENTES, comme le lecteur les controle :
//!   net avant impot = brut - cotisations salariales + lignes non soumises ; net a payer = net avant impot - impot ;
//!   quand la description porte les cotisations d'une paie (`cotisations`, `net_imposable`, `pas`), ce sont elles,
//!   imprimees telles quelles : les bulletins disent alors ce que ce moteur calcule de la meme paie, et une compagnie
//!   qui les importe ne s'ecarte de rien ;
//! - `moteur-atelier remplir-bulletins <description.json> <compagnie>` les y ecrit (`remplir`) : la paie que ce moteur
//!   calcule de chaque bulletin que la compagnie a importe (`engagement::ecarts::calculee`, la saisie que la
//!   comparaison de l'import refait), et l'impot preleve au taux du bulletin
//!   (`engagement::etats::prelevement_au_taux_imprime`) ; aucun calcul ici ;
//! - le net imposable (le net avant impot hors non soumis, plus la CSG/CRDS non deductible), le PAS au taux donne ;
//! - les cumuls de l'annee par salarie (cachets, heures, jours, brut, plafond, cout employeur, net imposable, PAS), dans
//!   l'ordre du temps, tels que `bulletin_lu::compteurs` les confronte aux lignes : ils s'accordent.
//!
//! RIEN DE REEL (regle 9 a) : l'identite vient de la description, et le NIR (cle juste) comme le Siret (cle de Luhn)
//! sont verifies. Une description qui ferait une page que l'import ne reprendrait pas entiere est refusee, avec sa
//! raison : un libelle de ligne que le gabarit ne connait pas ou qui n'a pas de rubrique sure (la page n'aurait pas de
//! contrat), une ligne non soumise sur CoolPaie (son gabarit n'en a pas), des dates hors de la periode, une page qui
//! deborderait.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use serde::{Deserialize, Serialize};
use socle::forme::Nombre;

use super::bulletins_fictifs::{m, page_a, page_b, pdf_octets, Identite, RangA, RangB, A, B};
use engagement::ligne::QuantiteDeLigne;
use fabrique_pdf::forme::Mesure;
use import::bulletin_lu::{jetons, Table};
use socle::date::{Jour, Periode};
use socle::echec::{refus, R};
use socle::grandeurs::{Cachets, Heures, HeuresParUnite, Jours, Montant, MontantUnitaire, Taux};
// Un montant tel que le bulletin l'imprime, « 1 234,56 » : l'ecriture du socle.
use socle::texte::chiffres_en_euros as fr;

pub const USAGE: &str = "usage : moteur-atelier fabriquer-bulletins <description.json> <dossier> [--gabarit coolpaie|spaiectacle]";

pub const USAGE_REMPLIR: &str = "usage : moteur-atelier remplir-bulletins <description.json> <compagnie qui a importé ses bulletins>";

// --------------------------------------------------------------------------------------------- la ligne de commande

pub fn commande(args: &[socle::ligne::Argument]) -> std::process::ExitCode {
    use std::process::ExitCode;
    let args: &[String] = &socle::ligne::textes(args);
    let mut libres = Vec::new();
    let mut gabarit = None;
    let mut k = 0;
    while k < args.len() {
        if args[k] == "--gabarit" {
            let g = match args.get(k + 1).map(String::as_str) {
                Some("coolpaie") => Gabarit::Coolpaie,
                Some("spaiectacle") => Gabarit::Spaiectacle,
                _ => {
                    eprintln!("{}", USAGE);
                    return ExitCode::from(2);
                }
            };
            gabarit = Some(g);
            k += 2;
        } else {
            libres.push(args[k].clone());
            k += 1;
        }
    }
    let [description, dossier] = libres.as_slice() else {
        eprintln!("{}", USAGE);
        return ExitCode::from(2);
    };
    let r = socle::forme::lire_fichier::<DescriptionLue>(Path::new(description), "La description des bulletins ne se lit pas.")
        .and_then(|d| fabriquer(d, gabarit, Path::new(dossier)));
    match r {
        Ok(faits) => {
            for f in &faits {
                dire!("{}", f);
            }
            dire!("{} bulletin(s) fabrique(s) dans {}", faits.len(), dossier);
            ExitCode::SUCCESS
        }
        Err(m) => {
            eprintln!("{}", m.en_lignes());
            ExitCode::FAILURE
        }
    }
}

/// `moteur-atelier remplir-bulletins <description.json> <compagnie>` : la description reecrite, la paie de chacun de ses
/// bulletins remplie (`remplir`). Rien ne s'ecrit si un bulletin se refuse.
pub fn commande_remplir(args: &[socle::ligne::Argument]) -> std::process::ExitCode {
    use std::process::ExitCode;
    let args: &[String] = &socle::ligne::textes(args);
    let [description, compagnie] = args else {
        eprintln!("{}", USAGE_REMPLIR);
        return ExitCode::from(2);
    };
    let chemin = Path::new(description);
    let r = socle::forme::lire_fichier::<DescriptionLue>(chemin, "La description des bulletins ne se lit pas.")
        .and_then(|d| remplir(d, Path::new(compagnie)))
        .and_then(|d| socle::ecriture::ecrire_json(chemin, &d, 1).map(|()| d.bulletins.len()));
    match r {
        Ok(n) => {
            dire!("{} bulletin(s) rempli(s) dans {}", n, description);
            ExitCode::SUCCESS
        }
        Err(m) => {
            eprintln!("{}", m.en_lignes());
            ExitCode::FAILURE
        }
    }
}

// --------------------------------------------------------------------------------------------- les taux du faux logiciel

#[derive(Clone, Copy, PartialEq)]
enum Base {
    Brut,
    Plafonnee,
    Csg,
}

/// Les cotisations d'un FAUX bulletin : (cle, base, taux employeur et salarie de l'intermittent, puis du permanent).
/// Des taux vraisemblables d'un logiciel de paie, ecrits en milliemes de point (7000 : 7,000 %) ; `None` : pas de part.
/// Ils ne fondent aucune regle (le moteur ne s'en sert pas, l'import ne les recalcule pas) : ils donnent des montants
/// coherents a relire.
const COTISATIONS: [(&str, Base, Option<Taux>, Option<Taux>, Option<Taux>, Option<Taux>); 13] = [
    ("maladie", Base::Brut, Some(t(7000)), None, Some(t(7000)), None),
    ("prevoyance", Base::Brut, Some(t(1040)), Some(t(120)), Some(t(800)), Some(t(800))),
    ("atmp", Base::Brut, Some(t(1050)), None, Some(t(1050)), None),
    ("vieillesse_plafonnee", Base::Plafonnee, Some(t(8550)), Some(t(6900)), Some(t(8550)), Some(t(6900))),
    ("vieillesse_deplafonnee", Base::Brut, Some(t(2020)), Some(t(400)), Some(t(2020)), Some(t(400))),
    ("complementaire", Base::Plafonnee, Some(t(6010)), Some(t(4010)), Some(t(6010)), Some(t(4010))),
    ("famille", Base::Brut, Some(t(3450)), None, Some(t(3450)), None),
    ("chomage", Base::Brut, Some(t(9050)), Some(t(2400)), Some(t(4300)), None),
    ("autres", Base::Brut, Some(t(2836)), None, Some(t(2836)), None),
    ("conges_spectacles", Base::Brut, Some(t(15500)), None, None, None),
    ("conventionnelles", Base::Brut, Some(t(300)), None, Some(t(300)), None),
    ("csg_deductible", Base::Csg, None, Some(t(6800)), None, Some(t(6800))),
    ("csg_non_deductible", Base::Csg, None, Some(t(2900)), None, Some(t(2900))),
];

/// Un taux de `COTISATIONS`, en milliemes de point.
const fn t(milliemes: u32) -> Taux {
    Taux::milliemes_de_point(milliemes)
}

/// L'ASSIETTE DE LA CSG DU FAUX BULLETIN de brut `brut` et de plafond `plafond`, par la regle du moteur, lue au bareme
/// livre de l'annee `annee` : la part du brut sous la limite de l'abattement (`droit::plafond::assiettes_plafonnees`),
/// abattue (`Pile::assiette_csg`). La paie seule (`Plafonnement::ParPaie`) : le moteur ne regularise pas une paie reprise
/// d'un ancien logiciel (`engagement::conges::plafonds_des_mois_anterieurs`), et c'est ainsi qu'il recalcule celle d'un
/// faux bulletin importe pour la comparer. Sans bareme livre de l'annee, refus : le taux et la limite ne s'inventent pas.
fn assiette_csg(brut: Montant, plafond: Montant, annee: socle::mois::Annee) -> R<Montant> {
    let Some(pile) = droit::contexte::socle_livre_lu(annee)? else {
        return Err(refus!(
            "pas de barème livré pour {} (logiciel/donnees/baremes/{}.json) : l'abattement de la CSG et sa limite ne s'y lisent pas",
            annee,
            annee
        ));
    };
    let plafonnees = droit::plafond::assiettes_plafonnees(brut, plafond, pile.limite_abattement_csg.plafonds, &droit::plafond::Plafonnement::ParPaie);
    Ok(pile.assiette_csg(brut, brut, plafonnees.regularisees.abattue_csg).au_centime())
}

/// La table des parametres du BOSS deposee : elle porte le PSS de 2021 a 2026 (codes PSS_M_METROP et PSS_H_METROP),
/// les annees qu'aucun bareme livre n'a (2024).
fn table_boss() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference/boss-table-parametres-2026.csv")
}

/// LE PLAFOND DE LA SECURITE SOCIALE D'UNE ANNEE (mensuel, horaire), d'un texte et non de memoire :
/// le bareme livre du moteur (`logiciel/donnees/baremes/<annee>.json`, `plafond_securite_sociale`, qui cite sa
/// source), sinon la table des parametres du BOSS deposee (`table_boss`), sinon refus -- l'annee manque, on ne
/// l'invente pas.
fn pss(annee: socle::mois::Annee) -> R<droit::plafond::Pss> {
    let entier = droit::plafond::Pss::en_euros_entiers;
    if let Some(p) = droit::contexte::socle_livre_lu(annee)?.and_then(|pile| pile.plafond_securite_sociale) {
        return Ok(entier(p.mensuel, p.horaire));
    }
    let chemin = table_boss();
    let octets = std::fs::read(&chemin).map_err(|e| refus!("{} : {}", chemin.display(), e))?;
    let texte = String::from_utf8_lossy(&octets);
    let effet = format!("01/01/{}", annee);
    fn valeur<G: socle::grandeurs::Grandeur>(texte: &str, chemin: &Path, effet: &str, code: &str) -> R<Option<G>> {
        let Some(c) = texte.lines().map(|l| l.split(';').collect::<Vec<&str>>()).find(|c| c.len() >= 4 && c[0] == code && c[3].trim() == effet) else {
            return Ok(None);
        };
        socle::grandeurs::lu::texte(c[2]).map(Some).ok_or_else(|| refus!("{} : {} au {} : {} ne se lit pas en nombre", chemin.display(), code, effet, c[2]))
    }
    let mensuel = valeur::<Montant>(&texte, &chemin, &effet, "PSS_M_METROP")?;
    let horaire = valeur::<MontantUnitaire>(&texte, &chemin, &effet, "PSS_H_METROP")?;
    match (mensuel, horaire) {
        (Some(m), Some(h)) => Ok(entier(m, h)),
        _ => Err(refus!(
            "pas de plafond de la Sécurité sociale pour {} : ni bareme livré (logiciel/donnees/baremes/{}.json) ni ligne \
             PSS_M_METROP/PSS_H_METROP au 01/01/{} dans {} -- déposez le texte qui le donne",
            annee,
            annee,
            annee,
            chemin.display()
        )),
    }
}

// --------------------------------------------------------------------------------------------- la description lue

struct Salarie {
    code: String,
    nom: String,
    prenom: String,
    nir: String,
    naissance: Jour,
    adresse: [String; 2],
    conges_spectacles: Option<String>,
}

/// Une ligne comptee : sa quantite dans l'unite de sa rubrique, son taux, ce qu'elle paie au centime.
struct Ligne {
    libelle: String,
    quantite: QuantiteDeLigne,
    taux: MontantUnitaire,
    montant: Montant,
    cachets: Cachets,
    heures: Heures,
}

struct Bulletin {
    rang: usize,
    salarie: usize,
    emploi: String,
    statut: String,
    permanent: bool,
    periode: Periode,
    dates: Vec<Periode>,
    objet: Option<(String, String)>,
    analytique: Option<String>,
    lignes: Vec<Ligne>,
    non_soumis: Vec<Ligne>,
    taux_pas: droit::pas::TauxDePas,
    regle_le: Jour,
    /// Ce que la paie a calcule, quand la description le porte : le bulletin l'imprime au lieu de le calculer.
    paie: Option<Paie>,
}

/// Les cotisations, le net imposable et l'impot preleve d'une paie (les `lignes` d'une paie de PuppetPaie, telles quelles).
struct Paie {
    cotisations: Vec<CotisationPayee>,
    net_imposable: Montant,
    pas: Montant,
}

struct CotisationPayee {
    /// La cle de sa famille (`familles` de la table de lecture), et comment sPAIEctacle l'imprime.
    famille: (&'static str, RenduB),
    libelle: String,
    /// Sa base, dans la colonne des montants du bulletin.
    base: Option<Montant>,
    taux_salarie: Option<Taux>,
    montant_salarie: Montant,
    montant_employeur: Montant,
}

// --------------------------------------------------------------------------------------------- la forme de la description

/// LA DESCRIPTION, telle qu'elle s'ecrit (`atelier/outils/banc/LISEZ-MOI.md`, « Une compagnie qui vient d'un logiciel
/// précédent ») : chaque valeur y est dite ; seuls sont facultatifs ce qui peut ne pas exister (le numero Congés
/// Spectacles d'un salarie, l'objet d'un bulletin, sa case « Analytique », ses lignes non soumises, la paie qu'il
/// imprime). Elle se reecrit telle quelle, la paie de chaque bulletin remplie (`remplir`).
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DescriptionLue {
    #[serde(skip_serializing_if = "Option::is_none")]
    gabarit: Option<Gabarit>,
    entreprise: EntrepriseLue,
    salaries: Vec<SalarieLu>,
    bulletins: Vec<BulletinLu>,
}

/// Le logiciel dont le bulletin prend la page.
#[derive(Deserialize, Serialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Gabarit {
    Coolpaie,
    Spaiectacle,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EntrepriseLue {
    raison_sociale: String,
    adresse: String,
    code_postal: String,
    commune: String,
    siret: droit::fiche::Siret,
    naf: String,
    urssaf: String,
    licence: String,
    convention: ConventionLue,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ConventionLue {
    idcc: droit::fiche::Idcc,
    libelle: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SalarieLu {
    code: String,
    nom: String,
    prenom: String,
    nir: String,
    naissance: Jour,
    adresse: String,
    code_postal: String,
    commune: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    conges_spectacles: Option<String>,
}

/// Le regime du salarie : l'intermittent cotise au chomage des annexes 8 et 10 et aux Congés Spectacles.
#[derive(Deserialize, Serialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Regime {
    Intermittent,
    Permanent,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct BulletinLu {
    salarie: String,
    emploi: String,
    statut: String,
    regime: Regime,
    debut: Jour,
    fin: Jour,
    /// Les jours travailles, en paires [debut, fin].
    dates: Vec<[Jour; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    objet: Option<ObjetLu>,
    /// La case « Analytique » de sPAIEctacle (le titre du spectacle, sous son numero d'objet).
    #[serde(skip_serializing_if = "Option::is_none")]
    analytique: Option<String>,
    lignes: Vec<LigneLue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    non_soumis: Option<Vec<LigneLue>>,
    #[serde(deserialize_with = "socle::grandeurs::lu::nombre_ou_texte::deserialize", serialize_with = "socle::grandeurs::ecrit::nombre_json::serialize")]
    taux_pas: Taux,
    regle_le: Jour,
    #[serde(skip_serializing_if = "Option::is_none")]
    cotisations: Option<Vec<CotisationLue>>,
    #[serde(default, with = "socle::grandeurs::ecrit::nombre::option", skip_serializing_if = "Option::is_none")]
    net_imposable: Option<Montant>,
    #[serde(default, with = "socle::grandeurs::ecrit::nombre::option", skip_serializing_if = "Option::is_none")]
    pas: Option<Montant>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ObjetLu {
    numero: String,
    titre: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct LigneLue {
    libelle: String,
    /// Dans l'unite de la rubrique de son libelle : lue a la lecture de la ligne (`QuantiteDeLigne::imprimee`).
    quantite: Nombre,
    #[serde(deserialize_with = "socle::grandeurs::lu::nombre_ou_texte::deserialize", serialize_with = "socle::grandeurs::ecrit::nombre_json::serialize")]
    taux: MontantUnitaire,
}

/// Une cotisation d'une paie de PuppetPaie : une de ses lignes (`engagement::calcul::LigneCotisation`), sans son taux
/// employeur, que le bulletin n'imprime pas.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CotisationLue {
    famille: String,
    libelle: String,
    #[serde(default, with = "socle::grandeurs::ecrit::nombre::option", skip_serializing_if = "Option::is_none")]
    base: Option<Montant>,
    #[serde(default, with = "socle::grandeurs::ecrit::nombre::option", skip_serializing_if = "Option::is_none")]
    taux_sal: Option<Taux>,
    #[serde(with = "socle::grandeurs::ecrit::nombre")]
    montant_sal: Montant,
    #[serde(with = "socle::grandeurs::ecrit::nombre")]
    montant_pat: Montant,
    masquer_base_et_taux: bool,
}

fn verifier_nir(nir: &str, ou: &str) -> R<()> {
    let ok = nir.is_ascii() && matches!(droit::fiche::Nir::et_sa_cle(nir), Some((treize, Some(cle))) if declarations::champs::cle_nir(treize).as_deref() == Some(cle));
    if ok {
        Ok(())
    } else {
        Err(refus!("{} : le NIR {} n'a pas quinze caractères ou sa clé est fausse (97 - NIR mod 97)", ou, nir))
    }
}

/// Un texte coupe en rangs d'au plus `largeur` caracteres, aux espaces.
fn rangs(texte: &str, largeur: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for mot in texte.split_whitespace() {
        match out.last_mut() {
            Some(l) if l.chars().count() + 1 + mot.chars().count() <= largeur => {
                l.push(' ');
                l.push_str(mot);
            }
            _ => out.push(mot.to_string()),
        }
    }
    out
}

/// Une valeur sur au plus deux rangs, sinon le refus qui le dit.
fn deux_rangs(texte: &str, largeur: usize, quoi: &str) -> R<[String; 2]> {
    let mut r = rangs(texte, largeur);
    if r.len() > 2 || r.iter().any(|l| l.chars().count() > largeur) {
        return Err(refus!("{} « {} » ne tient pas sur deux rangs de {} caractères : abrégez-le", quoi, texte, largeur));
    }
    r.resize(2, String::new());
    Ok([r[0].clone(), r[1].clone()])
}

// --------------------------------------------------------------------------------------------- l'ecriture des nombres

/// Une quantite au centieme, « 1 234,50 », comme les cumuls l'impriment : son ecriture au centime
/// (`grandeurs::ecrit::centimes`), a la francaise.
fn fr_quantite<G: socle::grandeurs::Grandeur>(g: G) -> R<String> {
    let t = socle::grandeurs::ecrit::centimes::texte(g).map_err(refus)?;
    Ok(match t.strip_prefix('-') {
        Some(t) => a_la_francaise(t, true),
        None => a_la_francaise(&t, false),
    })
}

/// Un nombre ecrit `1234.56`, sans signe, a la francaise : « 1 234,56 », precede de « - » s'il est `negatif`.
fn a_la_francaise(s: &str, negatif: bool) -> String {
    let (ent, point_dec) = s.split_at(s.len() - 3);
    let dec = &point_dec[1..];
    let mut groupes = Vec::new();
    let c: Vec<char> = ent.chars().collect();
    let mut fin = c.len();
    while fin > 3 {
        groupes.push(c[fin - 3..fin].iter().collect::<String>());
        fin -= 3;
    }
    groupes.push(c[..fin].iter().collect::<String>());
    groupes.reverse();
    format!("{}{},{}", if negatif { "-" } else { "" }, groupes.join(" "), dec)
}

/// Un taux a trois decimales, « 6,900 », ecrit comme le bulletin du logiciel l'ecrit.
use socle::texte::taux_au_millieme as taux3;

fn dmy(j: &Jour) -> String {
    j.jj_mm_aaaa()
}

fn dmy_court(j: &Jour) -> String {
    j.jj_mm_aa()
}

fn pct(base: Montant, taux: Option<Taux>) -> Option<Montant> {
    taux.map(|t| base.fois_taux(t).au_centime())
}

// --------------------------------------------------------------------------------------------- le calcul du faux bulletin

#[derive(Clone)]
struct Cumuls {
    cachets: Cachets,
    heures: Heures,
    jours: Jours,
    brut: Montant,
    plafond: Montant,
    cout: Montant,
    net_imposable: Montant,
    pas: Montant,
}

/// Ce que le faux logiciel a calcule pour un bulletin.
struct Calcule {
    brut: Montant,
    plafond: Montant,
    assiette_csg: Montant,
    /// (cle, base, taux employeur, montant employeur, taux salarie, montant salarie)
    cotisations: Vec<(&'static str, Montant, Option<Taux>, Option<Montant>, Option<Taux>, Option<Montant>)>,
    total_employeur: Montant,
    total_salarie: Montant,
    net_social: Montant,
    net_avant_impot: Montant,
    net_imposable: Montant,
    pas: Montant,
    net_a_payer: Montant,
    cout: Montant,
    cachets: Cachets,
    heures: Heures,
    jours: Jours,
    periode: Cumuls,
    annee: Cumuls,
}

fn calculer(b: &Bulletin, avant: &Cumuls) -> R<Calcule> {
    let brut: Montant = b.lignes.iter().map(|l| l.montant).sum();
    let mut jours_travailles: BTreeSet<Jour> = BTreeSet::new();
    jours_travailles.extend(b.dates.iter().flat_map(|p| p.jours()));
    let jours = Jours::de_dates(jours_travailles.len());
    // LE « PLAFOND URSSAF » IMPRIME, PAR LA REGLE DU MOTEUR (`plafond.rs`, qui cite ses textes) et le PSS d'un texte
    // (`pss`) : le prorata calendaire pour un permanent, les journees de l'artiste engage moins de cinq jours, le
    // prorata des jours remuneres pour les autres intermittents.
    let resoudre = pss;
    let plafond = if b.permanent {
        // Un bulletin du banc ne decrit aucune absence.
        droit::plafond::plafond_permanent(&b.periode, &droit::plafond::JoursSansRemuneration::aucun(), None, &resoudre)
    } else {
        let dates: Vec<Jour> = jours_travailles.iter().copied().collect();
        droit::plafond::plafond_urssaf(&b.periode, jetons(&b.statut).concat() == "artiste", &resoudre, &dates)
    }
    .map_err(|m| m.reformuler(|p| format!("bulletin {} : {}", b.rang, p)))?;
    let assiette_csg = assiette_csg(brut, plafond, b.periode.fin().annee()).map_err(|m| m.reformuler(|p| format!("bulletin {} : {}", b.rang, p)))?;
    let mut cotisations = Vec::new();
    let (mut total_e, mut total_s, mut csg_non_deductible) = (Montant::ZERO, Montant::ZERO, Montant::ZERO);
    for (cle, base, ei, si, ep, sp) in COTISATIONS.iter().copied().filter(|_| b.paie.is_none()) {
        let (te, ts) = if b.permanent { (ep, sp) } else { (ei, si) };
        if te.is_none() && ts.is_none() {
            continue;
        }
        let assiette = match base {
            Base::Brut => brut,
            Base::Plafonnee => brut.min(plafond),
            Base::Csg => assiette_csg,
        };
        let (me, ms) = (pct(assiette, te), pct(assiette, ts));
        total_e += me.into_iter().sum::<Montant>();
        total_s += ms.into_iter().sum::<Montant>();
        if cle == "csg_non_deductible" {
            csg_non_deductible = ms.into_iter().sum();
        }
        cotisations.push((cle, assiette, te, me, ts, ms));
    }
    if let Some(p) = &b.paie {
        total_e = p.cotisations.iter().map(|c| c.montant_employeur).sum();
        total_s = p.cotisations.iter().map(|c| c.montant_salarie).sum();
    }
    let non_soumis: Montant = b.non_soumis.iter().map(|l| l.montant).sum();
    let net_social = brut - total_s;
    let net_avant_impot = net_social + non_soumis;
    let (net_imposable, pas) = match &b.paie {
        Some(p) => (p.net_imposable, p.pas),
        None => {
            let n = net_social + csg_non_deductible;
            (n, b.taux_pas.prelevement(n).au_centime())
        }
    };
    let cout = brut + total_e + non_soumis;
    let cachets: Cachets = b.lignes.iter().map(|l| l.cachets).sum();
    let heures: Heures = b.lignes.iter().map(|l| l.heures).sum();
    let periode = Cumuls { cachets, heures, jours, brut, plafond, cout, net_imposable, pas };
    let annee = Cumuls {
        cachets: avant.cachets + cachets,
        heures: avant.heures + heures,
        jours: avant.jours + jours,
        brut: avant.brut + brut,
        plafond: avant.plafond + plafond,
        cout: avant.cout + cout,
        net_imposable: avant.net_imposable + net_imposable,
        pas: avant.pas + pas,
    };
    Ok(Calcule {
        brut,
        plafond,
        assiette_csg,
        cotisations,
        total_employeur: total_e,
        total_salarie: total_s,
        net_social,
        net_avant_impot,
        net_imposable,
        pas,
        net_a_payer: net_avant_impot - pas,
        cout,
        cachets,
        heures,
        jours,
        periode,
        annee,
    })
}

// --------------------------------------------------------------------------------------------- la lecture de la description

struct Description {
    gabarit: Gabarit,
    entreprise: EntrepriseLue,
    salaries: Vec<Salarie>,
    bulletins: Vec<Bulletin>,
    /// Les heures d'un cachet, le forfait du catalogue livre.
    forfait_cachet: HeuresParUnite,
}

fn lire_description(d: DescriptionLue, gabarit: Option<Gabarit>, racine_catalogue: &Path) -> R<Description> {
    let gabarit = gabarit.or(d.gabarit).ok_or_else(|| refus("le gabarit manque : `gabarit` dans la description, ou --gabarit coolpaie|spaiectacle"))?;
    let g = Table::livree()?;

    let e = d.entreprise;
    if !e.siret.cle_juste() {
        return Err(refus!("entreprise : la clé de Luhn du Siret {} est fausse", e.siret));
    }

    let mut salaries = Vec::new();
    let mut codes: HashMap<String, usize> = HashMap::new();
    for (k, s) in d.salaries.into_iter().enumerate() {
        let ou = format!("salarié {}", k + 1);
        verifier_nir(&s.nir, &ou)?;
        if codes.insert(s.code.clone(), k).is_some() {
            return Err(refus!("{} : le code {} est déjà pris", ou, s.code));
        }
        salaries.push(Salarie {
            code: s.code,
            nom: s.nom,
            prenom: s.prenom,
            nir: s.nir,
            naissance: s.naissance,
            adresse: [s.adresse, format!("{} {}", s.code_postal, s.commune)],
            conges_spectacles: s.conges_spectacles,
        });
    }

    let lire_ligne = |l: &LigneLue, ou: &str, soumise: bool| -> R<Ligne> {
        let libelle = &l.libelle;
        let canon = jetons(libelle);
        let connu = g.lignes().iter().find(|x| jetons(&x.libelle) == canon).ok_or_else(|| {
            let connus: Vec<&str> = g.lignes().iter().map(|x| x.libelle.as_str()).collect();
            refus!("{} : « {} » n'est pas un libellé de la table de lecture ({})", ou, libelle, connus.join(", "))
        })?;
        let code = connu.rubrique.clone().ok_or_else(|| {
            refus!("{} : « {} » n'a pas de rubrique sûre dans la table : la page se lirait sans contrat", ou, connu.libelle)
        })?;
        // Le dossier de fabrication n'a pas de rubriques de la compagnie : le catalogue livre, le meme a toute date.
        let r = droit::catalogue::rubrique(racine_catalogue, &code, &droit::contexte::jour_reel())?;
        if r.soumis != soumise {
            return Err(refus!(
                "{} : « {} » ({}) est une ligne {} : mettez-la dans `{}`",
                ou,
                connu.libelle,
                code,
                if r.soumis { "soumise" } else { "non soumise" },
                if r.soumis { "lignes" } else { "non_soumis" }
            ));
        }
        // La quantite de la ligne dans l'unite de sa rubrique (`QuantiteDeLigne`), comme le lecteur de bulletins la compte
        // (`import::bulletin_lu`) : ses cachets, ou ses heures (le forfait du catalogue, sinon celles que le libelle dit ;
        // une heure compte ses heures). Une ligne en pourcentage n'a pas de quantite : la page n'imprime que des lignes
        // comptees, quantite x taux.
        use QuantiteDeLigne as Q;
        let Some(quantite) = Q::imprimee(r.unite, l.quantite).map_err(|e| refus!("{} : « {} » : {}", ou, connu.libelle, e))? else {
            return Err(refus!("{} : « {} » est une ligne en pourcentage : la page n'imprime que des lignes comptées (quantité × taux)", ou, connu.libelle));
        };
        let (mut cachets, mut heures) = (Cachets::ZERO, Heures::ZERO);
        match (quantite, r.heures_forfait.or(connu.heures)) {
            (Q::Cachets(c), _) => cachets = c,
            (q, Some(u)) => heures = q.fois(u),
            (Q::Heures(h), None) => heures = h,
            (_, None) => {}
        }
        let montant = engagement::ligne::PaiementDeLigne::de(quantite, l.taux).montant().au_centime();
        Ok(Ligne { libelle: connu.libelle.clone(), quantite, taux: l.taux, montant, cachets, heures })
    };

    let mut bulletins = Vec::new();
    for (k, b) in d.bulletins.into_iter().enumerate() {
        let ou = format!("bulletin {}", k + 1);
        let salarie = *codes.get(&b.salarie).ok_or_else(|| refus!("{} : aucun salarié de code {}", ou, b.salarie))?;
        let periode = Periode::du_au(b.debut, b.fin).map_err(|_| refus!("{} : la fin précède le début", ou))?;
        if b.dates.is_empty() {
            return Err(refus!("{} : `dates` : au moins une paire [début, fin]", ou));
        }
        let mut dates = Vec::new();
        for [d0, d1] in b.dates {
            match Periode::du_au(d0, d1) {
                Ok(p) if periode.croise(&p) == Some(p) => dates.push(p),
                _ => return Err(refus!("{} : `dates` : {} - {} hors de la période", ou, d0.iso(), d1.iso())),
            }
        }
        if b.lignes.is_empty() {
            return Err(refus!("{} : `lignes` manque", ou));
        }
        let lignes = b.lignes.iter().map(|l| lire_ligne(l, &ou, true)).collect::<R<Vec<_>>>()?;
        let non_soumis = b.non_soumis.iter().flatten().map(|x| lire_ligne(x, &ou, false)).collect::<R<Vec<_>>>()?;
        if gabarit == Gabarit::Coolpaie && !non_soumis.is_empty() {
            return Err(refus!("{} : le gabarit CoolPaie n'a pas de ligne non soumise", ou));
        }
        if gabarit == Gabarit::Coolpaie && b.analytique.is_some() {
            return Err(refus!("{} : le gabarit CoolPaie n'a pas de case « Analytique »", ou));
        }
        let paie = lire_paie(b.cotisations, b.net_imposable, b.pas, &g, gabarit, &ou)?;
        let Some(taux_pas) = droit::pas::TauxDePas::du_taux(b.taux_pas) else {
            return Err(refus!("{} : `taux_pas` {} n'est pas un taux de PAS (deux chiffres au plus, deux décimales au plus)", ou, socle::grandeurs::ecrit::sans_zero::dit(b.taux_pas)));
        };
        bulletins.push(Bulletin {
            rang: k + 1,
            salarie,
            emploi: b.emploi,
            statut: b.statut,
            permanent: b.regime == Regime::Permanent,
            periode,
            dates,
            objet: b.objet.map(|o| (o.numero, o.titre)),
            analytique: b.analytique,
            lignes,
            non_soumis,
            taux_pas,
            regle_le: b.regle_le,
            paie,
        });
    }
    let forfait_cachet = droit::catalogue::rubrique(racine_catalogue, &droit::catalogue::CACHET, &droit::contexte::jour_reel())?
        .heures_forfait
        .ok_or_else(|| refus("le cachet du catalogue livré ne dit pas ses heures"))?;
    Ok(Description { gabarit, entreprise: e, salaries, bulletins, forfait_cachet })
}

/// Les cotisations d'une paie (`cotisations` : les `lignes` d'une paie de PuppetPaie), son `net_imposable` et son impot
/// preleve (`pas`) : les trois ensemble, ou aucun.
fn lire_paie(liste: Option<Vec<CotisationLue>>, net_imposable: Option<Montant>, pas: Option<Montant>, g: &Table, gabarit: Gabarit, ou: &str) -> R<Option<Paie>> {
    let (liste, net_imposable, pas) = match (liste, net_imposable, pas) {
        (None, None, None) => return Ok(None),
        (Some(l), Some(n), Some(p)) => (l, n, p),
        _ => return Err(refus!("{} : `cotisations`, `net_imposable` et `pas` vont ensemble", ou)),
    };
    if gabarit != Gabarit::Spaiectacle {
        return Err(refus!("{} : les cotisations d'une paie ne s'impriment qu'au gabarit sPAIEctacle", ou));
    }
    if liste.is_empty() {
        return Err(refus!("{} : `cotisations` : une liste non vide", ou));
    }
    let mut cotisations = Vec::new();
    for (k, c) in liste.into_iter().enumerate() {
        let ouc = format!("{} : cotisation {}", ou, k + 1);
        let famille = g
            .famille_nommee(&c.famille)
            .and_then(|f| FAMILLES_B.iter().find(|(x, _)| *x == f.as_str()).copied())
            .ok_or_else(|| refus!("{} : la famille « {} » n'en est pas une que le bulletin imprime dans son total", ouc, c.famille))?;
        let Some(cotisation) = g.cotisation(&c.libelle) else {
            return Err(refus!("{} : « {} » n'est pas une cotisation de la table de lecture : ajoutez-la à `cotisations`", ouc, c.libelle));
        };
        // L'APEC A SON RANG, apres celui de l'assurance chomage dont le bareme la fait famille : « Assurance chômage »,
        // puis « Apec » (arrete du 25/02/2016, art. 1, I, le modele : `atelier/reference/sources/montant-net-social/
        // legi-arrete-20160225-bulletin/LEGIARTI000049854119.xml`).
        let famille = match cotisation {
            engagement::ecarts::RegroupementDeComparaison::Apec => (famille.0, RenduB::Rang("Apec")),
            _ => famille,
        };
        let masquee = c.masquer_base_et_taux;
        cotisations.push(CotisationPayee {
            famille,
            libelle: c.libelle,
            base: if masquee { None } else { c.base },
            taux_salarie: if masquee { None } else { c.taux_sal },
            montant_salarie: c.montant_sal,
            montant_employeur: c.montant_pat,
        });
    }
    Ok(Some(Paie { cotisations, net_imposable, pas }))
}

// --------------------------------------------------------------------------------------------- les pages

fn identite(d: &Description, s: &Salarie) -> R<Identite<String>> {
    let e = &d.entreprise;
    let (idcc, libelle) = (&e.convention.idcc, &e.convention.libelle);
    let convention = if d.gabarit == Gabarit::Coolpaie {
        let [a, b] = deux_rangs(&format!("IDCC n° {} - {}", idcc, libelle), 34, "la convention")?;
        [a, b]
    } else {
        deux_rangs(&format!("{} (IDCC {})", libelle, idcc), 46, "la convention")?
    };
    Ok(Identite {
        entreprise: [e.raison_sociale.clone(), e.adresse.clone(), format!("{} {}", e.code_postal, e.commune)],
        siret: e.siret.to_string(),
        naf: e.naf.clone(),
        urssaf: e.urssaf.clone(),
        convention,
        licence: e.licence.clone(),
        nom: s.nom.clone(),
        prenom: s.prenom.clone(),
        adresse: s.adresse.clone(),
        naissance: dmy(&s.naissance),
        nir: s.nir.clone(),
        code: s.code.clone(),
        conges_spectacles: s.conges_spectacles.clone(),
    })
}

fn libelle_a(cle: &str) -> (Option<&'static str>, Mesure, &'static str, &'static str) {
    // (titre de famille sur son propre rang, x du libelle, libelle, famille)
    match cle {
        "maladie" => (Some("SANTE"), m!("33.9"), "Sécurité sociale - Maladie Maternité Invalidité Décès", SANTE),
        "prevoyance" => (None, m!("33.9"), "Complémentaire Incapacité Invalidité Décès", SANTE),
        "atmp" => (Some("ACCIDENTS DU TRAVAIL - MALADIES"), m!("40"), "PROFESSIONNELLES", ATMP),
        "vieillesse_plafonnee" => (Some("RETRAITE"), m!("33.9"), "Sécurité sociale plafonnée", RETRAITE),
        "vieillesse_deplafonnee" => (None, m!("33.9"), "Sécurité sociale déplafonnée", RETRAITE),
        "complementaire" => (None, m!("33.9"), "Complémentaire Tranche 1", RETRAITE),
        "famille" => (None, m!("33.9"), "FAMILLE", FAMILLE),
        "chomage" => (Some("ASSURANCE CHOMAGE"), m!("33.9"), "Chômage", CHOMAGE),
        "autres" => (Some("AUTRES CONTRIBUTIONS DUES PAR L'EMPLOYEUR"), m!("33.9"), "autres contributions dues par l'employeur", AUTRES),
        "conges_spectacles" => (None, m!("33.9"), "CONGES SPECTACLES", CONGES_SPECTACLES),
        "conventionnelles" => (Some("COTISATIONS STATUTAIRES OU PREVUES PAR LA"), m!("40"), "CONVENTION COLLECTIVE", CONVENTIONNELLES),
        "csg_deductible" => (None, m!("33.9"), "CSG déductible à l'impôt sur le revenu", CSG),
        _ => (None, m!("33.9"), "CSG/CRDS non déductible à l'impôt sur le revenu", CSG),
    }
}

fn libelle_b(cle: &str) -> (Option<&'static str>, Mesure, &'static str, &'static str) {
    match cle {
        "maladie" => (Some("Santé"), m!("74"), "Sécurité Sociale - Maladie Maternité Invalidité Décès", SANTE),
        "prevoyance" => (None, m!("74"), "Prévoyance, Incapacité, Invalidité, Décès, Autres", SANTE),
        "atmp" => (None, m!("39"), "Accidents du travail & maladies professionnelles", ATMP),
        "vieillesse_plafonnee" => (Some("Retraite"), m!("74"), "Sécurité Sociale Vieillesse plafonnée", RETRAITE),
        "vieillesse_deplafonnee" => (None, m!("74"), "Sécurité Sociale Vieillesse déplafonnée", RETRAITE),
        "complementaire" => (None, m!("74"), "Retraite complémentaire, CEG et CET T1", RETRAITE),
        "famille" => (None, m!("39"), "Famille", FAMILLE),
        "chomage" => (None, m!("39"), "Assurance chômage", CHOMAGE),
        "autres" => (None, m!("39"), "Autres charges dues par l\u{2019}employeur", AUTRES),
        "conges_spectacles" => (None, m!("39"), "Congés Spectacles", CONGES_SPECTACLES),
        "conventionnelles" => (None, m!("39"), "Cotisations statutaires ou prévues par la convention collective", CONVENTIONNELLES),
        "csg_deductible" => (None, m!("39"), "CSG déductible de l'impôt sur le revenu", CSG),
        _ => (None, m!("39"), "CSG/CRDS non déductible de l'impôt sur le revenu", CSG),
    }
}

/// Comment sPAIEctacle imprime une famille de cotisations, au modele du bulletin clarifie (arrete du 25 fevrier 2016,
/// art. 1, I : `atelier/reference/sources/montant-net-social/legi-arrete-20160225-bulletin/LEGIARTI000049854119.xml`) :
/// la sante et la retraite, un titre puis leurs lignes ; la CSG, ses lignes ; toute autre famille, un rang qui la nomme ;
/// l'Apec, le sien (`lire_paie`).
#[derive(Clone, Copy, PartialEq)]
enum RenduB {
    Titre(&'static str),
    Lignes,
    Rang(&'static str),
}

/// Chaque famille (`familles` de la table de lecture, ecrite comme le bareme l'ecrit), et comment sPAIEctacle l'imprime.
const FAMILLES_B: [(&str, RenduB); 10] = [
    (SANTE, RenduB::Titre("Santé")),
    (ATMP, RenduB::Rang("Accidents du travail & maladies professionnelles")),
    (RETRAITE, RenduB::Titre("Retraite")),
    (FAMILLE, RenduB::Rang("Famille")),
    (CHOMAGE, RenduB::Rang("Assurance chômage")),
    (AUTRES, RenduB::Rang("Autres charges dues par l\u{2019}employeur")),
    (CONGES_SPECTACLES, RenduB::Rang("Congés Spectacles")),
    (CONVENTIONNELLES, RenduB::Rang("Cotisations statutaires ou prévues par la convention collective")),
    (CSG, RenduB::Lignes),
    (EXONERATIONS, RenduB::Rang("Exonérations et allègements de cotisations")),
];

// Les familles de la table de lecture (`familles` de `bulletins-lecture-base.json`), telles qu'elle les ecrit.
const SANTE: &str = "SANTÉ";
const ATMP: &str = "ACCIDENTS DU TRAVAIL — MALADIES PROFESSIONNELLES";
const RETRAITE: &str = "RETRAITE";
const FAMILLE: &str = "FAMILLE";
const CHOMAGE: &str = "ASSURANCE CHÔMAGE";
const AUTRES: &str = "AUTRES CONTRIBUTIONS DUES PAR L'EMPLOYEUR";
const CONGES_SPECTACLES: &str = "CONGÉS SPECTACLES";
const CONVENTIONNELLES: &str = "COTISATIONS STATUTAIRES OU PRÉVUES PAR LA CONVENTION COLLECTIVE";
const CSG: &str = "CSG / CRDS";
const EXONERATIONS: &str = "EXONÉRATIONS ET ALLÈGEMENTS DE COTISATIONS";

/// Les rangs sPAIEctacle des cotisations d'une paie, famille par famille, dans l'ordre de la paie. Un montant nul ne
/// s'imprime pas ; une ligne sans aucun montant non plus (elle se lirait comme un titre).
fn rangs_de_la_paie(p: &Paie) -> Vec<RangB<String>> {
    let mut out: Vec<RangB<String>> = Vec::new();
    let montant = |x: Montant| if x.est_nul() { String::new() } else { fr(x) };
    let mut k = 0;
    while k < p.cotisations.len() {
        let (famille, rendu) = p.cotisations[k].famille;
        let n = p.cotisations[k..].iter().take_while(|c| c.famille == (famille, rendu)).count();
        let groupe: Vec<&CotisationPayee> = p.cotisations[k..k + n].iter().filter(|c| !(c.montant_salarie.est_nul() && c.montant_employeur.est_nul())).collect();
        k += n;
        if groupe.is_empty() {
            continue;
        }
        let seule = |x: Mesure, c: &CotisationPayee| -> RangB<String> {
            let taux = c.taux_salarie.filter(|_| !c.montant_salarie.est_nul()).map(taux3).into_iter().collect();
            (x, c.libelle.clone(), famille, c.base.map(fr).into_iter().collect(), taux, montant(c.montant_salarie), montant(c.montant_employeur))
        };
        match rendu {
            RenduB::Titre(t) => {
                out.push((m!("39"), t.into(), "", String::new(), String::new(), String::new(), String::new()));
                out.extend(groupe.iter().map(|c| seule(m!("74"), c)));
            }
            RenduB::Lignes => out.extend(groupe.iter().map(|c| seule(m!("39"), c))),
            RenduB::Rang(libelle) if groupe.len() == 1 => {
                let mut r = seule(m!("39"), groupe[0]);
                r.1 = libelle.into();
                out.push(r);
            }
            RenduB::Rang(libelle) => {
                let bases: BTreeSet<Option<Montant>> = groupe.iter().map(|c| c.base).collect();
                let base = if bases.len() == 1 { bases.into_iter().flatten().map(fr).collect() } else { String::new() };
                let s: Montant = groupe.iter().map(|c| c.montant_salarie).sum();
                let e: Montant = groupe.iter().map(|c| c.montant_employeur).sum();
                out.push((m!("39"), libelle.into(), famille, base, String::new(), montant(s), montant(e)));
            }
        }
    }
    out
}

fn dates_a(dates: &[Periode]) -> R<Vec<String>> {
    let morceaux: Vec<String> = dates.iter().map(|p| format!("du {} au {}", dmy(&p.debut()), dmy(&p.fin()))).collect();
    let rangs: Vec<String> = morceaux
        .chunks(2)
        .enumerate()
        .map(|(k, c)| {
            let mut r = c.join(", ");
            if (k + 1) * 2 < morceaux.len() {
                r.push(',');
            }
            r
        })
        .collect();
    // Trois rangs tiennent entre « Dates travaillées » et le tableau.
    if rangs.len() > 3 {
        return Err(refus!("{} intervalles de dates : CoolPaie n'en imprime que six, regroupez-les", morceaux.len()));
    }
    Ok(rangs)
}

fn page_coolpaie(d: &Description, b: &Bulletin, c: &Calcule, numero: String) -> R<Vec<fabrique_pdf::forme::Bloc>> {
    let s = &d.salaries[b.salarie];
    let mut cotisations: Vec<RangA<String>> = Vec::new();
    let vide = String::new;
    for (cle, base, te, me, ts, ms) in &c.cotisations {
        let (titre, x, libelle, famille) = libelle_a(cle);
        if let Some(t) = titre {
            cotisations.push((m!("33.9"), t.into(), "", vide(), vide(), vide(), vide(), vide()));
        }
        let taux = |t: &Option<Taux>| t.map(taux3).into_iter().collect::<String>();
        let montant = |m: &Option<Montant>| m.map(fr).into_iter().collect::<String>();
        cotisations.push((x, libelle.into(), famille, fr(*base), taux(te), montant(me), taux(ts), montant(ms)));
    }
    let cachets = c.periode.cachets > Cachets::ZERO;
    let (unite, p, a) = match cachets {
        true => ("Cachets:", fr_quantite(c.periode.cachets)?, fr_quantite(c.annee.cachets)?),
        false => ("Heures:", fr_quantite(c.periode.heures.au_centieme())?, fr_quantite(c.annee.heures.au_centieme())?),
    };
    let lignes: Vec<(String, String, String, String)> = b.lignes.iter().map(|l| (l.quantite.dit(), l.libelle.clone(), fr(l.taux), fr(l.montant))).collect();
    // La page tient-elle ? Le pied finit a 72 points sous le net a payer, la signature est a 810.
    let bas = m!("308") + (lignes.len() + 1 + cotisations.len()) * m!("11.2") + m!("14.2") + m!("22") + m!("20.3") + m!("11") + m!("11.5") + m!("14") + m!("8.5") + m!("8.5") + m!("13") + m!("72");
    if bas > m!("800") {
        return Err(refus!("bulletin {} : trop de lignes pour une page CoolPaie", b.rang));
    }
    let a_ = A {
        id: identite(d, s)?,
        numero,
        objet: b.objet.clone(),
        emploi: b.emploi.clone(),
        statut: b.statut.clone(),
        periode: format!("du {} au {}", dmy(&b.periode.debut()), dmy(&b.periode.fin())),
        dates: dates_a(&b.dates)?,
        lignes,
        brut: fr(c.brut),
        csg: fr(c.assiette_csg),
        cotisations,
        total: [fr(c.total_employeur), fr(c.total_salarie)],
        net: fr(c.net_avant_impot),
        pas: [
            fr(c.net_imposable),
            socle::grandeurs::ecrit::texte::dit(socle::grandeurs::decimales_ecrites(b.taux_pas.taux(), 3)),
            socle::grandeurs::ecrit::texte::dit(c.pas.au_centime()),
            fr(c.annee.pas),
        ],
        net_imposable: [fr(c.net_imposable), fr(c.annee.net_imposable)],
        net_a_payer: fr(c.net_a_payer),
        regle: dmy(&b.regle_le),
        mode: b.periode.fin().annee() >= socle::annee!("2023"),
        unite: unite.into(),
        cumuls: [
            [p, a],
            [fr(c.brut), fr(c.annee.brut)],
            [fr(c.plafond), fr(c.annee.plafond)],
            [fr(c.cout), fr(c.annee.cout)],
        ],
        total_verse: fr(c.cout),
    };
    Ok(page_a(&a_))
}

fn page_spaiectacle(d: &Description, b: &Bulletin, c: &Calcule) -> R<Vec<fabrique_pdf::forme::Bloc>> {
    let s = &d.salaries[b.salarie];
    let vide = String::new;
    let mut cotisations: Vec<RangB<String>> = b.paie.iter().flat_map(rangs_de_la_paie).collect();
    for (cle, base, _, me, ts, ms) in &c.cotisations {
        let (titre, x, libelle, famille) = libelle_b(cle);
        if let Some(t) = titre {
            cotisations.push((m!("39"), t.into(), "", vide(), vide(), vide(), vide()));
        }
        let montant = |m: &Option<Montant>| m.map(fr).into_iter().collect::<String>();
        cotisations.push((x, libelle.into(), famille, fr(*base), ts.map(taux3).into_iter().collect(), montant(ms), montant(me)));
    }
    let ligne = |l: &Ligne| (l.quantite.dit(), l.libelle.clone(), fr(l.taux), fr(l.montant));
    let lignes: Vec<_> = b.lignes.iter().map(ligne).collect();
    let non_soumis: Vec<_> = b.non_soumis.iter().map(ligne).collect();
    let titres = cotisations.iter().filter(|c| c.0 < m!("50")).count();
    let bas = m!("308.5") + lignes.len() * m!("11") + m!("9") + titres * m!("13") + (cotisations.len() - titres) * m!("10") + m!("13") + m!("18") + non_soumis.len() * m!("18");
    if bas > m!("645") {
        return Err(refus!("bulletin {} : trop de lignes pour une page sPAIEctacle", b.rang));
    }
    let mut cumuls = Vec::new();
    if c.annee.cachets > Cachets::ZERO {
        cumuls.push(("Cachets".to_string(), fr_quantite(c.periode.cachets)?, fr_quantite(c.annee.cachets)?));
    }
    cumuls.push(("Jours".to_string(), fr_quantite(c.periode.jours)?, fr_quantite(c.annee.jours)?));
    if c.heures > Heures::ZERO {
        // sPAIEctacle compte les cachets dans ses heures, au forfait du catalogue (12 h) ; les cumuls de l'annee aussi.
        let h = |x: &Cumuls| (x.heures + x.cachets * d.forfait_cachet).au_centieme();
        cumuls.push(("Heures".to_string(), fr_quantite(h(&c.periode))?, fr_quantite(h(&c.annee))?));
    }
    cumuls.push(("Salaire brut".to_string(), fr(c.brut), fr(c.annee.brut)));
    let (d0, d1) = (&b.periode.debut(), &b.periode.fin());
    let courte = if d0.mois_civil() == d1.mois_civil() {
        format!("du {} au {}", d0.quantieme_ecrit(), d1.j_mm_aa())
    } else {
        format!("du {} au {}", d0.j_mm(), d1.j_mm_aa())
    };
    let mut statut = b.statut.clone();
    if let Some(p) = statut.get(..1) {
        statut = format!("{}{}", p.to_uppercase(), &statut[1..]);
    }
    let p = B {
        id: identite(d, s)?,
        periode: format!("du {} au {}", dmy_court(d0), dmy_court(d1)),
        periode_courte: courte,
        reglement: format!("{} par virement", dmy_court(&b.regle_le)),
        objet: b.objet.iter().map(|o| o.0.as_str()).collect(),
        analytique: b.analytique.iter().map(String::as_str).collect(),
        emploi: b.emploi.clone(),
        statut,
        lignes,
        brut: ("MONTANT BRUT".into(), fr(c.brut)),
        cotisations,
        total: ("TOTAL COTISATIONS ET CONTRIB. OBLIGATOIRES".into(), fr(c.total_salarie), fr(c.total_employeur)),
        apres_total: Vec::new(),
        non_soumis,
        net_social: Some(fr(c.net_social)),
        net_avant_impot: fr(c.net_avant_impot),
        entete_pas: vec![(m!("280.9"), "base"), (m!("339"), "taux personnalisé"), (m!("442.2"), "montant"), (m!("502.7"), "cumul annuel")],
        pas: (
            "IMPOT SUR LE REVENU PRELEVE A LA SOURCE".into(),
            vec![(m!("316.1"), fr(c.net_imposable)), (m!("412.2"), socle::texte::chiffres_de_taux(b.taux_pas.taux())), (m!("487"), fr(c.pas)), (m!("563"), fr(c.annee.pas))],
        ),
        net_imposable: Some((fr(c.net_imposable), fr(c.annee.net_imposable))),
        cumuls,
        net_paye: fr(c.net_a_payer),
        allegement: false,
        total_verse: fr(c.cout),
    };
    Ok(page_b(&p))
}

// --------------------------------------------------------------------------------------------- la fabrique

/// Fabrique les bulletins de la description dans `dossier` (cree au besoin), un PDF par bulletin ; rend une ligne par
/// fichier ecrit. Rien n'est ecrit si un bulletin se refuse.
pub fn fabriquer(d: DescriptionLue, gabarit: Option<Gabarit>, dossier: &Path) -> R<Vec<String>> {
    let desc = lire_description(d, gabarit, dossier)?;
    let mut ordre: Vec<usize> = (0..desc.bulletins.len()).collect();
    ordre.sort_by_key(|&k| (desc.bulletins[k].periode.fin(), desc.bulletins[k].periode.debut(), k));
    // Les cumuls de l'annee de chaque salarie, qui partent de rien a son premier bulletin de l'annee.
    let mut cumuls: HashMap<(usize, socle::mois::Annee), Cumuls> = HashMap::new();
    let mut numeros: HashMap<socle::mois::Annee, usize> = HashMap::new();
    let mut pages = Vec::new();
    for k in ordre {
        let b = &desc.bulletins[k];
        // Le premier bulletin de l'annee part de cumuls nuls.
        let fin = b.periode.fin();
        let annee = cumuls.entry((b.salarie, fin.annee())).or_insert_with(|| {
            let z = Montant::ZERO;
            Cumuls { cachets: Cachets::ZERO, heures: Heures::ZERO, jours: Jours::ZERO, brut: z, plafond: z, cout: z, net_imposable: z, pas: z }
        });
        let c = calculer(b, annee)?;
        *annee = c.annee.clone();
        // CoolPaie numerote ses bulletins dans l'annee : `k/annee`.
        let n = numeros.entry(fin.annee()).or_insert(0);
        *n += 1;
        let numero = format!("{}/{}", n, fin.annee());
        let page = if desc.gabarit == Gabarit::Coolpaie { page_coolpaie(&desc, b, &c, numero)? } else { page_spaiectacle(&desc, b, &c)? };
        let s = &desc.salaries[b.salarie];
        let nom = format!("{}-{}-{:03}.pdf", fin.mois_civil(), s.code, b.rang);
        let ligne = format!(
            "{}  {} {}  du {} au {}  brut {}  net à payer {}  ({} cachet(s), {} h, {} jour(s))",
            nom,
            s.prenom,
            s.nom,
            b.periode.debut().iso(),
            fin.iso(),
            fr(c.brut),
            fr(c.net_a_payer),
            socle::texte::virgule(c.cachets),
            socle::texte::virgule(c.heures),
            socle::texte::virgule(c.jours)
        );
        pages.push((nom, pdf_octets(vec![page])?, ligne));
    }
    std::fs::create_dir_all(dossier).map_err(|e| refus!("{} : {}", dossier.display(), e))?;
    let mut faits = Vec::new();
    for (nom, octets, ligne) in pages {
        let chemin = dossier.join(&nom);
        std::fs::write(&chemin, octets).map_err(|e| refus!("{} : {}", chemin.display(), e))?;
        faits.push(ligne);
    }
    Ok(faits)
}

// --------------------------------------------------------------------------------------------- la paie qu'ils impriment

/// LA DESCRIPTION `d`, LA PAIE DE CHACUN DE SES BULLETINS REMPLIE de ce que ce moteur calcule : `compagnie` a importe
/// ces bulletins (le rejeu de son scenario), sa configuration est complete. Chaque bulletin se retrouve parmi ses paies
/// importees par son salarie (le NIR), son mois et sa periode, que les dates de la paie contiennent ; ses `cotisations` et son `net_imposable` sont ceux de la
/// paie que la comparaison de l'import refait de lui (`engagement::ecarts::calculee`), son `pas` l'impot preleve sur elle
/// au taux qu'il imprime (`engagement::etats::prelevement_au_taux_imprime`). Le reste de la description ne change pas.
/// Un bulletin qui ne se retrouve pas, ou dont le moteur refuse la paie, refuse le tout, en le nommant.
pub fn remplir(mut d: DescriptionLue, compagnie: &Path) -> R<DescriptionLue> {
    // Seul sPAIEctacle imprime les cotisations d'une paie (`lire_paie`).
    if d.gabarit != Some(Gabarit::Spaiectacle) {
        return Err(refus("la description ne dit pas `\"gabarit\": \"spaiectacle\"` : seul ce gabarit imprime les cotisations d'une paie"));
    }
    let importes = engagement::importe::tous(compagnie)?;
    let nirs: HashMap<&str, &str> = d
        .salaries
        .iter()
        .map(|s| match droit::fiche::Nir::et_sa_cle(&s.nir) {
            Some((treize, _)) => Ok((s.code.as_str(), treize)),
            None => Err(refus!("salarié {} : le NIR {} n'a ni treize ni quinze caractères", s.code, s.nir)),
        })
        .collect::<R<_>>()?;
    for (k, b) in d.bulletins.iter_mut().enumerate() {
        let ou = format!("bulletin {} (salarié {}, du {} au {})", k + 1, b.salarie, b.debut.iso(), b.fin.iso());
        let nir = *nirs.get(b.salarie.as_str()).ok_or_else(|| refus!("{} : aucun salarié de ce code", ou))?;
        let mois = b.fin.mois_civil();
        // La paie dont les dates contiennent la periode du bulletin : celles du contrat d'un mois que la DSN declare (un
        // permanent, de son debut et sans fin : `routes::importation::bulletins`), celles du bulletin sinon.
        let periode = socle::date::Periode::du_au(b.debut, b.fin).map_err(|m| m.reformuler(|p| format!("{} : {}", ou, p)))?;
        let siennes: Vec<&engagement::importe::Importe> = importes
            .iter()
            .filter(|i| i.paies().get(&mois).is_some_and(|p| p.nir.as_deref() == Some(nir) && p.periode.and_then(|o| o.croise(&periode)) == Some(periode)))
            .collect();
        let [imp] = siennes.as_slice() else {
            return Err(refus!("{} : {} paie(s) importée(s) de ce salarié, de ce mois et de cette période dans {}, il en faut une", ou, siennes.len(), compagnie.display()));
        };
        let paie = engagement::ecarts::calculee(compagnie, imp, mois).map_err(|m| m.reformuler(|p| format!("{} : ce moteur ne calcule pas sa paie : {}", ou, p)))?;
        let Some(taux) = droit::pas::TauxDePas::du_taux(b.taux_pas) else {
            return Err(refus!("{} : `taux_pas` {} n'est pas un taux de PAS", ou, socle::grandeurs::ecrit::sans_zero::dit(b.taux_pas)));
        };
        b.pas = Some(engagement::etats::prelevement_au_taux_imprime(&paie, taux)?);
        b.net_imposable = Some(paie.net_imposable);
        b.cotisations = Some(
            paie.lignes
                .iter()
                .map(|l| CotisationLue {
                    famille: l.famille.as_str().to_string(),
                    libelle: l.libelle.clone(),
                    base: l.base.montant(),
                    taux_sal: l.taux_sal,
                    montant_sal: l.montant_sal,
                    montant_pat: l.montant_pat,
                    masquer_base_et_taux: l.masquer_base_et_taux,
                })
                .collect(),
        );
    }
    Ok(d)
}
