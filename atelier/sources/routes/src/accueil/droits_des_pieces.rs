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
//! LES LIGNES PREVUES : la charge peut porter des contrats a venir, saisis a la main (`LignePrevue` : leur fin, des
//! cachets ou des heures, l'enseignement, un brut facultatif) ; chacune se compte comme une paie « prévisionnel », et rien
//! ne s'ecrit. Sans brut, elle compte pour les heures, pas pour l'allocation.
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
    rend: "dossier (mes-pieces/, où poser les pièces), pieces (une phrase par fichier), case (la personne à choisir, par son NIR, quand les pièces en portent plusieurs), personne (celle choisie : nom, fenetre, paies [periode, employeur, piece (bulletin, AEM, prévisionnel), cachets, heures, enseignement, brut], lignes [quoi, valeur], heures_exigees, manque, butoir [jour, phrase, pourquoi, reference], phrase, notes, estimations par annexe [a, b, c, calculee, salaire_de_reference, retenue, phrase, paies avec plafond et limite], visee par annexe [salaire_de_reference, allocation, phrase, arret, paies avec brut_vise, limite, par_cachet, par_heure]), phrase",
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
        personne: Option<Nir> = legende("Personne", "son numéro de sécurité sociale, parmi les choix que rend la route ; absent : la seule que portent les pièces").rendue(),
        anniversaire: Option<Jour> = legende("Date anniversaire", "celle du droit en cours ou du dernier droit, que France Travail a notifiée : la réadmission, la fin de contrat qui a ouvert ce droit étant douze mois avant ; absente : une première ouverture"),
        allocation_visee: Option<Montant> = legende("Allocation visée", "l'allocation journalière qu'on voudrait, en euros ; absente : rien ne se vise"),
        previsionnel: Option<Vec<LignePrevue>> = legende("Contrats prévus", "des contrats à venir, chacun sa fin, ses cachets ou ses heures, l'enseignement, son brut ; rien ne s'écrit"),
    }
}

/// UNE LIGNE PREVUE : un contrat a venir, saisi a la main pour un previsionnel. Sa fin ; son debut, facultatif (il dit
/// le plafond de la paie, pour la limite du salaire de reference) ; des cachets ou des heures ; si ses heures sont de
/// l'enseignement ; son brut, facultatif (sans lui, la ligne compte pour les heures, pas pour l'allocation).
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct LignePrevue {
    fin: Jour,
    debut: Option<Jour>,
    cachets: Option<Cachets>,
    heures: Option<Heures>,
    enseignement: Option<bool>,
    brut: Option<Montant>,
}

/// Les cases d'une ligne prevue, chacune la forme de son type.
const PARTIES_D_UNE_LIGNE_PREVUE: &[crate::catalogue::Partie] = &[
    crate::catalogue::Partie { cle: "fin", libelle: Affiche::new("Fin du contrat"), forme: Forme::Date, toujours: true },
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
    plafond: SourceDuPlafond,
}

/// D'OU VIENT LE PLAFOND D'UNE PAIE (le plafond de la Securite sociale de sa periode, `droit::plafond`), pour la limite du
/// salaire de reference (annexes, art. 11 §3) : celui que le bulletin imprime ; celui que le moteur calcule depuis le
/// contrat (`contrat`, entier quand la piece le dit : il dit si l'engagement d'un artiste est court) et la periode de la
/// paie ; ou rien, et pourquoi.
#[derive(Clone, Debug)]
enum SourceDuPlafond {
    Imprime(Montant),
    DuContrat { contrat: Option<Periode>, paie: Periode },
    NonDit(&'static str),
}

/// Le plafond que le bulletin imprime, colonne de la periode (`NomDeCumul::Plafond`).
fn plafond_imprime(b: &import::bulletin_lu::BulletinLu) -> Option<Montant> {
    b.cumuls.iter().find_map(|(n, c)| match (n, c) {
        (import::bulletin_lu::NomDeCumul::Plafond, import::bulletin_lu::Cumul::Euros(e)) => e.periode,
        _ => None,
    })
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
                        // Le contrat entier, quand l'AEM imprime son embauche et sa fin : il dit si l'engagement est court.
                        let contrat_imprime = match (a.embauche, a.fin_contrat) {
                            (Some(d), Some(f)) => Periode::du_au(d, f).ok(),
                            _ => None,
                        };
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
            plafond: match plafond_imprime(b) {
                Some(m) => SourceDuPlafond::Imprime(m),
                None => SourceDuPlafond::NonDit("le bulletin n'imprime pas son plafond"),
            },
        });
    }
    Ok(DossierDesPieces { attestees, phrases })
}

// ------------------------------------------------------------------ les paies comptees

/// UNE PAIE COMPTEE : un contrat qu'une piece atteste, chez son employeur, une fois ; ou une ligne prevue. Ce qu'elle
/// compte (un compteur que la piece ne dit pas reste vide), et d'ou vient son plafond.
struct PaieComptee {
    employeur: String,
    debut: Option<Jour>,
    fin: Jour,
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
        match self.debut {
            Some(d) if d != self.fin => format!("du {} au {}", d.jj_mm_aaaa(), self.fin.jj_mm_aaaa()),
            Some(_) => format!("le {}", self.fin.jj_mm_aaaa()),
            None => format!("finissant le {}", self.fin.jj_mm_aaaa()),
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
/// l'AEM du meme contrat qui compte autre chose ; une piece dont le contrat est deja atteste n'ajoute rien.
fn retenues(attestees: &[Attestee], nir: &Nir) -> R<Vec<PaieComptee>> {
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
            let a = &ligne.contrat;
            // La piece d'ou vient le contrat retenu (sa copie, sa page, ses dates) : son plafond.
            let plafond = match siennes().find(|x| x.periode == periode && x.ligne.contrat.fichier == a.fichier && x.ligne.contrat.page == a.page) {
                Some(x) => x.plafond.clone(),
                None => SourceDuPlafond::NonDit("la pièce du contrat ne se retrouve pas"),
            };
            out.push(PaieComptee {
                employeur: employeur.clone(),
                debut: Some(periode.debut()),
                fin: periode.fin(),
                piece: piece_dite(a.atteste_par),
                cachets: a.cachets,
                heures: a.heures,
                enseignement: a.heures_enseignement,
                jours: a.jours,
                brut: a.brut,
                plafond,
                prevue: false,
            });
        }
    }
    Ok(out)
}

/// Les lignes prevues de la charge, en paies comptees ; une ligne qui ne se compte pas le dit dans `notes`.
fn prevues(lignes: &[LignePrevue], notes: &mut Vec<String>) -> Vec<PaieComptee> {
    let mut out = Vec::new();
    for l in lignes {
        if l.cachets.is_none() && l.heures.is_none() {
            notes.push(format!("Le contrat prévu qui finit le {} ne dit ni cachets ni heures : il ne compte rien.", l.fin.jj_mm_aaaa()));
            continue;
        }
        if l.debut.is_some_and(|d| d > l.fin) {
            notes.push(format!("Le contrat prévu qui finit le {} commence après sa fin : il ne compte rien.", l.fin.jj_mm_aaaa()));
            continue;
        }
        let ens = l.enseignement == Some(true);
        let plafond = match l.debut {
            Some(d) => match Periode::du_au(d, l.fin) {
                Ok(p) => SourceDuPlafond::DuContrat { contrat: Some(p), paie: p },
                Err(_) => SourceDuPlafond::NonDit("ses dates ne font pas une période"),
            },
            None => SourceDuPlafond::NonDit("la ligne ne dit pas le début du contrat"),
        };
        out.push(PaieComptee {
            employeur: "prévisionnel".to_string(),
            debut: l.debut,
            fin: l.fin,
            piece: "prévisionnel",
            cachets: Some(l.cachets.into_iter().sum()),
            heures: Some(l.heures.into_iter().sum()),
            enseignement: ens.then(|| l.heures.into_iter().sum()),
            jours: None,
            brut: l.brut,
            plafond,
            prevue: true,
        });
    }
    out
}

/// LA LIGNE DECLAREE D'UNE PAIE COMPTEE (`droit::cumul::Declare`), sous la cle `cle` de la personne ; aucune quand sa
/// piece ne dit pas tous ses compteurs. La position ne lit d'une ligne que la fin de sa periode, ses heures, ses cachets
/// et son enseignement (`droit::cumul::Position`) : une ligne prevue sans debut se date de sa fin, sans jours ; son brut,
/// que le salaire de reference ne lit pas ici (il se compte paie par paie, `salaire_de_reference`), est nul quand elle ne
/// le dit pas.
fn declaree(p: &PaieComptee, cle: &droit::contexte::IdSalarie) -> R<Option<Declare>> {
    let jours = match (p.jours, p.prevue) {
        (Some(j), _) => j,
        (None, true) => Jours::ZERO,
        (None, false) => return Ok(None),
    };
    let brut = match (p.brut, p.prevue) {
        (Some(b), _) => b,
        (None, true) => Montant::ZERO,
        (None, false) => return Ok(None),
    };
    let (Some(heures), Some(cachets)) = (p.heures, p.cachets) else { return Ok(None) };
    let debut = match p.debut {
        Some(d) => d,
        None => p.fin,
    };
    Ok(Some(Declare { salarie: cle.clone(), periode: Periode::du_au(debut, p.fin)?, heures: heures.entieres(), cachets, jours, brut, heures_enseignement: p.heures_d_enseignement() }))
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
    nir: Nir,
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
fn plafond_de(s: &SourceDuPlafond, artiste: bool) -> Result<Montant, String> {
    use droit::plafond::{engagement_court, plafond_de_la_periode, plafond_urssaf, JoursSansRemuneration, JOURS_ENGAGEMENT_CONTINU};
    let (contrat, paie) = match s {
        SourceDuPlafond::Imprime(m) => return Ok(*m),
        SourceDuPlafond::NonDit(r) => return Err((*r).to_string()),
        SourceDuPlafond::DuContrat { contrat, paie } => (contrat, paie),
    };
    let court = match contrat {
        Some(c) => engagement_court(c, artiste),
        None if !artiste || paie.duree() >= JOURS_ENGAGEMENT_CONTINU => false,
        None => return Err("la pièce ne dit pas le contrat entier : l'engagement de l'artiste peut être de moins de cinq jours".to_string()),
    };
    let r = match (court, contrat) {
        (false, _) => plafond_de_la_periode(paie, &JoursSansRemuneration::aucun(), None, &pss),
        (true, Some(c)) if c.duree() == Duree::jours(1) => plafond_urssaf(c, true, &pss, &[c.debut()]),
        (true, _) => return Err("un artiste engagé moins de cinq jours a le plafond de ses journées de travail, que la pièce ne date pas".to_string()),
    };
    r.map_err(|e| e.phrase)
}

/// Ce que la route dit de la personne `nir` : ses paies, son affiliation, le jour butoir, son allocation estimee, et ce
/// que vise la charge.
fn droits_d_une_personne(inst: &Path, attestees: &[Attestee], nir: &Nir, nom: String, c: &ChargeDroitsDesPieces) -> R<DroitsDUnePersonne> {
    let aujourdhui = droit::contexte::aujourdhui()?;
    let constantes = Constantes::lire(inst, &aujourdhui)?;
    let t = droit::allocation::allocation()?;
    let naissance = attestees.iter().filter(|a| a.nir == *nir).find_map(|a| a.naissance);
    let mut notes = Vec::new();
    let mut comptees = retenues(attestees, nir)?;
    if let Some(v) = &c.previsionnel {
        comptees.extend(prevues(v, &mut notes));
    }
    comptees.sort_by(|a, b| (a.fin, &a.employeur).cmp(&(b.fin, &b.employeur)));
    // La personne se range sous son NIR : une seule, que `position` ne mele a personne.
    let cle = droit::contexte::IdSalarie::lire(nir)?;
    let mut declarees: Vec<(Declare, usize)> = Vec::new();
    for (i, p) in comptees.iter().enumerate() {
        match declaree(p, &cle)? {
            Some(d) => declarees.push((d, i)),
            None => notes.push(format!(
                "La paie {} chez {} ne se compte pas : sa pièce ne dit pas tous ses compteurs ({}).",
                p.dates(),
                p.employeur,
                import::attestes::en_mots(p.heures, p.cachets, p.jours, p.brut)
            )),
        }
    }
    let lignes: Vec<Declare> = declarees.iter().map(|(d, _)| d.clone()).collect();
    // LA FIN DE CONTRAT A LAQUELLE L'AFFILIATION SE RECHERCHE : aujourd'hui, la date anniversaire si elle vient, ou la
    // derniere fin des paies, prevues comprises. A la readmission, la fin de contrat qui a ouvert le droit precedent est
    // douze mois avant la date anniversaire (art. 9 §1 c : « la date anniversaire correspondant au terme des douze mois
    // suivant la fin du contrat de travail retenue pour l'ouverture »).
    let mut fin = aujourdhui;
    for j in [c.anniversaire, lignes.iter().map(|d| d.periode.fin()).max()].into_iter().flatten() {
        fin = fin.max(j);
    }
    let precedente = match c.anniversaire {
        Some(a) => Some(a.avant(Duree::mois(12))?),
        None => None,
    };
    let r = &t.readmission;
    let aff = droit::allocation::affiliation(inst, &constantes, r, &lignes, &cle, fin, precedente, naissance)?;
    let butoir = droit::allocation::butoir(inst, &constantes, r, &lignes, &cle, fin, precedente, naissance, &aff)?;
    let (lignes_du_compte, notes_du_compte) = crate::archives::comptes_de_la_position(&aff.position, "des pièces")?;
    notes.extend(notes_du_compte);
    if let Some(p) = precedente {
        notes.push(format!("Réadmission : seules comptent les paies qui finissent après le {}, la fin de contrat qui a ouvert le droit précédent.", p.jj_mm_aaaa()));
    }
    let butoir_dit = butoir.as_ref().map(|b| butoir_dit(b, &aff, r, precedente));
    let phrase = match (&butoir_dit, aff.manque.est_nul()) {
        (Some(b), _) => b.phrase.clone(),
        (None, true) => format!("{} h sur la fenêtre : les {} h sont atteintes.", dit(aff.position.heures_ouverture()?), dit(aff.heures_exigees)),
        (None, false) => format!("Il vous manque {} h sur la fenêtre.", dit(aff.manque)),
    };
    // Les paies que la fenetre compte : celles de la position retenue.
    let dans_la_fenetre: Vec<&PaieComptee> = declarees.iter().filter(|(d, _)| aff.position.engagements.contains(d)).map(|(_, i)| &comptees[*i]).collect();
    let (estimations, visee) = if aff.manque.est_nul() { estimer(&t, &aff, &dans_la_fenetre, c.allocation_visee, fin, &mut notes)? } else { (Vec::new(), Vec::new()) };
    let paies = comptees
        .iter()
        .map(|p| PaieDite { periode: p.dates(), employeur: p.employeur.clone(), piece: p.piece, cachets: p.cachets, heures: p.heures, enseignement: p.enseignement, brut: p.brut })
        .collect();
    Ok(DroitsDUnePersonne {
        nir: nir.clone(),
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
    })
}

/// LE JOUR BUTOIR EN MOTS : ce qui manque et jusqu'a quand ; pourquoi la date se deplace, chiffre depuis les paies (celles
/// qui sortent de la fenetre, la fenetre qui s'allonge) ; et le texte.
fn butoir_dit(b: &droit::allocation::Butoir, aff: &Affiliation, r: &droit::allocation::Readmission, precedente: Option<Jour>) -> ButoirDit {
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
    let reference = match precedente {
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
fn estimer(t: &Allocation, aff: &Affiliation, paies: &[&PaieComptee], visee: Option<Montant>, fin: Jour, notes: &mut Vec<String>) -> R<(Vec<EstimationDite>, Vec<ViseeDite>)> {
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
        notes.push(format!("Le contrat prévu {} n'a pas de brut : il compte pour les heures, pas pour l'allocation.", x.dates()));
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
    let mut estimations = Vec::new();
    let mut visees = Vec::new();
    for annexe in [AnnexeChomage::X, AnnexeChomage::Viii] {
        let artiste = annexe == AnnexeChomage::X;
        let mut lignes = Vec::new();
        let mut bruts: Vec<(Montant, Option<Montant>)> = Vec::new();
        for x in &du_salaire {
            let Some(brut) = x.brut else { continue };
            let (plafond_paie, note) = match plafond_de(&x.plafond, artiste) {
                Ok(m) => (Some(m), None),
                Err(r) => (None, Some(format!("son plafond ne se connaît pas ({}) : elle compte entière", r))),
            };
            let limite = plafond_paie.map(|m| t.limite_de_la_paie(m));
            let compte = droit::allocation::salaire_limite(&[(brut, limite)], Coefficient::UN);
            bruts.push((brut, limite));
            lignes.push(PaieDuSalaire { periode: x.dates(), employeur: x.employeur.clone(), brut, plafond: plafond_paie, limite, compte, note });
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
            let a_viser: Vec<(&PaieComptee, Montant, Option<Montant>)> = du_salaire.iter().zip(&bruts).map(|(x, (b, l))| (**x, *b, *l)).collect();
            visees.push(viser(t, annexe, v, sr, nht, plafond, diviseurs, &bruts, &a_viser)?);
        }
    }
    Ok((estimations, visees))
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
        Visee::DesLePlancher => return Ok(vide(format!("{} : {} par jour ne passe pas le plancher, que les heures ouvrent déjà.", nom, euros(v)))),
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
