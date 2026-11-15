//! Les etats du personnel et du controle : la liste des contrats, le registre
//! unique du personnel (L1221-13, D1221-23), le fichier du personnel, le journal des declarations.
//!
//! Le registre et le fichier du personnel SONT des fichiers du personnel : ils portent le NIR, la date de
//! naissance et l'adresse, et l'avertissement le dit.

use droit::fiche::Lue as _;
use std::collections::BTreeSet;

use droit::contexte::Contexte;
use socle::echec::Classe;
use socle::date::Jour;
use socle::mois::Mois;
use crate::etats_paie::{finir, titre_spectacle, Demande, Lecture, Periode};
use crate::plan_comptable::EtatDExport;
use socle::grandeurs::Montant;
use crate::tableaux_export::{jj, qui, refus, section_du_mois, sections_de, ColonneDEtat as C, Ligne, Section, Tableau, R};

// ============================================================ 13. les contrats

/// Un contrat par mois de paie, vu des etats du personnel.
struct ContratMois {
    mois: Mois,
    cle: droit::contexte::IdSalarie,
    fiche: droit::fiche::ChampsSalarie,
    salarie: String,
    /// Le numero du contrat, quand il est ecrit (un mois produit l'a fige).
    contrat: Option<engagement::numeros::NumeroContrat>,
    libelle_nature: String,
    emploi: Option<String>,
    spectacle: Option<String>,
    objet: Option<droit::fiche::NumeroObjetSpectacle>,
    debut: Jour,
    /// La fin du contrat, sinon son dernier jour travaille.
    fin: Option<Jour>,
    brut: Option<Montant>,
}

/// Chaque contrat de la periode, une ligne par mois de paie, lu dans les paies du mois (`Lecture::paies` : la paie
/// ecrite d'un engagement valide, ou celle que l'import a lue).
fn contrats_de(p: &Periode, lecture: &mut Lecture) -> R<Vec<ContratMois>> {
    let mut out = Vec::new();
    for m in p.mois.clone() {
        for paie in lecture.paies(m)?.iter() {
            let e = paie.engagement();
            let s = paie.salarie();
            let emploi = e.emploi.clone().filter(|x| !x.is_empty()).or_else(|| s.emploi.en_texte());
            out.push(ContratMois {
                mois: m,
                cle: s.id.clone(),
                fiche: s.champs.clone(),
                salarie: qui(s)?,
                contrat: e.numero.clone(),
                libelle_nature: e.nat().map_err(|x| x.reclasser(Classe::Refuse))?.libelle.clone(),
                emploi,
                spectacle: titre_spectacle(paie),
                objet: paie.spectacle().and_then(|s| s.numero_objet.dite()).cloned(),
                debut: e.contrat.debut(),
                fin: e.contrat.fin().or(e.dates_travail.last().copied()),
                brut: paie.brut().map(Montant::au_centime),
            });
        }
    }
    Ok(out)
}

/// La liste des contrats.
pub fn liste_des_contrats(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let p = Periode::de(ctx, d, true)?;
    let mut avert = Vec::new();
    let mut lecture = Lecture::nouvelle(ctx, &d.departs);
    let contrats = contrats_de(&p, &mut lecture)?;
    let mut lignes = Vec::new();
    for c in &contrats {
        let mut l = Ligne::nouvelle();
        l.m(C::Mois, c.mois).t(C::Cle, c.cle.to_string()).t(C::Salarie, c.salarie.clone()).ot(C::Contrat, c.contrat.clone()).t(C::Nature, c.libelle_nature.clone());
        l.ot(C::Emploi, c.emploi.clone()).ot(C::Spectacle, c.spectacle.clone()).ot(C::Objet, c.objet.as_ref().map(|o| o.to_string())).t(C::Debut, c.debut.jj_mm_aaaa()).ot(C::Fin, c.fin.map(|j| j.jj_mm_aaaa()));
        l.od(C::Brut, c.brut.map(Montant::decimal));
        lignes.push(l);
    }
    if contrats.iter().any(|c| c.contrat.is_none()) {
        avert.push("Contrat sans numéro : son mois n'a pas été produit, aucun numéro n'est figé. Rien n'est inventé.".to_string());
    }
    let mut t = Tableau::nouveau(EtatDExport::Contrats, "Liste des contrats", p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    t.sections = sections_de(lignes, |l| l.mois(C::Mois), section_du_mois, Some(&|l| vec![l.texte(C::Salarie), l.texte(C::Debut), l.texte(C::Contrat)]))?;
    t.colonnes = vec![
        C::Mois.titree("Mois de paie"),
        C::Contrat.colonne(),
        C::Salarie.colonne(),
        C::Nature.colonne(),
        C::Emploi.colonne(),
        C::Spectacle.colonne(),
        C::Objet.colonne(),
        C::Debut.colonne(),
        C::Fin.colonne(),
        C::Brut.titree("Brut du mois"),
    ];
    t.sources = vec!["paies écrites des engagements validés ; paies des engagements importés, telles que l'import les a lues".to_string()];
    finir(ctx, &mut t, Some(&p), "contrats", d.format,d.ecrire)?;
    Ok(t)
}

// ================================================ 14. le registre du personnel

/// `NATIONALITES`.
fn nationalite(code: droit::fiche::CodeNationalite) -> &'static str {
    match code {
        droit::fiche::CodeNationalite::Francaise => "française",
        droit::fiche::CodeNationalite::UnionEuropeenne => "Union européenne",
        droit::fiche::CodeNationalite::HorsUnionEuropeenne => "EEE hors Union européenne",
        droit::fiche::CodeNationalite::Inconnue => "hors Union européenne et EEE",
    }
}

/// Le premier chiffre du NIR (1 homme, 2 femme) ; un NIR provisoire ne le dit pas.
fn sexe(nir: Option<&droit::fiche::Nir>) -> Option<&'static str> {
    match nir?.sexe()? {
        droit::fiche::Sexe::Masculin => Some("M"),
        droit::fiche::Sexe::Feminin => Some("F"),
    }
}

/// Une ligne par embauche, dans l'ordre des embauches, avec les mentions
/// obligatoires. Un intermittent est embauche a chaque contrat : chaque CDDU est une entree et une sortie.
pub fn registre_du_personnel(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let p = Periode::de(ctx, d, true)?;
    let mut avert = Vec::new();
    let mut vus: BTreeSet<(droit::contexte::IdSalarie, Option<engagement::numeros::NumeroContrat>, Jour, Option<Jour>)> = BTreeSet::new();
    let mut a_preciser: BTreeSet<String> = BTreeSet::new();
    // (entree, nom, contrat) : l'ordre des embauches.
    let mut rangees: Vec<((Jour, Option<String>, Option<engagement::numeros::NumeroContrat>), Ligne)> = Vec::new();
    let mut lecture = Lecture::nouvelle(ctx, &d.departs);
    for c in contrats_de(&p, &mut lecture)? {
        // Un contrat qui deborde du mois apparait une fois par mois de paie : c'est UNE embauche.
        if !vus.insert((c.cle.clone(), c.contrat.clone(), c.debut, c.fin)) {
            continue;
        }
        let f = &c.fiche;
        let nom = f.nom_affichage.remplie().or(f.nom.remplie()).map(str::to_string);
        let mut l = Ligne::nouvelle();
        l.ot(C::Nom, nom.clone()).ot(C::Prenom, f.prenom_affichage.remplie().or(f.prenom.remplie()));
        // La codification relue comme la DSN la declare (`droit::nationalite`) : un ancien 03 sans pays ne se lit pas « EEE ».
        let dite = f.code_nationalite_dsn.est_dite() || f.pays_nationalite.remplie().is_some() || f.pays_nationalite_hors_liste.remplie().is_some();
        let categorie = if dite {
            match droit::nationalite::codification_ue(&f.nationalite()) {
                Ok(code) => Some(nationalite(code)),
                Err(pourquoi) => {
                    a_preciser.insert(format!("{} : {}", c.salarie, pourquoi.phrase));
                    Some("à préciser dans la fiche")
                }
            }
        } else {
            None
        };
        // LE PAYS, quand la fiche le porte : le registre dit la nationalite (C. trav.,
        // art. D1221-23, 1°), la categorie DSN la suit entre parentheses ; sans pays, la categorie seule. Un pays hors de
        // la liste du logiciel se dit par son code.
        let pays = match f.pays_nationalite.remplie() {
            Some(code) => match socle::pays::du_code(code).map_err(|m| m.reclasser(Classe::Illisible))? {
                Some(p) => Some(p.nom),
                None => Some(code.to_string()),
            },
            None => f.pays_nationalite_hors_liste.remplie().map(str::to_string),
        };
        l.ot(
            C::Nationalite,
            match (pays, categorie) {
                (Some(pays), Some(cat)) => Some(format!("{} ({})", pays, cat)),
                (Some(pays), None) => Some(pays),
                (None, cat) => cat.map(str::to_string),
            },
        );
        l.ot(C::Naissance, f.date_naissance.dite().map(|j| jj(&j.iso()))).ot(C::Sexe, sexe(f.nir.dite()));
        l.ot(C::Emploi, c.emploi.clone()).ot(C::Qualification, f.statut.dite().map(|s| s.to_string())).t(C::ContratType, c.libelle_nature.clone()).ot(C::Contrat, c.contrat.clone());
        l.t(C::Entree, c.debut.jj_mm_aaaa()).ot(C::Sortie, c.fin.map(|j| j.jj_mm_aaaa()));
        rangees.push(((c.debut, nom, c.contrat.clone()), l));
    }
    rangees.sort_by(|a, b| a.0.cmp(&b.0));
    let mut lignes: Vec<Ligne> = Vec::new();
    for (i, (_, mut l)) in rangees.into_iter().enumerate() {
        l.n(C::Ordre, (i + 1) as i64);
        lignes.push(l);
    }
    avert.push("La nationalité est le pays de nationalité de la fiche, suivi de la catégorie déclarée en DSN (française, Union européenne, EEE hors Union, reste du monde) ; une fiche sans pays n'en montre que la catégorie. Pour un salarié étranger hors UE, le registre doit aussi porter le type et le numéro du titre de travail (art. D1221-24) — à compléter à la main.".to_string());
    avert.extend(a_preciser);
    avert.push("Registre unique du personnel : il porte des données personnelles et se conserve cinq ans après le départ de chaque salarié. Il se présente à l'inspection du travail sur demande.".to_string());
    let mut t = Tableau::nouveau(EtatDExport::RegistreDuPersonnel, "Registre unique du personnel", p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    t.sections = vec![Section::nouvelle("", lignes)];
    t.avec_total = false;
    t.donnees_personnelles = true;
    t.colonnes = vec![
        C::Ordre.colonne(),
        C::Nom.colonne(),
        C::Prenom.colonne(),
        C::Nationalite.colonne(),
        C::Naissance.colonne(),
        C::Sexe.colonne(),
        C::Emploi.colonne(),
        C::Qualification.colonne(),
        C::ContratType.colonne(),
        C::Contrat.colonne(),
        C::Entree.colonne(),
        C::Sortie.colonne(),
    ];
    t.sources = vec!["Code du travail, art. L1221-13 (registre, ordre des embauches), D1221-23 (mentions : nationalité, date de naissance, sexe, emploi, qualification, dates d'entrée et de sortie), D1221-24 (type de contrat, titre de travail), Légifrance, relevé le 23/09/2026".to_string()];
    finir(ctx, &mut t, Some(&p), "registre-du-personnel", d.format,d.ecrire)?;
    Ok(t)
}

// ============================================= 15. le fichier du personnel

/// Chaque fiche de salarie, coordonnees comprises -- ce que le comptable
/// demande a l'ouverture du dossier. Pas de periode : ce sont les fiches d'aujourd'hui. Une case que la fiche ne dit
/// pas reste vide.
pub fn fichier_du_personnel(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let mut fiches: Vec<(String, &droit::fiche::FicheSalarie)> = ctx.salaries.iter().map(|(k, f)| (k.to_string(), f)).collect();
    fiches.sort_by(|a, b| a.0.cmp(&b.0));
    let mut lignes = Vec::new();
    for (_, f) in &fiches {
        let mut l = Ligne::nouvelle();
        l.ot(C::Code, f.code_salarie.remplie());
        l.ot(C::Nom, f.nom_affichage.remplie().or(f.nom.remplie()));
        l.ot(C::Prenom, f.prenom_affichage.remplie().or(f.prenom.remplie()));
        l.ot(C::Nir, f.nir.remplie().map(|n| [Some(n), f.nir_cle.remplie()].into_iter().flatten().collect::<Vec<_>>().join(" ")));
        l.ot(C::Naissance, f.date_naissance.dite().map(|j| jj(&j.iso())));
        l.ot(C::Lieu, f.lieu_naissance_affichage.remplie().or(f.lieu_naissance.remplie()));
        l.ot(C::Adresse, f.adresse_affichage.remplie().or(f.adresse.remplie()));
        l.ot(C::Cp, f.code_postal.remplie());
        l.ot(C::Commune, f.commune_affichage.remplie().or(f.commune.remplie()));
        l.ot(C::Email, f.email.remplie());
        l.ot(C::Emploi, f.emploi_affichage.remplie().or(f.emploi.remplie()));
        l.ot(C::Statut, f.statut.dite().map(|s| s.to_string()));
        l.ot(C::Cs, f.num_conges_spectacles.remplie());
        l.t(C::Actif, if f.archivee() { "archivé" } else { "oui" });
        lignes.push(l);
    }
    if lignes.is_empty() {
        return Err(refus("Aucune fiche de salarié."));
    }
    let mut t = Tableau::nouveau(EtatDExport::FichierDuPersonnel, "Fichier du personnel", "fiches au jour de l'export");
    t.avec_total = false;
    t.donnees_personnelles = true;
    t.sections = vec![Section::nouvelle("", lignes)];
    t.avertissements = vec!["Ce fichier porte le NIR, la date de naissance et l'adresse des salariés : il reste dans prive/, et ne s'envoie qu'à qui en a besoin.".to_string()];
    t.colonnes = vec![
        C::Code.colonne(),
        C::Nom.colonne(),
        C::Prenom.colonne(),
        C::Nir.colonne(),
        C::Naissance.titree("Né(e) le"),
        C::Lieu.colonne(),
        C::Adresse.colonne(),
        C::Cp.colonne(),
        C::Commune.colonne(),
        C::Email.colonne(),
        C::Emploi.colonne(),
        C::Statut.colonne(),
        C::Cs.colonne(),
        C::Actif.colonne(),
    ];
    t.sources = vec![droit::contexte::SALARIES.to_string()];
    finir(ctx, &mut t, None, "fichier-du-personnel", d.format,d.ecrire)?;
    Ok(t)
}

// ===================================== 16. le journal des declarations

/// L'acte tel que le journal des declarations le dit ; une fiche de demande, par sa sorte.
fn acte_lisible(quoi: crate::Acte) -> &'static str {
    match quoi {
        crate::Acte::Dsn => "DSN",
        crate::Acte::Dpae => "DPAE",
        crate::Acte::Documents => "Documents au salarié",
        crate::Acte::ContratSignature => "Contrat envoyé à la signature",
        crate::Acte::ContratContresigne => "Contrat contresigné renvoyé",
        crate::Acte::RegularisationCasc => "Régularisation CASC-SVP",
        crate::Acte::DossierSalarie => "Dossier envoyé au salarié",
        crate::Acte::FicheDemande => quoi.nom(),
    }
}

/// Une reponse lisible, entiere ; un rapport XML se lit au journal, pas dans une case.
fn reponse(r: &str) -> String {
    let t = r.split(socle::texte::blanc).filter(|m| !m.is_empty()).collect::<Vec<_>>().join(" ");
    if t.starts_with('<') {
        return "rapport de l'organisme (XML, conservé au journal)".to_string();
    }
    t
}

/// Chaque acte qui a engage la compagnie -- DSN, DPAE, envois,
/// signatures --, avec sa date, son mode, son issue et son fichier : la piste d'audit.
/// UN ACTE DU JOURNAL DES ENVOIS, tel que le journal des declarations le montre : `moteur` le tire de chaque ligne du
/// journal (`envois::journal::Ligne`), que ce module ne lit pas.
pub struct ActeDuJournal {
    pub quand: socle::paris::InstantOuJour,
    /// Sa sorte.
    pub quoi: crate::Acte,
    /// Sa remise, s'il en a une (la signature d'un contrat, l'envoi des documents).
    pub remise: Option<crate::ModeDeRemise>,
    /// Le mois qu'il declare, s'il en declare un.
    pub mois: Option<Mois>,
    pub mode: socle::mode::Mode,
    pub accepte: bool,
    /// Le fichier parti, s'il y en a un.
    pub fichier: Option<socle::fichiers::NomDeFichier>,
    pub destinataire: String,
    pub reponse: String,
    pub origine: String,
}

pub fn journal_des_declarations(ctx: &Contexte, d: &Demande, journal: &[ActeDuJournal]) -> R<Tableau> {
    let p = Periode::de(ctx, d, false)?;
    let mut lignes = Vec::new();
    for l in journal {
        // Un acte porte le mois qu'il declare ; un acte annuel se range au mois ou il a eu lieu.
        let m = match l.mois {
            Some(m) => m,
            None => l.quand.jour().mois_civil(),
        };
        if !p.contient(m) {
            continue;
        }
        let mut x = Ligne::nouvelle();
        x.t(C::Quand, l.quand.iso().replace('T', " ").chars().take(16).collect::<String>());
        let acte = match (l.quoi, l.remise) {
            (crate::Acte::ContratSignature, Some(crate::ModeDeRemise::Imprime)) => "Contrat imprimé, à remettre en main propre",
            (quoi, _) => acte_lisible(quoi),
        };
        x.t(C::Acte, acte);
        x.m(C::Mois, m).t(C::Mode, l.mode.nom()).t(C::Issue, if l.accepte { "accepté" } else { "refusé ou échoué" });
        x.ot(C::Fichier, l.fichier.clone()).t(C::Destinataire, l.destinataire.clone()).t(C::Reponse, reponse(&l.reponse)).t(C::Origine, l.origine.clone());
        lignes.push(x);
    }
    if lignes.is_empty() {
        return Err(refus(format!("Aucun acte au journal sur la période ({}).", p.libelle)));
    }
    let mut t = Tableau::nouveau(EtatDExport::JournalDesDeclarations, "Journal des déclarations et envois", p.libelle.clone());
    t.mois = lignes.iter().filter_map(|l| l.mois(C::Mois)).collect::<BTreeSet<_>>().into_iter().collect();
    t.avec_total = false;
    t.sections = sections_de(lignes, |l| l.mois(C::Mois), section_du_mois, Some(&|l| vec![l.texte(C::Quand)]))?;
    t.colonnes = vec![
        C::Quand.colonne(),
        C::Acte.colonne(),
        C::Mois.colonne(),
        C::Mode.colonne(),
        C::Issue.colonne(),
        C::Fichier.colonne(),
        C::Destinataire.colonne(),
        C::Reponse.colonne(),
        C::Origine.colonne(),
    ];
    t.sources = vec!["journal des actes de PuppetPaie : une ligne écrite après chaque acte, jamais avant".to_string()];
    finir(ctx, &mut t, Some(&p), "journal-des-declarations", d.format,d.ecrire)?;
    Ok(t)
}
