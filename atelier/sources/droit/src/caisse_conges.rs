//! Qui l'employeur declare a la caisse des Conges Spectacles.
//!
//! Les textes (deposes ; `atelier/reference/sources/caisse-conges-affiliation/LISEZ-MOI.md` dit ou, et ce qu'on en lit) :
//! - code du travail D. 7121-40 : les employeurs du spectacle « s'affilient, pour le personnel artistique et technique
//!   qu'ils emploient, a la caisse de conges payes » ; D. 7121-41 : ils lui « declarent [...] le personnel artistique
//!   et technique qu'ils n'ont pas employe de facon continue pendant les douze mois precedant la demande de conge »
//!   (releve LEGI du 10/10/2026, `code-du-travail/articles-20261010.md`) ;
//! - la fiche pratique Audiens « Les Conges Spectacles » (fevrier 2023) : la caisse sert les artistes et techniciens
//!   « quelle que soit [...] la nature du contrat de travail (CDD de droit commun, CDD d'usage, etc) », et la cotisation
//!   se calcule « sur chaque bulletin de paie d'artiste ou de technicien embauche sous contrat a duree determinee de
//!   moins de 12 mois » (`majoration-caisses-conges/audiens-fiche-pratique-conges-spectacles-202302.txt`, l. 15-31) ;
//! - CCN 3090, art. 10.1.1 (etendu) : le droit commun s'applique au CDD d'usage « occupe[] de maniere continue, dans
//!   le cadre d'un meme contrat de travail, pendant les 12 mois » ; CCNEAC (1285), art. IX.1 : D. 7121-41 repris.
//!
//! Ce que le moteur en fait : un CDDU de moins de douze mois (du debut au terme signes) est declare a la caisse, un
//! CDDU de douze mois ou plus ne l'est pas, des son premier bulletin ; un CDD de droit commun de moins de douze mois
//! d'un artiste ou d'un technicien du spectacle (`personnel_du_spectacle` : un metier de la nomenclature des emplois du
//! spectacle, que la liste de l'annexe 8 le nomme ou non -- D. 7121-40 et D. 7121-41 disent « le personnel artistique et
//! technique », Audiens « artistes et techniciens », sans liste) l'est ; un CDI ne l'est jamais. Ce qui n'est pas porte
//! (des contrats successifs qui, mis bout a bout, font douze mois continus ; un contrat prolonge au-dela de douze mois) :
//! aucun texte depose ne le tranche.
//!
//! Le CDDU d'un technicien que la liste de l'annexe 8 de la convention de l'employeur ne nomme pas
//! (`cddu_hors_des_annexes`) : permis (C. trav. D. 1242-1, 6° ; accord du 24/06/2008 de la 1285, art. 3.3 et annexe C,
//! `atelier/reference/sources/cddu-hors-annexe8/`), declare a la caisse comme tout CDDU, mais hors de l'annexe 8, qui ne
//! range que les fonctions de sa liste (art. 1er, § 2) : son chomage est celui du reglement general (4 %, sans part
//! salariale ni majoration du CDDU court, art. 50-1), recouvre par l'Urssaf (C. trav. L. 5427-1, e) : France Travail
//! ne recouvre que « lorsque l'activité exercée bénéficie de l'aménagement des conditions d'indemnisation »). La pile le
//! porte (`bareme::bareme_chomage_hors_des_annexes`).

use crate::natures::Nature;
use crate::fiche::{Lue, SalarieDuContrat};
use socle::date::{Jour, Periode, PeriodeOuverte};
use crate::emplois;
use socle::echec::R;

/// Ce que la pile du bareme decrit sans contrat : le CORPS est un intermittent affilie (le CDDU), la section
/// `par_nature.permanent` un salarie que l'employeur paie de ses conges (CDI, CDD de droit commun). C'est la valeur
/// que prennent les lectures qui n'ont pas de contrat.
pub fn par_defaut(nat: &Nature) -> bool {
    !nat.mensuelle()
}

/// Le jour ou un contrat commence a durer douze mois : son debut, douze mois plus tard (le dernier jour du mois quand
/// ce jour n'existe pas : un 29 fevrier). Un contrat dont le lendemain du terme l'atteint dure douze mois ou plus.
fn douze_mois_apres(debut: &Jour) -> R<Jour> {
    debut.apres(socle::date::Duree::mois(12))
}

/// Le `contrat`, du debut au terme signes, dure-t-il douze mois ou plus ?
pub fn douze_mois_ou_plus(contrat: &Periode) -> R<bool> {
    Ok(contrat.fin().apres(socle::date::Duree::jours(1))? >= douze_mois_apres(&contrat.debut())?)
}

/// LE CONTRAT D'UN INTERMITTENT. Un CDD d'usage (la nature payee a l'activite) l'est toujours ; un CDI jamais ; un CDD
/// de droit commun l'est quand l'annexe de l'assurance chomage range son salarie, dans les memes termes que pour le
/// CDDU :
/// - l'annexe 10 : « les artistes [...] engages au titre d'un contrat de travail a duree determinee » (art. 1er, § 2,
///   `atelier/reference/sources/plafond-intermittents/unedic-annexe10-convention-20241115.txt`, l. 15-17) : un metier
///   de l'annexe 10 de la nomenclature des emplois du spectacle ;
/// - l'annexe 8 : « les ouvriers et techniciens engages dans le cadre d'un contrat a duree determinee, pour assurer
///   l'une des fonctions enumerees dans la liste figurant au titre XIV, [...] dans les domaines d'activite ou dans les
///   entreprises enumeres dans la meme liste », domaines « definis par les numeros d'identifiant des conventions
///   collectives (IDCC) » (art. 1er, § 2, et art. 71, `unedic-annexe8-convention-20241115.txt`, l. 16-22 et 1530 s.) :
///   un metier du catalogue dont la liste de la convention de l'employeur nomme la fonction
///   (`emplois::Metier::nomme_par_l_annexe8` ; `IDCC_DE_L_EMPLOYEUR` sur la fiche). Un metier hors catalogue ne dit pas sur
///   quelle liste il est : non.
/// INFERENCE (ecrite) : l'employeur est dans le domaine de sa convention. Pour le spectacle vivant (1285, 3090), la
/// liste n'ajoute que des categories que remplit tout employeur de la convention qui fait jouer un spectacle (licencie,
/// ou organisateur occasionnel : art. 71, listes 6 et 7, l. 2949-2965). Pour les autres domaines, elle nomme aussi des
/// codes NAF (art. 71, listes 1 a 5 et 8 a 10), que le moteur ne confronte pas au code APE de la fiche Entreprise, pour
/// trois raisons : ces codes sont ceux de la NAF rev. 2, que la NAF 2025 remplace le 1er janvier 2027 sans
/// correspondance mecanique (decret 2025-736, art. 2 ; `atelier/reference/sources/insee-naf-2025/LISEZ-MOI.md` : 90.01Z
/// se scinde en 90.20Y et 90.39G) ; la liste les restreint par des mots qu'un code ne dit pas (« uniquement
/// animation », « uniquement studios de radio », l. 2907 et 3596-3598) ; et le code APE « ne saurait suffire à créer
/// des droits ou des obligations » (meme decret, art. 5, `legifrance-decret-2025-736-naf-2025-20250731.txt`, l. 30).
/// Ce qui en suit, et le lit ici : le code emploi de la nomenclature du spectacle en DSN (S21.G00.40.005, CCH-18 :
/// « 999SPT » n'est que pour les emplois permanents), l'attestation employeur Cinema Spectacle (`documents::aer::due`).
/// La caisse des Conges Spectacles ne suit pas la liste : `personnel_du_spectacle`.
pub fn intermittent(nat: &Nature, sal: &SalarieDuContrat) -> R<bool> {
    if nat.duree_indeterminee {
        return Ok(false);
    }
    if !nat.mensuelle() {
        return Ok(true);
    }
    // Un emploi permanent (999SPT), ou sans code, n'a pas de metier du catalogue.
    let code = sal.code_emploi_ft.dite().filter(|c| !c.est_permanent());
    let metier = match code {
        Some(code) => emplois::metier(code)?,
        None => None,
    };
    let range = |m: emplois::Metier| -> R<bool> {
        match m.annexe() {
            emplois::AnnexeChomage::X => Ok(true),
            emplois::AnnexeChomage::Viii => {
                let idcc = sal.idcc_de_l_employeur.as_ref().ok_or_else(|| {
                    socle::echec::refus("La convention collective de l'employeur n'est pas dite : la liste de l'annexe 8 qui nomme le métier en dépend (Configuration › Entreprise).")
                })?;
                Ok(m.nomme_par_l_annexe8(idcc))
            }
        }
    };
    Ok(metier.map(range).transpose()?.is_some_and(|oui| oui))
}

/// LE PERSONNEL ARTISTIQUE ET TECHNIQUE DU SPECTACLE, celui que l'employeur declare a la caisse des Conges Spectacles
/// (`affilie`) : « Une caisse de congés payés assure le service des congés annuels au personnel artistique et technique
/// employé de façon intermittente » (C. trav. D. 7121-38) ; les employeurs « s'affilient, pour le personnel artistique
/// et technique qu'ils emploient » (D. 7121-40) et le declarent (D. 7121-41 ;
/// `atelier/reference/sources/code-du-travail/articles-20261102.md`) ; Audiens : « artistes et techniciens [...] quelle
/// que soit [...] la nature du contrat de travail (CDD de droit commun, CDD d'usage, etc) »
/// (`majoration-caisses-conges/audiens-fiche-pratique-conges-spectacles-202302.txt`, l. 17-21). Aucun de ces textes ne
/// renvoie a la liste de l'annexe 8. Un CDD d'usage en est toujours ; un CDI jamais ; un CDD de droit commun, quand le
/// metier du salarie est un emploi de la nomenclature des emplois du spectacle, artistique (annexe 10) ou technique
/// (annexe 8), que la liste de la convention le nomme ou non. INFERENCE (ecrite) : un emploi de cette nomenclature est
/// du personnel artistique ou technique du spectacle ; un emploi permanent (999SPT) ou sans code n'en dit rien : non.
pub fn personnel_du_spectacle(nat: &Nature, sal: &SalarieDuContrat) -> R<bool> {
    if nat.duree_indeterminee {
        return Ok(false);
    }
    if !nat.mensuelle() {
        return Ok(true);
    }
    let Some(code) = sal.code_emploi_ft.dite().filter(|c| !c.est_permanent()) else {
        return Ok(false);
    };
    Ok(emplois::metier(code)?.is_some())
}

/// Le technicien que la liste de l'annexe 8 de la convention de l'employeur ne nomme pas, en CDD de droit commun ou en
/// CDD d'usage : son metier (le libelle du catalogue) et l'IDCC. Un metier de l'annexe 8 au catalogue se range par la
/// liste de la convention, qui doit etre dite (refus sinon, comme `intermittent`) ; un metier hors du catalogue ne dit
/// pas sur quelle liste il est, et un emploi sans code ou permanent n'a pas de metier : rien.
pub fn hors_de_l_annexe8(nat: &Nature, sal: &SalarieDuContrat) -> R<Option<(String, crate::fiche::Idcc)>> {
    if nat.duree_indeterminee {
        return Ok(None);
    }
    let Some(code) = sal.code_emploi_ft.dite().filter(|c| !c.est_permanent()) else {
        return Ok(None);
    };
    let Some(m) = emplois::metier(code)?.filter(|m| m.annexe() == emplois::AnnexeChomage::Viii && m.au_catalogue().is_some()) else {
        return Ok(None);
    };
    let idcc = sal.idcc_de_l_employeur.as_ref().ok_or_else(|| {
        socle::echec::refus("La convention collective de l'employeur n'est pas dite : la liste de l'annexe 8 qui nomme le métier en dépend (Configuration › Entreprise).")
    })?;
    if m.nomme_par_l_annexe8(idcc) {
        return Ok(None);
    }
    Ok(Some((m.libelle().to_string(), idcc.clone())))
}

/// LE CDD D'USAGE HORS DES ANNEXES : celui d'un technicien que la liste de l'annexe 8 de la convention ne nomme pas
/// (`hors_de_l_annexe8`). Le CDD d'usage lui est permis (C. trav. D. 1242-1, 6° « Les spectacles » ; pour la 1285,
/// l'accord du 24/06/2008, art. 3.3 et annexe C, qui nomme le poursuiteur, le staffeur, le bottier...,
/// `atelier/reference/sources/cddu-hors-annexe8/`), mais l'annexe 8 ne range que les fonctions de sa liste (art. 1er,
/// § 2) : son assurance chomage est celle du reglement general (4 %, art. 50-1 § 1er ; la majoration du CDD d'usage court
/// du § 2 ne vise que les dockers occasionnels ; aucune part salariale, art. 50), recouvree par l'Urssaf (L. 5427-1, e).
/// Le reste du CDDU ne change pas : la caisse des Conges Spectacles (`personnel_du_spectacle`), la formation des
/// intermittents de L. 6331-55 (« d'usage constant de ne pas recourir au contrat à durée indéterminée »), la
/// prevoyance de l'accord interbranche (art. 2.2 : la fonction reprise « dans la liste des emplois d'une des conventions
/// collectives du spectacle pour lesquels le recours au contrat à durée déterminée d'usage est autorisé »).
pub fn cddu_hors_des_annexes(nat: &Nature, sal: &SalarieDuContrat) -> R<bool> {
    Ok(!nat.mensuelle() && hors_de_l_annexe8(nat, sal)?.is_some())
}

/// L'URSSAF RECOUVRE-T-ELLE L'ASSURANCE CHOMAGE DE CE CONTRAT ? Celle d'un permanent (la pile `par_nature.permanent`)
/// et celle du CDD d'usage hors des annexes (`cddu_hors_des_annexes`) : oui ; celle d'un intermittent des annexes 8 et
/// 10, France Travail (C. trav. L. 5427-1, e). Le bordereau Urssaf en tire ses CTP 772, 937 et 668.
pub fn chomage_a_l_urssaf(nat: &Nature, sal: &SalarieDuContrat) -> R<bool> {
    Ok(nat.mensuelle() || cddu_hors_des_annexes(nat, sal)?)
}

/// L'employeur declare-t-il ce contrat a la caisse des Conges Spectacles ? `sal` est la fiche telle que la paie la lit
/// (`emplois::emploi_de` a la date du contrat).
/// - un CDI : non ;
/// - un CDD d'usage (CDDU) : oui, sauf s'il dure douze mois ou plus du debut au terme signes (le chargeur des
///   engagements pose toujours le terme d'un CDDU : sa derniere date ; sans terme, il est de moins de douze mois) ;
/// - un CDD de droit commun d'un artiste ou d'un technicien du spectacle (`personnel_du_spectacle`) : oui, sauf s'il
///   dure douze mois ou plus ; sans terme, refus -- sa duree decide, et le moteur ne la devine pas ;
/// - un CDD de droit commun d'un autre salarie : non.
pub fn affilie(nat: &Nature, sal: &SalarieDuContrat, contrat: &PeriodeOuverte) -> R<bool> {
    if !personnel_du_spectacle(nat, sal)? {
        return Ok(false);
    }
    let signe = match contrat.fermee() {
        Some(p) => p,
        None if !nat.mensuelle() => return Ok(true),
        None => {
            return Err(socle::refus!(
                "{} d'un artiste ou d'un technicien du spectacle commence le {} sans terme : la duree d'un contrat a duree determinee dit s'il est \
declare a la caisse des Conges Spectacles (moins de douze mois : code du travail D. 7121-41, fiche Audiens), et le \
moteur ne la devine pas.",
                nat.libelle,
                contrat.debut().jj_mm_aaaa()
            ))
        }
    };
    Ok(!douze_mois_ou_plus(&signe)?)
}
