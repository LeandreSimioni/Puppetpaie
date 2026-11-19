//! Les etats de comptabilite de `exporter` : les ecritures de paie dans les formats que les logiciels de
//! comptabilite importent (fec, ximport, oxygene, sage, quadra, ebp, pennylane), le grand livre, la balance, la
//! provision pour conges payes des permanents, la taxe sur les salaires.
//!
//! Les ecritures sont celles de `ecritures_comptables::ecritures_du_mois` (le meme plan, le meme refus d'un compte
//! devine) ; chaque mois fait une piece (`PAIE-AAAA-MM`) et un numero d'ecriture. Chaque format de logiciel s'ecrit
//! d'apres sa notice deposee (`SOURCE_*`) ; ceux que la notice ne dit pas en UTF-8 sont ecrits en cp1252 avec
//! remplacement (`cp1252`). Une zone ne se coupe jamais : un libelle ou un intitule trop long prend la valeur courte que
//! la compagnie a validee (`libelles_courts`), sinon l'export se refuse et dit combien en raccourcir.

use droit::fiche::Lue as _;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use droit::contexte::Contexte;
use droit::fiche::CodeAnalytique;
use socle::echec::{erreur, Classe};
use socle::fichiers::NomDeFichier;
use socle::mois::Mois;
use crate::etats_paie::{ecrire_fini, finir, lignes_livre, charger_mois, Demande, NomDExport, Periode};
use crate::ecritures_comptables::{ecritures_du_mois, Ecriture, NumeroDeCompte};
use crate::libelles_courts::{Courts, Depasse, FormatBorne, Quoi};
use crate::plan_comptable::{EtatDExport, FormatDExport, Logiciel, LogicielComptable, Plan};
use socle::texte::euros;
use socle::grandeurs::ecrit::{sans_zero, texte};
use socle::grandeurs::{Jours, Montant, Taux};
use crate::tableaux_export::{
    ecrire as ecrire_tableau, format_possible, jj, libelle_mois, nom_compagnie, refus, totaliser, totaliser_general, colonne as C, Ligne, Section, Tableau,
    FORMATS_TABLEAU, R, TOTAL_GENERAL,
};

// ------------------------------------------------------------------ les tables

/// `FORMATS_ECRITURES` : les six formats de la machinerie, puis les sept des logiciels.
pub const FORMATS_ECRITURES: [FormatDExport; 13] = [
    FormatDExport::Csv,
    FormatDExport::Tsv,
    FormatDExport::Xlsx,
    FormatDExport::Pdf,
    FormatDExport::Html,
    FormatDExport::Json,
    FormatDExport::Fec,
    FormatDExport::Ximport,
    FormatDExport::Oxygene,
    FormatDExport::Sage,
    FormatDExport::Quadra,
    FormatDExport::Ebp,
    FormatDExport::Pennylane,
];

const SOURCES: &str = "plan comptable : plan-comptable-base.json (plan comptable général, classes 64, 42, 43, 44), remplacé clé par clé par config/plan-comptable.json";
// LES NOTICES, deposees dans `atelier/reference/sources/formats-comptables/` (son LISEZ-MOI) : chaque format s'ecrit
// d'apres la sienne, et chaque source dit le fichier et les lignes (ou l'article) ou se lisent ses zones.
const SOURCE_FEC: &str = "FEC : art. A47 A-1 du Livre des procédures fiscales (formats-comptables/lpf-article-A47-A-1-20261030.txt) : VI, 1°, c, les zones séparées par une tabulation ; VII, 1° et 4°, les dix-huit champs dans l'ordre et leurs noms en première ligne ; XII, l'UTF-8, la virgule décimale, les dates AAAAMMJJ ; BOI-CF-IOR-60-40-20, § 70 (formats-comptables/bofip-BOI-CF-IOR-60-40-20-20170607.html) : un champ « à blanc si non utilisé » reste vide";
const SOURCE_XIMPORT: &str = "XIMPORT : Sage 50 (ex-Ciel), « Comprendre la structure du fichier d'import XImport » (formats-comptables/sage-50-structure-ximport-20261104.txt, l. 31-48) : treize zones à la suite, sans délimiteur, la virgule décimale, les nombres à droite, le reste à gauche. La notice dit « 137 caractères » au plus, ses zones en font 138 : elles sont écrites telles que la table les donne. Le jeu de caractères et la fin de ligne n'y sont pas dits : Windows et CR LF, une inférence";
const SOURCE_OXYGENE: &str = "Oxygène : Memsoft, « Structure des fichiers ASCII utilisés dans le module d'importation de données » (formats-comptables/memsoft-oxygene-structure-fichiers-ascii-20261104.txt) : l. 5-9, aucun format de taille fixe ; l. 19-27, le CSV à point-virgule conseillé (EXCEL2), CR LF ; l. 42-136, l'import à partir d'un seul fichier et ses zones (LIBEC 40, NPIEC 10, NUMCP 13, NUMJL 5, CSECT 20, CODAGE 9) ; l. 353-371, la fiche compte (NUMCP 13, CODCP 15 non obligatoire, LIBCP 50). Ni le séparateur décimal ni le jeu de caractères n'y sont dits : la virgule et Windows, une inférence";
const SOURCE_SAGE: &str = "Sage : Sage 100, « Description du format PNM » (formats-comptables/sage-100-description-format-pnm-20261104.txt, l. 40-43 : la raison sociale en première ligne, 30 caractères, puis une ligne par retour chariot ; l. 86-178 : le format 105 caractères, positions et longueurs ; l. 293-309 : l'exemple, le point décimal, la ligne A d'une section). Le jeu de caractères n'y est pas dit : Windows, une inférence";
const SOURCE_EBP: &str = "EBP : « Format d'importation des écritures en ASCII », version du 01/07/2006 (formats-comptables/ebp-format-importation-ecritures-ascii-20261104.txt, l. 52-115 : les onze zones, la virgule pour séparateur, le guillemet autour du libellé (40) et de la pièce (15), le point décimal, la ventilation analytique ; l. 124-166 : COMPTES.TXT, le compte (15) et l'intitulé (60) parmi dix zones). Le jeu de caractères n'y est pas dit, ni la largeur du poste analytique : Windows, une inférence";
const SOURCE_PENNYLANE: &str = "Pennylane : centre d'aide, « Importer des écritures » (formats-comptables/pennylane-importer-des-ecritures-20261104.txt, l. 85-133 : la date, le numéro de compte et le débit ou le crédit obligatoires, seuls 401, 411, 410, 421, 455 et 467 en lettres, le code journal de cinq lettres au plus, la catégorie et sa famille avec le code analytique ; l. 209-220 : une pièce par numéro et par journal). La notice ne borne pas le libellé ; le séparateur et le jeu de caractères n'y sont pas dits : le point-virgule et l'UTF-8, une inférence";
const SOURCE_QUADRA: &str = "Quadra : Cegid, « Fichier d'entrée ASCII dans QuadraCOMPTA », QC_ASC.doc, mise à jour 2023 (formats-comptables/cegid-quadra-fichier-entree-ascii-20261104.txt, l. 1-34 : l'enregistrement M, positions et longueurs, le libellé de 30 en 117 « blanc si renseigné en 22 sur 20 » ; l. 64-82 : l'enregistrement C). Le jeu de caractères n'y est pas dit : Windows, une inférence";

const ESSAI: &str = " Faire un premier import d'essai sur un dossier de test avant de s'y fier.";

/// `LOGICIELS[logiciel]`, lu dans `exports_tables.json`.
fn logiciel_connu(logiciel: LogicielComptable) -> Option<&'static Logiciel> {
    crate::plan_comptable::tables().logiciels.iter().find(|l| l.logiciel == logiciel)
}

/// Les logiciels connus, dans l'ordre de `LOGICIELS`.
fn logiciels_dits() -> Vec<String> {
    crate::plan_comptable::logiciels_connus().iter().map(|s| s.to_string()).collect()
}

// ------------------------------------------------------------------ petits outils de texte

/// `" ".join(s.split())`.
fn blancs(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}


/// `s.ljust(n)` / `s.rjust(n)`.
fn caler(s: String, n: usize, a_droite: bool) -> String {
    let manque = n.saturating_sub(s.chars().count());
    let blanc = " ".repeat(manque);
    if a_droite {
        format!("{}{}", blanc, s)
    } else {
        format!("{}{}", s, blanc)
    }
}

/// Ce qu'une zone porte, en mots, et ce qui se raccourcit pour qu'elle tienne.
fn ce_que_porte(quoi: &str) -> (&'static str, &'static str) {
    match quoi {
        "journal" | "journal3" => ("le code journal", "Raccourcir le code journal dans le plan comptable de la compagnie"),
        "CODAGE" => ("le poste analytique", "Raccourcir le poste analytique dans le plan comptable de la compagnie"),
        "compte" => ("le compte", "Choisir dans le plan comptable de la compagnie un compte qui tient"),
        "analytique" | "affaire" | "auxiliaire" => ("le code analytique", "Raccourcir le code analytique du spectacle dans Configuration › Spectacles"),
        "piece" | "reference" | "piece10" => ("la pièce", "Ce numéro de pièce est celui du mois : le signaler"),
        _ => ("une zone", "Cette valeur est calculée : le signaler"),
    }
}

/// Le texte d'une zone de `logiciel` qui en prend `n` caracteres : espaces ramenees a une, entier ; plus long, l'export
/// se refuse en disant quoi raccourcir -- rien ne se coupe. Le plan comptable, `config/plan-comptable.json`, va a part.
fn tenir(logiciel: &str, quoi: &str, v: &str, n: usize) -> R<String> {
    let t = blancs(v);
    let c = t.chars().count();
    if c > n {
        let (porte, conseil) = ce_que_porte(quoi);
        /// Ce que le refus porte a part : la zone, et le fichier ou la raccourcir.
        #[derive(Serialize)]
        struct Zone {
            zone: String,
            fichier: socle::fichiers::CheminRelatif,
        }
        return Err(socle::echec::refus_avec(
            format!("{} : « {} » a {} caractères, {} en prend {} au plus dans ce logiciel. {}.", logiciel, t, c, porte, n, conseil),
            socle::echec::APart::forme(Zone { zone: quoi.to_string(), fichier: crate::plan_comptable::PLAN_COMPTABLE }),
        ));
    }
    Ok(t)
}

/// Une zone que rien ne remplit s'ecrit vide.
fn ou_vide(v: Option<String>) -> String {
    v.into_iter().collect()
}

/// Une zone a largeur fixe (`tenir`), cadree a gauche (`g`) ou a droite (`d`).
fn zone(logiciel: &str, quoi: &str, v: &str, n: usize, cote: char) -> R<String> {
    Ok(caler(tenir(logiciel, quoi, v, n)?, n, cote == 'd'))
}

/// `"%.2f" % v` d'un montant a deux decimales.
fn deux(v: Montant) -> String {
    texte::dit(v.au_centime())
}

/// Le meme, la virgule decimale.
fn deux_a_virgule(v: Montant) -> String {
    socle::texte::a_virgule(v.au_centime())
}

/// Non vide, que des chiffres.
fn tout_chiffres(t: &str) -> bool {
    !t.is_empty() && t.chars().all(|c| c.is_ascii_digit())
}

/// `"\r\n".join(lignes) + "\r\n"`, en cp1252 : ce que ce jeu ne porte pas devient « ? » (`errors="replace"`).
fn cp1252(lignes: &[String]) -> Vec<u8> {
    let texte = format!("{}\r\n", lignes.join("\r\n"));
    let mut out = Vec::with_capacity(texte.len());
    for c in texte.chars() {
        let n = c as u32;
        let octet = match n {
            0..=0x7F | 0xA0..=0xFF => n as u8,
            0x20AC => 0x80,
            0x201A => 0x82,
            0x0192 => 0x83,
            0x201E => 0x84,
            0x2026 => 0x85,
            0x2020 => 0x86,
            0x2021 => 0x87,
            0x02C6 => 0x88,
            0x2030 => 0x89,
            0x0160 => 0x8A,
            0x2039 => 0x8B,
            0x0152 => 0x8C,
            0x017D => 0x8E,
            0x2018 => 0x91,
            0x2019 => 0x92,
            0x201C => 0x93,
            0x201D => 0x94,
            0x2022 => 0x95,
            0x2013 => 0x96,
            0x2014 => 0x97,
            0x02DC => 0x98,
            0x2122 => 0x99,
            0x0161 => 0x9A,
            0x203A => 0x9B,
            0x0153 => 0x9C,
            0x017E => 0x9E,
            0x0178 => 0x9F,
            _ => b'?',
        };
        out.push(octet);
    }
    out
}

// ------------------------------------------------------------------ les ecritures

/// Le libelle d'un compte, lu dans le plan : tiers, brut, net, PAS, retenues, puis les charges
/// (le premier libelle de ligne qui le porte) ; aucun pour un compte que le plan ne nomme pas.
fn libelle_compte(plan: &Plan, compte: &NumeroDeCompte) -> Option<String> {
    if let Some(n) = plan.tiers.get(compte) {
        return Some(n.clone());
    }
    for (c, libelle) in [(&plan.brut.compte, &plan.brut.libelle), (&plan.net.compte, &plan.net.libelle), (&plan.pas.compte, &plan.pas.libelle)] {
        if c == compte {
            return Some(libelle.clone());
        }
    }
    if let Some((_, regle)) = plan.retenues.iter().find(|(_, r)| r.compte == *compte) {
        return regle.libelle.clone();
    }
    let regle = plan.lignes.iter().map(|(_, r)| r).chain(plan.regroupements.iter().map(|(_, r)| r)).find(|r| r.charge == *compte)?;
    Some(match compte.as_str() {
        "645100" => "Cotisations Urssaf".to_string(),
        "645300" => "Cotisations retraite et prévoyance".to_string(),
        "645400" => "Cotisations assurance chômage".to_string(),
        "645500" => "Cotisations Congés Spectacles".to_string(),
        "645800" => "Autres charges sociales".to_string(),
        _ => return regle.libelle.clone(),
    })
}

/// Le sous-compte individuel du salarie quand le plan en tient un, le compte general sinon.
fn compte_de(e: &Ecriture) -> &NumeroDeCompte {
    match &e.auxiliaire {
        Some(a) => a,
        None => &e.compte,
    }
}

/// Le libelle d'une ecriture : celui de son compte, complete de ce qui la distingue (le salarie de son sous-compte).
fn libelle_de(plan: &Plan, e: &Ecriture) -> Option<String> {
    let lib = libelle_compte(plan, &e.compte)?;
    Some(match &e.auxiliaire_libelle {
        Some(qui) => format!("{} — {}", lib, qui),
        None => lib,
    })
}

/// Une ligne d'ecriture pour un tableau.
fn ligne_de(e: &Ecriture, plan: &Plan) -> Ligne {
    let mut l = Ligne::nouvelle();
    l.t(C::Journal, e.journal.to_string()).t(C::Date, e.date.jj_mm_aaaa()).t(C::Piece, e.piece()).t(C::Compte, compte_de(e).to_string());
    l.ot(C::LibelleCompte, libelle_de(plan, e)).t(C::Libelle, e.libelle.clone());
    l.om(C::Debit, Some(e.debit).filter(|m| !m.est_nul()));
    l.om(C::Credit, Some(e.credit).filter(|m| !m.est_nul()));
    l.ot(C::Analytique, e.analytique.clone()).m(C::Mois, e.mois);
    l
}

/// LE NUMERO D'UNE ECRITURE COMPTABLE : le rang de son mois (sa piece, `PAIE-AAAA-MM`) parmi les mois de la periode
/// exportee, dit a partir de un et partage par toutes les lignes du mois ; un mois sans ecriture garde son rang. Seules
/// ses zones l'ecrivent : `EcritureNum` du FEC et `mouvement` de XIMPORT.
struct NumeroDEcriture(usize);

impl NumeroDEcriture {
    /// Celui de l'ecriture `e` parmi les mois `periode` de l'export.
    fn de(e: &Ecriture, periode: &[Mois]) -> R<NumeroDEcriture> {
        match periode.iter().position(|m| *m == e.mois) {
            Some(i) => Ok(NumeroDEcriture(i + 1)),
            None => Err(erreur(Classe::DefautDuLogiciel, format!("Défaut du logiciel : une écriture de {} n'est d'aucun mois de l'export.", e.mois))),
        }
    }
}

impl std::fmt::Display for NumeroDEcriture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

/// Les ecritures de la periode -- une ecriture (une piece) par mois, ses lignes dans l'ordre de `ecritures_du_mois`.
fn ecritures_de(ctx: &Contexte, p: &Periode, avert: &mut Vec<String>, departs: &engagement::departs::Departs) -> R<(Vec<Ecriture>, Plan, String)> {
    let (plan, origine) = crate::plan_comptable::plan(ctx)?;
    let mut out = Vec::new();
    for &m in &p.mois {
        let (ecritures, non_portees) = ecritures_du_mois(ctx, m, &plan, departs)?;
        out.extend(ecritures);
        avert.extend(non_portees);
    }
    if !socle::fichiers::existe_lu(&crate::plan_comptable::PLAN_COMPTABLE.dans(&ctx.racine))? {
        avert.push("Plan comptable : celui livré avec le logiciel, à faire valider par le comptable (config/plan-comptable.json pour le remplacer).".to_string());
    }
    Ok((out, plan, origine))
}

// ------------------------------------------------------------------ ce que chaque ecrivain recoit

/// `Envoi` : le plan, le contexte, les mois de la periode (qui numerotent les ecritures), les codes de section quand
/// l'analytique est demandee, les libelles courts valides du format, et ce qui depasse sa zone (`libelle`, `intitule`),
/// releve au fil de l'ecriture.
struct Envoi<'a> {
    plan: &'a Plan,
    ctx: &'a Contexte,
    periode: &'a [Mois],
    sections: Option<BTreeMap<String, CodeAnalytique>>,
    /// Le format, quand ses zones sont bornees, et les libelles courts que la compagnie y a valides.
    borne: Option<FormatBorne>,
    courts: Option<Courts>,
    depasses: RefCell<BTreeMap<(Quoi, String), Depasse>>,
}

impl Envoi<'_> {
    /// La valeur d'une zone de `largeur` caracteres : `long` s'il tient ; sinon sa valeur courte validee, si elle tient ;
    /// sinon rien, et la valeur est notee (`depasses`) : l'export se refuse a la fin (`refus_des_depasses`), rien ne se
    /// coupe.
    fn tenir_ou_court(&self, quoi: Quoi, cle: &str, long: &str, largeur: usize) -> Option<String> {
        let long = blancs(long);
        if long.chars().count() <= largeur {
            return Some(long);
        }
        let court = self.courts.as_ref().and_then(|c| c.de(quoi, cle)).map(|c| blancs(c)).filter(|c| !c.is_empty() && c.chars().count() <= largeur);
        self.depasses
            .borrow_mut()
            .entry((quoi, cle.to_string()))
            .or_insert_with(|| Depasse { quoi, cle: cle.to_string(), long: long.clone(), largeur, court: court.clone() });
        court
    }

    /// Le libelle d'une ligne dans une zone de `n` caracteres : ce qui la distingue (`libelle_court`), puis son mois,
    /// toujours. Ce qui le distingue se remplace par sa valeur courte validee quand il ne tient pas.
    /// Ce qui ne tient pas laisse la zone vide : l'export se refuse a la fin (`refus_des_depasses`), ce fichier ne
    /// s'ecrit pas.
    fn libelle(&self, e: &Ecriture, nature: bool, n: usize) -> String {
        let (tete, mois) = libelle_court(e, nature);
        let place = n.saturating_sub(mois.chars().count() + 1);
        ou_vide(self.tenir_ou_court(Quoi::Libelle, &blancs(&tete), &tete, place).map(|t| format!("{} {}", t, mois)))
    }

    /// L'intitule du compte `compte` dans une zone de `n` caracteres, ou sa valeur courte validee ; ce qui ne tient pas
    /// laisse la zone vide, l'export se refuse a la fin (`refus_des_depasses`).
    fn intitule(&self, compte: &str, lib: &str, n: usize) -> String {
        ou_vide(self.tenir_ou_court(Quoi::Compte, compte, lib, n))
    }

    /// Ce qui ne tient pas et n'a pas de valeur courte validee : l'export se refuse, en disant combien et ou les
    /// raccourcir ; la page y renvoie (`libelles_courts`, le format).
    fn refus_des_depasses(&self, format: FormatDExport) -> R<()> {
        let n = self.depasses.borrow().values().filter(|d| d.court.is_none()).count();
        if n == 0 {
            return Ok(());
        }
        // Seul un format aux zones bornees note ce qui depasse.
        let nom = match self.borne {
            Some(f) => f.nom(),
            None => format.as_str(),
        };
        /// Ce que le refus porte a part : le format des libelles courts a poser, et combien.
        #[derive(Serialize)]
        struct APoser {
            libelles_courts: FormatDExport,
            nombre: usize,
        }
        Err(socle::echec::refus_avec(
            format!("{} trop long{} pour {} : les raccourcir (Exports, « Libellés courts »).", if n > 1 { format!("{} libellés", n) } else { "1 libellé".to_string() }, if n > 1 { "s" } else { "" }, nom),
            socle::echec::APart::forme(APoser { libelles_courts: format, nombre: n }),
        ))
    }

    /// `Envoi.section` : le code de section d'une ligne -- seules les lignes de salaires bruts en ont une.
    fn section(&self, e: &Ecriture) -> Option<String> {
        let sections = self.sections.as_ref()?;
        if e.compte != self.plan.brut.compte {
            return None;
        }
        sections.get(e.analytique.as_ref()?).map(CodeAnalytique::to_string)
    }
}

/// Un titre en code -- lettres et chiffres, sans accent, en majuscules ; plus large que la zone, `codes_sections` le refuse.
fn code_du_titre(titre: &str) -> String {
    let ascii = socle::texte::ascii_nfkd(titre);
    ascii.to_uppercase().chars().filter(|c| c.is_alphanumeric()).collect::<String>()
}

/// `codes_des_fiches` : titre -> code analytique, lu sur les fiches spectacle. La fiche active l'emporte, puis le
/// numero le plus recent.
fn codes_des_fiches(spectacles: Vec<&droit::fiche::ChampsSpectacle>) -> BTreeMap<String, CodeAnalytique> {
    let mut rangees = spectacles;
    rangees.sort_by_cached_key(|v| (!v.archivee(), v.numero_objet.en_texte()));
    let mut out = BTreeMap::new();
    for v in rangees {
        if let (Some(titre), Some(code)) = (v.titre.remplie(), v.code_analytique.dite()) {
            out.insert(titre.to_string(), code.clone());
        }
    }
    out
}

/// `codes_sections` : le code de chaque spectacle -- celui de sa fiche, sinon celui du plan, sinon tire du titre.
fn codes_sections(plan: &Plan, ecritures: &[Ecriture], largeur: usize, fiches: &BTreeMap<String, CodeAnalytique>) -> R<BTreeMap<String, CodeAnalytique>> {
    let brut = &plan.brut.compte;
    // Les codes choisis : ceux du plan, puis ceux des fiches.
    let mut propres: BTreeMap<String, CodeAnalytique> = BTreeMap::new();
    if let Some(s) = &plan.sections {
        propres.extend(s.iter().map(|(k, v)| (k.clone(), v.clone())));
    }
    propres.extend(fiches.iter().map(|(k, v)| (k.clone(), v.clone())));
    let mut out = BTreeMap::new();
    let mut qui: BTreeMap<CodeAnalytique, String> = BTreeMap::new();
    let titres: BTreeSet<&String> = ecritures.iter().filter(|e| e.compte == *brut).filter_map(|e| e.analytique.as_ref()).filter(|a| !a.is_empty()).collect();
    for titre in titres {
        let choisi = propres.get(titre);
        let code = match choisi {
            Some(c) => c.to_string(),
            None => code_du_titre(titre),
        };
        if code.is_empty() || code.chars().count() > largeur {
            return Err(refus(format!(
                "Analytique : le code « {} » du spectacle « {} » ne tient pas dans la zone de ce logiciel ({} caractères au plus). Le raccourcir dans Configuration › Spectacles, case « Code analytique ».",
                code, titre, largeur
            )));
        }
        let code = CodeAnalytique::lire(&code)?;
        if let Some(autre) = qui.get(&code) {
            let meme_choix = choisi.is_some() && propres.get(autre) == Some(&code);
            if !meme_choix {
                return Err(refus(format!(
                    "Analytique : « {} » et « {} » donnent le même code « {} ». En donner un à chacun dans Configuration › Spectacles, case « Code analytique ».",
                    autre, titre, code
                )));
            }
        }
        out.insert(titre.clone(), code.clone());
        qui.insert(code, titre.clone());
    }
    Ok(out)
}

/// `comptes_utilises` : (compte, libelle) de chaque compte que les ecritures mouvementent, dans l'ordre des numeros.
fn comptes_utilises(ecritures: &[Ecriture], plan: &Plan) -> Vec<(NumeroDeCompte, String)> {
    let mut vus: BTreeMap<NumeroDeCompte, String> = BTreeMap::new();
    for e in ecritures {
        let c = compte_de(e);
        if !vus.contains_key(c) {
            // Un compte que le plan ne nomme pas prend le libelle de sa ligne, sans le mois.
            let lib = match libelle_de(plan, e).filter(|l| !l.is_empty()) {
                Some(l) => l,
                None => match e.libelle.split_once(" -- ") {
                    Some((tete, _)) => tete.to_string(),
                    None => e.libelle.clone(),
                },
            };
            vus.insert(c.clone(), lib);
        }
    }
    vus.into_iter().collect()
}

// ------------------------------------------------------------------ FEC

const ZONES_FEC: [&str; 18] = [
    "JournalCode", "JournalLib", "EcritureNum", "EcritureDate", "CompteNum", "CompteLib", "CompAuxNum", "CompAuxLib", "PieceRef", "PieceDate", "EcritureLib", "Debit",
    "Credit", "EcritureLet", "DateLet", "ValidDate", "Montantdevise", "Idevise",
];

/// Ni tabulation ni fin de ligne dans une zone.
fn fec_propre(s: &str) -> String {
    blancs(&s.replace('\t', " "))
}

/// `fec` : l'extrait du journal de paie aux 18 champs, UTF-8, tabulations, CRLF.
fn fec(ecritures: &[Ecriture], env: &Envoi) -> R<Vec<u8>> {
    let plan = env.plan;
    let mut lignes = vec![ZONES_FEC.join("\t")];
    let journal_lib = plan.journal_libelle.clone();
    for e in ecritures {
        let date = e.date.aaaammjj();
        let lib = match e.analytique.as_ref().filter(|a| !a.is_empty()) {
            Some(a) => format!("{} — {}", e.libelle, a),
            None => e.libelle.clone(),
        };
        // Un champ « à blanc si non utilisé » (BOI-CF-IOR-60-40-20, § 70) reste vide.
        let champs = [
            e.journal.to_string(),
            journal_lib.clone(),
            format!("{}{:05}", e.date.annee(), NumeroDEcriture::de(e, env.periode)?),
            date.clone(),
            e.compte.to_string(),
            ou_vide(libelle_compte(plan, &e.compte)),
            ou_vide(e.auxiliaire.as_ref().map(NumeroDeCompte::to_string)),
            ou_vide(e.auxiliaire_libelle.clone()),
            e.piece(),
            date,
            lib,
            deux_a_virgule(e.debit),
            deux_a_virgule(e.credit),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        ];
        lignes.push(champs.iter().map(|c| fec_propre(c)).collect::<Vec<_>>().join("\t"));
    }
    Ok(format!("{}\r\n", lignes.join("\r\n")).into_bytes())
}

// ------------------------------------------------------------------ outils communs

/// Le libelle d'une ligne dans une zone etroite, en deux morceaux : ce qui la distingue (le spectacle ou le salarie,
/// sinon la nature ; les deux si `nature`), et son mois (MM/AAAA). La zone prend les deux, ou ce qui la distingue se
/// remplace par sa valeur courte validee (`Envoi::libelle`).
fn libelle_court(e: &Ecriture, nature: bool) -> (String, String) {
    let mm = e.date.mois_civil().mm_aaaa();
    let sans_mois = match e.libelle.rfind(" -- ") {
        Some(i) => &e.libelle[..i],
        None => e.libelle.as_str(),
    };
    let quoi = sans_mois.split(" -- ").collect::<Vec<_>>().join(" ");
    let tete = match e.analytique.as_ref().filter(|a| !a.is_empty()) {
        Some(a) if nature => format!("{} — {}", quoi, a),
        Some(a) => a.clone(),
        None => quoi,
    };
    (blancs(&tete), mm)
}

/// Le libelle entier d'une ligne, quand la zone n'a pas de largeur.
fn libelle_entier(e: &Ecriture) -> String {
    let (tete, mois) = libelle_court(e, true);
    format!("{} {}", tete, mois)
}

/// PAIE-2026-08 fait 12 caracteres ; Oxygene et Quadra en prennent 10 : PAIE202608.
fn piece_courte(e: &Ecriture) -> String {
    format!("PAIE{:04}{:02}", e.mois.annee(), e.mois.numero())
}

/// Le sens (D ou C) et le montant d'une ecriture.
fn sens(e: &Ecriture) -> (&'static str, Montant) {
    if e.debit.est_nul() {
        ("C", e.credit)
    } else {
        ("D", e.debit)
    }
}

/// Ni point-virgule ni fin de ligne dans une zone.
fn csv_propre(s: &str) -> String {
    blancs(&s.replace(';', ","))
}

/// Un montant a deux decimales, la virgule pour separateur ; vide s'il est nul.
fn virgule(v: Montant) -> String {
    if v.est_nul() {
        String::new()
    } else {
        deux_a_virgule(v)
    }
}

/// Le compte et le journal tiennent dans leurs zones, ou on refuse.
fn borne(format: FormatDExport, compte: &str, journal: &str) -> R<()> {
    let l = crate::plan_comptable::tables().limites.get(&format).ok_or_else(|| erreur(Classe::Manque, format!("format inconnu : {}", socle::fichiers::repr(format.as_str()))))?;
    if let Some(lc) = l.compte.filter(|lc| compte.chars().count() > *lc) {
        return Err(refus(format!("{} : le compte « {} » dépasse sa zone ({} caractères). Choisir dans le plan comptable de la compagnie un compte qui tient.", l.nom, compte, lc)));
    }
    if journal.chars().count() > l.journal {
        return Err(refus(format!("{} : le journal « {} » dépasse sa zone ({} caractères). Raccourcir le code journal dans le plan comptable de la compagnie.", l.nom, journal, l.journal)));
    }
    Ok(())
}

// ------------------------------------------------------------------ XIMPORT

/// Les treize zones de la notice (`SOURCE_XIMPORT`, l. 35-48), dans son ordre : (cle, longueur, cadrage). La version de
/// Ciel 2003 (l. 49-52, « avec en plus ») n'est pas ecrite.
const LARGEURS_XIMPORT: [(&str, usize, char); 13] = [
    ("mouvement", 5, 'd'), ("journal", 2, 'g'), ("date", 8, 'g'), ("echeance", 8, 'g'), ("piece", 12, 'g'), ("compte", 11, 'g'), ("libelle", 25, 'g'), ("montant", 13, 'd'),
    ("sens", 1, 'g'), ("pointage", 12, 'g'), ("analytique", 6, 'g'), ("libelle_compte", 34, 'g'), ("euro", 1, 'g'),
];

/// `ximport` : une ligne par ligne d'ecriture, a largeurs fixes, cp1252.
fn ximport(ecritures: &[Ecriture], env: &Envoi) -> R<Vec<u8>> {
    let mut out = Vec::new();
    for e in ecritures {
        let (s, montant) = sens(e);
        let champs: BTreeMap<&str, String> = [
            ("mouvement", NumeroDEcriture::de(e, env.periode)?.to_string()),
            ("journal", e.journal.to_string()),
            ("date", e.date.aaaammjj()),
            ("echeance", String::new()),
            ("piece", e.piece()),
            ("compte", compte_de(e).to_string()),
            ("libelle", env.libelle(e, false, 25)),
            ("montant", deux_a_virgule(montant)),
            ("sens", s.to_string()),
            ("pointage", String::new()),
            ("analytique", ou_vide(env.section(e))),
            ("libelle_compte", env.intitule(compte_de(e).as_str(), &ou_vide(libelle_de(env.plan, e)), 34)),
            ("euro", "O".to_string()),
        ]
        .into_iter()
        .collect();
        out.push(LARGEURS_XIMPORT.iter().map(|(k, n, c)| zone("XIMPORT", k, &champs[k], *n, *c)).collect::<R<String>>()?);
    }
    Ok(cp1252(&out))
}

// ------------------------------------------------------------------ Oxygene

/// Les zones de « L'importation à partir d'un seul fichier des écritures » (`SOURCE_OXYGENE`, l. 46-136), dans l'ordre
/// de la notice.
fn zones_oxygene() -> Vec<String> {
    let mut z: Vec<String> =
        ["CODSOC", "CODAGE", "NUMJL", "MONNAIE_IDENT", "NUMCP", "COURS", "CSECT", "DTOPE", "NPIEC", "LIBEC", "MTDEB", "MTCRE", "RELET", "NIPIE", "LIBAUTO"].iter().map(|s| s.to_string()).collect();
    for i in 1..=5 {
        for p in ["CB", "CR", "DE", "ME"] {
            z.push(format!("{}{}", p, i));
        }
    }
    z.push("NIDENT".to_string());
    z.push("NUMCP2".to_string());
    z
}

/// `oxygene` : une ligne par ligne d'ecriture ; NPIEC regroupe les lignes d'une meme piece, le mois.
fn oxygene(ecritures: &[Ecriture], env: &Envoi) -> R<Vec<u8>> {
    let zones = zones_oxygene();
    let mut out = vec![zones.join(";")];
    for e in ecritures {
        borne(FormatDExport::Oxygene, compte_de(e).as_str(), e.journal.as_str())?;
        let section = env.section(e);
        let mut z: BTreeMap<&str, String> = zones.iter().map(|k| (k.as_str(), String::new())).collect();
        // Le poste analytique d'une ligne ventilee : celui que la compagnie dit ; sinon la zone reste vide.
        let poste: String = section.as_ref().and(env.plan.poste_analytique.as_ref()).map(|p| tenir("Oxygène", "CODAGE", p.as_str(), 9)).transpose()?.into_iter().collect();
        let section = ou_vide(section);
        for (k, v) in [
            ("NUMJL", e.journal.to_string()),
            ("NUMCP", compte_de(e).to_string()),
            ("DTOPE", e.date.aaaammjj()),
            ("NPIEC", piece_courte(e)),
            ("LIBEC", env.libelle(e, true, 40)),
            ("MTDEB", virgule(e.debit)),
            ("MTCRE", virgule(e.credit)),
            ("CSECT", section),
            ("CODAGE", poste),
        ] {
            z.insert(k, v);
        }
        out.push(zones.iter().map(|k| csv_propre(&z[k.as_str()])).collect::<Vec<_>>().join(";"));
    }
    Ok(cp1252(&out))
}

/// `oxygene_comptes` : « L'importation du plan comptable », fiche compte (`SOURCE_OXYGENE`, l. 353-371) : le numero,
/// l'intitule reduit (non obligatoire : laisse vide), l'intitule.
fn oxygene_comptes(comptes: &[(NumeroDeCompte, String)], env: &Envoi) -> R<Vec<u8>> {
    let mut out = vec!["NUMCP;CODCP;LIBCP".to_string()];
    for (compte, lib) in comptes {
        let compte = compte.as_str();
        borne(FormatDExport::Oxygene, compte, "")?;
        out.push([csv_propre(compte), String::new(), csv_propre(&env.intitule(compte, lib, 50))].join(";"));
    }
    Ok(cp1252(&out))
}

// ------------------------------------------------------------------ Sage (PNM)

/// Le format 105 caracteres (`SOURCE_SAGE`, l. 86-178) : (cle, longueur, cadrage), aux positions 1, 4, 10, 12, 25, 26,
/// 39, 52, 77, 78, 84, 85 et 105.
const LARGEURS_PNM: [(&str, usize, char); 13] = [
    ("journal", 3, 'g'), ("date", 6, 'g'), ("type_piece", 2, 'g'), ("compte", 13, 'g'), ("type_compte", 1, 'g'), ("auxiliaire", 13, 'g'), ("reference", 13, 'g'),
    ("libelle", 25, 'g'), ("paiement", 1, 'g'), ("echeance", 6, 'g'), ("sens", 1, 'g'), ("montant", 20, 'd'), ("type_ecriture", 1, 'g'),
];

/// `sage_pnm` : le format 105 caracteres. Avec l'analytique, la ligne generale est suivie d'une ligne de type A.
fn sage_pnm(ecritures: &[Ecriture], env: &Envoi) -> R<Vec<u8>> {
    let nom = match nom_compagnie(env.ctx) {
        Some(n) => n.to_string(),
        None => "Paie".to_string(),
    };
    // Un nom qui ne tient pas laisse la ligne vide : l'export se refuse a la fin (`refus_des_depasses`).
    let mut out = vec![ou_vide(env.tenir_ou_court(Quoi::Compagnie, &nom, &nom, 30))];
    for e in ecritures {
        borne(FormatDExport::Sage, compte_de(e).as_str(), e.journal.as_str())?;
        let (s, montant) = sens(e);
        let mut champs: BTreeMap<&str, String> = [
            ("journal", e.journal.to_string()),
            ("date", e.date.jjmmaa()),
            ("type_piece", "OD".to_string()),
            ("compte", compte_de(e).to_string()),
            ("type_compte", String::new()),
            ("auxiliaire", String::new()),
            ("reference", e.piece()),
            ("libelle", env.libelle(e, false, 25)),
            ("paiement", String::new()),
            ("echeance", String::new()),
            ("sens", s.to_string()),
            ("montant", deux(montant)),
            ("type_ecriture", "N".to_string()),
        ]
        .into_iter()
        .collect();
        let ligne = |c: &BTreeMap<&str, String>| LARGEURS_PNM.iter().map(|(k, n, cote)| zone("Sage", k, &c[k], *n, *cote)).collect::<R<String>>();
        out.push(ligne(&champs)?);
        if let Some(section) = env.section(e) {
            champs.insert("type_compte", "A".to_string());
            champs.insert("auxiliaire", section);
            out.push(ligne(&champs)?);
        }
    }
    Ok(cp1252(&out))
}

// ------------------------------------------------------------------ Quadra

/// L'enregistrement M (`SOURCE_QUADRA`, l. 9-34) : (cle, longueur), les positions se suivant sans trou de 1 a 158. Le
/// libelle s'ecrit sur 30 en 117 : celui de 20 en 22 reste blanc (« blanc si renseigné en 22 sur 20 caract. », l. 32).
const POSITIONS_QUADRA: [(&str, usize); 23] = [
    ("type", 1), ("compte", 8), ("journal", 2), ("folio", 3), ("date", 6), ("code_libelle", 1), ("libelle", 20), ("sens", 1), ("montant", 13), ("contrepartie", 8),
    ("echeance", 6), ("lettrage", 2), ("statistiques", 3), ("piece5", 5), ("affaire", 10), ("quantite", 10), ("piece8", 8), ("devise", 3), ("journal3", 3), ("tva", 3),
    ("libelle30", 30), ("tva2", 2), ("piece10", 10),
];
/// L'enregistrement C (`SOURCE_QUADRA`, l. 64-82), de 1 a 218.
const POSITIONS_QUADRA_COMPTE: [(&str, usize); 12] = [
    ("type", 1), ("compte", 8), ("libelle", 30), ("cle", 7), ("debit1", 13), ("credit1", 13), ("debit2", 13), ("credit2", 13), ("collectif", 8), ("adresse", 110), ("maj", 1),
    ("type_compte", 1),
];

/// Huit caracteres, pas un de plus ; un compte de chiffres plus court est complete de zeros.
fn compte_quadra(compte: &str) -> R<String> {
    if compte.chars().count() > 8 {
        return Err(refus(format!(
            "Quadra : le compte « {} » a plus de 8 caractères, la largeur de la zone. Choisir dans le plan comptable de la compagnie un sous-compte qui tient (par exemple 421 suivi du numéro du salarié sur trois chiffres, plutôt que 421 suivi de son nom).",
            compte
        )));
    }
    Ok(if tout_chiffres(compte) { caler_zeros(compte, 8) } else { compte.to_string() })
}

/// `compte.ljust(8, "0")`.
fn caler_zeros(compte: &str, n: usize) -> String {
    let mut s = compte.to_string();
    while s.chars().count() < n {
        s.push('0');
    }
    s
}

/// Les zones a la suite, cadrees a gauche ; une zone que rien ne remplit s'ecrit en blancs.
fn quadra_ligne(champs: &BTreeMap<&str, String>, positions: &[(&str, usize)]) -> R<String> {
    positions.iter().map(|(k, n)| zone("Quadra", k, &ou_vide(champs.get(k).cloned()), *n, 'g')).collect()
}

/// `quadra` : les comptes d'abord (type C), puis les ecritures (enregistrements M).
fn quadra(ecritures: &[Ecriture], env: &Envoi) -> R<Vec<u8>> {
    let mut sortie = Vec::new();
    for (c, lib) in comptes_utilises(ecritures, env.plan) {
        let mut ch: BTreeMap<&str, String> = BTreeMap::new();
        ch.insert("type", "C".to_string());
        ch.insert("compte", compte_quadra(c.as_str())?);
        ch.insert("libelle", env.intitule(c.as_str(), &lib, 30));
        for k in ["debit1", "credit1", "debit2", "credit2"] {
            ch.insert(k, "+000000000000".to_string());
        }
        ch.insert("type_compte", "G".to_string());
        sortie.push(quadra_ligne(&ch, &POSITIONS_QUADRA_COMPTE)?);
    }
    for e in ecritures {
        let (s, montant) = sens(e);
        let mut ch: BTreeMap<&str, String> = BTreeMap::new();
        ch.insert("type", "M".to_string());
        ch.insert("compte", compte_quadra(compte_de(e).as_str())?);
        ch.insert("journal", e.journal.to_string());
        ch.insert("folio", "000".to_string());
        ch.insert("date", e.date.jjmmaa());
        ch.insert("sens", s.to_string());
        ch.insert("montant", format!("+{}", socle::grandeurs::ecrit::en_centimes::chiffres(montant, 12)));
        ch.insert("affaire", ou_vide(env.section(e)));
        ch.insert("devise", "EUR".to_string());
        ch.insert("journal3", e.journal.to_string());
        ch.insert("libelle30", env.libelle(e, false, 30));
        ch.insert("piece10", piece_courte(e));
        sortie.push(quadra_ligne(&ch, &POSITIONS_QUADRA)?);
    }
    Ok(cp1252(&sortie))
}

// ------------------------------------------------------------------ EBP

/// Entre guillemets, sans guillemet dedans.
fn ebp_texte(quoi: &str, s: &str, n: usize) -> R<String> {
    Ok(format!("\"{}\"", tenir("EBP", quoi, &s.replace('"', "'"), n)?))
}

/// Une zone sans guillemets ne contient pas de virgule.
fn ebp_zone(s: &str) -> String {
    blancs(&s.replace(',', " ").replace('"', "'"))
}

/// `ebp` : les onze zones de la notice (`SOURCE_EBP`, l. 52-80), toutes, meme vides ; avec l'analytique, la ligne de
/// ventilation suit l'ecriture (l. 101-115).
fn ebp(ecritures: &[Ecriture], env: &Envoi) -> R<Vec<u8>> {
    let mut out = Vec::new();
    for (i, e) in ecritures.iter().enumerate() {
        borne(FormatDExport::Ebp, compte_de(e).as_str(), e.journal.as_str())?;
        let (s, montant) = sens(e);
        out.push(
            [
                (i + 1).to_string(),
                e.date.jjmmaa(),
                e.journal.to_string(),
                compte_de(e).to_string(),
                String::new(),
                format!("\"{}\"", env.libelle(e, true, 40).replace('"', "'")),
                ebp_texte("piece", &e.piece(), 15)?,
                deux(montant),
                s.to_string(),
                String::new(),
                String::new(),
            ]
            .join(","),
        );
        if let Some(section) = env.section(e) {
            out.push(format!(">{},100.00,{}", ebp_zone(&section), deux(montant)));
        }
    }
    Ok(cp1252(&out))
}

/// `ebp_comptes` : COMPTES.TXT, ses dix zones (`SOURCE_EBP`, l. 137-150).
fn ebp_comptes(comptes: &[(NumeroDeCompte, String)], env: &Envoi) -> R<Vec<u8>> {
    let out: Vec<String> = comptes
        .iter()
        .map(|(c, lib)| {
            let c = c.as_str();
            borne(FormatDExport::Ebp, c, "")?;
            let mut z = vec![ebp_zone(c), env.intitule(c, &ebp_zone(lib), 60)];
            z.extend(std::iter::repeat(String::new()).take(8));
            Ok(z.join(","))
        })
        .collect::<R<_>>()?;
    Ok(cp1252(&out))
}

// ------------------------------------------------------------------ Pennylane

/// Les comptes que Pennylane prend en lettres (`SOURCE_PENNYLANE`, l. 91-93) ; les autres sont des chiffres.
const PENNYLANE_ALPHANUMERIQUES: [&str; 6] = ["401", "411", "410", "421", "455", "467"];

/// `pennylane` : une ligne par ligne d'ecriture, ses colonnes nommees (la notice fait correspondre les colonnes au
/// premier import) ; le code journal n'a que des lettres (5 au plus) ; avec l'analytique, la categorie (le spectacle), sa
/// famille et son code. UTF-8, point-virgule.
fn pennylane(ecritures: &[Ecriture], env: &Envoi) -> R<Vec<u8>> {
    let analytique = env.sections.is_some();
    let mut entete = "Date;Code journal;Numéro de pièce;Numéro de compte;Libellé;Débit;Crédit".to_string();
    if analytique {
        entete.push_str(";Famille de catégories;Catégorie;Code analytique");
    }
    let mut out = vec![entete];
    for e in ecritures {
        let journal = e.journal.as_str();
        if !(journal.chars().all(char::is_alphabetic) && journal.chars().count() <= 5) {
            return Err(refus(format!("Pennylane : le code journal « {} » doit tenir en 5 lettres. Le changer dans le plan comptable de la compagnie.", journal)));
        }
        let compte = compte_de(e).to_string();
        if !tout_chiffres(&compte) && !PENNYLANE_ALPHANUMERIQUES.iter().any(|p| compte.starts_with(p)) {
            return Err(refus(format!(
                "Pennylane : le compte « {} » porte des lettres, ce que Pennylane n'accepte que pour les comptes 401, 411, 410, 421, 455 et 467. Le changer dans le plan comptable de la compagnie.",
                compte
            )));
        }
        let mut champs = vec![e.date.jj_mm_aaaa(), journal.to_string(), e.piece(), compte, libelle_entier(e), virgule(e.debit), virgule(e.credit)];
        if analytique {
            // Une ligne ventilee porte sa famille, sa categorie (le spectacle) et son code ; les autres laissent les trois
            // colonnes vides.
            match env.section(e) {
                Some(code) => champs.extend(["Spectacles".to_string(), ou_vide(e.analytique.clone()), code]),
                None => champs.extend([String::new(), String::new(), String::new()]),
            }
        }
        out.push(champs.iter().map(|c| csv_propre(c)).collect::<Vec<_>>().join(";"));
    }
    Ok(format!("{}\r\n", out.join("\r\n")).into_bytes())
}

// ------------------------------------------------------------------ les formats de logiciel

/// L'analytique d'un format : il ne la porte pas, ou il la porte dans une zone que sa notice borne ou non.
#[derive(Clone, Copy)]
enum Analytique {
    Non,
    Largeur(usize),
    SansLargeur,
}

/// `Ecrivain` : le fichier, l'ecrivain, la source, ce qu'on dit a celui qui l'importe, l'analytique du format, et le
/// fichier des comptes quand le format en a un.
struct Ecrivain {
    fichier: NomDExport,
    ecrire: fn(&[Ecriture], &Envoi) -> R<Vec<u8>>,
    source: &'static str,
    conseil: String,
    section: Analytique,
    /// (nom, ecrivain, conseil)
    comptes: Option<(NomDExport, fn(&[(NumeroDeCompte, String)], &Envoi) -> R<Vec<u8>>, &'static str)>,
}


/// `ECRIVAINS.get(format)`.
fn ecrivain(format: FormatDExport) -> Option<Ecrivain> {
    Some(match format {
        FormatDExport::Fec => Ecrivain {
            fichier: const { NomDExport::autour("extrait-fec-paie-", ".txt") },
            ecrire: fec,
            source: SOURCE_FEC,
            conseil: "Extrait du journal de paie à la forme du FEC, à importer dans la comptabilité — ce n'est pas le FEC de l'entreprise, que seul le logiciel de comptabilité produit. ValidDate est vide : la validation se fait à l'import.".to_string(),
            section: Analytique::Non,
            comptes: None,
        },
        FormatDExport::Ximport => Ecrivain {
            fichier: const { NomDExport::autour("XIMPORT-paie-", ".TXT") },
            ecrire: ximport,
            source: SOURCE_XIMPORT,
            conseil: format!("Sage 50 (ex-Ciel) : Échanges > Importer des écritures, format XImport.{}", ESSAI),
            section: Analytique::Largeur(6),
            comptes: None,
        },
        FormatDExport::Oxygene => Ecrivain {
            fichier: const { NomDExport::autour("oxygene-ecritures-paie-", ".csv") },
            ecrire: oxygene,
            source: SOURCE_OXYGENE,
            conseil: format!(
                "Oxygène : importation de données, « Import écritures à partir d'1 seul fichier », format EXCEL2 (CSV à point-virgule, celui que Memsoft conseille). L'aide de Memsoft ne dit ni le séparateur décimal ni le jeu de caractères : virgule et Windows ont été retenus.{}",
                ESSAI
            ),
            section: Analytique::Largeur(20),
            comptes: Some((const { NomDExport::autour("oxygene-plan-comptable-paie-", ".csv") }, oxygene_comptes, "à importer d'abord, « Import plan comptable » ; un compte qui existe déjà n'a pas à y être.")),
        },
        FormatDExport::Sage => Ecrivain {
            fichier: const { NomDExport::autour("ecritures-paie-", ".pnm") },
            ecrire: sage_pnm,
            source: SOURCE_SAGE,
            conseil: format!(
                "Sage 100 : Fichier > Importer, type « Écritures Sage (*.pnm) ». Journal et comptes doivent exister dans la société ; le montant est écrit au point décimal, cadré à droite.{}",
                ESSAI
            ),
            section: Analytique::Largeur(13),
            comptes: None,
        },
        FormatDExport::Quadra => Ecrivain {
            fichier: const { NomDExport::autour("quadra-ecritures-paie-", ".txt") },
            ecrire: quadra,
            source: SOURCE_QUADRA,
            conseil: format!(
                "Quadra : Suivi des dossiers > Import ASCII (Compta). Les comptes utilisés sont décrits en tête (sans toucher à un compte existant) ; les comptes de chiffres sont complétés à 8 (641100 → 64110000) ; montants en centimes signés, folio 000.{}",
                ESSAI
            ),
            section: Analytique::Largeur(10),
            comptes: None,
        },
        FormatDExport::Ebp => Ecrivain {
            fichier: const { NomDExport::autour("ECRITURES-paie-", ".TXT") },
            ecrire: ebp,
            source: SOURCE_EBP,
            conseil: format!("EBP : Outils > Import de données > Import écritures, format « EBP V3 pour Windows ».{}", ESSAI),
            section: Analytique::SansLargeur,
            comptes: Some((NomDExport::Fixe(const { NomDeFichier::du_moteur("COMPTES.TXT") }), ebp_comptes, "EBP le lit seul s'il est dans le même dossier que le fichier des écritures, et crée les comptes absents avec leur intitulé.")),
        },
        FormatDExport::Pennylane => Ecrivain {
            fichier: const { NomDExport::autour("pennylane-ecritures-paie-", ".csv") },
            ecrire: pennylane,
            source: SOURCE_PENNYLANE,
            conseil: format!(
                "Pennylane : Saisie > Imports > Importer un fichier > Écritures ; faire correspondre les colonnes au premier import et enregistrer le modèle. Le 421 individuel est accepté en lettres.{}",
                ESSAI
            ),
            section: Analytique::SansLargeur,
            comptes: None,
        },
        // Les formats de la machinerie des tableaux : pas d'ecrivain de logiciel.
        FormatDExport::Csv | FormatDExport::Tsv | FormatDExport::Xlsx | FormatDExport::Pdf | FormatDExport::Html | FormatDExport::Json => return None,
    })
}

/// Le logiciel du comptable et l'analytique que la compagnie a retenus, sous sa racine.
const COMPTA: socle::fichiers::CheminRelatif = socle::fichiers::CheminRelatif::du_moteur("config/compta.json");

/// `config/compta.json` : le logiciel du comptable (absent : aucun) et l'analytique, retenus une fois pour toutes. Lu
/// par `ChoixCompta::lu` seule (les ecritures comptables, la page des exports), ecrit par `ChoixCompta::ecrire` seule
/// (`retenir-logiciel`).
#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChoixCompta {
    #[serde(rename = "quoi")]
    _quoi: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logiciel: Option<LogicielComptable>,
    pub analytique: bool,
}

impl ChoixCompta {
    /// Le choix `logiciel` (aucun : aucun logiciel retenu) et `analytique`, tel qu'il s'ecrit.
    pub fn nouveau(logiciel: Option<LogicielComptable>, analytique: bool) -> ChoixCompta {
        ChoixCompta { _quoi: "Le logiciel du comptable, retenu pour les écritures comptables (Exports).".to_string(), logiciel, analytique }
    }

    /// `config/compta.json`, lu par sa forme ; rien de retenu s'il n'existe pas.
    pub fn lu(racine: &std::path::Path) -> R<ChoixCompta> {
        let chemin = COMPTA.dans(racine);
        if !chemin.exists() {
            return Ok(ChoixCompta::nouveau(None, false));
        }
        socle::forme::lire_fichier(&chemin, "Le logiciel du comptable retenu (config/compta.json) ne se lit pas.").map_err(|m| m.reclasser(Classe::Illisible))
    }

    /// Le choix ecrit dans `config/compta.json`.
    pub fn ecrire(&self, racine: &std::path::Path) -> R<()> {
        socle::ecriture::ecrire_json(&COMPTA.dans(racine), self, 2).map_err(|m| m.reclasser(Classe::Disque))
    }

    /// Le logiciel retenu, s'il en est un.
    fn retenu(&self) -> Option<LogicielComptable> {
        self.logiciel
    }
}

// ------------------------------------------------------------------ le journal de paie

/// `ecritures_comptables` : le journal de paie, une piece par mois, equilibree, dans le format choisi -- tableau
/// (CSV, texte tabule, XLSX, PDF, HTML, JSON), ou celui d'un logiciel de comptabilite.
pub fn ecritures_comptables(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let p = Periode::de(ctx, d, true)?;
    let mut avert: Vec<String> = Vec::new();
    let retenu = ChoixCompta::lu(&ctx.racine)?;
    let mut format = d.format;
    let logiciel = d.logiciel;
    let analytique = match d.analytique {
        Some(a) => a,
        None => retenu.analytique,
    };
    if let Some(l) = logiciel {
        let Some(connu) = logiciel_connu(l) else {
            return Err(refus(format!("Logiciel inconnu : « {} ». Les logiciels connus sont {}.", l, logiciels_dits().join(", "))));
        };
        avert.push(format!("{} : format « {} » — {}.", connu.nom, connu.format, connu.pourquoi));
        format = Some(connu.format);
    }
    // `exporter` exige un format de cet etat : sans format ni logiciel, rien ne se choisit a sa place.
    let Some(format) = format else {
        return Err(refus("Les écritures comptables s'exportent dans un format : choisir le format, ou le logiciel comptable.".to_string()));
    };
    let (ecr, plan, origine) = ecritures_de(ctx, &p, &mut avert, &d.departs)?;
    let mut t = Tableau::nouveau(EtatDExport::EcrituresComptables, "Journal de paie — écritures comptables", p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    let mois: BTreeSet<Mois> = ecr.iter().map(|e| e.mois).collect();
    for m in mois {
        let mut lignes = Vec::new();
        for e in ecr.iter().filter(|e| e.mois == m) {
            lignes.push(ligne_de(e, &plan));
        }
        t.sections.push(Section::nouvelle(libelle_mois(m), lignes));
    }
    t.colonnes = vec![
        C::Journal.colonne(),
        C::Date.colonne(),
        C::Piece.titree("N° pièce"),
        C::Compte.titree("N° compte"),
        C::Libelle.colonne(),
        C::Debit.colonne(),
        C::Credit.colonne(),
        C::Analytique.colonne(),
    ];
    t.sources = vec![SOURCES.to_string(), format!("plan : {}", origine)];
    let ecr_format = ecrivain(format);
    if let Some(ec) = &ecr_format {
        t.sources.push(ec.source.to_string());
        t.avertissements.push(ec.conseil.clone());
    }
    t.format = format_possible(Some(format), &FORMATS_ECRITURES)?;
    totaliser(&mut t, TOTAL_GENERAL);
    // L'equilibre du journal : chaque mois s'equilibre a sa fabrication (`ecritures_du_mois`), seul endroit qui le tienne.
    let mut sections = None;
    if analytique {
        let largeur = match ecr_format.as_ref().map(|e| e.section) {
            Some(Analytique::Largeur(n)) => Some(n),
            Some(Analytique::SansLargeur) => Some(usize::MAX),
            _ => None,
        };
        if let Some(largeur) = largeur {
            let codes = codes_sections(&plan, &ecr, largeur, &codes_des_fiches(ctx.spectacles.values().collect()))?;
            let dits: Vec<String> = codes.iter().map(|(titre, code)| format!("{} = {}", code, titre)).collect();
            t.avertissements.push(format!(
                "Analytique : les salaires bruts sont ventilés par spectacle ({}). Les sections doivent exister dans le logiciel du comptable ; leurs codes se changent dans Configuration › Spectacles, case « Code analytique ».",
                dits.join(", ")
            ));
            sections = Some(codes);
        } else if ecr_format.is_some() {
            t.avertissements.push(format!("Analytique : le format « {} » ne la porte pas ; les écritures sortent sans ventilation.", t.format));
        }
    }
    if d.ecrire {
        let dossier = p.dossier(ctx);
        if let Some(ec) = &ecr_format {
            let io = |c: &std::path::Path, e: std::io::Error| socle::echec::du_disque(&c, &e).reclasser(Classe::Disque);
            let env = envoi(ctx, &plan, &p.mois, sections, format)?;
            // Les deux fichiers se fabriquent avant d'ecrire : une zone qui ne tient pas refuse l'export entier.
            let (ecritures, comptes) = fabriquer(ec, &ecr, &env)?;
            env.refus_des_depasses(format)?;
            std::fs::create_dir_all(&dossier).map_err(|e| io(&dossier, e))?;
            let periode = p.nom();
            let chemin = dossier.join(ec.fichier.de(&periode));
            std::fs::write(&chemin, ecritures).map_err(|e| io(&chemin, e))?;
            t.fichier = Some(chemin);
            if let Some((modele, octets, conseil)) = comptes {
                let nom_comptes = modele.de(&periode);
                let cible = dossier.join(&nom_comptes);
                std::fs::write(&cible, octets).map_err(|e| io(&cible, e))?;
                t.avertissements.push(format!("Les comptes et leurs libellés sont dans « {} », à côté : {}", nom_comptes, conseil));
            }
        } else {
            ecrire_tableau(&mut t, &dossier, &format!("ecritures-comptables-{}", p.nom()), ctx)?;
        }
    }
    Ok(t)
}

/// Ce qu'un ecrivain recoit pour `format` : le plan, les mois de la periode, les sections, et les libelles courts valides
/// de ce format.
fn envoi<'a>(ctx: &'a Contexte, plan: &'a Plan, periode: &'a [Mois], sections: Option<BTreeMap<String, CodeAnalytique>>, format: FormatDExport) -> R<Envoi<'a>> {
    let borne = FormatBorne::de(format);
    let courts = match borne {
        Some(f) => crate::libelles_courts::lire(&ctx.racine)?.formats.remove(&f),
        None => None,
    };
    Ok(Envoi { plan, ctx, periode, sections, borne, courts, depasses: RefCell::new(BTreeMap::new()) })
}

/// Le fichier des ecritures, et celui des comptes quand le format en a un : (son nom, octets, conseil).
type Fabrique<'a> = (Vec<u8>, Option<(&'a NomDExport, Vec<u8>, &'static str)>);

/// Les fichiers d'un format, fabriques sans rien ecrire ; ce qui depasse sa zone est note dans `env`.
fn fabriquer<'a>(ec: &'a Ecrivain, ecr: &[Ecriture], env: &Envoi) -> R<Fabrique<'a>> {
    let ecritures = (ec.ecrire)(ecr, env)?;
    let comptes = match &ec.comptes {
        Some((modele, fabrique, conseil)) => Some((modele, fabrique(&comptes_utilises(ecr, env.plan), env)?, *conseil)),
        None => None,
    };
    Ok((ecritures, comptes))
}

/// CE QUI NE TIENT PAS DANS LES ZONES DE `format`, sur tous les mois de la compagnie : chaque libelle et chaque intitule
/// de compte trop long, avec sa valeur courte validee si elle tient. Les memes ecrivains que l'export, qui ne s'ecrit
/// pas : la page des libelles courts lit ce que l'export refuserait.
pub fn trop_longs(ctx: &Contexte, format: FormatDExport, departs: &engagement::departs::Departs) -> R<Vec<Depasse>> {
    let Some(ec) = ecrivain(format) else {
        return Err(socle::refus!("Le format d'export « {} » n'existe pas.", format));
    };
    // Un format qui ne borne pas ses zones n'a rien de trop long.
    if FormatBorne::de(format).is_none() {
        return Ok(Vec::new());
    }
    let (plan, _) = crate::plan_comptable::plan(ctx)?;
    // Mois par mois : un mois dont les ecritures se refusent (sa paie incomplete) ne s'exporte pas, et son refus est
    // celui de son export : il se dit ici aussi.
    let mut ecr = Vec::new();
    let periode = Periode::nouvelle(ctx, None, None, true, false)?.mois;
    // Les libelles des seules ecritures fabriquees : une paie reprise qu'elles ne portent pas n'a pas de libelle ici.
    for &m in &periode {
        ecr.extend(ecritures_du_mois(ctx, m, &plan, departs)?.0);
    }
    let env = envoi(ctx, &plan, &periode, None, format)?;
    fabriquer(&ec, &ecr, &env)?;
    let depasses = env.depasses.into_inner();
    Ok(depasses.into_values().collect())
}

/// Le format des ecritures du logiciel retenu par la compagnie (`config/compta.json`), s'il en est un.
pub fn format_retenu(ctx: &Contexte) -> R<Option<FormatDExport>> {
    let retenu = ChoixCompta::lu(&ctx.racine)?;
    Ok(retenu.retenu().and_then(logiciel_connu).map(|l| l.format))
}

// ------------------------------------------------------------------ le total par compte

/// LE TOTAL D'UN COMPTE sur les ecritures d'une periode : son libelle (celui que le plan lui donne, `libelle_de`), ce
/// qu'il porte au debit et au credit. Le grand livre et la balance le lisent.
struct TotalDuCompte {
    libelle: Option<String>,
    debit: Montant,
    credit: Montant,
}

impl TotalDuCompte {
    /// Son solde : le debit moins le credit.
    fn solde(&self) -> Montant {
        self.debit - self.credit
    }

    /// Son solde dit « débiteur », « créditeur » ou « nul », la virgule decimale.
    fn solde_dit(&self) -> String {
        let v = self.solde();
        if v > Montant::ZERO {
            format!("débiteur {}", socle::texte::a_virgule(v))
        } else if v < Montant::ZERO {
            format!("créditeur {}", socle::texte::a_virgule(-v))
        } else {
            "nul".to_string()
        }
    }
}

/// Le total de chaque compte que les ecritures mouvementent (`compte_de`), dans l'ordre des numeros.
fn totaux_par_compte(ecritures: &[Ecriture], plan: &Plan) -> BTreeMap<NumeroDeCompte, TotalDuCompte> {
    let mut par: BTreeMap<NumeroDeCompte, TotalDuCompte> = BTreeMap::new();
    for e in ecritures {
        let total = par.entry(compte_de(e).clone()).or_insert_with(|| TotalDuCompte { libelle: libelle_de(plan, e), debit: Montant::ZERO, credit: Montant::ZERO });
        total.debit += e.debit;
        total.credit += e.credit;
    }
    par
}

// ------------------------------------------------------------------ le grand livre

/// `grand_livre` : les ecritures de paie rangees par compte, un sous-total par compte (son total, `totaux_par_compte`).
pub fn grand_livre(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let p = Periode::de(ctx, d, true)?;
    let mut avert = Vec::new();
    let (ecr, plan, origine) = ecritures_de(ctx, &p, &mut avert, &d.departs)?;
    let mut par: BTreeMap<NumeroDeCompte, Vec<Ligne>> = BTreeMap::new();
    for e in &ecr {
        par.entry(compte_de(e).clone()).or_default().push(ligne_de(e, &plan));
    }
    let totaux = totaux_par_compte(&ecr, &plan);
    let mut t = Tableau::nouveau(EtatDExport::GrandLivrePaie, "Grand livre des comptes de paie", p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    // Une section par compte : son numero, et son libelle quand le plan le nomme ; son sous-total, celui du compte (un
    // montant qu'aucune de ses lignes ne porte reste vide, comme dans ses lignes), et son solde.
    let mut sections = Vec::new();
    for (c, lignes) in par {
        let total = totaux.get(&c).ok_or_else(|| erreur(Classe::DefautDuLogiciel, format!("Défaut du logiciel : le compte {} n'a pas de total.", c)))?;
        let libelle = [Some(c.to_string()), total.libelle.clone()].into_iter().flatten().collect::<Vec<_>>().join(" ");
        let mut sous_total = Ligne::nouvelle();
        sous_total.om(C::Debit, Some(total.debit).filter(|m| !m.est_nul())).om(C::Credit, Some(total.credit).filter(|m| !m.est_nul()));
        sous_total.t(C::Compte, format!("Total {}", libelle)).t(C::Analytique, format!("solde {}", total.solde_dit()));
        sections.push(Section { libelle, lignes, total: Some(sous_total) });
    }
    t.sections = sections;
    t.colonnes = vec![
        C::Compte.colonne(),
        C::Date.colonne(),
        C::Piece.colonne(),
        C::Libelle.colonne(),
        C::Analytique.colonne(),
        C::Debit.colonne(),
        C::Credit.colonne(),
    ];
    t.sources = vec![SOURCES.to_string(), format!("plan : {}", origine)];
    t.format = format_possible(d.format, &FORMATS_TABLEAU)?;
    totaliser_general(&mut t, TOTAL_GENERAL);
    if d.ecrire {
        ecrire_tableau(&mut t, &p.dossier(ctx), &format!("grand-livre-paie-{}", p.nom()), ctx)?;
    }
    Ok(t)
}

// ------------------------------------------------------------------ la balance

/// `balance` : une ligne par compte -- total debit, total credit, solde. Le total general tombe a zero.
pub fn balance(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let p = Periode::de(ctx, d, true)?;
    let mut avert = Vec::new();
    let (ecr, plan, origine) = ecritures_de(ctx, &p, &mut avert, &d.departs)?;
    let zero = Montant::ZERO;
    let mut lignes = Vec::new();
    for (c, total) in totaux_par_compte(&ecr, &plan) {
        let v = total.solde();
        let mut l = Ligne::nouvelle();
        l.t(C::Compte, c.to_string()).ot(C::Libelle, total.libelle).montant(C::Debit, total.debit).montant(C::Credit, total.credit);
        l.om(C::SoldeDebiteur, Some(v).filter(|v| *v > zero));
        l.om(C::SoldeCrediteur, Some(-v).filter(|_| v < zero));
        lignes.push(l);
    }
    let mut t = Tableau::nouveau(EtatDExport::BalancePaie, "Balance des comptes de paie", p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    t.sections = vec![Section::nouvelle("", lignes)];
    t.colonnes = vec![
        C::Compte.colonne(),
        C::Libelle.colonne(),
        C::Debit.titree("Total débit"),
        C::Credit.titree("Total crédit"),
        C::SoldeDebiteur.colonne(),
        C::SoldeCrediteur.colonne(),
    ];
    t.sources = vec![SOURCES.to_string(), format!("plan : {}", origine)];
    t.format = format_possible(d.format, &FORMATS_TABLEAU)?;
    totaliser(&mut t, TOTAL_GENERAL);
    if d.ecrire {
        ecrire_tableau(&mut t, &p.dossier(ctx), &format!("balance-paie-{}", p.nom()), ctx)?;
    }
    Ok(t)
}

// ------------------------------------------------------------------ la provision de conges payes

/// Le taux REEL des cotisations patronales d'un salarie -- ses cotisations patronales sur son
/// brut, sur les douze mois qui finissent a `jusqu_a`. Rend (taux en %, nombre de bulletins lus).
fn taux_patronal(lignes: &[crate::etats_paie::LigneLivre], cle: &droit::contexte::IdSalarie, jusqu_a: Mois) -> R<(Option<Taux>, usize)> {
    let depart = jusqu_a.avant(socle::date::Duree::mois(12)).map_err(|x| x.reclasser(Classe::Illisible))?;
    // Les bulletins du salarie sur les douze mois qui disent leur brut et leurs cotisations patronales.
    let c = crate::etats_paie::CumulDuLivre::des(
        lignes.iter().filter(|l| l.brut.is_some() && l.cotisations_patronales.is_some() && l.cle_salarie == *cle && depart < l.mois && l.mois <= jusqu_a),
    );
    // Montant / montant = coefficient ; sans brut, pas de taux, et les bulletins lus restent ceux qu'ils sont.
    let Some(part) = c.cotisations_patronales.montant.rapport(c.brut.montant) else { return Ok((None, c.bulletins)) };
    Ok((Some(part.en_taux().a_deux_decimales()), c.bulletins))
}

/// LA PROVISION D'UNE ANNEE REPRISE D'UN ANCIEN LOGICIEL, au dernier jour du dernier mois repris `m` : celle de chaque
/// salarie au mois dont le contrat repris court au-dela. Ses jours sont ceux que la personne a recopies du dernier bulletin
/// de l'ancien logiciel dans sa fiche (`conges_reprise`, dates du premier jour paye ici, que « Pour démarrer » demande :
/// `declarations::demarrage`), soit son compteur a ce jour-la (`engagement::conges::compteur_a_la_reprise`). L'assiette du
/// dixieme est « la rémunération brute totale perçue par le salarié au cours de la période de référence » (C. trav.
/// L3141-24, I, `code-du-travail/articles-20260919.md`) : la somme des bruts que ses bulletins repris impriment sur la
/// periode, et sur celle du report pour les jours reportes, quand les bulletins repris couvrent la periode entiere ;
/// sinon la remuneration que la fiche recopie. INFERENCES : le brut imprime est cette remuneration (le bulletin ne
/// detaille pas ce que L3141-24 ajoute ou retire) ; le maintien (L3141-24, II) prend pour salaire mensuel le brut du
/// dernier mois repris.
fn provision_d_une_annee_importee(
    ctx: &Contexte,
    d: &Demande,
    p: &Periode,
    m: Mois,
    a_provisionner: &mut Vec<(droit::contexte::IdSalarie, Option<engagement::numeros::NumeroContrat>, String, engagement::conges::Provision)>,
    avert: &mut Vec<String>,
) -> R<usize> {
    let fin = m.dernier_jour();
    let lendemain = fin.apres(socle::date::Duree::jours(1)).map_err(|x| x.reclasser(Classe::Illisible))?;
    let importes: Vec<Mois> = p.dispo.iter().filter(|(_, s)| **s == crate::etats_paie::OrigineDuMois::Importe).map(|(k, _)| *k).collect();
    // `m` est un mois repris : il y en a au moins un.
    let premier_importe = importes.first().copied().ok_or_else(|| erreur(Classe::Refuse, format!("{} : aucun mois repris d'un ancien logiciel.", m)))?;
    let mut lecture = crate::etats_paie::Lecture::nouvelle(ctx, &d.departs);
    let mut sans_compteur = 0;
    let cle_de = |p: &engagement::importe::PaieImportee| p.salarie.id.clone();
    // Les salaries dont un contrat repris court au-dela du dernier jour repris : il contient le lendemain (un contrat
    // repris commence au plus tard au dernier mois repris, qui l'atteste).
    let continuent: BTreeSet<droit::contexte::IdSalarie> = engagement::importe::contrats_lus(&ctx.racine)
        .map_err(|x| x.reclasser(Classe::Refuse))?
        .into_iter()
        .filter(|c| c.periode.is_none_or(|p| p.contient(&lendemain)))
        .map(|c| c.salarie)
        .collect();
    let paies = lecture.paies(m)?;
    // Les bruts repris d'un salarie du mois de `du` a celui de `au`, et si les bulletins repris couvrent ces mois ; un
    // brut que les pieces taisent, la somme ne se fait pas (`None`).
    fn bruts(
        lecture: &mut crate::etats_paie::Lecture, importes: &[Mois], premier_importe: Mois, cle: &droit::contexte::IdSalarie, du: &socle::date::Jour, au: &socle::date::Jour,
    ) -> R<Option<(Montant, bool)>> {
        let mut total = Montant::ZERO;
        for &mois in importes.iter().filter(|x| **x >= du.mois_civil() && **x <= au.mois_civil()) {
            for q in lecture.paies(mois)?.iter().filter_map(engagement::etats::PaieDuMois::importee).filter(|q| q.salarie.id == *cle) {
                let Some(brut) = q.brut else { return Ok(None) };
                total += brut;
            }
        }
        Ok(Some((total, premier_importe <= du.mois_civil())))
    }
    // UNE PAIE REPRISE DONT LA NATURE RESTE A DIRE n'est pas comptee (`engagement::importe::NatureADire`) : la provision du
    // salarie qui en a une sur ses periodes ne se fait pas, et se dit, sur lui seul ; les autres se provisionnent.
    let pas_comptee = |cle: &droit::contexte::IdSalarie, manquent: &[engagement::importe::NatureADire]| -> String {
        let mois: Vec<String> = manquent.iter().map(|a| a.mois.libelle(socle::mois::Graphie::Accentuee)).collect();
        format!(
            "{} : sa paie reprise de {} n'est pas comptée, la nature de son contrat reste à dire (Configuration, « Reprendre l'historique ») : sa provision ne se calcule pas sans elle, il n'est pas dans la provision.",
            ctx.nomme(cle),
            mois.join(", ")
        )
    };
    // Les salaries deja vus : un salarie se provisionne, ou se dit, une fois.
    let mut vus: BTreeSet<droit::contexte::IdSalarie> = a_provisionner.iter().map(|(c, ..)| c.clone()).collect();
    // Les paies a dire que la phrase d'un salarie a deja nommees (`pas_comptee`) : elles ne se redisent pas.
    let mut dites: Vec<engagement::importe::NatureADire> = Vec::new();
    for pa in paies.iter().filter_map(engagement::etats::PaieDuMois::importee) {
        let nat = pa.engagement.nat().map_err(|x| x.reclasser(Classe::Refuse))?;
        let cle = cle_de(pa);
        let Some(au) = nat.au_mois().filter(|_| continuent.contains(&cle)) else {
            continue;
        };
        if !vus.insert(cle.clone()) {
            continue;
        }
        let date = ctx.salaries.get(&cle).and_then(|f| f.permanent.ouvrir(&au).conges_reprise.dite()).and_then(|r| r.date.dite()).map(|j| j.iso());
        let Some(mut cpt) = engagement::conges::compteur_a_la_reprise(ctx, &au, &cle).map_err(|x| x.reclasser(Classe::Refuse))?.filter(|_| date == Some(lendemain.iso())) else {
            avert.push(format!(
                "{} : sa fiche ne dit pas ses congés payés au {} (« Congés payés à la reprise », recopiés de son dernier bulletin repris) : son compteur ne se lit pas, il n'est pas dans la provision.",
                ctx.nomme(&cle),
                lendemain.jj_mm_aaaa()
            ));
            sans_compteur += 1;
            continue;
        };
        // Ses paies de la periode, du report et du mois : celle qui ne se range pas manque a ses sommes.
        let mut du = cpt.periode.debut().mois_civil();
        if let Some(r) = cpt.report {
            du = du.min(r.debut().mois_civil());
        }
        let manquent = lecture.a_dire_de(&cle, du.max(premier_importe), m)?;
        if !manquent.is_empty() {
            avert.push(pas_comptee(&cle, &manquent));
            dites.extend(manquent);
            sans_compteur += 1;
            continue;
        }
        // Un bulletin repris qui ne dit pas son brut ne couvre pas sa periode : la remuneration est celle que la fiche recopie.
        if let Some((periode, true)) = bruts(&mut lecture, &importes, premier_importe, &cle, &cpt.periode.debut(), &fin)? {
            if !cpt.remuneration_periode.est_nul() && cpt.remuneration_periode != periode {
                avert.push(format!(
                    "{} : la rémunération de la période que sa fiche recopie ({}) n'est pas la somme des bruts de ses bulletins repris ({}) ; la provision prend les bulletins.",
                    ctx.nomme(&cle),
                    euros(cpt.remuneration_periode),
                    euros(periode)
                ));
            }
            cpt.remuneration_periode = periode;
        }
        if let Some(r) = cpt.report {
            if let Some((report, true)) = bruts(&mut lecture, &importes, premier_importe, &cle, &r.debut(), &r.fin())? {
                cpt.remuneration_report = report;
            }
        }
        // Le salaire du maintien : le brut du dernier mois repris ; un bulletin qui ne le dit pas, il ne se lit pas.
        let Some(salaire) = paies.iter().filter_map(engagement::etats::PaieDuMois::importee).filter(|q| cle_de(q) == cle).map(|q| q.brut).try_fold(Montant::ZERO, |a, b| b.map(|b| a + b)) else {
            avert.push(format!(
                "{} : un de ses bulletins repris de {} ne dit pas son brut : son salaire du mois ne se lit pas, il n'est pas dans la provision.",
                ctx.nomme(&cle),
                m
            ));
            sans_compteur += 1;
            continue;
        };
        let provision = engagement::conges::provision_du_compteur(cpt, m, salaire.pour_une_unite()).map_err(|x| x.reclasser(Classe::Refuse))?;
        a_provisionner.push((cle, pa.engagement.numero.clone(), nat.libelle.clone(), provision));
    }
    // Le salarie dont le contrat repris qui continue n'a, en `m`, qu'une paie qui ne se range pas : sans sa nature, rien ne
    // dit qu'il est paye au mois ; il n'est pas dans la provision, et le dit.
    for cle in continuent.iter().filter(|c| !vus.contains(*c)) {
        let manquent = lecture.a_dire_de(cle, m, m)?;
        if !manquent.is_empty() {
            avert.push(pas_comptee(cle, &manquent));
            dites.extend(manquent);
            sans_compteur += 1;
        }
    }
    avert.extend(lecture.a_dire().iter().filter(|a| !dites.contains(a)).map(engagement::importe::NatureADire::phrase));
    Ok(sans_compteur)
}

/// `provision_conges` : la dette de conges payes des salaries au mois a la fin de la periode -- ce que la compagnie
/// devrait s'ils partaient ce jour-la. Un salarie, une ligne ; les intermittents n'y sont pas.
pub fn provision_conges(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    if d.toutes {
        return Err(refus("La provision de congés se calcule à UNE date : demander un mois (la fin du mois) ou une année (la fin du dernier mois produit)."));
    }
    let p = Periode::de(ctx, d, true)?;
    let mut avert: Vec<String> = Vec::new();
    let zero = Montant::ZERO;
    let cle_de = |p: &engagement::calcul::Paie| p.salarie.id.clone();
    // CE QUI SE PROVISIONNE, salarie par salarie : (cle, contrat, nature, compteur et indemnite a la fin du mois `m`).
    let mut a_provisionner: Vec<(droit::contexte::IdSalarie,Option<engagement::numeros::NumeroContrat>, String, engagement::conges::Provision)> = Vec::new();
    let saisis = p.saisis();
    let mut sans_compteur = 0;
    let m = match saisis.last().copied() {
        Some(m) => {
            let fin = m.dernier_jour();
            // Ce qui a ete paye ce mois-la : les paies ecrites de ses engagements valides, la saisie au meme rang.
            let (saisie, paies) = charger_mois(ctx, m, &d.departs)?;
            for (e, pa) in saisie.engagements.iter().zip(paies.iter()) {
                let nat = pa.engagement.nat().map_err(|x| x.reclasser(Classe::Refuse))?;
                // Un contrat fini ce mois-ci a eu son solde de tout compte : les conges non pris y sont payes, il n'y a plus de dette.
                if !nat.mensuelle() || pa.engagement.contrat().fin().is_some_and(|f| f <= fin) {
                    continue;
                }
                let cle = cle_de(pa);
                if a_provisionner.iter().any(|(c, ..)| *c == cle) {
                    continue;
                }
                let brut_du_mois = paies.iter().filter(|q| cle_de(q) == cle).fold(Montant::ZERO, |a, q| a + q.brut);
                let Some(cpt) = engagement::conges::provision_conges_de(pa, m, brut_du_mois, e.salaire_mensuel.as_ref())? else { continue };
                a_provisionner.push((cle, e.numero.clone(), nat.libelle.clone(), cpt));
            }
            m
        }
        None => {
            let Some(m) = p.importes().last().copied() else {
                return Err(refus(format!("Aucun mois saisi ni repris d'un ancien logiciel sur la période ({}) : il n'y a pas de congés à provisionner.", p.libelle)));
            };
            sans_compteur = provision_d_une_annee_importee(ctx, d, &p, m, &mut a_provisionner, &mut avert)?;
            m
        }
    };
    let fin = m.dernier_jour();
    let toutes = Periode::nouvelle(ctx, None, None, true, true)?;
    // Le livre de toutes les annees, pour le taux de chaque salarie : ce qu'il dit des autres annees n'est pas de cet etat ;
    // ce qui manque au taux d'un salarie se dit sur lui (`a_dire_de`).
    let livre = lignes_livre(ctx, &toutes, &mut Vec::new(), &d.departs)?;
    let mut lecture = crate::etats_paie::Lecture::nouvelle(ctx, &d.departs);
    let depart_du_taux = m.avant(socle::date::Duree::mois(12)).map_err(|x| x.reclasser(Classe::Illisible))?.suivant();
    let mut lignes: Vec<Ligne> = Vec::new();
    // Ce que l'ecriture de la provision porte : les indemnites, et les charges que le taux reel permet de calculer.
    let (mut indemnites, mut charges_dites) = (zero, zero);
    for (cle, contrat, nature, cpt) in a_provisionner {
        // Le salarie, tel que l'ecran le nomme.
        let qui = ctx.nomme(&cle);
        for a in &cpt.avertissements {
            // Un avis du compteur nomme deja son salarie.
            avert.push(if a.texte.starts_with(&format!("{} :", cle)) { a.texte.clone() } else { format!("{} : {}", qui, a.texte) });
        }
        let (dixieme, maintien, retenu, regle) = match &cpt.indemnite {
            Some(i) => (i.dixieme, i.maintien, i.retenu, i.regle.to_string()),
            None => {
                if cpt.solde < Jours::ZERO {
                    avert.push(format!("{} : compteur négatif ({} jours) — des congés pris par anticipation ; aucune dette, et une avance à régler.", qui, sans_zero::dit(cpt.solde.au_demi_superieur())));
                }
                (zero, zero, zero, String::new())
            }
        };
        // Une paie reprise des douze mois dont la nature reste a dire n'est pas au livre : le taux de ce salarie ne se lit
        // pas sans elle, et se dit sur lui.
        let manquent = lecture.a_dire_de(&cle, depart_du_taux, m)?;
        let (taux, n) = match manquent.is_empty() {
            true => taux_patronal(&livre, &cle, m)?,
            false => (None, 0),
        };
        match (taux, n) {
            _ if !manquent.is_empty() => avert.push(format!(
                "{} : sa paie reprise de {} n'est pas comptée, la nature de son contrat reste à dire — le taux des charges patronales ne se lit pas sans elle, la colonne reste vide.",
                qui,
                manquent.iter().map(|a| a.mois.libelle(socle::mois::Graphie::Accentuee)).collect::<Vec<_>>().join(", ")
            )),
            (Some(_), _) => {}
            (None, 0) => avert.push(format!("{} : aucun bulletin de PuppetPaie sur les douze derniers mois — le taux des charges patronales ne se lit pas, la colonne reste vide.", qui)),
            (None, _) => avert.push(format!("{} : ses bulletins des douze derniers mois ne portent aucun brut — le taux des charges patronales ne se lit pas, la colonne reste vide.", qui)),
        }
        let charges = taux.map(|t| retenu.fois_taux(t).au_centime());
        indemnites += retenu;
        if let Some(c) = charges {
            charges_dites += c;
        }
        let fiche = ctx.salaries.get(&cle).ok_or_else(|| socle::fichiers::manque(cle.as_str()))?;
        let nom = fiche.nom.exigee("nom")?;
        let mut l = Ligne::nouvelle();
        l.t(C::Salarie, match fiche.prenom.portee() {
            Some(prenom) => format!("{} {}", nom, prenom),
            None => nom.to_string(),
        });
        l.ot(C::Contrat, contrat).t(C::Nature, nature);
        l.grandeur(C::Report, cpt.report_restant.au_demi_superieur()).grandeur(C::Acquis, cpt.acquis.au_demi_superieur());
        l.grandeur(C::Pris, cpt.pris.au_demi_superieur()).grandeur(C::Solde, cpt.solde.au_demi_superieur());
        l.montant(C::Dixieme, dixieme).montant(C::Maintien, maintien).montant(C::Indemnite, retenu).t(C::Regle, regle);
        // Le taux s'ecrit a deux decimales (`12.50`).
        l.taux(C::TauxPatronal, taux.map(|t| socle::grandeurs::decimales_ecrites(t, 2))).compte(C::BulletinsDuTaux, n).om(C::Charges, charges);
        l.om(C::Total, charges.map(|c| retenu + c));
        lignes.push(l);
    }
    if lignes.is_empty() && sans_compteur == 0 {
        // RIEN A PROVISIONNER N'EST PAS UNE ERREUR : le comptable qui la demande a sa reponse -- zero, et pourquoi.
        avert.push(format!(
            "Aucun salarié au mois sous contrat au {} : pas de congés payés à provisionner (les intermittents relèvent de la Caisse des Congés Spectacles, à laquelle la compagnie cotise chaque mois).",
            jj(&fin.iso())
        ));
    }
    let (plan, _origine) = crate::plan_comptable::plan(ctx)?;
    let pc = &plan.provision_conges;
    avert.push(format!(
        "Écriture au {} (journal OD) : {} {} au débit, {} {} au crédit, {} ; {} {} au débit, {} {} au crédit, {}. À extourner le premier jour de l'exercice suivant — la dette se paie au fil des congés pris, ou au solde de tout compte.",
        jj(&fin.iso()),
        pc.conges.compte,
        pc.conges.libelle,
        pc.dette.compte,
        pc.dette.libelle,
        euros(indemnites),
        pc.charges.compte,
        pc.charges.libelle,
        pc.dette_charges.compte,
        pc.dette_charges.libelle,
        euros(charges_dites)
    ));
    let mut t = Tableau::nouveau(EtatDExport::ProvisionConges, format!("Provision pour congés payés au {}", jj(&fin.iso())), p.libelle.clone());
    t.mois = vec![m];
    t.avertissements = avert;
    t.sections = vec![Section::nouvelle("", lignes)];
    t.colonnes = vec![
        C::Salarie.colonne(),
        C::Contrat.colonne(),
        C::Nature.colonne(),
        C::Report.colonne(),
        C::Acquis.colonne(),
        C::Pris.colonne(),
        C::Solde.colonne(),
        C::Dixieme.colonne(),
        C::Maintien.colonne(),
        C::Indemnite.colonne(),
        C::Regle.colonne(),
        C::TauxPatronal.colonne(),
        C::Charges.colonne(),
        C::Total.titree("Provision"),
    ];
    t.sources = vec!["compteur de congés écrit dans la paie validée du mois, indemnité L3141-24 comme au solde de tout compte, taux patronal = cotisations patronales / brut des bulletins des douze derniers mois".to_string()];
    // Les jours, le taux et le nombre de bulletins d'un salarie ne s'additionnent pas : leurs colonnes n'ont pas de total.
    finir(ctx, &mut t, Some(&p), "provision-conges", d.format,false)?;
    if d.ecrire {
        ecrire_fini(ctx, &mut t, Some(&p), "provision-conges")?;
    }
    Ok(t)
}

// ------------------------------------------------------------------ la taxe sur les salaires

/// `taxe_sur_les_salaires` : la taxe sur les salaires d'une annee (CGI art. 231), salarie par salarie, et ce qui
/// reste a payer apres franchise, decote et abattement. Se calcule sur une ANNEE (ses seuils sont annuels).
pub fn taxe_sur_les_salaires(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let Some(annee) = d.annee else {
        return Err(socle::echec::cle("La taxe sur les salaires se calcule sur une ANNÉE (ses seuils sont annuels) : choisir l'année.", "annee"));
    };
    // Qui doit la taxe, et sur quelle part : le meme regime que la DSN de decembre (`assujettissement`, CGI art. 231, 1).
    // NE PAS SAVOIR N'EST PAS UNE ERREUR : l'etat le dit, et dit quoi repondre.
    let jour = annee.premier_jour();
    let bar = ctx.bareme(&jour)?;
    // Les plafonds de la franchise, que la conclusion nomme : ceux du bareme de l'annee.
    let plafonds = crate::taxe_sur_les_salaires::plafonds_franchise(Some(&*bar));
    // La fiche telle qu'elle est au 31 decembre de l'annee : celle qu'`exports-periodes` lit pour proposer l'etat.
    let fin = annee.dernier_jour();
    let regime = crate::taxe_sur_les_salaires::regime_taxe_sur_les_salaires(&ctx.entreprise_au(&fin), plafonds);
    let p = Periode::nouvelle(ctx, None, Some(annee), false, true)?;
    let mut avert: Vec<String> = Vec::new();
    use crate::taxe_sur_les_salaires::Decision;
    let part = match &regime.decision {
        Decision::DueSur(part) => Some(*part),
        Decision::NonDue => None,
        Decision::ADire(m) => {
            avert.push(m.clone());
            None
        }
    };
    if regime.decision == Decision::NonDue {
        avert.push(format!("{} Rien à calculer.", regime.consequence));
    }
    let ts = match bar.taxe_sur_les_salaires.clone() {
        Some(t) => t,
        None => {
            return Err(refus(format!(
                "Le barème {} ne porte pas la taxe sur les salaires (seuils, franchise, abattement) : ils se lisent sur la notice 2501-SD de l'année.",
                annee
            )))
        }
    };
    let zero = Montant::ZERO;
    // cle -> (salarie, brut, base, mois) dans l'ordre d'insertion
    let mut ordre: Vec<String> = Vec::new();
    let mut par: BTreeMap<String, (String, Montant, Montant, BTreeSet<Mois>)> = BTreeMap::new();
    // Un mois qui ne se charge pas, une paie importee que l'etat ne compte pas (ses pieces taisent son brut ou son assiette
    // de CSG) : le total est alors incomplet, et, la ou un montant se calcule (une part taxee dite), le calcul comme sa
    // conclusion le redisent.
    let mut ecartees = false;
    for m in p.mois.clone() {
        let paies = match engagement::etats::paies_du_mois(ctx, m, &d.departs).map_err(|x| x.reclasser(Classe::Refuse)) {
            Ok((paies, a_dire)) => {
                // Une paie reprise dont la nature reste a dire ne se compte pas : le total est incomplet, et l'etat la nomme.
                ecartees |= !a_dire.is_empty();
                avert.extend(a_dire.iter().map(engagement::importe::NatureADire::phrase));
                paies
            }
            Err(e) if matches!(e.classe, Some(Classe::Refuse) | Some(Classe::Illisible) | Some(Classe::Manque) | Some(Classe::FichierAbsent)) => {
                avert.push(format!("{} : le mois ne se charge pas ({}) — écarté.", m, e.phrase));
                ecartees = true;
                continue;
            }
            Err(e) => return Err(e),
        };
        for pa in &paies {
            // Un engagement valide sans ligne ne paie rien. Une paie importee dont aucune ligne n'est rattachee a une
            // rubrique a pourtant son brut : elle entre dans l'assiette, ou la taxe se refuse en la nommant.
            if pa.importee().is_none() && pa.sans_paie() {
                continue;
            }
            // L'assiette de la CSG, sans la reduction de 1,75 % (CGI 231) : la meme que la DSN de decembre declare
            // (`assujettissement`, bloc S21.G00.44). Le brut ou l'assiette de la CSG que les pieces d'une paie importee
            // taisent ne s'invente pas : cette paie ne se compte pas, l'etat la nomme avec ce qui lui manque et rend les
            // autres (« Le logiciel est doux »).
            let (brut, base) = (pa.brut(), crate::taxe_sur_les_salaires::assiette_d_une_paie_du_mois(pa, &bar)?);
            let (Some(brut), Some(base)) = (brut, base) else {
                let manque = match (brut, base) {
                    (None, None) => "son brut ni son assiette de CSG",
                    (None, Some(_)) => "son brut",
                    _ => "son assiette de CSG",
                };
                ecartees = true;
                avert.push(format!(
                    "{}, {} : les pièces importées ne disent pas {} ; cette paie n'est pas comptée ci-dessous.",
                    crate::tableaux_export::qui_souple(pa.salarie()),
                    m,
                    manque
                ));
                continue;
            };
            let cle = pa.salarie().id.to_string();
            if !par.contains_key(&cle) {
                ordre.push(cle.clone());
                par.insert(cle.clone(), (crate::tableaux_export::qui_souple(pa.salarie()), zero, zero, BTreeSet::new()));
            }
            let x = par.get_mut(&cle).expect("insere");
            x.1 += brut;
            x.2 += base;
            x.3.insert(m);
        }
    }
    let (s1, s2) = (ts.seuil_1, ts.seuil_2);
    let mut rangs: Vec<&String> = ordre.iter().collect();
    rangs.sort_by(|a, b| par[*a].0.cmp(&par[*b].0));
    let mut lignes = Vec::new();
    let mut brute = zero;
    for cle in rangs {
        let (nom, brut, base, mois) = &par[cle];
        let b = *base;
        let normal = b.fois_taux(ts.taux_normal).au_centime();
        let (au_dela_1, au_dela_2) = crate::taxe_sur_les_salaires::fractions_majorees(b, s1, s2);
        let m1 = au_dela_1.fois_taux(ts.majoration_1).au_centime();
        let m2 = au_dela_2.fois_taux(ts.majoration_2).au_centime();
        let taxe = normal + m1 + m2;
        let mut l = Ligne::nouvelle();
        l.t(C::Salarie, nom.clone()).compte(C::NombreDeMois, mois.len()).montant(C::Brut, *brut).montant(C::BaseDeLaTaxe, b).montant(C::TauxNormal, normal).montant(C::MajorationUn, m1).montant(C::MajorationDeux, m2).montant(C::Taxe, taxe);
        brute += taxe;
        lignes.push(l);
    }
    // Sans part taxee dite (non assujettie, ou le regime ne se lit pas : l'avertissement le dit), rien n'est du.
    let (pct_a, due) = match part {
        Some(pct) => (pct, brute.fois_taux(pct).a_l_euro_au_pair()),
        None => (Taux::ZERO, zero),
    };
    // Sans part taxee, rien ne se calcule : il n'y a pas de montant a dire incomplet.
    let incomplet = ecartees && part.is_some();
    let franchise = ts.franchise;
    let limite = ts.decote_jusqu_a;
    let mut etapes = vec![format!(
        "taxe brute {} × {} % taxés (CGI art. 231, 1) = {}",
        euros(brute),
        socle::texte::virgule(pct_a),
        euros(due)
    )];
    let mut net;
    if due <= franchise {
        net = zero;
        let rien = if incomplet { "" } else { " — rien à payer" };
        etapes.push(format!("franchise : {} n'excède pas {}{}", euros(due), euros(ts.franchise), rien));
    } else if due <= limite {
        let decote = (limite - due).fois(3).reparti_sur(4).a_l_euro_au_pair();
        net = due - decote;
        etapes.push(format!(
            "décote : ({} − {}) × 3/4 = {}, reste {}",
            euros(ts.decote_jusqu_a),
            euros(due),
            euros(decote),
            euros(net)
        ));
    } else {
        net = due;
    }
    if regime.association && net > zero {
        let reste = net - ts.abattement_1679a;
        let reste = if reste > zero { reste } else { zero };
        etapes.push(format!("abattement des associations (art. 1679 A) : {}, reste {}", euros(ts.abattement_1679a), euros(reste)));
        net = reste;
    }
    etapes.push(format!("taxe nette due : {}", euros(net)));
    let sans = if incomplet { ", sans ce qui est écarté ci-dessus" } else { "" };
    avert.push(format!("Calcul {}{} : {}.", annee, sans, etapes.join(" ; ")));
    if incomplet {
        avert.push(format!(
            "Taxe nette sans ce qui est écarté ci-dessus : {} ; ce qui est à payer et à déposer se lit quand plus rien n'est écarté.",
            euros(net)
        ));
    } else if net.est_nul() {
        avert.push("Rien à payer : pas de relevé 2501-SD ni de déclaration 2502-SD à déposer (franchise ou abattement).".to_string());
    } else {
        let periodicite = if net < Montant::euros(4000) {
            "un seul versement, déclaration 2502-SD au 15 janvier"
        } else if net <= Montant::euros(10000) {
            "trimestrielle (2501-SD)"
        } else {
            "mensuelle (2501-SD)"
        };
        avert.push(format!("Périodicité l'année suivante : {}.", periodicite));
    }
    let mut t = Tableau::nouveau(EtatDExport::TaxeSurLesSalaires, format!("Taxe sur les salaires {}", annee), p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    t.sections = vec![Section::nouvelle("", lignes)];
    t.colonnes = vec![
        C::Salarie.colonne(),
        C::NombreDeMois.colonne(),
        C::Brut.colonne(),
        C::BaseDeLaTaxe.colonne(),
        C::TauxNormal.colonne(),
        C::MajorationUn.titree(format!("+4,25 % au-delà de {}", euros(ts.seuil_1))),
        C::MajorationDeux.titree(format!("+9,35 % au-delà de {}", euros(ts.seuil_2))),
        C::Taxe.colonne(),
    ];
    t.sources = vec!["CGI art. 231 et 1679 A ; notices 2501-SD et 2502-SD de la DGFiP (seuils, franchise, décote, abattement de l'année)".to_string()];
    finir(ctx, &mut t, Some(&p), "taxe-sur-les-salaires", d.format,false)?;
    if d.ecrire {
        ecrire_fini(ctx, &mut t, Some(&p), "taxe-sur-les-salaires")?;
    }
    Ok(t)
}
