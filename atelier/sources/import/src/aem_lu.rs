//! LE LECTEUR DES AEM EN PDF. L'attestation d'employeur mensuelle des annexes 8 et 10 que l'employeur remet a
//! l'intermittent pour chaque mois (« les attestations mensuelles nominatives afférentes à chaque salarié intermittent
//! rémunéré au cours du mois écoulé » : Unédic, circulaire 2016-25,
//! `atelier/reference/sources/unedic-circulaire-2016-25-annexes-8-10.txt`, l. 5094-5121) a la mise en page du
//! formulaire d'Unédic, quel que soit le logiciel qui l'edite.
//!
//! ELLE SE LIT PAR SES RUBRIQUES, PAS PAR DES POSITIONS SUR LA PAGE. Une rubrique (« N° SIRET », « Raison sociale »,
//! « Nom de famille », « NIR », « Date d'embauche »...) est une suite de mots sur une meme ligne ; sa valeur est ce qui
//! la suit a droite sur sa ligne, jusqu'a la rubrique suivante. Les cases cochees s'impriment par une marque (`MARQUES`)
//! juste a droite de leur libelle (« Artiste Ñ », « Non cadre Ñ », « AEM INITIALE Ñ »). LES NOMBRES DU MOIS se lisent
//! PAR COLONNE : les heures, les cachets isoles et groupes, les jours sont sous leurs en-tetes, sur deux rangs ; un
//! nombre va a l'en-tete dont il est le plus proche, a l'aplomb. Le brut est le nombre sous « SALAIRES BRUTS ».
//!
//! Ce que l'AEM porte et que l'import reprend :
//! - l'employeur : Siret, code APE, IDCC, raison sociale, code postal et commune, numero d'affiliation a la caisse des
//!   Congés Spectacles (dix chiffres et une lettre, `documents::conges_spectacles::faute_adherent`) ;
//! - le salarie : nom de famille, prenom, NIR et sa cle (verifiee), date de naissance, adresse, emploi, cadre ou non,
//!   artiste ou technicien (« Les salariés intermittents de l'annexe VIII sont les ouvriers et techniciens », l. 783 ;
//!   « Les salariés intermittents de l'annexe X sont les artistes du spectacle », l. 1846 : la case « Technicien » ou
//!   « Ouvrier » est l'annexe 8, la case « Artiste » l'annexe 10 ; la case « Réalisateur » ne dit pas de statut ici) ;
//! - le mois, le numero de l'attestation, le numero d'objet, la date d'embauche et la date de fin du contrat, les
//!   heures, les cachets, les jours travailles et le salaire brut du mois.
//!
//! UNE AEM DIT UN MOIS. Ses nombres et son brut sont ceux du mois de l'attestation (« RÉMUNERATION VERSÉES AU COURS DU
//! MOIS ») ; la periode qu'elle atteste est celle du contrat bornee a ce mois (un contrat « en cours » va jusqu'au
//! dernier jour du mois). C'est une inference, tiree du formulaire lui-meme.
//!
//! RIEN NE SE DEVINE (regle 1) : une AEM complementaire ou rectificative ne se reprend pas (elle corrige une attestation
//! que l'import ne voit pas) ; une AEM sans NIR, sans dates, sans ses colonnes de nombres ou sans brut n'est pas
//! reprise, et le refus dit ce qui manque. UN FORMULAIRE DONT LES LIBELLES NE SE LISENT PAS (polices sans table de
//! caracteres : les mots sortent en « � », ou en caracteres que rien ne dit) ne se lit pas : aucune rubrique n'ancre
//! ses valeurs, et une grille de positions relevee sur un exemplaire serait une position devinee ; l'import le dit et
//! demande la piece du meme mois qui se lit. Une case lue (sa valeur, une colonne du mois, le brut, une case a cocher)
//! qui porte un caractere que rien ne dit ne se lit pas : l'AEM le dit, en nommant la page, la police, le code et la
//! case.

use std::path::Path;

use socle::date::Jour;
use socle::echec::{refus, R};
use socle::mois::Mois;
use socle::num::entier_de_chiffres;
use socle::num::Decimal;
use fabrique_pdf::mesure::mesure;
use crate::releve::Mesure;

use crate::certificat_lu::{date_en_tete, jetons, nir_en_tete, Certificat};

/// Ce qui marque une case cochee, a droite de son libelle (« Ñ » : la coche d'une police sans table).
const MARQUES: [&str; 5] = ["Ñ", "✔", "✓", "☒", "X"];

/// Ce que dit une AEM qui ne se lit pas.
pub const ILLISIBLE: &str = "cette AEM ne se lit pas : ses libellés s'impriment en signes sans lettres (police sans table de caractères), rien ne dit quelle case porte quelle valeur ; déposez le bulletin ou le certificat Congés Spectacles du même mois";

/// Un mot de la page et sa boite (sa ligne de base si sa police ne dit pas sa hauteur : `hauteur_connue` faux).
#[derive(Clone, Debug)]
struct Mot {
    texte: String,
    x0: Mesure,
    x1: Mesure,
    y0: Mesure,
    y1: Mesure,
    hauteur_connue: bool,
}

impl Mot {
    fn cy(&self) -> Mesure {
        (self.y0 + self.y1) / 2
    }
    fn cx(&self) -> Mesure {
        (self.x0 + self.x1) / 2
    }
}

/// Une rubrique trouvee : les mots qui la font (rangs dans la page) et sa boite.
#[derive(Clone, Debug)]
struct Rubrique {
    cle: &'static str,
    mots: Vec<usize>,
    x0: Mesure,
    x1: Mesure,
    y0: Mesure,
    y1: Mesure,
    /// Chacun de ses mots a sa hauteur : sa ligne se borne par elle.
    hauteur_connue: bool,
}

impl Rubrique {
    fn cy(&self) -> Mesure {
        (self.y0 + self.y1) / 2
    }
}

/// Les rubriques du formulaire : (cle, jetons). Les plus longues d'abord : « N° d'affiliation à la caisse » n'est pas
/// « N° d'affiliation ». Celles qui ne se lisent pas ici bornent la valeur de leur voisine de ligne.
const RUBRIQUES: [(&str, &str); 37] = [
    ("fin", "date de fin du contrat de travail"),
    ("adherent", "n d affiliation a la caisse"),
    ("licence", "licence de spectacle n"),
    ("prestation", "4 prestation de travail"),
    ("authentification", "5 authentification"),
    ("idcc_prestation", "si different n idcc"),
    ("embauche", "date d embauche"),
    ("naissance", "date de naissance"),
    ("attestation_initiale", "n de l attestation initiale"),
    ("organisateur", "organisateur occasionnel"),
    ("ape", "code ape naf"),
    ("affiliation", "n d affiliation"),
    ("attestation", "attestation n"),
    ("nom", "nom de famille"),
    ("usage", "nom d usage"),
    ("emploi", "emploi occupe"),
    ("objet", "numero objet"),
    ("en_cours", "contrat en cours"),
    ("raison", "raison sociale"),
    ("code_postal", "code postal"),
    ("employeur", "2 employeur"),
    ("salarie", "3 salarie"),
    ("siret", "n siret"),
    ("idcc", "n idcc"),
    ("brut", "salaires bruts"),
    ("retraite", "regime de"),
    ("motif", "motif de"),
    ("prenom", "prenom"),
    ("nir", "nir"),
    ("adresse", "adresse"),
    ("commune", "commune"),
    ("telephone", "telephone"),
    ("courriel", "courriel"),
    ("mois", "mois"),
    ("sinon", "sinon"),
    ("cadre", "cadre"),
    ("heures", "heures"),
];

/// Les en-tetes des colonnes de nombres du mois.
const COLONNES: [(&str, &str); 4] = [("heures", "heures"), ("isoles", "isoles"), ("groupes", "groupes"), ("jours", "jours")];

/// Deux mots sont sur la meme ligne quand leurs milieux verticaux sont a moins de cela.
const LIGNE: Mesure = mesure("3.5");

/// `n` points.
const fn pt(n: i64) -> Mesure {
    Mesure::entier(n)
}

/// Ce qu'une AEM en PDF dit, page par page.
#[derive(Clone, Debug, PartialEq)]
pub struct Aem {
    pub page: usize,
    /// Le mois de l'attestation.
    pub mois: Option<Mois>,
    pub numero: Option<String>,
    pub siret: Option<droit::fiche::Siret>,
    pub ape: Option<String>,
    pub idcc: Option<droit::fiche::Idcc>,
    pub employeur: Option<String>,
    pub employeur_code_postal: Option<String>,
    pub employeur_commune: Option<String>,
    /// Le numero d'adherent a la caisse des Congés Spectacles, ecrit comme la fiche le garde.
    pub adherent: Option<String>,
    pub nir: Option<String>,
    pub cle_nir: Option<droit::fiche::CleNir>,
    pub nom: Option<String>,
    pub prenom: Option<String>,
    pub nom_usage: Option<String>,
    pub naissance: Option<Jour>,
    pub rue: Option<String>,
    pub code_postal: Option<String>,
    pub commune: Option<String>,
    pub emploi: Option<String>,
    pub numero_objet: Option<String>,
    pub cadre: Option<bool>,
    /// « artiste » (annexe 10) ou « technicien » (annexe 8).
    pub statut: Option<&'static str>,
    /// La date d'embauche et la date de fin du contrat, telles qu'imprimees.
    pub embauche: Option<Jour>,
    pub fin_contrat: Option<Jour>,
    pub en_cours: bool,
    pub heures: Option<i64>,
    pub cachets_isoles: Option<i64>,
    pub cachets_groupes: Option<i64>,
    pub jours: Option<i64>,
    pub brut: Option<Decimal>,
    /// « complementaire » ou « rectificative » quand la case est cochee.
    pub corrige: Option<&'static str>,
    /// Une periode d'une AER Cinéma Spectacle (`lire_aer`), qui atteste au meme rang qu'une AEM.
    pub aer: bool,
    /// Ce que la page dit d'elle-meme qui ne tient pas (une cle de NIR fausse, un nombre hors colonne).
    pub faute: Option<String>,
}

impl Aem {
    /// La periode que l'AEM atteste : le contrat borne au mois de l'attestation.
    pub fn periode(&self) -> Option<(Jour, Jour)> {
        let mois = self.mois?;
        let (premier, dernier) = (mois.premier_jour(), mois.dernier_jour());
        let debut = self.embauche?.max(premier);
        let fin = match (self.fin_contrat, self.en_cours) {
            (Some(f), _) => f.min(dernier),
            (None, true) => dernier,
            (None, false) => return None,
        };
        (debut <= fin).then_some((debut, fin))
    }

    /// Les cachets du mois : isoles et groupes.
    pub fn cachets(&self) -> i64 {
        self.cachets_isoles.into_iter().chain(self.cachets_groupes).sum()
    }

    /// Ce qui manque pour que l'AEM atteste un contrat ; `None` : rien.
    pub fn manque(&self) -> Option<String> {
        if let Some(c) = self.corrige {
            return Some(format!(
                "AEM {} : elle corrige une attestation initiale que l'import ne lit pas, elle ne se reprend pas seule ; déposez le bulletin du même mois",
                c
            ));
        }
        if let Some(f) = &self.faute {
            return Some(format!("AEM : {}", f));
        }
        let mut m: Vec<&str> = Vec::new();
        if self.nir.is_none() {
            m.push("le NIR");
        }
        if self.mois.is_none() {
            m.push("le mois");
        }
        if self.embauche.is_none() || (self.fin_contrat.is_none() && !self.en_cours) {
            m.push("les dates du contrat");
        }
        if self.heures.is_none() && self.cachets_isoles.is_none() && self.cachets_groupes.is_none() && self.jours.is_none() {
            m.push("les heures, cachets et jours");
        }
        if self.brut.is_none() {
            m.push("le salaire brut");
        }
        if m.is_empty() && self.periode().is_none() {
            m.push("une période dans le mois de l'attestation");
        }
        (!m.is_empty()).then(|| format!("AEM dont {} ne se lit pas", m.join(", ")))
    }

    /// Ce que l'AEM dit en commun avec un certificat d'emploi : l'employeur, le salarie, le contrat du mois.
    pub fn commun(&self) -> Certificat {
        let periode = self.periode();
        let mut adresse: Vec<String> = self.rue.iter().cloned().collect();
        if let (Some(cp), Some(c)) = (&self.code_postal, &self.commune) {
            adresse.push(format!("{} {}", cp, c));
        }
        Certificat {
            page: self.page,
            siret: self.siret.clone(),
            compte: self.adherent.clone(),
            employeur: self.employeur.clone(),
            numero: self.numero.clone(),
            nir: self.nir.clone(),
            cle_nir: self.cle_nir.clone(),
            nom: self.nom.clone(),
            prenom: self.prenom.clone(),
            nom_usage: self.nom_usage.clone(),
            pseudonyme: None,
            nom_complet: match (&self.prenom, &self.nom) {
                (Some(p), Some(n)) => Some(format!("{} {}", p, n)),
                _ => None,
            },
            adresse,
            emploi: self.emploi.clone(),
            cadre: self.cadre,
            debut: periode.map(|p| p.0),
            fin: periode.map(|p| p.1),
            nombre: None,
            base_conge: None,
            brut: self.brut,
            signe_le: None,
        }
    }
}

/// Une page d'AEM : lue, refusee (et ce qui s'en est lu), ou la notice du formulaire.
#[derive(Clone, Debug)]
pub enum Lecture {
    Lue(Aem),
    Refusee { sort: crate::reponse::Sort, raison: String, partiel: Option<Aem> },
    Notice,
}

#[derive(Clone, Debug)]
pub struct PageAem {
    pub page: usize,
    pub lecture: Lecture,
}

/// Les pages du PDF qui sont des AEM (ou leur notice), lues. Les autres pages n'y sont pas.
pub fn lire(chemin: &Path) -> R<Vec<PageAem>> {
    let r = crate::releve::du_pdf(chemin).map_err(crate::releve::Refus::en_echec)?;
    let pages: Vec<Vec<Mot>> = r.releve.pages.iter().map(mots_de).collect();
    let inconnus: Vec<Vec<Inconnu>> = r.releve.pages.iter().map(|p| p.inconnus.iter().map(Inconnu::de).collect()).collect();
    let notices: Vec<bool> = pages.iter().map(|m| est_notice(m)).collect();
    let mut out = Vec::new();
    for (i, mots) in pages.iter().enumerate() {
        let page = i + 1;
        if notices[i] {
            out.push(PageAem { page, lecture: Lecture::Notice });
        } else if notices.iter().any(|n| *n) && illisible(mots, &inconnus[i]) {
            out.push(PageAem { page, lecture: Lecture::Refusee { sort: crate::reponse::Sort::SansTexte, raison: ILLISIBLE.into(), partiel: None } });
        } else if let Some(a) = lire_page(mots, &inconnus[i], page) {
            let lecture = match a.manque() {
                Some(raison) => Lecture::Refusee { sort: crate::reponse::Sort::Incomplet, raison, partiel: Some(a) },
                None => Lecture::Lue(a),
            };
            out.push(PageAem { page, lecture });
        }
    }
    Ok(out)
}

/// L'AER CINEMA SPECTACLE, le formulaire que France Travail edite depuis la DSN (retour de nature 46, rangé par
/// `routes::documents::ranger_aer`), se lit PAR SES CHAMPS (`documents::aer::champs`), aux noms que porte le
/// formulaire (`envois::aer_fictive`, en tete) : l'employeur (`EMP_*`, `ENSEIGNE`), le salarie (`SALARIE_*`), l'emploi
/// (`CON_LIB_EMPLOI`), et au cadre 4, par colonne `nn`, une periode d'emploi dans un mois (`SAL_DT_DEB_nn`,
/// `SAL_DT_FIN_nn`, `SAL_NB_HEURES_nn`, `SAL_NB_CACHET_nn`, `SAL_MONTANT_nn`) et son contrat (`CON_NUMERO_nn`,
/// `CON_OBJET_nn`, dans la colonne de sa premiere periode). Chaque colonne atteste ce qu'une AEM du meme mois
/// attesterait : elle se lit en `Aem`, et prend le meme chemin (le registre des contrats attestes, le bulletin qui
/// l'emporte). `None` : ce PDF n'est pas une AER Cinéma Spectacle (un autre formulaire, l'AER du regime general
/// comprise, n'a pas ces champs). Un formulaire que la fabrique ne lit pas se refuse, avec sa raison.
///
/// Une case qui n'ecrit pas ce que sa rubrique attend (une date, un nombre) ne dit rien : l'AEM qu'elle fait dit alors
/// ce qui lui manque (`Aem::manque`).
pub fn lire_aer(octets: &[u8]) -> R<Option<Vec<PageAem>>> {
    let champs = match documents::aer::champs(octets) {
        Ok(c) => c,
        Err(documents::aer::Echec::Fabrique(e) | documents::aer::Echec::Autre(e)) => return Err(e),
        Err(documents::aer::Echec::Ecarts(l)) => return Err(refus(l.join(" ; "))),
    };
    let champ = |nom: &str| champs.iter().find(|c| c.nom == nom);
    let v = |nom: &str| champ(nom).and_then(|c| c.valeur.texte()).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    if champ("SALARIE_NIR").is_none() || champ("SAL_DT_DEB_01").is_none() {
        return Ok(None);
    }
    let jour = |nom: &str| v(nom).and_then(|t: String| socle::piece::Zone::lue(&t, socle::piece::format::JourEcrit).valeur());
    let entier = |nom: &str| v(nom).and_then(|t| entier_de_chiffres(&t.replace([' ', '\u{a0}'], "")));
    let nir = v("SALARIE_NIR").map(|n| n.replace(' ', ""));
    let chiffres = |nom: &str| v(nom).and_then(|t: String| entier_de_chiffres(&t));
    let naissance = match (chiffres("SALARIE_DT_NAISSANCE_A"), chiffres("SALARIE_DT_NAISSANCE_M"), chiffres("SALARIE_DT_NAISSANCE_J")) {
        (Some(a), Some(m), Some(j)) => match (i32::try_from(a), u32::try_from(m), u32::try_from(j)) {
            (Ok(a), Ok(m), Ok(j)) => Jour::nouveau(a, m, j),
            _ => None,
        },
        _ => None,
    };
    // UN SIRET OU UNE CLE DU NIR QUI N'ONT PAS LEUR FORME se disent, et la page se refuse (`Aem::manque`) ; la cle
    // imprimee se garde quand c'est celle du NIR (`declarations::champs::cle_nir`), comme sur une AEM (`lire_page`).
    let mut fautes: Vec<String> = Vec::new();
    let siret = match v("EMP_SIRET").map(|s| droit::fiche::Siret::lire(&s)).transpose() {
        Ok(s) => s,
        Err(e) => {
            fautes.push(e.phrase);
            None
        }
    };
    let cle_nir = match nir.as_ref().and_then(|n| n.get(13..)).filter(|k| !k.is_empty()).map(droit::fiche::CleNir::lire).transpose() {
        Ok(Some(k)) => match nir.as_ref().and_then(|n| n.get(..13)).and_then(declarations::champs::cle_nir).filter(|c| *c == k) {
            Some(c) => Some(c),
            None => {
                fautes.push("la clé du NIR imprimé ne correspond pas à son numéro".to_string());
                None
            }
        },
        Ok(None) => None,
        Err(e) => {
            fautes.push(e.phrase);
            None
        }
    };
    let idcc = match v("EMP_IDCC").map(|i| droit::fiche::Idcc::lire(&i)).transpose() {
        Ok(i) => i,
        Err(e) => {
            fautes.push(e.phrase);
            None
        }
    };
    let faute = (!fautes.is_empty()).then(|| fautes.join(" ; "));
    let mut out = Vec::new();
    let mut numero_objet = None;
    for rang in 1..=5 {
        let nom = |p: &str| format!("{}{:02}", p, rang);
        // Un contrat se dit dans la colonne de sa premiere periode ; la colonne sans contrat en est la suite.
        if v(&nom("CON_NUMERO_")).is_some() {
            numero_objet = v(&nom("CON_OBJET_"));
        }
        let (Some(debut), Some(fin)) = (jour(&nom("SAL_DT_DEB_")), jour(&nom("SAL_DT_FIN_"))) else { continue };
        let page = champ(&nom("SAL_DT_DEB_")).and_then(|c| c.page).ok_or_else(|| socle::refus!("la case {} de l'AER n'est sur aucune page", nom("SAL_DT_DEB_")))?;
        let a = Aem {
            page,
            mois: Some(debut.mois_civil()),
            numero: None,
            siret: siret.clone(),
            ape: v("EMP_APE"),
            idcc: idcc.clone(),
            employeur: v("ENSEIGNE"),
            employeur_code_postal: v("ADR_EMP_CP"),
            employeur_commune: v("ADR_EMP_LOCALITE"),
            adherent: None,
            cle_nir: cle_nir.clone(),
            nir: nir.as_ref().and_then(|n| n.get(..13)).map(str::to_string),
            nom: v("SALARIE_NOM_FAMILLE"),
            prenom: v("SALARIE_PRENOMS"),
            nom_usage: None,
            naissance,
            rue: v("SALARIE_ADRESSE"),
            code_postal: v("SALARIE_CP"),
            commune: v("SALARIE_ADR_LOCALITE"),
            emploi: v("CON_LIB_EMPLOI"),
            numero_objet: numero_objet.clone(),
            cadre: None,
            statut: None,
            embauche: Some(debut),
            fin_contrat: Some(fin),
            en_cours: false,
            heures: entier(&nom("SAL_NB_HEURES_")),
            cachets_isoles: entier(&nom("SAL_NB_CACHET_")),
            cachets_groupes: None,
            jours: None,
            brut: v(&nom("SAL_MONTANT_")).and_then(|t| socle::num::decimal_a_virgule(&t)),
            corrige: None,
            aer: true,
            faute: faute.clone(),
        };
        let lecture = match a.manque() {
            Some(raison) => Lecture::Refusee { sort: crate::reponse::Sort::Incomplet, raison: raison.replacen("AEM", "AER", 1), partiel: Some(a) },
            None => Lecture::Lue(a),
        };
        out.push(PageAem { page, lecture });
    }
    Ok(Some(out))
}

fn mots_de(p: &crate::releve::PageRelevee) -> Vec<Mot> {
    let mut mots = Vec::new();
    for m in &p.mots {
        let texte = m.texte.trim().to_string();
        if texte.is_empty() {
            continue;
        }
        let [x0, y0, x1, y1] = m.rect.connue();
        mots.push(Mot { texte, x0, y0, x1, y1, hauteur_connue: m.rect.boite().is_some() });
    }
    mots
}

/// Les jetons de la page, a la suite (dans l'ordre de lecture), pour reconnaitre ce qu'elle est.
fn texte_en_jetons(mots: &[Mot]) -> String {
    let mut m: Vec<&Mot> = mots.iter().collect();
    m.sort_by(|a, b| a.y0.cmp(&b.y0).then(a.x0.cmp(&b.x0)));
    format!(" {} ", m.iter().flat_map(|w| jetons(&w.texte)).collect::<Vec<_>>().join(" "))
}

/// La notice du formulaire (« RAPPEL DES OBLIGATIONS DU SALARIE », « l'attestation mensuelle ») : rien a reprendre.
fn est_notice(mots: &[Mot]) -> bool {
    let t = texte_en_jetons(mots);
    t.contains(" rappel des obligations du salarie ") && t.contains(" attestation mensuelle ")
}

/// Une page dont la moitie des mots au moins sortent en signes sans lettres (U+FFFD, ou un caractere que rien ne dit,
/// qui borne un mot : chacun compte pour un).
fn illisible(mots: &[Mot], inconnus: &[Inconnu]) -> bool {
    let sans_lettres = mots.iter().filter(|m| m.texte.contains('\u{FFFD}')).count() + inconnus.len();
    sans_lettres > 0 && sans_lettres * 2 >= mots.len() + inconnus.len()
}

/// Un caractere que rien ne dit, et sa boite.
struct Inconnu {
    releve: crate::releve::InconnuReleve,
    x0: Mesure,
    x1: Mesure,
    y0: Mesure,
    y1: Mesure,
}

impl Inconnu {
    fn de(u: &crate::releve::InconnuReleve) -> Inconnu {
        let [x0, y0, x1, y1] = u.rect.connue();
        Inconnu { releve: u.clone(), x0, x1, y0, y1 }
    }
    fn cy(&self) -> Mesure {
        (self.y0 + self.y1) / 2
    }
    fn cx(&self) -> Mesure {
        (self.x0 + self.x1) / 2
    }
}

/// Les mots sur la ligne de `m`, a sa droite, de gauche a droite (lui compris).
fn ligne_depuis(mots: &[Mot], m: usize) -> Vec<usize> {
    let cy = mots[m].cy();
    let mut v: Vec<usize> = (0..mots.len()).filter(|&k| (mots[k].cy() - cy).abs() <= LIGNE && mots[k].x0 >= mots[m].x0).collect();
    v.sort_by(|a, b| mots[*a].x0.cmp(&mots[*b].x0));
    v
}

/// Les rubriques de la page. Un mot ne sert qu'a une rubrique ; les plus longues se cherchent d'abord.
fn rubriques(mots: &[Mot]) -> Vec<Rubrique> {
    let mut prises = vec![false; mots.len()];
    let mut out: Vec<Rubrique> = Vec::new();
    let mut ordre: Vec<usize> = (0..mots.len()).collect();
    ordre.sort_by(|a, b| mots[*a].y0.cmp(&mots[*b].y0).then(mots[*a].x0.cmp(&mots[*b].x0)));
    for (cle, phrase) in RUBRIQUES {
        let p: Vec<&str> = phrase.split(' ').collect();
        for &m in &ordre {
            if prises[m] || jetons(&mots[m].texte).first().map(String::as_str) != Some(p[0]) {
                continue;
            }
            // Les jetons de la ligne, chacun avec son mot ; la rubrique commence au premier jeton du mot.
            let ligne = ligne_depuis(mots, m);
            let js: Vec<(String, usize)> = ligne.iter().flat_map(|&k| jetons(&mots[k].texte).into_iter().map(move |j| (j, k))).collect();
            if js.len() < p.len() || !p.iter().enumerate().all(|(i, x)| js[i].0 == *x) {
                continue;
            }
            let mut faits: Vec<usize> = js[..p.len()].iter().map(|j| j.1).collect();
            faits.dedup();
            // Les mots d'une rubrique se suivent (pas de valeur entre eux), et ne finissent pas au milieu d'un mot.
            let fin_dans_le_mot = js.get(p.len()).is_some_and(|j| j.1 == *faits.last().unwrap());
            let serres = faits.windows(2).all(|w| mots[w[1]].x0 - mots[w[0]].x1 < pt(15));
            if fin_dans_le_mot || !serres || faits.iter().any(|k| prises[*k]) {
                continue;
            }
            for k in &faits {
                prises[*k] = true;
            }
            let b = |f: fn(&Mot) -> Mesure, min: bool| faits.iter().map(|k| f(&mots[*k])).fold(if min { Mesure::MAX } else { Mesure::MIN }, |a, x| if min { a.min(x) } else { a.max(x) });
            out.push(Rubrique { cle, x0: b(|w| w.x0, true), x1: b(|w| w.x1, false), y0: b(|w| w.y0, true), y1: b(|w| w.y1, false), hauteur_connue: faits.iter().all(|k| mots[*k].hauteur_connue), mots: faits });
        }
    }
    out
}

/// La page : `None` si ce n'est pas une AEM (son titre, les annexes 8 et 10, ses rubriques de contrat).
fn lire_page(mots: &[Mot], inconnus: &[Inconnu], page: usize) -> Option<Aem> {
    let t = texte_en_jetons(mots);
    let titre = t.contains(" attestation aem ") || t.contains(" attestation d employeur mensuelle ") || t.contains(" attestation employeur mensuelle ");
    let annexes = t.contains(" annexes 8 et 10 ") || t.contains(" annexes viii et x ");
    if !titre || !annexes || !t.contains(" salaires bruts ") || !t.contains(" date d embauche ") {
        return None;
    }
    let rs = rubriques(mots);
    let pris: std::collections::HashSet<usize> = rs.iter().flat_map(|r| r.mots.iter().copied()).collect();
    let marques: Vec<usize> = (0..mots.len()).filter(|&k| MARQUES.contains(&mots[k].texte.as_str())).collect();
    // Les sections du formulaire, par leur titre : 2 EMPLOYEUR, 3 SALARIE, 4 PRESTATION DE TRAVAIL, 5 AUTHENTIFICATION.
    let haut_de = |cle: &str| rs.iter().find(|r| r.cle == cle).map(|r| r.y0);
    let bornes = [haut_de("employeur"), haut_de("salarie"), haut_de("prestation"), haut_de("authentification")];
    let section = |y: Mesure| -> usize { bornes.iter().filter(|b| b.is_some_and(|b| y >= b - pt(1))).count() };

    // Le premier caractere que rien ne dit d'une case lue : la case ne se lit pas, l'AEM le dit (ISO 32000-1, 9.10.2).
    // Une case dont la rubrique n'a pas de hauteur (sa police ne la dit pas) ne se lit pas non plus : sa ligne se borne
    // par cette hauteur.
    let inconnu: std::cell::RefCell<Option<String>> = std::cell::RefCell::new(None);
    let noter_phrase = |phrase: String| {
        let mut i = inconnu.borrow_mut();
        if i.is_none() {
            *i = Some(phrase);
        }
    };
    let noter = |u: &Inconnu, case: &str| noter_phrase(u.releve.phrase(page, case));
    // La valeur d'une rubrique : les mots a sa droite, sur sa ligne, jusqu'a la rubrique suivante de la ligne.
    let valeur_de = |r: &Rubrique| -> Vec<String> {
        if !r.hauteur_connue {
            noter_phrase(format!(
                "page {} : la case « {} » est écrite dans une police qui ne dit pas la hauteur de ses glyphes (ni son programme, ni ses métriques, ni son descripteur : ISO 32000-1, table 122) : sa ligne ne se borne pas",
                page, r.cle
            ));
            return Vec::new();
        }
        let tol = (r.y1 - r.y0).max(pt(4)) * mesure("0.6");
        let suivante = rs.iter().filter(|o| o.x0 > r.x1 && (o.cy() - r.cy()).abs() <= tol).map(|o| o.x0).fold(Mesure::MAX, Mesure::min);
        if let Some(u) = inconnus.iter().find(|u| u.x1 >= r.x0 && u.x0 < suivante && (u.cy() - r.cy()).abs() <= tol) {
            noter(u, r.cle);
        }
        let mut v: Vec<usize> = (0..mots.len())
            .filter(|&k| !pris.contains(&k) && !marques.contains(&k) && mots[k].x0 >= r.x1 - pt(1) && mots[k].x0 < suivante && (mots[k].cy() - r.cy()).abs() <= tol)
            .collect();
        v.sort_by(|a, b| mots[*a].x0.cmp(&mots[*b].x0));
        v.into_iter().map(|k| mots[k].texte.clone()).collect()
    };
    // Les mots de la premiere rubrique `cle` de la section `s` (toutes si `None`) qui a une valeur ; aucun s'il n'y en a pas.
    let lu = |cle: &str, s: Option<usize>| -> Vec<String> {
        rs.iter().filter(|r| r.cle == cle && s.is_none_or(|s| section(r.y0) == s)).map(&valeur_de).find(|v| !v.is_empty()).into_iter().flatten().collect()
    };
    let texte = |v: Vec<String>| -> Option<String> {
        let t = v.join(" ").trim().to_string();
        (!t.is_empty()).then_some(t)
    };

    // Une AEM (pas une AER) dont chaque case se lit ci-dessous ; la case « en cours » n'est cochee que lue.
    let mut a = Aem {
        page,
        mois: None,
        numero: None,
        siret: None,
        ape: None,
        idcc: None,
        employeur: None,
        employeur_code_postal: None,
        employeur_commune: None,
        adherent: None,
        nir: None,
        cle_nir: None,
        nom: None,
        prenom: None,
        nom_usage: None,
        naissance: None,
        rue: None,
        code_postal: None,
        commune: None,
        emploi: None,
        numero_objet: None,
        cadre: None,
        statut: None,
        embauche: None,
        fin_contrat: None,
        en_cours: false,
        heures: None,
        cachets_isoles: None,
        cachets_groupes: None,
        jours: None,
        brut: None,
        corrige: None,
        aer: false,
        faute: None,
    };
    // Le mois : ses chiffres, imprimes case a case (« 0 4 20 2 3 ») : MM puis AAAA.
    let chiffres: String = lu("mois", Some(0)).iter().take_while(|m| m.chars().all(|c| c.is_ascii_digit())).map(String::as_str).collect();
    a.mois = socle::piece::Zone::lue(&chiffres, socle::piece::format::Mmaaaa).valeur();
    a.numero = lu("attestation", Some(0)).first().cloned();
    let siret: String = lu("siret", Some(1)).concat().chars().filter(char::is_ascii_digit).collect();
    // Un SIRET ou un IDCC imprimés qui n'ont pas leur forme se disent (`a.faute`), comme la clé du NIR plus bas.
    let mut fautes_d_identite: Vec<String> = Vec::new();
    a.siret = match (!siret.is_empty()).then(|| droit::fiche::Siret::lire(&siret)).transpose() {
        Ok(s) => s,
        Err(e) => {
            fautes_d_identite.push(e.phrase);
            None
        }
    };
    a.ape = lu("ape", Some(1)).first().cloned();
    a.idcc = match lu("idcc", Some(1)).first().map(|x| droit::fiche::Idcc::lire(x)).transpose() {
        Ok(i) => i,
        Err(e) => {
            fautes_d_identite.push(e.phrase);
            None
        }
    };
    a.employeur = texte(lu("raison", Some(1)));
    a.employeur_code_postal = lu("code_postal", Some(1)).first().filter(|x| x.len() == 5 && x.chars().all(|c| c.is_ascii_digit())).cloned();
    a.employeur_commune = texte(lu("commune", Some(1)));
    a.adherent = lu("adherent", Some(1))
        .iter()
        .map(|x| documents::conges_spectacles::forme_adherent(x))
        .find(|x| documents::conges_spectacles::faute_adherent(x).is_none());

    a.nom = texte(lu("nom", Some(2)));
    a.prenom = texte(lu("prenom", Some(2)));
    a.nom_usage = texte(lu("usage", Some(2)));
    let mut fautes: Vec<&str> = Vec::new();
    if let Some((nir, cle, _)) = nir_en_tete(&lu("nir", Some(2))) {
        // La cle imprimee, quand c'est celle du NIR (`declarations::champs::cle_nir`).
        match &cle {
            Some(k) => match declarations::champs::cle_nir(&nir).filter(|c| **c == **k) {
                Some(c) => {
                    a.nir = Some(nir);
                    a.cle_nir = Some(c);
                }
                None => fautes.push("la clé du NIR imprimé ne correspond pas à son numéro"),
            },
            None => a.nir = Some(nir),
        }
    }
    a.naissance = date_en_tete(&lu("naissance", Some(2))).map(|d| d.0);
    a.rue = texte(lu("adresse", Some(2)));
    a.code_postal = lu("code_postal", Some(2)).first().filter(|x| x.len() == 5 && x.chars().all(|c| c.is_ascii_digit())).cloned();
    a.commune = texte(lu("commune", Some(2)));

    a.emploi = texte(lu("emploi", Some(3)));
    a.numero_objet = lu("objet", Some(3)).first().cloned();
    a.embauche = date_en_tete(&lu("embauche", Some(3))).map(|d| d.0);
    a.fin_contrat = date_en_tete(&lu("fin", Some(3))).map(|d| d.0);

    // Les cases cochees : le mot dont la marque suit le bord droit.
    for &k in &marques {
        let m = &mots[k];
        let Some(l) = (0..mots.len())
            .filter(|&j| j != k && mots[j].x1 <= m.x0 + pt(1) && m.x0 - mots[j].x1 <= pt(12) && (mots[j].cy() - m.cy()).abs() <= pt(5))
            .min_by(|a, b| (m.x0 - mots[*a].x1).cmp(&(m.x0 - mots[*b].x1)))
        else {
            continue;
        };
        let Some(lib) = jetons(&mots[l].texte).last().cloned() else { continue };
        let s = section(mots[l].y0);
        match (s, lib.as_str()) {
            (0, "complementaire") => a.corrige = Some("complémentaire"),
            (0, "positive" | "negative") => a.corrige = Some("rectificative"),
            (3, "artiste") => a.statut = Some("artiste"),
            (3, "technicien" | "ouvrier") => a.statut = Some("technicien"),
            (3, "cours") => a.en_cours = true,
            (3, "cadre") => {
                // « Non cadre » : « Non » juste avant, ou juste au-dessus (le libelle sur deux rangs).
                let w = &mots[l];
                let non = mots.iter().any(|o| {
                    jetons(&o.texte) == ["non"]
                        && (((o.cy() - w.cy()).abs() <= LIGNE && w.x0 - o.x1 >= pt(0) && w.x0 - o.x1 < pt(8))
                            || ((o.x0 - w.x0).abs() <= pt(3) && w.cy() - o.cy() > pt(2) && w.cy() - o.cy() < pt(12)))
                });
                a.cadre = Some(!non);
            }
            _ => {}
        }
    }

    // Une case a cocher dont la marque est un caractere que rien ne dit (une marque d'une police de symboles) : cochee
    // ou non, rien ne le dit.
    for u in inconnus {
        let l = (0..mots.len())
            .filter(|&j| mots[j].x1 <= u.x0 + pt(1) && u.x0 - mots[j].x1 <= pt(12) && (mots[j].cy() - u.cy()).abs() <= pt(5))
            .min_by(|a, b| (u.x0 - mots[*a].x1).cmp(&(u.x0 - mots[*b].x1)));
        if let Some(lib) = l.and_then(|l| jetons(&mots[l].texte).last().cloned()) {
            if matches!(lib.as_str(), "complementaire" | "positive" | "negative" | "artiste" | "technicien" | "ouvrier" | "cours" | "cadre") {
                noter(u, &lib);
            }
        }
    }

    // Les nombres du mois, par colonne : sous les en-tetes, au-dessus de « SALAIRES BRUTS ».
    let en_tetes: Vec<(&str, &Mot)> = COLONNES
        .iter()
        .filter_map(|(c, j)| mots.iter().find(|m| section(m.y0) == 3 && jetons(&m.texte).first().map(String::as_str) == Some(*j)).map(|m| (*c, m)))
        .collect();
    let brut_tete = rs.iter().filter(|r| r.cle == "brut" && section(r.y0) == 3).min_by(|a, b| a.x0.cmp(&b.x0));
    if !en_tetes.is_empty() {
        let haut = en_tetes.iter().map(|(_, m)| m.y1).fold(Mesure::MIN, Mesure::max);
        let mut par_colonne: std::collections::BTreeMap<&str, Vec<i64>> = std::collections::BTreeMap::new();
        let mut ambigu = false;
        for u in inconnus.iter().filter(|u| u.y0 >= haut && !brut_tete.is_some_and(|r| u.y1 > r.y0) && section(u.y0) == 3) {
            if let Some((c, _)) = en_tetes.iter().map(|(c, t)| (*c, (t.cx() - u.cx()).abs())).filter(|x| x.1 <= pt(45)).min_by(|a, b| a.1.cmp(&b.1)) {
                noter(u, c);
            }
        }
        for (k, m) in mots.iter().enumerate() {
            if pris.contains(&k) || m.y0 < haut || brut_tete.is_some_and(|r| m.y1 > r.y0) {
                continue;
            }
            let Some(n) = entier(&m.texte) else { continue };
            match en_tetes.iter().map(|(c, t)| (*c, (t.cx() - m.cx()).abs())).filter(|x| x.1 <= pt(45)).min_by(|a, b| a.1.cmp(&b.1)) {
                Some((c, _)) => par_colonne.entry(c).or_default().push(n),
                None => ambigu = true,
            }
        }
        if ambigu || par_colonne.values().any(|v| v.len() > 1) {
            fautes.push("un nombre du mois ne tombe sous aucune colonne, ou une colonne en porte deux");
        } else {
            // Une colonne du formulaire sans nombre ne compte rien : son total est la somme de ce qu'elle porte (aucun
            // nombre, zero). C'est une inference, tiree du formulaire lui-meme.
            let v = |c: &str| -> i64 { par_colonne.get(c).into_iter().flatten().sum() };
            let a_l_en_tete = |c: &str| en_tetes.iter().any(|x| x.0 == c);
            if a_l_en_tete("heures") && a_l_en_tete("jours") && (a_l_en_tete("isoles") || a_l_en_tete("groupes")) {
                a.heures = Some(v("heures"));
                a.cachets_isoles = Some(v("isoles"));
                a.cachets_groupes = Some(v("groupes"));
                a.jours = Some(v("jours"));
            }
        }
    }
    // Le brut : le nombre sous « SALAIRES BRUTS », a son aplomb.
    if let Some(r) = brut_tete {
        let cx = (r.x0 + r.x1) / 2;
        if let Some(u) = inconnus.iter().find(|u| u.y0 > r.y1 && u.y0 < r.y1 + pt(40) && (u.cx() - cx).abs() <= (r.x1 - r.x0) / 2 + pt(10)) {
            noter(u, "salaires bruts");
        }
        a.brut = mots
            .iter()
            .filter(|m| m.y0 > r.y1 && m.y0 < r.y1 + pt(40) && (m.cx() - cx).abs() <= (r.x1 - r.x0) / 2 + pt(10))
            .filter_map(|m| crate::bulletin_lu::lire_nombre(&m.texte).filter(|_| m.texte.chars().any(|c| c.is_ascii_digit())).map(|n| (m.y0, n)))
            .min_by(|a, b| a.0.cmp(&b.0))
            .map(|x| x.1);
    }
    a.faute = match inconnu.into_inner() {
        Some(i) => Some(i),
        None => fautes_d_identite.first().cloned().or_else(|| fautes.first().map(|f| f.to_string())),
    };
    Some(a)
}

/// Un nombre entier du mois (« 1 », « 12 », « 8,00 ») ; `None` pour ce qui n'en est pas un.
fn entier(t: &str) -> Option<i64> {
    if !t.chars().all(|c| c.is_ascii_digit() || c == ',') || !t.chars().next()?.is_ascii_digit() {
        return None;
    }
    match t.split_once(',') {
        Some((e, d)) if d.chars().all(|c| c == '0') => entier_de_chiffres(e),
        Some(_) => None,
        None => entier_de_chiffres(t),
    }
}
