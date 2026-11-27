//! LES DROITS D'UN INTERMITTENT DEPUIS SES PIECES (`droits-des-pieces`) : une route de l'installation, sans compagnie.
//! La personne pose dans un seul dossier, `mes-pieces/` de l'installation (cree vide s'il manque, comme la boite de
//! l'import), les pieces de tous ses employeurs : bulletins en PDF, AEM en PDF ou deposees. La route les lit par les
//! lecteurs de l'import (`import::tri`, `bulletin_lu`, `aem_lu`, `dsn_aem`), PAR PERSONNE (le NIR), jamais en melant deux
//! personnes : plusieurs NIR, et la route fait choisir la personne (une case a choix), puis ne montre qu'elle. Une
//! personne choisie qu'aucune piece ne porte plus ne compte pas, et la route le dit ; sa case reste, pour la vider.
//!
//! UN CONTRAT NE S'ATTESTE QU'UNE FOIS, CHEZ SON EMPLOYEUR : le registre de l'import (`import::attestes::Registre`)
//! tient les contrats d'un employeur (son Siren, sinon sa raison sociale), un registre par employeur : deux employeurs
//! qui paient le meme jour font deux contrats, jamais un. Une AEM et le bulletin du meme contrat n'en font qu'un, et le
//! bulletin l'emporte (`Registre::ajouter`), comme a l'import d'une compagnie.
//!
//! LES LIGNES PREVUES : la charge peut porter des contrats a venir, saisis a la main (`LignePrevue` : leur fin, des
//! cachets ou des heures, leur nature, un brut facultatif) ; chacune se compte comme une paie, et rien ne s'ecrit. Sans
//! brut, elle compte pour les heures, pas pour l'allocation. Sans piece qui porte une personne, les lignes se comptent
//! seules (« simple pour les simples » : on pose ses bulletins, ou l'on remplit des lignes).
//!
//! L'ANNEXE (artiste, annexe X ; technicien, annexe VIII) : CHAQUE PAIE A LA SIENNE. Une paie des pieces, celle que sa
//! piece dit (la case d'une AEM, le statut ou l'emploi d'un bulletin) ou ses cachets ; une ligne prevue, sa nature
//! (`NatureDUneLigne` : artiste, technicien, enseignement, formation). Une paie qui ne la dit pas prend celle de base :
//! l'annexe de la personne, quand ses paies n'en disent qu'une (`AnnexesDesPaies`). Les heures des deux annexes
//! s'additionnent pour les 507 heures, et L'ANNEXE APPLIQUEE EST CELLE QUI A LE PLUS D'HEURES dans les 12 mois : toute
//! l'activite compte sous elle (annexes VIII et X du 15/11/2024, art. 65 §9 :
//! `plafond-intermittents/unedic-annexe10-convention-20241115.txt` l. 1545-1549, `…annexe8…` l. 1509-1513 ; circulaire
//! Unedic 2016-25, fiche 1, § 2.3.1, `unedic-circulaire-2016-25-annexes-8-10.txt` l. 2623-2674).
//!
//! LES CASES : la route rend les cases de sa charge telles qu'on les saisit (`cases` : libelle, forme, valeur, choix,
//! parties des rangees), chacune tiree de la declaration de son champ ; la page de puppetpaie.fr les dessine, et qui lit
//! la reponse en ligne de commande voit les memes.
//!
//! LE PLAFOND D'UNE PAIE, pour la limite du salaire de reference (art. 11 §3) : quand il ne se connait pas, la paie compte
//! entiere (Leandre, 27/11/2026 : « L'estimation sort toujours ») ; la remarque de la paie le dit, dans le detail. Une
//! paie des pieces qui peut passer sa limite (au-dessus de quatre fois le plus bas plafond possible) et dont le debut
//! du contrat le dirait se demande par la case « Dates d'embauche » (`debuts`) ; une ligne prevue ne dit que sa fin, et
//! rien ne se demande pour elle.
//!
//! CE QUE LA ROUTE REND, pour la personne : d'abord la reponse, les 507 heures sur les 12 mois (365 jours) avant la
//! derniere fin de contrat, lignes prevues comprises (art. 3 ; `droit::allocation::affiliation_des_droits`, sans
//! readmission ni allongement, compte par le meme code que la route `droits` d'une compagnie, `droit::cumul::Position`,
//! `archives::comptes_de_la_position`) : les heures, en vert, et l'allocation journaliere de son annexe
//! (`droit::allocation`, chaque paie sous sa limite, art. 11 §3) ; ou ce qui manque et jusqu'a quand (la date
//! anniversaire quand elle est a venir, sinon le jour ou la plus ancienne paie sort des 12 mois). Puis, avec une
//! allocation visee, ce qu'il faudrait par cachet ou de l'heure, et une colonne du tableau des paies : le brut vise de
//! chacune. Puis une ligne par paie (sa fin, les cachets, les heures, l'enseignement, la formation, le brut). Le reste
//! (le compte des heures, l'estimation, ce qui arrete la hausse) est dans le detail, replie. Des chiffres, pas un
//! conseil.
//!
//! CE QUI NE SE COMPTE PAS ICI, et la route le dit : une DSN (ses contrats se comptent par leurs AEM ou leurs bulletins),
//! un certificat Congés Spectacles, une piece qui ne se lit pas ; et, dans l'estimation, le salaire annuel de reference
//! (maternite, adoption, ALD), la clause de rattrapage, le differe d'indemnisation.
//!
//! L'ANNEE DE CHOMAGE QUI VIENT, quand l'allocation s'estime (Léandre, 27/11/2026 ; `droit::annee_de_chomage`,
//! `annee_dite`) : sans aucun travail, du lendemain de la derniere fin de contrat a sa date anniversaire, le delai
//! d'attente, les franchises, la participation de 0,93 % et, quand la personne dit son taux (`csg`), la CSG et la CRDS ;
//! ce que retire un jour de travail (art. 30), chiffre ; et l'annee si elle ressemble a la derniere (le meme travail aux
//! memes mois). La phrase de la reponse en dit le chiffre.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use droit::allocation::{Affiliation, Allocation, Borne, Visee};
use droit::annee_de_chomage::{Cumul, Manque, OuvertureDesDroits, TauxDeCsg, TravailDuMois};
use droit::cumul::{Constantes, Declare};
use droit::emplois::AnnexeChomage;
use droit::fiche::{Nir, Siren};
use engagement::importe::{Attestation, ContratLu};
use import::attestes::{BlocDImport, Registre};
use import::reponse::Source;
use import::tri::Sorte;
use socle::date::{Duree, Jour, Periode};
use socle::ecran::{ColonneRendue, RangeeRendue, SaisieDuTableau, SorteDeRangee, TableauRendu};
use socle::echec::{Classe, R};
use socle::fichiers::NomDeFichier;
use socle::grandeurs::ecrit::sans_zero::dit;
use socle::grandeurs::{Cachets, Coefficient, Heures, Jours, Montant};
use socle::mois::Annee;
use socle::texte::euros;

use crate::catalogue::{Acte, Affiche, Ecran, Fiche, Forme, Refus, Route, OPERATION};
use crate::declaration::{charge, cle, dit as legende};
use crate::enveloppe::{en_json, Charge, Dite, Phrase, Rendu};
use crate::importation::bulletins::{contrat_de_l_aem, contrat_du_bulletin, nir_lu, nom_copie};

/// Le dossier des pieces d'un intermittent, a la racine de l'installation.
pub const MES_PIECES: NomDeFichier = NomDeFichier::du_moteur("mes-pieces");

/// La route de ce module, et sa fiche.
pub const ROUTES: &[Fiche] = &[Fiche {
    route: Route::DroitsDesPieces,
    geste: "Dire les droits d'un intermittent depuis ses pièces, et son allocation estimée.",
    charge: &ChargeDroitsDesPieces::CHARGE,
    exemple: r#"{"allocation_visee":80,"previsionnel":[{"fin":"2026-12-12","cachets":1,"nature":"artiste","brut":250}]}"#,
    rend: "phrase (la réponse : les heures, jusqu'à quand, l'allocation), et ecran : les blocs dans l'ordre où ils se lisent, chacun son `est`. Avec un calcul : la personne (une case, quand elle se choisit), la réponse, l'allocation visée (quand une allocation s'estime), l'année de chômage (quand une allocation s'estime), la date anniversaire (une case), les paies, le dépôt, les dates d'embauche (une case, quand la route en demande pour une paie des pièces), les pièces, le détail ; sans calcul : la personne, la date anniversaire, les paies (vides, quand aucune pièce ne porte de personne), le dépôt, les pièces. case : libelle, forme, valeur (les nombres tels qu'on les lit), choix, parties (chacune sa forme, ses choix, et prerempli : la nature d'une ligne prévue vide prend l'annexe de base, quand les paies n'en disent qu'une), ajouter et retirer d'une rangée, dit. reponse : nom (seulement quand les pièces portent plusieurs personnes), couleur (vert : les heures sont là), heures (les 507 h sur les 12 mois avant la dernière fin de contrat, ou ce qui manque, compté à partir d'aujourd'hui quand cette fin est passée), jusqu_a (quand il en manque : la date anniversaire à venir, sinon le jour où la plus ancienne paie sort des 12 mois), allocation (celle de l'annexe qui s'applique, celle qui a le plus d'heures, ou ce qui l'attend), annee (le chômage de l'année qui vient sans aucun travail, « au plus » quand une franchise ne se compte pas, avant CSG et CRDS quand le taux n'est pas dit), sous (la fenêtre, et l'annexe qui s'applique avec ses heures contre celles de l'autre). visee : case (« Je voudrais »), unite, reponse (ce qu'il faudrait par cachet ou de l'heure), reinitialiser (vide la case). annee (l'année qui suit la dernière fin de contrat, jusqu'à sa date anniversaire : annexes, art. 19 à 23 et 30) : titre, sans_travail (titre, chiffre, sous, calculs [lignes [quoi, valeur], total [quoi, valeur]] : les jours payés, puis le montant ; aide), case (le taux de CSG, une liste fermée, vide par défaut), cout (titre, aide : ce qu'un cachet, une heure, une journée retirent ; exemples [titre, calcul, aide]), comme (quand la fenêtre a du travail : le même travail aux mêmes mois ; titre, aide, tableau des mois, barres [mois, chomage, salaire], calcul, dit), notes.paies : tableau (titre, colonnes, rangées de cellules dites ; une ligne par paie que les 12 mois comptent, dans l'ordre des fins : fin, cachets, heures, nature, brut, et le brut visé quand l'allocation visée est remplie ; les lignes prévues s'y saisissent, chacune à sa fin, une ligne vide attendant en bas : saisie [case, retirer], les parties que tient chaque colonne, le rang de chaque rangée prévue) et lignes (la case des lignes prévues : ses parties, ses rangées). depot : dossier (mes-pieces/, où poser les pièces), libelle, aide. pieces : pieces [fichier (le nom posé dans mes-pieces/), nature (quand elle est reconnue), employeurs (ceux que ses pièces disent), dit (ce qui en est lu)], retirer. details (replié ; la personne choisie, ou les lignes prévues seules sans pièce) : libelle, annexes (les heures de chaque annexe, celle qui s'applique et pourquoi, l'annexe de chaque paie et d'où elle se lit), nir, fenetre, compte [quoi, valeur], heures_exigees, manque, estimation (de l'annexe : a, b, c, calculee, salaire_de_reference, retenue, paies : un tableau avec plafond, limite, ce qui compte), lignes (le jour butoir, l'estimation, ce qui arrête la hausse, les notes)",
    refus: &[Refus { classe: Classe::Refuse, corriger: "les pièces que la phrase nomme ; rien n'est écrit" }, OPERATION],
    pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
    acte: Acte::Lit,
    phrase: 260,
    ecran: Ecran::Lecture,
}];

charge! {
    /// La charge de `droits-des-pieces` : la personne (aucune : la seule des pieces, ou aucune s'il y en a plusieurs),
    /// la date anniversaire (aucune : ce qui manque se compte jusqu'a la sortie de la plus ancienne paie), l'allocation
    /// visee (aucune : rien ne se vise), les lignes prevues (aucune : les pieces seules).
    #[derive(Debug)]
    struct ChargeDroitsDesPieces {
        personne: Option<Nir> = legende("Personne", "son numéro de sécurité sociale, parmi ceux que portent les pièces ; absent : la seule qu'elles portent ; un numéro qu'elles ne portent plus ne compte pas").rendue(),
        anniversaire: Option<Jour> = legende("Date anniversaire (si vous en avez une)", "celle que France Travail a notifiée : les heures qui manquent se comptent jusqu'à elle ; absente ou passée : jusqu'au jour où la plus ancienne paie sort des 12 mois"),
        allocation_visee: Option<Montant> = legende("Je voudrais", "l'allocation journalière qu'on voudrait, en euros par jour ; absente : rien ne se vise"),
        previsionnel: Option<Vec<LignePrevue>> = legende("Lignes prévues", "le travail à venir, une ligne par engagement : sa fin, ses cachets ou ses heures, sa nature (artiste, technicien, enseignement ou formation ; vide : l'annexe de base, quand les paies n'en disent qu'une), son brut ; rien ne s'écrit"),
        debuts: Option<Vec<DebutDUnContrat>> = legende("Dates d'embauche", "la date d'embauche d'une paie des pièces dont la route la demande (la case Dates d'embauche, ses choix), sous le nom de la paie ; absente : rien n'est dit"),
        csg: Option<TauxDeCsg> = legende("Votre CSG sur le chômage", "le taux de CSG de la personne sur son allocation, selon son revenu fiscal de référence ; absent : l'année de chômage se dit avant CSG et CRDS"),
    }
}

/// LE DEBUT DU CONTRAT D'UNE PAIE DES PIECES, que la route demande quand le plafond de la paie ne se connait pas et que
/// son brut peut passer sa limite (art. 11 §3) : la paie, par le nom que la route rend (les choix de sa case), et la
/// date. Une ligne prevue dit le sien dans sa ligne (`LignePrevue::debut`).
#[derive(Debug, Clone, serde::Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DebutDUnContrat {
    paie: String,
    debut: Jour,
}

/// Les cases du debut du contrat d'une paie : la paie, parmi celles que sa case offre, et la date.
const PARTIES_D_UN_DEBUT: &[crate::catalogue::Partie] = &[
    crate::catalogue::Partie { cle: "paie", libelle: Affiche::new("Paie"), forme: Forme::ChoixRendu, toujours: true },
    crate::catalogue::Partie { cle: "debut", libelle: Affiche::new("Embauche le"), forme: Forme::Date, toujours: true },
];

/// Dans la charge, le debut du contrat d'une paie est une valeur faite de parties : les debuts, des rangees.
impl crate::declaration::ValeurDeLaCharge<crate::declaration::ParSaForme> for DebutDUnContrat {
    const FORME: Forme = Forme::Parties(PARTIES_D_UN_DEBUT);
}

/// UNE LIGNE PREVUE : un contrat a venir, saisi a la main pour un previsionnel. Sa fin seule (la ligne declaree se situe
/// par elle, `droit::cumul::Declare` ; son debut ne se demande pas, Léandre, 27/11/2026 : « on s'en fiche ») ; des
/// cachets ou des heures ; sa nature (`NatureDUneLigne` ; vide : l'annexe de base, `AnnexesDesPaies::base`) ; son brut,
/// facultatif (sans lui, la ligne compte pour les heures, pas pour l'allocation).
/// Elle s'ecrit aussi dans la valeur de sa case (`cases`), ses nombres tels qu'on les lit (`a_lire`).
#[derive(Debug, Clone, serde::Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct LignePrevue {
    fin: Jour,
    #[serde(serialize_with = "a_lire")]
    cachets: Option<Cachets>,
    #[serde(serialize_with = "a_lire")]
    heures: Option<Heures>,
    nature: Option<NatureDUneLigne>,
    #[serde(serialize_with = "a_lire")]
    brut: Option<Montant>,
}

/// LA NATURE D'UNE LIGNE PREVUE (Léandre, 27/11/2026 : « un seul déroulant : Artiste, Technicien, Enseignement,
/// Formation ») : ses heures ou ses cachets comptent sous elle. Artiste ou technicien : l'activite d'une annexe (X ou
/// VIII), pour les 507 heures, le choix de l'annexe et l'allocation. Enseignement ou formation : des heures seulement,
/// que les annexes assimilent a du travail sous leur plafond (art. 3 §1) ; elles ne se comptent sous aucune annexe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum NatureDUneLigne {
    Artiste,
    Technicien,
    Enseignement,
    Formation,
}

impl NatureDUneLigne {
    /// L'annexe dont elle est l'activite ; aucune : l'enseignement et la formation.
    fn annexe(self) -> Option<AnnexeChomage> {
        match self {
            NatureDUneLigne::Artiste => Some(AnnexeChomage::X),
            NatureDUneLigne::Technicien => Some(AnnexeChomage::Viii),
            NatureDUneLigne::Enseignement | NatureDUneLigne::Formation => None,
        }
    }

    /// La nature d'une annexe.
    fn de_l_annexe(a: AnnexeChomage) -> NatureDUneLigne {
        match a {
            AnnexeChomage::X => NatureDUneLigne::Artiste,
            AnnexeChomage::Viii => NatureDUneLigne::Technicien,
        }
    }
}

/// La nature d'une ligne est une liste fermee : la charge l'ecrit par son code (`artiste`…), l'ecran la fait choisir.
impl socle::liste::ListeFermee for NatureDUneLigne {
    const TOUTES: &'static [NatureDUneLigne] = &[NatureDUneLigne::Artiste, NatureDUneLigne::Technicien, NatureDUneLigne::Enseignement, NatureDUneLigne::Formation];

    fn code(&self) -> R<String> {
        socle::liste::code_ecrit(self)
    }

    fn libelle(&self) -> R<&'static str> {
        Ok(self.dite())
    }
}

impl NatureDUneLigne {
    /// Son libelle : ce que la liste deroulante et le tableau des paies en montrent.
    fn dite(self) -> &'static str {
        match self {
            NatureDUneLigne::Artiste => "Artiste (annexe 10)",
            NatureDUneLigne::Technicien => "Technicien (annexe 8)",
            NatureDUneLigne::Enseignement => "Enseignement",
            NatureDUneLigne::Formation => "Formation",
        }
    }
}

/// UN NOMBRE DANS LA VALEUR D'UNE CASE, tel qu'on le lit et qu'on le retape (`socle::texte::virgule` : « 2,5 »,
/// « 1250 ») ; la charge le relit comme tout nombre tape (`lu::tapee`). Vide, `null`.
fn a_lire<G: socle::grandeurs::Grandeur, S: serde::Serializer>(g: &Option<G>, s: S) -> Result<S::Ok, S::Error> {
    match g {
        Some(g) => s.serialize_str(&socle::texte::virgule(*g)),
        None => s.serialize_none(),
    }
}

/// Les cases d'une ligne prevue, chacune la forme de son type.
const PARTIES_D_UNE_LIGNE_PREVUE: &[crate::catalogue::Partie] = &[
    crate::catalogue::Partie { cle: "fin", libelle: Affiche::new("Fin"), forme: Forme::Date, toujours: true },
    crate::catalogue::Partie { cle: "cachets", libelle: Affiche::new("Cachets"), forme: Forme::Nombre, toujours: true },
    crate::catalogue::Partie { cle: "heures", libelle: Affiche::new("Heures"), forme: Forme::Nombre, toujours: true },
    crate::catalogue::Partie { cle: "nature", libelle: Affiche::new("Nature"), forme: Forme::Choix(crate::catalogue::ListeDeChoix::de::<NatureDUneLigne>()), toujours: true },
    crate::catalogue::Partie { cle: "brut", libelle: Affiche::new("Brut"), forme: Forme::Montant, toujours: true },
];

/// Dans la charge, une ligne prevue est une valeur faite de parties : les lignes, des rangees.
impl crate::declaration::ValeurDeLaCharge<crate::declaration::ParSaForme> for LignePrevue {
    const FORME: Forme = Forme::Parties(PARTIES_D_UNE_LIGNE_PREVUE);
}

/// Le bouton qui retire une ligne prevue (une ligne vide attend toujours en bas du tableau), et ceux d'une date
/// d'embauche.
const RETIRER_LA_LIGNE: Affiche = Affiche::new("Retirer la ligne");
const AJOUTER_UNE_EMBAUCHE: Affiche = Affiche::new("Ajouter une date d'embauche");
const RETIRER_LA_DATE: Affiche = Affiche::new("Retirer la date d'embauche");

/// LES LIGNES PREVUES SANS PIECE : quand aucune piece ne porte de personne, les lignes de la charge se comptent seules, sous
/// cette cle (celle de `droit::cumul::Declare`, que la position range ; elle ne se montre pas).
const LIGNES_SEULES: &str = "lignes prévues";

/// LE CHAMP `nom` DE LA CHARGE, tel que sa declaration le dit (son libelle, sa forme, son sens) : une case que la route rend
/// pour lui en porte la forme et les mots, jamais une copie.
fn champ(nom: &'static str) -> R<crate::catalogue::Champ> {
    match ChargeDroitsDesPieces::CHARGE.champs.iter().find(|c| c.champ.nom == nom) {
        Some(c) => Ok(c.champ),
        None => Err(socle::echec::erreur(Classe::DefautDuLogiciel, format!("Défaut du logiciel : la charge de droits-des-pieces n'a pas de champ {}.", nom))),
    }
}

/// LA CASE D'UN CHAMP D'UNE SEULE VALEUR de la charge, avec ce que la charge y porte : son libelle et sa forme sont ceux
/// de la declaration ; sans aide, son libelle se comprend seul (Léandre, 27/11/2026 : « supprime 1 et 2 ») : le sens du
/// champ reste dans la charge (`moteur routes`).
fn case_simple(nom: &'static str, valeur: Option<socle::ecriture::ValeurDeCase>) -> R<crate::fiches::CaseDeLaFiche<&'static str>> {
    let c = champ(nom)?;
    Ok(crate::fiches::CaseDeLaFiche::de_la_boite(nom, c.libelle, c.forme, "", valeur))
}

/// LA CASE DES RANGEES D'UN CHAMP de la charge (les dates d'embauche) : ses parties, ce que la charge y porte, le bouton
/// qui ajoute une rangee et celui qui en retire une ; sans aide, comme `case_simple`.
fn case_en_rangees(nom: &'static str, valeur: Option<socle::ecriture::ValeurDeCase>, parties: Vec<crate::fiches::PartieDeLaFiche>, ajouter: Affiche, retirer: Affiche) -> R<crate::fiches::CaseDeLaFiche<&'static str>> {
    let c = champ(nom)?;
    Ok(crate::fiches::CaseDeLaFiche::de_rangees(nom, c.libelle, valeur, parties).ajoutant(ajouter).retirant(retirer))
}

/// Le depot des pieces, tel que l'ecran le dit.
const DEPOSER: &str = "Déposer mes bulletins";
const DEPOSER_AIDE: &str = "Glissez-les ici, ou cliquez pour les choisir.";

/// Ce qui se dit sous l'allocation visee, et le bouton qui vide sa case.
const PAR_JOUR: &str = "€ par jour";
const REINITIALISER: &str = "Réinitialiser";

/// L'ECRAN DES DROITS, BLOC PAR BLOC, dans l'ordre ou il se lit (la maquette de Léandre, 27/11/2026 : « Les gens, ils
/// viennent là pour savoir s'ils ont leurs heures […] c'est la première information qu'il faut voir ») ; chaque case
/// porte ce que la charge y dit, ses nombres tels qu'on les lit (`a_lire`).
/// - Avec un calcul (`CalculDesDroits`) : la personne quand elle se choisit ; la reponse (les heures, l'allocation) ;
///   l'allocation visee quand une allocation s'estime ou que la charge en dit une ; la date anniversaire ; le tableau des
///   paies, ou les lignes prevues se saisissent (leur nature montre l'annexe de base pour une ligne qui ne la dit pas) ;
///   le depot ; les dates d'embauche quand la route en demande ou que la charge en dit ; les pieces deposees ; le detail
///   du calcul, replie.
/// - Sans calcul : la personne quand elle se choisit ; la date anniversaire ; le tableau des paies, vide, ou
///   les lignes prevues se saisissent, quand aucune piece ne porte de personne (`sans_personne`) ; le depot ; les pieces
///   deposees.
/// La personne se choisit quand les pieces en portent plusieurs ou que la charge en dit une, meme qu'elles ne portent
/// plus (`personnes` : ses choix, celles des pieces, et ce qui se dit sous elle ; on la vide pour en sortir).
fn ecran(
    c: &ChargeDroitsDesPieces,
    personnes: Option<(Vec<crate::ouverture::Choix>, Vec<String>)>,
    calcul: Option<CalculDesDroits>,
    sans_personne: bool,
    dossier: PathBuf,
    pieces: DitDesPieces,
) -> R<Vec<BlocDesDroits>> {
    use socle::ecriture::ValeurDeCase;
    use socle::liste::ListeFermee;
    let mut ecran = Vec::new();
    if let Some((offerts, dit)) = personnes {
        let case = case_simple(cle!(ChargeDroitsDesPieces, personne), c.personne.as_ref().and_then(ValeurDeCase::de))?;
        ecran.push(BlocDesDroits::Case(case.offrant(offerts).disant(dit)));
    }
    let anniversaire = BlocDesDroits::Case(case_simple(cle!(ChargeDroitsDesPieces, anniversaire), c.anniversaire.as_ref().and_then(ValeurDeCase::de))?);
    let depot = BlocDesDroits::Depot(DepotDit { dossier, libelle: DEPOSER, aide: DEPOSER_AIDE });
    // Les lignes prevues : leurs parties et ce que la charge en dit ; elles se saisissent dans le tableau des paies, qui
    // dit ou chacune se pose, et ses boutons (`tableau_des_paies`). Leur nature se choisit parmi ses quatre valeurs ;
    // vide, la ligne prend l'annexe de base (`AnnexesDesPaies::base`), que la case montre sans l'ecrire.
    let nom = cle!(ChargeDroitsDesPieces, previsionnel);
    let natures = NatureDUneLigne::TOUTES.iter().map(|n| Ok(crate::ouverture::Choix::de(n.code()?, n.libelle()?))).collect::<R<Vec<_>>>()?;
    let base = calcul.as_ref().and_then(|x| x.base).map(NatureDUneLigne::de_l_annexe);
    let parties = PARTIES_D_UNE_LIGNE_PREVUE
        .iter()
        .map(|p| match p.cle {
            "nature" => crate::fiches::PartieDeLaFiche::de(p, Some(natures.clone())).preremplie(base.as_ref().and_then(ValeurDeCase::de)),
            _ => crate::fiches::PartieDeLaFiche::de(p, None),
        })
        .collect();
    let champ_des_lignes = champ(nom)?;
    let lignes = crate::fiches::CaseDeLaFiche::de_rangees(nom, champ_des_lignes.libelle, c.previsionnel.as_ref().and_then(ValeurDeCase::de), parties);
    let mut details = None;
    match calcul {
        Some(x) => {
            ecran.push(BlocDesDroits::Reponse(x.reponse));
            if x.visee_offerte || c.allocation_visee.is_some() {
                let valeur = c.allocation_visee.map(|m| ValeurDeCase::texte(socle::texte::virgule(m)));
                let case = case_simple(cle!(ChargeDroitsDesPieces, allocation_visee), valeur)?;
                ecran.push(BlocDesDroits::Visee(ViseeDite { case, unite: PAR_JOUR, reponse: x.visee, reinitialiser: REINITIALISER }));
            }
            if let Some(a) = x.annee {
                ecran.push(BlocDesDroits::Annee(Box::new(a)));
            }
            ecran.push(anniversaire);
            ecran.push(BlocDesDroits::Paies(PaiesDites { tableau: x.paies, lignes }));
            ecran.push(depot);
            if !x.embauches.is_empty() || c.debuts.as_ref().is_some_and(|d| !d.is_empty()) {
                let parties = PARTIES_D_UN_DEBUT
                    .iter()
                    .map(|p| crate::fiches::PartieDeLaFiche::de(p, (p.forme == Forme::ChoixRendu).then(|| x.embauches.clone())))
                    .collect();
                let case = case_en_rangees(cle!(ChargeDroitsDesPieces, debuts), c.debuts.as_ref().and_then(ValeurDeCase::de), parties, AJOUTER_UNE_EMBAUCHE, RETIRER_LA_DATE)?;
                ecran.push(BlocDesDroits::Case(case.disant(x.demande.into_iter().collect())));
            }
            details = Some(x.details);
        }
        None => {
            ecran.push(anniversaire);
            if sans_personne {
                ecran.push(BlocDesDroits::Paies(PaiesDites { tableau: tableau_des_paies(&[], &[], None, None), lignes }));
            }
            ecran.push(depot);
        }
    }
    let pieces = pieces.dites();
    if !pieces.is_empty() {
        ecran.push(BlocDesDroits::Pieces(PiecesDites { pieces, retirer: RETIRER_LA_PIECE }));
    }
    if let Some(d) = details {
        ecran.push(BlocDesDroits::Details(Box::new(d)));
    }
    Ok(ecran)
}

pub fn route(inst: &Path, nom: Route, charge: &Charge) -> R<Rendu> {
    match nom {
        Route::DroitsDesPieces => en_json(droits_des_pieces(inst, charge.lire()?)?),
        autre => Err(crate::enveloppe::route_inconnue(autre.nom())),
    }
}

// ------------------------------------------------------------------ ce que les pieces attestent

/// L'EMPLOYEUR D'UNE PIECE, tel qu'elle le dit : son Siren (celui de son Siret), sinon sa raison sociale. Il range les
/// contrats : un registre par employeur. Une piece qui ne dit pas un employeur seul (ni Siren ni raison sociale, ou une
/// AEM deposee a plusieurs Siren) est un employeur a elle seule (`Seule`, son rang parmi elles) : deux employeurs ne font
/// jamais un contrat, et la route le dit.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum CleDEmployeur {
    Siren(Siren),
    Nom(String),
    Seule(usize),
}

/// Une piece lue : de qui (NIR), chez quel employeur, et le contrat qu'elle atteste.
struct Attestee {
    /// La piece, telle que la route la nomme (le fichier pose, sa page).
    piece: String,
    nir: Nir,
    employeur: CleDEmployeur,
    raison_sociale: Option<String>,
    nom: Option<String>,
    naissance: Option<Jour>,
    periode: Periode,
    ligne: ContratLu,
    source: Source,
    bloc: BlocDImport,
    plafond: SourceDuPlafond,
    /// Artiste ou technicien, quand la piece le dit (la case d'une AEM, le statut ou l'emploi d'un bulletin) ; aucun :
    /// elle ne le dit pas.
    statut: Option<droit::fiche::Statut>,
}

/// D'OU VIENT LE PLAFOND D'UNE PAIE (le plafond de la Securite sociale de sa periode, `droit::plafond`), pour la limite du
/// salaire de reference (annexes, art. 11 §3) : celui que le bulletin imprime ; celui que le moteur calcule depuis le
/// contrat (`contrat`, entier quand la piece le dit : il dit si l'engagement d'un artiste est court) et la periode de la
/// paie. Le contrat ne se demande que quand la periode ne suffit pas : un artiste dont la paie fait moins de cinq jours
/// (`plafond_de`). La periode d'un bulletin est celle qu'il paie (R. 3243-1, 5°) : elle peut etre le mois entier quand le
/// contrat est plus court ; le plafond s'y calcule alors au mois, plus haut que celui du contrat, et la limite de la paie
/// est trop haute (INFERENCE : la periode payee d'un intermittent est celle de son contrat) ; une note de l'estimation le
/// dit, pour chaque paie dont la periode est un mois entier (`estimer`). Une ligne prevue ne dit que sa fin
/// (`SansDebut`) : elle n'a ni contrat ni periode, son plafond ne se connait pas, et elle compte entiere.
#[derive(Clone, Debug)]
enum SourceDuPlafond {
    Imprime(Montant),
    DuContrat { contrat: Option<Periode>, paie: Periode },
    SansDebut,
}

/// POURQUOI LE PLAFOND D'UNE PAIE NE SE CONNAIT PAS : la phrase, et si la date de debut de son contrat le dirait.
struct PlafondInconnu {
    raison: String,
    le_debut_le_dit: bool,
}

impl SourceDuPlafond {
    /// La meme, avec la date de debut `debut` que la charge dit du contrat de la paie (`debuts`) : le contrat va de ce
    /// debut a la fin de la paie, et la paie n'en prend que ce qui est dans sa periode (INFERENCE : le debut ne se demande
    /// que pour une paie de moins de cinq jours, qui n'est pas un mois entier ; elle finit avec son contrat) ; un debut
    /// apres la fin de la paie ne dit rien, et la note le dit. Une ligne prevue n'a pas de debut : sa source reste ce
    /// qu'elle est.
    fn avec_le_debut(&self, debut: Jour) -> Option<SourceDuPlafond> {
        let paie = match self {
            SourceDuPlafond::DuContrat { contrat: None, paie } => *paie,
            SourceDuPlafond::Imprime(_) | SourceDuPlafond::DuContrat { contrat: Some(_), .. } | SourceDuPlafond::SansDebut => return Some(self.clone()),
        };
        let contrat = Periode::du_au(debut, paie.fin()).ok()?;
        let paie = Periode::du_au(debut.max(paie.debut()), paie.fin()).ok()?;
        Some(SourceDuPlafond::DuContrat { contrat: Some(contrat), paie })
    }
}

/// Le plafond que le bulletin imprime, colonne de la periode (`NomDeCumul::Plafond`).
fn plafond_imprime(b: &import::bulletin_lu::BulletinLu) -> Option<Montant> {
    b.cumuls.iter().find_map(|(n, c)| match (n, c) {
        (import::bulletin_lu::NomDeCumul::Plafond, import::bulletin_lu::Cumul::Euros(e)) => e.periode,
        _ => None,
    })
}

/// La cle de l'employeur que dit une piece ; aucune quand elle ne dit ni Siren ni raison sociale.
fn cle_d_employeur(siren: Option<Siren>, raison_sociale: Option<&String>) -> Option<CleDEmployeur> {
    match (siren, raison_sociale.map(|r| r.trim()).filter(|r| !r.is_empty())) {
        (Some(s), _) => Some(CleDEmployeur::Siren(s)),
        (None, Some(r)) => Some(CleDEmployeur::Nom(r.to_uppercase())),
        (None, None) => None,
    }
}

/// LES CONTRATS DES PIECES QUI NE DISENT PAS UN EMPLOYEUR SEUL : chacun le sien (`CleDEmployeur::Seule`, son rang), et
/// une phrase par piece (`dites`, les pieces deja dites).
#[derive(Default)]
struct Seules {
    rang: usize,
    dites: std::collections::BTreeSet<(String, Option<usize>)>,
}

impl Seules {
    /// La cle d'un contrat de la piece `chemin` (a sa page `page`) : celle que dit `lue`, sinon une a lui seul ; la piece
    /// se dit une fois dans `pieces`, avec `pourquoi` et l'effet.
    fn cle(&mut self, lue: Option<CleDEmployeur>, chemin: &str, page: Option<usize>, pourquoi: &str, pieces: &mut DitDesPieces) -> CleDEmployeur {
        if let Some(c) = lue {
            return c;
        }
        self.rang += 1;
        if self.dites.insert((chemin.to_string(), page)) {
            pieces.dire(
                chemin,
                page,
                &format!("{} ; son contrat compte à part, sans se rapprocher d'aucune autre pièce : ses heures peuvent compter deux fois si une autre pièce dit le même contrat, et le seuil paraître atteint à tort.", pourquoi),
            );
        }
        CleDEmployeur::Seule(self.rang)
    }
}

/// Pourquoi une piece n'a pas d'employeur.
const SANS_EMPLOYEUR: &str = "elle ne dit pas son employeur (ni Siret ni raison sociale)";

/// Le nom d'une personne, tel qu'une piece l'imprime.
fn nom_de(identite: &declarations::identite::IdentiteDeclaree) -> Option<String> {
    let mots: Vec<&str> = [identite.prenoms.as_deref(), identite.nom.as_deref()].into_iter().flatten().filter(|m| !m.trim().is_empty()).collect();
    (!mots.is_empty()).then(|| mots.join(" "))
}

/// CE QUE LE DOSSIER DIT : les pieces lues, et une phrase par fichier.
struct DossierDesPieces {
    attestees: Vec<Attestee>,
    pieces: DitDesPieces,
}

/// CE QUE LA ROUTE DIT DES PIECES DEPOSEES, fichier par fichier : chaque fichier pose a la racine de `mes-pieces/` (une
/// archive tient ce qu'elle contient), meme s'il n'y a rien a en dire : sa nature, ses employeurs, et ses phrases.
#[derive(Default)]
struct DitDesPieces(BTreeMap<String, CeQuiSeDitDUnFichier>);

/// Ce que la route dit d'un fichier pose : sa nature (ce que le tri et la lecture en ont reconnu ; aucune : rien de
/// reconnu), les employeurs que ses pieces disent (aucun : aucun n'est lu), et ses phrases.
#[derive(Default)]
struct CeQuiSeDitDUnFichier {
    nature: Option<&'static str>,
    employeurs: std::collections::BTreeSet<String>,
    dit: Vec<String>,
}

/// L'EMPLOYEUR D'UNE PIECE, en mots, tel que le moteur l'a lu : sa raison sociale, sinon son Siren ; aucun quand la
/// piece ne dit pas un employeur seul.
fn employeur_lu(raison_sociale: Option<&String>, cle: &CleDEmployeur) -> Option<String> {
    match (raison_sociale.map(|r| r.trim()).filter(|r| !r.is_empty()), cle) {
        (Some(r), _) => Some(r.to_string()),
        (None, CleDEmployeur::Siren(s)) => Some(format!("SIREN {}", &**s)),
        (None, CleDEmployeur::Nom(n)) => Some(n.clone()),
        (None, CleDEmployeur::Seule(_)) => None,
    }
}

/// Le fichier pose d'ou vient le chemin `chemin` (relatif au dossier ; dans une archive, `lot.zip/x.pdf` : `lot.zip`).
fn fichier_pose(chemin: &str) -> &str {
    match chemin.split_once('/') {
        Some((fichier, _)) => fichier,
        None => chemin,
    }
}

impl DitDesPieces {
    /// Le fichier d'ou vient `chemin`, sans rien en dire.
    fn fichier(&mut self, chemin: &str) {
        self.0.entry(fichier_pose(chemin).to_string()).or_default();
    }

    /// Ce que la route dit de `chemin`, a sa page `page` quand elle en a une : la phrase, sous le fichier pose, precedee
    /// du chemin dans l'archive et de la page.
    fn dire(&mut self, chemin: &str, page: Option<usize>, phrase: &str) {
        let pose = fichier_pose(chemin);
        let dedans = chemin.strip_prefix(pose).and_then(|r| r.strip_prefix('/')).map(str::to_string);
        let ou: Vec<String> = dedans.into_iter().chain(page.map(|p| format!("page {}", p))).collect();
        let dite = match ou.is_empty() {
            true => {
                // La phrase seule commence par une capitale ; le reste tel qu'il s'ecrit (« AEM », « DSN »).
                let mut c = phrase.chars();
                c.next().into_iter().flat_map(char::to_uppercase).chain(c).collect()
            }
            false => format!("{} : {}", ou.join(", "), phrase),
        };
        self.0.entry(pose.to_string()).or_default().dit.push(dite);
    }

    /// La phrase `phrase` sous le fichier pose d'ou vient `chemin`, telle quelle.
    fn dire_tel_quel(&mut self, chemin: &str, phrase: String) {
        self.0.entry(fichier_pose(chemin).to_string()).or_default().dit.push(phrase);
    }

    /// La nature du fichier `chemin`, quand il est pose lui-meme (un fichier dans une archive : l'archive a la sienne).
    fn nature(&mut self, chemin: &str, nature: &'static str) {
        if fichier_pose(chemin) == chemin {
            self.0.entry(chemin.to_string()).or_default().nature = Some(nature);
        }
    }

    /// Un employeur que dit une piece du fichier pose d'ou vient `chemin` ; aucun : rien a noter.
    fn employeur(&mut self, chemin: &str, employeur: Option<String>) {
        if let Some(e) = employeur {
            self.0.entry(fichier_pose(chemin).to_string()).or_default().employeurs.insert(e);
        }
    }

    /// Les pieces, telles que la route les rend.
    fn dites(self) -> Vec<PieceDite> {
        self.0
            .into_iter()
            .map(|(fichier, f)| PieceDite { fichier, nature: f.nature, employeurs: f.employeurs.into_iter().collect(), dit: f.dit })
            .collect()
    }
}

/// LE DOSSIER LU, fichier par fichier, par les lecteurs de l'import ; `travail` : le dossier de travail (les archives
/// ouvertes, les AEM deposees copiees pour leur lecteur), efface par l'appelant.
fn lire_le_dossier(inst: &Path, dossier: &Path, travail: &Path) -> R<DossierDesPieces> {
    let tri = import::tri::trier(dossier, &travail.join("tri"))?;
    let table = import::bulletin_lu::Table::livree()?;
    let mut pieces = DitDesPieces::default();
    // Une entree mise de cote se dit sous le fichier pose qui la porte (l'archive), ou sous son nom ; une entree sans nom
    // a la racine, sous le dossier.
    for m in &tri.mises_de_cote {
        let sous = match (&m.dans, &m.entree) {
            (Some(d), _) => d.texte().to_string(),
            (None, import::tri::EntreeDeCote::Nommee(n)) => n.clone(),
            (None, import::tri::EntreeDeCote::SansNom(_)) => MES_PIECES.texte().to_string(),
        };
        pieces.fichier(&sous);
        pieces.dire_tel_quel(&sous, m.dit());
    }
    let mut attestees = Vec::new();
    let mut bulletins: Vec<(String, usize, NomDeFichier, import::bulletin_lu::BulletinLu, Nir)> = Vec::new();
    let aem_dir = travail.join("aem");
    // Les AEM deposees, par leur copie : le nom du fichier pose, et l'employeur s'il est seul.
    let mut aem_employeurs: BTreeMap<String, (String, Option<CleDEmployeur>, usize)> = BTreeMap::new();
    let mut seules = Seules::default();
    for f in &tri.fichiers {
        let nom = f.chemin.texte().to_string();
        pieces.fichier(&nom);
        match f.sorte {
            Sorte::Aem => {
                let Some(copie) = nom_copie(f)? else { continue };
                std::fs::create_dir_all(&aem_dir).map_err(|e| socle::echec::du_disque(&aem_dir, &e).reclasser(Classe::Disque))?;
                let cible = aem_dir.join(copie.texte());
                std::fs::copy(&f.disque, &cible).map_err(|e| socle::echec::du_disque(&cible, &e).reclasser(Classe::Disque))?;
                let e = import::dsn_aem::employeur_aem(&f.disque)?;
                let siren = (e.sirens.len() == 1).then(|| e.sirens.iter().next().cloned()).flatten();
                aem_employeurs.insert(copie.texte().to_string(), (nom.clone(), cle_d_employeur(siren, None), e.sirens.len()));
                pieces.dire(&nom, None, "AEM déposée, lue.");
                pieces.nature(&nom, "AEM déposée");
            }
            Sorte::PdfTexte => {
                let Some(copie) = nom_copie(f)? else { continue };
                let aem = import::aem_lu::lire(&f.disque)?;
                if !aem.is_empty() {
                    let mut lues = 0usize;
                    for p in aem {
                        let import::aem_lu::Lecture::Lue(a) = p.lecture else {
                            if let import::aem_lu::Lecture::Refusee { raison, .. } = p.lecture {
                                pieces.dire(&nom, Some(p.page), &format!("{}.", raison));
                            }
                            continue;
                        };
                        let (Some(nir), Some(periode)) = (a.identite.nir.clone(), a.periode) else {
                            let manque = match (a.identite.nir.is_some(), a.periode.is_some()) {
                                (false, false) => "ni numéro de sécurité sociale ni période",
                                (false, true) => "pas de numéro de sécurité sociale",
                                (true, _) => "pas de période",
                            };
                            pieces.dire(&nom, Some(p.page), &format!("l'AEM n'imprime {} : elle ne se compte pas.", manque));
                            continue;
                        };
                        let (ligne, _) = contrat_de_l_aem(&a, None, &copie)?;
                        // Le contrat entier, quand l'AEM imprime son embauche et sa fin : il dit si l'engagement est court.
                        // Une embauche apres la fin : les dates de l'AEM sont incoherentes, la route le dit, et le
                        // contrat entier ne se connait pas.
                        let contrat_imprime = match (a.embauche, a.fin_contrat) {
                            (Some(d), Some(f)) => match Periode::du_au(d, f) {
                                Ok(c) => Some(c),
                                Err(_) => {
                                    pieces.dire(&nom, Some(p.page), &format!("l'AEM imprime une embauche le {}, après la fin du contrat le {} : ses dates sont incohérentes.", d.jj_mm_aaaa(), f.jj_mm_aaaa()));
                                    None
                                }
                            },
                            _ => None,
                        };
                        let lue = cle_d_employeur(a.employeur.siret.as_ref().map(droit::fiche::Siret::siren), a.employeur.raison_sociale.as_ref());
                        let piece = format!("{}, page {}", nom, p.page);
                        let employeur = seules.cle(lue, &nom, Some(p.page), SANS_EMPLOYEUR, &mut pieces);
                        pieces.employeur(&nom, employeur_lu(a.employeur.raison_sociale.as_ref(), &employeur));
                        attestees.push(Attestee {
                            piece,
                            nir,
                            employeur,
                            raison_sociale: a.employeur.raison_sociale.clone(),
                            nom: nom_de(&a.identite),
                            naissance: a.identite.naissance.and_then(|n| n.jour()),
                            periode,
                            ligne,
                            source: Source::Aem,
                            bloc: BlocDImport::AemPdf,
                            plafond: SourceDuPlafond::DuContrat { contrat: contrat_imprime, paie: periode },
                            statut: a.statut,
                        });
                        lues += 1;
                    }
                    pieces.dire(&nom, None, &format!("AEM en PDF, {} page(s) lue(s).", lues));
                    pieces.nature(&nom, "AEM en PDF");
                    continue;
                }
                let mut lues = 0usize;
                for l in import::bulletin_lu::lire_avec(&f.disque, &table)? {
                    let b = match l.resultat {
                        Ok(b) => b,
                        Err(import::bulletin_lu::Refus::Annexe) => continue,
                        Err(r) => {
                            pieces.dire(&nom, Some(l.page), &format!("ne se lit pas comme un bulletin ({}).", r.message()));
                            continue;
                        }
                    };
                    match nir_lu(&b) {
                        Ok(Some((nir, _))) => {
                            bulletins.push((nom.clone(), l.page, copie.clone(), b, nir));
                            lues += 1;
                        }
                        Ok(None) => pieces.dire(&nom, Some(l.page), "le bulletin n'imprime pas de numéro de sécurité sociale, il ne se range à personne."),
                        Err(e) => pieces.dire(&nom, Some(l.page), &e.phrase),
                    }
                }
                if lues > 0 {
                    pieces.dire(&nom, None, &format!("{} bulletin(s) lu(s).", lues));
                    pieces.nature(&nom, "bulletin de paie");
                }
            }
            Sorte::Dsn => {
                pieces.dire(&nom, None, "une DSN ne se compte pas ici ; déposez les bulletins de ses paies.");
                pieces.nature(&nom, "DSN");
            }
            // Une archive se dit par ce qu'elle contient ; une copie exacte, par ce que le tri en dit.
            Sorte::Archive | Sorte::Doublon => {
                if let Some(r) = &f.raison {
                    pieces.dire(&nom, None, r);
                }
                if f.sorte == Sorte::Archive {
                    pieces.nature(&nom, "archive");
                }
            }
            Sorte::PdfSansTexte => {
                pieces.dire(&nom, None, &format!("{}.", import::tri::IMAGE));
                pieces.nature(&nom, "image");
            }
            Sorte::Contrat => {
                pieces.dire(&nom, None, &format!("{}.", import::tri::CONTRAT));
                pieces.nature(&nom, "contrat");
            }
            Sorte::Illisible | Sorte::Autre => pieces.dire(&nom, None, &format!("ce fichier ne se lit pas. {}", import::tri::ACCEPTE)),
        }
    }
    // Les AEM deposees, par le lecteur de l'import (`dsn_aem::lire_aem`), dans leur copie : ce qu'il dit d'une copie se
    // dit du fichier pose.
    if aem_dir.is_dir() {
        let (contrats, illisibles, fautes) = import::dsn_aem::lire_aem(&aem_dir)?;
        for n in illisibles.into_iter().chain(fautes) {
            match aem_employeurs.get(&n.fichier) {
                Some((pose, _, _)) => pieces.dire(pose, None, &n.phrase),
                None => pieces.dire(&n.fichier, None, &n.phrase),
            }
        }
        for c in contrats {
            let (piece, lue, sirens) = match aem_employeurs.get(c.fichier.texte()) {
                Some((nom, e, n)) => (nom.clone(), e.clone(), *n),
                None => (c.fichier.texte().to_string(), None, 0),
            };
            let pourquoi = match sirens {
                0 | 1 => SANS_EMPLOYEUR.to_string(),
                n => format!("l'AEM porte {} employeurs (Siren), et la route ne lit pas lequel est celui de chaque contrat", n),
            };
            let employeur = seules.cle(lue, &piece, None, &pourquoi, &mut pieces);
            pieces.employeur(&piece, employeur_lu(None, &employeur));
            attestees.push(Attestee {
                piece,
                nir: c.nir.clone(),
                employeur,
                raison_sociale: None,
                nom: None,
                naissance: None,
                periode: c.periode,
                ligne: import::dsn_aem::ligne_du_contrat(&c, None),
                source: Source::Aem,
                bloc: BlocDImport::Declarations,
                plafond: SourceDuPlafond::DuContrat { contrat: Some(c.periode), paie: c.periode },
                // L'enregistrement d'une AEM deposee ne dit pas l'annexe que la route lirait.
                statut: None,
            });
        }
    }
    // Les bulletins, comptes avec leurs freres (le meme salarie, le meme logiciel, une periode qui finit le meme mois),
    // comme a l'import (`importation::bulletins::compter`).
    for (i, (nom, page, copie, b, nir)) in bulletins.iter().enumerate() {
        let freres: Vec<&import::bulletin_lu::BulletinLu> = bulletins
            .iter()
            .enumerate()
            .filter(|(k, x)| *k != i && x.4 == *nir && x.3.periode.fin().mois_civil() == b.periode.fin().mois_civil() && x.3.logiciel == b.logiciel)
            .filter(|(_, x)| (&x.3.numero, x.3.periode) != (&b.numero, b.periode))
            .map(|(_, x)| &x.3)
            .collect();
        let compte = import::bulletin_lu::compteurs(b, &freres, inst).and_then(|c| contrat_du_bulletin(b, *page, &c, &None, copie));
        let ligne = match compte {
            Ok((_, ligne)) => ligne,
            Err(e) => {
                pieces.dire(nom, Some(*page), &format!("le contrat ne se compte pas ({}).", e.phrase));
                continue;
            }
        };
        let raison = b.entreprise.employeur.raison_sociale.clone();
        let lue = cle_d_employeur(b.entreprise.employeur.siret.as_ref().map(droit::fiche::Siret::siren), raison.as_ref());
        let piece = format!("{}, page {}", nom, page);
        let employeur = seules.cle(lue, nom, Some(*page), SANS_EMPLOYEUR, &mut pieces);
        pieces.employeur(nom, employeur_lu(raison.as_ref(), &employeur));
        // Artiste ou technicien : le statut imprime, sinon l'emploi quand il nomme un metier du catalogue, comme a
        // l'import (`statut_et_metier`) ; un emploi qui ne se resout pas se dit, et le bulletin ne dit rien.
        let statut = match crate::importation::bulletins::statut_et_metier(b.salarie.statut.as_deref(), b.salarie.emploi.as_deref(), false, None) {
            Ok(e) => e.ecrit.statut,
            Err(e) => {
                pieces.dire(nom, Some(*page), &format!("artiste ou technicien ne se lit pas ({}).", e.phrase));
                None
            }
        };
        attestees.push(Attestee {
            statut,
            piece,
            nir: nir.clone(),
            employeur,
            raison_sociale: raison,
            nom: b.salarie.nom_complet.clone().or_else(|| nom_de(&b.salarie.identite)),
            naissance: b.salarie.identite.naissance.and_then(|n| n.jour()),
            periode: b.periode,
            ligne,
            source: Source::Bulletin,
            bloc: BlocDImport::Bulletins,
            plafond: match plafond_imprime(b) {
                Some(m) => SourceDuPlafond::Imprime(m),
                None => SourceDuPlafond::DuContrat { contrat: None, paie: b.periode },
            },
        });
    }
    Ok(DossierDesPieces { attestees, pieces })
}

// ------------------------------------------------------------------ les paies comptees

/// UNE PAIE COMPTEE : un contrat qu'une piece atteste, chez son employeur, une fois ; ou une ligne prevue. Ce qu'elle
/// compte (un compteur que la piece ne dit pas reste vide), et d'ou vient son plafond.
struct PaieComptee {
    /// Son nom, tel que la route le rend et que la charge le reprend (`debuts`) : sa piece (le fichier pose, sa page), ou
    /// le contrat prevu.
    id: String,
    /// Son employeur ; aucun : une ligne prevue, qui n'en a pas.
    employeur: Option<String>,
    /// Son debut : celui de la periode que la piece atteste ; aucun pour une ligne prevue, qui ne dit que sa fin.
    debut: Option<Jour>,
    fin: Jour,
    cachets: Option<Cachets>,
    /// Ses heures, l'enseignement compris (`droit::cumul::Declare::heures`).
    heures: Option<Heures>,
    enseignement: Option<Heures>,
    /// Ses heures de formation, en plus des heures : une ligne prevue seule les dit.
    formation: Option<Heures>,
    jours: Option<Jours>,
    brut: Option<Montant>,
    plafond: SourceDuPlafond,
    /// Une ligne prevue : son rang dans les lignes prevues de la charge (la rangee qui se saisit dans le tableau des
    /// paies) ; aucun : une paie qu'une piece atteste.
    prevue: Option<usize>,
    /// Son annexe, quand sa piece, sa nature ou ses cachets la disent (`annexe_de_la_paie`), et d'ou elle se lit ;
    /// aucune : elle prend celle de base (`AnnexesDesPaies::base`), s'il en est une.
    annexe: Option<(AnnexeChomage, String)>,
}

impl PaieComptee {
    /// Ses dates, en mots.
    fn dates(&self) -> String {
        match self.debut {
            Some(d) if d != self.fin => format!("du {} au {}", d.jj_mm_aaaa(), self.fin.jj_mm_aaaa()),
            Some(_) => format!("le {}", self.fin.jj_mm_aaaa()),
            None => format!("finissant le {}", self.fin.jj_mm_aaaa()),
        }
    }

    /// Elle, en mots : la paie, ses dates et son employeur ; une ligne prevue, ses dates.
    fn en_mots(&self) -> String {
        match &self.employeur {
            Some(e) => format!("la paie {} chez {}", self.dates(), e),
            None => format!("la ligne prévue {}", self.dates()),
        }
    }

    /// Ses heures d'enseignement.
    fn heures_d_enseignement(&self) -> Heures {
        self.enseignement.into_iter().sum()
    }

    /// Toutes ses heures sont-elles de l'enseignement ? Son salaire sort alors du salaire de reference.
    fn d_enseignement(&self) -> bool {
        let ens = self.heures_d_enseignement();
        !ens.est_nul() && self.cachets.is_none_or(|c| c.est_nul()) && self.heures == Some(ens)
    }

    /// Mele-t-elle l'enseignement et d'autres heures ou des cachets, sans dire le salaire de chacun ?
    fn melee(&self) -> bool {
        !self.heures_d_enseignement().est_nul() && !self.d_enseignement()
    }

    /// Son annexe : celle qu'elle dit, sinon celle de base (`AnnexesDesPaies::base`) ; aucune : ni l'une ni l'autre.
    fn annexe_ou(&self, base: Option<AnnexeChomage>) -> Option<AnnexeChomage> {
        self.annexe.as_ref().map(|(a, _)| *a).or(base)
    }

    /// Sa nature, en mots, dans le tableau des paies : son annexe (celle de base se dit telle), l'enseignement qu'elle
    /// compte ; la formation d'une ligne prevue ; « non dite » quand rien ne la dit.
    fn nature_dite(&self, base: Option<AnnexeChomage>) -> String {
        if self.d_enseignement() {
            return NatureDUneLigne::Enseignement.dite().to_string();
        }
        if self.formation.is_some() && self.heures.is_none() && self.cachets.is_none() {
            return NatureDUneLigne::Formation.dite().to_string();
        }
        let annexe = match (&self.annexe, base) {
            (Some((a, _)), _) => NatureDUneLigne::de_l_annexe(*a).dite().to_string(),
            (None, Some(b)) => format!("{}, comme vos autres paies", NatureDUneLigne::de_l_annexe(b).dite()),
            (None, None) => "Non dite".to_string(),
        };
        match self.heures_d_enseignement().est_nul() {
            true => annexe,
            false => format!("{}, dont {} h d'enseignement", annexe, dit(self.heures_d_enseignement())),
        }
    }
}

/// L'ANNEXE D'UNE PAIE, quand elle se lit : le statut que dit une piece du contrat (`statut`, avec la piece), sinon ses
/// cachets : l'annexe X dit l'activite des artistes « déclarée sous la forme de cachets »
/// (`plafond-intermittents/unedic-annexe10-convention-20241115.txt`, art. 3 §1 ; INFERENCE : un cachet ne se paie qu'a
/// un artiste). Aucune : rien ne la dit.
fn annexe_de_la_paie(statut: Option<(droit::fiche::Statut, &str)>, cachets: Option<Cachets>) -> Option<(AnnexeChomage, String)> {
    use droit::fiche::Statut;
    match statut {
        Some((Statut::Artiste, piece)) => Some((AnnexeChomage::X, format!("{} dit artiste", piece))),
        Some((Statut::Technicien, piece)) => Some((AnnexeChomage::Viii, format!("{} dit technicien", piece))),
        None if cachets.is_some_and(|c| !c.est_nul()) => Some((AnnexeChomage::X, "elle compte des cachets, que seul un artiste reçoit (annexe 10, art. 3)".to_string())),
        None => None,
    }
}

/// LES CONTRATS DE LA PERSONNE `nir`, un registre par employeur : les AEM d'abord, puis les bulletins, qui remplacent
/// l'AEM du meme contrat qui compte autre chose ; une piece dont le contrat est deja atteste n'ajoute rien. Comme a
/// l'import d'une compagnie (`importation::bulletins`) : une piece aux memes dates qu'un contrat atteste qui porte
/// d'autres compteurs (deux AEM, deux bulletins, une AEM apres un bulletin) laisse le contrat atteste ; elle n'est pas
/// ajoutee, et l'ecart se dit dans `notes`, avec ses deux valeurs.
fn retenues(attestees: &[Attestee], nir: &Nir, notes: &mut Vec<String>) -> R<Vec<PaieComptee>> {
    let mut registres: BTreeMap<CleDEmployeur, (Registre, Option<String>)> = BTreeMap::new();
    let siennes = || attestees.iter().filter(|a| a.nir == *nir);
    for a in siennes().filter(|a| a.source == Source::Aem).chain(siennes().filter(|a| a.source == Source::Bulletin)) {
        let (registre, raison) = registres.entry(a.employeur.clone()).or_insert_with(|| (Registre::vide(), None));
        if raison.is_none() {
            raison.clone_from(&a.raison_sociale);
        }
        let c = &a.ligne.contrat;
        let pareils = registre.pareils(&a.nir, &a.periode, c.numero_objet.as_ref(), c.nature, a.source);
        let le_sien = (c.heures, c.cachets, c.jours, c.brut);
        if pareils.iter().any(|p| p.compte() == le_sien) {
            continue;
        }
        let garde = match a.source {
            Source::Bulletin => pareils.iter().find(|p| p.contrat().atteste_par != Attestation::Aem),
            _ => pareils.first(),
        };
        if let Some(p) = garde {
            notes.push(format!(
                "{} : un contrat attesté aux mêmes dates ({}) porte d'autres compteurs ; il est gardé, celui de cette pièce n'est pas ajouté (le contrat attesté dit {}, la pièce {}).",
                a.piece,
                p.d_ou(),
                p.en_mots(),
                import::attestes::en_mots(c.heures, c.cachets, c.jours, c.brut)
            ));
            continue;
        }
        // Ce que l'ajout fait des contrats qu'il remplace se dit, comme a l'import (`importation::bulletins`).
        notes.extend(registre.ajouter(&a.nir, a.ligne.clone(), a.periode, a.source, a.bloc)?.into_iter().map(|r| format!("{} : {}.", a.piece, r)));
    }
    let mut out = Vec::new();
    for (cle, (registre, raison)) in registres {
        let employeur = match (raison, &cle) {
            (Some(r), _) => r,
            (None, CleDEmployeur::Siren(s)) => format!("SIREN {}", &**s),
            (None, CleDEmployeur::Nom(n)) => n.clone(),
            (None, CleDEmployeur::Seule(_)) => "employeur non lu".to_string(),
        };
        for (_, ligne, _) in registre.retenus {
            let a = &ligne.contrat;
            let Some(periode) = ligne.periode.and_then(|p| p.fermee()) else {
                notes.push(format!("Un contrat chez {} ({}) n'a pas de fin : il ne se compte pas.", employeur, import::attestes::en_mots(a.heures, a.cachets, a.jours, a.brut)));
                continue;
            };
            // La piece d'ou vient le contrat retenu (sa copie, sa page, ses dates) : son nom et son plafond ; une piece
            // qui ne se retrouve pas se dit, et son plafond se calcule sur la periode.
            let (id, plafond) = match siennes().find(|x| x.periode == periode && x.ligne.contrat.fichier == a.fichier && x.ligne.contrat.page == a.page) {
                Some(x) => (x.piece.clone(), x.plafond.clone()),
                None => {
                    let id = format!("paie du {} au {} chez {}", periode.debut().jj_mm_aaaa(), periode.fin().jj_mm_aaaa(), employeur);
                    notes.push(format!("La pièce de la {} ne se retrouve pas : son plafond se calcule sur ses dates. Le signaler au développeur.", id));
                    (id, SourceDuPlafond::DuContrat { contrat: None, paie: periode })
                }
            };
            // Son annexe : celle que dit sa piece, ou une autre piece du meme contrat (l'AEM qu'un bulletin a remplacee).
            let dite = siennes()
                .filter(|x| x.employeur == cle && x.periode == periode)
                .find_map(|x| x.statut.map(|s| (s, x.piece.as_str())));
            let annexe = annexe_de_la_paie(dite, a.cachets);
            out.push(PaieComptee {
                id,
                plafond,
                employeur: Some(employeur.clone()),
                debut: Some(periode.debut()),
                fin: periode.fin(),
                cachets: a.cachets,
                heures: a.heures,
                enseignement: a.heures_enseignement,
                formation: None,
                jours: a.jours,
                brut: a.brut,
                prevue: None,
                annexe,
            });
        }
    }
    Ok(out)
}

/// Les lignes prevues de la charge, en paies comptees ; une ligne qui ne se compte pas le dit dans `notes`.
fn prevues(lignes: &[LignePrevue], notes: &mut Vec<String>) -> R<Vec<PaieComptee>> {
    let mut out = Vec::new();
    for (rang, l) in lignes.iter().enumerate() {
        if l.cachets.is_none() && l.heures.is_none() {
            notes.push(format!("La ligne prévue qui finit le {} ne dit ni cachets ni heures : elle ne compte rien.", l.fin.jj_mm_aaaa()));
            continue;
        }
        // L'enseignement et la formation se comptent en heures : les cachets d'une telle ligne ne comptent pas.
        let (cachets, heures, enseignement, formation) = match l.nature {
            Some(n @ (NatureDUneLigne::Enseignement | NatureDUneLigne::Formation)) => {
                if l.cachets.is_some_and(|c| !c.est_nul()) {
                    notes.push(format!(
                        "La ligne prévue qui finit le {} est de {} : elle se compte en heures, ses cachets ne comptent pas.",
                        l.fin.jj_mm_aaaa(),
                        if n == NatureDUneLigne::Enseignement { "l'enseignement" } else { "la formation" }
                    ));
                }
                match n {
                    // Les heures d'enseignement sont dans les heures (`droit::cumul::Declare::heures`) ; celles de la
                    // formation a cote (`Declare::heures_formation`).
                    NatureDUneLigne::Enseignement => (None, l.heures, l.heures, None),
                    _ => (None, None, None, l.heures),
                }
            }
            _ => (l.cachets, l.heures, None, None),
        };
        // Son annexe, quand sa nature la dit ; sinon ses cachets (`annexe_de_la_paie`) ; l'enseignement et la formation
        // n'en ont pas.
        let annexe = match l.nature {
            Some(n) => n.annexe().map(|a| (a, "sa nature le dit".to_string())),
            None => annexe_de_la_paie(None, cachets),
        };
        out.push(PaieComptee {
            id: format!("ligne prévue n° {}", rang + 1),
            employeur: None,
            debut: None,
            fin: l.fin,
            cachets,
            heures,
            enseignement,
            formation,
            jours: None,
            brut: l.brut,
            plafond: SourceDuPlafond::SansDebut,
            prevue: Some(rang),
            annexe,
        });
    }
    Ok(out)
}

/// LA LIGNE DECLAREE D'UNE PAIE COMPTEE (`droit::cumul::Declare`), sous la cle `cle` de la personne ; aucune quand sa
/// piece ne dit pas ses heures, ses cachets et son brut. Un contrat prevu
/// declare ce qu'on y saisit : sans heures, il n'en declare aucune, sans cachets aucun, comme une paie de ce logiciel
/// (`engagement::cumul`) ; son brut, s'il le dit.
fn declaree(p: &PaieComptee, cle: &droit::contexte::IdSalarie) -> Option<Declare> {
    let (heures, cachets) = match (p.heures, p.cachets, p.prevue) {
        (Some(h), Some(c), _) => (h, c),
        (h, c, Some(_)) => (h.into_iter().sum(), c.into_iter().sum()),
        (_, _, None) => return None,
    };
    if p.prevue.is_none() && p.brut.is_none() {
        return None;
    }
    Some(Declare {
        salarie: cle.clone(),
        debut: p.debut,
        fin: p.fin,
        heures: heures.entieres(),
        cachets,
        brut: p.brut,
        heures_enseignement: p.heures_d_enseignement(),
        heures_formation: p.formation.into_iter().sum(),
    })
}

// ------------------------------------------------------------------ ce que la route rend

/// UNE LIGNE PAR PAIE `paies` (celles que la fenetre compte, `droits_d_une_personne`), en tableau
/// (`socle::ecran::TableauRendu`), dans l'ordre des fins : sa
/// fin, ses cachets, ses heures (l'enseignement compris ; la formation d'une ligne prevue), sa nature (`nature_dite` :
/// son annexe, celle de base, l'enseignement, la formation ; pour une ligne prevue, la case de sa nature), son brut ; un
/// compteur que la piece ne dit pas reste vide. L'employeur et la piece ne s'y montrent pas (Leandre, 27/11/2026 : « ça
/// alourdit ») : la liste des pieces les dit. Avec une allocation visee (`visee` : son montant, et le brut vise de
/// chaque paie par son nom), une colonne de plus, « Pour … € / jour » : le brut qu'il faudrait a chaque paie (Leandre,
/// 27/11/2026 : « pour qu'on voie le brut »).
/// Les lignes prevues s'y saisissent, chacune a sa place parmi les paies (son rang dans la case `previsionnel`) ; une
/// ligne prevue qui ne se compte pas (ni cachets ni heures) prend aussi sa place, a sa fin : `lignes`, celles de la
/// charge.
fn tableau_des_paies(paies: &[&PaieComptee], lignes: &[LignePrevue], base: Option<AnnexeChomage>, visee: Option<(Montant, &BTreeMap<String, Montant>)>) -> TableauRendu {
    let mut colonnes = vec![
        ColonneRendue::de_texte("Fin").tenant(&["fin"]),
        ColonneRendue::de_nombres("Cachets").tenant(&["cachets"]),
        ColonneRendue::de_nombres("Heures").tenant(&["heures"]),
        ColonneRendue::de_texte("Nature").tenant(&["nature"]),
        ColonneRendue::de_nombres("Brut").tenant(&["brut"]),
    ];
    if let Some((v, _)) = visee {
        colonnes.push(ColonneRendue::de_nombres_dite(format!("Pour {} € / jour", socle::texte::virgule(v))));
    }
    let vise = |id: &str| visee.map(|(_, par_paie)| par_paie.get(id).copied().map(euros));
    let mut rangees: Vec<(Jour, RangeeRendue)> = paies
        .iter()
        .map(|p| {
            let heures = p.heures.or(p.formation).map(dit);
            let mut cellules = vec![Some(p.fin.jj_mm_aaaa()), p.cachets.map(dit), heures, Some(p.nature_dite(base)), p.brut.map(euros)];
            cellules.extend(vise(&p.id));
            let rangee = match p.prevue {
                Some(rang) => RangeeRendue::saisie(cellules, rang),
                None => RangeeRendue::de(SorteDeRangee::Ligne, cellules),
            };
            (p.fin, rangee)
        })
        .collect();
    for (rang, l) in lignes.iter().enumerate().filter(|(rang, _)| !paies.iter().any(|p| p.prevue == Some(*rang))) {
        let nature = l.nature.map(|n| n.dite().to_string());
        let mut cellules = vec![Some(l.fin.jj_mm_aaaa()), l.cachets.map(dit), l.heures.map(dit), nature, l.brut.map(euros)];
        cellules.extend(visee.map(|_| None));
        rangees.push((l.fin, RangeeRendue::saisie(cellules, rang)));
    }
    // Le tri est stable : a la meme fin, les paies gardent l'ordre de `paies`, puis viennent les lignes qui ne se comptent pas.
    rangees.sort_by_key(|(fin, _)| *fin);
    let rangees = rangees.into_iter().map(|(_, r)| r).collect();
    let saisie = SaisieDuTableau { case: cle!(ChargeDroitsDesPieces, previsionnel), retirer: RETIRER_LA_LIGNE.texte() };
    TableauRendu::de(colonnes, rangees).intitule("Vos paies des 12 derniers mois").saisissant(saisie)
}

/// LES PAIES DU SALAIRE DE REFERENCE D'UNE ANNEXE, en tableau : le brut de chacune, son plafond et sa limite (quatre
/// plafonds, art. 11 §3), ce qu'elle y compte, et pourquoi son plafond ne se connait pas.
fn tableau_du_salaire(lignes: Vec<Vec<Option<String>>>) -> TableauRendu {
    let colonnes = vec![
        ColonneRendue::de_texte("Période"),
        ColonneRendue::de_texte("Employeur"),
        ColonneRendue::de_nombres("Brut"),
        ColonneRendue::de_nombres("Plafond"),
        ColonneRendue::de_nombres("Limite"),
        ColonneRendue::de_nombres("Compte"),
        ColonneRendue::discrete("Remarque"),
    ];
    TableauRendu::de_lignes(colonnes, lignes).intitule("Le salaire de référence, paie par paie")
}

/// L'allocation de l'annexe de la personne, estimee : ses parties, leur somme, le salaire de reference, ce qui est
/// retenu, et les paies du salaire (le detail du calcul, replie).
#[derive(Debug, Serialize)]
struct EstimationDite {
    annexe: &'static str,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    a: Montant,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    b: Montant,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    c: Montant,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    calculee: Montant,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    salaire_de_reference: Montant,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    retenue: Montant,
    paies: TableauRendu,
}

/// CE QUE L'ALLOCATION VISEE DEMANDE, calcule (`viser`) : la reponse (ce qu'il faudrait par cachet ou de l'heure), ce
/// qui arrete la hausse (dans le detail), et le brut vise de chaque paie, par son nom (la colonne du tableau des paies).
struct ViseeCalculee {
    reponse: String,
    arret: String,
    par_paie: BTreeMap<String, Montant>,
}

/// LA REPONSE, en tete de l'ecran (Leandre, 27/11/2026 : « Est-ce qu'on les a, est-ce qu'on les a pas ? Et le
/// logiciel, il sert à faire le calcul de taux ») : le nom, quand les pieces portent plusieurs personnes ; sa couleur
/// (`socle::ecran::couleur_de_l_etat` : vert quand les heures sont la, gris sinon) ; les heures (ou ce qui manque) ;
/// jusqu'a quand, quand il en manque ; l'allocation de l'annexe de la personne (ou la demande de l'annexe) ; et la
/// fenetre, en petit.
#[derive(Debug, Serialize)]
struct ReponseDite {
    #[serde(skip_serializing_if = "Option::is_none")]
    nom: Option<String>,
    couleur: &'static str,
    heures: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    jusqu_a: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    allocation: Option<String>,
    /// Le chomage de l'annee qui vient, sans aucun travail (`AnneeDite`), quand l'allocation s'estime.
    #[serde(skip_serializing_if = "Option::is_none")]
    annee: Option<String>,
    sous: String,
}

/// L'ANNEE DE CHOMAGE, telle que l'ecran la dit (Léandre, 27/11/2026 : le chiffre principal est le chomage de l'annee
/// sans aucun travail, « une explication de ce qu'on perd chaque jour travaillé, chiffrée avec exemple concret », et
/// « un exemple sur l'année avec l'année dernière ») : son titre ; l'annee sans travail ; la case du taux de CSG ; ce
/// que coute un jour de travail ; l'annee si elle ressemble a la derniere ; et ce que le calcul suppose
/// (`droit::annee_de_chomage`).
#[derive(Debug, Serialize)]
struct AnneeDite {
    titre: &'static str,
    sans_travail: SansTravailDit,
    case: crate::fiches::CaseDeLaFiche<&'static str>,
    cout: CoutDit,
    #[serde(skip_serializing_if = "Option::is_none")]
    comme: Option<CommeLaDerniereDite>,
    notes: Vec<String>,
}

/// UN CALCUL EN LIGNES : chaque ligne `[quoi, valeur]`, puis le total, en gras.
#[derive(Debug, Serialize)]
struct CalculDit {
    lignes: Vec<(String, String)>,
    total: (String, String),
}

/// L'ANNEE SANS AUCUN TRAVAIL : son titre, le chiffre (« au plus » quand une franchise ne se compte pas), ce qu'il compte
/// en petit, le calcul des jours payes puis celui du montant, et ce qui s'en dit (la CSG, ce qui manque).
#[derive(Debug, Serialize)]
struct SansTravailDit {
    titre: &'static str,
    chiffre: String,
    sous: String,
    calculs: Vec<CalculDit>,
    aide: Vec<String>,
}

/// CE QUE COUTE UN JOUR DE TRAVAIL : son titre, l'explication chiffree (art. 30), et des exemples, chacun son titre, son
/// calcul et ce qui s'en dit.
#[derive(Debug, Serialize)]
struct CoutDit {
    titre: &'static str,
    aide: String,
    exemples: Vec<ExempleDit>,
}

/// UN EXEMPLE CHIFFRE.
#[derive(Debug, Serialize)]
struct ExempleDit {
    titre: String,
    calcul: CalculDit,
    #[serde(skip_serializing_if = "Option::is_none")]
    aide: Option<String>,
}

/// L'ANNEE SI ELLE RESSEMBLE A LA DERNIERE : le meme travail aux memes mois ; son titre, ce qu'il est, le tableau des mois
/// (les heures, les jours payes, le chomage, le salaire brut, et les totaux), les barres de chaque mois, le calcul de
/// l'annee, et ce qui s'en dit (la difference avec l'annee sans travail).
#[derive(Debug, Serialize)]
struct CommeLaDerniereDite {
    titre: &'static str,
    aide: String,
    tableau: TableauRendu,
    barres: Vec<BarreDite>,
    calcul: CalculDit,
    dit: Vec<String>,
}

/// LA BARRE D'UN MOIS : son nom court, son chomage et son salaire brut (aucun : il n'y a pas de travail, ou sa paie ne
/// dit pas son brut), les montants tels qu'ils s'ecrivent.
#[derive(Debug, Serialize)]
struct BarreDite {
    mois: String,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    chomage: Montant,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::option::serialize")]
    salaire: Option<Montant>,
}

/// L'ALLOCATION VISEE, telle que l'ecran la dit : sa case (« Je voudrais »), son unite (« € par jour »), la reponse quand
/// elle est remplie (ce qu'il faudrait par cachet ou de l'heure), et le bouton qui la vide.
#[derive(Debug, Serialize)]
struct ViseeDite {
    case: crate::fiches::CaseDeLaFiche<&'static str>,
    unite: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    reponse: Option<String>,
    reinitialiser: &'static str,
}

/// LE DETAIL DU CALCUL, replie en bas de l'ecran : son libelle ; la fenetre ; le compte des heures (`[quoi, valeur]`,
/// jusqu'au seuil) ; le seuil et ce qui manque ; l'estimation de l'annexe de la personne (ses parties et les paies du
/// salaire de reference) ; et une phrase par ligne (le jour butoir, l'estimation, ce que les pieces ont d'etrange).
/// En tete, les heures de chaque annexe et celle qui s'applique (`annexes`, `AnnexesDesPaies::dites`).
#[derive(Debug, Serialize)]
struct DetailsDits {
    libelle: &'static str,
    annexes: Vec<String>,
    /// Le numero de la personne ; aucun : les lignes prevues seules, sans piece (`LIGNES_SEULES`).
    #[serde(skip_serializing_if = "Option::is_none")]
    nir: Option<Nir>,
    fenetre: String,
    compte: Vec<(String, String)>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    heures_exigees: Heures,
    /// Ce qui manque, compte a partir d'aujourd'hui quand la derniere fin de contrat est passee.
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    manque: Heures,
    #[serde(skip_serializing_if = "Option::is_none")]
    estimation: Option<EstimationDite>,
    lignes: Vec<String>,
}

/// Le libelle du detail du calcul.
const DETAILS: &str = "Détails du calcul";

/// CE QUE LE CALCUL DES DROITS DE LA PERSONNE DONNE A L'ECRAN (`ecran`) : la reponse ; si l'allocation visee s'offre,
/// et sa reponse ; l'annexe de base (`AnnexesDesPaies::base`, celle que prend une ligne prevue sans nature) ; le tableau
/// des paies ; les paies dont la date d'embauche se choisit, et la phrase qui la demande ; le detail.
struct CalculDesDroits {
    reponse: ReponseDite,
    visee_offerte: bool,
    visee: Option<String>,
    annee: Option<AnneeDite>,
    base: Option<AnnexeChomage>,
    paies: TableauRendu,
    embauches: Vec<crate::ouverture::Choix>,
    demande: Option<String>,
    details: DetailsDits,
}

/// LES ANNEXES DES PAIES D'UNE PERSONNE (annexes VIII et X, art. 65 §9 ; voir le module) : l'annexe de base, celle que
/// ses paies disent quand elles n'en disent qu'une (`base`) ; dans les 12 mois, les heures de chaque annexe, chaque paie
/// sous la sienne ou celle de base (`viii`, `x`), et celles des paies dont l'annexe ne se dit pas (`sans`, et leur nom,
/// `non_dites` ; `pieces_non_dites` : une paie des pieces en est, que nulle case ne fait dire) ; d'ou chaque annexe se
/// lit, paie par paie (`sources`).
/// Les heures d'une paie sont celles de son activite : ses cachets a 12 heures et ses heures, sans l'enseignement ni la
/// formation, que les annexes assimilent a du travail sans en faire l'activite d'une annexe (art. 3 §1 ; INFERENCE :
/// l'art. 65 §9 compare « les activités », la circulaire 2016-25 des heures de travail, exemple 7).
struct AnnexesDesPaies {
    base: Option<AnnexeChomage>,
    viii: Heures,
    x: Heures,
    sans: Heures,
    non_dites: Vec<String>,
    pieces_non_dites: bool,
    sources: Vec<String>,
}

/// L'ANNEXE QUI S'APPLIQUE : celle qui a le plus d'heures (`Une`), meme en donnant aux paies dont l'annexe ne se dit pas
/// toutes leurs heures a l'autre ; autant d'heures dans l'une que dans l'autre (`Egales` : le texte ne dit que « la plus
/// importante », rien ne se devine) ; ou elle depend des paies dont l'annexe ne se dit pas (`NonDite`).
#[derive(Clone, Copy)]
enum AnnexeAppliquee {
    Une(AnnexeChomage),
    Egales,
    NonDite,
}

impl AnnexesDesPaies {
    /// L'annexe qui s'applique.
    fn appliquee(&self) -> AnnexeAppliquee {
        let (viii, x, sans) = (self.viii, self.x, self.sans);
        if viii > x + sans {
            AnnexeAppliquee::Une(AnnexeChomage::Viii)
        } else if x > viii + sans {
            AnnexeAppliquee::Une(AnnexeChomage::X)
        } else if sans.est_nul() && !viii.est_nul() {
            AnnexeAppliquee::Egales
        } else {
            AnnexeAppliquee::NonDite
        }
    }

    /// Les heures de l'annexe `a`.
    fn heures(&self, a: AnnexeChomage) -> Heures {
        match a {
            AnnexeChomage::Viii => self.viii,
            AnnexeChomage::X => self.x,
        }
    }

    /// Ce que le detail en dit : les heures de chaque annexe, celles qui ne se disent pas, celle qui s'applique et
    /// pourquoi, et d'ou chaque annexe se lit.
    fn dites(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut heures = vec![format!("annexe 8 (technicien) : {} h", dit(self.viii)), format!("annexe 10 (artiste) : {} h", dit(self.x))];
        if !self.sans.est_nul() {
            heures.push(format!("annexe non dite : {} h ({})", dit(self.sans), self.non_dites.join(", ")));
        }
        out.push(format!("Les heures de chaque annexe sur les 12 mois (cachets à 12 h, sans l'enseignement ni la formation) : {}.", heures.join(" ; ")));
        out.push(match self.appliquee() {
            AnnexeAppliquee::Une(a) => format!(
                "L'annexe qui s'applique est celle qui a le plus d'heures, {} ; toutes vos heures et tous vos salaires comptent sous elle (annexes VIII et X, art. 65 §9).",
                annexe_en_petit(a)
            ),
            AnnexeAppliquee::Egales => "Les deux annexes ont autant d'heures : le texte applique celle « ayant permis de constater l'affiliation la plus importante » (annexes VIII et X, art. 65 §9) et ne dit rien d'une égalité ; l'allocation ne s'estime pas.".to_string(),
            AnnexeAppliquee::NonDite => "L'annexe qui s'applique, celle qui a le plus d'heures (annexes VIII et X, art. 65 §9), dépend des paies dont l'annexe n'est pas dite.".to_string(),
        });
        out.extend(self.sources.iter().cloned());
        out
    }
}

/// Ce que rend `droits-des-pieces` : sa phrase, et l'ecran, bloc par bloc dans l'ordre ou il se lit (`BlocDesDroits`).
#[derive(Debug, Serialize)]
struct DroitsDesPieces {
    phrase: Phrase,
    ecran: Vec<BlocDesDroits>,
}

/// UN BLOC DE L'ECRAN DES DROITS, dans l'ordre que la route lui donne (`ecran`) : une case de la charge ; la reponse
/// (les heures, l'allocation) ; l'allocation visee ; le tableau des paies, ou les lignes prevues se saisissent ; le
/// depot des pieces ; les pieces deposees et ce qui en est lu ; le detail du calcul.
#[derive(Debug, Serialize)]
#[serde(tag = "est", rename_all = "snake_case")]
enum BlocDesDroits {
    Case(crate::fiches::CaseDeLaFiche<&'static str>),
    Reponse(ReponseDite),
    Visee(ViseeDite),
    Annee(Box<AnneeDite>),
    Paies(PaiesDites),
    Depot(DepotDit),
    Pieces(PiecesDites),
    Details(Box<DetailsDits>),
}

/// LE TABLEAU DES PAIES : une ligne par paie, les lignes prevues s'y saisissant (`TableauRendu::saisie`), et la case des
/// lignes prevues (son libelle, son sens, ses parties et leurs formes, ses rangees).
#[derive(Debug, Serialize)]
struct PaiesDites {
    tableau: TableauRendu,
    lignes: crate::fiches::CaseDeLaFiche<&'static str>,
}

/// LE DEPOT DES PIECES : le dossier ou elles se posent (`mes-pieces/`), son libelle et ce qu'on y fait.
#[derive(Debug, Serialize)]
struct DepotDit {
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    dossier: PathBuf,
    libelle: &'static str,
    aide: &'static str,
}

/// LES PIECES DEPOSEES, une par fichier pose, et le bouton qui en retire une.
#[derive(Debug, Serialize)]
struct PiecesDites {
    pieces: Vec<PieceDite>,
    retirer: &'static str,
}

/// UNE PIECE DEPOSEE : le fichier pose (son nom dans `mes-pieces/`), sa nature (aucune : rien de reconnu), ses
/// employeurs (Leandre, 27/11/2026 : « ça peut apparaître dans le résumé des pièces en bas. L'employeur et la nature »),
/// et ce que la route en lit, une phrase chacune.
#[derive(Debug, Serialize)]
struct PieceDite {
    fichier: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    nature: Option<&'static str>,
    employeurs: Vec<String>,
    dit: Vec<String>,
}

/// Le bouton qui retire une piece deposee.
const RETIRER_LA_PIECE: &str = "Retirer";

impl Dite for DroitsDesPieces {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

/// Le nom d'une annexe, tel que la route le dit.
fn annexe_dite(annexe: AnnexeChomage) -> &'static str {
    match annexe {
        AnnexeChomage::Viii => "Technicien (annexe 8)",
        AnnexeChomage::X => "Artiste (annexe 10)",
    }
}

/// LES DROITS D'UN INTERMITTENT DEPUIS SES PIECES (voir le module).
fn droits_des_pieces(inst: &Path, c: ChargeDroitsDesPieces) -> R<DroitsDesPieces> {
    let dossier = inst.join(MES_PIECES.texte());
    if !dossier.exists() {
        std::fs::create_dir_all(&dossier).map_err(|e| socle::echec::du_disque(&dossier, &e).reclasser(Classe::Disque))?;
    }
    let travail = crate::import::dossier_de_travail();
    let lu = lire_le_dossier(inst, &dossier, &travail);
    if travail.exists() {
        std::fs::remove_dir_all(&travail).map_err(|e| socle::echec::du_disque(&travail, &e).reclasser(Classe::Disque))?;
    }
    let DossierDesPieces { attestees, pieces } = lu?;
    // Les personnes des pieces, par leur NIR, chacune son nom tel qu'une piece l'imprime.
    let mut personnes: BTreeMap<Nir, Option<String>> = BTreeMap::new();
    for a in &attestees {
        let n = personnes.entry(a.nir.clone()).or_insert(None);
        if n.is_none() {
            n.clone_from(&a.nom);
        }
    }
    let nom_de = |nir: &Nir| match personnes.get(nir).cloned().flatten() {
        Some(n) => n,
        None => format!("NIR {}", &**nir),
    };
    // Une personne que la charge choisit et qu'aucune piece ne porte plus (ses bulletins retires) ne compte pas, et se
    // dit, sous sa case : les autres et les lignes seules se comptent comme sans choix, et sa case reste pour qu'on la
    // vide.
    let mut sous_la_personne: Vec<String> = Vec::new();
    let choisie: Option<Nir> = match (c.personne.as_ref().filter(|n| personnes.contains_key(*n)), personnes.len()) {
        (Some(n), _) => Some(n.clone()),
        (None, 1) => personnes.keys().next().cloned(),
        (None, _) => None,
    };
    if let Some(n) = c.personne.as_ref().filter(|n| !personnes.contains_key(*n)) {
        sous_la_personne.push(format!("Aucune pièce ne porte le numéro de sécurité sociale {} choisi dans « Personne » : ce choix ne compte pas.", &**n));
    }
    let offerts = (personnes.len() > 1 || c.personne.is_some()).then(|| personnes.keys().map(|k| crate::ouverture::Choix::de(&**k, nom_de(k))).collect());
    // Sans piece qui porte une personne, les lignes prevues se comptent seules.
    let lignes_seules = personnes.is_empty() && c.previsionnel.as_ref().is_some_and(|v| !v.is_empty());
    // Le nom ne se dit que quand les pieces portent plusieurs personnes : il dit laquelle est choisie.
    let calcul = match &choisie {
        Some(nir) => Some(droits_d_une_personne(inst, &attestees, Some(nir), (personnes.len() > 1).then(|| nom_de(nir)), &c)?),
        None if lignes_seules => Some(droits_d_une_personne(inst, &attestees, None, None, &c)?),
        None => None,
    };
    // La phrase de la route : la reponse, en une ligne (les heures, jusqu'a quand, l'allocation).
    let phrase = match (&calcul, personnes.len()) {
        (Some(x), _) => {
            let r = &x.reponse;
            let heures: Vec<&str> = [Some(r.heures.as_str()), r.jusqu_a.as_deref()].into_iter().flatten().collect();
            let heures = heures.join(" ");
            let dite: Vec<&str> = [Some(heures.as_str()), r.allocation.as_deref(), r.annee.as_deref()].into_iter().flatten().collect();
            match &r.nom {
                Some(nom) => socle::phrase!("{} : {}.", nom, dite.join(". ")),
                None => socle::phrase!("{}.", dite.join(". ")),
            }
        }
        (None, 0) => Phrase::fixe("Aucune pièce ne se compte encore : posez vos bulletins, de tous vos employeurs, ou remplissez des lignes prévues."),
        (None, n) => socle::phrase!("Les pièces portent {} personnes : choisissez-en une, ses droits se comptent seuls.", n),
    };
    let ecran = ecran(&c, offerts.map(|o| (o, sous_la_personne)), calcul, personnes.is_empty(), dossier, pieces)?;
    Ok(DroitsDesPieces { phrase, ecran })
}

/// Le plafond de la Securite sociale de l'annee `an`, tel que le bareme livre le porte.
fn pss(an: Annee) -> R<droit::plafond::Pss> {
    match droit::contexte::socle_livre_lu(an)?.and_then(|p| p.plafond_securite_sociale) {
        Some(x) => Ok(droit::plafond::Pss::en_euros_entiers(x.mensuel, x.horaire)),
        None => Err(socle::refus!("le logiciel n'a pas le plafond de la Sécurité sociale de {}", an)),
    }
}


/// LE PLAFOND D'UNE PAIE (`droit::plafond`) : celui que le bulletin imprime ; sinon celui que le moteur calcule, pour un
/// artiste (`artiste`, l'annexe X) ou un technicien ; ou la raison pour laquelle il ne se connait pas. Un artiste engage
/// moins de cinq jours a le plafond de ses journees de travail (arrete du 24/01/1975, art. 3, al. 2) : la piece ne les
/// date pas, sauf un contrat d'un seul jour.
fn plafond_de(s: &SourceDuPlafond, artiste: bool) -> Result<Montant, PlafondInconnu> {
    use droit::plafond::{engagement_court, plafond_de_la_periode, plafond_urssaf, JoursSansRemuneration, JOURS_ENGAGEMENT_CONTINU};
    let inconnu = |raison: &str, le_debut_le_dit: bool| PlafondInconnu { raison: raison.to_string(), le_debut_le_dit };
    let (contrat, paie) = match s {
        SourceDuPlafond::Imprime(m) => return Ok(*m),
        SourceDuPlafond::DuContrat { contrat, paie } => (contrat, paie),
        SourceDuPlafond::SansDebut => return Err(inconnu("une ligne prévue ne dit que sa fin", false)),
    };
    let court = match contrat {
        Some(c) => engagement_court(c, artiste),
        None if !artiste || paie.duree() >= JOURS_ENGAGEMENT_CONTINU => false,
        None => return Err(inconnu("la pièce ne dit pas la date d'embauche : l'engagement de l'artiste peut être de moins de cinq jours", true)),
    };
    let r = match (court, contrat) {
        (false, _) => plafond_de_la_periode(paie, &JoursSansRemuneration::aucun(), None, &pss),
        (true, Some(c)) if c.duree() == Duree::jours(1) => plafond_urssaf(c, true, &pss, &[c.debut()]),
        (true, _) => return Err(inconnu("un artiste engagé moins de cinq jours a le plafond de ses journées de travail, que la pièce ne date pas", false)),
    };
    r.map_err(|e| PlafondInconnu { raison: e.phrase.to_string(), le_debut_le_dit: false })
}

/// LE PLUS BAS PLAFOND QU'UNE PAIE QUI FINIT LE `fin` PEUT AVOIR, par `droit::plafond` : le plafond du mois au prorata
/// d'un jour (un contrat d'un jour, ou un jour paye d'un contrat plus long) ; pour un artiste, aussi celui d'un
/// engagement court d'une journee de travail (douze fois le plafond horaire, arrete du 24/01/1975, art. 3, al. 2) : le
/// plus petit des deux.
fn plafond_le_plus_bas(fin: Jour, artiste: bool) -> Result<Montant, PlafondInconnu> {
    use droit::plafond::{plafond_de_la_periode, plafond_urssaf, JoursSansRemuneration};
    let inconnu = |e: socle::echec::Echec| PlafondInconnu { raison: e.phrase.to_string(), le_debut_le_dit: false };
    let jour = Periode::du_au(fin, fin).map_err(inconnu)?;
    let au_prorata = plafond_de_la_periode(&jour, &JoursSansRemuneration::aucun(), None, &pss).map_err(inconnu)?;
    if !artiste {
        return Ok(au_prorata);
    }
    let court = plafond_urssaf(&jour, true, &pss, &[fin]).map_err(inconnu)?;
    Ok(au_prorata.min(court))
}

/// Ce que la route dit de la personne `nir` (aucune : les lignes prevues seules) : la reponse (ses heures, ou ce qui
/// manque et jusqu'a quand ; son allocation, pour son annexe), ce que vise la charge, ce que les pieces disent de son
/// annexe, le tableau de ses paies, les paies dont la date d'embauche se choisit (celles dont la route la demande, et
/// celles que la charge en dit deja), et le detail du calcul. `nom` : celui qui se dit, quand les pieces portent
/// plusieurs personnes.
fn droits_d_une_personne(inst: &Path, attestees: &[Attestee], nir: Option<&Nir>, nom: Option<String>, c: &ChargeDroitsDesPieces) -> R<CalculDesDroits> {
    let aujourdhui = droit::contexte::aujourdhui()?;
    let constantes = Constantes::lire(inst, &aujourdhui)?;
    let t = droit::allocation::allocation()?;
    let naissance = nir.and_then(|nir| attestees.iter().filter(|a| a.nir == *nir).find_map(|a| a.naissance));
    let mut notes = Vec::new();
    let mut comptees = match nir {
        Some(nir) => retenues(attestees, nir, &mut notes)?,
        None => Vec::new(),
    };
    if let Some(v) = &c.previsionnel {
        comptees.extend(prevues(v, &mut notes)?);
    }
    // PAR DATE DE FIN, les paies lues et les lignes prevues melees : une ligne prevue prend sa place des qu'elle a sa fin
    // (Leandre, 27/11/2026 : « si on choisit le 2 mars, elle va se mettre a l'endroit du 2 mars »).
    comptees.sort_by(|a, b| (a.fin, &a.employeur).cmp(&(b.fin, &b.employeur)));
    // La personne se range sous son NIR (les lignes seules, sous `LIGNES_SEULES`) : une seule, que `position` ne mele a
    // personne.
    let cle = droit::contexte::IdSalarie::lire(match nir {
        Some(n) => &**n,
        None => LIGNES_SEULES,
    })?;
    let mut declarees: Vec<(Declare, usize)> = Vec::new();
    for (i, p) in comptees.iter().enumerate() {
        match declaree(p, &cle) {
            Some(d) => declarees.push((d, i)),
            None => notes.push(format!(
                "Ne se compte pas, {} : sa pièce ne dit pas ses heures, ses cachets et son brut ({}).",
                p.en_mots(),
                import::attestes::en_mots(p.heures, p.cachets, p.jours, p.brut)
            )),
        }
    }
    let lignes: Vec<Declare> = declarees.iter().map(|(d, _)| d.clone()).collect();
    // UNE SEULE QUESTION : la personne a-t-elle ses 507 heures sur les 12 mois (365 jours) avant sa derniere fin de
    // contrat, lignes prevues comprises (art. 3) ? Ni readmission, ni allongement (Leandre, 27/11/2026 : « on fait sur
    // 1 an […] les gens ils veulent savoir si ils ont leurs heures »). Par le seul compte des droits
    // (`droit::allocation::affiliation_des_droits`), sans date anniversaire : elle ne dit ici que jusqu'a quand.
    let r = &t.readmission;
    let aff = droit::allocation::affiliation_des_droits(inst, &constantes, r, &lignes, &cle, None, None, naissance)?;
    let fin = aff.position.periode.fin();
    let atteintes = aff.manque.est_nul();
    // CE QUI MANQUE, compte a partir d'aujourd'hui quand cette fin est passee : un contrat de plus finira au plus tot
    // aujourd'hui. Le meme compte (`droit::allocation::affiliation`), a une autre fin.
    let a_partir = match aujourdhui > fin && !atteintes {
        true => droit::allocation::affiliation(inst, &constantes, r, &lignes, &cle, aujourdhui, None, naissance)?,
        false => aff.clone(),
    };
    // JUSQU'A QUAND : la date anniversaire, quand elle est a venir ; sinon le jour ou la plus ancienne paie de la
    // fenetre en sort (`droit::allocation::butoir`, sans allongement).
    let anniversaire = c.anniversaire.filter(|da| *da >= aujourdhui);
    let butoir = match (atteintes, anniversaire) {
        (false, None) => droit::allocation::butoir(inst, &constantes, r, &lignes, &cle, naissance, &a_partir)?,
        (true, _) | (false, Some(_)) => None,
    };
    let (lignes_du_compte, notes_du_compte) = crate::archives::comptes_de_la_position(&aff, "des pièces")?;
    notes.extend(notes_du_compte);
    // Pourquoi ce jour, replie dans le detail : la paie qui sort des 12 mois, et ce qui manquera ensuite.
    if let Some(b) = &butoir {
        notes.push(pourquoi_le_butoir(b, &a_partir));
    }
    // Les paies que la fenetre compte : celles de la position retenue.
    let fenetre: Vec<(&Declare, &PaieComptee)> = declarees.iter().filter(|(d, _)| aff.position.engagements.contains(d)).map(|(d, i)| (d, &comptees[*i])).collect();
    let dans_la_fenetre: Vec<&PaieComptee> = fenetre.iter().map(|(_, p)| *p).collect();
    // L'ANNEXE : chaque paie la sienne, ou celle de base ; celle qui a le plus d'heures s'applique (`AnnexesDesPaies`).
    let annexes = annexes_des_paies(&comptees, &fenetre, &aff.position.constantes);
    let appliquee = annexes.appliquee();
    // Les debuts de contrat que la charge dit des paies des pieces, par le nom de leur paie ; un nom qu'aucune ne porte se
    // dit.
    let mut debuts: BTreeMap<String, Jour> = BTreeMap::new();
    for d in c.debuts.iter().flatten() {
        if comptees.iter().any(|p| p.id == d.paie && p.prevue.is_none()) {
            debuts.insert(d.paie.clone(), d.debut);
        } else {
            notes.push(format!("« {} » (Dates d'embauche) n'est le nom d'aucune paie des pièces : cette date ne dit rien.", d.paie));
        }
    }
    // L'ALLOCATION, quand les heures sont la : sous l'annexe qui s'applique, le chiffre exact (Léandre, 27/11/2026 : « il
    // n'y a pas de environ, c'est un calcul exact ») ; sans elle, la reponse dit ce qu'elle attend.
    let mut supposees: BTreeMap<String, (Option<Montant>, bool)> = BTreeMap::new();
    let estimee = match (atteintes, appliquee) {
        (true, AnnexeAppliquee::Une(a)) => estimer(&t, &aff, &dans_la_fenetre, a, annexes.base, c.allocation_visee, fin, &debuts, &mut supposees, &mut notes)?,
        (true, AnnexeAppliquee::Egales | AnnexeAppliquee::NonDite) | (false, _) => None,
    };
    // L'ANNEE DE CHOMAGE QUI VIENT, quand l'allocation s'estime (`droit::annee_de_chomage`).
    let annee = match (appliquee, &estimee) {
        (AnnexeAppliquee::Une(a), Some((e, _))) => Some(annee_dite(&t, a, e, &aff, &dans_la_fenetre, c)?),
        _ => None,
    };
    // LA REPONSE : les heures d'abord, puis l'allocation (la maquette de Léandre, 27/11/2026).
    let exigees = dit(a_partir.heures_exigees);
    let jusqu_a = match (atteintes, anniversaire, &butoir) {
        (true, _, _) | (false, None, None) => None,
        (false, Some(da), _) => Some(format!("d'ici votre date anniversaire, le {}", da.jj_mm_aaaa())),
        (false, None, Some(b)) => Some(format!("d'ici le {}", b.jour.jj_mm_aaaa())),
    };
    let reponse = match atteintes {
        true => {
            let allocation = match (appliquee, &estimee) {
                (AnnexeAppliquee::Une(_), Some((e, _))) => format!("Votre allocation : {} par jour", euros(e.retenue)),
                (AnnexeAppliquee::Une(_), None) => "L'allocation ne s'estime pas : le détail du calcul dit pourquoi".to_string(),
                (AnnexeAppliquee::Egales, _) => "L'allocation ne s'estime pas : autant d'heures en annexe 8 qu'en annexe 10, le détail du calcul dit pourquoi".to_string(),
                (AnnexeAppliquee::NonDite, _) if annexes.pieces_non_dites => "L'allocation ne s'estime pas : des paies ne disent pas leur annexe, le détail du calcul dit lesquelles".to_string(),
                (AnnexeAppliquee::NonDite, _) => "Votre allocation attend la nature de vos lignes prévues : artiste ou technicien".to_string(),
            };
            // L'annexe qui s'applique, et pourquoi, quand les paies en disent deux (la consigne de Léandre, 27/11/2026 :
            // « annexe 10 (artiste) : 260 h, contre 250 h en annexe 8 »).
            let sous = match appliquee {
                AnnexeAppliquee::Une(a) => {
                    let autre = match a {
                        AnnexeChomage::X => AnnexeChomage::Viii,
                        AnnexeChomage::Viii => AnnexeChomage::X,
                    };
                    match annexes.heures(autre).est_nul() {
                        true => format!("Sur les 12 mois au {} · {}", fin.jj_mm_aaaa(), annexe_en_petit(a)),
                        false => format!(
                            "Sur les 12 mois au {} · {} : {} h, contre {} h en annexe {}",
                            fin.jj_mm_aaaa(),
                            annexe_en_petit(a),
                            dit(annexes.heures(a)),
                            dit(annexes.heures(autre)),
                            autre.numero()
                        ),
                    }
                }
                AnnexeAppliquee::Egales => format!("Sur les 12 mois au {} · {} h en annexe 8, autant en annexe 10", fin.jj_mm_aaaa(), dit(annexes.viii)),
                AnnexeAppliquee::NonDite => format!("Sur les 12 mois au {}", fin.jj_mm_aaaa()),
            };
            ReponseDite {
                nom,
                couleur: socle::ecran::couleur_de_l_etat(socle::ecran::EtatDEcran::Fait),
                heures: format!("Vous avez vos heures : {} h sur {}", dit(aff.position.heures_ouverture()?), exigees),
                jusqu_a: None,
                allocation: Some(allocation),
                annee: annee.as_ref().map(|(_, phrase)| phrase.clone()),
                sous,
            }
        }
        false => {
            let a_la_date = a_partir.position.periode.fin();
            let fenetre = match a_la_date == aujourdhui {
                true => "sur les 12 derniers mois".to_string(),
                false => format!("sur les 12 mois au {}", a_la_date.jj_mm_aaaa()),
            };
            ReponseDite {
                nom,
                couleur: socle::ecran::couleur_de_l_etat(socle::ecran::EtatDEcran::Information),
                heures: format!("Il vous manque {} h", dit(a_partir.manque)),
                jusqu_a,
                allocation: None,
                annee: None,
                sous: format!("{} h sur {} {}", dit(a_partir.position.heures_ouverture()?), exigees, fenetre),
            }
        }
    };
    // Une paie des pieces dont le debut dirait le plafond se demande par la case « Dates d'embauche » ; ce que
    // l'estimation suppose sans pouvoir le demander va dans le detail. Une date d'embauche deja dite reste un choix de
    // sa case, meme quand elle a comble la demande.
    let demandees = |p: &&PaieComptee| supposees.get(&p.id).is_some_and(|(_, le_debut_le_dit)| *le_debut_le_dit);
    let embauches: Vec<crate::ouverture::Choix> = comptees
        .iter()
        .filter(|p| p.prevue.is_none() && (demandees(p) || debuts.contains_key(&p.id)))
        .map(|p| crate::ouverture::Choix::de(&p.id, choix_d_embauche(p)))
        .collect();
    let demande = demande_dite(&supposees);
    let demande = match (demande, embauches.is_empty()) {
        (Some(d), true) => {
            notes.push(d);
            None
        }
        (d, _) => d,
    };
    // Aucune ligne prevue dans la charge : aucune n'a de place a prendre.
    let lignes_prevues: &[LignePrevue] = match &c.previsionnel {
        Some(v) => v,
        None => &[],
    };
    let (estimation, visee) = match estimee {
        Some((e, v)) => (Some(e), v),
        None => (None, None),
    };
    if let Some(v) = &visee {
        notes.push(v.arret.clone());
    }
    // La colonne de l'allocation visee, quand une paie s'y augmente.
    let par_paie = visee.as_ref().map(|v| &v.par_paie).filter(|p| !p.is_empty());
    // Le tableau ne montre que les paies des pieces que la fenetre compte, celle des heures dites (Léandre, 27/11/2026 :
    // « Si elle ne compte pas, elle n'a pas à être affichée »), et les lignes prevues, qui s'y saisissent.
    let comptent = |i: usize| declarees.iter().any(|(d, j)| *j == i && a_partir.position.engagements.contains(d));
    let du_tableau: Vec<&PaieComptee> = comptees.iter().enumerate().filter(|(i, p)| p.prevue.is_some() || comptent(*i)).map(|(_, p)| p).collect();
    let paies = tableau_des_paies(&du_tableau, lignes_prevues, annexes.base, c.allocation_visee.zip(par_paie));
    Ok(CalculDesDroits {
        reponse,
        visee_offerte: estimation.is_some(),
        visee: visee.map(|v| v.reponse),
        annee: annee.map(|(a, _)| a),
        base: annexes.base,
        paies,
        embauches,
        demande,
        details: DetailsDits {
            libelle: DETAILS,
            annexes: annexes.dites(),
            nir: nir.cloned(),
            fenetre: aff.position.libelle_fenetre.clone(),
            compte: lignes_du_compte,
            heures_exigees: a_partir.heures_exigees,
            manque: a_partir.manque,
            estimation,
            lignes: notes,
        },
    })
}

/// L'annexe en petit, sous la reponse : « annexe 10 (artiste) ».
fn annexe_en_petit(annexe: AnnexeChomage) -> &'static str {
    match annexe {
        AnnexeChomage::Viii => "annexe 8 (technicien)",
        AnnexeChomage::X => "annexe 10 (artiste)",
    }
}

/// LES ANNEXES DES PAIES DE LA PERSONNE (`AnnexesDesPaies`) : l'annexe de base, celle que ses paies `comptees` disent
/// quand elles n'en disent qu'une (Léandre, 27/11/2026 : « celui de base pour ceux qui sont uniques ») ; puis, sur les
/// paies de la fenetre (`fenetre`, chacune avec sa ligne declaree), les heures d'activite de chaque annexe, chaque paie
/// sous la sienne, sinon sous celle de base, sinon parmi celles dont l'annexe ne se dit pas.
fn annexes_des_paies(comptees: &[PaieComptee], fenetre: &[(&Declare, &PaieComptee)], c: &Constantes) -> AnnexesDesPaies {
    let dites: Vec<AnnexeChomage> = comptees.iter().filter_map(|p| p.annexe.as_ref().map(|(a, _)| *a)).collect();
    let base = match dites.first() {
        Some(a) if dites.iter().all(|b| b == a) => Some(*a),
        _ => None,
    };
    let mut out = AnnexesDesPaies { base, viii: Heures::ZERO, x: Heures::ZERO, sans: Heures::ZERO, non_dites: Vec::new(), pieces_non_dites: false, sources: Vec::new() };
    for (d, p) in fenetre {
        let heures = d.heures_montant(c);
        if heures.est_nul() {
            continue;
        }
        match p.annexe_ou(base) {
            Some(AnnexeChomage::Viii) => out.viii += heures,
            Some(AnnexeChomage::X) => out.x += heures,
            None => {
                out.sans += heures;
                out.non_dites.push(p.en_mots());
                out.pieces_non_dites |= p.prevue.is_none();
            }
        }
        let d_ou = match &p.annexe {
            Some((_, d_ou)) => d_ou.clone(),
            None if base.is_some() => "l'annexe de vos autres paies".to_string(),
            None => match p.prevue {
                Some(_) => "sa nature n'est pas choisie".to_string(),
                None => "sa pièce ne la dit pas".to_string(),
            },
        };
        let annexe = match p.annexe_ou(base) {
            Some(a) => annexe_en_petit(a),
            None => "annexe non dite",
        };
        out.sources.push(format!("{}, {} h : {} ({}).", majuscule(&p.en_mots()), dit(heures), annexe, d_ou));
    }
    out
}

/// Une phrase qui commence par une capitale.
fn majuscule(s: &str) -> String {
    let mut c = s.chars();
    c.next().into_iter().flat_map(char::to_uppercase).chain(c).collect()
}

/// Le libelle d'une paie des pieces parmi les choix des dates d'embauche : son nom, ses dates, son employeur.
fn choix_d_embauche(x: &PaieComptee) -> String {
    format!("{} : {}", x.id, x.en_mots())
}

/// LA PHRASE QUI DIT CE QUE L'ESTIMATION SUPPOSE, une fois pour toutes les paies `supposees` (par leur nom : la plus
/// basse limite au-dessus de laquelle chacune peut etre limitee, aucune quand elle ne se connait pas, et si le debut de
/// son contrat le dirait) : une paie au-dessus de la plus basse de ces limites peut etre limitee si elle a dure peu de
/// jours ; la phrase demande quand elle a commence, quand cela le dirait. Aucune : rien n'est suppose.
fn demande_dite(supposees: &BTreeMap<String, (Option<Montant>, bool)>) -> Option<String> {
    if supposees.is_empty() {
        return None;
    }
    let paie = match supposees.values().filter_map(|(limite, _)| *limite).min() {
        Some(seuil) => format!("Une paie de plus de {}", euros(seuil)),
        None => "Une paie".to_string(),
    };
    Some(match supposees.values().any(|(_, le_debut_le_dit)| *le_debut_le_dit) {
        true => format!("{} peut être limitée si elle a duré peu de jours : dites quand elle a commencé.", paie),
        false => format!("{} peut être limitée si elle a duré peu de jours ; ses pièces ne permettent pas de le savoir.", paie),
    })
}

/// POURQUOI LE JOUR BUTOIR, en une phrase des details : les paies qui sortent des 12 mois le lendemain, et ce qui
/// manquera alors ; le texte (art. 3).
fn pourquoi_le_butoir(b: &droit::allocation::Butoir, aff: &Affiliation) -> String {
    let c = &aff.position.constantes;
    let sortent: Vec<&Declare> = aff.position.engagements.iter().filter(|d| !b.lendemain.position.engagements.contains(d)).collect();
    let heures: Heures = sortent.iter().map(|d| d.heures_equivalentes(c)).sum();
    let fins: Vec<String> = sortent.iter().map(|d| d.fin.jj_mm_aaaa()).collect();
    format!(
        "Après le {}, les {} h {} le {} sortent des 12 mois : il vous manquera {} h (annexes VIII et X, art. 3).",
        b.jour.jj_mm_aaaa(),
        dit(heures),
        if fins.len() > 1 { "des paies qui finissent" } else { "de la paie qui finit" },
        fins.join(" et le "),
        dit(b.lendemain.manque)
    )
}

/// L'ALLOCATION DE L'ANNEXE `annexe` sur la fenetre de `aff`, et ce que demande l'allocation `visee` ; aucune quand elle
/// ne s'estime pas (une note dit pourquoi). Le salaire de
/// reference : le brut des paies de la fenetre, chacune sous sa limite (quatre fois son plafond, annexes, art. 11 §3) ;
/// une paie toute d'enseignement en sort (guide de France Travail,
/// `chomage-intermittents/francetravail-guide-intermittents-20261020.txt` : « les salaires au titre des heures
/// d'enseignement ne sont pas retenus dans le SR ») ; une paie qui mele l'enseignement a d'autres heures ne dit pas leur
/// salaire a part : rien ne s'estime, et la note dit laquelle. Les heures travaillees : celles du montant (sans
/// l'enseignement : `Position::heures_montant`). Sur les 365 jours, les diviseurs de A et B sont ceux de l'annexe.
/// UNE PAIE DONT LE PLAFOND NE SE CONNAIT PAS compte entiere (Leandre, 27/11/2026 : « L'estimation sort toujours ») ;
/// quand son brut passe quatre fois le plus bas plafond qu'elle puisse avoir (`plafond_le_plus_bas`), elle peut etre
/// limitee : l'estimation le suppose, la remarque de la paie le dit, et une paie des pieces entre dans `supposees` (son
/// nom : cette plus basse limite, et si le debut de son contrat dirait son plafond), d'ou la route tire sa seule
/// demande (`demande_dite`). Une ligne prevue, qui ne dit que sa fin, compte entiere sans rien demander (Léandre,
/// 27/11/2026).
#[allow(clippy::too_many_arguments)]
fn estimer(
    t: &Allocation,
    aff: &Affiliation,
    paies: &[&PaieComptee],
    annexe: AnnexeChomage,
    base: Option<AnnexeChomage>,
    visee: Option<Montant>,
    fin: Jour,
    debuts: &BTreeMap<String, Jour>,
    supposees: &mut BTreeMap<String, (Option<Montant>, bool)>,
    notes: &mut Vec<String>,
) -> R<Option<(EstimationDite, Option<ViseeCalculee>)>> {
    let p = &aff.position;
    if let Some(m) = paies.iter().find(|x| x.melee()) {
        notes.push(format!(
            "L'allocation ne s'estime pas : {} mêle {} h d'enseignement à d'autres heures ou des cachets, et le salaire de l'enseignement, qui sort du salaire de référence, n'y est pas dit à part.",
            m.en_mots(),
            dit(m.heures_d_enseignement())
        ));
        return Ok(None);
    }
    for x in paies.iter().filter(|x| x.d_enseignement()) {
        notes.push(format!("Toute d'enseignement, {} sort du salaire de référence.", x.en_mots()));
    }
    for x in paies.iter().filter(|x| x.brut.is_none()) {
        notes.push(format!("La ligne prévue {} n'a pas de brut : elle compte pour les heures, pas pour l'allocation.", x.dates()));
    }
    let du_salaire: Vec<&&PaieComptee> = paies.iter().filter(|x| !x.d_enseignement() && x.brut.is_some()).collect();
    let annee = fin.annee();
    let pss_annuel = match droit::contexte::socle_livre_lu(annee)?.and_then(|s| s.plafond_securite_sociale) {
        Some(x) => x.annuel,
        None => {
            notes.push(format!("L'allocation ne s'estime pas : le logiciel n'a pas le plafond de la Sécurité sociale de {}.", annee));
            return Ok(None);
        }
    };
    let plafond = t.plafond(pss_annuel);
    let nht = p.heures_montant();
    notes.push("Estimation : sans maternité, adoption ou ALD dans la fenêtre ; l'allocation par jour est avant la participation retraite de 0,93 % (art. 19) et les franchises, que l'année de chômage déduit.".to_string());
    // D'ou vient le plafond de chaque paie, avec le debut de son contrat quand la charge le dit (`debuts`).
    let mut sources = Vec::new();
    for x in &du_salaire {
        let s = match debuts.get(&x.id) {
            Some(d) => match x.plafond.avec_le_debut(*d) {
                Some(s) => s,
                None => {
                    notes.push(format!("Le début dit pour {} ({}) est après sa fin : il ne dit rien.", x.en_mots(), d.jj_mm_aaaa()));
                    x.plafond.clone()
                }
            },
            None => x.plafond.clone(),
        };
        // Une paie sans plafond imprime ni contrat dont la periode est un mois entier : le plafond est celui du mois
        // (`SourceDuPlafond`, l'inference qu'elle ne fait que dire).
        if let SourceDuPlafond::DuContrat { contrat: None, paie } = &s {
            if *paie == Periode::du_mois(paie.debut().mois_civil()) {
                notes.push(format!(
                    "Sans plafond imprimé, {} paie le mois entier : sa limite (art. 11 §3) se calcule sur le plafond du mois, peut-être plus haut que celui de son emploi ; l'estimation peut être trop haute.",
                    x.en_mots()
                ));
            }
        }
        sources.push(s);
    }
    let mut lignes = Vec::new();
    let mut bruts: Vec<(Montant, Option<Montant>)> = Vec::new();
    let mut a_viser: Vec<(&PaieComptee, Montant, Option<Montant>)> = Vec::new();
    let mut inconnues: Vec<PlafondSuppose> = Vec::new();
    for (x, source) in du_salaire.iter().zip(&sources) {
        let Some(brut) = x.brut else { continue };
        // Le plafond d'une paie est celui de son emploi : artiste quand son annexe est la X. Une paie dont l'annexe ne se
        // dit pas prend celle qui s'applique, dont son activite est « considérée comme relevant » (circulaire Unedic
        // 2016-25, fiche 1, § 2.3.1, `unedic-circulaire-2016-25-annexes-8-10.txt` l. 2673-2674 ; INFERENCE : la phrase
        // range les activites pour le chomage, le plafond de la Securite sociale suit l'emploi).
        let artiste = match x.annexe_ou(base) {
            Some(a) => a == AnnexeChomage::X,
            None => annexe == AnnexeChomage::X,
        };
        // Un plafond qui ne se connait pas : la paie compte entiere. Sous quatre fois le plafond d'un contrat d'un
        // jour, elle ne passe pas sa limite, quel que soit le contrat ; au-dessus, l'estimation suppose qu'elle ne la
        // passe pas, et une paie des pieces le fait savoir (`supposees`).
        let (plafond_paie, limite, note) = match plafond_de(source, artiste) {
            Ok(m) => (Some(m), Some(t.limite_de_la_paie(m)), None),
            Err(inconnu) => {
                let (au_plus_bas, note) = match plafond_le_plus_bas(x.fin, artiste) {
                    Ok(bas) => {
                        let l = t.limite_de_la_paie(bas);
                        match brut <= l {
                            true => (Some(l), format!("son plafond ne se connaît pas ({}) ; son brut ne passe pas {}, quatre fois le plafond d'un emploi d'un jour : il compte entier", inconnu.raison, euros(l))),
                            false => (Some(l), format!("son plafond ne se connaît pas ({}) ; son brut passe {}, quatre fois le plafond d'un emploi d'un jour : il compte entier, comme s'il ne passait pas sa limite", inconnu.raison, euros(l))),
                        }
                    }
                    Err(bas) => (None, format!("son plafond ne se connaît pas ({}), ni le plus bas qu'il puisse être ({}) : il compte entier, comme s'il ne passait pas sa limite", inconnu.raison, bas.raison)),
                };
                if x.prevue.is_none() && au_plus_bas.is_none_or(|l| brut > l) {
                    supposer(supposees, x, au_plus_bas, inconnu.le_debut_le_dit);
                }
                // Une ligne prevue aussi : la visee dit ce qu'elle suppose d'elle (`viser`), sans rien demander.
                inconnues.push(PlafondSuppose { rang: bruts.len(), limite: au_plus_bas, le_debut_le_dit: inconnu.le_debut_le_dit, paie: x });
                (None, None, Some(note))
            }
        };
        let compte = droit::allocation::salaire_limite(&[(brut, limite)], Coefficient::UN);
        bruts.push((brut, limite));
        a_viser.push((**x, brut, limite));
        lignes.push(vec![Some(x.dates()), x.employeur.clone(), Some(euros(brut)), plafond_paie.map(euros), limite.map(euros), Some(euros(compte)), note]);
    }
    let total: Montant = bruts.iter().fold(Montant::ZERO, |a, (b, _)| a + *b);
    let sr = droit::allocation::salaire_limite(&bruts, Coefficient::UN);
    let e = t.estimer(annexe, sr, nht, plafond)?;
    let borne = match e.borne {
        Some(Borne::Plancher) => format!(" ; le calcul donne {}, le plancher de l'annexe est de {}", euros(e.calculee), euros(e.plancher)),
        Some(Borne::Plafond) => format!(" ; le calcul donne {}, le plafond est de {}", euros(e.calculee), euros(e.plafond)),
        None => String::new(),
    };
    let limitee = if sr < total { format!(" ; les paies ne comptent qu'à quatre fois leur plafond (art. 11 §3) : {} de salaire, {} retenus", euros(total), euros(sr)) } else { String::new() };
    notes.push(format!(
        "{} : A {} pour {} de salaire de référence, B {} pour {} h, C {}{}{}.",
        annexe_dite(annexe),
        euros(e.a),
        euros(sr),
        euros(e.b),
        dit(nht),
        euros(e.c),
        borne,
        limitee
    ));
    let estimation = EstimationDite { annexe: annexe_dite(annexe), a: e.a, b: e.b, c: e.c, calculee: e.calculee, salaire_de_reference: sr, retenue: e.retenue, paies: tableau_du_salaire(lignes) };
    let visee = match visee {
        Some(v) => Some(viser(t, annexe, v, sr, nht, plafond, &bruts, &a_viser, &inconnues, supposees)?),
        None => None,
    };
    Ok(Some((estimation, visee)))
}

/// LA PAIE `x`, QUE L'ESTIMATION SUPPOSE NON LIMITEE, dans `supposees` (`estimer`) : la plus basse de ses limites
/// possibles, d'une annexe a l'autre (`limite`, aucune quand elle ne se connait pas), et si le debut de son contrat dirait
/// son plafond.
fn supposer(supposees: &mut BTreeMap<String, (Option<Montant>, bool)>, x: &PaieComptee, limite: Option<Montant>, le_debut_le_dit: bool) {
    let deja = supposees.entry(x.id.clone()).or_insert((limite, le_debut_le_dit));
    deja.0 = match (deja.0, limite) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    };
    deja.1 |= le_debut_le_dit;
}

/// UNE PAIE DONT LE PLAFOND NE SE CONNAIT PAS, QUI COMPTE ENTIERE : `rang`, sa place parmi les bruts de l'annexe ;
/// `limite`, quatre fois le plus bas plafond qu'elle puisse avoir (`plafond_le_plus_bas` ; aucune quand il ne se connait
/// pas) ; si le debut de son contrat dirait son plafond. Au-dessus de cette limite, brute ou augmentee pour une
/// allocation visee, elle peut etre limitee : l'estimation suppose qu'elle ne l'est pas (`supposer`).
struct PlafondSuppose<'a> {
    rang: usize,
    limite: Option<Montant>,
    le_debut_le_dit: bool,
    paie: &'a PaieComptee,
}

/// CE QUE L'ALLOCATION `v` DEMANDE SOUS L'ANNEXE `annexe` : le salaire de reference, chaque paie augmentee dans la meme
/// proportion (son brut vise), ce que cela fait par cachet et de l'heure, et ce qui arrete la hausse : la limite des
/// paies (art. 11 §3), ou le plafond de l'allocation (art. 16), celle des deux qui vient la premiere.
#[allow(clippy::too_many_arguments)]
fn viser(
    t: &Allocation,
    annexe: AnnexeChomage,
    v: Montant,
    sr: Montant,
    nht: Heures,
    plafond: Montant,
    bruts: &[(Montant, Option<Montant>)],
    a_viser: &[(&PaieComptee, Montant, Option<Montant>)],
    inconnues: &[PlafondSuppose],
    supposees: &mut BTreeMap<String, (Option<Montant>, bool)>,
) -> R<ViseeCalculee> {
    let au_plus = droit::allocation::salaire_au_plus(bruts);
    let aj_de = |s: Montant| t.estimer(annexe, s, nht, plafond).map(|e| e.retenue);
    let pour_le_plafond = match t.salaire_vise(annexe, plafond, nht, plafond)? {
        Visee::Salaire(s) => Some(s),
        Visee::DesLePlancher | Visee::SansSalaire | Visee::AuDelaDuPlafond => None,
    };
    let arret = match (au_plus, pour_le_plafond) {
        (Some(m), Some(s)) if m < s => format!(
            "Ce qui arrête la hausse, c'est la limite des paies (art. 11 §3) : chacune ne compte qu'à quatre fois son plafond, soit au plus {} de salaire de référence et {} par jour, avant le plafond de {} par jour (art. 16).",
            euros(m),
            euros(aj_de(m)?),
            euros(plafond)
        ),
        (_, Some(s)) => format!("Au-delà de {} par jour, le plafond (art. 16), l'allocation ne monte plus : il y faut {} de salaire de référence.", euros(plafond), euros(s)),
        (Some(m), None) => format!("Les paies ne comptent qu'à quatre fois leur plafond (art. 11 §3) : au plus {} de salaire de référence ; le plafond de l'allocation est de {} par jour.", euros(m), euros(plafond)),
        (None, None) => format!("Le plafond de l'allocation est de {} par jour (art. 16).", euros(plafond)),
    };
    let vide = |reponse: String| ViseeCalculee { reponse, arret: arret.clone(), par_paie: BTreeMap::new() };
    let cible = match t.salaire_vise(annexe, v, nht, plafond)? {
        Visee::Salaire(s) => s,
        Visee::AuDelaDuPlafond => return Ok(vide(format!("{} par jour dépasse le plafond de l'allocation, {} par jour : aucun salaire n'y mène.", euros(v), euros(plafond)))),
        Visee::DesLePlancher => return Ok(vide(format!("{} par jour ne passe pas le plancher de l'annexe, {} par jour, qui s'applique dès que le droit est ouvert.", euros(v), euros(t.formule(annexe).plancher)))),
        Visee::SansSalaire => return Ok(vide(format!("Les heures seules donnent déjà {} par jour.", euros(v)))),
    };
    let Some(k) = droit::allocation::facteur_vise(bruts, cible) else {
        return Ok(match au_plus {
            Some(m) if m < cible => vide(format!(
                "Il faudrait {} de salaire de référence ; la limite des paies (art. 11 §3) l'arrête à {}, soit {} par jour.",
                euros(cible),
                euros(m),
                euros(aj_de(m)?)
            )),
            _ => vide(format!("Il faudrait {} de salaire de référence ; les paies des 12 mois n'ont aucun brut à augmenter.", euros(cible))),
        });
    };
    // Une paie dont le plafond ne se connait pas, augmentee au-dela de la plus basse limite qu'elle puisse avoir : la
    // visee suppose qu'elle ne la passe pas, et le dit avec ce qui arrete la hausse.
    let mut suppose = false;
    for s in inconnues.iter().filter(|s| bruts.get(s.rang).is_some_and(|(b, _)| s.limite.is_none_or(|l| *b * k > l))) {
        // Une ligne prevue ne dit que sa fin : rien ne se demande pour elle (Léandre, 27/11/2026).
        if s.paie.prevue.is_none() {
            supposer(supposees, s.paie, s.limite, s.le_debut_le_dit);
        }
        suppose = true;
    }
    let arret = match suppose {
        true => format!("{} Le calcul suppose que ces paies ne sont pas limitées.", arret),
        false => arret,
    };
    let atteinte = aj_de(cible)?;
    // Chaque paie augmentee dans la meme proportion : son brut vise (la colonne du tableau des paies) ; et ce que cela
    // fait par cachet (les paies faites de cachets seuls) et de l'heure (celles faites d'heures seules), aujourd'hui et
    // pour l'allocation visee (Léandre, 27/11/2026 : « on donne les deux montants »).
    let mut par_paie = BTreeMap::new();
    let (mut en_cachets, mut vise_en_cachets, mut cachets) = (Montant::ZERO, Montant::ZERO, Cachets::ZERO);
    let (mut en_heures, mut vise_en_heures, mut heures) = (Montant::ZERO, Montant::ZERO, Heures::ZERO);
    for (x, brut, _) in a_viser {
        let brut_vise = (*brut * k).au_centime();
        par_paie.insert(x.id.clone(), brut_vise);
        let (c, h) = (x.cachets.into_iter().sum::<Cachets>(), x.heures.into_iter().sum::<Heures>());
        if h.est_nul() && !c.est_nul() {
            (en_cachets, vise_en_cachets, cachets) = (en_cachets + *brut, vise_en_cachets + brut_vise, cachets + c);
        } else if c.est_nul() && !h.est_nul() {
            (en_heures, vise_en_heures, heures) = (en_heures + *brut, vise_en_heures + brut_vise, heures + h);
        }
    }
    let dits = |en_cachets: Montant, en_heures: Montant| -> Vec<String> {
        let cachet = (!cachets.est_nul()).then(|| format!("{} par cachet", euros(en_cachets.prorata(Cachets::UN, cachets).au_centime())));
        let heure = en_heures.par_heure(heures).map(|m| format!("{} de l'heure", euros(m)));
        cachet.into_iter().chain(heure).collect()
    };
    let vises = dits(vise_en_cachets, vise_en_heures);
    let aujourdhui = dits(en_cachets, en_heures);
    let reponse = match vises.is_empty() {
        false => format!("Il faudrait {}, sur vos paies des 12 mois (aujourd'hui : {}).", vises.join(", ou "), aujourdhui.join(", ")),
        true => format!("Il faudrait {} de salaire de référence sur vos paies des 12 mois (aujourd'hui : {}).", euros(cible), euros(sr)),
    };
    let (sens, ecart) = if k >= Coefficient::UN { ("augmentée", k - Coefficient::UN) } else { ("baissée", Coefficient::UN - k) };
    let arret = format!(
        "Pour {} par jour, il faut {} de salaire de référence au lieu de {}, chaque paie {} de {} % ; l'allocation serait de {}. {}",
        euros(v),
        euros(cible),
        euros(sr),
        sens,
        dit(ecart.en_taux().au_pair(1)),
        euros(atteinte),
        arret
    );
    Ok(ViseeCalculee { reponse, arret, par_paie })
}

// ------------------------------------------------------------------ l'annee de chomage qui vient

/// Le titre du bloc, et de ses parties (la maquette de Léandre, 27/11/2026).
const L_ANNEE: &str = "Votre année de chômage";
const SANS_TRAVAIL: &str = "Sans aucun travail";
const UN_JOUR_DE_TRAVAIL: &str = "Ce que coûte un jour de travail";
const COMME_LA_DERNIERE: &str = "Si l'année qui vient ressemble à la dernière";

/// Un montant « environ », a l'euro : « 21 081 € ».
fn environ(m: Montant) -> String {
    format!("{}\u{a0}€", socle::texte::par_milliers(m.a_l_euro()))
}

/// Des jours tels qu'on les lit (« 19,56 »).
fn jours_dits(j: Jours) -> String {
    socle::texte::virgule(j.a_deux_decimales())
}

/// Des jours qu'un calcul retire : « − 7 », ou « 0 ».
fn moins(j: Jours) -> String {
    match j.est_nul() {
        true => "0".to_string(),
        false => format!("−\u{a0}{}", jours_dits(j)),
    }
}

/// Un taux en points tel qu'on le lit : « 6,2 % ».
fn taux_dit(t: socle::grandeurs::Taux) -> String {
    format!("{}\u{a0}%", socle::texte::virgule(t))
}

/// CE QUI MANQUE A L'ANNEE, en une phrase (`droit::annee_de_chomage::Manque`) : le calcul dit ce qu'il ne peut pas
/// dire, et son chomage est « au plus ».
fn manque_dit(m: &Manque, fin: Jour) -> String {
    match m {
        Manque::JoursTravailles => "La franchise de congés payés ne se compte pas : une paie ne dit pas ses jours de travail (ceux de l'attestation employeur) ; le chômage dit est au plus.".to_string(),
        Manque::SalairesDeLaPeriode => "La franchise de salaires ne se compte pas : une paie ou une ligne prévue des 12 mois ne dit pas son brut ; le chômage dit est au plus.".to_string(),
        Manque::Smic => format!("La franchise de salaires ne se compte pas : le logiciel n'a pas le SMIC du {} ; le chômage dit est au plus.", fin.jj_mm_aaaa()),
        Manque::SalaireDuMois(mois) => format!(
            "En {}, une paie ne dit pas son brut : le plafond du cumul ne se compte pas, et le chômage du mois est au plus.",
            mois.libelle(socle::mois::Graphie::Accentuee)
        ),
        Manque::PlafondDeLAnnee(an) => format!(
            "Le logiciel n'a pas encore le plafond de la Sécurité sociale de {} : le plafond du cumul ne se compte pas en {}, et le chômage d'un mois travaillé est au plus.",
            an, an
        ),
    }
}

/// UN EXEMPLE DE TRAVAIL qui ne retire que des jours (art. 30) : `quoi` (son titre), le detail de ses heures, et ce
/// qu'il retire, en jours et en euros.
fn exemple_de_jours(t: &Allocation, annexe: AnnexeChomage, jour: &droit::annee_de_chomage::AllocationDuJour, quoi: String, detail: String, heures: Heures) -> R<ExempleDit> {
    let a = t.activite(annexe);
    let jt = droit::annee_de_chomage::activite_du_mois(t, annexe, Jours::ZERO, heures)?.jours_de_travail;
    let retires = droit::annee_de_chomage::retires(t, annexe, heures)?;
    Ok(ExempleDit {
        titre: quoi,
        calcul: CalculDit {
            lignes: vec![
                (detail, socle::texte::heures(heures)),
                (format!("Jours de travail ({} ÷ {})", socle::texte::virgule(heures), socle::texte::virgule(a.heures_par_jour)), jours_dits(jt)),
            ],
            total: (format!("Jours de chômage retirés ({} × {})", jours_dits(jt), socle::texte::virgule(a.coefficient)), jours_dits(retires)),
        },
        aide: Some(format!("Soit {} de chômage en moins le mois où il compte.", euros(jour.pour_les_jours(retires)))),
    })
}

/// L'ANNEE DE CHOMAGE QUI VIENT, de l'annexe `annexe` et de son estimation `e` sur la fenetre de `aff` et ses paies `paies`
/// (`droit::annee_de_chomage`), telle que l'ecran la dit (`AnneeDite`), et la phrase de la reponse. L'ouverture : les
/// salaires de toute la periode (art. 21 §1 b : toutes ses paies, quel que soit le regime ; aucune quand une paie ne dit pas
/// son brut), ses jours de travail (art. 21 §1 a : ceux de l'AEM, que dit la piece ; aucun quand une paie ne les dit pas :
/// une ligne prevue ne les dit pas), le SMIC de son dernier jour (`smic_horaire` du bareme livre). Le meme travail aux memes
/// mois : chaque paie de la fenetre au mois ou son contrat finit, douze mois plus tard ; ses heures, ses cachets a 12
/// heures et l'enseignement compris ; la formation, qu'on suit, n'est pas un travail et ne retire rien (art. 30 : « les
/// heures de travail effectuées » ; INFERENCE).
fn annee_dite(t: &Allocation, annexe: AnnexeChomage, e: &EstimationDite, aff: &Affiliation, paies: &[&PaieComptee], c: &ChargeDroitsDesPieces) -> R<(AnneeDite, String)> {
    use socle::ecriture::ValeurDeCase;
    use socle::liste::ListeFermee;
    let fin = aff.position.periode.fin();
    let ar = &t.activite_reprise;
    let a = t.activite(annexe);
    let salaires = paies.iter().try_fold(Montant::ZERO, |s, p| p.brut.map(|b| s + b));
    let jours_travailles = paies.iter().try_fold(Jours::ZERO, |s, p| p.jours.map(|j| s + j));
    let smic = match droit::contexte::socle_livre_lu(fin.annee())? {
        Some(p) => droit::contexte::smic_en_vigueur(&p.smic_horaire, &fin)?.map(|(_, v)| v),
        None => None,
    };
    let o = OuvertureDesDroits {
        annexe,
        allocation: e.retenue,
        salaire_de_reference: e.salaire_de_reference,
        heures: aff.position.heures_montant(),
        fin,
        salaires_de_la_periode: salaires,
        jours_travailles,
        smic_horaire: smic,
        csg: c.csg,
    };
    let pmss = |an: Annee| -> R<Option<Montant>> { Ok(droit::contexte::socle_livre_lu(an)?.and_then(|p| p.plafond_securite_sociale).map(|x| x.mensuel)) };
    let sans = droit::annee_de_chomage::annee(t, &o, &[], &pmss)?;
    let mut travail = Vec::new();
    let (mut cachets, mut heures) = (Cachets::ZERO, Heures::ZERO);
    for p in paies {
        let (cs, hs) = (p.cachets.into_iter().sum::<Cachets>(), p.heures.into_iter().sum::<Heures>());
        let h = cs * ar.heures_par_cachet + hs;
        if h.est_nul() {
            continue;
        }
        (cachets, heures) = (cachets + cs, heures + hs);
        travail.push(TravailDuMois { mois: p.fin.apres(Duree::mois(12))?.mois_civil(), heures: h, salaire: p.brut });
    }
    let comme = match travail.is_empty() {
        true => None,
        false => Some(droit::annee_de_chomage::annee(t, &o, &travail, &pmss)?),
    };
    let jour = &sans.jour;
    let (par_jour, nette) = jour.versee();
    let avant_csg = if nette { "" } else { " avant CSG et CRDS" };
    let au_plus = !sans.manques.is_empty();
    let chiffre = match au_plus {
        true => format!("au plus {}", environ(sans.versee)),
        false => environ(sans.versee),
    };
    let phrase = format!("Sans travailler, {} sur l'année qui vient{}", if au_plus { chiffre.clone() } else { format!("environ {}", chiffre) }, avant_csg);
    // SANS AUCUN TRAVAIL : les jours payes, puis le montant.
    let somme = |f: fn(&droit::annee_de_chomage::MoisDeChomage) -> Jours| -> Jours { sans.mois.iter().map(f).sum() };
    let conges = match (&sans.conges, jours_travailles) {
        (Some(f), Some(jt)) => (
            format!("Franchise congés payés (vos {} jours de travail ont acquis {} jours de congés)", jours_dits(jt), jours_dits(f.jours)),
            moins(somme(|m| m.conges)),
        ),
        _ => ("Franchise congés payés".to_string(), "non comptée".to_string()),
    };
    let franchise_salaires = match &sans.salaires {
        Some(f) => (
            format!(
                "Franchise salaires ({} jours : vos salaires sur le SMIC mensuel de {}, et votre salaire journalier moyen sur trois SMIC journaliers de {})",
                jours_dits(f.franchise.jours),
                euros(f.smic_mensuel),
                euros(f.smic_journalier)
            ),
            moins(somme(|m| m.franchise_salaires)),
        ),
        None => ("Franchise salaires".to_string(), "non comptée".to_string()),
    };
    let jours = CalculDit {
        lignes: vec![
            (format!("Jours de l'année, du {} au {}", sans.periode.debut().jj_mm_aaaa(), sans.periode.fin().jj_mm_aaaa()), jours_dits(sans.jours())),
            ("Délai d'attente".to_string(), moins(somme(|m| m.delai))),
            conges,
            franchise_salaires,
        ],
        total: ("Jours payés".to_string(), jours_dits(sans.jours_payes)),
    };
    let mut lignes = vec![
        (format!("Allocation : {} jours à {}", jours_dits(sans.jours_payes), euros(jour.brute)), euros(sans.brute)),
        (
            format!(
                "Participation retraite : {} par jour, {} de votre salaire journalier moyen de {} (art. 19)",
                euros(jour.participation),
                taux_dit(t.participation.taux),
                euros(jour.salaire_journalier_moyen)
            ),
            format!("−\u{a0}{}", euros(sans.participation)),
        ),
    ];
    if let (Some(par_jour), Some(total)) = (jour.contributions, sans.contributions) {
        if !total.est_nul() {
            lignes.push((format!("CSG et CRDS : {} par jour", euros(par_jour)), format!("−\u{a0}{}", euros(total))));
        }
    }
    let montant = CalculDit { lignes, total: ("Sur l'année".to_string(), euros(sans.versee)) };
    let k = &t.contributions;
    let mut aide = vec![match (c.csg, jour.contributions) {
        (_, None) => format!(
            "Votre taux de CSG n'est pas dit : le montant est avant CSG ({} ou {}) et CRDS ({}), prises sur l'allocation abattue de {}, sans la faire passer sous {} par jour. Choisissez votre taux ci-dessous.",
            taux_dit(k.csg),
            taux_dit(k.csg_reduite),
            taux_dit(k.crds),
            taux_dit(k.abattement),
            euros(k.plancher)
        ),
        (Some(TauxDeCsg::Exonere), Some(_)) => "Exonéré de CSG : ni CSG ni CRDS.".to_string(),
        (_, Some(x)) if x.est_nul() => format!("Votre allocation, après la participation, est de {} par jour, pas plus de {} : ni CSG ni CRDS.", euros(jour.apres_participation), euros(k.plancher)),
        (_, Some(x)) => format!("CSG et CRDS : {} par jour, prises sur l'allocation abattue de {}, sans la faire passer sous {}.", euros(x), taux_dit(k.abattement), euros(k.plancher)),
    }];
    aide.push("Avant l'impôt sur le revenu.".to_string());
    aide.extend(sans.manques.iter().map(|m| manque_dit(m, fin)));
    let sans_travail = SansTravailDit {
        titre: SANS_TRAVAIL,
        chiffre,
        sous: format!("{} jours payés à {} par jour{}", jours_dits(sans.jours_payes), euros(par_jour), avant_csg),
        calculs: vec![jours, montant],
        aide,
    };
    // CE QUE COUTE UN JOUR DE TRAVAIL (art. 30), et ses exemples : un cachet, une journee de repetition, un mois mele
    // pour un artiste ; une journee et un mois pour un technicien, qui n'a pas de cachets.
    let hpj = socle::texte::virgule(a.heures_par_jour);
    let coef = socle::texte::virgule(a.coefficient);
    let huit = Heures::fixes(8);
    let mut cout = format!(
        "Le chômage se compte en heures : toutes celles du mois, cachets et répétitions ensemble ; {} heures font un jour de travail, et chaque jour de travail retire {} jour de chômage le même mois (annexe {}, art. 30). À {} jours de travail dans le mois, rien n'est versé.",
        hpj,
        coef,
        annexe.numero(),
        jours_dits(a.jours_sans_allocation)
    );
    let dit_de = |heures: Heures| -> R<String> {
        let j = droit::annee_de_chomage::retires(t, annexe, heures)?;
        Ok(format!("{} jour, soit {}", jours_dits(j), euros(jour.pour_les_jours(j))))
    };
    let heure = dit_de(Heures::UNE)?;
    let mut exemples = Vec::new();
    // Le mois des exemples : le premier mois entier de l'annee.
    let mois_entier = sans.mois.iter().find(|m| m.activite.jours_du_mois == Jours::fixes(m.mois.jours()));
    let (mois_mele, detail_mele, heures_melees) = match annexe {
        AnnexeChomage::X => {
            let cachet = Cachets::UN * ar.heures_par_cachet;
            cout.push_str(&format!(
                " Un cachet compte {} : il retire {}. Une heure retire {}. Une journée de répétition de 8 heures retire {}.",
                socle::texte::heures(cachet),
                dit_de(cachet)?,
                heure,
                dit_de(huit)?
            ));
            exemples.push(exemple_de_jours(t, annexe, jour, "Un cachet".to_string(), "Heures : un cachet".to_string(), cachet)?);
            exemples.push(exemple_de_jours(t, annexe, jour, "Une journée de répétition, 2 services de 4 heures".to_string(), "Heures : 2 × 4 h".to_string(), huit)?);
            let six = Cachets::fixes(6) * ar.heures_par_cachet;
            ("6 cachets et 2 journées de répétition de 8 heures", format!("Heures : 6 × {} + 2 × 8 h", socle::texte::heures(cachet)), six + huit + huit)
        }
        AnnexeChomage::Viii => {
            cout.push_str(&format!(" Une heure retire {}. Une journée de 8 heures retire {}.", heure, dit_de(huit)?));
            exemples.push(exemple_de_jours(t, annexe, jour, "Une journée de 8 heures".to_string(), "Heures : une journée".to_string(), huit)?);
            ("11 journées de 8 heures", "Heures : 11 × 8 h".to_string(), huit.fois(11))
        }
    };
    if let Some(m) = mois_entier {
        let nom = m.mois.nom(socle::mois::Graphie::Accentuee);
        let x = droit::annee_de_chomage::activite_du_mois(t, annexe, m.activite.jours_du_mois, heures_melees)?;
        let total = match x.au_dela_du_seuil {
            true => (format!("Jours payés en {} : {} jours de travail ou plus, rien n'est versé", nom, jours_dits(a.jours_sans_allocation)), "0".to_string()),
            false => (format!("Jours payés en {} ({} − {})", nom, jours_dits(x.jours_du_mois), jours_dits(x.retires)), jours_dits(x.indemnisables)),
        };
        exemples.push(ExempleDit {
            titre: format!("Un mois mêlé : en {}, {}", nom, mois_mele),
            calcul: CalculDit {
                lignes: vec![
                    (detail_mele, socle::texte::heures(heures_melees)),
                    (format!("Jours de travail ({} ÷ {})", socle::texte::virgule(heures_melees), hpj), jours_dits(x.jours_de_travail)),
                    (format!("Jours de chômage retirés ({} × {})", jours_dits(x.jours_de_travail), coef), jours_dits(x.retires)),
                ],
                total,
            },
            aide: Some(format!(
                "Soit {} de chômage en {}, au lieu de {} sans travail (hors délai d'attente et franchises).",
                euros(jour.pour_les_jours(x.indemnisables)),
                nom,
                euros(jour.pour_les_jours(x.jours_du_mois))
            )),
        });
    }
    let cout = CoutDit { titre: UN_JOUR_DE_TRAVAIL, aide: cout, exemples };
    // L'ANNEE SI ELLE RESSEMBLE A LA DERNIERE.
    let mut notes = vec![
        "Le travail d'un mois compte au mois où le contrat finit ; la formation qu'on suit n'est pas un travail et ne retire rien.".to_string(),
        "La franchise de congés payés compte les jours de travail de l'attestation employeur, pas les heures.".to_string(),
    ];
    let comme = match comme {
        None => None,
        Some(cm) => {
            let mot = |n: Cachets, h: Heures| -> String {
                let c = (!n.est_nul()).then(|| format!("{} cachet{}", socle::texte::virgule(n), if socle::texte::au_pluriel(n) { "s" } else { "" }));
                let h = (!h.est_nul()).then(|| socle::texte::heures(h));
                c.into_iter().chain(h).collect::<Vec<_>>().join(" et ")
            };
            let chomage = if nette { "Chômage" } else { "Chômage avant CSG" };
            let colonnes = vec![
                ColonneRendue::de_texte("Mois"),
                ColonneRendue::de_nombres("Heures"),
                ColonneRendue::de_nombres("Jours payés"),
                ColonneRendue::de_nombres(chomage),
                ColonneRendue::de_nombres("Salaire brut"),
            ];
            let mut rangees: Vec<RangeeRendue> = cm
                .mois
                .iter()
                .map(|m| {
                    RangeeRendue::de(
                        SorteDeRangee::Ligne,
                        vec![
                            Some(m.mois.libelle(socle::mois::Graphie::Abregee)),
                            (!m.activite.heures.est_nul()).then(|| socle::texte::heures(m.activite.heures)),
                            Some(jours_dits(m.jours_payes)),
                            Some(euros(m.versee)),
                            m.salaire.map(euros),
                        ],
                    )
                })
                .collect();
            let brut = cm.salaires();
            let heures_annee: Heures = cm.mois.iter().map(|m| m.activite.heures).sum();
            rangees.push(RangeeRendue::de(
                SorteDeRangee::TotalGeneral,
                vec![Some("Sur l'année".to_string()), Some(socle::texte::heures(heures_annee)), Some(jours_dits(cm.jours_payes)), Some(euros(cm.versee)), brut.map(euros)],
            ));
            let barres = cm.mois.iter().map(|m| BarreDite { mois: m.mois.nom(socle::mois::Graphie::Abregee).to_string(), chomage: m.versee, salaire: m.salaire }).collect();
            let retire = sans.versee - cm.versee;
            let mut dit = vec![match brut {
                Some(b) => format!("Ce travail vous rapporte {} de salaire brut, et retire {} de chômage.", environ(b), environ(retire)),
                None => format!("Ce travail retire {} de chômage ; une paie ne dit pas son brut, son salaire ne se dit pas.", environ(retire)),
            }];
            dit.push("Le salaire net et les congés de la Caisse des Congés Spectacles ne se disent pas ici : il y faudrait les bulletins d'une compagnie.".to_string());
            for m in &cm.manques {
                let phrase = manque_dit(m, fin);
                if !sans.manques.contains(m) && !dit.contains(&phrase) {
                    dit.push(phrase);
                }
            }
            // Le plafond du cumul (art. 30) : les mois ou il ramene le chomage, ou qu'il ne joue pas.
            let plafonnes: Vec<String> = cm.mois.iter().filter(|m| matches!(m.cumul, Cumul::Plafonne(_))).map(|m| m.mois.libelle(socle::mois::Graphie::Accentuee)).collect();
            let un_plafond = cm.mois.iter().find_map(|m| match m.cumul {
                Cumul::Plafonne(p) | Cumul::SousLePlafond(p) => Some(p),
                Cumul::NeJouePas | Cumul::Inconnu => None,
            });
            if let Some(p) = un_plafond {
                notes.push(match plafonnes.is_empty() {
                    true => format!(
                        "Le plafond du cumul : le salaire brut et le chômage d'un mois ne dépassent pas {} ({} fois le plafond mensuel de la Sécurité sociale, art. 30) ; il ne joue pas ici.",
                        euros(p),
                        socle::texte::virgule(ar.plafond_du_cumul)
                    ),
                    false => format!(
                        "Le plafond du cumul : le salaire brut et le chômage d'un mois ne dépassent pas {} ({} fois le plafond mensuel de la Sécurité sociale, art. 30) ; il ramène le chômage en {}.",
                        euros(p),
                        socle::texte::virgule(ar.plafond_du_cumul),
                        plafonnes.join(", ")
                    ),
                });
            }
            Some(CommeLaDerniereDite {
                titre: COMME_LA_DERNIERE,
                aide: format!("Le même travail, aux mêmes mois : {}.", mot(cachets, heures)),
                tableau: TableauRendu::de(colonnes, rangees),
                barres,
                calcul: CalculDit {
                    lignes: vec![("Chômage sans travailler".to_string(), euros(sans.versee)), ("Chômage avec ce travail".to_string(), euros(cm.versee))],
                    total: ("Chômage que ce travail retire".to_string(), euros(retire)),
                },
                dit,
            })
        }
    };
    notes.push("Estimation : sans maternité, adoption ou maladie de longue durée, sans différé d'indemnisation ; l'année va du lendemain de la dernière fin de contrat à sa date anniversaire.".to_string());
    let choix = TauxDeCsg::TOUTES.iter().map(|x| Ok(crate::ouverture::Choix::de(x.code()?, x.libelle()?))).collect::<R<Vec<_>>>()?;
    let case = case_simple(cle!(ChargeDroitsDesPieces, csg), c.csg.as_ref().and_then(ValeurDeCase::de))?.offrant(choix);
    Ok((AnneeDite { titre: L_ANNEE, sans_travail, case, cout, comme, notes }, phrase))
}
