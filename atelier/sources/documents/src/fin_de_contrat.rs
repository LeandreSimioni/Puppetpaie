//! Le certificat de travail et le reçu pour solde de tout compte d'un contrat qui se termine : deux papiers que la
//! compagnie remet au salarié, rien ne part nulle part. Ils se fabriquent depuis la paie validée de l'engagement.
//!
//! Le certificat se délivre à l'expiration du contrat (L1234-19) et porte les mentions de D1234-6, et le maintien des
//! garanties collectives de prévoyance et de santé que CSS L. 911-8, 6° lui fait signaler (`mention`, `engagement::portabilite`) ; le reçu
//! fait l'inventaire des sommes versées (L1234-20), ligne à ligne, et s'établit en double exemplaire (D1234-7).

use droit::fiche::Lue as _;
use std::path::{Path, PathBuf};

use fabrique_pdf::forme::{Document, Style};

use crate::bulletin::{fmt_eur, identite_de};
use crate::marqueurs::{ModeleLu, Valeurs};
use droit::marqueurs::Marqueur;
use droit::contexte::Contexte;
use engagement::calcul::Paie;
use socle::date::Jour;
use socle::echec::{erreur, Classe, R};
use socle::num::eur2;

/// Ce qu'il faut savoir du reçu et du certificat en les remettant.
pub const AVIS: &[&str] = &[
    "Le reçu pour solde de tout compte s'établit en double exemplaire (D1234-7) : un pour le salarié, un pour la compagnie. Il se signe de sa main, avec la mention « pour solde de tout compte ».",
    "Il peut être dénoncé par lettre recommandée dans les six mois (L1234-20 et D1234-8) ; passé ce délai, il est libératoire pour les seules sommes qui y sont inscrites.",
    "Le certificat de travail contient les mentions de D1234-6 (dates d'entrée et de sortie, emplois occupés et leurs périodes) et, pour un salarié couvert par une prévoyance ou une complémentaire santé collective, le maintien de ces garanties (code de la sécurité sociale, L911-8, 6°), sauf faute lourde.",
    "L'organisme assureur apprend la fin du contrat par la DSN (sa fin dans la mensuelle, et le signalement de fin de contrat quand il part) : rien d'autre à lui envoyer.",
];

/// LE MAINTIEN DES GARANTIES QUE LE CERTIFICAT SIGNALE (CSS L. 911-8,
/// `atelier/reference/sources/code-de-la-securite-sociale/xml/L911-8.xml`, 6° : « L'employeur signale le maintien de ces
/// garanties dans le certificat de travail » ; les accords santé des permanents de la 1285 et de la 3090 le redisent :
/// `kali-1285-accord-frais-de-sante-20260130-20261011.md`, l. 201, `kali-3090-accord-sante-permanents-20240719-20261011.md`,
/// l. 153), en mots, depuis ce que l'engagement en dit (`engagement::portabilite`). La garantie santé des intermittents
/// est une adhésion individuelle, hors de L. 911-8 : seule leur prévoyance s'y lit.
fn mention(m: &engagement::portabilite::Portabilite, nom: &str, fin: &Jour) -> String {
    let garanties = match (m.prevoyance, m.sante) {
        (true, true) => "de prévoyance (décès, incapacité de travail, invalidité) et de complémentaire santé",
        (true, false) => "de prévoyance (décès, incapacité de travail, invalidité)",
        _ => "de complémentaire santé",
    };
    let contrats = if m.consecutifs { "de ses derniers contrats consécutifs dans l'entreprise, depuis le " } else { "de son dernier contrat de travail, depuis le " };
    format!(
        "Maintien des garanties (article L. 911-8 du code de la sécurité sociale) : si la cessation de son contrat de \
travail lui ouvre droit à une prise en charge par l'assurance chômage, {} bénéficie du maintien à titre gratuit des \
garanties collectives {} en vigueur dans l'entreprise, à compter du {} et pendant la durée de son indemnisation, dans \
la limite de {} mois (la durée {}{}, arrondie au mois supérieur, douze mois au plus). Il en justifie auprès de \
l'organisme assureur, à l'ouverture et au cours de la période de maintien.",
        nom,
        garanties,
        fin.jj_mm_aaaa(),
        m.mois,
        contrats,
        m.depuis.jj_mm_aaaa()
    )
}

/// Écrit les deux papiers de la paie `p` (dont l'engagement finit), établis le `date`, dans `sortie` : le certificat
/// puis le reçu ; les deux ou aucun (un reçu qui ne s'écrit pas emporte le certificat déjà posé).
pub fn ecrire(ctx: &Contexte, p: &Paie, date: Jour, sortie: &Path) -> R<[PathBuf; 2]> {
    let e = &p.engagement;
    let Some(numero) = e.numero.as_ref() else {
        return Err(erreur(Classe::Illisible, "Certificat de travail : le contrat n'a pas de numéro, il n'est pas validé. Le valider d'abord."));
    };
    let Some(fin) = e.fin else {
        return Err(erreur(
            Classe::Illisible,
            format!("Certificat de travail : le contrat {} n'a pas de date de fin. Il se délivre à l'expiration du contrat (L1234-19).", numero),
        ));
    };
    let cle = p.salarie.id.to_string();
    let ent = ctx.entreprise_et_gabarits();
    let poste = ctx.gabarits();
    let certificat = sortie.join(format!("CertificatTravail_{}_{}.pdf", cle, fin.aaaammjj()));
    let recu = sortie.join(format!("RecuSoldeToutCompte_{}_{}.pdf", cle, fin.aaaammjj()));
    let mut valeurs_cert = marqueurs_communs(&date, p, numero, fin, &ent)?;
    let mut valeurs = valeurs_cert.clone();
    let nom = format!("{} {}", p.salarie.prenom.ecrite("prenom").map_err(|m| m.reclasser(Classe::Manque))?, p.salarie.nom.ecrite("nom").map_err(|m| m.reclasser(Classe::Manque))?);
    let maintien = engagement::portabilite::de(ctx, p, numero, &fin)?;
    // LE MODELE DU CERTIFICAT : celui de la compagnie (ses `gabarits/`, une copie qu'elle a pu modifier) ou celui livre.
    // Quand un maintien est du, un modele qui ne cite pas sa mention la tairait sans le dire : il se refuse.
    let chemin_cert = socle::fichiers::modele_de("certificat-travail.modele", Some(&ent.gabarits), &poste);
    let lu_cert = ModeleLu::du_fichier(&chemin_cert)?;
    if maintien.is_some() && !lu_cert.cite(Marqueur::CertificatPortabilite) {
        return Err(erreur(
            Classe::ModeleIncomplet,
            format!(
                "Le modèle du certificat de travail ({}) ne cite pas {} : le maintien des garanties de {} (code de la \
sécurité sociale, L911-8, 6°) se tairait. Ajouter {} au modèle, sur sa propre ligne avant les signatures (le modèle livré le \
fait). Rien n'est écrit.",
                socle::fichiers::NomDeFichier::du(&chemin_cert)?,
                Marqueur::CertificatPortabilite,
                nom,
                Marqueur::CertificatPortabilite
            ),
        ));
    }
    // Sans maintien du (aucune garantie collective, ou faute lourde), L911-8, 6° n'a rien a faire signaler : le
    // paragraphe du modele reste vide.
    let dite: String = maintien.iter().map(|m| mention(m, &nom, &fin)).collect();
    valeurs_cert.insert(Marqueur::CertificatPortabilite, dite);
    valeurs.insert(Marqueur::SoldeInventaire, inventaire(p));
    valeurs.insert(Marqueur::SoldeNet, fmt_eur(eur2(p.net_a_payer)));
    let lu_recu = ModeleLu::du_fichier(&socle::fichiers::modele_de("recu-solde-de-tout-compte.modele", Some(&ent.gabarits), &poste))?;
    ecrire_papier(&lu_cert, valeurs_cert, &certificat, &ent, true)?;
    if let Err(e) = ecrire_papier(&lu_recu, valeurs, &recu, &ent, false) {
        return Err(socle::fichiers::effacer_apres(e, &certificat));
    }
    Ok([certificat, recu])
}

/// Les mentions communes du certificat de travail et du reçu, du contrat `numero` qui finit le `fin`.
fn marqueurs_communs(date: &Jour, p: &Paie, numero: &engagement::numeros::NumeroContrat, fin: Jour, ent: &droit::fiche::ChampsEntreprise) -> R<Valeurs> {
    let s = &p.salarie;
    let e = &p.engagement;
    fn ecrite<T: AsRef<str>>(c: &Option<T>, k: &str) -> R<String> {
        c.ecrite(k).map(str::to_string).map_err(|m| m.reclasser(Classe::Manque))
    }
    let sig = ent.signataire.exigee("signataire").map_err(|m| m.reclasser(Classe::Manque))?;
    let aujourdhui = date.jj_mm_aaaa();
    let mut m = Valeurs::new();
    let mut poser = |k: Marqueur, v: String| {
        m.insert(k, v);
    };
    poser(Marqueur::EmployeurRaisonSociale, ecrite(&ent.raison_sociale, "raison_sociale")?);
    poser(Marqueur::EmployeurAdresse, droit::fiche::avec_complement(ecrite(&ent.adresse.en_texte(), "adresse")?, ent.complement_adresse.remplie()));
    poser(Marqueur::EmployeurCodePostal, ecrite(&ent.code_postal, "code_postal")?);
    poser(Marqueur::EmployeurCommune, ecrite(&ent.commune, "commune")?);
    poser(Marqueur::EmployeurSiret, ecrite(&ent.siret, "siret")?);
    poser(Marqueur::EmployeurNaf, ecrite(&ent.naf, "naf")?);
    poser(Marqueur::EmployeurSignataire, format!("{} {}", ecrite(&sig.prenom, "prenom")?, ecrite(&sig.nom, "nom")?));
    poser(Marqueur::EmployeurSignataireFonction, ecrite(&sig.fonction, "fonction")?);
    poser(Marqueur::SalarieNomComplet, format!("{} {}", ecrite(&s.prenom, "prenom")?, ecrite(&s.nom, "nom")?));
    poser(Marqueur::SalarieDateNaissance, s.date_naissance.exigee("date_naissance").map_err(|m| m.reclasser(Classe::Manque))?.jj_mm_aaaa());
    poser(Marqueur::SalarieLieuNaissance, ecrite(&s.lieu_naissance, "lieu_naissance")?);
    poser(Marqueur::SalarieAdresse, ecrite(&s.adresse, "adresse")?);
    poser(Marqueur::SalarieCodePostal, ecrite(&s.code_postal, "code_postal")?);
    poser(Marqueur::SalarieCommune, ecrite(&s.commune, "commune")?);
    poser(Marqueur::ContratNumero, numero.to_string());
    poser(Marqueur::ContratDebut, e.debut.jj_mm_aaaa());
    poser(Marqueur::ContratFin, fin.jj_mm_aaaa());
    poser(Marqueur::ContratEmploi, ecrite(&s.emploi, "emploi")?);
    poser(Marqueur::ContratNature, e.nat().map_err(|m| m.reclasser(Classe::Illisible))?.libelle.clone());
    poser(Marqueur::CertificatDate, aujourdhui.clone());
    poser(Marqueur::RecuDate, aujourdhui);
    Ok(m)
}

/// Le détail des sommes, ligne à ligne -- pas un total.
fn inventaire(p: &Paie) -> String {
    let mut lignes = Vec::new();
    for l in &p.remuneration {
        lignes.push(format!("{} : {}", l.libelle, fmt_eur(eur2(l.montant))));
    }
    for l in &p.lignes {
        let m = eur2(l.montant_sal);
        if m != socle::num::Decimal::ZERO {
            lignes.push(format!("{} : -{}", l.libelle, fmt_eur(m)));
        }
    }
    lignes.join("\n")
}

/// Le papier du modèle lu `lu`, par la fabrique (style « attestation »). `signe` : le certificat s'établit signé de
/// l'employeur.
fn ecrire_papier(lu: &ModeleLu, valeurs: Valeurs, chemin: &Path, ent: &droit::contexte::EntrepriseDuDocument, signe: bool) -> R<()> {
    let mut un = crate::contrat::document_modele(lu, &valeurs);
    if signe {
        un.signature_employeur = identite_de(&ent.gabarits, "signature").as_deref().map(crate::bulletin::chemin_net);
    }
    let d = crate::contrat::enveloppe(Style::Attestation, ent, vec![Document::Modele(un)]);
    crate::fabrique::ecrire_pdf(&d, chemin)?;
    Ok(())
}
