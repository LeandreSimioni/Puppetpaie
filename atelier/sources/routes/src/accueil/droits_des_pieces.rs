//! LES DROITS D'UN INTERMITTENT DEPUIS SES PIECES (`droits-des-pieces`) : une route de l'installation, sans compagnie.
//! La personne pose dans un seul dossier, `mes-pieces/` de l'installation (cree vide s'il manque, comme la boite de
//! l'import), les pieces de tous ses employeurs : bulletins en PDF, AEM en PDF ou deposees. La route les lit par les
//! lecteurs de l'import (`import::tri`, `bulletin_lu`, `aem_lu`, `dsn_aem`), PAR PERSONNE (le NIR), jamais en melant deux
//! personnes : plusieurs NIR, et la route fait choisir la personne (une case a choix), puis ne montre qu'elle.
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
//! entiere si son brut reste sous quatre fois le plus bas plafond possible (un contrat d'un jour) ; sinon l'estimation
//! attend, en nommant la paie, et la route demande la date de debut de son contrat (`debuts`) quand elle le dirait.
//!
//! CE QUE LA ROUTE REND, pour la personne : une ligne par paie retenue (la periode, l'employeur, les cachets, les heures
//! dont l'enseignement, le brut, la piece) ; l'affiliation a la derniere fin de contrat (`droit::allocation::affiliation` :
//! la fenetre de 365 jours, allongee a la readmission, compte par le meme code que la route `droits` d'une compagnie,
//! `droit::cumul::Position`, `archives::comptes_de_la_position`), ce qui manque, et le jour butoir ou, sans contrat de
//! plus, il en faudra davantage, et pourquoi ; l'allocation journaliere estimee de l'annexe VIII et de l'annexe X
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

use droit::allocation::{Affiliation, Allocation, Borne, Diviseurs, Visee};
use droit::cumul::{Constantes, Declare};
use droit::emplois::AnnexeChomage;
use droit::fiche::{Nir, Siren};
use engagement::importe::{Attestation, ContratLu};
use import::attestes::{BlocDImport, Registre};
use import::reponse::Source;
use import::tri::Sorte;
use socle::date::{Duree, Jour, Periode};
use socle::echec::{Classe, R};
use socle::fichiers::NomDeFichier;
use socle::grandeurs::ecrit::sans_zero::dit;
use socle::grandeurs::{Cachets, Coefficient, Heures, Jours, Montant, MontantUnitaire};
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
    exemple: r#"{"allocation_visee":80,"previsionnel":[{"fin":"2026-12-12","debut":"2026-12-12","cachets":1,"brut":250}]}"#,
    rend: "dossier (mes-pieces/, où poser les pièces), pieces (une phrase par fichier), cases (les cases de la charge telles qu'on les saisit : la personne quand les pièces en portent plusieurs, la date anniversaire, les lignes prévues, l'allocation visée quand une allocation s'estime, les dates d'embauche quand la route en demande ; chacune libelle, forme, valeur, choix, parties), personne (celle choisie, ou les lignes prévues seules sans pièce : nir, fenetre, paies [periode, employeur, piece (bulletin, AEM, prévisionnel), cachets, heures, enseignement, brut], lignes [quoi, valeur], heures_exigees, manque, butoir [jour, phrase, pourquoi, reference], phrase, notes, estimations par annexe [a, b, c, calculee, salaire_de_reference, retenue, phrase, paies avec plafond et limite], visee par annexe [salaire_de_reference, allocation, phrase, arret, paies avec brut_vise, limite, par_cachet, par_heure]), phrase",
    refus: &[Refus { classe: Classe::Refuse, corriger: "les pièces que la phrase nomme ; rien n'est écrit" }, OPERATION],
    pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
    acte: Acte::Lit,
    phrase: 200,
    ecran: Ecran::Lecture,
}];

charge! {
    /// La charge de `droits-des-pieces` : la personne (aucune : la seule des pieces, ou aucune s'il y en a plusieurs), la
    /// date anniversaire du droit precedent (aucune : une premiere ouverture), l'allocation visee (aucune : rien ne se
    /// vise), les lignes prevues (aucune : les pieces seules).
    #[derive(Debug)]
    struct ChargeDroitsDesPieces {
        personne: Option<Nir> = legende("Personne", "son numéro de sécurité sociale, parmi ceux que portent les pièces ; absent : la seule qu'elles portent").rendue(),
        anniversaire: Option<Jour> = legende("Date anniversaire", "celle du droit en cours ou du dernier droit, que France Travail a notifiée : la réadmission, la fin de contrat qui a ouvert ce droit étant douze mois avant ; absente : une première ouverture"),
        allocation_visee: Option<Montant> = legende("Allocation visée", "l'allocation journalière qu'on voudrait, en euros ; absente : rien ne se vise"),
        previsionnel: Option<Vec<LignePrevue>> = legende("Lignes prévues", "le travail à venir, une ligne par engagement : ses dates, ses cachets ou ses heures, l'enseignement, son brut ; rien ne s'écrit"),
        debuts: Option<Vec<DebutDUnContrat>> = legende("Dates d'embauche", "la date d'embauche d'une paie dont la route la demande (la case Dates d'embauche, ses choix), sous le nom de la paie ; absente : rien n'est dit"),
    }
}

/// LE DEBUT DU CONTRAT D'UNE PAIE, que la route demande quand le plafond de la paie ne se connait pas et que son brut peut
/// passer sa limite (art. 11 §3) : la paie, par le nom que la route rend (les choix de sa case), et la date.
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

/// UNE LIGNE PREVUE : un contrat a venir, saisi a la main pour un previsionnel. Ses dates, sa fin et son debut (un
/// contrat a les deux : la ligne declaree a sa periode, `droit::cumul::Declare`, et la paie son plafond) ; des cachets
/// ou des heures ; si ses heures sont de l'enseignement ; son brut, facultatif (sans lui, la ligne compte pour les
/// heures, pas pour l'allocation).
/// Elle s'ecrit aussi, telle qu'elle est saisie, dans la valeur de sa case (`cases`).
#[derive(Debug, Clone, serde::Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct LignePrevue {
    fin: Jour,
    debut: Jour,
    #[serde(serialize_with = "socle::grandeurs::ecrit::nombre_json::option::serialize")]
    cachets: Option<Cachets>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::nombre_json::option::serialize")]
    heures: Option<Heures>,
    enseignement: Option<bool>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::nombre_json::option::serialize")]
    brut: Option<Montant>,
}

/// Les cases d'une ligne prevue, chacune la forme de son type.
const PARTIES_D_UNE_LIGNE_PREVUE: &[crate::catalogue::Partie] = &[
    crate::catalogue::Partie { cle: "fin", libelle: Affiche::new("Fin"), forme: Forme::Date, toujours: true },
    crate::catalogue::Partie { cle: "debut", libelle: Affiche::new("Début"), forme: Forme::Date, toujours: true },
    crate::catalogue::Partie { cle: "cachets", libelle: Affiche::new("Cachets"), forme: Forme::Nombre, toujours: true },
    crate::catalogue::Partie { cle: "heures", libelle: Affiche::new("Heures"), forme: Forme::Nombre, toujours: true },
    crate::catalogue::Partie { cle: "enseignement", libelle: Affiche::new("Enseignement"), forme: Forme::OuiNon, toujours: true },
    crate::catalogue::Partie { cle: "brut", libelle: Affiche::new("Brut"), forme: Forme::Montant, toujours: true },
];

/// Dans la charge, une ligne prevue est une valeur faite de parties : les lignes, des rangees.
impl crate::declaration::ValeurDeLaCharge<crate::declaration::ParSaForme> for LignePrevue {
    const FORME: Forme = Forme::Parties(PARTIES_D_UNE_LIGNE_PREVUE);
}

/// Le bouton qui ajoute une ligne prevue, et celui qui ajoute une date d'embauche.
const AJOUTER_UNE_LIGNE: Affiche = Affiche::new("Ajouter une ligne");
const AJOUTER_UNE_EMBAUCHE: Affiche = Affiche::new("Ajouter une date d'embauche");

/// LES LIGNES PREVUES SANS PIECE : quand aucune piece ne porte de personne, les lignes de la charge se comptent seules, sous
/// cette cle (celle de `droit::cumul::Declare`, que la position range ; elle ne se montre pas) et ce nom.
const LIGNES_SEULES: &str = "lignes prévues";

/// LE CHAMP `nom` DE LA CHARGE, tel que sa declaration le dit (son libelle, sa forme, son sens) : une case que la route rend
/// pour lui en porte la forme et les mots, jamais une copie.
fn champ(nom: &'static str) -> R<crate::catalogue::Champ> {
    match ChargeDroitsDesPieces::CHARGE.champs.iter().find(|c| c.champ.nom == nom) {
        Some(c) => Ok(c.champ),
        None => Err(socle::echec::erreur(Classe::DefautDuLogiciel, format!("Défaut du logiciel : la charge de droits-des-pieces n'a pas de champ {}.", nom))),
    }
}

/// LA CASE D'UN CHAMP D'UNE SEULE VALEUR de la charge, avec ce que la charge y porte : son libelle, sa forme et son sens
/// (la bulle) sont ceux de la declaration.
fn case_simple(nom: &'static str, valeur: Option<socle::ecriture::ValeurDeCase>) -> R<crate::fiches::CaseDeLaFiche<&'static str>> {
    let c = champ(nom)?;
    Ok(crate::fiches::CaseDeLaFiche::de_la_boite(nom, c.libelle, c.forme, c.sens, valeur))
}

/// LA CASE DES RANGEES D'UN CHAMP de la charge (les lignes prevues, les dates d'embauche) : ses parties, ce que la charge y
/// porte, et le bouton qui ajoute une rangee.
fn case_en_rangees(nom: &'static str, valeur: Option<socle::ecriture::ValeurDeCase>, parties: Vec<crate::fiches::PartieDeLaFiche>, ajouter: Affiche) -> R<crate::fiches::CaseDeLaFiche<&'static str>> {
    let c = champ(nom)?;
    Ok(crate::fiches::CaseDeLaFiche::de_rangees(nom, c.libelle, valeur, parties).ajoutant(ajouter).disant(vec![c.sens.to_string()]))
}

/// LES CASES QUE LA ROUTE REND, chacune un champ de sa charge, avec ce que la charge y porte : la personne quand les
/// pieces en portent plusieurs (`personnes`, ses choix) ; la date anniversaire ; les lignes prevues ; l'allocation visee
/// quand une allocation s'estime (`visee_offerte`) ou que la charge en dit une ; les dates d'embauche quand la route en
/// demande ou que la charge en dit (`embauches`, les paies qui se choisissent).
fn les_cases(c: &ChargeDroitsDesPieces, personnes: Option<Vec<crate::ouverture::Choix>>, visee_offerte: bool, embauches: Vec<crate::ouverture::Choix>) -> R<Vec<crate::fiches::CaseDeLaFiche<&'static str>>> {
    use socle::ecriture::ValeurDeCase;
    let mut cases = Vec::new();
    if let Some(offerts) = personnes {
        cases.push(case_simple(cle!(ChargeDroitsDesPieces, personne), c.personne.as_ref().and_then(ValeurDeCase::de))?.offrant(offerts));
    }
    cases.push(case_simple(cle!(ChargeDroitsDesPieces, anniversaire), c.anniversaire.as_ref().and_then(ValeurDeCase::de))?);
    let parties = PARTIES_D_UNE_LIGNE_PREVUE.iter().map(|p| crate::fiches::PartieDeLaFiche::de(p, None)).collect();
    cases.push(case_en_rangees(cle!(ChargeDroitsDesPieces, previsionnel), c.previsionnel.as_ref().and_then(ValeurDeCase::de), parties, AJOUTER_UNE_LIGNE)?);
    if visee_offerte || c.allocation_visee.is_some() {
        cases.push(case_simple(cle!(ChargeDroitsDesPieces, allocation_visee), ValeurDeCase::de(&socle::ecriture::CaseEcrite(&c.allocation_visee)))?);
    }
    if !embauches.is_empty() || c.debuts.as_ref().is_some_and(|d| !d.is_empty()) {
        let parties = PARTIES_D_UN_DEBUT
            .iter()
            .map(|p| crate::fiches::PartieDeLaFiche::de(p, (p.forme == Forme::ChoixRendu).then(|| embauches.clone())))
            .collect();
        cases.push(case_en_rangees(cle!(ChargeDroitsDesPieces, debuts), c.debuts.as_ref().and_then(ValeurDeCase::de), parties, AJOUTER_UNE_EMBAUCHE)?);
    }
    Ok(cases)
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
/// dit, pour chaque paie dont la periode est un mois entier (`estimer`).
#[derive(Clone, Debug)]
enum SourceDuPlafond {
    Imprime(Montant),
    DuContrat { contrat: Option<Periode>, paie: Periode },
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
    /// apres la fin de la paie ne dit rien, et la note le dit.
    fn avec_le_debut(&self, debut: Jour) -> Option<SourceDuPlafond> {
        let paie = match self {
            SourceDuPlafond::DuContrat { contrat: None, paie } => *paie,
            SourceDuPlafond::Imprime(_) | SourceDuPlafond::DuContrat { contrat: Some(_), .. } => return Some(self.clone()),
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
    dites: std::collections::BTreeSet<String>,
}

impl Seules {
    /// La cle d'un contrat de la piece `piece` : celle que dit `lue`, sinon une a lui seul ; la piece se dit une fois dans
    /// `phrases`, avec `pourquoi` et l'effet.
    fn cle(&mut self, lue: Option<CleDEmployeur>, piece: &str, pourquoi: &str, phrases: &mut Vec<String>) -> CleDEmployeur {
        if let Some(c) = lue {
            return c;
        }
        self.rang += 1;
        if self.dites.insert(piece.to_string()) {
            phrases.push(format!(
                "{} : {} ; son contrat compte à part, sans se rapprocher d'aucune autre pièce : ses heures peuvent compter deux fois si une autre pièce dit le même contrat, et le seuil paraître atteint à tort.",
                piece, pourquoi
            ));
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
    phrases: Vec<String>,
}

/// LE DOSSIER LU, fichier par fichier, par les lecteurs de l'import ; `travail` : le dossier de travail (les archives
/// ouvertes, les AEM deposees copiees pour leur lecteur), efface par l'appelant.
fn lire_le_dossier(inst: &Path, dossier: &Path, travail: &Path) -> R<DossierDesPieces> {
    let tri = import::tri::trier(dossier, &travail.join("tri"))?;
    let table = import::bulletin_lu::Table::livree()?;
    let mut phrases: Vec<String> = tri.mises_de_cote.iter().map(import::tri::MiseDeCote::dit).collect();
    let mut attestees = Vec::new();
    let mut bulletins: Vec<(String, usize, NomDeFichier, import::bulletin_lu::BulletinLu, Nir)> = Vec::new();
    let aem_dir = travail.join("aem");
    // Les AEM deposees, par leur copie : le nom du fichier pose, et l'employeur s'il est seul.
    let mut aem_employeurs: BTreeMap<String, (String, Option<CleDEmployeur>, usize)> = BTreeMap::new();
    let mut seules = Seules::default();
    for f in &tri.fichiers {
        let nom = f.chemin.texte().to_string();
        match f.sorte {
            Sorte::Aem => {
                let Some(copie) = nom_copie(f)? else { continue };
                std::fs::create_dir_all(&aem_dir).map_err(|e| socle::echec::du_disque(&aem_dir, &e).reclasser(Classe::Disque))?;
                let cible = aem_dir.join(copie.texte());
                std::fs::copy(&f.disque, &cible).map_err(|e| socle::echec::du_disque(&cible, &e).reclasser(Classe::Disque))?;
                let e = import::dsn_aem::employeur_aem(&f.disque)?;
                let siren = (e.sirens.len() == 1).then(|| e.sirens.iter().next().cloned()).flatten();
                aem_employeurs.insert(copie.texte().to_string(), (nom.clone(), cle_d_employeur(siren, None), e.sirens.len()));
                phrases.push(format!("{} : AEM déposée, lue.", nom));
            }
            Sorte::PdfTexte => {
                let Some(copie) = nom_copie(f)? else { continue };
                let aem = import::aem_lu::lire(&f.disque)?;
                if !aem.is_empty() {
                    let mut lues = 0usize;
                    for p in aem {
                        let import::aem_lu::Lecture::Lue(a) = p.lecture else {
                            if let import::aem_lu::Lecture::Refusee { raison, .. } = p.lecture {
                                phrases.push(format!("{}, page {} : {}.", nom, p.page, raison));
                            }
                            continue;
                        };
                        let (Some(nir), Some(periode)) = (a.identite.nir.clone(), a.periode) else {
                            let manque = match (a.identite.nir.is_some(), a.periode.is_some()) {
                                (false, false) => "ni numéro de sécurité sociale ni période",
                                (false, true) => "pas de numéro de sécurité sociale",
                                (true, _) => "pas de période",
                            };
                            phrases.push(format!("{}, page {} : l'AEM n'imprime {} : elle ne se compte pas.", nom, p.page, manque));
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
                                    phrases.push(format!("{}, page {} : l'AEM imprime une embauche le {}, après la fin du contrat le {} : ses dates sont incohérentes.", nom, p.page, d.jj_mm_aaaa(), f.jj_mm_aaaa()));
                                    None
                                }
                            },
                            _ => None,
                        };
                        let lue = cle_d_employeur(a.employeur.siret.as_ref().map(droit::fiche::Siret::siren), a.employeur.raison_sociale.as_ref());
                        let piece = format!("{}, page {}", nom, p.page);
                        let employeur = seules.cle(lue, &piece, SANS_EMPLOYEUR, &mut phrases);
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
                    phrases.push(format!("{} : AEM en PDF, {} page(s) lue(s).", nom, lues));
                    continue;
                }
                let mut lues = 0usize;
                for l in import::bulletin_lu::lire_avec(&f.disque, &table)? {
                    let b = match l.resultat {
                        Ok(b) => b,
                        Err(import::bulletin_lu::Refus::Annexe) => continue,
                        Err(r) => {
                            phrases.push(format!("{}, page {} : ne se lit pas comme un bulletin ({}).", nom, l.page, r.message()));
                            continue;
                        }
                    };
                    match nir_lu(&b) {
                        Ok(Some((nir, _))) => {
                            bulletins.push((nom.clone(), l.page, copie.clone(), b, nir));
                            lues += 1;
                        }
                        Ok(None) => phrases.push(format!("{}, page {} : le bulletin n'imprime pas de numéro de sécurité sociale, il ne se range à personne.", nom, l.page)),
                        Err(e) => phrases.push(format!("{}, page {} : {}", nom, l.page, e.phrase)),
                    }
                }
                if lues > 0 {
                    phrases.push(format!("{} : {} bulletin(s) lu(s).", nom, lues));
                }
            }
            Sorte::Dsn => phrases.push(format!("{} : une DSN ne se compte pas ici ; déposez les bulletins de ses paies.", nom)),
            Sorte::Archive | Sorte::Doublon => {}
            Sorte::PdfSansTexte => phrases.push(format!("{} : {}.", nom, import::tri::IMAGE)),
            Sorte::Contrat => phrases.push(format!("{} : {}.", nom, import::tri::CONTRAT)),
            Sorte::Illisible | Sorte::Autre => phrases.push(format!("{} : ce fichier ne se lit pas. {}", nom, import::tri::ACCEPTE)),
        }
    }
    // Les AEM deposees, par le lecteur de l'import (`dsn_aem::lire_aem`), dans leur copie.
    if aem_dir.is_dir() {
        let (contrats, illisibles, fautes) = import::dsn_aem::lire_aem(&aem_dir)?;
        phrases.extend(illisibles.into_iter().chain(fautes));
        for c in contrats {
            let (piece, lue, sirens) = match aem_employeurs.get(c.fichier.texte()) {
                Some((nom, e, n)) => (nom.clone(), e.clone(), *n),
                None => (c.fichier.texte().to_string(), None, 0),
            };
            let pourquoi = match sirens {
                0 | 1 => SANS_EMPLOYEUR.to_string(),
                n => format!("l'AEM porte {} employeurs (Siren), et la route ne lit pas lequel est celui de chaque contrat", n),
            };
            let employeur = seules.cle(lue, &piece, &pourquoi, &mut phrases);
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
                phrases.push(format!("{}, page {} : le contrat ne se compte pas ({}).", nom, page, e.phrase));
                continue;
            }
        };
        let raison = b.entreprise.employeur.raison_sociale.clone();
        let lue = cle_d_employeur(b.entreprise.employeur.siret.as_ref().map(droit::fiche::Siret::siren), raison.as_ref());
        let piece = format!("{}, page {}", nom, page);
        let employeur = seules.cle(lue, &piece, SANS_EMPLOYEUR, &mut phrases);
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
    Ok(DossierDesPieces { attestees, phrases })
}

// ------------------------------------------------------------------ les paies comptees

/// UNE PAIE COMPTEE : un contrat qu'une piece atteste, chez son employeur, une fois ; ou une ligne prevue. Ce qu'elle
/// compte (un compteur que la piece ne dit pas reste vide), et d'ou vient son plafond.
struct PaieComptee {
    /// Son nom, tel que la route le rend et que la charge le reprend (`debuts`) : sa piece (le fichier pose, sa page), ou
    /// le contrat prevu.
    id: String,
    employeur: String,
    periode: Periode,
    piece: &'static str,
    cachets: Option<Cachets>,
    heures: Option<Heures>,
    enseignement: Option<Heures>,
    jours: Option<Jours>,
    brut: Option<Montant>,
    plafond: SourceDuPlafond,
    prevue: bool,
}

impl PaieComptee {
    /// Ses dates, en mots.
    fn dates(&self) -> String {
        match self.periode.debut() == self.periode.fin() {
            false => format!("du {} au {}", self.periode.debut().jj_mm_aaaa(), self.periode.fin().jj_mm_aaaa()),
            true => format!("le {}", self.periode.fin().jj_mm_aaaa()),
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
                    let id = format!("contrat du {} au {} chez {}", periode.debut().jj_mm_aaaa(), periode.fin().jj_mm_aaaa(), employeur);
                    notes.push(format!("La pièce du {} ne se retrouve pas : son plafond se calcule sur ses dates. Le signaler au développeur.", id));
                    (id, SourceDuPlafond::DuContrat { contrat: None, paie: periode })
                }
            };
            out.push(PaieComptee {
                id,
                plafond,
                employeur: employeur.clone(),
                periode,
                piece: piece_dite(a.atteste_par),
                cachets: a.cachets,
                heures: a.heures,
                enseignement: a.heures_enseignement,
                jours: a.jours,
                brut: a.brut,
                prevue: false,
            });
        }
    }
    Ok(out)
}

/// Les lignes prevues de la charge, en paies comptees ; une ligne qui ne se compte pas le dit dans `notes`.
fn prevues(lignes: &[LignePrevue], notes: &mut Vec<String>) -> R<Vec<PaieComptee>> {
    let mut out = Vec::new();
    for l in lignes {
        if l.cachets.is_none() && l.heures.is_none() {
            notes.push(format!("La ligne prévue qui finit le {} ne dit ni cachets ni heures : elle ne compte rien.", l.fin.jj_mm_aaaa()));
            continue;
        }
        let Ok(periode) = Periode::du_au(l.debut, l.fin) else {
            notes.push(format!("La ligne prévue qui finit le {} commence après sa fin : elle ne compte rien.", l.fin.jj_mm_aaaa()));
            continue;
        };
        // Un oui-non absent est un non : ses heures ne sont pas de l'enseignement.
        let enseignement = match l.enseignement {
            Some(true) => l.heures,
            Some(false) | None => None,
        };
        out.push(PaieComptee {
            id: format!("ligne prévue qui finit le {}", l.fin.jj_mm_aaaa()),
            employeur: "prévisionnel".to_string(),
            periode,
            piece: "prévisionnel",
            cachets: l.cachets,
            heures: l.heures,
            enseignement,
            jours: None,
            brut: l.brut,
            plafond: SourceDuPlafond::DuContrat { contrat: Some(periode), paie: periode },
            prevue: true,
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
        (h, c, true) => (h.into_iter().sum(), c.into_iter().sum()),
        (_, _, false) => return None,
    };
    if !p.prevue && p.brut.is_none() {
        return None;
    }
    Some(Declare { salarie: cle.clone(), periode: p.periode, heures: heures.entieres(), cachets, brut: p.brut, heures_enseignement: p.heures_d_enseignement() })
}

// ------------------------------------------------------------------ ce que la route rend

/// Une paie, telle que la route la dit : ce qu'elle compte (un compteur que la piece ne dit pas reste vide).
#[derive(Debug, Serialize)]
struct PaieDite {
    periode: String,
    employeur: String,
    piece: &'static str,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::option::serialize")]
    cachets: Option<Cachets>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::option::serialize")]
    heures: Option<Heures>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::option::serialize")]
    enseignement: Option<Heures>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::option::serialize")]
    brut: Option<Montant>,
}

/// Une paie dans le salaire de reference d'une annexe : son brut, son plafond et sa limite (quatre plafonds, art. 11 §3),
/// ce qu'elle y compte, et pourquoi son plafond ne se connait pas.
#[derive(Debug, Serialize)]
struct PaieDuSalaire {
    periode: String,
    employeur: String,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    brut: Montant,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::option::serialize")]
    plafond: Option<Montant>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::option::serialize")]
    limite: Option<Montant>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    compte: Montant,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
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
    paies: Vec<PaieDuSalaire>,
}

/// Une paie de la fenetre, augmentee pour l'allocation visee : son brut, le brut vise, sa limite, et le montant d'un
/// cachet ou d'une heure qui y mene (quand la paie n'est faite que de cachets, ou que d'heures).
#[derive(Debug, Serialize)]
struct PaieVisee {
    periode: String,
    employeur: String,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    brut: Montant,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    brut_vise: Montant,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::option::serialize")]
    limite: Option<Montant>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::option::serialize")]
    par_cachet: Option<Montant>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::option::serialize")]
    par_heure: Option<MontantUnitaire>,
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
    paies: Vec<PaieVisee>,
}

/// Le jour butoir, tel que la route le dit : le jour, sa phrase, pourquoi il se deplace, et le texte.
#[derive(Debug, Serialize)]
struct ButoirDit {
    jour: Jour,
    phrase: String,
    pourquoi: String,
    reference: String,
}

/// Ce que la route dit de la personne choisie.
#[derive(Debug, Serialize)]
struct DroitsDUnePersonne {
    /// Son numero ; aucun : les lignes prevues seules, sans piece (`LIGNES_SEULES`).
    #[serde(skip_serializing_if = "Option::is_none")]
    nir: Option<Nir>,
    nom: String,
    fenetre: String,
    paies: Vec<PaieDite>,
    lignes: Vec<(String, String)>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    heures_exigees: Heures,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    manque: Heures,
    #[serde(skip_serializing_if = "Option::is_none")]
    butoir: Option<ButoirDit>,
    phrase: String,
    notes: Vec<String>,
    estimations: Vec<EstimationDite>,
    visee: Vec<ViseeDite>,
}

/// Ce que rend `droits-des-pieces`.
#[derive(Debug, Serialize)]
struct DroitsDesPieces {
    phrase: Phrase,
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    dossier: PathBuf,
    pieces: Vec<String>,
    phrases: Vec<String>,
    /// Les cases de la charge, telles qu'on les saisit (`les_cases`).
    cases: Vec<crate::fiches::CaseDeLaFiche<&'static str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    personne: Option<DroitsDUnePersonne>,
}

impl Dite for DroitsDesPieces {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

/// Le nom d'une annexe, tel que la route le dit.
fn annexe_dite(annexe: AnnexeChomage) -> &'static str {
    match annexe {
        AnnexeChomage::Viii => "annexe VIII (technicien)",
        AnnexeChomage::X => "annexe X (artiste)",
    }
}

/// Le nom d'une piece.
fn piece_dite(a: Attestation) -> &'static str {
    match a {
        Attestation::Bulletin => "bulletin",
        Attestation::Aem => "AEM",
        Attestation::Certificat => "certificat",
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
    let DossierDesPieces { attestees, phrases: pieces } = lu?;
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
    let choisie: Option<Nir> = match (&c.personne, personnes.len()) {
        (Some(n), _) => Some(n.clone()),
        (None, 1) => personnes.keys().next().cloned(),
        (None, _) => None,
    };
    let offerts = (personnes.len() > 1).then(|| personnes.keys().map(|k| crate::ouverture::Choix::de(&**k, nom_de(k))).collect());
    // Sans piece qui porte une personne, les lignes prevues se comptent seules.
    let lignes_seules = personnes.is_empty() && c.previsionnel.as_ref().is_some_and(|v| !v.is_empty());
    let calcul = match &choisie {
        Some(nir) if personnes.contains_key(nir) => Some(droits_d_une_personne(inst, &attestees, Some(nir), nom_de(nir), &c)?),
        Some(_) => None,
        None if lignes_seules => Some(droits_d_une_personne(inst, &attestees, None, LIGNES_SEULES.to_string(), &c)?),
        None => None,
    };
    let (personne, embauches) = match calcul {
        Some((p, e)) => (Some(p), e),
        None => (None, Vec::new()),
    };
    let phrase = match (&personne, &choisie, personnes.len()) {
        (Some(p), _, _) => socle::phrase!("{} ({}) : {}", p.nom, p.fenetre, p.phrase),
        (None, Some(_), _) => Phrase::fixe("Les pièces ne portent pas ce numéro de sécurité sociale : choisissez une personne parmi celles qu'elles portent."),
        (None, None, 0) => Phrase::fixe("Aucune pièce ne se compte encore : posez vos bulletins, de tous vos employeurs, ou remplissez des lignes prévues."),
        (None, None, n) => socle::phrase!("Les pièces portent {} personnes : choisissez-en une, ses droits se comptent seuls.", n),
    };
    let phrases = personne.iter().flat_map(|p| p.notes.iter().cloned()).collect();
    let visee_offerte = personne.as_ref().is_some_and(|p| !p.estimations.is_empty());
    let cases = les_cases(&c, offerts, visee_offerte, embauches)?;
    Ok(DroitsDesPieces { phrase, dossier, pieces, phrases, cases, personne })
}

/// Le plafond de la Securite sociale de l'annee `an`, tel que le bareme livre le porte.
fn pss(an: Annee) -> R<droit::plafond::Pss> {
    match droit::contexte::socle_livre_lu(an)?.and_then(|p| p.plafond_securite_sociale) {
        Some(x) => Ok(droit::plafond::Pss::en_euros_entiers(x.mensuel, x.horaire)),
        None => Err(socle::refus!("le logiciel n'a pas le plafond de la Sécurité sociale de {}", an)),
    }
}

/// Le SMIC horaire en vigueur le `jour`, tel que le bareme livre de son annee le porte ; aucun s'il ne le porte pas.
fn smic_au(jour: Jour) -> R<Option<MontantUnitaire>> {
    let Some(p) = droit::contexte::socle_livre_lu(jour.annee())? else { return Ok(None) };
    Ok(droit::contexte::smic_en_vigueur(&p.smic_horaire, &jour)?.map(|(_, v)| v))
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
    };
    let court = match contrat {
        Some(c) => engagement_court(c, artiste),
        None if !artiste || paie.duree() >= JOURS_ENGAGEMENT_CONTINU => false,
        None => return Err(inconnu("la pièce ne dit pas le contrat entier : l'engagement de l'artiste peut être de moins de cinq jours", true)),
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
/// butoir, son allocation estimee, et ce que vise la charge ; et les paies dont la date d'embauche se choisit (celles
/// dont la route la demande, et celles que la charge en dit deja).
fn droits_d_une_personne(inst: &Path, attestees: &[Attestee], nir: Option<&Nir>, nom: String, c: &ChargeDroitsDesPieces) -> R<(DroitsDUnePersonne, Vec<crate::ouverture::Choix>)> {
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
    comptees.sort_by(|a, b| (a.periode.fin(), &a.employeur).cmp(&(b.periode.fin(), &b.employeur)));
    // La personne se range sous son NIR (les lignes seules, sous `LIGNES_SEULES`) : une seule, que `position` ne mele a
    // personne.
    let cle = droit::contexte::IdSalarie::lire(nir.map_or(LIGNES_SEULES, |n| &**n))?;
    let mut declarees: Vec<(Declare, usize)> = Vec::new();
    for (i, p) in comptees.iter().enumerate() {
        match declaree(p, &cle) {
            Some(d) => declarees.push((d, i)),
            None => notes.push(format!(
                "La paie {} chez {} ne se compte pas : sa pièce ne dit pas ses heures, ses cachets et son brut ({}).",
                p.dates(),
                p.employeur,
                import::attestes::en_mots(p.heures, p.cachets, p.jours, p.brut)
            )),
        }
    }
    let lignes: Vec<Declare> = declarees.iter().map(|(d, _)| d.clone()).collect();
    // L'AFFILIATION, par le seul compte des droits (`droit::allocation::affiliation_des_droits`, celui de la route
    // `droits` d'une compagnie) : a la derniere fin des paies, prevues comprises, la date anniversaire disant la
    // readmission.
    let r = &t.readmission;
    let aff = droit::allocation::affiliation_des_droits(inst, &constantes, r, &lignes, &cle, c.anniversaire, None, naissance)?;
    let fin = aff.position.periode.fin();
    let butoir = droit::allocation::butoir(inst, &constantes, r, &lignes, &cle, naissance, &aff)?;
    let (lignes_du_compte, notes_du_compte) = crate::archives::comptes_de_la_position(&aff, "des pièces")?;
    notes.extend(notes_du_compte);
    let butoir_dit = butoir.as_ref().map(|b| butoir_dit(b, &aff, r));
    let phrase = match (&butoir_dit, aff.manque.est_nul()) {
        (Some(b), _) => b.phrase.clone(),
        (None, true) => format!("{} h sur la fenêtre : les {} h sont atteintes.", dit(aff.position.heures_ouverture()?), dit(aff.heures_exigees)),
        (None, false) => format!("Il vous manque {} h sur la fenêtre.", dit(aff.manque)),
    };
    // Les paies que la fenetre compte : celles de la position retenue.
    let dans_la_fenetre: Vec<&PaieComptee> = declarees.iter().filter(|(d, _)| aff.position.engagements.contains(d)).map(|(_, i)| &comptees[*i]).collect();
    // Les debuts de contrat que la charge dit, par le nom de leur paie ; un nom qu'aucune paie ne porte se dit.
    let mut debuts: BTreeMap<String, Jour> = BTreeMap::new();
    for d in c.debuts.iter().flatten() {
        if comptees.iter().any(|p| p.id == d.paie) {
            debuts.insert(d.paie.clone(), d.debut);
        } else {
            notes.push(format!("« {} » (Dates d'embauche) n'est le nom d'aucune paie, des pièces ni des lignes prévues : cette date ne dit rien.", d.paie));
        }
    }
    let mut demandes: BTreeMap<String, String> = BTreeMap::new();
    let (estimations, visee) = if aff.manque.est_nul() { estimer(&t, &aff, &dans_la_fenetre, c.allocation_visee, fin, &debuts, &mut demandes, &mut notes)? } else { (Vec::new(), Vec::new()) };
    // Une date d'embauche deja dite reste un choix de sa case, meme quand elle a comble la demande.
    for id in debuts.keys() {
        if let Some(p) = comptees.iter().find(|p| p.id == *id) {
            demandes.entry(id.clone()).or_insert_with(|| choix_d_embauche(p));
        }
    }
    let embauches = demandes.into_iter().map(|(code, libelle)| crate::ouverture::Choix::de(&code, libelle)).collect();
    let paies = comptees
        .iter()
        .map(|p| PaieDite { periode: p.dates(), employeur: p.employeur.clone(), piece: p.piece, cachets: p.cachets, heures: p.heures, enseignement: p.enseignement, brut: p.brut })
        .collect();
    let personne = DroitsDUnePersonne {
        nir: nir.cloned(),
        nom,
        fenetre: aff.position.libelle_fenetre.clone(),
        paies,
        lignes: lignes_du_compte,
        heures_exigees: aff.heures_exigees,
        manque: aff.manque,
        butoir: butoir_dit,
        phrase,
        notes,
        estimations,
        visee,
    };
    Ok((personne, embauches))
}

/// Le libelle d'une paie parmi les choix des dates d'embauche : son nom, ses dates, son employeur.
fn choix_d_embauche(x: &PaieComptee) -> String {
    format!("{} : paie {} chez {}", x.id, x.dates(), x.employeur)
}

/// LE JOUR BUTOIR EN MOTS : ce qui manque et jusqu'a quand ; pourquoi la date se deplace, chiffre depuis les paies (celles
/// qui sortent de la fenetre, la fenetre qui s'allonge) ; et le texte.
fn butoir_dit(b: &droit::allocation::Butoir, aff: &Affiliation, r: &droit::allocation::Readmission) -> ButoirDit {
    let jour = b.jour.en_clair();
    let apres = &b.lendemain;
    let phrase = if aff.manque.est_nul() {
        format!("Les {} h sont atteintes ; sans contrat de plus, elles ne le sont plus après le {}.", dit(aff.heures_exigees), jour)
    } else {
        format!("Il vous manque {} h, à faire d'ici le {} : ce n'est pas une échéance de France Travail, c'est le jour où, sans contrat de plus, il en faudra davantage.", dit(aff.manque), jour)
    };
    let c = &aff.position.constantes;
    let sortent: Vec<&Declare> = aff.position.engagements.iter().filter(|d| !apres.position.engagements.contains(d)).collect();
    let mut raisons = Vec::new();
    if !sortent.is_empty() {
        let heures: Heures = sortent.iter().map(|d| d.heures_equivalentes(c)).sum();
        let fins: Vec<String> = sortent.iter().map(|d| d.periode.fin().jj_mm_aaaa()).collect();
        raisons.push(format!(
            "les {} h de votre contrat qui finit le {} ne comptent plus : la fenêtre ne remonte qu'à {} jours avant la fin de contrat",
            dit(heures),
            fins.join(" et le "),
            c.fenetre_jours.jours_a_ecrire() + apres.periodes_en_plus * r.jours_par_periode
        ));
    }
    if apres.periodes_en_plus > aff.periodes_en_plus {
        raisons.push(format!(
            "la fenêtre s'allonge de {} jours, et il faut {} h ({} h de plus par {} jours)",
            r.jours_par_periode * (apres.periodes_en_plus - aff.periodes_en_plus),
            dit(apres.heures_exigees),
            dit(r.heures_par_periode),
            r.jours_par_periode
        ));
    }
    let pourquoi = match raisons.is_empty() {
        true => format!("Après le {}, il vous manquera {} h.", jour, dit(apres.manque)),
        false => format!("Après le {}, {} : il vous manquera {} h.", jour, raisons.join(" ; "), dit(apres.manque)),
    };
    let reference = match aff.precedente {
        None => "Annexes VIII et X au règlement d'assurance chômage, art. 3 : 507 heures sur les 365 jours qui précèdent la fin de contrat.".to_string(),
        Some(_) => "Annexes VIII et X au règlement d'assurance chômage, art. 3, et art. 9 §1 a et b : à la réadmission, 42 heures de plus par période de 30 jours au-delà du 365e jour, sans remonter avant la fin de contrat qui a ouvert le droit précédent.".to_string(),
    };
    ButoirDit { jour: b.jour, phrase, pourquoi, reference }
}

/// L'ALLOCATION ESTIMEE DE CHAQUE ANNEXE sur la fenetre de `aff`, et ce que demande l'allocation `visee`. Le salaire de
/// reference : le brut des paies de la fenetre, chacune sous sa limite (quatre fois son plafond, annexes, art. 11 §3) ;
/// une paie toute d'enseignement en sort (guide de France Travail,
/// `chomage-intermittents/francetravail-guide-intermittents-20261020.txt` : « les salaires au titre des heures
/// d'enseignement ne sont pas retenus dans le SR ») ; une paie qui mele l'enseignement a d'autres heures ne dit pas leur
/// salaire a part : rien ne s'estime, et la note dit laquelle. Les heures travaillees : celles du montant (sans
/// l'enseignement : `Position::heures_montant`). A la readmission (une fenetre allongee), les diviseurs de A et B sont
/// les heures exigees, fois le SMIC horaire de la fin de contrat pour A : INFERENCE, l'article 14 ne dit pas a quelle
/// date lire le SMIC.
#[allow(clippy::too_many_arguments)]
fn estimer(
    t: &Allocation,
    aff: &Affiliation,
    paies: &[&PaieComptee],
    visee: Option<Montant>,
    fin: Jour,
    debuts: &BTreeMap<String, Jour>,
    demandes: &mut BTreeMap<String, String>,
    notes: &mut Vec<String>,
) -> R<(Vec<EstimationDite>, Vec<ViseeDite>)> {
    let p = &aff.position;
    if let Some(m) = paies.iter().find(|x| x.melee()) {
        notes.push(format!(
            "L'allocation ne s'estime pas : la paie {} chez {} mêle {} h d'enseignement à d'autres heures ou des cachets, et le salaire de l'enseignement, qui sort du salaire de référence, n'y est pas dit à part.",
            m.dates(),
            m.employeur,
            dit(m.heures_d_enseignement())
        ));
        return Ok((Vec::new(), Vec::new()));
    }
    for x in paies.iter().filter(|x| x.d_enseignement()) {
        notes.push(format!("La paie {} chez {}, toute d'enseignement, sort du salaire de référence.", x.dates(), x.employeur));
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
    let diviseurs = match aff.periodes_en_plus {
        0 => Diviseurs::DeLAnnexe,
        _ => match smic_au(fin)? {
            Some(smic_horaire) => Diviseurs::Allonges { heures_exigees: aff.heures_exigees, smic_horaire },
            None => {
                notes.push(format!("L'allocation ne s'estime pas : à la réadmission, son diviseur prend le SMIC horaire, que le logiciel n'a pas au {}.", fin.jj_mm_aaaa()));
                return Ok((Vec::new(), Vec::new()));
            }
        },
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
                    notes.push(format!("Le début dit pour la paie {} chez {} ({}) est après sa fin : il ne dit rien.", x.dates(), x.employeur, d.jj_mm_aaaa()));
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
                    "La paie {} chez {} n'imprime pas son plafond et paie le mois entier : sa limite (art. 11 §3) se calcule sur le plafond du mois, peut-être plus haut que celui de son contrat ; l'estimation peut être trop haute.",
                    x.dates(),
                    x.employeur
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
        let mut sous_le_plus_bas: Vec<SousLePlusBas> = Vec::new();
        let mut attendent: Vec<String> = Vec::new();
        for (x, source) in du_salaire.iter().zip(&sources) {
            let Some(brut) = x.brut else { continue };
            // Un plafond qui ne se connait pas ne compte que si le brut peut passer sa limite : sous quatre fois le
            // plafond d'un contrat d'un jour, il ne la passe pas, quel que soit le contrat (« simple pour les simples »).
            let (plafond_paie, limite, note) = match plafond_de(source, artiste) {
                Ok(m) => (Some(m), Some(t.limite_de_la_paie(m)), None),
                Err(inconnu) => match plafond_le_plus_bas(x.periode.fin(), artiste) {
                    Ok(bas) if brut <= t.limite_de_la_paie(bas) => {
                        let au_plus_bas = t.limite_de_la_paie(bas);
                        let note = format!("son plafond ne se connaît pas ({}) ; son brut ne passe pas {}, quatre fois le plafond d'un contrat d'un jour : il compte entier", inconnu.raison, euros(au_plus_bas));
                        sous_le_plus_bas.push(SousLePlusBas { rang: bruts.len(), limite: au_plus_bas, inconnu, paie: x });
                        (None, None, Some(note))
                    }
                    Ok(_) | Err(_) => {
                        attendent.push(attente(x, &inconnu, demandes));
                        continue;
                    }
                },
            };
            let compte = droit::allocation::salaire_limite(&[(brut, limite)], Coefficient::UN);
            bruts.push((brut, limite));
            a_viser.push((**x, brut, limite));
            lignes.push(PaieDuSalaire { periode: x.dates(), employeur: x.employeur.clone(), brut, plafond: plafond_paie, limite, compte, note });
        }
        if !attendent.is_empty() {
            notes.push(format!("{} : l'allocation n'est pas estimée ; {}.", annexe_dite(annexe), attendent.join(" ; ")));
            continue;
        }
        let total: Montant = bruts.iter().fold(Montant::ZERO, |a, (b, _)| a + *b);
        let sr = droit::allocation::salaire_limite(&bruts, Coefficient::UN);
        let e = t.estimer(annexe, sr, nht, plafond, diviseurs)?;
        let borne = match e.borne {
            Some(Borne::Plancher) => format!(" ; le calcul donne {}, le plancher de l'annexe est de {}", euros(e.calculee), euros(e.plancher)),
            Some(Borne::Plafond) => format!(" ; le calcul donne {}, le plafond est de {}", euros(e.calculee), euros(e.plafond)),
            None => String::new(),
        };
        let limitee = if sr < total { format!(" ; les paies ne comptent qu'à quatre fois leur plafond (art. 11 §3) : {} de salaire, {} retenus", euros(total), euros(sr)) } else { String::new() };
        let phrase = format!(
            "{} : {} par jour, estimation (A {} pour {} de salaire de référence, B {} pour {} h, C {}){}{}.",
            annexe_dite(annexe),
            euros(e.retenue),
            euros(e.a),
            euros(sr),
            euros(e.b),
            dit(nht),
            euros(e.c),
            borne,
            limitee
        );
        estimations.push(EstimationDite { annexe: annexe_dite(annexe), a: e.a, b: e.b, c: e.c, calculee: e.calculee, salaire_de_reference: sr, retenue: e.retenue, phrase, paies: lignes });
        if let Some(v) = visee {
            visees.push(viser(t, annexe, v, sr, nht, plafond, diviseurs, &bruts, &a_viser, &sous_le_plus_bas, demandes)?);
        }
    }
    Ok((estimations, visees))
}

/// UNE PAIE DONT LE PLAFOND NE SE CONNAIT PAS, ET QUI COMPTE ENTIERE : son brut reste sous `limite`, quatre fois le plus
/// bas plafond qu'elle puisse avoir (`plafond_le_plus_bas`) ; `rang`, sa place parmi les bruts de l'annexe. Augmentee
/// pour une allocation visee, elle peut passer cette limite : alors la visee attend son plafond.
struct SousLePlusBas<'a> {
    rang: usize,
    limite: Montant,
    inconnu: PlafondInconnu,
    paie: &'a PaieComptee,
}

/// CE QU'UNE PAIE DONT LE PLAFOND NE SE CONNAIT PAS FAIT ATTENDRE, en mots : la paie, et pourquoi ; quand la date de
/// debut de son contrat le dirait, la route la demande (`demandes`, sous le nom de la paie, que la charge reprend dans
/// `debuts`).
fn attente(x: &PaieComptee, inconnu: &PlafondInconnu, demandes: &mut BTreeMap<String, String>) -> String {
    let demande = if inconnu.le_debut_le_dit {
        demandes.insert(x.id.clone(), choix_d_embauche(x));
        format!(" ; dites sa date d'embauche (« Dates d'embauche », paie « {} »)", x.id)
    } else {
        String::new()
    };
    format!("la paie {} chez {} peut passer sa limite (quatre fois son plafond, art. 11 §3), et son plafond ne se connaît pas ({}){}", x.dates(), x.employeur, inconnu.raison, demande)
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
    d: Diviseurs,
    bruts: &[(Montant, Option<Montant>)],
    a_viser: &[(&PaieComptee, Montant, Option<Montant>)],
    sous_le_plus_bas: &[SousLePlusBas],
    demandes: &mut BTreeMap<String, String>,
) -> R<ViseeDite> {
    let nom = annexe_dite(annexe);
    let au_plus = droit::allocation::salaire_au_plus(bruts);
    let aj_de = |s: Montant| t.estimer(annexe, s, nht, plafond, d).map(|e| e.retenue);
    let pour_le_plafond = match t.salaire_vise(annexe, plafond, nht, plafond, d)? {
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
    let vide = |phrase: String| ViseeDite { annexe: nom, salaire_de_reference: None, allocation: None, phrase, arret: arret.clone(), paies: Vec::new() };
    let cible = match t.salaire_vise(annexe, v, nht, plafond, d)? {
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
    // visee attend son plafond.
    let passent: Vec<String> = sous_le_plus_bas.iter().filter(|s| bruts.get(s.rang).is_some_and(|(b, _)| *b * k > s.limite)).map(|s| attente(s.paie, &s.inconnu, demandes)).collect();
    if !passent.is_empty() {
        return Ok(vide(format!("{} : pour {} par jour, il faudrait {} de salaire de référence ; la visée n'est pas estimée : {}.", nom, euros(v), euros(cible), passent.join(" ; "))));
    }
    let atteinte = aj_de(cible)?;
    let mut paies = Vec::new();
    for (x, brut, limite) in a_viser {
        let brut_vise = (*brut * k).au_centime();
        let (cachets, heures) = (x.cachets.into_iter().sum::<Cachets>(), x.heures.into_iter().sum::<Heures>());
        let par_cachet = (heures.est_nul() && !cachets.est_nul()).then(|| brut_vise.prorata(Cachets::UN, cachets).au_centime());
        let par_heure = if cachets.est_nul() { brut_vise.par_heure(heures) } else { None };
        paies.push(PaieVisee { periode: x.dates(), employeur: x.employeur.clone(), brut: *brut, brut_vise, limite: *limite, par_cachet, par_heure });
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
    Ok(ViseeDite { annexe: nom, salaire_de_reference: Some(cible), allocation: Some(atteinte), phrase, arret, paies })
}
