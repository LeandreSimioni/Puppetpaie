//! LA REGULARISATION DE LA CASC-SVP DES MOIS IMPORTES : le dossier que prepare `exporter regularisation-casc-svp`, et
//! `casc-regularise`, qui note qu'une annee est regularisee. Un mois tenu dans un autre logiciel a pu partir sans la
//! CASC-SVP ; PuppetPaie la declare lui-meme dans les DSN qu'il fait (`declarations::dsn`).
//!
//! Tout se lit dans les engagements importes (`engagement::importe`) : les paies versees que la DSN deposee declare
//! (leur brut), la fiche du salarie, le contrat atteste (numero, bornes, cachets, heures et jours). N'y entre
//! qu'un mois dont une DSN importee se lit (`importation::ecrite::lire_dsn_casc`) et ne porte pas la CASC-SVP : c'est
//! elle qui dit si la cotisation est partie. Un mois sans DSN importee, ou dont la DSN ne se lit pas, n'y entre pas et
//! rien n'en est dit ; un mois deja regularise (`config/casc-svp.json`) non plus.
//!
//! Le classeur du CASC-SVP se recree avec les colonnes du modele (`donnees/modeles/casc-svp-liste-nominative.xlsx`) :
//! voir `liste_nominative_xlsx`.

use droit::fiche::Lue as _;
use droit::fiche::Nir;
use socle::echec::Classe;
use std::path::Path;

use serde::Serialize;
use socle::grandeurs::{nulle, Cachets, Heures, Jours, Montant, Taux};

use droit::contexte::Contexte;
use socle::date::{Jour, PeriodeOuverte};
use socle::ecriture::ecrire_json;
use socle::echec::R;
use socle::echec::erreur;
use socle::mois::{Annee, Mois};
use socle::texte::euros;

#[track_caller]
fn refus(m: impl Into<String>) -> socle::echec::Echec {
    erreur(Classe::Refuse, m)
}


/// Ce qui a ete verse a un salarie un mois importe, tel que l'engagement importe le porte.
struct Versee {
    salarie: droit::fiche::FicheSalarie,
    brut: Montant,
    numero: Option<engagement::numeros::NumeroContrat>,
    periode: Option<PeriodeOuverte>,
    /// Les cachets, heures et jours que le contrat atteste, quand il tient dans le mois.
    activite: Activite,
    /// Artiste ou technicien : ce que le bulletin de la paie imprime, sinon la fiche ; `None` : rien ne le dit.
    statut: Option<droit::fiche::Statut>,
}

/// L'ACTIVITE D'UN EMPLOI DE LA REGULARISATION : les cachets, les heures et les jours que son contrat atteste, ceux
/// qu'il dit et qui ne sont pas nuls.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Activite {
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "socle::grandeurs::ecrit::texte::option::serialize")]
    pub cachets: Option<Cachets>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "socle::grandeurs::ecrit::texte::option::serialize")]
    pub heures: Option<Heures>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "socle::grandeurs::ecrit::texte::option::serialize")]
    pub jours: Option<Jours>,
}

impl Activite {
    /// N'en dit-elle rien ?
    fn est_vide(&self) -> bool {
        self.cachets.is_none() && self.heures.is_none() && self.jours.is_none()
    }
}

/// Les paies versees de `mois` que sa DSN importee declare, d'apres les engagements importes : une par engagement qui
/// a un brut ce mois-la ; une paie qu'un bulletin seul declare n'en est pas. Un salarie sans fiche refuse (sa ligne de
/// la liste nominative ne se devine pas).
fn versees_du_mois(ctx: &Contexte, mois: Mois) -> R<Vec<Versee>> {
    let mut out = Vec::new();
    for i in engagement::importe::du_mois(&ctx.racine, mois)? {
        let Some(p) = i.paies_lues()?.into_iter().find(|p| p.mois == mois && !p.paie.contrat.d_un_bulletin()) else { continue };
        let Some(brut) = p.paie.brut else { continue };
        let cle = &p.salarie;
        let salarie = ctx.salaries.get(cle).cloned().ok_or_else(|| refus(format!("{} : la fiche du salarie « {} » d'un engagement importe n'existe pas.", mois, cle)))?;
        let periode = p.paie.periode;
        let mut activite = Activite::default();
        if let Some(c) = i.contrat_lu()? {
            if c.periode.and_then(|o| o.fermee()).is_some_and(|p| p.mois() == [mois]) {
                activite = Activite {
                    cachets: c.contrat.cachets.filter(|n| !n.est_nul()),
                    heures: c.contrat.heures.filter(|n| !n.est_nul()),
                    jours: c.contrat.jours.filter(|n| !n.est_nul()),
                };
            }
        }
        let numero = p.paie.contrat.numero_declare().cloned();
        let statut = p.paie.emploi_imprime.as_ref().and_then(|e| e.ecrit.statut).or_else(|| salarie.champs.statut.dite().copied());
        out.push(Versee { salarie, brut, numero, periode, activite, statut });
    }
    Ok(out)
}

/// Un mois du dossier.
#[derive(Serialize)]
pub struct MoisReg {
    pub mois: Mois,
    pub libelle: String,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    pub masse: Montant,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    pub taux: Taux,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    pub cotisation: Montant,
    pub source: String,
}

/// Une annee du dossier : un virement, et ce que l'ecran en dit.
#[derive(Serialize)]
pub struct AnneeReg {
    pub annee: Annee,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    pub masse: Montant,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    pub taux: Taux,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    pub cotisation: Montant,
    pub libelle_virement: String,
    pub libelle_dit: String,
    pub date_libelle: String,
}

/// Un salarie d'une annee du dossier, et la masse de ses salaires.
#[derive(Serialize)]
pub struct SalarieReg {
    pub annee: Annee,
    pub individu: IndividuReg,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    pub masse: Montant,
}

/// Le salarie d'une ligne de la liste nominative, tel que sa fiche le dit : ses cases, dans leurs types.
#[derive(Clone, Serialize)]
pub struct IndividuReg {
    /// La liste nominative se tient par NIR : une fiche sans NIR ne s'y range pas.
    pub nir: Nir,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nom: Option<droit::fiche::NomDeclare>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prenom: Option<droit::fiche::PrenomDeclare>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_naissance: Option<Jour>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adresse: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_postal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commune: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<droit::fiche::Courriel>,
}

/// Le contrat d'une ligne de la liste nominative, tel que l'engagement importe et la fiche le disent.
#[derive(Serialize)]
pub struct ContratReg {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub numero: Option<engagement::numeros::NumeroContrat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub periode: Option<PeriodeOuverte>,
    pub statut: droit::fiche::Statut,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emploi: Option<String>,
}

#[derive(Serialize)]
pub struct EmploiReg {
    pub annee: Annee,
    pub mois: Mois,
    pub individu: IndividuReg,
    pub contrat: ContratReg,
    pub activite: Activite,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub periode: Option<PeriodeOuverte>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    pub brut: Montant,
    pub fichier: String,
}

/// Un courriel du dossier, et son en-tete tel que l'ecran le dit.
#[derive(Serialize)]
pub struct Courriel {
    pub a: String,
    pub objet: String,
    pub corps: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub piece_jointe: Option<&'static str>,
    pub entete_dit: String,
}

impl Courriel {
    fn nouveau(a: String, objet: String, corps: String, piece_jointe: Option<&'static str>) -> Courriel {
        let entete_dit = format!("à {}, objet {}", a, objet);
        Courriel { a, objet, corps, piece_jointe, entete_dit }
    }
}

/// La regularisation CASC-SVP d'une annee : ce qui manque, par salarie et par mois, et ce qui en part. `etat` la nomme
/// parmi les etats qu'`exporter` rend.
#[derive(Serialize)]
pub struct Regularisation {
    pub etat: &'static str,
    pub periode: String,
    pub virements_titre: String,
    pub mois: Vec<MoisReg>,
    pub annees: Vec<AnneeReg>,
    pub salaries: Vec<SalarieReg>,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    pub total: Montant,
    pub a_faire: Vec<String>,
    pub courriel_audiens: Courriel,
    pub courriel_casc: Courriel,
    pub avertissements: Vec<String>,
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    pub fichiers: Vec<std::path::PathBuf>,
    /// Ou les fichiers sont ecrits ; aucun tant qu'ils ne le sont pas.
    #[serde(serialize_with = "socle::fichiers::ecrire_chemin")]
    pub dossier: Option<std::path::PathBuf>,
    pub boite_liee: bool,
    pub emplois: Vec<EmploiReg>,
}

/// Ce que demande une regularisation (`regularisation`).
pub struct Demande {
    pub mois: Option<Mois>,
    pub annee: Option<Annee>,
    pub toutes_les_annees: bool,
    pub jusqu_a: Option<Mois>,
    pub annees: Vec<Annee>,
    /// (annee, jour du virement)
    pub vire_le: Vec<(Annee, Jour)>,
    /// La boite courriel de la compagnie est-elle liee ? Le dossier le recopie (`Regularisation::boite_liee`).
    pub boite_liee: bool,
}

/// Le plafond mensuel de la Securite sociale du mois `mois` (`AAAA-MM`), tel que le socle livre de son annee le dit
/// (un plafond national : ni la convention ni la compagnie n'y changent rien), en euros entiers comme tout plafond lu
/// (`Pss::en_euros_entiers`) ; aucun quand le logiciel ne livre pas le bareme de cette annee.
fn plafond_mensuel(mois: Mois) -> R<Option<Montant>> {
    let an = mois.annee();
    let Some(socle) = droit::contexte::socle_livre_lu(an).map_err(|m| m.reclasser(Classe::Illisible))? else { return Ok(None) };
    match socle.plafond_securite_sociale {
        Some(p) => Ok(Some(droit::plafond::Pss::en_euros_entiers(p.mensuel, p.horaire).mensuel())),
        None => Err(erreur(Classe::DefautDuLogiciel, format!("Le barème livré de {} n'a pas le plafond de la Sécurité sociale : le signaler au développeur.", an))),
    }
}

/// L'EXEMPTION DE LA CASC-SVP AU MOIS `m` (« AAAA-MM ») : la fiche entreprise de ce mois, lue avec les cles de l'effectif
/// `eff` de son annee (`droit::bareme::casc_exemptee`), comme la DSN du mois la lit.
pub(crate) fn exemptee_au_mois(ctx: &Contexte, eff: &droit::contexte::EffectifsParAnnee, mois: Mois) -> R<bool> {
    let du_mois = ctx.au(&mois.dernier_jour())?;
    let an = mois.annee();
    Ok(droit::bareme::casc_exemptee(&du_mois.entreprise, eff.cinquante(an).as_ref(), eff.cse(an).as_ref()))
}

/// CE QU'UNE DEMANDE DE REGULARISATION TROUVE : le dossier, ou rien a regulariser, et pourquoi (tout est regularise,
/// porte par les DSN, ou hors du dispositif), dit comme l'export le dit.
pub enum ARegulariser {
    Dossier(Box<Regularisation>),
    Rien(String),
}

impl ARegulariser {
    /// Le dossier ; rien a regulariser refuse, en disant pourquoi.
    pub fn dossier(self) -> R<Regularisation> {
        match self {
            ARegulariser::Dossier(r) => Ok(*r),
            ARegulariser::Rien(pourquoi) => Err(refus(pourquoi)),
        }
    }
}

/// La regularisation CASC-SVP que demande `d`, calculee sans rien ecrire (l'ecriture des fichiers : `ecrire`).
pub fn regularisation(ctx: &Contexte, d: &Demande) -> R<ARegulariser> {
    let base = droit::adhesions::casc_svp_base().map_err(|m| m.reclasser(Classe::Disque))?;
    let ent = &ctx.entreprise;
    let idcc = ent.idcc.exigee("idcc").map_err(|e| e.reclasser(Classe::Refuse))?;
    if *idcc != base.idcc {
        return Err(refus(format!(
            "La CASC-SVP est une cotisation de la convention 3090 ; la fiche entreprise dit {}. Il n'y a rien a regulariser.",
            idcc
        )));
    }
    let eff = ctx.effectif().map_err(|e| e.reclasser(Classe::Refuse))?;
    let faits = declarations::casc::mois_regularises(ctx).map_err(|m| m.reclasser(Classe::Disque))?;
    let depuis = base.depuis;
    let mut candidats: Vec<Mois> = import::mois::mois_importes(&ctx.racine)?.into_iter().filter(|m| *m >= depuis && !faits.contains(m)).collect();
    let annee_de = |m: &Mois| m.annee();
    if let Some(m) = d.mois {
        candidats.retain(|x| m == *x);
    } else if let Some(a) = d.annee.filter(|_| !d.toutes_les_annees) {
        candidats.retain(|x| annee_de(x) == a);
    }
    if let Some(j) = d.jusqu_a {
        candidats.retain(|x| *x <= j);
    }
    if !d.annees.is_empty() {
        candidats.retain(|x| d.annees.iter().any(|a| annee_de(x) == *a));
    }
    // Un mois d'une annee ou la compagnie est exemptee (article 4.22 B) ne doit rien : l'exemption se lit au mois.
    let mut exemptes: Vec<Mois> = vec![];
    for &m in &candidats {
        if exemptee_au_mois(ctx, &eff, m)? {
            exemptes.push(m);
        }
    }
    candidats.retain(|m| !exemptes.contains(m));
    let liste = |v: &[Mois]| v.iter().map(Mois::to_string).collect::<Vec<_>>().join(", ");
    if candidats.is_empty() && !exemptes.is_empty() {
        return Ok(ARegulariser::Rien(format!(
            "L'entreprise est hors du dispositif CASC-SVP (article 4.22 B : au moins 50 salaries, un CSE qui contribue au moins autant) pour {} : il n'y a rien a regulariser.",
            liste(&exemptes)
        )));
    }
    let mut avert: Vec<String> = vec![];
    if !exemptes.is_empty() {
        avert.push(format!("Hors du dispositif CASC-SVP (article 4.22 B), donc hors du dossier : {}.", liste(&exemptes)));
    }
    let mut portees: Vec<Mois> = vec![];
    let mut lues: Vec<(Mois, Vec<Versee>)> = vec![];
    let contrat_casc = base.dsn.contrat.clone();
    for &m in &candidats {
        // La DSN deposee est ce qui dit si la CASC-SVP est partie : un mois sans DSN importee, ou dont la DSN ne se lit
        // pas, n'entre pas, et rien n'en est dit.
        let Some(dsn) = crate::importation::ecrite::dsn_importee(ctx, m).map_err(|e| e.reclasser(Classe::Refuse))? else { continue };
        let Some(porte) = crate::importation::ecrite::lire_dsn_casc(&dsn, &contrat_casc)? else { continue };
        let paies = versees_du_mois(ctx, m)?;
        if paies.is_empty() {
            continue;
        }
        if porte {
            portees.push(m);
            continue;
        }
        lues.push((m, paies));
    }
    if lues.is_empty() {
        if !faits.is_empty() {
            return Ok(ARegulariser::Rien(format!(
                "Tout est regularise : {}. Les DSN suivantes portent la CASC-SVP elles-memes.",
                deja_faites(ctx)?
            )));
        }
        return Ok(ARegulariser::Rien(format!(
            "Aucun mois importe sans la CASC-SVP dans sa DSN depuis {} dans la periode demandee : rien a regulariser.",
            depuis
        )));
    }
    if !faits.is_empty() {
        let mut tries = faits.clone();
        tries.sort();
        avert.push(format!("Deja regularises, donc hors du dossier : {}.", tries.iter().map(|m| m.to_string()).collect::<Vec<_>>().join(", ")));
    }
    if !portees.is_empty() {
        avert.push(format!("La DSN porte deja la CASC-SVP, donc hors du dossier : {}.", liste(&portees)));
    }

    let mut par_mois: Vec<MoisReg> = vec![];
    let mut par_salarie: Vec<((Annee, Nir), SalarieReg)> = vec![];
    let mut emplois: Vec<EmploiReg> = vec![];
    for (m, paies) in &lues {
        let source = format!("engagements importes de {}", m);
        let mut masse: Montant = nulle();
        let mut mois_sal: Vec<(Nir, String, Montant)> = vec![];
        let an = annee_de(m);
        for p in paies {
            // Les colonnes de l'etat nomment les cases de la fiche typee, chacune telle qu'elle s'ecrit.
            let f = &p.salarie.champs;
            let dit = |t: Option<&str>| t.filter(|t| !t.is_empty()).map(str::to_string);
            let (adresse, code_postal, commune) = (dit(f.adresse.portee()), dit(f.code_postal.portee()), dit(f.commune.portee()));
            let brut = p.brut;
            let nir = f.nir.dite().cloned().ok_or_else(|| refus(format!("{} : la fiche du salarié « {} » n'a pas de NIR ; la liste nominative de la régularisation le demande.", m, p.salarie.id)))?;
            let individu = IndividuReg {
                nir: nir.clone(),
                nom: f.nom.clone(),
                prenom: f.prenom.clone(),
                date_naissance: f.date_naissance,
                adresse,
                code_postal,
                commune,
                email: f.email.clone(),
            };
            masse += brut;
            match mois_sal.iter_mut().find(|(n, _, _)| *n == nir) {
                Some((_, _, t)) => *t += brut,
                None => mois_sal.push((nir.clone(), match &f.nom {
                    Some(n) => n.to_string(),
                    None => nir.to_string(),
                }, brut)),
            }
            let cle = (an, nir);
            let pos = match par_salarie.iter().position(|(k, _)| *k == cle) {
                Some(i) => i,
                None => {
                    par_salarie.push((cle, SalarieReg { annee: an, individu: individu.clone(), masse: nulle() }));
                    par_salarie.len() - 1
                }
            };
            par_salarie[pos].1.masse += brut;
            // La colonne « A » ou « T » de la liste nominative : un statut que rien ne dit se demande.
            let statut = p.statut.ok_or_else(|| {
                socle::echec::cle(
                    format!("{} : la fiche du salarié « {} » ne dit pas son statut, artiste ou technicien ; la liste nominative de la régularisation le demande : le dire sur sa fiche.", m, p.salarie.id),
                    "statut",
                )
            })?;
            let contrat = ContratReg { numero: p.numero.clone(), periode: p.periode, statut, emploi: dit(f.emploi.portee()) };
            emplois.push(EmploiReg {
                annee: an,
                mois: *m,
                individu,
                activite: p.activite.clone(),
                periode: p.periode,
                contrat,
                brut,
                fichier: source.clone(),
            });
        }
        if let Some(plafond) = plafond_mensuel(*m)? {
            for (_, nom, b) in &mois_sal {
                if *b > plafond {
                    avert.push(format!("{}, {} : brut {} au-dessus du plafond mensuel -- la T1 n'est plus égale au brut, base à recalculer.", m, nom, euros(*b)));
                }
            }
        }
        let taux = base.taux.de(an).ok_or_else(|| {
            socle::echec::refus_avec(
                format!("Le taux de la CASC-SVP pour {} n'est pas releve : l'export refuse plutot que de prolonger celui d'une autre annee.", an),
                socle::echec::Lieu { fichier: Some(const { socle::fichiers::CheminRelatif::du_moteur("donnees/casc-svp-base.json") }), cle: Some(format!("taux.{}", an)), ..Default::default() },
            )
            .reclasser(Classe::Refuse)
        })?;
        let cot = droit::adhesions::TauxCasc::cotisation(taux, masse);
        par_mois.push(MoisReg { mois: *m, libelle: m.libelle(socle::mois::Graphie::Accentuee), masse, taux, cotisation: cot, source });
    }

    let siret = ent.siret.ecrite("siret").map_err(|e| e.reclasser(Classe::Refuse))?.to_string();
    let mut annees: Vec<AnneeReg> = vec![];
    for r in &par_mois {
        let an = annee_de(&r.mois);
        let pos = match annees.iter().position(|a| a.annee == an) {
            Some(p) => p,
            None => {
                let libelle_virement = base.reglement.libelle.replace("{siret}", &siret).replace("{annee}", &an.to_string());
                annees.push(AnneeReg {
                    libelle_dit: format!("libellé « {} »", libelle_virement),
                    date_libelle: format!("Virement {} fait le", an),
                    libelle_virement,
                    annee: an,
                    masse: nulle(),
                    taux: r.taux,
                    cotisation: nulle(),
                });
                annees.len() - 1
            }
        };
        annees[pos].masse += r.masse;
        annees[pos].cotisation += r.cotisation;
    }
    let total = annees.iter().fold(nulle::<Montant>(), |t, a| t + a.cotisation);

    let nom = ent.raison_sociale.ecrite("raison_sociale").map_err(|e| e.reclasser(Classe::Refuse))?.to_string();
    let ou = |a: &Option<String>, b: &Option<String>| a.remplie().or_else(|| b.remplie()).map(String::from);
    // Le signataire de la fiche, s'il est dit ; sans lui, les courriels ne sont pas signes.
    let signe: String = ent
        .signataire
        .dite()
        .map(|sig| [ou(&sig.prenom_affichage, &sig.prenom), ou(&sig.nom_affichage, &sig.nom)].into_iter().flatten().collect::<Vec<_>>().join(" "))
        .into_iter()
        .collect();
    let (premier, dernier) = (&par_mois[0].libelle, &par_mois[par_mois.len() - 1].libelle);
    let periode = if premier == dernier { premier.clone() } else { format!("{} à {}", premier, dernier) };

    let mut tableau = vec!["| mois | masse | cotisation |".to_string(), "|---|---:|---:|".to_string()];
    for a in &annees {
        for r in &par_mois {
            if annee_de(&r.mois) == a.annee {
                tableau.push(format!("| {} | {} | {} |", r.libelle, euros(r.masse), euros(r.cotisation)));
            }
        }
        tableau.push(format!("| **{}, à {} %** | **{}** | **{}** |", a.annee, socle::texte::virgule(a.taux), euros(a.masse), euros(a.cotisation)));
    }
    let virements: Vec<String> = annees
        .iter()
        .map(|a| {
            // Une date de virement que la demande ne dit pas reste a remplir a la main.
            let date = match d.vire_le.iter().find(|(k, _)| *k == a.annee).map(|(_, v)| v.jj_mm_aaaa()) {
                Some(j) => j,
                None => "__/__/____".to_string(),
            };
            format!("- le {}, {}, libellé « {} » ;", date, euros(a.cotisation), a.libelle_virement)
        })
        .collect();
    let beneficiaire = base.reglement.beneficiaire.clone();
    let corps_audiens = format!(
        "Bonjour,\n\nNotre logiciel de paie n'était pas paramétré pour déclarer la cotisation CASC-SVP en DSN (contrat CASCSVP) : elle manque à nos DSN de {}. Nous régularisons par virement, comme l'indique la page employeurs du CASC-SVP, et nos DSN la porteront désormais.\n\n{} — SIRET {} — IDCC 3090.\n\nMasses salariales soumises à cotisation (tranche 1 Agirc-Arrco) :\n\n{}\n\nVirements effectués vers {} :\n{}\n\nLa liste nominative des salariés part en parallèle au CASC-SVP.\n\nBien cordialement,\n\n{}\n{}\n",
        periode, nom, siret, tableau.join("\n"), beneficiaire, virements.join("\n"), signe, nom
    );
    let a_audiens = base.courriel_audiens.a.clone();
    let corps_casc = format!(
        "Bonjour,\n\nNotre logiciel de paie ne déclarait pas la cotisation CASC-SVP en DSN. Nous régularisons auprès d'Audiens ({}) les cotisations de {}, et vous trouverez en pièce jointe la liste nominative de nos salariés pour cette période.\n\n{} — SIRET {} — IDCC 3090.\n\nBien cordialement,\n\n{}\n{}\n",
        a_audiens, periode, nom, siret, signe, nom
    );
    let courriel_audiens = Courriel::nouveau(a_audiens.clone(), base.courriel_audiens.objet.replace("{siret}", &siret), corps_audiens, None);
    let courriel_casc = Courriel::nouveau(
        base.courriel_casc.a.clone(),
        base.courriel_casc.objet.replace("{siret}", &siret),
        corps_casc,
        Some("CASC-SVP_liste-nominative.xlsx (le modele du CASC-SVP, rempli)"),
    );
    let mut a_faire = vec![format!(
        "Faire {} virement(s), un par année, à {} — l'IBAN se lit sur la page employeurs du CASC-SVP ({}) ou dans l'espace Audiens :",
        annees.len(),
        beneficiaire,
        base.page
    )];
    for a in &annees {
        a_faire.push(format!("    {} : {}, libellé « {} »", a.annee, euros(a.cotisation), a.libelle_virement));
    }
    a_faire.push(format!("Reporter la date de chaque virement dans le courriel à Audiens, puis l'envoyer à {} (objet : {}).", courriel_audiens.a, courriel_audiens.objet));
    a_faire.push(format!(
        "Envoyer à {} (objet : {}) le courriel au CASC-SVP, avec en pièce jointe CASC-SVP_liste-nominative.xlsx — leur modèle, déjà rempli.",
        courriel_casc.a, courriel_casc.objet
    ));
    a_faire.push("Une fois les virements faits, le noter dans le logiciel : Exports, bouton « Noter comme régularisé » (route `casc-regularise`) — les mois sortent alors des dossiers suivants.".to_string());
    a_faire.push("Vérifier que la fiche de paramétrage Audiens porte les deux lignes CASC-SVP, pour que les DSN suivantes la déclarent.".to_string());

    let boite = d.boite_liee;
    Ok(ARegulariser::Dossier(Box::new(Regularisation {
        etat: "regularisation-casc-svp",
        periode,
        virements_titre: if annees.len() > 1 { format!("Les {} virements", annees.len()) } else { "Le virement".to_string() },
        mois: par_mois,
        annees,
        salaries: par_salarie.into_iter().map(|(_, s)| s).collect(),
        total,
        a_faire,
        courriel_audiens,
        courriel_casc,
        avertissements: avert,
        fichiers: vec![],
        dossier: None,
        boite_liee: boite,
        emplois,
    })))
}

fn fichier_suivi(ctx: &Contexte) -> std::path::PathBuf {
    declarations::casc::CASC_SVP.dans(&ctx.racine)
}

/// UNE CELLULE DE LA LISTE : ce que la fiche dit ; une case qu'elle ne dit pas reste une cellule vide.
fn cellule<T: std::fmt::Display>(o: &Option<T>) -> String {
    o.iter().map(ToString::to_string).collect()
}

/// Les annees deja notees regularisees, en une phrase : l'annee, le montant vire (en euros s'il se lit en nombre) et le
/// jour du virement.
fn deja_faites(ctx: &Contexte) -> R<String> {
    let d = declarations::casc::lue(ctx).map_err(|m| m.reclasser(Classe::Illisible))?;
    Ok(d.iter()
        .flat_map(|d| &d.regularisations)
        .map(|x| format!("{} ({}, viré le {})", x.annee, euros(x.montant), x.vire_le.jj_mm_aaaa()))
        .collect::<Vec<_>>()
        .join(", "))
}

/// L'ecriture de la regularisation : `exports/casc-svp/` -- les CSV, les deux courriels, la liste des gestes. Rend le
/// dossier ecrit.
pub fn ecrire(ctx: &Contexte, r: &mut Regularisation) -> R<std::path::PathBuf> {
    let dossier = documents::DossierDeTravail::RegularisationCasc.sous(&ctx.racine);
    let io = |p: &Path, e: std::io::Error| socle::echec::du_disque(&p, &e).reclasser(Classe::Disque);
    std::fs::create_dir_all(&dossier).map_err(|e| io(&dossier, e))?;
    // Un nombre d'une case du CSV : la grandeur telle qu'elle s'ecrit, la virgule decimale.
    use socle::texte::a_virgule as virgule;
    let mut fichiers: Vec<std::path::PathBuf> = vec![];
    // Le classeur modele : voir `liste_nominative_xlsx`.
    liste_nominative_xlsx(ctx, r, &dossier, &mut fichiers)?;
    let ecrit_csv = |nom: &str, entetes: &[&str], lignes: Vec<Vec<String>>| -> R<std::path::PathBuf> {
        let mut t = socle::csv::ligne(&entetes.iter().map(|s| s.to_string()).collect::<Vec<_>>(), ';');
        for l in &lignes {
            t.push_str(&socle::csv::ligne(l, ';'));
        }
        let p = dossier.join(nom);
        std::fs::write(&p, format!("\u{feff}{}", t)).map_err(|e| io(&p, e))?;
        Ok(p)
    };
    fichiers.push(ecrit_csv(
        "CASC-SVP_masses-par-mois.csv",
        &["mois", "masse", "taux %", "cotisation", "source"],
        r.mois.iter().map(|x| vec![x.mois.to_string(), virgule(x.masse), virgule(x.taux), virgule(x.cotisation), x.source.clone()]).collect(),
    )?);
    fichiers.push(ecrit_csv(
        "CASC-SVP_liste-nominative.csv",
        &["annee", "salarie", "NIR", "date de naissance", "adresse", "masse"],
        r.salaries
            .iter()
            .map(|s| {
                let i = &s.individu;
                let salarie = [cellule(&i.nom), cellule(&i.prenom)].into_iter().filter(|t| !t.is_empty()).collect::<Vec<_>>().join(" ");
                let adresse = [&i.adresse, &i.code_postal, &i.commune].into_iter().flatten().cloned().collect::<Vec<_>>().join(" ");
                vec![s.annee.to_string(), salarie, i.nir.to_string(), i.date_naissance.iter().map(Jour::iso).collect(), adresse, virgule(s.masse)]
            })
            .collect(),
    )?);
    for (nom, c) in [("CASC-SVP_courriel-audiens.txt", &r.courriel_audiens), ("CASC-SVP_courriel-casc.txt", &r.courriel_casc)] {
        let p = dossier.join(nom);
        std::fs::write(&p, format!("À : {}\nObjet : {}\n\n{}", c.a, c.objet, c.corps)).map_err(|e| io(&p, e))?;
        fichiers.push(p);
    }
    let p = dossier.join("CASC-SVP_a-faire.txt");
    let verifier = if r.avertissements.is_empty() { String::new() } else { format!("\nÀ vérifier :\n{}", r.avertissements.join("\n")) };
    std::fs::write(&p, format!("Régularisation CASC-SVP — {} — total {}\n\n{}\n{}\n", r.periode, euros(r.total), r.a_faire.join("\n"), verifier)).map_err(|e| io(&p, e))?;
    fichiers.insert(0, p);
    r.fichiers = fichiers;
    r.dossier = Some(dossier.clone());
    Ok(dossier)
}

const ENTETES: [(u32, u16, &str); 42] = [
    (0, 0, "Les 3 lignes du haut de la feuille sont à supprimer avant d'enregistrer le fichier au format .CSV (séparateur point-virgule) pour télétransmission."),
    (0, 26, "Salariés autre qu'Intermittents"),
    (0, 30, "Salariés Intermittents"),
    (1, 26, "Nombre d'heures"),
    (1, 30, "\"A\" si artiste\n\"T\" si technicien"),
    (1, 31, "Techniciens"),
    (1, 33, "Artistes"),
    (2, 0, "mention\n\"CASC-SVP\""),
    (2, 1, "N°Siret"),
    (2, 2, "VIDE"),
    (2, 3, "VIDE"),
    (2, 4, "Année de déclaration concernée"),
    (2, 5, "Date de constitution du Fichier"),
    (2, 6, "Nom de naissance"),
    (2, 7, "Nom d'artiste\nNom Marital"),
    (2, 8, "Prénom"),
    (2, 9, "Date de Naissance"),
    (2, 10, "Numéro de Sécurité Sociale avec Clé"),
    (2, 11, "n° App ou Bal / couloir / escalier"),
    (2, 12, "Entrée / bâtiment / immeuble / résidence"),
    (2, 13, "Numéro et libellé de la voie"),
    (2, 14, "Lieu dit ou boîte postale"),
    (2, 15, "Code Postal  en France"),
    (2, 16, "Libellé Commune"),
    (2, 17, "Code postal si hors France"),
    (2, 18, "Pays"),
    (2, 19, "Adresse mail personnelle"),
    (2, 20, "Adresse mail professionnelle"),
    (2, 21, "Libellé de l'Emploi"),
    (2, 22, "Date de début d' emploi"),
    (2, 23, "Date de fin d'emploi"),
    (2, 24, "Salaire de la période"),
    (2, 25, "\"I\" si intermittent\n\"P\" si permanent"),
    (2, 26, "VIDE"),
    (2, 28, "VIDE"),
    (2, 29, "VIDE"),
    (2, 31, "NbJ"),
    (2, 32, "NbH"),
    (2, 33, "NbC"),
    (2, 34, "NbH"),
    (2, 35, "Si P : \ntotalisation des salaires ligne à ligne ex: (ligne1 =salaire1, ligne2=salaire 1+ salaire2, ligne 3=salaire1+2+3.....)"),
    (2, 36, "Si I : \ntotalisation des salaires ligne à ligne ex: (ligne1 =salaire1, ligne2=salaire 1+ salaire2, ligne 3=salaire1+2+3.....)"),
];

/// Les cellules fusionnees du modele : (premiere ligne, premiere colonne, derniere ligne, derniere colonne), a partir de 0.
const FUSIONS: [(u32, u16, u32, u16); 7] = [(0, 0, 1, 25), (0, 26, 0, 29), (0, 30, 0, 34), (1, 26, 1, 29), (1, 30, 2, 30), (1, 31, 1, 32), (1, 33, 1, 34)];

/// La liste nominative du CASC-SVP, une ligne par emploi, a partir de la 4e ligne.
///
/// Le modele officiel (`donnees/modeles/casc-svp-liste-nominative.xlsx`) ne s'ouvre pas : le classeur est RECREE par
/// `documents::classeur` avec les memes colonnes (37), les memes trois lignes de consignes (memes textes, memes cellules
/// fusionnees) et les memes valeurs ; la mise en forme est plus simple (gras, retour a la ligne, volet fige sous la
/// 3e ligne). Les consignes de la 4e ligne du modele (« NE PAS REMPLIR »...) n'y sont pas : elles sont
/// effacees avant d'ecrire.
/// Les maximums des colonnes de la liste nominative, et ce qu'elles portent : les consignes du modele du CASC-SVP
/// (`atelier/reference/sources/casc-svp-modele-liste-nominative.csv`, ligne 11 : « 30 Car alpha maximum »
/// pour le nom de naissance, « 32 Car alpha maximum » pour le prenom, « 32 caractères maximum » pour l'adresse,
/// « 5 caractères maximum » pour le code postal, « 26 caractères maximum » pour la commune, « 34 caractères maximum »
/// pour l'emploi).
const MAX_NOM: (&str, usize) = ("nom", 30);
const MAX_PRENOM: (&str, usize) = ("prénom", 32);
const MAX_ADRESSE: (&str, usize) = ("adresse", 32);
const MAX_CODE_POSTAL: (&str, usize) = ("code postal", 5);
const MAX_COMMUNE: (&str, usize) = ("commune", 26);
const MAX_EMPLOI: (&str, usize) = ("emploi", 34);

fn liste_nominative_xlsx(ctx: &Contexte, r: &mut Regularisation, dossier: &Path, fichiers: &mut Vec<std::path::PathBuf>) -> R<()> {
    use documents::classeur::{Feuille, Style, Vertical};
    let mut ws = Feuille::nouvelle("Feuil1");
    let titre = Style { gras: true, retour_a_la_ligne: true, centre: true, vertical: Some(Vertical::Centre), ..Style::CLASSEUR };
    let entete = Style { retour_a_la_ligne: true, vertical: Some(Vertical::Haut), ..Style::CLASSEUR };
    for (a, b, c, d) in FUSIONS {
        ws.fusionner(a, b, c, d, &titre);
    }
    for (ligne, col, texte) in ENTETES {
        ws.texte(ligne, col, texte, if ligne < 2 { &titre } else { &entete });
    }
    ws.hauteur(2, 132);
    ws.figer(3);
    ws.largeur(0, 36, 14);
    let simple = Style::CLASSEUR;
    let siret = ctx.entreprise.siret.ecrite("siret").map_err(|e| e.reclasser(Classe::Refuse))?.to_string();
    let aujourdhui = droit::contexte::jour_reel().jj_mm_aaaa();
    let mut cumul: Vec<((Annee, Nir), Montant)> = Vec::new();
    // Le texte s'ecrit entier, jamais coupe : celui qui depasse le maximum du modele se dit, ligne et colonne.
    let mut depasses: Vec<String> = Vec::new();
    let mut entier = |ligne: u32, (quoi, max): (&str, usize), t: String| -> String {
        let n = t.chars().count();
        if n > max {
            depasses.push(format!("ligne {}, {} : {} caractères, {} de plus que les {} du modèle", ligne + 1, quoi, n, n - max, max));
        }
        t
    };
    for (n, x) in r.emplois.iter().enumerate() {
        let ligne = 3 + n as u32;
        let (i, c) = (&x.individu, &x.contrat);
        let artiste = c.statut == droit::fiche::Statut::Artiste;
        let cle = (x.annee, i.nir.clone());
        let total = match cumul.iter_mut().find(|(k, _)| *k == cle) {
            Some((_, t)) => {
                *t += x.brut;
                *t
            }
            None => {
                cumul.push((cle, x.brut));
                x.brut
            }
        };
        let cle_nir = cellule(&droit::fiche::CleNir::du_nir(&i.nir));
        // Une case vide du fichier quand la date n'est pas dite.
        let jj = |j: &Option<Jour>| -> String { j.iter().map(Jour::jj_mm_aaaa).collect() };
        let textes: Vec<(u16, String)> = vec![
            (0, "CASC-SVP".to_string()),
            (1, siret.clone()),
            (4, x.annee.to_string()),
            (5, aujourdhui.clone()),
            (6, entier(ligne, MAX_NOM, cellule(&i.nom))),
            (8, entier(ligne, MAX_PRENOM, cellule(&i.prenom))),
            (9, jj(&i.date_naissance)),
            (10, format!("{}{}", i.nir, cle_nir)),
            (13, entier(ligne, MAX_ADRESSE, cellule(&i.adresse))),
            (15, entier(ligne, MAX_CODE_POSTAL, cellule(&i.code_postal))),
            (16, entier(ligne, MAX_COMMUNE, cellule(&i.commune))),
            (19, cellule(&i.email)),
            (21, entier(ligne, MAX_EMPLOI, cellule(&c.emploi))),
            (22, jj(&x.periode.map(|o| o.debut()))),
            (23, jj(&x.periode.and_then(|o| o.fin()))),
            (25, "I".to_string()),
            (30, if artiste { "A" } else { "T" }.to_string()),
        ];
        for (col, t) in textes {
            ws.texte(ligne, col, t, &simple);
        }
        // Les nombres de la liste nominative, ecrits dans ses cases : sa porte. Les salaires, a l'euro le plus proche.
        ws.nombre(ligne, 24, x.brut.a_l_euro(), &simple);
        ws.nombre(ligne, 36, total.a_l_euro(), &simple);
        let a = &x.activite;
        // Un artiste dit ses cachets (colonne 33) et ses heures (34), un technicien ses jours (31) et ses heures (32).
        match (artiste, a.cachets, a.jours) {
            (true, Some(c), _) => ws.nombre(ligne, 33, c, &simple),
            (false, _, Some(j)) => ws.nombre(ligne, 31, j, &simple),
            (true, None, _) | (false, _, None) => {}
        }
        if let Some(h) = a.heures {
            ws.nombre(ligne, if artiste { 34 } else { 32 }, h, &simple);
        }
    }
    if !depasses.is_empty() {
        r.avertissements.push(format!("Liste nominative : écrit en entier, plus long que le modèle du CASC-SVP ({}).", depasses.join(" ; ")));
    }
    let sans: Vec<&EmploiReg> = r.emplois.iter().filter(|x| x.activite.est_vide()).collect();
    if !sans.is_empty() {
        let mut mois: Vec<Mois> = sans.iter().map(|x| x.mois).collect();
        mois.sort();
        mois.dedup();
        let mois: Vec<String> = mois.iter().map(Mois::to_string).collect();
        r.avertissements.push(format!(
            "Liste nominative : {} emploi(s) sans jours, cachets ni heures dans leur DSN ({}) -- colonnes laissees vides.",
            sans.len(),
            mois.join(", ")
        ));
    }
    let chemin = dossier.join("CASC-SVP_liste-nominative.xlsx");
    std::fs::write(&chemin, ws.octets()?).map_err(|e| socle::echec::du_disque(&chemin, &e).reclasser(Classe::Disque))?;
    fichiers.push(chemin);
    Ok(())
}

pub use declarations::casc::RegularisationNotee;

/// La route `casc-regularise` note qu'une annee est regularisee. S'ecrit
/// APRES le virement, jamais avant.
pub fn marquer_regularise(ctx: &Contexte, annee: Annee, vire_le: Jour, montant: Montant, qui: &str, aujourd_hui: Jour) -> R<RegularisationNotee> {
    let r = regularisation(ctx, &Demande { mois: None, annee: Some(annee), toutes_les_annees: false, jusqu_a: None, annees: Vec::new(), vire_le: Vec::new(), boite_liee: false })?.dossier()?;
    let p = fichier_suivi(ctx);
    let mut d = match declarations::casc::lue(ctx).map_err(|m| m.reclasser(Classe::Illisible))? {
        Some(d) => d,
        None => declarations::casc::CascSvp {
            quoi: "Les regularisations CASC-SVP faites hors DSN. Un mois ici ne revient plus dans un dossier de regularisation.".into(),
            regularisations: vec![],
        },
    };
    let entree = RegularisationNotee {
        annee,
        mois: r.mois.iter().map(|m| m.mois).collect(),
        montant,
        calcule: r.total,
        vire_le,
        note_le: aujourd_hui,
        qui: qui.to_string(),
    };
    d.regularisations.push(entree.clone());
    ecrire_json(&p, &d, 2).map_err(|m| m.reclasser(Classe::Disque))?;
    Ok(entree)
}
