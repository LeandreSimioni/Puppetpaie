//! L'ECRAN DES ORGANISMES ET DES CONTRATS DE LA DSN (Configuration › Organismes et contrats) : ses deux routes, qui
//! lisent et ecrivent par `compagnie::organismes`. La lecture rend l'ecran tel qu'on le saisit (`ecran::EcranRendu`) :
//! un contrat par case, ses parties que la compagnie saisit, et le geste qui l'enregistre.

use serde::{Deserialize, Serialize};

use socle::date::Jour;
use socle::echec::R;
use socle::mois::Mois;
use compagnie::organismes::{enregistrer, lire, ChargeEnregistrer, Lus};
use droit::organismes::{CleDeContrat, IdentifiantDOrganisme};
use crate::enveloppe::{en_json, Charge, Dite, Phrase, Porte, Rendu};
use crate::ecran::{EcranRendu, EnregistrerLEcran};
use crate::fiches::{CaseDeLaFiche, HistoireDUneCase, PartieDeLaFiche};
use crate::temps::TempsDeLaCase;

use crate::catalogue::{Acte, Affiche, Champ, Forme, Fiche, Refus, ILLISIBLE, OPERATION};
use crate::catalogue::Route;
use crate::catalogue::{Ecran, Place};

/// Le champ de la charge d'`organismes-enregistrer` ou vont les cases de l'ecran, un contrat par cle.
const CONTRATS: Champ = Champ { nom: "contrats", libelle: Affiche::new("Contrats"), forme: Forme::Objet, sens: "{clé du contrat: {libelle, organisme}}, ce qui est saisi ; un contrat nul ne dit rien", avance: false };

/// Les parties d'un contrat que la compagnie saisit, et le bouton qui les enregistre.
const ORGANISME: Affiche = Affiche::new("Organisme");
const REFERENCE: Affiche = Affiche::new("Référence du contrat");
const ENREGISTRER: Affiche = Affiche::new("Enregistrer");

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::OrganismesContrats,
        geste: "Dire les contrats de la DSN (prévoyance, santé…) d'un mois, et ce qui manque, tels qu'on les saisit.",
        charge: &[Champ { nom: "mois", libelle: Affiche::new("Mois"), forme: Forme::Mois, sens: "le mois regardé, AAAA-MM ; le mois en cours sinon", avance: false }],
        exemple: r#"{"mois":"2026-11"}"#,
        rend: "mois, encarts, cases (un contrat par case, sa clé : ses parties à saisir, organisme et libelle ; dessous, sa période, ses bénéficiaires et ce que les textes en donnent ; ses temps), historique (les valeurs datées de chaque contrat saisi), enregistrer (la route, la charge, le champ où vont les cases : chacune sous sa clé)",
        refus: &[OPERATION, ILLISIBLE],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Lit,
        phrase: 0,
        ecran: Ecran::Rendu(Place::Organismes),
    },
    Fiche {
        route: Route::OrganismesEnregistrer,
        geste: "Écrire, à partir d'un mois, les contrats de la DSN que la compagnie dit.",
        charge: &[
            CONTRATS,
            Champ { nom: "mois", libelle: Affiche::new("À partir de"), forme: Forme::Mois, sens: "le mois à partir duquel ils valent, AAAA-MM ; le mois en cours sinon", avance: false },
        ],
        exemple: r#"{"mois":"2026-11","contrats":{"prevoyance_permanents":{"libelle":"PREVOYANCE","organisme":"P0983"},"sante_permanents":{"libelle":"SANTE","organisme":"P0983"}}}"#,
        rend: "ce que organismes-contrats rend",
        refus: &[Refus { classe: socle::echec::Classe::Refuse, corriger: "la référence du contrat que la phrase nomme ; rien n'est écrit" }, ILLISIBLE],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: Ecran::Geste(Place::Organismes),
    },
];

/// La charge de `organismes-contrats` : le mois regarde (le mois en cours sans lui) ; celle que l'ecran rend pour
/// l'enregistrer, le mois qu'il regarde.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeOrganismes {
    #[serde(skip_serializing_if = "Option::is_none")]
    mois: Option<Mois>,
}

/// CE QUE RENDENT `organismes-contrats` ET `organismes-enregistrer` : la phrase, le mois regarde, et l'ecran tel qu'on le
/// saisit.
#[derive(Debug, Serialize)]
pub(crate) struct OrganismesDits {
    phrase: Phrase,
    mois: Mois,
    #[serde(flatten)]
    ecran: EcranRendu<ChargeOrganismes, CleDeContrat>,
}

/// Ce que la compagnie a saisi d'un contrat, a une date : sa reference et son organisme (vide : non saisi, ou donne par
/// les textes).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct ContratSaisi {
    libelle: Option<String>,
    organisme: Option<IdentifiantDOrganisme>,
}

impl ContratSaisi {
    /// En mots, par les libelles des parties de l'ecran ; vide, un tiret.
    fn en_mots(v: Option<ContratSaisi>) -> String {
        let Some(v) = v else { return "—".to_string() };
        let mut dits = Vec::new();
        if let Some(l) = &v.libelle {
            dits.push(format!("{} : {}", REFERENCE.texte(), l));
        }
        if let Some(o) = &v.organisme {
            dits.push(format!("{} : {}", ORGANISME.texte(), o.as_str()));
        }
        if dits.is_empty() {
            return "—".to_string();
        }
        dits.join(" · ")
    }
}

/// LES TEMPS DE CHAQUE CONTRAT SAISI a la date du jour (`crate::temps::d_une_case`), par sa cle : la case datee
/// `adhesions_prevoyance` de `config/organismes.json`, lue contrat par contrat (sa valeur a chaque version, une fois tant
/// qu'elle ne change pas ; absent d'une version, il n'y est pas saisi).
fn temps_des_contrats(ctx: &droit::contexte::Contexte) -> R<Vec<TempsDeLaCase<ContratSaisi>>> {
    let fichier = droit::organismes::fichier_organismes(ctx)?;
    let mut par_cle: Vec<(CleDeContrat, Vec<droit::fiche::Datee<ContratSaisi>>)> = Vec::new();
    // Aucune adhesion saisie : aucun contrat.
    let versions = fichier.adhesions_prevoyance.as_deref().map(droit::fiche::valeurs_datees);
    for version in versions.into_iter().flatten() {
        for a in version.valeur.iter().flatten() {
            if !par_cle.iter().any(|(c, _)| *c == a.cle) {
                par_cle.push((a.cle.clone(), Vec::new()));
            }
        }
        for (cle, datees) in par_cle.iter_mut() {
            let valeur = version.valeur.iter().flatten().find(|a| a.cle == *cle).map(|a| ContratSaisi { libelle: Some(a.libelle.as_str().trim().to_string()), organisme: Some(a.delegataire.clone()) });
            if datees.last().is_some_and(|d| d.valeur == valeur) {
                continue;
            }
            datees.push(droit::fiche::Datee::dite(version.depuis, valeur, version.note.clone()));
        }
    }
    let mut out = Vec::new();
    for (cle, datees) in par_cle {
        out.extend(crate::temps::d_une_case(crate::temps::CaseDatee::Contrat(cle), &datees, &ctx.aujourdhui, |a, b| a == b)?);
    }
    Ok(out)
}

impl Dite for OrganismesDits {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

/// Une route de ce module.
pub fn route(porte: &Porte, charge: &Charge) -> R<Rendu> {
    let ctx = porte.ctx;
    match porte.route {
        Route::OrganismesContrats => {
            let c: ChargeOrganismes = charge.lire()?;
            let dits: OrganismesDits = dire(lire(ctx, c.mois)?, false, temps_des_contrats(ctx)?)?;
            en_json(dits)
        }
        // Les temps se lisent apres l'ecriture : ceux du contrat qui vient d'etre enregistre.
        Route::OrganismesEnregistrer => {
            let c: ChargeEnregistrer = charge.lire()?;
            let dits: OrganismesDits = dire(enregistrer(ctx, c)?, true, temps_des_contrats(ctx)?)?;
            en_json(dits)
        }
        autre => Err(crate::enveloppe::route_inconnue(autre.nom())),
    }
}

/// L'ecran des contrats tel qu'on le saisit : sa phrase (ce qui reste a dire a la compagnie, sinon combien de contrats la
/// DSN declare), d'ou viennent les valeurs, les contrats de quel mois, ce qui manque ; un contrat par case, ses parties
/// que la compagnie saisit (ce que les textes donnent se dit dessous et ne se saisit pas), sa periode, ses beneficiaires,
/// ses temps ; l'historique de chaque contrat saisi ; et le geste qui enregistre, a partir du mois regarde.
fn dire(lu: Lus, enregistre: bool, mut temps: Vec<TempsDeLaCase<ContratSaisi>>) -> R<OrganismesDits> {
    let n = lu.lignes.len();
    let reste = if lu.a_saisir { "il en reste à saisir" } else { "rien à saisir" };
    let phrase = if enregistre { socle::phrase!("Enregistré. Contrats de la DSN : {} ; {}.", n, reste) } else { socle::phrase!("Contrats de la DSN : {} ; {}.", n, reste) };
    let mois = lu.mois;
    let mut encarts = vec![crate::Encart::de(socle::ecran::EtatDEcran::Information, "", "Ce que la DSN déclare de vos adhésions de prévoyance et de santé. La référence du contrat et le code de l'organisme se lisent sur le certificat d'adhésion et sur la fiche de paramétrage DSN de l'organisme. Le reste (organismes payés, Urssaf, taux AT/MP, compte France Travail) vient de la loi, de la convention et de la fiche Entreprise.")];
    encarts.push(crate::Encart::de(socle::ecran::EtatDEcran::Information, "", format!("Les contrats en vigueur en {}.", mois.libelle(socle::mois::Graphie::Accentuee))));
    if lu.a_saisir {
        encarts.push(crate::Encart::de(socle::ecran::EtatDEcran::AFaire, "", "Une référence de contrat manque : la DSN refuse les salariés que ce contrat couvre tant qu'elle n'est pas enregistrée."));
    }
    let en_clair = |j: &Option<Jour>| -> Option<String> { j.map(|j| j.jj_mm_aaaa()) };
    let mut cases = Vec::new();
    let mut historique = Vec::new();
    for ligne in lu.lignes {
        let mut dit = Vec::new();
        match (en_clair(&ligne.depuis), en_clair(&ligne.jusqu_a)) {
            (Some(de), Some(a)) => dit.push(format!("Du {} au {}.", de, a)),
            (Some(de), None) => dit.push(format!("Depuis le {}.", de)),
            (None, Some(a)) => dit.push(format!("Jusqu'au {}.", a)),
            (None, None) => {}
        }
        if let Some(note) = &ligne.note {
            dit.push(note.clone());
        }
        dit.push(format!("Bénéficiaires : {}.", ligne.beneficiaires));
        // Ce que les textes donnent se dit sous le contrat, et ne se saisit pas ; le reste est une partie a saisir.
        let mut parties = Vec::new();
        let donne = |t: Option<&str>| t.unwrap_or("—").to_string();
        if ligne.organisme_fixe {
            dit.push(format!("{} : {}, donné par les textes.", ORGANISME.texte(), donne(ligne.organisme.as_ref().map(IdentifiantDOrganisme::as_str))));
        } else {
            parties.push(PartieDeLaFiche::texte("organisme", ORGANISME, "De 5 à 9 caractères, sans espace : sur le certificat d'adhésion et la fiche de paramétrage DSN.".to_string(), Some("P0983".to_string())));
        }
        if ligne.libelle_fixe {
            dit.push(format!("{} : {}, donnée par les textes.", REFERENCE.texte(), donne(ligne.libelle.as_deref())));
        } else {
            parties.push(PartieDeLaFiche::texte("libelle", REFERENCE, "De 1 à 30 caractères : sur la fiche de paramétrage DSN de l'organisme.".to_string(), None));
        }
        // La valeur de la case : ce qui se saisit, seul ; ce que les textes donnent ne repart pas.
        let saisi = ContratSaisi {
            libelle: if ligne.libelle_fixe { None } else { ligne.libelle.clone() },
            organisme: if ligne.organisme_fixe { None } else { ligne.organisme.clone() },
        };
        let valeur = socle::ecriture::ValeurDeCase::de(&saisi);
        let siens = temps.iter().position(|t| matches!(&t.champ, crate::temps::CaseDatee::Contrat(c) if *c == ligne.cle)).map(|i| temps.swap_remove(i));
        let siens = siens.map(|t| t.dits(&|v| Some(ContratSaisi::en_mots(v))));
        if let Some(t) = &siens {
            historique.push(HistoireDUneCase::de(ligne.quoi.clone(), t.clone()));
        }
        cases.push(CaseDeLaFiche::de_parties(ligne.cle, ligne.quoi, valeur, parties, dit, siens));
    }
    let enregistrer = Some(EnregistrerLEcran::de(Route::OrganismesEnregistrer, ChargeOrganismes { mois: Some(mois) }, &CONTRATS, ENREGISTRER));
    Ok(OrganismesDits { phrase, mois, ecran: EcranRendu { encarts, rangs: Vec::new(), cases, historique, enregistrer } })
}
