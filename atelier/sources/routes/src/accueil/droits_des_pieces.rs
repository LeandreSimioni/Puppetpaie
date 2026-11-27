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
//! CE QUE LA ROUTE REND, pour la personne : une ligne par paie retenue (la periode, l'employeur, les cachets, les heures
//! dont l'enseignement, le brut, la piece) ; le compte de la fenetre par le meme code que la route `droits` d'une
//! compagnie (`droit::cumul::Position`, `archives::comptes_de_la_position`) et ce qui manque aux 507 heures ;
//! l'allocation journaliere estimee de l'annexe VIII et de l'annexe X (`droit::allocation`) ; et, avec une allocation
//! visee, le salaire de reference qu'elle demande, chaque paie augmentee dans la meme proportion, et le plafond au-dela
//! duquel l'allocation ne monte plus. Des chiffres, pas un conseil.
//!
//! CE QUI NE SE COMPTE PAS ICI, et la route le dit : une DSN (ses contrats se comptent par leurs AEM ou leurs bulletins),
//! un certificat Congés Spectacles, une piece qui ne se lit pas ; et, dans l'estimation, la readmission, le salaire annuel
//! de reference (maternite, adoption, ALD), la limite du salaire de reference (annexes, art. 11 §3), la participation de
//! 0,93 % (art. 19), les franchises.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use droit::allocation::{Borne, Visee};
use droit::cumul::{Constantes, Declare, Position};
use droit::emplois::AnnexeChomage;
use droit::fiche::{Nir, Siren};
use engagement::importe::{Attestation, ContratLu};
use import::attestes::{BlocDImport, Registre};
use import::reponse::Source;
use import::tri::Sorte;
use socle::date::{Jour, Periode};
use socle::echec::{Classe, R};
use socle::fichiers::NomDeFichier;
use socle::grandeurs::ecrit::sans_zero::dit;
use socle::grandeurs::{Cachets, Coefficient, Heures, Montant};
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
    exemple: r#"{"allocation_visee":80}"#,
    rend: "dossier (mes-pieces/, où poser les pièces), pieces (une phrase par fichier), case (la personne à choisir, par son NIR, quand les pièces en portent plusieurs), personne (celle choisie : nom, fenetre, paies [periode, employeur, piece, cachets, heures, enseignement, brut], lignes [quoi, valeur], manque, phrase, notes, estimations par annexe [a, b, c, calculee, retenue, phrase], visee par annexe [salaire_de_reference, allocation, phrase, paies avec brut_vise, par_cachet, par_heure]), phrase",
    refus: &[Refus { classe: Classe::Refuse, corriger: "les pièces que la phrase nomme ; rien n'est écrit" }, OPERATION],
    pendant_la_demande: crate::catalogue::PendantLaDemande::Servie,
    acte: Acte::Lit,
    phrase: 200,
    ecran: Ecran::Lecture,
}];

charge! {
    /// La charge de `droits-des-pieces` : la personne (aucune : la seule des pieces, ou aucune s'il y en a plusieurs), la
    /// date anniversaire (aucune : les 12 mois jusqu'a aujourd'hui), l'allocation visee (aucune : rien ne se vise).
    #[derive(Debug)]
    struct ChargeDroitsDesPieces {
        personne: Option<Nir> = legende("Personne", "son numéro de sécurité sociale, parmi les choix que rend la route ; absent : la seule que portent les pièces").rendue(),
        anniversaire: Option<Jour> = legende("Date anniversaire", "la fin de la fenêtre que France Travail a notifiée ; absente : les 12 mois jusqu'à aujourd'hui"),
        allocation_visee: Option<Montant> = legende("Allocation visée", "l'allocation journalière qu'on voudrait, en euros ; absente : rien ne se vise"),
    }
}

/// Le libelle de la case de la personne.
const PERSONNE: Affiche = Affiche::new("Personne");

pub fn route(inst: &Path, nom: Route, charge: &Charge) -> R<Rendu> {
    match nom {
        Route::DroitsDesPieces => en_json(droits_des_pieces(inst, charge.lire()?)?),
        autre => Err(crate::enveloppe::route_inconnue(autre.nom())),
    }
}

// ------------------------------------------------------------------ ce que les pieces attestent

/// L'EMPLOYEUR D'UNE PIECE, tel qu'elle le dit : son Siren (celui de son Siret), sinon sa raison sociale ; aucun des deux,
/// les pieces sans employeur lu font un employeur a part. Il range les contrats : un registre par employeur.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum CleDEmployeur {
    Siren(Siren),
    Nom(String),
    NonLu,
}

/// Une piece lue : de qui (NIR), chez quel employeur, et le contrat qu'elle atteste.
struct Attestee {
    nir: Nir,
    employeur: CleDEmployeur,
    raison_sociale: Option<String>,
    nom: Option<String>,
    naissance: Option<Jour>,
    periode: Periode,
    ligne: ContratLu,
    source: Source,
    bloc: BlocDImport,
}

/// La cle de l'employeur que dit une piece.
fn cle_d_employeur(siren: Option<Siren>, raison_sociale: Option<&String>) -> CleDEmployeur {
    match (siren, raison_sociale.map(|r| r.trim()).filter(|r| !r.is_empty())) {
        (Some(s), _) => CleDEmployeur::Siren(s),
        (None, Some(r)) => CleDEmployeur::Nom(r.to_uppercase()),
        (None, None) => CleDEmployeur::NonLu,
    }
}

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
    let mut aem_employeurs: BTreeMap<String, CleDEmployeur> = BTreeMap::new();
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
                aem_employeurs.insert(copie.texte().to_string(), cle_d_employeur(siren, None));
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
                        let (Some(nir), Some(periode)) = (a.identite.nir.clone(), a.periode) else { continue };
                        let (ligne, _) = contrat_de_l_aem(&a, None, &copie)?;
                        attestees.push(Attestee {
                            nir,
                            employeur: cle_d_employeur(a.employeur.siret.as_ref().map(droit::fiche::Siret::siren), a.employeur.raison_sociale.as_ref()),
                            raison_sociale: a.employeur.raison_sociale.clone(),
                            nom: nom_de(&a.identite),
                            naissance: a.identite.naissance.and_then(|n| n.jour()),
                            periode,
                            ligne,
                            source: Source::Aem,
                            bloc: BlocDImport::AemPdf,
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
            Sorte::Dsn => phrases.push(format!("{} : une DSN ne se compte pas ici ; déposez les AEM ou les bulletins de ses contrats.", nom)),
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
            let employeur = match aem_employeurs.get(c.fichier.texte()) {
                Some(e) => e.clone(),
                None => CleDEmployeur::NonLu,
            };
            attestees.push(Attestee {
                nir: c.nir.clone(),
                employeur,
                raison_sociale: None,
                nom: None,
                naissance: None,
                periode: c.periode,
                ligne: import::dsn_aem::ligne_du_contrat(&c, None),
                source: Source::Aem,
                bloc: BlocDImport::Declarations,
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
        attestees.push(Attestee {
            nir: nir.clone(),
            employeur: cle_d_employeur(b.entreprise.employeur.siret.as_ref().map(droit::fiche::Siret::siren), raison.as_ref()),
            raison_sociale: raison,
            nom: b.salarie.nom_complet.clone().or_else(|| nom_de(&b.salarie.identite)),
            naissance: b.salarie.identite.naissance.and_then(|n| n.jour()),
            periode: b.periode,
            ligne,
            source: Source::Bulletin,
            bloc: BlocDImport::Bulletins,
        });
    }
    Ok(DossierDesPieces { attestees, phrases })
}

/// UNE PAIE RETENUE : un contrat atteste, chez son employeur, une fois.
struct Retenue {
    employeur: String,
    periode: Periode,
    ligne: ContratLu,
}

/// LES CONTRATS DE LA PERSONNE `nir`, un registre par employeur : les AEM d'abord, puis les bulletins, qui remplacent
/// l'AEM du meme contrat qui compte autre chose ; une piece dont le contrat est deja atteste n'ajoute rien.
fn retenues(attestees: &[Attestee], nir: &Nir) -> R<Vec<Retenue>> {
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
        if !pareils.is_empty() && (a.source != Source::Bulletin || pareils.iter().any(|p| p.contrat().atteste_par != Attestation::Aem)) {
            continue;
        }
        registre.ajouter(&a.nir, a.ligne.clone(), a.periode, a.source, a.bloc)?;
    }
    let mut out = Vec::new();
    for (cle, (registre, raison)) in registres {
        let employeur = match (raison, &cle) {
            (Some(r), _) => r,
            (None, CleDEmployeur::Siren(s)) => format!("SIREN {}", &**s),
            (None, CleDEmployeur::Nom(n)) => n.clone(),
            (None, CleDEmployeur::NonLu) => "employeur non lu".to_string(),
        };
        for (_, ligne, _) in registre.retenus {
            let Some(periode) = ligne.periode.and_then(|p| p.fermee()) else { continue };
            out.push(Retenue { employeur: employeur.clone(), periode, ligne });
        }
    }
    out.sort_by(|a, b| (a.periode, &a.employeur).cmp(&(b.periode, &b.employeur)));
    Ok(out)
}

// ------------------------------------------------------------------ ce que la route rend

/// Une paie retenue, telle que la route la dit : ce que sa piece compte (un compteur qu'elle ne dit pas reste vide).
#[derive(Debug, Serialize)]
struct PaieDesPieces {
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

/// L'allocation d'une annexe, estimee : ses parties, leur somme, ce qui est retenu, et sa phrase.
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
    retenue: Montant,
    phrase: String,
}

/// Une paie de la fenetre, augmentee pour l'allocation visee : son brut, le brut vise, et le montant d'un cachet ou d'une
/// heure qui y mene (quand la paie n'est faite que de cachets, ou que d'heures).
#[derive(Debug, Serialize)]
struct PaieVisee {
    periode: String,
    employeur: String,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    brut: Montant,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    brut_vise: Montant,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::option::serialize")]
    par_cachet: Option<Montant>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::option::serialize")]
    par_heure: Option<socle::grandeurs::MontantUnitaire>,
}

/// Ce que l'allocation visee demande sous une annexe.
#[derive(Debug, Serialize)]
struct ViseeDite {
    annexe: &'static str,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::option::serialize")]
    salaire_de_reference: Option<Montant>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::option::serialize")]
    allocation: Option<Montant>,
    phrase: String,
    paies: Vec<PaieVisee>,
}

/// Ce que la route dit de la personne choisie.
#[derive(Debug, Serialize)]
struct DroitsDUnePersonne {
    nir: Nir,
    nom: String,
    fenetre: String,
    paies: Vec<PaieDesPieces>,
    lignes: Vec<(String, String)>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    manque: Heures,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    case: Option<crate::fiches::CaseDeLaFiche<&'static str>>,
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
    let case = (personnes.len() > 1).then(|| {
        let offerts: Vec<crate::ouverture::Choix> = personnes.keys().map(|k| crate::ouverture::Choix::de(&**k, nom_de(k))).collect();
        crate::fiches::CaseDeLaFiche::de_la_boite(cle!(ChargeDroitsDesPieces, personne), PERSONNE, Forme::ChoixRendu, "", choisie.as_ref().and_then(socle::ecriture::ValeurDeCase::de)).offrant(offerts)
    });
    let personne = match &choisie {
        Some(nir) if personnes.contains_key(nir) => Some(droits_d_une_personne(inst, &attestees, nir, nom_de(nir), &c)?),
        Some(_) | None => None,
    };
    let phrase = match (&personne, &choisie, personnes.len()) {
        (Some(p), _, _) => socle::phrase!("{} ({}) : {}", p.nom, p.fenetre, p.phrase),
        (None, Some(_), _) => Phrase::fixe("Les pièces ne portent pas ce numéro de sécurité sociale : choisissez une personne parmi celles qu'elles portent."),
        (None, None, 0) => Phrase::fixe("Aucune pièce ne se compte encore : posez vos bulletins et vos AEM, de tous vos employeurs, dans le dossier mes-pieces."),
        (None, None, n) => socle::phrase!("Les pièces portent {} personnes : choisissez-en une, ses droits se comptent seuls.", n),
    };
    let phrases = personne.iter().flat_map(|p| p.notes.iter().cloned()).collect();
    Ok(DroitsDesPieces { phrase, dossier, pieces, phrases, case, personne })
}

/// Ce que la route dit de la personne `nir` : ses paies, sa fenetre, son allocation estimee, et ce que vise la charge.
fn droits_d_une_personne(inst: &Path, attestees: &[Attestee], nir: &Nir, nom: String, c: &ChargeDroitsDesPieces) -> R<DroitsDUnePersonne> {
    let aujourdhui = droit::contexte::aujourdhui()?;
    let constantes = Constantes::lire(inst, &aujourdhui)?;
    let fenetre = match c.anniversaire {
        Some(a) => droit::cumul::fenetre_anniversaire(&constantes, a)?,
        None => droit::cumul::fenetre_douze_mois(&constantes, aujourdhui)?,
    };
    let naissance = attestees.iter().filter(|a| a.nir == *nir).find_map(|a| a.naissance);
    let retenues = retenues(attestees, nir)?;
    // La personne se range sous son NIR : une seule, que `position` ne mele a personne.
    let cle = droit::contexte::IdSalarie::lire(nir)?;
    let mut notes = Vec::new();
    let mut declares: Vec<(Declare, String)> = Vec::new();
    let mut paies = Vec::new();
    for r in &retenues {
        let a = &r.ligne.contrat;
        paies.push(PaieDesPieces {
            periode: format!("du {} au {}", r.periode.debut().jj_mm_aaaa(), r.periode.fin().jj_mm_aaaa()),
            employeur: r.employeur.clone(),
            piece: piece_dite(a.atteste_par),
            cachets: a.cachets,
            heures: a.heures,
            enseignement: a.heures_enseignement,
            brut: a.brut,
        });
        match (a.heures, a.cachets, a.jours, a.brut) {
            (Some(heures), Some(cachets), Some(jours), Some(brut)) => declares.push((
                Declare { salarie: cle.clone(), periode: r.periode, heures: heures.entieres(), cachets, jours, brut, heures_enseignement: a.heures_enseignement.into_iter().sum() },
                r.employeur.clone(),
            )),
            _ => notes.push(format!(
                "La paie du {} au {} chez {} ne se compte pas : sa pièce ne dit pas tous ses compteurs ({}).",
                r.periode.debut().jj_mm_aaaa(),
                r.periode.fin().jj_mm_aaaa(),
                r.employeur,
                import::attestes::en_mots(a.heures, a.cachets, a.jours, a.brut)
            )),
        }
    }
    let lignes_declarees: Vec<Declare> = declares.iter().map(|(d, _)| d.clone()).collect();
    let p = droit::cumul::position(inst, &constantes, &lignes_declarees, &cle, &fenetre, naissance);
    // Les paies que la fenetre compte (`position` : celles dont la fin y est), avec leur employeur.
    let de_la_fenetre: Vec<(Declare, String)> = declares.into_iter().filter(|(d, _)| p.periode.contient(&d.periode.fin())).collect();
    let (lignes, notes_du_compte) = crate::archives::comptes_de_la_position(&p, "des pièces")?;
    notes.extend(notes_du_compte);
    let s = &p.constantes;
    let manque = p.manque()?;
    let phrase = if p.atteint()? {
        format!("{} h sur la fenêtre, le seuil de {} h est atteint.", dit(p.heures_ouverture()?), dit(s.seuil_heures))
    } else {
        format!("{} h sur la fenêtre : il manque {} h pour les {} h.", dit(p.heures_ouverture()?), dit(manque), dit(s.seuil_heures))
    };
    let (estimations, visee) = estimer(&p, &de_la_fenetre, c.allocation_visee, &mut notes)?;
    Ok(DroitsDUnePersonne { nir: nir.clone(), nom, fenetre: p.libelle_fenetre.clone(), paies, lignes, manque, phrase, notes, estimations, visee })
}

/// L'ALLOCATION ESTIMEE DE CHAQUE ANNEXE sur la fenetre `p`, et ce que demande l'allocation `visee`. Le salaire de
/// reference est le brut des paies de la fenetre, les heures travaillees celles du montant (sans l'enseignement :
/// `Position::heures_montant`). Le salaire de l'enseignement n'entre pas dans le salaire de reference (guide de France
/// Travail, `chomage-intermittents/francetravail-guide-intermittents-20261020.txt` : « les salaires au titre des heures
/// d'enseignement ne sont pas retenus dans le SR ») ; les pieces ne le disent pas a part : avec de l'enseignement dans la
/// fenetre, rien ne s'estime, et la note le dit.
fn estimer(p: &Position, de_la_fenetre: &[(Declare, String)], visee: Option<Montant>, notes: &mut Vec<String>) -> R<(Vec<EstimationDite>, Vec<ViseeDite>)> {
    if p.engagements.is_empty() || !p.atteint()? {
        return Ok((Vec::new(), Vec::new()));
    }
    if !p.heures_enseignement().est_nul() {
        notes.push("L'allocation ne s'estime pas : le salaire de l'enseignement n'entre pas dans le salaire de référence, et les pièces ne le disent pas à part.".to_string());
        return Ok((Vec::new(), Vec::new()));
    }
    let annee = p.periode.fin().annee();
    let Some(pss) = droit::contexte::socle_livre_lu(annee)?.and_then(|s| s.plafond_securite_sociale.map(|x| x.annuel)) else {
        notes.push(format!("L'allocation ne s'estime pas : le logiciel n'a pas le plafond de la Sécurité sociale de {}.", annee));
        return Ok((Vec::new(), Vec::new()));
    };
    let t = droit::allocation::allocation()?;
    let plafond = t.plafond(pss);
    let (sr, nht) = (p.brut(), p.heures_montant());
    notes.push(
        "Estimation : sans réadmission, ni maternité, adoption ou ALD dans la fenêtre, ni la limite du salaire de référence à quatre plafonds par paie (art. 11 §3), ni la participation retraite de 0,93 % (art. 19), ni franchises.".to_string(),
    );
    let mut estimations = Vec::new();
    let mut visees = Vec::new();
    for annexe in [AnnexeChomage::X, AnnexeChomage::Viii] {
        let e = t.estimer(annexe, sr, nht, plafond)?;
        let borne = match e.borne {
            Some(Borne::Plancher) => format!(" ; le calcul donne {}, le plancher de l'annexe est de {}", euros(e.calculee), euros(e.plancher)),
            Some(Borne::Plafond) => format!(" ; le calcul donne {}, le plafond est de {}", euros(e.calculee), euros(e.plafond)),
            None => String::new(),
        };
        let phrase = format!(
            "{} : {} par jour, estimation (A {} pour {} de salaire, B {} pour {} h, C {}){}.",
            annexe_dite(annexe),
            euros(e.retenue),
            euros(e.a),
            euros(sr),
            euros(e.b),
            dit(nht),
            euros(e.c),
            borne
        );
        estimations.push(EstimationDite { annexe: annexe_dite(annexe), a: e.a, b: e.b, c: e.c, calculee: e.calculee, retenue: e.retenue, phrase });
        let Some(v) = visee else { continue };
        visees.push(viser(&t, annexe, v, sr, nht, plafond, de_la_fenetre)?);
    }
    Ok((estimations, visees))
}

/// CE QUE L'ALLOCATION `v` DEMANDE SOUS L'ANNEXE `annexe` : le salaire de reference, chaque paie de la fenetre augmentee
/// dans la meme proportion (le brut vise, et le montant d'un cachet ou d'une heure qui y mene), et le plafond au-dela
/// duquel augmenter ne change plus l'allocation.
fn viser(t: &droit::allocation::Allocation, annexe: AnnexeChomage, v: Montant, sr: Montant, nht: Heures, plafond: Montant, de_la_fenetre: &[(Declare, String)]) -> R<ViseeDite> {
    let nom = annexe_dite(annexe);
    let au_plafond = match t.salaire_vise(annexe, plafond, nht, plafond)? {
        Visee::Salaire(s) => format!(" Au-delà de {} par jour, le plafond, l'allocation ne monte plus : il y faut {} de salaire de référence.", euros(plafond), euros(s)),
        Visee::DesLePlancher | Visee::SansSalaire | Visee::AuDelaDuPlafond => format!(" Le plafond est de {} par jour.", euros(plafond)),
    };
    let vide = |phrase: String| ViseeDite { annexe: nom, salaire_de_reference: None, allocation: None, phrase, paies: Vec::new() };
    let sr_vise = match t.salaire_vise(annexe, v, nht, plafond)? {
        Visee::Salaire(s) => s,
        Visee::AuDelaDuPlafond => return Ok(vide(format!("{} : {} par jour dépasse le plafond ; aucun salaire n'y mène.{}", nom, euros(v), au_plafond))),
        Visee::DesLePlancher => return Ok(vide(format!("{} : {} par jour ne passe pas le plancher, que les heures ouvrent déjà.{}", nom, euros(v), au_plafond))),
        Visee::SansSalaire => return Ok(vide(format!("{} : les heures seules donnent déjà {} par jour.{}", nom, euros(v), au_plafond))),
    };
    let Some(k) = sr_vise.rapport(sr) else {
        return Ok(vide(format!("{} : il faudrait {} de salaire de référence ; les paies de la fenêtre n'en ont aucun à augmenter.{}", nom, euros(sr_vise), au_plafond)));
    };
    let atteinte = t.estimer(annexe, sr_vise, nht, plafond)?.retenue;
    let mut paies = Vec::new();
    for (d, employeur) in de_la_fenetre {
        let brut_vise = (d.brut * k).au_centime();
        let par_cachet = (d.heures.est_nul() && !d.cachets.est_nul()).then(|| brut_vise.prorata(Cachets::UN, d.cachets).au_centime());
        let par_heure = if d.cachets.est_nul() { brut_vise.par_heure(d.heures) } else { None };
        paies.push(PaieVisee {
            periode: format!("du {} au {}", d.periode.debut().jj_mm_aaaa(), d.periode.fin().jj_mm_aaaa()),
            employeur: employeur.clone(),
            brut: d.brut,
            brut_vise,
            par_cachet,
            par_heure,
        });
    }
    let phrase = format!(
        "{} : pour {} par jour, il faut {} de salaire de référence au lieu de {}, chaque paie augmentée de {} % ; l'allocation serait de {}.{}",
        nom,
        euros(v),
        euros(sr_vise),
        euros(sr),
        dit((k - Coefficient::UN).en_taux().au_pair(1)),
        euros(atteinte),
        au_plafond
    );
    Ok(ViseeDite { annexe: nom, salaire_de_reference: Some(sr_vise), allocation: Some(atteinte), phrase, paies })
}
