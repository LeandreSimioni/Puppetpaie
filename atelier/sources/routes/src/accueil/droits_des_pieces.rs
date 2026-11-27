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
//! LES LIGNES PREVUES : la charge peut porter des contrats a venir, saisis a la main (`LignePrevue` : leurs dates, des
//! cachets ou des heures, l'enseignement, un brut facultatif) ; chacune se compte comme une paie « prévisionnel », et rien
//! ne s'ecrit. Sans brut, elle compte pour les heures, pas pour l'allocation. Sans piece qui porte une personne, les
//! lignes se comptent seules (« simple pour les simples » : on pose ses bulletins, ou l'on remplit des lignes).
//!
//! LES CASES : la route rend les cases de sa charge telles qu'on les saisit (`cases` : libelle, forme, valeur, choix,
//! parties des rangees), chacune tiree de la declaration de son champ ; la page de puppetpaie.fr les dessine, et qui lit
//! la reponse en ligne de commande voit les memes.
//!
//! LE PLAFOND D'UNE PAIE, pour la limite du salaire de reference (art. 11 §3) : quand il ne se connait pas, la paie compte
//! entiere ; sous quatre fois le plus bas plafond possible (un contrat d'un jour), elle ne peut pas passer sa limite ;
//! au-dessus, elle le peut si elle a dure peu de jours : l'estimation sort quand meme, et dit qu'elle suppose ces paies
//! non limitees (Leandre, 27/11/2026 : « L'estimation sort toujours »). La route demande alors, en une phrase, le debut
//! de leur contrat quand il le dirait : dans sa ligne pour une ligne prevue (la case « Début », que la rangee ouvre),
//! par la case « Dates d'embauche » (`debuts`) pour une paie des pieces.
//!
//! CE QUE LA ROUTE REND, pour la personne : une ligne par paie retenue (la periode, l'employeur, les cachets, les heures
//! dont l'enseignement, le brut, la piece) ; une seule question, les 507 heures sur les 12 mois (365 jours) avant la
//! derniere fin de contrat, lignes prevues comprises (art. 3 ; `droit::allocation::affiliation_des_droits`, sans
//! readmission ni allongement, compte par le meme code que la route `droits` d'une compagnie, `droit::cumul::Position`,
//! `archives::comptes_de_la_position`) : les heures, ou ce qui manque et jusqu'a quand (la date anniversaire quand elle
//! est a venir, sinon le jour ou la plus ancienne paie sort des 12 mois) ; l'allocation journaliere estimee de l'annexe VIII et de l'annexe X
//! (`droit::allocation`), chaque paie sous sa limite (art. 11 §3) ; et, avec une allocation visee, le salaire de
//! reference qu'elle demande, chaque paie augmentee dans la meme proportion, et ce qui arrete la hausse (la limite des
//! paies, ou le plafond de l'allocation). Des chiffres, pas un conseil.
//!
//! CE QUI NE SE COMPTE PAS ICI, et la route le dit : une DSN (ses contrats se comptent par leurs AEM ou leurs bulletins),
//! un certificat Congés Spectacles, une piece qui ne se lit pas ; et, dans l'estimation, le salaire annuel de reference
//! (maternite, adoption, ALD), la participation de 0,93 % (art. 19), la clause de rattrapage, les franchises.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use droit::allocation::{Affiliation, Allocation, Borne, Visee};
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
    exemple: r#"{"allocation_visee":80,"previsionnel":[{"fin":"2026-12-12","cachets":1,"brut":250}]}"#,
    rend: "phrase, et ecran : les blocs dans l'ordre où ils se lisent, chacun son `est`. Avec un calcul : la personne (une case, quand elle se choisit), le bilan, les paies, le résumé, l'allocation visée (une case, quand une allocation s'estime), la date anniversaire (une case), le dépôt, les dates d'embauche (une case, quand la route en demande pour une paie des pièces), les pièces ; sans calcul : la personne, la date anniversaire, le dépôt, les paies (vides, quand aucune pièce ne porte de personne), les pièces. case : libelle, forme, valeur (les nombres tels qu'on les lit), choix, parties, ajouter et retirer d'une rangée, dit. depot : dossier (mes-pieces/, où poser les pièces), libelle, aide. pieces : pieces [fichier (le nom posé dans mes-pieces/), nature (quand elle est reconnue), employeurs (ceux que ses pièces disent), dit (ce qui en est lu)], retirer. paies : tableau (titre, colonnes, rangées de cellules dites ; une ligne par paie, dans l'ordre des fins : période (la fin d'une ligne prévue, et son début dans la rangée qui l'ouvre), cachets, heures, enseignement, brut ; les lignes prévues s'y saisissent, chacune à sa fin, une ligne vide attendant en bas : saisie [case, retirer], les parties que tient chaque colonne, le rang de chaque rangée prévue et les parties qu'elle ouvre) et lignes (la case des lignes prévues : ses parties, ses rangées). bilan (au-dessus des paies) : nom (seulement quand les pièces portent plusieurs personnes), bilan (la réponse : les 507 h sur les 12 mois avant la dernière fin de contrat ; ou ce qui manque, compté à partir d'aujourd'hui quand cette fin est passée, d'ici la date anniversaire quand elle est à venir, sinon d'ici le jour où la plus ancienne paie sort des 12 mois). resume (sous les paies ; la personne choisie, ou les lignes prévues seules sans pièce) : nir, fenetre, lignes [quoi, valeur], heures_exigees, manque, demande (une phrase, quand une paie que l'estimation suppose non limitée peut l'être), estimations par annexe [a, b, c, calculee, salaire_de_reference, retenue, phrase (le chiffre, en une ligne), paies : un tableau avec plafond, limite, ce qui compte], visee par annexe [salaire_de_reference, allocation, phrase, arret, paies : un tableau avec brut visé, limite, par cachet, par heure], details (replié : libelle, lignes ; le détail des estimations et les notes)",
    refus: &[Refus { classe: Classe::Refuse, corriger: "les pièces que la phrase nomme ; rien n'est écrit" }, OPERATION],
    pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
    acte: Acte::Lit,
    phrase: 200,
    ecran: Ecran::Lecture,
}];

charge! {
    /// La charge de `droits-des-pieces` : la personne (aucune : la seule des pieces, ou aucune s'il y en a plusieurs), la
    /// date anniversaire (aucune : ce qui manque se compte jusqu'a la sortie de la plus ancienne paie), l'allocation visee (aucune : rien ne se
    /// vise), les lignes prevues (aucune : les pieces seules).
    #[derive(Debug)]
    struct ChargeDroitsDesPieces {
        personne: Option<Nir> = legende("Personne", "son numéro de sécurité sociale, parmi ceux que portent les pièces ; absent : la seule qu'elles portent ; un numéro qu'elles ne portent plus ne compte pas").rendue(),
        anniversaire: Option<Jour> = legende("Date anniversaire", "celle que France Travail a notifiée : les heures qui manquent se comptent jusqu'à elle ; absente ou passée : jusqu'au jour où la plus ancienne paie sort des 12 mois"),
        allocation_visee: Option<Montant> = legende("Allocation visée", "l'allocation journalière qu'on voudrait, en euros ; absente : rien ne se vise"),
        previsionnel: Option<Vec<LignePrevue>> = legende("Lignes prévues", "le travail à venir, une ligne par engagement : sa fin (son début, dans la ligne qui le demande), ses cachets ou ses heures, l'enseignement, son brut ; rien ne s'écrit"),
        debuts: Option<Vec<DebutDUnContrat>> = legende("Dates d'embauche", "la date d'embauche d'une paie des pièces dont la route la demande (la case Dates d'embauche, ses choix), sous le nom de la paie ; absente : rien n'est dit"),
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

/// UNE LIGNE PREVUE : un contrat a venir, saisi a la main pour un previsionnel. Sa fin (la ligne declaree se situe par
/// elle, `droit::cumul::Declare`) ; son debut, facultatif : il ne sert qu'au plafond de la paie, et ne se saisit que
/// dans la rangee qui l'ouvre, quand la route le demande ou qu'il est dit (`tableau_des_paies`) ; des cachets ou des heures ; si
/// ses heures sont de l'enseignement ; son brut, facultatif (sans lui, la ligne compte pour les heures, pas pour
/// l'allocation).
/// Elle s'ecrit aussi dans la valeur de sa case (`cases`), ses nombres tels qu'on les lit (`a_lire`).
#[derive(Debug, Clone, serde::Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct LignePrevue {
    fin: Jour,
    debut: Option<Jour>,
    #[serde(serialize_with = "a_lire")]
    cachets: Option<Cachets>,
    #[serde(serialize_with = "a_lire")]
    heures: Option<Heures>,
    enseignement: Option<bool>,
    #[serde(serialize_with = "a_lire")]
    brut: Option<Montant>,
}

/// UN NOMBRE DANS LA VALEUR D'UNE CASE, tel qu'on le lit et qu'on le retape (`socle::texte::virgule` : « 2,5 »,
/// « 1250 ») ; la charge le relit comme tout nombre tape (`lu::tapee`). Vide, `null`.
fn a_lire<G: socle::grandeurs::Grandeur, S: serde::Serializer>(g: &Option<G>, s: S) -> Result<S::Ok, S::Error> {
    match g {
        Some(g) => s.serialize_str(&socle::texte::virgule(*g)),
        None => s.serialize_none(),
    }
}

/// Les cases d'une ligne prevue, chacune la forme de son type ; le debut, seulement dans la rangee qui l'ouvre.
const PARTIES_D_UNE_LIGNE_PREVUE: &[crate::catalogue::Partie] = &[
    crate::catalogue::Partie { cle: "debut", libelle: Affiche::new("Début"), forme: Forme::Date, toujours: false },
    crate::catalogue::Partie { cle: "fin", libelle: Affiche::new("Fin"), forme: Forme::Date, toujours: true },
    crate::catalogue::Partie { cle: "cachets", libelle: Affiche::new("Cachets"), forme: Forme::Nombre, toujours: true },
    crate::catalogue::Partie { cle: "heures", libelle: Affiche::new("Heures"), forme: Forme::Nombre, toujours: true },
    crate::catalogue::Partie { cle: "enseignement", libelle: Affiche::new("Enseignement"), forme: Forme::OuiNon, toujours: true },
    crate::catalogue::Partie { cle: "brut", libelle: Affiche::new("Brut"), forme: Forme::Montant, toujours: true },
];

/// Dans la charge, une ligne prevue est une valeur faite de parties : les lignes, des rangees.
impl crate::declaration::ValeurDeLaCharge<crate::declaration::ParSaForme> for LignePrevue {
    const FORME: Forme = Forme::Parties(PARTIES_D_UNE_LIGNE_PREVUE);
}

/// Le bouton qui retire une ligne prevue (une ligne vide attend toujours en bas du tableau), et ceux d'une date
/// d'embauche.
const RETIRER_LA_LIGNE: Affiche = Affiche::new("Retirer la ligne");
/// Ce que la rangee d'une ligne prevue ouvre quand son debut se demande ou se dit : la partie du debut.
const OUVRE_LE_DEBUT: &[&str] = &["debut"];
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

/// L'ECRAN DES DROITS, BLOC PAR BLOC, dans l'ordre ou il se lit ; chaque case porte ce que la charge y dit, ses nombres
/// tels qu'on les lit (`a_lire`).
/// - Avec un calcul (`calcul` : la personne, le tableau de ses paies, les paies dont la date d'embauche se choisit) : la
///   personne quand elle se choisit ; son bilan ; le tableau des paies, ou les lignes prevues se saisissent ; le resume ;
///   l'allocation visee quand une allocation s'estime ou que la charge en dit une ; la date anniversaire ; le depot ; les dates
///   d'embauche quand la route en demande ou que la charge en dit ; les pieces deposees.
/// - Sans calcul : la personne quand elle se choisit ; la date anniversaire ; le depot ; le tableau des paies, vide, ou
///   les lignes prevues se saisissent, quand aucune piece ne porte de personne (`sans_personne`) ; les pieces deposees.
/// La personne se choisit quand les pieces en portent plusieurs ou que la charge en dit une, meme qu'elles ne portent
/// plus (`personnes` : ses choix, celles des pieces, et ce qui se dit sous elle ; on la vide pour en sortir).
fn ecran(
    c: &ChargeDroitsDesPieces,
    personnes: Option<(Vec<crate::ouverture::Choix>, Vec<String>)>,
    calcul: Option<(BilanDit, DroitsDUnePersonne, TableauRendu, Vec<crate::ouverture::Choix>)>,
    sans_personne: bool,
    dossier: PathBuf,
    pieces: DitDesPieces,
) -> R<Vec<BlocDesDroits>> {
    use socle::ecriture::ValeurDeCase;
    let mut ecran = Vec::new();
    if let Some((offerts, dit)) = personnes {
        let case = case_simple(cle!(ChargeDroitsDesPieces, personne), c.personne.as_ref().and_then(ValeurDeCase::de))?;
        ecran.push(BlocDesDroits::Case(case.offrant(offerts).disant(dit)));
    }
    let anniversaire = BlocDesDroits::Case(case_simple(cle!(ChargeDroitsDesPieces, anniversaire), c.anniversaire.as_ref().and_then(ValeurDeCase::de))?);
    let depot = BlocDesDroits::Depot(DepotDit { dossier, libelle: DEPOSER, aide: DEPOSER_AIDE });
    // Les lignes prevues : leurs parties et ce que la charge en dit ; elles se saisissent dans le tableau des paies, qui
    // dit ou chacune se pose, et ses boutons (`tableau_des_paies`).
    let nom = cle!(ChargeDroitsDesPieces, previsionnel);
    let parties = PARTIES_D_UNE_LIGNE_PREVUE.iter().map(|p| crate::fiches::PartieDeLaFiche::de(p, None)).collect();
    let champ_des_lignes = champ(nom)?;
    let lignes = crate::fiches::CaseDeLaFiche::de_rangees(nom, champ_des_lignes.libelle, c.previsionnel.as_ref().and_then(ValeurDeCase::de), parties);
    match calcul {
        Some((tete, personne, tableau, embauches)) => {
            // Le bilan au-dessus du tableau des paies, le detail du compte (la fenetre jusqu'au seuil, l'allocation)
            // dessous (Léandre, 27/11/2026).
            let visee_offerte = !personne.estimations.is_empty();
            ecran.push(BlocDesDroits::Bilan(tete));
            ecran.push(BlocDesDroits::Paies(PaiesDites { tableau, lignes }));
            ecran.push(BlocDesDroits::Resume(Box::new(personne)));
            if visee_offerte || c.allocation_visee.is_some() {
                let visee = c.allocation_visee.map(|m| ValeurDeCase::texte(socle::texte::virgule(m)));
                ecran.push(BlocDesDroits::Case(case_simple(cle!(ChargeDroitsDesPieces, allocation_visee), visee)?));
            }
            ecran.push(anniversaire);
            ecran.push(depot);
            if !embauches.is_empty() || c.debuts.as_ref().is_some_and(|d| !d.is_empty()) {
                let parties = PARTIES_D_UN_DEBUT
                    .iter()
                    .map(|p| crate::fiches::PartieDeLaFiche::de(p, (p.forme == Forme::ChoixRendu).then(|| embauches.clone())))
                    .collect();
                ecran.push(BlocDesDroits::Case(case_en_rangees(cle!(ChargeDroitsDesPieces, debuts), c.debuts.as_ref().and_then(ValeurDeCase::de), parties, AJOUTER_UNE_EMBAUCHE, RETIRER_LA_DATE)?));
            }
        }
        None => {
            ecran.push(anniversaire);
            ecran.push(depot);
            if sans_personne {
                ecran.push(BlocDesDroits::Paies(PaiesDites { tableau: tableau_des_paies(&[], &[], &[]), lignes }));
            }
        }
    }
    let pieces = pieces.dites();
    if !pieces.is_empty() {
        ecran.push(BlocDesDroits::Pieces(PiecesDites { pieces, retirer: RETIRER_LA_PIECE }));
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
}

/// D'OU VIENT LE PLAFOND D'UNE PAIE (le plafond de la Securite sociale de sa periode, `droit::plafond`), pour la limite du
/// salaire de reference (annexes, art. 11 §3) : celui que le bulletin imprime ; celui que le moteur calcule depuis le
/// contrat (`contrat`, entier quand la piece le dit : il dit si l'engagement d'un artiste est court) et la periode de la
/// paie. Le contrat ne se demande que quand la periode ne suffit pas : un artiste dont la paie fait moins de cinq jours
/// (`plafond_de`). La periode d'un bulletin est celle qu'il paie (R. 3243-1, 5°) : elle peut etre le mois entier quand le
/// contrat est plus court ; le plafond s'y calcule alors au mois, plus haut que celui du contrat, et la limite de la paie
/// est trop haute (INFERENCE : la periode payee d'un intermittent est celle de son contrat) ; une note de l'estimation le
/// dit, pour chaque paie dont la periode est un mois entier (`estimer`). Une ligne prevue qui ne dit que sa fin
/// (`SansDebut`) n'a ni contrat ni periode : son plafond ne se connait pas tant que son debut ne se dit pas.
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
    /// apres la fin de la paie ne dit rien, et la note le dit. Une ligne prevue dit son debut dans sa ligne, jamais ici :
    /// sa source reste ce qu'elle est.
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
        attestees.push(Attestee {
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
    /// Son debut : celui de la periode que la piece atteste ; une ligne prevue peut ne dire que sa fin.
    debut: Option<Jour>,
    fin: Jour,
    cachets: Option<Cachets>,
    heures: Option<Heures>,
    enseignement: Option<Heures>,
    jours: Option<Jours>,
    brut: Option<Montant>,
    plafond: SourceDuPlafond,
    /// Une ligne prevue : son rang dans les lignes prevues de la charge (la rangee qui se saisit dans le tableau des
    /// paies) ; aucun : une paie qu'une piece atteste.
    prevue: Option<usize>,
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
            out.push(PaieComptee {
                id,
                plafond,
                employeur: Some(employeur.clone()),
                debut: Some(periode.debut()),
                fin: periode.fin(),
                cachets: a.cachets,
                heures: a.heures,
                enseignement: a.heures_enseignement,
                jours: a.jours,
                brut: a.brut,
                prevue: None,
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
        // Son plafond : celui de son contrat quand elle dit son debut ; sinon il ne se connait pas, et la route ouvre son
        // debut dans sa ligne quand il compte (`tableau_des_paies`).
        let plafond = match l.debut {
            Some(d) => match Periode::du_au(d, l.fin) {
                Ok(p) => SourceDuPlafond::DuContrat { contrat: Some(p), paie: p },
                Err(_) => {
                    notes.push(format!("La ligne prévue qui finit le {} commence après sa fin : elle ne compte rien.", l.fin.jj_mm_aaaa()));
                    continue;
                }
            },
            None => SourceDuPlafond::SansDebut,
        };
        // Un oui-non absent est un non : ses heures ne sont pas de l'enseignement.
        let enseignement = match l.enseignement {
            Some(true) => l.heures,
            Some(false) | None => None,
        };
        out.push(PaieComptee {
            id: format!("ligne prévue n° {}", rang + 1),
            employeur: None,
            debut: l.debut,
            fin: l.fin,
            cachets: l.cachets,
            heures: l.heures,
            enseignement,
            jours: None,
            brut: l.brut,
            plafond,
            prevue: Some(rang),
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
    Some(Declare { salarie: cle.clone(), debut: p.debut, fin: p.fin, heures: heures.entieres(), cachets, brut: p.brut, heures_enseignement: p.heures_d_enseignement() })
}

// ------------------------------------------------------------------ ce que la route rend

/// UNE LIGNE PAR PAIE, en tableau (`socle::ecran::TableauRendu`), dans l'ordre des fins (`droits_d_une_personne`) : sa
/// periode, ses cachets, ses heures dont l'enseignement, son brut ; un compteur que la piece ne dit pas reste vide.
/// L'employeur et la piece ne s'y montrent pas (Leandre, 27/11/2026 : « ça alourdit ») : la liste des pieces les dit.
/// Les lignes prevues s'y saisissent, chacune a sa place parmi les paies (son rang dans la case `previsionnel`) : la
/// periode tient leur fin, les cachets, les heures, l'enseignement et le brut les leurs.
/// Une ligne prevue qui ne se compte pas (ni cachets ni heures, ou un debut apres sa fin) prend aussi sa place, a sa fin :
/// `lignes`, celles de la charge.
/// La periode tient aussi le debut, dans la rangee qui l'ouvre seulement : celle d'une ligne dont l'estimation demande
/// le debut (`demandes`, leurs rangs), et celle qui le dit deja (on le corrige, ou on le vide).
fn tableau_des_paies(paies: &[PaieComptee], lignes: &[LignePrevue], demandes: &[usize]) -> TableauRendu {
    let ouvre = |rang: usize, r: RangeeRendue| match demandes.contains(&rang) || lignes.get(rang).is_some_and(|l| l.debut.is_some()) {
        true => r.ouvrant(OUVRE_LE_DEBUT),
        false => r,
    };
    let colonnes = vec![
        ColonneRendue::de_texte("Période").tenant(&["debut", "fin"]),
        ColonneRendue::de_nombres("Cachets").tenant(&["cachets"]),
        ColonneRendue::de_nombres("Heures").tenant(&["heures"]),
        ColonneRendue::de_nombres("Enseignement").tenant(&["enseignement"]),
        ColonneRendue::de_nombres("Brut").tenant(&["brut"]),
    ];
    let mut rangees: Vec<(Jour, RangeeRendue)> = paies
        .iter()
        .map(|p| {
            let cellules = vec![Some(p.dates()), p.cachets.map(dit), p.heures.map(dit), p.enseignement.map(dit), p.brut.map(euros)];
            let rangee = match p.prevue {
                Some(rang) => ouvre(rang, RangeeRendue::saisie(cellules, rang)),
                None => RangeeRendue::de(SorteDeRangee::Ligne, cellules),
            };
            (p.fin, rangee)
        })
        .collect();
    for (rang, l) in lignes.iter().enumerate().filter(|(rang, _)| !paies.iter().any(|p| p.prevue == Some(*rang))) {
        let cellules = vec![Some(format!("finissant le {}", l.fin.jj_mm_aaaa())), l.cachets.map(dit), l.heures.map(dit), None, l.brut.map(euros)];
        rangees.push((l.fin, ouvre(rang, RangeeRendue::saisie(cellules, rang))));
    }
    // Le tri est stable : a la meme fin, les paies gardent l'ordre de `paies`, puis viennent les lignes qui ne se comptent pas.
    rangees.sort_by_key(|(fin, _)| *fin);
    let rangees = rangees.into_iter().map(|(_, r)| r).collect();
    let saisie = SaisieDuTableau { case: cle!(ChargeDroitsDesPieces, previsionnel), retirer: RETIRER_LA_LIGNE.texte() };
    TableauRendu::de(colonnes, rangees).intitule("Une ligne par paie").saisissant(saisie)
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

/// LES PAIES DE LA FENETRE AUGMENTEES POUR L'ALLOCATION VISEE, en tableau : le brut de chacune, le brut vise, sa limite,
/// et le montant d'un cachet ou d'une heure qui y mene (quand la paie n'est faite que de cachets, ou que d'heures).
fn tableau_de_la_visee(lignes: Vec<Vec<Option<String>>>) -> TableauRendu {
    let colonnes = vec![
        ColonneRendue::de_texte("Période"),
        ColonneRendue::de_texte("Employeur"),
        ColonneRendue::de_nombres("Brut"),
        ColonneRendue::de_nombres("Brut visé"),
        ColonneRendue::de_nombres("Limite"),
        ColonneRendue::de_nombres("Par cachet"),
        ColonneRendue::de_nombres("Par heure"),
    ];
    TableauRendu::de_lignes(colonnes, lignes).intitule("Chaque paie, pour l'allocation visée")
}

/// L'allocation d'une annexe, estimee : ses parties, leur somme, le salaire de reference, ce qui est retenu, sa phrase,
/// et les paies du salaire.
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
    phrase: String,
    paies: TableauRendu,
}

/// Ce que l'allocation visee demande sous une annexe, et ce qui arrete la hausse.
#[derive(Debug, Serialize)]
struct ViseeDite {
    annexe: &'static str,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::option::serialize")]
    salaire_de_reference: Option<Montant>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::option::serialize")]
    allocation: Option<Montant>,
    phrase: String,
    arret: String,
    /// Aucune quand la visee ne se chiffre pas.
    #[serde(skip_serializing_if = "Option::is_none")]
    paies: Option<TableauRendu>,
}

/// LE BILAN DE LA PERSONNE CHOISIE, au-dessus du tableau des paies : son nom, quand les pieces portent plusieurs
/// personnes (celle qui est devant la page n'a pas a lire le sien, Leandre, 27/11/2026), et la reponse a la seule
/// question, les 507 heures sur les 12 mois avant la derniere fin de contrat : les heures, ou ce qui manque et jusqu'a
/// quand.
#[derive(Debug, Serialize)]
struct BilanDit {
    #[serde(skip_serializing_if = "Option::is_none")]
    nom: Option<String>,
    bilan: String,
}

/// Ce que la route dit de la personne choisie, sous le tableau des paies.
#[derive(Debug, Serialize)]
struct DroitsDUnePersonne {
    /// Son numero ; aucun : les lignes prevues seules, sans piece (`LIGNES_SEULES`).
    #[serde(skip_serializing_if = "Option::is_none")]
    nir: Option<Nir>,
    fenetre: String,
    lignes: Vec<(String, String)>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    heures_exigees: Heures,
    /// Ce qui manque, compte a partir d'aujourd'hui quand la derniere fin de contrat est passee (le bilan le dit :
    /// `BilanDit`).
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    manque: Heures,
    /// Quand l'estimation suppose non limitee une paie qui peut l'etre (son plafond ne se connait pas), la phrase qui le
    /// dit, une fois, et demande son debut quand il le dirait ; aucune : rien n'est suppose.
    #[serde(skip_serializing_if = "Option::is_none")]
    demande: Option<String>,
    estimations: Vec<EstimationDite>,
    visee: Vec<ViseeDite>,
    /// Ce qui n'est pas la reponse elle-meme, replie : le detail des estimations et les notes.
    details: DetailsDits,
}

/// LE DETAIL REPLIE DU RESUME : son libelle, et ses lignes (le detail de chaque estimation, le jour butoir, ce que
/// l'estimation ne compte pas, ce que les pieces ont d'etrange), une phrase chacune.
#[derive(Debug, Serialize)]
struct DetailsDits {
    libelle: &'static str,
    lignes: Vec<String>,
}

/// Le libelle du detail replie du resume.
const DETAILS: &str = "Détails";

/// Ce que rend `droits-des-pieces` : sa phrase, et l'ecran, bloc par bloc dans l'ordre ou il se lit (`BlocDesDroits`).
#[derive(Debug, Serialize)]
struct DroitsDesPieces {
    phrase: Phrase,
    ecran: Vec<BlocDesDroits>,
}

/// UN BLOC DE L'ECRAN DES DROITS, dans l'ordre que la route lui donne (`ecran`) : une case de la charge ; le resume de la
/// personne (son bilan : ses heures, ou ce qui lui manque et jusqu'a quand ; son allocation estimee et visee, les notes) ; le tableau
/// des paies, ou les lignes prevues se saisissent ; le depot des pieces ; les pieces deposees et ce qui en est lu.
#[derive(Debug, Serialize)]
#[serde(tag = "est", rename_all = "snake_case")]
enum BlocDesDroits {
    Case(crate::fiches::CaseDeLaFiche<&'static str>),
    Bilan(BilanDit),
    Resume(Box<DroitsDUnePersonne>),
    Paies(PaiesDites),
    Depot(DepotDit),
    Pieces(PiecesDites),
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
    let phrase = match (&calcul, personnes.len()) {
        (Some((t, _, _, _)), _) => match &t.nom {
            Some(nom) => socle::phrase!("{} : {}", nom, t.bilan),
            None => socle::phrase!("{}", t.bilan),
        },
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
        SourceDuPlafond::SansDebut => return Err(inconnu("la ligne ne dit pas son début", true)),
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

/// Ce que la route dit de la personne `nir` (aucune : les lignes prevues seules) : ses paies, son affiliation, le jour
/// butoir, son allocation estimee, et ce que vise la charge ; le tableau de ses paies ; et les paies dont la date
/// d'embauche se choisit (celles dont la route la demande, et celles que la charge en dit deja). `nom` : celui qui se
/// dit, quand les pieces portent plusieurs personnes.
fn droits_d_une_personne(inst: &Path, attestees: &[Attestee], nir: Option<&Nir>, nom: Option<String>, c: &ChargeDroitsDesPieces) -> R<(BilanDit, DroitsDUnePersonne, TableauRendu, Vec<crate::ouverture::Choix>)> {
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
    // LE BILAN : la reponse, en une phrase.
    let manque = dit(a_partir.manque);
    let bilan = match (atteintes, anniversaire, &butoir) {
        (true, _, _) => format!("Vous avez vos heures : {} h sur les 12 mois au {}.", dit(aff.position.heures_ouverture()?), fin.jj_mm_aaaa()),
        (false, Some(da), _) => format!("Il vous manque {} h d'ici votre date anniversaire, le {}.", manque, da.jj_mm_aaaa()),
        (false, None, Some(b)) => format!("Il vous manque {} h d'ici le {}.", manque, b.jour.jj_mm_aaaa()),
        (false, None, None) => format!("Il vous manque {} h sur les 12 mois.", manque),
    };
    // Pourquoi ce jour, replie dans les details : la paie qui sort des 12 mois, et ce qui manquera ensuite.
    if let Some(b) = &butoir {
        notes.push(pourquoi_le_butoir(b, &a_partir));
    }
    // Les paies que la fenetre compte : celles de la position retenue.
    let dans_la_fenetre: Vec<&PaieComptee> = declarees.iter().filter(|(d, _)| aff.position.engagements.contains(d)).map(|(_, i)| &comptees[*i]).collect();
    // Les debuts de contrat que la charge dit des paies des pieces, par le nom de leur paie ; un nom qu'aucune ne porte se
    // dit (une ligne prevue dit le sien dans sa ligne).
    let mut debuts: BTreeMap<String, Jour> = BTreeMap::new();
    for d in c.debuts.iter().flatten() {
        if comptees.iter().any(|p| p.id == d.paie && p.prevue.is_none()) {
            debuts.insert(d.paie.clone(), d.debut);
        } else {
            notes.push(format!("« {} » (Dates d'embauche) n'est le nom d'aucune paie des pièces : cette date ne dit rien.", d.paie));
        }
    }
    let mut supposees: BTreeMap<String, (Option<Montant>, bool)> = BTreeMap::new();
    let (estimations, visee) = if aff.manque.est_nul() { estimer(&t, &aff, &dans_la_fenetre, c.allocation_visee, fin, &debuts, &mut supposees, &mut notes)? } else { (Vec::new(), Vec::new()) };
    let demande = demande_dite(&supposees);
    // Le debut se demande dans la ligne d'une ligne prevue, par la case « Dates d'embauche » pour une paie des pieces ;
    // une date d'embauche deja dite reste un choix de sa case, meme quand elle a comble la demande.
    let demandees = |p: &&PaieComptee| supposees.get(&p.id).is_some_and(|(_, le_debut_le_dit)| *le_debut_le_dit);
    let lignes_demandees: Vec<usize> = comptees.iter().filter(demandees).filter_map(|p| p.prevue).collect();
    let embauches = comptees
        .iter()
        .filter(|p| p.prevue.is_none() && (demandees(p) || debuts.contains_key(&p.id)))
        .map(|p| crate::ouverture::Choix::de(&p.id, choix_d_embauche(p)))
        .collect();
    // Aucune ligne prevue dans la charge : aucune n'a de place a prendre.
    let lignes_prevues: &[LignePrevue] = match &c.previsionnel {
        Some(v) => v,
        None => &[],
    };
    let paies = tableau_des_paies(&comptees, lignes_prevues, &lignes_demandees);
    let personne = DroitsDUnePersonne {
        nir: nir.cloned(),
        fenetre: aff.position.libelle_fenetre.clone(),
        lignes: lignes_du_compte,
        heures_exigees: a_partir.heures_exigees,
        manque: a_partir.manque,
        demande,
        estimations,
        visee,
        details: DetailsDits { libelle: DETAILS, lignes: notes },
    };
    Ok((BilanDit { nom, bilan }, personne, paies, embauches))
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

/// L'ALLOCATION ESTIMEE DE CHAQUE ANNEXE sur la fenetre de `aff`, et ce que demande l'allocation `visee`. Le salaire de
/// reference : le brut des paies de la fenetre, chacune sous sa limite (quatre fois son plafond, annexes, art. 11 §3) ;
/// une paie toute d'enseignement en sort (guide de France Travail,
/// `chomage-intermittents/francetravail-guide-intermittents-20261020.txt` : « les salaires au titre des heures
/// d'enseignement ne sont pas retenus dans le SR ») ; une paie qui mele l'enseignement a d'autres heures ne dit pas leur
/// salaire a part : rien ne s'estime, et la note dit laquelle. Les heures travaillees : celles du montant (sans
/// l'enseignement : `Position::heures_montant`). Sur les 365 jours, les diviseurs de A et B sont ceux de l'annexe.
/// UNE PAIE DONT LE PLAFOND NE SE CONNAIT PAS compte entiere (Leandre, 27/11/2026 : « L'estimation sort toujours ») ;
/// quand son brut passe quatre fois le plus bas plafond qu'elle puisse avoir (`plafond_le_plus_bas`), elle peut etre
/// limitee : l'estimation le suppose, le dit dans sa phrase, et la paie entre dans `supposees` (son nom : cette plus
/// basse limite, et si le debut de son contrat dirait son plafond), d'ou la route tire sa seule demande (`demande_dite`).
#[allow(clippy::too_many_arguments)]
fn estimer(
    t: &Allocation,
    aff: &Affiliation,
    paies: &[&PaieComptee],
    visee: Option<Montant>,
    fin: Jour,
    debuts: &BTreeMap<String, Jour>,
    supposees: &mut BTreeMap<String, (Option<Montant>, bool)>,
    notes: &mut Vec<String>,
) -> R<(Vec<EstimationDite>, Vec<ViseeDite>)> {
    let p = &aff.position;
    if let Some(m) = paies.iter().find(|x| x.melee()) {
        notes.push(format!(
            "L'allocation ne s'estime pas : {} mêle {} h d'enseignement à d'autres heures ou des cachets, et le salaire de l'enseignement, qui sort du salaire de référence, n'y est pas dit à part.",
            m.en_mots(),
            dit(m.heures_d_enseignement())
        ));
        return Ok((Vec::new(), Vec::new()));
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
            return Ok((Vec::new(), Vec::new()));
        }
    };
    let plafond = t.plafond(pss_annuel);
    let nht = p.heures_montant();
    notes.push("Estimation : sans maternité, adoption ou ALD dans la fenêtre, sans la participation retraite de 0,93 % (art. 19), ni franchises.".to_string());
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
    let mut estimations = Vec::new();
    let mut visees = Vec::new();
    for annexe in [AnnexeChomage::X, AnnexeChomage::Viii] {
        let artiste = annexe == AnnexeChomage::X;
        let mut lignes = Vec::new();
        let mut bruts: Vec<(Montant, Option<Montant>)> = Vec::new();
        let mut a_viser: Vec<(&PaieComptee, Montant, Option<Montant>)> = Vec::new();
        let mut inconnues: Vec<PlafondSuppose> = Vec::new();
        let mut suppose = false;
        for (x, source) in du_salaire.iter().zip(&sources) {
            let Some(brut) = x.brut else { continue };
            // Un plafond qui ne se connait pas : la paie compte entiere. Sous quatre fois le plafond d'un contrat d'un
            // jour, elle ne passe pas sa limite, quel que soit le contrat ; au-dessus, l'estimation suppose qu'elle ne la
            // passe pas.
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
                    if au_plus_bas.is_none_or(|l| brut > l) {
                        supposer(supposees, x, au_plus_bas, inconnu.le_debut_le_dit);
                        suppose = true;
                    }
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
        // LE CHIFFRE, en une ligne ; son detail, dans le replie.
        let phrase = match suppose {
            false => format!("{} : environ {} par jour.", annexe_dite(annexe), euros(e.retenue)),
            true => format!("{} : environ {} par jour, si ces paies ne sont pas limitées.", annexe_dite(annexe), euros(e.retenue)),
        };
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
        estimations.push(EstimationDite { annexe: annexe_dite(annexe), a: e.a, b: e.b, c: e.c, calculee: e.calculee, salaire_de_reference: sr, retenue: e.retenue, phrase, paies: tableau_du_salaire(lignes) });
        if let Some(v) = visee {
            visees.push(viser(t, annexe, v, sr, nht, plafond, &bruts, &a_viser, &inconnues, supposees)?);
        }
    }
    Ok((estimations, visees))
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
/// proportion (le brut vise, sa limite, le montant d'un cachet ou d'une heure qui y mene), et ce qui arrete la hausse :
/// la limite des paies (art. 11 §3), ou le plafond de l'allocation (art. 16), celle des deux qui vient la premiere.
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
) -> R<ViseeDite> {
    let nom = annexe_dite(annexe);
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
    let vide = |phrase: String| ViseeDite { annexe: nom, salaire_de_reference: None, allocation: None, phrase, arret: arret.clone(), paies: None };
    let cible = match t.salaire_vise(annexe, v, nht, plafond)? {
        Visee::Salaire(s) => s,
        Visee::AuDelaDuPlafond => return Ok(vide(format!("{} : {} par jour dépasse le plafond ; aucun salaire n'y mène.", nom, euros(v)))),
        Visee::DesLePlancher => return Ok(vide(format!("{} : {} par jour ne passe pas le plancher de l'annexe, {} par jour, qui s'applique dès que le droit est ouvert.", nom, euros(v), euros(t.formule(annexe).plancher)))),
        Visee::SansSalaire => return Ok(vide(format!("{} : les heures seules donnent déjà {} par jour.", nom, euros(v)))),
    };
    let Some(k) = droit::allocation::facteur_vise(bruts, cible) else {
        return Ok(match au_plus {
            Some(m) if m < cible => vide(format!(
                "{} : pour {} par jour, il faudrait {} de salaire de référence ; la limite des paies (art. 11 §3) l'arrête à {}, soit {} par jour.",
                nom,
                euros(v),
                euros(cible),
                euros(m),
                euros(aj_de(m)?)
            )),
            _ => vide(format!("{} : il faudrait {} de salaire de référence ; les paies de la fenêtre n'ont aucun brut à augmenter.", nom, euros(cible))),
        });
    };
    // Une paie dont le plafond ne se connait pas, augmentee au-dela de la plus basse limite qu'elle puisse avoir : la
    // visee suppose qu'elle ne la passe pas, et le dit avec ce qui arrete la hausse.
    let mut suppose = false;
    for s in inconnues.iter().filter(|s| bruts.get(s.rang).is_some_and(|(b, _)| s.limite.is_none_or(|l| *b * k > l))) {
        supposer(supposees, s.paie, s.limite, s.le_debut_le_dit);
        suppose = true;
    }
    let arret = match suppose {
        true => format!("{} Le calcul suppose que ces paies ne sont pas limitées.", arret),
        false => arret,
    };
    let atteinte = aj_de(cible)?;
    let mut paies = Vec::new();
    for (x, brut, limite) in a_viser {
        let brut_vise = (*brut * k).au_centime();
        let (cachets, heures) = (x.cachets.into_iter().sum::<Cachets>(), x.heures.into_iter().sum::<Heures>());
        let par_cachet = (heures.est_nul() && !cachets.est_nul()).then(|| brut_vise.prorata(Cachets::UN, cachets).au_centime());
        let par_heure = if cachets.est_nul() { brut_vise.par_heure(heures) } else { None };
        paies.push(vec![Some(x.dates()), x.employeur.clone(), Some(euros(*brut)), Some(euros(brut_vise)), limite.map(euros), par_cachet.map(euros), par_heure.map(euros)]);
    }
    let (sens, ecart) = if k >= Coefficient::UN { ("augmentée", k - Coefficient::UN) } else { ("baissée", Coefficient::UN - k) };
    let phrase = format!(
        "{} : pour {} par jour, il faut {} de salaire de référence au lieu de {}, chaque paie {} de {} % ; l'allocation serait de {}.",
        nom,
        euros(v),
        euros(cible),
        euros(sr),
        sens,
        dit(ecart.en_taux().au_pair(1)),
        euros(atteinte)
    );
    Ok(ViseeDite { annexe: nom, salaire_de_reference: Some(cible), allocation: Some(atteinte), phrase, arret, paies: Some(tableau_de_la_visee(paies)) })
}
