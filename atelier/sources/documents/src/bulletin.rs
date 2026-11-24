//! La description du bulletin de paie (`fabrique_pdf::forme`) : des blocs que la fabrique met en page. Ce module
//! s'arrete a la description ; l'ecriture du PDF est `fabrique::ecrire`.

use engagement::calcul::Paie;
use engagement::conges::CompteurConges;
use droit::contexte::Contexte;
use droit::fiche::{ChampsEntreprise, Lue, ChampsSalarie, FicheSalarie, ModeleBulletin};
use droit::cumul::{tableau_cumul, Colonne};
use engagement::cumul::Passe;
use engagement::numeros::NumeroDeBulletin;
use socle::date::Jour;
use socle::mois::{Graphie, Mois};
use socle::texte::{chiffres_en_euros, euros};
use socle::echec::{refus, R};
use socle::grandeurs::{Montant, Taux};
use droit::pas::Versement;
use droit::forme_bareme::{self as cot, IdDeCotisation};
use engagement::ecarts::RegroupementDeComparaison as Regroupement;
use fabrique_pdf::forme::{self as f, Bloc, Cellule, Description, Document, Ligne, Mesure, Morceau};
use fabrique_pdf::mesure;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const ROUGE: &str = "#B22222";
pub const ARDOISE: &str = "#2C3E50";
const GRIS: &str = "#555555";
const GRIS_CLAIR: &str = "#777777";
const FOND_FAMILLE: &str = "#f4f0f0";
const FOND_BRUT: &str = "#fbf7f7";
const NBSP: &str = "\u{a0}";
const BLANC: &str = "#FFFFFF";

/// Emplacement du logo sur la page, en points PDF (A4 = 595 x 842).
pub const LOGO_LARGEUR: Mesure = mesure!("74");
pub const LOGO_MARGE_DROITE: Mesure = mesure!("34");
pub const LOGO_MARGE_HAUT: Mesure = mesure!("28");

/// Le pied de chaque page du bulletin : « Édité avec » et le mot Puppetpaie dessiné (`donnees/puppetpaie-mot.svg`),
/// en gris clair, en bas a droite. Le mot a 657,2 sur 195,4 de cadre, sa ligne de base a 84,1 du bas : a 33 points de
/// large, il descend de 4,2 points sous celle du texte.
const PIED_TEXTE: &str = "Édité avec";
const PIED_GRIS: &str = "#C5C9CE";
const PIED_MOT_LARGEUR: Mesure = mesure!("33");
const PIED_MOT_DESCENTE: Mesure = mesure!("4.2");

/// Dans l'ordre de preference : le vectoriel d'abord.
pub const IDENTITE_EXTENSIONS: [&str; 3] = [".svg", ".png", ".jpg"];

// ------------------------------------------------------------ les formats (util)

/// Le taux d'une ligne de remuneration au bulletin : un montant par unite (« 1 234,50 »), ou le pourcentage de sa base
/// qu'une ligne en pourcentage paie (« 10 % »).
fn fmt_taux_de_ligne(t: engagement::ligne::TauxDeLigne) -> String {
    match t {
        // Le montant d'une unite.
        engagement::ligne::TauxDeLigne::ParUnite(u) => chiffres_en_euros(u),
        engagement::ligne::TauxDeLigne::PartDeLaBase(c) => format!("{} %", socle::texte::virgule(c.en_taux())),
    }
}

/// Une ligne de remuneration au bulletin : sa designation, a son taux.
fn ligne_au_bulletin(l: &engagement::calcul::LigneBrut) -> String {
    format!("{} à {}", l.designation(), fmt_taux_de_ligne(l.paiement.taux()))
}


/// Un taux tel que le bulletin l'imprime : 9.1 -> « 9,100 » (`socle::texte::taux_au_millieme`).
use socle::texte::taux_au_millieme as fmt_taux;

/// Un taux de PAS, imprime comme un taux (`fmt_taux`).
fn fmt_taux_de_pas(t: droit::pas::TauxDePas) -> String {
    fmt_taux(t.taux())
}

/// Des jours de conges : au centieme, sans zero inutile, la virgule decimale.
fn jours_de_conges(j: socle::grandeurs::Jours) -> String {
    socle::texte::virgule(j.a_deux_decimales())
}

// ------------------------------------------------------------ les briques

/// La case texte `cle` du spectacle de la paie ; refus si elle manque.
fn du_spectacle<T: AsRef<str>>(sp: Option<&droit::fiche::Spectacle>, case: fn(&droit::fiche::Spectacle) -> &Option<T>, cle: &str) -> R<String> {
    sp.map(case).and_then(Option::as_ref).map(|t| t.as_ref().to_string()).ok_or_else(|| socle::fichiers::manque(cle))
}


/// Un morceau de texte d'un paragraphe ou d'une cellule, droit ; sans couleur, celle de ce qui le porte.
fn m(texte: impl Into<String>, gras: bool, couleur: Option<&str>) -> Morceau {
    Morceau { texte: texte.into(), gras, italique: false, couleur: couleur.map(str::to_string) }
}

fn mt(texte: impl Into<String>) -> Morceau {
    m(texte, false, None)
}

/// Une mesure du bulletin ; nulle, elle ne se dit pas.
fn dite(x: Mesure) -> Option<Mesure> {
    (!x.est_nulle()).then_some(x)
}

/// Un paragraphe du bulletin, à gauche : ses morceaux, sa taille (nulle : celle de la police), sa couleur, l'espace
/// avant et après.
fn para(morceaux: Vec<Morceau>, taille: Mesure, couleur: Option<&str>, avant: Mesure, apres: Mesure) -> Bloc {
    Bloc::Paragraphe(f::Paragraphe {
        morceaux,
        taille: dite(taille),
        couleur: couleur.map(str::to_string),
        alignement: f::Alignement::Gauche,
        majuscules: false,
        espace_avant: avant,
        espace_apres: apres,
        retrait: Mesure::ZERO,
    })
}

/// Une cellule de morceaux, de la couleur dite.
fn cm(morceaux: Vec<Morceau>, couleur: Option<&str>) -> Cellule {
    Cellule::Decrite(f::CelluleDecrite {
        texte: None,
        morceaux: Some(morceaux),
        gras: false,
        couleur: couleur.map(str::to_string),
        alignement: None,
        etendue: 1,
    })
}

/// Une cellule du bulletin : son texte, sur combien de colonnes, en gras ou non, sa couleur.
fn cell(texte: impl Into<String>, etendue: usize, gras: bool, couleur: Option<&str>) -> Cellule {
    Cellule::Decrite(f::CelluleDecrite {
        texte: Some(texte.into()),
        morceaux: None,
        gras,
        couleur: couleur.map(str::to_string),
        alignement: None,
        etendue,
    })
}

fn cel(texte: impl Into<String>, etendue: usize) -> Cellule {
    cell(texte, etendue, false, None)
}

fn s(texte: impl Into<String>) -> Cellule {
    Cellule::Texte(texte.into())
}

/// Une cellule vide : rien ne s'y ecrit.
fn vide() -> Cellule {
    Cellule::Decrite(f::CelluleDecrite { texte: None, morceaux: None, gras: false, couleur: None, alignement: None, etendue: 1 })
}

/// Une cellule qui dit son texte, ou vide.
fn so(texte: Option<String>) -> Cellule {
    match texte {
        Some(t) => s(t),
        None => vide(),
    }
}

/// Ce qu'une ligne dit en plus de ses cellules.
enum Opt {
    Gras,
    Fond(&'static str),
    Couleur(&'static str),
    Taille(Mesure),
    FiletHaut(&'static str, Mesure),
}

/// Une ligne maigre ; ce qu'aucune option ne dit est celui du tableau.
fn ligne(cellules: Vec<Cellule>, options: Vec<Opt>) -> Ligne {
    let mut l = Ligne { cellules, gras: false, fond: None, couleur: None, taille: None, filet_haut: None, filet_bas: None };
    for o in options {
        match o {
            Opt::Gras => l.gras = true,
            Opt::Fond(c) => l.fond = Some(c.to_string()),
            Opt::Couleur(c) => l.couleur = Some(c.to_string()),
            Opt::Taille(t) => l.taille = Some(t),
            Opt::FiletHaut(c, e) => l.filet_haut = Some(filet(c, e)),
        }
    }
    l
}

fn gras() -> Opt {
    Opt::Gras
}

fn filet_haut(couleur: &'static str, epaisseur: Mesure) -> Opt {
    Opt::FiletHaut(couleur, epaisseur)
}

fn filet(couleur: &str, epaisseur: Mesure) -> f::Filet {
    f::Filet { couleur: couleur.to_string(), epaisseur }
}

/// Les cellules des tableaux : 7 pt, 1,4 pt au-dessus et au-dessous, 3 pt de chaque cote.
fn marge_cellule() -> f::MargeCellule {
    f::MargeCellule { haut: mesure!("1.4"), droite: mesure!("3"), bas: mesure!("1.4"), gauche: mesure!("3") }
}

/// Premiere colonne a gauche, les autres a droite, un filet fin sous chaque ligne.
fn tableau(largeurs: &[Mesure], entete: Vec<Ligne>, lignes: Vec<Ligne>, teinte: &str) -> Bloc {
    let colonnes = largeurs
        .iter()
        .enumerate()
        .map(|(i, l)| f::ColonneTableau {
            largeur: *l,
            alignement: if i == 0 { f::Alignement::Gauche } else { f::Alignement::Droite },
        })
        .collect();
    Bloc::Tableau(f::Tableau {
        colonnes,
        entete: Some(entete),
        lignes: Some(lignes),
        taille: Some(mesure!("7")),
        couleur: None,
        interligne: Some(mesure!("1.27")),
        marge_cellule: marge_cellule(),
        filet_lignes: Some(filet(teinte, mesure!("0.3"))),
        filets_cellules: None,
        espace_avant: Mesure::ZERO,
        espace_apres: Mesure::ZERO,
    })
}

/// Une ligne « net », blanc sur rouge, 9,5 pt, en gras.
fn ligne_net(libelle: &str, montant: Montant, colonnes: usize) -> Ligne {
    ligne(
        vec![cel(libelle, colonnes - 1), s(chiffres_en_euros(montant))],
        vec![gras(), Opt::Fond(ROUGE), Opt::Couleur(BLANC), Opt::Taille(mesure!("9.5"))],
    )
}

// ------------------------------------------------------------ ce que le bulletin dit

/// Le SMIC de la reduction
/// generale, celui du CALCUL (gele a 12,02 en 2026), pas le SMIC du jour.
pub fn smic_de_la_reduction(p: &Paie) -> R<String> {
    if let Some(rg) = &p.reduction_generale {
        // Des heures nulles n'ont pas de SMIC horaire.
        if let Some(horaire) = rg.smic_reference.par_heure(rg.heures) {
            // La phrase cite les trois nombres : sa porte.
            return Ok(format!(
                "{} ({} h à {})",
                euros(rg.smic_reference),
                socle::texte::virgule(rg.heures),
                euros(horaire)
            ));
        }
    }
    Ok(format!("{} ({} h à {})", euros(p.plafond), socle::texte::virgule(p.heures), euros(p.smic)))
}

/// (IDCC, libellé) de la convention d'un contrat, pour ce qui s'imprime : le libellé se lit au catalogue des conventions
/// du barème de l'année de `jour` (`droit::bareme::libelle_convention`), la 1734, qui n'a pas de couche de barème, à
/// son titre KALI (`sources.json`, KALICONT000005635286).
/// LA POSITION DU SALARIE DANS LA CLASSIFICATION DE SA CONVENTION, telle que le bulletin l'imprime : « le nom et l'emploi
/// du salarie ainsi que sa position dans la classification conventionnelle qui lui est applicable. La position du
/// salarie est notamment definie par le niveau ou le coefficient hierarchique qui lui est attribue » (art. R. 3243-1, 4°,
/// du code du travail, `atelier/reference/sources/code-du-travail/articles-20261028.md`). La categorie de la convention
/// (`droit::clauses::profil` : la fiche, sinon le metier, jamais devinee) dite par le libelle de la convention, puis le
/// classement que la fiche porte dans la grille (`classement_conventionnel`) ; `None` quand ni l'une ni l'autre ne se
/// lisent.
fn classification(idcc: Option<&droit::fiche::Idcc>, sal: &ChampsSalarie, au_mois: Option<&droit::natures::AuMois>) -> R<Option<String>> {
    let profil = droit::clauses::profil(idcc, sal, au_mois, &Default::default())?;
    // La categorie dite par la convention ; celle que la convention ne nomme pas s'imprime telle que la fiche la dit.
    let categorie = match profil.categorie {
        Some(c) => Some(match droit::clauses::categorie_imprimee(idcc, &c)? {
            Some(dite) => dite,
            None => c.as_str().replace('_', " "),
        }),
        None => None,
    };
    let classement = sal.classement_conventionnel.portee().filter(|t| !t.trim().is_empty()).map(str::to_string);
    Ok(match (categorie, classement) {
        (Some(c), Some(k)) => Some(format!("{}, {}", c, k)),
        (Some(c), None) => Some(c),
        (None, Some(k)) => Some(k),
        (None, None) => None,
    })
}

/// Le nom du salarie tel que le bulletin l'imprime : son nom d'usage avec ses accents (`nom_affichage`, sinon `nom`), en
/// capitales, puis son prenom (`prenom_affichage`, sinon `prenom`). La DSN et la DPAE declarent le nom sans accents ;
/// le bulletin est lu par le salarie, sous son nom.
fn nom_imprime(sal: &ChampsSalarie) -> R<String> {
    let lu = |cases: [Option<&str>; 2]| -> Option<String> {
        cases.into_iter().flatten().find(|t| !t.trim().is_empty()).map(str::to_string)
    };
    let nom = lu([sal.nom_affichage.portee(), sal.nom.portee()]).ok_or_else(|| socle::fichiers::manque("nom"))?;
    let prenom = lu([sal.prenom_affichage.portee(), sal.prenom.portee()]).ok_or_else(|| socle::fichiers::manque("prenom"))?;
    Ok(format!("{} {}", nom.to_uppercase(), prenom))
}

/// L'emploi tel que le bulletin l'imprime : celui que l'engagement choisit, lisible et accorde a la fiche
/// (`emploi_affichage`), sinon celui de la fiche, sinon le libelle que la DSN declare (`emploi`, S21.G00.40.006).
fn emploi_imprime(e: &engagement::Engagement, sal: &ChampsSalarie) -> R<String> {
    let lisible = e.emploi.emploi_affichage.clone().filter(|t| !t.trim().is_empty())
        .or_else(|| sal.emploi_affichage.portee().filter(|t| !t.trim().is_empty()).map(str::to_string));
    match lisible {
        Some(t) => Ok(t),
        None => exige(&sal.emploi, "emploi"),
    }
}

pub fn convention_du_contrat(ent: &ChampsEntreprise, sp: Option<&droit::fiche::Spectacle>, sal: &ChampsSalarie, jour: &Jour) -> R<(droit::fiche::Idcc, String)> {
    let Some(idcc_compagnie) = &ent.idcc else {
        return Err(socle::echec::cle("La fiche entreprise ne dit pas sa convention collective : la choisir dans Configuration › Entreprise.", "idcc"));
    };
    let idcc = engagement::minima::idcc_du_contrat(idcc_compagnie, sp, sal)?;
    let libelle = if idcc == droit::fiche::IDCC_1734 {
        "Artistes-interprètes engagés pour des émissions de télévision".to_string()
    } else {
        droit::bareme::libelle_convention(&idcc, jour.annee())?
    };
    Ok((idcc, libelle))
}

/// LE BULLETIN RECTIFICATIF d'une paie partie puis refaite : il dit le bulletin qu'il rectifie (`rectifie`, son numero)
/// et le mois de la paie (`mois_origine`, en clair). Tant que la DSN du mois se refait, il REMPLACE en entier celui qui
/// est parti : la paie du mois refaite, imprimee comme un bulletin du mois, sous un nouveau numero. Quand elle ne se
/// refait plus, il porte l'ECART avec le bulletin parti, verse avec une paie suivante (`Ecart`).
#[derive(Debug, Clone)]
pub struct Rectificatif {
    pub rectifie: NumeroDeBulletin,
    pub mois_origine: Mois,
    /// `None` : il remplace en entier le bulletin parti.
    pub ecart: Option<Ecart>,
}

/// L'ECART AVEC LE BULLETIN PARTI d'un mois clos : les nets avant et apres, leurs differences, le mois (en clair) de la
/// paie qui le verse, le prelevement a la source de la difference, et ce qui reste a verser (ou a retenir).
#[derive(Debug, Clone)]
pub struct Ecart {
    pub net_avant: Montant,
    pub net_apres: Montant,
    pub difference_net: Montant,
    pub difference_net_imposable: Montant,
    pub mois_versement: Mois,
    /// `None` : un trop-verse sans taux d'origine, dont le prelevement ne se calcule pas.
    pub prelevement: Option<PrelevementDeLEcart>,
    pub a_verser: Montant,
}

/// Le prelevement a la source de la difference : au taux du mois de l'indu (`indu`), ou a celui du versement.
#[derive(Debug, Clone)]
pub struct PrelevementDeLEcart {
    pub indu: bool,
    pub taux: droit::pas::TauxDePas,
    pub montant: Montant,
}

/// Le bas d'un bulletin RECTIFICATIF.
fn rectificatif_lignes(r: &Ecart, colonnes: usize) -> R<Vec<Ligne>> {
    let lignes = [
        ("Net à payer avant impôt du bulletin rectifié", r.net_avant),
        ("Net à payer avant impôt rectifié", r.net_apres),
        ("Différence de net à payer", r.difference_net),
        ("Différence de net imposable", r.difference_net_imposable),
    ];
    let mut out = vec![ligne(
        vec![cel(format!("RÉGULARISATION VERSÉE AVEC LA PAIE {}", socle::texte::de(&r.mois_versement.libelle(Graphie::Accentuee)).to_uppercase()), colonnes)],
        vec![gras(), filet_haut(ROUGE, mesure!("0.6"))],
    )];
    // Les montants s'impriment : leur porte.
    for (libelle, montant) in lignes {
        out.push(ligne(vec![cel(libelle, colonnes - 1), s(chiffres_en_euros(montant))], vec![]));
    }
    let Some(p) = &r.prelevement else {
        out.push(ligne(vec![cel("Impôt prélevé à la source sur la différence", colonnes - 1), s("non calculé")], vec![]));
        return Ok(out);
    };
    let libelle = if p.indu {
        "Régularisation du prélèvement à la source (taux du mois de l'indu)"
    } else {
        "Impôt sur le revenu prélevé à la source sur la différence"
    };
    out.push(ligne(
        vec![cel(format!("{} (taux {} %)", libelle, fmt_taux_de_pas(p.taux)), colonnes - 1), s(chiffres_en_euros(p.montant))],
        vec![],
    ));
    out.push(ligne_net(if r.a_verser >= Montant::ZERO { "NET À VERSER" } else { "NET À RETENIR" }, r.a_verser.abs(), colonnes));
    Ok(out)
}

/// L'impot a la source, et le net paye. La ligne du prelevement suit le modele de l'arrete du
/// 25/02/2016 (art. 1, I : « IMPÔT SUR LE REVENU PRÉLEVÉ À LA SOURCE », colonnes Base, Taux, Salarié) : son assiette
/// (le montant soumis, apres l'abattement d'un contrat court ; C. trav. R. 3243-1, 9°), son taux, son montant. Le
/// « Cumul annuel » du modele (art. 1, V) n'est pas imprime : le moteur ne le calcule pas.
fn lignes_pas_tableau(p: &Paie, pas: &Versement, colonnes: usize) -> R<Vec<Ligne>> {
    let taux = fmt_taux_de_pas(pas.taux);
    Ok(vec![
        ligne(
            vec![s("IMPÔT SUR LE REVENU PRÉLEVÉ À LA SOURCE"), s(chiffres_en_euros(pas.montant_soumis)), s(taux), s(chiffres_en_euros(pas.prelevement))],
            vec![],
        ),
        ligne_net("MONTANT NET À PAYER (en Euros)", p.net_a_payer - pas.prelevement, colonnes),
    ])
}

/// Une colonne de libelles, une par fenetre.
fn tableau_des_cumuls(colonnes: &[Colonne]) -> Bloc {
    let n = colonnes.len();
    let mut cellules = vec![cell("CUMULS", 1, true, Some(ARDOISE))];
    for c in colonnes {
        cellules.push(cm(vec![m(c.titre.clone(), true, None), mt("\n"), mt(c.sous_titre.clone())], None));
    }
    let entete = vec![ligne(
        cellules,
        vec![Opt::Couleur(ROUGE), Opt::Taille(mesure!("6.8")), Opt::Fond(FOND_FAMILLE), filet_haut(ROUGE, mesure!("0.8"))],
    )];
    let enseigne = colonnes.iter().any(|c| !c.enseignement.est_nul());
    // (libelle, valeur de chaque colonne, suffixe) ; une valeur que la colonne n'a pas laisse sa case vide.
    type Rang = (&'static str, fn(&Colonne) -> Option<String>, &'static str);
    use socle::grandeurs::ecrit::sans_zero::dit;
    let mut rangs: Vec<Rang> = vec![
        ("Cachets", |c| Some(dit(c.cachets)), ""),
        ("Heures déclarées", |c| Some(dit(c.heures)), " h"),
        ("Total heures (cachets × 12 + heures)", |c| Some(dit(c.heures_equivalentes)), " h"),
    ];
    if enseigne {
        rangs.push(("dont enseignement (droits seuls, pas le montant)", |c| Some(dit(c.enseignement)), " h"));
        rangs.push(("Heures retenues pour les 507 h", |c| c.heures_ouverture.map(dit), " h"));
    }
    rangs.push(("Salaire brut", |c| Some(euros(c.brut)), ""));
    let lignes = rangs
        .iter()
        .map(|(libelle, valeur, suffixe)| {
            let mut cel = vec![s(*libelle)];
            cel.extend(colonnes.iter().map(|c| so(valeur(c).map(|v| format!("{}{}", v, suffixe)))));
            ligne(cel, vec![])
        })
        .collect();
    let mut largeurs = vec![mesure!("0.4")];
    // Chaque colonne, la part d'une sur `n` ; aucune colonne, aucune part.
    largeurs.extend((0..n).filter_map(|_| mesure!("0.6").part_sur(n)));
    tableau(&largeurs, entete, lignes, "#eeeeee")
}

/// Le compteur de conges payes d'un permanent.
fn tableau_conges(cpt: &CompteurConges) -> Bloc {
    let mut lignes = vec![("Acquis sur la période", cpt.acquis), ("Pris", cpt.pris)];
    if !cpt.reports.est_nul() {
        lignes.insert(1, ("Report de la période précédente", cpt.reports));
    }
    let entete = vec![ligne(
        vec![
            cm(
                vec![
                    m("CONGÉS PAYÉS", true, None),
                    mt("\n"),
                    mt(format!("période du {} au {}", cpt.periode.debut().jj_mm_aaaa(), cpt.periode.fin().jj_mm_aaaa())),
                ],
                Some(ARDOISE),
            ),
            cell("Jours ouvrables", 1, true, None),
        ],
        vec![Opt::Couleur(ROUGE), Opt::Taille(mesure!("6.8")), Opt::Fond(FOND_FAMILLE), filet_haut(ROUGE, mesure!("0.8"))],
    )];
    let mut corps: Vec<Ligne> =
        lignes.iter().map(|(l, v)| ligne(vec![s(*l), s(format!("{} j", jours_de_conges(*v)))], vec![])).collect();
    corps.push(ligne(vec![s("Solde"), s(format!("{} j", jours_de_conges(cpt.solde())))], vec![gras()]));
    tableau(&[mesure!("0.4"), mesure!("0.6")], entete, corps, "#eeeeee")
}

// ------------------------------------------------------------ le modele de l'arrete

/// Les colonnes du tableau du bulletin : le libelle, puis celles du modele de l'arrete du 25/02/2016, art. 1, I
/// (redaction du 25/06/2024 ; atelier/reference/sources/montant-net-social/legi-arrete-20160225-bulletin/
/// LEGIARTI000049854119.xml) : « Base », « Taux », « Salarié », « Employeur ». Un seul taux : celui du salarie, le seul
/// que le code du travail exige (R. 3243-1, 8°, a : « pour les cotisations et contributions [...] a la charge du
/// salarie, leurs taux ») ; le modele met « - » au taux des lignes que l'employeur paie seul (accidents du travail,
/// famille). Les deux modeles impriment ce tableau-la (voir `ModeleBulletin`).
const COLONNES: usize = 5;
const LARGEURS: [Mesure; 5] = [mesure!("0.55"), mesure!("0.12"), mesure!("0.09"), mesure!("0.12"), mesure!("0.12")];
/// L'annexe du modele *complet* : le libelle, la base, le taux et le montant de chaque part.
const COLONNES_ANNEXE: usize = 6;
const LARGEURS_ANNEXE: [Mesure; 6] = [mesure!("0.46"), mesure!("0.11"), mesure!("0.09"), mesure!("0.11"), mesure!("0.11"), mesure!("0.12")];

/// LES DEUX MODELES DE BULLETIN.
///
/// - *Simplifie* : le modele de l'arrete du 25/02/2016, art. 1, I.
/// - *Complet* : le MEME bulletin, ligne pour ligne, suivi d'une page annexe qui detaille chaque cotisation et
///   contribution sous la ligne du modele qui la porte (sa base, le taux et le montant de chaque part, le taux employeur
///   compris). Le BOSS, FAQ « Règles générales relatives au bulletin de paie », Q 10
///   (atelier/reference/sources/montant-net-social/boss-regles-generales-bulletin-de-paie-20250207-archive-web.txt,
///   l. 170-172, copie de l'archive du web, a redeposer depuis le BOSS) : « Il n'est pas possible d'indiquer plus de
///   détails pour la rubrique « Cotisations et contributions sociales obligatoires » [...] son uniformisation permet
///   d'assurer la lisibilité entre les bulletins des différents employeurs » ; « il est autorisé de fournir le détail de
///   certaines rubriques (cotisations statutaires, autres charges dues, complémentaire garantie frais de santé
///   obligatoire, etc.) au verso du bulletin de paie ou bien sur un document annexe ». Le complet
///   n'ajoute donc ni colonne « Taux employeur » ni lignes « dont ... » DANS la rubrique, que Q 10 refuse.
///   L'annexe est une page qui suit celle du bulletin.
///
/// QUI CHOISIT : la compagnie, pour tous (`modele_bulletin` de la fiche entreprise, ou il se
/// choisit) ; un salarie qui demande l'autre modele l'obtient par la case `modele_bulletin` de sa fiche (Avance),
/// qui l'emporte. Sans choix : le simplifie. Le modele est un choix de la fiche (`droit::fiche::ModeleBulletin`), lu par
/// sa forme : une valeur inconnue refuse la fiche (regle 1).
///
/// Le modele du bulletin d'une paie : la case de la fiche du salarie l'emporte ; sinon celle de la compagnie. Sans choix,
/// le bulletin est celui de l'arrete du 25/02/2016, art. 1, I, le simplifie, sans l'annexe que seul le complet ajoute.
pub fn modele_du_bulletin(ent: &ChampsEntreprise, sal: &FicheSalarie) -> R<ModeleBulletin> {
    Ok(match modele_du_salarie(sal)? {
        Some(m) => m,
        None => modele_de_la_compagnie(ent),
    })
}

/// LE MODELE DE LA COMPAGNIE : celui qu'elle a choisi ; sans choix, le modele que l'arrete du 25/02/2016 donne, art. 1,
/// I (redaction du 25/06/2024 ; atelier/reference/sources/montant-net-social/legi-arrete-20160225-bulletin/
/// LEGIARTI000049854119.xml, l. 68) : « Les informations mentionnées à l'article R. 3243-1 du code du travail sont
/// présentées selon le modèle suivant », le simplifie. Le complet n'y ajoute qu'une page annexe.
pub fn modele_de_la_compagnie(ent: &ChampsEntreprise) -> ModeleBulletin {
    match ent.modele_bulletin.dite() {
        Some(m) => *m,
        None => ModeleBulletin::Simplifie,
    }
}

/// Le style d'un titre de rubrique du modele (« COTISATIONS ET CONTRIBUTIONS SOCIALES OBLIGATOIRES », « IMPÔT SUR LE
/// REVENU »...) : en gras, rouge, sur fond rose.
fn style_section() -> Vec<Opt> {
    vec![gras(), Opt::Couleur(ROUGE), Opt::Taille(mesure!("6.8")), Opt::Fond(FOND_FAMILLE)]
}

/// Ou une ligne de cotisation se range dans le modele : son rang (l'ordre du modele), le titre du groupe qui la porte
/// (« Santé », « Retraite »), et le libelle du modele quand elle est seule sur sa ligne du modele.
#[derive(Clone, PartialEq)]
enum Rang {
    /// « Santé » : ses lignes sous le titre du groupe.
    Sante,
    /// Une ligne du modele (accidents du travail, famille, chomage, Apec, cotisations conventionnelles, CSG,
    /// prevoyance) : seule, elle prend le libelle du modele ; a plusieurs (deux tranches d'Apec, FNAS et FCAP), le
    /// libelle du modele les titre et chacune garde le sien.
    Ligne(u8, &'static str),
    /// « Retraite » : ses lignes sous le titre du groupe.
    Retraite,
    /// « Autres charges dues par l'employeur » : UNE ligne, le seul montant employeur, toutes ses contributions
    /// agregees (C. trav. R. 3243-2, al. 2 ; arrete, art. 3), la caisse des Conges Spectacles comprise.
    Autres,
    /// Une famille que le moteur ne range pas : titree par sa famille.
    Hors(droit::forme_bareme::FamilleDeCotisation),
    /// « COTISATIONS ET CONTRIBUTIONS SOCIALES FACULTATIVES ».
    Facultative(&'static str),
    /// « EXONÉRATIONS ET ALLÈGEMENTS DE COTISATIONS ».
    Allegement,
}

impl Rang {
    /// Le libelle qui titre le rang dans l'annexe du modele complet : la ligne (ou le groupe) du bulletin qui le porte.
    fn titre(&self) -> &str {
        match self {
            Rang::Sante => "Santé",
            Rang::Retraite => "Retraite",
            Rang::Ligne(_, nom) | Rang::Facultative(nom) => nom,
            Rang::Autres => AUTRES,
            Rang::Hors(famille) => famille.as_str(),
            Rang::Allegement => EXONERATIONS,
        }
    }
}

const SANTE_SECU: &str = "Sécurité Sociale Maladie Maternité Invalidité Décès";
const SANTE_COMPLEMENTAIRE: &str = "Complémentaire garanties frais de santé obligatoire";
const ATMP: &str = "Accidents du travail & maladies professionnelles";
const FAMILLE: &str = "Famille";
const CHOMAGE: &str = "Assurance chômage";
const APEC: &str = "Apec";
const AUTRES: &str = "Autres charges dues par l'employeur";
const CONVENTION: &str = "Cotisations statutaires ou prévues par la convention collective";
const CSG_DEDUCTIBLE: &str = "CSG déductible de l'impôt sur le revenu";
const CSG_NON_DEDUCTIBLE: &str = "CSG/CRDS non déductible de l'impôt sur le revenu";
const PREVOYANCE: &str = "Prévoyance, Incapacité, Invalidité, Décès, Autres";
const OBLIGATOIRES: &str = "COTISATIONS ET CONTRIBUTIONS SOCIALES OBLIGATOIRES";
const TOTAL_OBLIGATOIRES: &str = "TOTAL DES COTISATIONS ET CONTRIBUTIONS OBLIGATOIRES";
const EXONERATIONS: &str = "EXONÉRATIONS ET ALLÈGEMENTS DE COTISATIONS";
const FACULTATIVES: &str = "COTISATIONS ET CONTRIBUTIONS SOCIALES FACULTATIVES";

/// Le rang d'une ligne de cotisation, par son identifiant (le libelle et la famille sont ceux du bareme, que le
/// bulletin ne lit plus que pour une ligne qu'il ne connait pas). Le modele range la prevoyance, obligatoire ou non,
/// sous « COTISATIONS ET CONTRIBUTIONS SOCIALES FACULTATIVES », sa seule ligne « Prévoyance, Incapacité, Invalidité,
/// Décès, Autres » (art. 1, I) ; la FAQ du BOSS, Q 18 (meme fichier que `modele_du_bulletin`, l. 192-196) : une cotisation rendue
/// obligatoire par un accord de branche, et non par la loi, est facultative.
///
/// La caisse des Conges Spectacles : dans « Autres charges dues par l'employeur », et non sur une ligne
/// a elle, que le modele ne nomme pas. FAQ du BOSS, Q 11 (l. 173-174) : « Est-il possible d'afficher des lignes
/// supplémentaires pour certaines contributions propres à un secteur d'activité [...] (ex : BTP [...]) ? Non, il convient
/// de regrouper certaines contributions particulières dans les rubriques « autres » prévues à cet effet » ; Q 12
/// (l. 175-178) : « totaliser les contributions légales à la charge exclusive de l'employeur dans la rubrique « Autres
/// charges dues par l'employeur » ». La caisse est propre au secteur, legale (C. trav. L. 3141-32 et D. 7121-40,
/// atelier/reference/sources/code-du-travail/articles-20261013.md : l'employeur du spectacle « s'affilie ») et a la seule
/// charge de l'employeur (le bareme n'a pas de taux salarial).
///
/// Une garantie de prevoyance de la paie (`garanties`, `engagement::calcul::garanties_de_prevoyance`) est facultative.
fn rang(l: &engagement::calcul::LigneCotisation, garanties: &[IdDeCotisation]) -> Rang {
    let seule_employeur = l.taux_sal.is_none() && l.montant_sal.est_nul();
    if garanties.contains(&l.id) {
        return Rang::Facultative(PREVOYANCE);
    }
    let id = &l.id;
    if Regroupement::DE_L_APEC.contains(id) {
        return Rang::Ligne(5, APEC);
    }
    if [cot::MALADIE, cot::REGIME_LOCAL, cot::MUTUELLE].contains(id) {
        Rang::Sante
    } else if *id == cot::ATMP {
        Rang::Ligne(1, ATMP)
    } else if [cot::VIEIL_PLAF, cot::VIEIL_DEPLAF, cot::RETRAITE_T1, cot::RETRAITE_T2, cot::CET].contains(id) {
        Rang::Retraite
    } else if *id == cot::ALLOCATIONS_FAMILIALES {
        Rang::Ligne(3, FAMILLE)
    } else if *id == cot::CHOMAGE {
        Rang::Ligne(4, CHOMAGE)
    } else if seule_employeur && (Regroupement::DE_L_EMPLOYEUR_SEUL.contains(id) || *id == cot::CONGES_SPEC) {
        Rang::Autres
    } else if Regroupement::DE_LA_CONVENTION.contains(id) {
        Rang::Ligne(8, CONVENTION)
    } else if *id == cot::CSG_DED {
        Rang::Ligne(9, CSG_DEDUCTIBLE)
    } else if *id == cot::CSG_CRDS_ND {
        Rang::Ligne(10, CSG_NON_DEDUCTIBLE)
    } else if [cot::REDUCTION_GENERALE, engagement::heures_en_plus::REDUCTION, engagement::heures_en_plus::DEDUCTION].contains(id) {
        Rang::Allegement
    } else {
        Rang::Hors(l.famille.clone())
    }
}

/// L'ordre du modele : santé, accidents du travail, retraite, famille, chomage, Apec, autres charges, cotisations
/// conventionnelles, CSG deductible, CSG/CRDS non deductible, (ce que le moteur ne range pas).
fn ordre(r: &Rang) -> u8 {
    match r {
        Rang::Sante => 0,
        Rang::Retraite => 2,
        Rang::Ligne(n, _) => *n,
        Rang::Autres => 6,
        Rang::Hors(_) => 11,
        Rang::Facultative(_) => 20,
        Rang::Allegement => 30,
    }
}

fn obligatoire(r: &Rang) -> bool {
    !matches!(r, Rang::Facultative(_) | Rang::Allegement)
}

/// Les lignes de cotisation d'une paie, groupees par rang dans l'ordre du modele (un tri stable : les familles que le
/// moteur ne range pas gardent leur ordre entre elles, et les lignes d'un rang celui du moteur).
fn rangees<'p>(p: &'p Paie, garanties: &[IdDeCotisation]) -> Vec<(Rang, Vec<&'p engagement::calcul::LigneCotisation>)> {
    let mut rangees: Vec<(Rang, Vec<&engagement::calcul::LigneCotisation>)> = Vec::new();
    for l in &p.lignes {
        let r = rang(l, garanties);
        match rangees.iter_mut().find(|(x, _)| *x == r) {
            Some((_, v)) => v.push(l),
            None => rangees.push((r, vec![l])),
        }
    }
    rangees.sort_by_key(|(r, _)| ordre(r));
    rangees
}

/// Le libelle qu'une ligne de « Santé » ou de « Retraite » prend du modele ; celles qu'il ne nomme pas (le regime
/// local d'Alsace-Moselle, la CET d'un permanent, que le moteur calcule sur T1 + T2 sans la partager entre les deux
/// tranches) gardent le leur.
fn libelle_du_modele(l: &engagement::calcul::LigneCotisation) -> String {
    const DU_MODELE: [(IdDeCotisation, &str); 6] = [
        (cot::MALADIE, SANTE_SECU),
        (cot::MUTUELLE, SANTE_COMPLEMENTAIRE),
        (cot::VIEIL_PLAF, "Sécurité Sociale Vieillesse plafonnée"),
        (cot::VIEIL_DEPLAF, "Sécurité Sociale Vieillesse déplafonnée"),
        (cot::RETRAITE_T1, "Retraite complémentaire, CEG et CET T1"),
        (cot::RETRAITE_T2, "Retraite complémentaire, CEG et CET T2"),
    ];
    match DU_MODELE.iter().find(|(id, _)| *id == l.id) {
        Some((_, libelle)) => (*libelle).to_string(),
        None => l.libelle.clone(),
    }
}

/// Un montant, que sa case ne dit pas quand il est nul.
fn montant_ou_rien(x: Montant) -> Cellule {
    if x.est_nul() {
        vide()
    } else {
        s(chiffres_en_euros(x))
    }
}

/// Un taux, s'il y en a un.
fn taux_dit(t: Option<Taux>) -> Cellule {
    so(t.map(fmt_taux))
}

/// La base d'une ligne de cotisation, dans la colonne des montants : ses euros, ou ses heures, ecrits de meme.
fn base_dite(l: &engagement::calcul::LigneCotisation) -> Cellule {
    use engagement::calcul::BaseDeCotisation;
    s(match l.base {
        BaseDeCotisation::Montant(m) => chiffres_en_euros(m),
        BaseDeCotisation::Heures(h) => socle::texte::chiffres_d_heures(h),
    })
}

/// La base d'une ligne, et son taux salarial et son taux employeur ; rien la ou le bareme masque base et taux.
fn base_et_taux(l: &engagement::calcul::LigneCotisation) -> R<(Cellule, Cellule, Cellule)> {
    if l.masquer_base_et_taux {
        return Ok((vide(), vide(), vide()));
    }
    Ok((base_dite(l), taux_dit(l.taux_sal), taux_dit(l.taux_pat)))
}

/// Une ligne de cotisation du bulletin : libelle, base (sauf `masquer_base_et_taux`), taux salarial, montant salarial,
/// montant employeur ; un montant nul ne s'ecrit pas.
fn ligne_cotisation(libelle: String, l: &engagement::calcul::LigneCotisation) -> R<Ligne> {
    let base = if l.masquer_base_et_taux { vide() } else { base_dite(l) };
    Ok(ligne(vec![s(libelle), base, taux_dit(l.taux_sal), montant_ou_rien(l.montant_sal), montant_ou_rien(l.montant_pat)], vec![]))
}

fn titre_groupe(titre: &str, colonnes: usize) -> Ligne {
    ligne(vec![cel(titre, colonnes)], vec![gras()])
}

/// Les lignes d'un groupe de rangs egaux, dans l'ordre du moteur.
fn lignes_du_rang(r: &Rang, lignes: &[&engagement::calcul::LigneCotisation]) -> R<Vec<Ligne>> {
    let mut out = Vec::new();
    match r {
        Rang::Sante | Rang::Retraite => {
            out.push(titre_groupe(r.titre(), COLONNES));
            for l in lignes {
                out.push(ligne_cotisation(libelle_du_modele(l), l)?);
            }
        }
        Rang::Ligne(_, nom) | Rang::Facultative(nom) => {
            if let [seule] = lignes {
                out.push(ligne_cotisation((*nom).to_string(), seule)?);
            } else {
                out.push(titre_groupe(nom, COLONNES));
                for l in lignes {
                    out.push(ligne_cotisation(l.libelle.clone(), l)?);
                }
            }
        }
        Rang::Autres => {
            let total = lignes.iter().fold(Montant::ZERO, |a, l| a + l.montant_pat);
            out.push(ligne(vec![s(AUTRES), vide(), vide(), vide(), s(chiffres_en_euros(total))], vec![]));
        }
        Rang::Hors(famille) => {
            out.push(titre_groupe(famille.as_str(), COLONNES));
            for l in lignes {
                out.push(ligne_cotisation(l.libelle.clone(), l)?);
            }
        }
        Rang::Allegement => {}
    }
    Ok(out)
}

/// Les sommes salariale et employeur des lignes des rangs que `filtre` retient.
fn somme_des_rangs(
    rangees: &[(Rang, Vec<&engagement::calcul::LigneCotisation>)],
    filtre: &dyn Fn(&Rang) -> bool,
) -> (Montant, Montant) {
    rangees.iter().filter(|(r, _)| filtre(r)).flat_map(|(_, v)| v.iter()).fold((Montant::ZERO, Montant::ZERO), |(s, e), l| {
        (s + l.montant_sal, e + l.montant_pat)
    })
}

/// Le milieu du bulletin, du brut au montant net social : le modele de l'arrete du 25/02/2016, art. 1, I.
/// - « COTISATIONS ET CONTRIBUTIONS SOCIALES OBLIGATOIRES », ses lignes dans l'ordre et sous les libelles du modele ;
///   « Autres charges dues par l'employeur » agrege les contributions que l'employeur paie seul (C. trav. R. 3243-2,
///   al. 2 ; arrete, art. 3, `legi-arrete-20160225-bulletin/LEGIARTI000047098507.xml`).
/// - « TOTAL DES COTISATIONS ET CONTRIBUTIONS OBLIGATOIRES » : salarie et employeur, AVANT les exonerations (R. 3243-1,
///   8°, a : « avant deduction des exonerations et exemptions mentionnees au 13° »).
/// - « EXONÉRATIONS ET ALLÈGEMENTS DE COTISATIONS » : les reductions qui ne sont pas deduites de chaque cotisation
///   (arrete, art. 4, `LEGIARTI000047098456.xml` : la reduction generale de L. 241-13 pour l'employeur ; la reduction
///   salariale des heures supplementaires et complementaires de L. 241-17, `engagement::heures_en_plus`).
/// - « COTISATIONS ET CONTRIBUTIONS SOCIALES FACULTATIVES » : la prevoyance.
/// Aucun montant ne change : chaque ligne de la paie s'imprime une fois, et les trois sommes refont les totaux de la
/// paie (sinon refus).
fn tableau_modele(p: &Paie, garanties: &[IdDeCotisation]) -> R<Vec<Ligne>> {
    let rangees = rangees(p, garanties);
    let (oblig_sal, oblig_pat) = somme_des_rangs(&rangees, &obligatoire);
    let (fac_sal, fac_pat) = somme_des_rangs(&rangees, &|r| matches!(r, Rang::Facultative(_)));
    let (all_sal, all_pat) = somme_des_rangs(&rangees, &|r| *r == Rang::Allegement);
    let (total_sal, total_pat) = (p.total_sal, p.total_pat);
    if oblig_sal + fac_sal + all_sal != total_sal || oblig_pat + fac_pat + all_pat != total_pat {
        let e = socle::texte::euros;
        return Err(socle::refus!(
            "Le bulletin ne retrouve pas les totaux de la paie : salarie {} + {} + {} pour {}, employeur {} + {} + {} \
             pour {}. Une ligne de cotisation n'a pas trouve sa place dans le modele.",
            e(oblig_sal), e(fac_sal), e(all_sal), e(total_sal), e(oblig_pat), e(fac_pat), e(all_pat), e(total_pat)
        ));
    }
    let mut out = vec![ligne(vec![cel(OBLIGATOIRES, COLONNES)], style_section())];
    for (r, lignes) in rangees.iter().filter(|(r, _)| obligatoire(r)) {
        out.extend(lignes_du_rang(r, lignes)?);
    }
    out.push(ligne(
        vec![cel(TOTAL_OBLIGATOIRES, 3), s(chiffres_en_euros(oblig_sal)), s(chiffres_en_euros(oblig_pat))],
        vec![gras(), filet_haut(ROUGE, mesure!("0.6"))],
    ));
    if rangees.iter().any(|(r, _)| *r == Rang::Allegement) {
        out.push(ligne(vec![cel(EXONERATIONS, 3), montant_ou_rien(all_sal), s(chiffres_en_euros(all_pat))], vec![gras()]));
    }
    if rangees.iter().any(|(r, _)| matches!(r, Rang::Facultative(_))) {
        out.push(ligne(vec![cel(FACULTATIVES, COLONNES)], style_section()));
        for (r, lignes) in rangees.iter().filter(|(r, _)| matches!(r, Rang::Facultative(_))) {
            out.extend(lignes_du_rang(r, lignes)?);
        }
    }
    Ok(out)
}

/// L'annexe du modele complet (voir `Modele`) : une page qui suit le bulletin et detaille chacune de ses lignes de
/// cotisation -- chaque ligne du bareme sous la ligne (ou le groupe) du bulletin qui la porte, avec sa base, le taux et
/// le montant de la part salariale et de la part employeur --, puis les trois totaux du bulletin, aux memes montants.
/// Elle ne dit rien que la paie ne dise : chaque ligne de la paie s'y imprime une fois.
fn blocs_annexe(p: &Paie, numero: Option<&NumeroDeBulletin>, garanties: &[IdDeCotisation]) -> R<Vec<Bloc>> {
    let sal = &p.salarie;
    // Un bulletin sans numero (un apercu) ne le dit pas.
    let bulletin = if let Some(n) = numero { format!("au bulletin n° {}", n) } else { "au bulletin".to_string() };
    let mut blocs = vec![
        para(vec![m("Détail des cotisations et contributions", true, None)], mesure!("11"), Some(ROUGE), Mesure::ZERO, Mesure::UN),
        para(
            vec![mt(format!(
                "Document annexe {} — {} — du {} au {}.\nIl détaille, sous chaque ligne du bulletin \
                 qui les regroupe, les cotisations et contributions de la paie : leur base, le taux et le montant de la \
                 part salariale et de la part employeur. Il ne remplace aucune ligne du bulletin ; les totaux sont les \
                 siens.",
                bulletin,
                nom_imprime(sal)?,
                p.engagement.periode_payee()?.debut().jj_mm_aaaa(),
                p.engagement.periode_payee()?.fin().jj_mm_aaaa(),
            ))],
            mesure!("7"),
            Some(GRIS),
            Mesure::ZERO,
            mesure!("6"),
        ),
    ];
    let th = vec![Opt::Couleur(BLANC), gras(), Opt::Taille(mesure!("6.6")), Opt::Fond(ROUGE)];
    let entete = vec![ligne(
        vec![
            cell("LIBELLÉ", 1, true, None),
            s("Base"),
            s("Taux salarié"),
            s("Salarié"),
            s("Taux employeur"),
            s("Employeur"),
        ],
        th,
    )];
    let rangees = rangees(p, garanties);
    let detail = |r: &Rang, lignes: &[&engagement::calcul::LigneCotisation], corps: &mut Vec<Ligne>| -> R<()> {
        // Les exonerations n'ont pas de titre : la ligne du bulletin qui les porte est leur total, qui les suit.
        if *r != Rang::Allegement {
            corps.push(titre_groupe(r.titre(), COLONNES_ANNEXE));
        }
        for l in lignes {
            let (base, ts, tp) = base_et_taux(l)?;
            corps.push(ligne(
                vec![
                    s(format!("{}{}{}", NBSP, NBSP, l.libelle)),
                    base,
                    ts,
                    montant_ou_rien(l.montant_sal),
                    tp,
                    montant_ou_rien(l.montant_pat),
                ],
                vec![],
            ));
        }
        Ok(())
    };
    let total = |libelle: &str, (sal, pat): (Montant, Montant)| {
        ligne(
            vec![cel(libelle, 3), montant_ou_rien(sal), vide(), s(chiffres_en_euros(pat))],
            vec![gras(), filet_haut(ROUGE, mesure!("0.6"))],
        )
    };
    let mut corps = vec![ligne(vec![cel(OBLIGATOIRES, COLONNES_ANNEXE)], style_section())];
    for (r, lignes) in rangees.iter().filter(|(r, _)| obligatoire(r)) {
        detail(r, lignes, &mut corps)?;
    }
    corps.push(total(TOTAL_OBLIGATOIRES, somme_des_rangs(&rangees, &obligatoire)));
    for (r, lignes) in rangees.iter().filter(|(r, _)| *r == Rang::Allegement) {
        detail(r, lignes, &mut corps)?;
        corps.push(total(EXONERATIONS, somme_des_rangs(&rangees, &|r| *r == Rang::Allegement)));
    }
    if rangees.iter().any(|(r, _)| matches!(r, Rang::Facultative(_))) {
        corps.push(ligne(vec![cel(FACULTATIVES, COLONNES_ANNEXE)], style_section()));
        for (r, lignes) in rangees.iter().filter(|(r, _)| matches!(r, Rang::Facultative(_))) {
            detail(r, lignes, &mut corps)?;
        }
    }
    blocs.push(tableau(&LARGEURS_ANNEXE, entete, corps, "#e4e4e4"));
    Ok(blocs)
}

// ------------------------------------------------------------ un bulletin

/// Les arguments des blocs d'un bulletin : ce que le bulletin dit en plus d'elle.
pub struct Jeu<'a> {
    pub paie: &'a Paie,
    /// Le numero du bulletin (`N/AAAA`), tel que l'engagement le porte (`engagement::numeros`) ou que le depart le
    /// reserve : il se donne a cote de la paie, qui ne se copie pas sous un autre numero.
    pub numero: Option<&'a NumeroDeBulletin>,
    pub colonnes: Option<&'a [Colonne]>,
    pub conges: Option<&'a CompteurConges>,
    pub pas: &'a Versement,
    pub rectificatif: Option<&'a Rectificatif>,
    /// Les indices, dans `paie.remuneration`, des lignes hors assiette qui ne sont pas un revenu (remboursements de
    /// frais, acompte : `HorsAssiette::Frais` et `Mouvement`) : elles s'impriment sous « REMBOURSEMENTS ET DEDUCTIONS
    /// DIVERSES », entre le montant net social et le net a payer avant impot (arrete du 25/02/2016, art. 1, I ; BOSS,
    /// montant net social, Q 39). Vide : toutes les lignes sont au-dessus du brut.
    pub remboursements: Vec<usize>,
    /// Les indices, dans `paie.remuneration`, des avantages en nature : comptes dans le brut, recus en nature, ils
    /// s'impriment aussi en deduction sous « REMBOURSEMENTS ET DEDUCTIONS DIVERSES », pour qu'on passe du montant net
    /// social au net a payer avant impot, qui ne les compte pas (R. 3243-1, 9°, `engagement::calcul`). Les ranger la est
    /// une inference : C. trav. R. 3243-1, 8° b) demande « La nature et le montant des versements et retenues autres que
    /// celles mentionnées au a effectués sur la période », et le modele de l'arrete du 25/02/2016 (art. 1, I) place cette
    /// section entre ces deux nets, sans nommer l'avantage en nature.
    pub en_nature: Vec<usize>,
    /// Les garanties de prevoyance de la paie (`engagement::calcul::garanties_de_prevoyance`) : leurs lignes sont
    /// facultatives.
    pub prevoyance: Vec<IdDeCotisation>,
}

/// Les blocs d'UN bulletin.
fn blocs_un(j: &Jeu, ent: &ChampsEntreprise) -> R<Vec<Bloc>> {
    let p = j.paie;
    let (sal, e, sp) = (&p.salarie, &p.engagement, &p.spectacle);
    let conv = convention_du_contrat(ent, sp.as_ref(), sal, &e.fin_mois())?;
    let n = COLONNES;
    let nat = e.nat()?;
    let permanent = nat.mensuelle();
    let tiret = format!("{}—{}", NBSP, NBSP);
    let barre = format!(" {}|{} ", NBSP, NBSP);

    // L'adresse et la commune lisibles de la compagnie, sinon celles de sa fiche, qu'elle doit dire.
    let nom_affichage = ent.nom_affichage.ecrite("nom_affichage")?;
    let adresse_civile = ent.adresse.ecrite("adresse")?;
    let adresse = match ent.adresse_affichage.portee() {
        Some(a) => a,
        None => adresse_civile,
    };
    let code_postal = ent.code_postal.ecrite("code_postal")?;
    let commune_civile = ent.commune.ecrite("commune")?;
    let commune = match ent.commune_affichage.portee() {
        Some(c) => c,
        None => commune_civile,
    };
    let licence = ent.licence_spectacle.ecrite("licence_spectacle")?;
    // « Le nom et l'adresse de l'employeur ainsi que, le cas echeant, la designation de l'etablissement dont depend le
    // salarie » et l'activite « de l'etablissement d'emploi » (code du travail, R. 3243-1, 1° et 2°,
    // `atelier/reference/sources/code-du-travail/xml/R3243-1.xml`) : l'en-tete dit l'employeur, le cadre EMPLOYEUR
    // l'etablissement du contrat, celui sous lequel la DSN le declare (`droit::bareme::entreprise_du_salarie`).
    let vue = droit::bareme::entreprise_du_salarie(ent, sal)?;
    let (siret, naf) = (vue.siret.ecrite("siret")?, vue.naf.ecrite("naf")?);
    let etablissement = vue.nic.dite().and_then(|n| ent.etablissement(n)).map(|l| {
        // La forme lisible, sinon la forme civile ; ce que l'etablissement ne dit pas ne s'imprime pas.
        let ou = |usage: &Option<String>, civil: &Option<String>| usage.remplie().or(civil.portee()).map(str::to_string);
        let complement = l.complement_adresse_affichage.remplie().or(l.complement_adresse.remplie());
        let adresse = ou(&l.adresse_affichage, &l.adresse.en_texte()).map(|a| droit::fiche::avec_complement(a, complement));
        let lieu = [adresse, l.code_postal.en_texte(), ou(&l.commune_affichage, &l.commune.en_texte())]
            .into_iter()
            .flatten()
            .filter(|s| !s.trim().is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        match l.libelle.portee().filter(|n| !n.trim().is_empty()) {
            Some(nom) => format!("{}, {}", nom, lieu),
            None => lieu,
        }
    });
    let (siret_siege, naf_siege) = (ent.siret.ecrite("siret")?, ent.naf.ecrite("naf")?);
    let mut blocs = vec![
        para(vec![m(nom_affichage, true, None)], mesure!("13"), Some(ARDOISE), Mesure::ZERO, Mesure::ZERO),
        para(
            vec![mt(format!(
                "{} — {} {}\nSiret {} {}·{} APE {} {}·{} Licence {}",
                adresse, code_postal, commune, siret_siege, NBSP, NBSP, naf_siege, NBSP, NBSP, licence
            ))],
            mesure!("6.6"),
            Some(GRIS),
            Mesure::ZERO,
            Mesure::ZERO,
        ),
    ];
    let titre_rectif = if j.rectificatif.is_some() { " — RECTIFICATIF" } else { "" };
    // Un bulletin sans numero (un apercu) ne le dit pas.
    let titre = if let Some(n) = j.numero { format!("Bulletin de paie n° {}{}", n, titre_rectif) } else { format!("Bulletin de paie{}", titre_rectif) };
    blocs.push(para(
        vec![m(
            titre,
            true,
            None,
        )],
        mesure!("11"),
        Some(ROUGE),
        mesure!("8"),
        Mesure::UN,
    ));
    if let Some(r) = j.rectificatif {
        let suite = match &r.ecart {
            None => ", qu'il remplace.".to_string(),
            Some(ecart) => format!(" : il en porte l'écart, versé avec la paie {}.", socle::texte::de(&ecart.mois_versement.libelle(Graphie::Accentuee))),
        };
        blocs.push(para(
            vec![m(format!("Rectifie le bulletin n° {} de {}", r.rectifie, r.mois_origine.libelle(Graphie::Accentuee)), true, None), mt(suite)],
            mesure!("7"),
            Some(GRIS),
            Mesure::ZERO,
            mesure!("5"),
        ));
    }
    let k = |t: &str| m(t, false, Some(GRIS_CLAIR));
    let sous_titre;
    let ligne_caisse;
    let travail;
    if permanent {
        sous_titre = format!(
            "Durée contractuelle : {} h par mois {} durée de référence de l'entreprise : {} h",
            e.quotite()?.dite(),
            tiret,
            match e.quotite_reference()? {
                Some(q) => q.dite(),
                None => return Err(refus("nombre de jours absent")),
            }
        );
        ligne_caisse = Vec::new();
        travail = vec![m("Durée du travail :", true, None), mt(format!(" {} h", e.quotite()?.dite()))];
    } else {
        sous_titre =
            format!("Numéro d'objet : {} {} Spectacle : {}", du_spectacle(sp.as_ref(), |s| &s.numero_objet, "numero_objet")?, tiret, du_spectacle(sp.as_ref(), |s| &s.titre, "titre")?);
        ligne_caisse = vec![
            k("N° Congés Spectacles :"),
            // « N'en a pas » s'imprime comme l'absence de numero.
            mt(format!(" {}\n", match sal.num_conges_spectacles.dite().and_then(droit::fiche::NumeroCongesSpectacles::numero) { Some(n) => n, None => "-" })),
        ];
        let dates: Vec<String> = e.dates_travail.iter().map(Jour::jj_mm_aaaa).collect();
        travail = vec![m("Dates travaillées :", true, None), mt(format!(" {}", dates.join(", ")))];
    }
    blocs.push(para(vec![mt(sous_titre)], mesure!("7"), Some(GRIS), Mesure::ZERO, mesure!("5")));

    // Le cadre d'identite : un tableau d'une ligne, deux cellules a filet.
    let nir_affiche = match sal.nir.remplie() {
        Some(n) => format!("{}{}", n, sal.nir_cle.exigee("nir_cle")?),
        None => "en cours d'immatriculation".to_string(),
    };
    let mut salarie = vec![
        m(nom_imprime(sal)?, true, None),
        mt(format!("\n{}\n{} {}\n\n", exige(&sal.adresse, "adresse")?, exige(&sal.code_postal, "code_postal")?, exige(&sal.commune, "commune")?)),
        k("N° Sécurité sociale :"),
        mt(format!(" {}\n", nir_affiche)),
        k("Date de naissance :"),
        mt(format!(" {}\n", sal.date_naissance.exigee("date_naissance")?.jj_mm_aaaa())),
        k("Emploi :"),
        mt(format!(" {}\n", emploi_imprime(e, sal)?)),
    ];
    if let Some(c) = classification(Some(&conv.0), sal, e.nat()?.au_mois().as_ref())? {
        salarie.push(k("Classification :"));
        salarie.push(mt(format!(" {}\n", c)));
    }
    salarie.extend(ligne_caisse);
    // LA DEDUCTION FORFAITAIRE SPECIFIQUE que la paie a appliquee : le taux de l'annee de sa categorie (arrete du
    // 4 septembre 2025, art. 9, II : 16 % en 2026 pour la categorie 20 %, 18 % pour la 25 %), et ce qu'elle a retire,
    // apres le plafond annuel et l'assiette minimum (`engagement::calcul::abattement_dfs` ;
    // `atelier/reference/sources/dfs/LISEZ-MOI.md`).
    salarie.push(k("Abattement :"));
    salarie.push(mt(match &p.deduction_forfaitaire {
        Some(d) => format!(
            " {} % (catégorie {} %, taux de l'année) — {} déduits",
            socle::texte::virgule(d.taux),
            d.categorie,
            euros(d.montant)
        ),
        None => " 0 %".to_string(),
    }));
    let mut employeur = vec![m("EMPLOYEUR", true, None), mt("\n\n")];
    if let Some(e) = &etablissement {
        employeur.push(k("Établissement :"));
        employeur.push(mt(format!(" {}\n", e)));
    }
    employeur.extend([
        k("N° Siret :"),
        mt(format!(" {}\n", siret)),
        k("NAF (APE) :"),
        mt(format!(" {}\n", naf)),
        k("Urssaf :"),
        mt(format!(
            " {}\n",
            ent.urssaf_compte.ecrite("urssaf_compte")?
        )),
        k("Conv. collective :"),
        mt(format!(" IDCC {} - {}\n", conv.0, conv.1)),
        k("Licence spec. :"),
        mt(format!(" {}", licence)),
    ]);
    let moitie = || f::ColonneTableau { largeur: mesure!("0.5"), alignement: f::Alignement::Gauche };
    blocs.push(Bloc::Tableau(f::Tableau {
        colonnes: vec![moitie(), moitie()],
        entete: None,
        lignes: Some(vec![ligne(vec![cm(salarie, None), cm(employeur, None)], vec![])]),
        taille: Some(mesure!("7")),
        couleur: None,
        interligne: Some(mesure!("1.27")),
        marge_cellule: marge_cellule(),
        filet_lignes: None,
        filets_cellules: Some(filet("#dddddd", mesure!("0.5"))),
        espace_avant: Mesure::ZERO,
        espace_apres: Mesure::ZERO,
    }));

    let contrat_long = if permanent {
        let mut long = format!(" ({} depuis le {}", nat.libelle, e.contrat().debut().jj_mm_aaaa());
        if let Some(f) = e.contrat().fin() {
            long.push_str(&format!(", jusqu'au {}", f.jj_mm_aaaa()));
        }
        long.push(')');
        long
    } else if e.place_du_mois()? != engagement::PlaceDuMois::Seul {
        let contrat = e.contrat().fermee().ok_or_else(|| socle::refus!("Le contrat qui court sur plusieurs mois depuis le {} n'a pas de date de fin.", e.contrat().debut().jj_mm_aaaa()))?;
        format!(" (contrat du {} au {})", contrat.debut().jj_mm_aaaa(), contrat.fin().jj_mm_aaaa())
    } else {
        String::new()
    };
    let mut periode = vec![
        m("Période :", true, None),
        mt(format!(
            " du {} au {}{} {} ",
            e.periode_payee()?.debut().jj_mm_aaaa(),
            e.periode_payee()?.fin().jj_mm_aaaa(),
            contrat_long,
            tiret
        )),
    ];
    periode.extend(travail);
    blocs.push(para(periode, mesure!("7"), Some(GRIS), mesure!("5"), mesure!("5")));

    // Le tableau : le modele de l'arrete du 25/02/2016, art. 1, I (redaction du 25/06/2024, seul modele depuis le
    // 01/01/2026 ; atelier/reference/sources/montant-net-social/legi-arrete-20160225-bulletin/LEGIARTI000049854119.xml),
    // ses colonnes (Base, Taux, Salarié, Employeur), ses libelles, son ordre et ses regroupements : voir `tableau_modele`.
    let th = || {
        vec![Opt::Couleur(BLANC), gras(), Opt::Taille(mesure!("6.6")), Opt::Fond(ROUGE)]
    };
    let entete =
        vec![ligne(vec![cell("LIBELLÉ", 1, true, None), s("Base"), s("Taux"), s("Salarié"), s("Employeur")], th())];
    let mut corps = Vec::new();
    let rem: Vec<&engagement::calcul::LigneBrut> =
        p.remuneration.iter().enumerate().filter(|(i, _)| !j.remboursements.contains(i)).map(|(_, l)| l).collect();
    // « MONTANT BRUT » (art. 1, I ; VI : mis en valeur).
    if rem.len() <= 1 {
        let titre = match rem.first() {
            Some(l) => ligne_au_bulletin(l),
            None => "SALAIRE".to_string(),
        };
        corps.push(ligne(
            vec![s(format!("{} — MONTANT BRUT", titre)), s(chiffres_en_euros(p.brut))],
            vec![gras(), Opt::Fond(FOND_BRUT)],
        ));
    } else {
        for l in &rem {
            corps.push(ligne(
                vec![s(ligne_au_bulletin(l)), s(chiffres_en_euros(l.montant))],
                vec![],
            ));
        }
        corps.push(ligne(vec![s("MONTANT BRUT"), s(chiffres_en_euros(p.brut))], vec![gras(), Opt::Fond(FOND_BRUT)]));
    }
    corps.extend(tableau_modele(p, &j.prevoyance)?);
    // Le bas du tableau : « MONTANT NET SOCIAL », puis « REMBOURSEMENTS ET DÉDUCTIONS DIVERSES », puis
    // « MONTANT NET À PAYER AVANT IMPÔT SUR LE REVENU », puis « IMPÔT SUR LE REVENU » (« Montant net imposable »,
    // « IMPÔT SUR LE REVENU PRÉLEVÉ À LA SOURCE ») et « MONTANT NET À PAYER (en Euros) », enfin « TOTAL VERSÉ PAR
    // L'EMPLOYEUR » ; deux nets egaux ne partagent pas une ligne (la derogation de l'arrete du 31/01/2023 a pris fin
    // au 01/01/2026). Les libelles sont ceux du modele de l'arrete du 25/02/2016, art. 1, I
    // (`legi-arrete-20160225-bulletin/LEGIARTI000049854119.xml`), ecrits avec les accents du francais, capitales comprises.
    corps.push(ligne_net("MONTANT NET SOCIAL", p.net_social, n));
    if !j.remboursements.is_empty() || !j.en_nature.is_empty() {
        corps.push(ligne(vec![cel("REMBOURSEMENTS ET DÉDUCTIONS DIVERSES", n)], style_section()));
        for i in &j.remboursements {
            let l = &p.remuneration[*i];
            corps.push(ligne(vec![s(ligne_au_bulletin(l)), s(chiffres_en_euros(l.montant))], vec![]));
        }
        // L'avantage en nature, deja dans le brut, se deduit : le salarie l'a recu en nature.
        for i in &j.en_nature {
            let l = &p.remuneration[*i];
            corps.push(ligne(vec![s(l.designation()), s(chiffres_en_euros(-l.montant))], vec![]));
        }
    }
    corps.push(ligne_net("MONTANT NET À PAYER AVANT IMPÔT SUR LE REVENU", p.net_a_payer, n));
    corps.push(ligne(vec![s("IMPÔT SUR LE REVENU"), s("Base"), s("Taux"), s("Salarié")], style_section()));
    corps.push(ligne(vec![cel("Montant net imposable", 3), s(chiffres_en_euros(p.net_imposable))], vec![]));
    match j.rectificatif.and_then(|r| r.ecart.as_ref()) {
        Some(ecart) => corps.extend(rectificatif_lignes(ecart, n)?),
        None => corps.extend(lignes_pas_tableau(p, j.pas, n)?),
    }
    // « TOTAL VERSÉ PAR L'EMPLOYEUR » (art. 1, I ; C. trav. R. 3243-1, 14° : le brut et les cotisations patronales,
    // exonerations deduites) : le cout employeur de la paie, imprime au pied du bulletin
    // (« Coût total employeur »).
    corps.push(ligne(
        vec![cel("TOTAL VERSÉ PAR L'EMPLOYEUR", n - 1), s(chiffres_en_euros(p.cout_employeur))],
        vec![gras(), filet_haut(ROUGE, mesure!("0.6"))],
    ));
    // Les montants ne se coupent pas : les largeurs de colonnes le permettent.
    blocs.push(tableau(&LARGEURS, entete, corps, "#e4e4e4"));

    let cumul = if permanent {
        j.conges.map(tableau_conges)
    } else {
        j.colonnes.filter(|c| !c.is_empty()).map(tableau_des_cumuls)
    };
    blocs.extend(cumul);
    let date_paiement = e.date_du_versement()?;
    // Un contrat sans numero (un apercu avant la validation) ne le dit pas.
    let contrat = if let Some(n) = e.numero.as_ref() { format!("Contrat n° {} — {}", n, nat.libelle) } else { format!("Contrat — {}", nat.libelle) };
    blocs.push(para(
        vec![mt(format!(
            "Plafond Urssaf de la période : {}{}SMIC retenu pour la réduction générale : {}{}Règlement le {} \
             par virement.\n{}, convention collective IDCC {}. Conservez ce bulletin sans limitation \
             de durée.",
            euros(p.plafond_urssaf),
            barre,
            smic_de_la_reduction(p)?,
            barre,
            date_paiement.jj_mm_aaaa(),
            contrat,
            conv.0
        ))],
        mesure!("6.2"),
        Some(GRIS_CLAIR),
        mesure!("5"),
        Mesure::ZERO,
    ));
    Ok(blocs)
}

// ------------------------------------------------------------ la description

/// Un chemin que la fabrique lit : sans `.` ni séparateur double.
pub(crate) fn chemin_net(p: &Path) -> PathBuf {
    p.components().collect::<PathBuf>()
}

/// `identite_de` : le fichier d'identite (`logo`, `signature`) de la compagnie dont `gabarits` est le dossier de
/// gabarits (`Contexte::gabarits`), ou `None` ; un fichier que le disque ne dit ni present ni absent est une erreur, pas
/// une absence.
pub fn identite_de(gabarits: &Path, nom: &str) -> R<Option<PathBuf>> {
    let dossier = gabarits.join("identite");
    for ext in IDENTITE_EXTENSIONS {
        let f = dossier.join(format!("{}{}", nom, ext));
        if f.try_exists().map_err(|e| socle::echec::du_disque(&f, &e).reclasser(socle::echec::Classe::Disque))? {
            return Ok(Some(f));
        }
    }
    Ok(None)
}

/// `description` : un document par jeu, chacun sur sa page ; au modele complet (la case du salarie, sinon celle de la
/// compagnie : `modele_du_bulletin`), l'annexe suit le bulletin, document a part (une page neuve).
pub fn description(jeux: &[Jeu], ent: &droit::contexte::EntrepriseDuDocument) -> R<Description> {
    let mut documents = Vec::new();
    for j in jeux {
        let modele = modele_du_bulletin(ent, &j.paie.salarie)?;
        documents.push(Document::Blocs(blocs_un(j, ent)?));
        if modele == ModeleBulletin::Complet {
            documents.push(Document::Blocs(blocs_annexe(j.paie, j.numero, &j.prevoyance)?));
        }
    }
    let marge = mesure!("42");
    Ok(Description {
        format: fabrique_pdf::FORMAT,
        page: Some(f::Page {
            largeur: f::A4_LARGEUR,
            hauteur: f::A4_HAUTEUR,
            marges: f::Marges { haut: marge, droite: marge, bas: marge, gauche: marge },
        }),
        police: Some(f::Police { taille: mesure!("7.4"), interligne: mesure!("1.2"), couleur: ARDOISE.to_string() }),
        style: None,
        logo: identite_de(&ent.gabarits, "logo")?.map(|logo| f::Logo {
            chemin: chemin_net(&logo),
            largeur: LOGO_LARGEUR,
            marge_haut: LOGO_MARGE_HAUT,
            marge_droite: LOGO_MARGE_DROITE,
        }),
        titre: None,
        documents,
        entete_page: None,
        pied_page: Some(f::Repere {
            gauche: None,
            centre: None,
            droite: Some(PIED_TEXTE.to_string()),
            taille: Some(mesure!("6")),
            couleur: Some(PIED_GRIS.to_string()),
            gras: false,
            distance: mesure!("24"),
            premiere_page: true,
            image: Some(f::ImageRepere {
                chemin: chemin_net(&socle::fichiers::dossier_du_code().join("puppetpaie-mot.svg")),
                largeur: PIED_MOT_LARGEUR,
                descente: PIED_MOT_DESCENTE,
            }),
        }),
    })
}

/// Les lignes de remuneration d'une paie qui s'impriment sous « REMBOURSEMENTS ET DEDUCTIONS DIVERSES » : hors assiette
/// et pas un revenu (la nature de la rubrique, `Rubrique::hors_assiette`, celle que le montant net social lit).
pub fn remboursements(ctx: &Contexte, p: &Paie) -> R<Vec<usize>> {
    let mut out = Vec::new();
    for (i, l) in p.remuneration.iter().enumerate() {
        if !l.soumis && droit::catalogue::rubrique(&ctx.racine, &l.code, &ctx.lue_au)?.hors_assiette()? != droit::catalogue::HorsAssiette::Revenu {
            out.push(i);
        }
    }
    Ok(out)
}

/// Les lignes de remuneration d'une paie qui sont un avantage en nature (la nature de leur rubrique) : elles se deduisent
/// sous « REMBOURSEMENTS ET DEDUCTIONS DIVERSES » (`Jeu::en_nature`).
pub fn en_nature(ctx: &Contexte, p: &Paie) -> R<Vec<usize>> {
    let mut out = Vec::new();
    for (i, l) in p.remuneration.iter().enumerate() {
        if droit::catalogue::rubrique(&ctx.racine, &l.code, &ctx.lue_au)?.nature == droit::catalogue::NatureDeRubrique::AvantageNature {
            out.push(i);
        }
    }
    Ok(out)
}

/// Ce que `bulletin.ecrire` prepare avant d'appeler la fabrique : la licence
/// resolue, le prelevement a la source, les cumuls et le compteur de conges, le
/// plafond ; les mois sans paie sont ecartes. `paies_du_mois` : TOUTES les
/// paies du mois quand on n'en imprime qu'une ; `borne_au_mois` : les cumuls
/// ne lisent que les contrats commences avant la fin du mois ; `rectificatifs` :
/// indice dans `paies` -> ce que le bloc rectificatif imprime ; `numeros` : le numero du bulletin de chaque paie, au
/// meme rang. `passe` : ce que la compagnie a declare jusqu'a `mois`, pour au moins les salaries au cachet de `paies`
/// (`passe_des_bulletins`), lu une fois pour tous les bulletins du geste.
pub fn jeux_et_description(
    ctx: &Contexte,
    paies: &[Paie],
    numeros: &[Option<NumeroDeBulletin>],
    ent: &droit::contexte::EntrepriseDuDocument,
    mois: Mois,
    paies_du_mois: Option<&[Paie]>,
    borne_au_mois: bool,
    rectificatifs: &HashMap<usize, Rectificatif>,
    departs: &engagement::departs::Departs,
    passe: &Passe,
) -> R<Description> {
    if numeros.len() != paies.len() {
        return Err(socle::refus!("{} numeros de bulletin pour {} paies.", numeros.len(), paies.len()));
    }
    let pas_par_paie = crate::prelevement::des_paies(ctx, paies, departs)?;
    let mut colonnes_par_salarie: HashMap<droit::contexte::IdSalarie, Vec<Colonne>> = HashMap::new();
    // Tout ce que la compagnie a declare jusqu'a ce mois, ce mois-ci compris : les paies
    // qu'on imprime remplacent le fichier de leur mois.
    let courantes = match paies_du_mois {
        Some(toutes) if !toutes.is_empty() => toutes,
        _ => paies,
    };
    let mut engagements = passe.engagements(Some(courantes))?;
    if borne_au_mois {
        let fin = mois.dernier_jour();
        engagements.retain(|e| e.periode.debut() <= fin);
    }
    for pa in paies {
        let cle = pa.salarie.id.clone();
        // Le compteur de conges est par contrat, ecrit dans la paie a la validation (`Paie::conges`) ; le cumul des
        // 507 h par salarie.
        if !pa.engagement.nat()?.mensuelle() && !colonnes_par_salarie.contains_key(&cle) {
            colonnes_par_salarie.insert(cle.clone(), tableau_cumul(ctx, &cle, mois, &engagements)?);
        }
    }
    let mut jeux = Vec::new();
    for (i, p) in paies.iter().enumerate() {
        if p.engagement.sans_paie() {
            continue;
        }
        let cle = &p.salarie.id;
        // Une paie qui n'est pas un mois sans paie porte son prelevement (`prelevement::des_paies`).
        let pas = pas_par_paie[i].as_ref().ok_or_else(|| socle::refus!("{} : la paie de {} n'a pas de prélèvement à la source.", cle, p.engagement.mois))?;
        jeux.push(Jeu {
            paie: p,
            numero: numeros[i].as_ref(),
            colonnes: colonnes_par_salarie.get(cle).map(Vec::as_slice),
            conges: p.conges.as_ref(),
            pas,
            rectificatif: rectificatifs.get(&i),
            remboursements: remboursements(ctx, p)?,
            en_nature: en_nature(ctx, p)?,
            prevoyance: engagement::calcul::garanties_de_prevoyance(ctx, p)?,
        });
    }
    if jeux.is_empty() {
        return Err(refus("Aucun bulletin a ecrire : toutes les paies sont des mois sans paie."));
    }
    description(&jeux, ent)
}

/// CE QUE LES BULLETINS DE `paies` CUMULENT (`jeux_et_description`) : ce que la compagnie a declare jusqu'a `mois`, pour
/// les seuls salaries au cachet de ces paies -- le cumul ne se tire que pour eux, seuls leurs engagements se relisent.
pub fn passe_des_bulletins<'a>(ctx: &'a Contexte, paies: &[Paie], mois: Mois, departs: &'a engagement::departs::Departs) -> R<Passe<'a>> {
    let mut au_cachet = std::collections::BTreeSet::new();
    for pa in paies {
        if !pa.engagement.nat()?.mensuelle() {
            au_cachet.insert(pa.salarie.id.clone());
        }
    }
    Ok(Passe::lire(ctx, Some(mois), Some(au_cachet), departs))
}


/// Le texte d'une case de la fiche, exigee : vide, elle se refuse en se nommant.
fn exige(c: &Option<String>, cle: &str) -> R<String> {
    c.ecrite(cle).map(str::to_string)
}

/// Le modele du bulletin que la fiche du salarie choisit, `None` si elle n'en choisit pas.
pub fn modele_du_salarie(sal: &FicheSalarie) -> R<Option<ModeleBulletin>> {
    Ok(sal.modele_bulletin.dite().copied())
}
