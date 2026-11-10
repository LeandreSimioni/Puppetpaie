//! Les etats du personnel et du controle : la liste des contrats, le registre
//! unique du personnel (L1221-13, D1221-23), le fichier du personnel, le journal des declarations.
//!
//! Le registre et le fichier du personnel SONT des fichiers du personnel : ils portent le NIR, la date de
//! naissance et l'adresse, et l'avertissement le dit.

use droit::fiche::Lue as _;
use std::collections::BTreeSet;

use droit::contexte::Contexte;
use socle::echec::{Classe, Echec};
use crate::etats_paie::{finir, titre_spectacle, Demande, Lecture, Periode};
use socle::num::{eur2, Decimal};
use crate::tableaux_export::{col_m, col_t, jj, qui, refus, section_du_mois, sections_de, Ligne, Section, Tableau, R};

// ============================================================ 13. les contrats

/// Un contrat par mois de paie, vu des etats du personnel.
struct ContratMois {
    mois: String,
    cle: String,
    fiche: droit::fiche::ChampsSalarie,
    salarie: String,
    /// Le numero du contrat, quand il est ecrit (un mois produit l'a fige).
    contrat: Option<String>,
    nature: String,
    emploi: Option<String>,
    spectacle: Option<String>,
    objet: Option<String>,
    debut: String,
    /// La fin du contrat, sinon son dernier jour travaille.
    fin: Option<String>,
    brut: Option<Decimal>,
}

/// Chaque contrat de la periode, une ligne par mois de paie, lu dans les paies du mois (`Lecture::paies` : la paie
/// ecrite d'un engagement valide, ou celle que l'import a lue).
fn contrats_de(p: &Periode, lecture: &mut Lecture) -> R<Vec<ContratMois>> {
    let mut out = Vec::new();
    for m in p.mois.clone() {
        for paie in lecture.paies(&m)?.iter() {
            let e = &paie.engagement;
            let emploi = e.emploi.clone().filter(|x| !x.is_empty()).or_else(|| paie.salarie.emploi.en_texte());
            out.push(ContratMois {
                mois: m.clone(),
                cle: paie.salarie.id.to_string(),
                fiche: paie.salarie.champs.clone(),
                salarie: qui(&paie.salarie)?,
                contrat: e.numero.as_deref().map(str::to_string),
                nature: e.nat().map_err(|x| Echec { classe: Some(Classe::Refuse), ..x })?.libelle.clone(),
                emploi,
                spectacle: titre_spectacle(paie),
                objet: paie.spectacle.as_ref().and_then(|s| s.numero_objet.dite()).cloned(),
                debut: e.debut.iso(),
                fin: match e.fin {
                    Some(f) => Some(f.iso()),
                    None => e.dates_travail.last().map(|j| j.iso()),
                },
                brut: Some(eur2(paie.brut)),
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
        l.t("mois", c.mois.clone()).t("cle", c.cle.clone()).t("salarie", c.salarie.clone()).ot("contrat", c.contrat.clone()).t("nature", c.nature.clone());
        l.ot("emploi", c.emploi.clone()).ot("spectacle", c.spectacle.clone()).ot("objet", c.objet.clone()).t("debut", jj(&c.debut)).ot("fin", c.fin.as_deref().map(jj));
        l.od("brut", c.brut);
        lignes.push(l);
    }
    if contrats.iter().any(|c| c.contrat.is_none()) {
        avert.push("Contrat sans numéro : son mois n'a pas été produit, aucun numéro n'est figé. Rien n'est inventé.".to_string());
    }
    let mut t = Tableau::nouveau("contrats", "Liste des contrats", p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    t.sections = sections_de(lignes, |l| l.texte("mois"), section_du_mois, Some(&|l| vec![l.texte("salarie"), l.texte("debut"), l.texte("contrat")]))?;
    t.colonnes = vec![
        col_t("mois", "Mois de paie"),
        col_t("contrat", "Contrat"),
        col_t("salarie", "Salarié"),
        col_t("nature", "Nature"),
        col_t("emploi", "Emploi"),
        col_t("spectacle", "Spectacle"),
        col_t("objet", "N° d'objet"),
        col_t("debut", "Début"),
        col_t("fin", "Fin"),
        col_m("brut", "Brut du mois"),
    ];
    t.sources = vec!["paies écrites des engagements validés ; paies des engagements importés, telles que l'import les a lues".to_string()];
    finir(ctx, &mut t, Some(&p), "contrats", &d.format, d.ecrire)?;
    Ok(t)
}

// ================================================ 14. le registre du personnel

/// `NATIONALITES`.
fn nationalite(code: &str) -> Option<&'static str> {
    Some(match code {
        "01" => "française",
        "02" => "Union européenne",
        "03" => "EEE hors Union européenne",
        "04" => "hors Union européenne et EEE",
        _ => return None,
    })
}

/// Le premier chiffre du NIR (1 homme, 2 femme) ; un NIR provisoire ne le dit pas.
fn sexe(nir: Option<&str>) -> Option<&'static str> {
    match nir?.chars().next() {
        Some('1') => Some("M"),
        Some('2') => Some("F"),
        _ => None,
    }
}

/// Une ligne par embauche, dans l'ordre des embauches, avec les mentions
/// obligatoires. Un intermittent est embauche a chaque contrat : chaque CDDU est une entree et une sortie.
pub fn registre_du_personnel(ctx: &Contexte, d: &Demande) -> R<Tableau> {
    let p = Periode::de(ctx, d, true)?;
    let mut avert = Vec::new();
    let mut vus: BTreeSet<(String, Option<String>, String, Option<String>)> = BTreeSet::new();
    let mut a_preciser: BTreeSet<String> = BTreeSet::new();
    // (entree AAAA-MM-JJ, nom, contrat) : l'ordre des embauches.
    let mut rangees: Vec<((String, Option<String>, Option<String>), Ligne)> = Vec::new();
    let mut lecture = Lecture::nouvelle(ctx, &d.departs);
    for c in contrats_de(&p, &mut lecture)? {
        // Un contrat qui deborde du mois apparait une fois par mois de paie : c'est UNE embauche.
        if !vus.insert((c.cle.clone(), c.contrat.clone(), c.debut.clone(), c.fin.clone())) {
            continue;
        }
        let f = &c.fiche;
        let nom = f.nom_affichage.remplie().or(f.nom.remplie()).map(str::to_string);
        let mut l = Ligne::nouvelle();
        l.ot("nom", nom.clone()).ot("prenom", f.prenom_affichage.remplie().or(f.prenom.remplie()));
        // La codification relue comme la DSN la declare (`codification_ue`) : un ancien 03 sans pays ne se lit pas « EEE ».
        let dite = f.code_nationalite_dsn.est_dite() || f.pays_nationalite.remplie().is_some() || f.pays_nationalite_hors_liste.remplie().is_some();
        let categorie = if dite {
            match socle::codification_ue::codification_ue(&f.nationalite()) {
                Ok(code) => nationalite(&code),
                Err(pourquoi) => {
                    a_preciser.insert(format!("{} : {}", c.salarie, String::from(pourquoi)));
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
            Some(code) => match socle::pays::du_code(code).map_err(|m| Echec { classe: Some(Classe::Illisible), ..m })? {
                Some(p) => Some(p.nom),
                None => Some(code.to_string()),
            },
            None => f.pays_nationalite_hors_liste.remplie().map(str::to_string),
        };
        l.ot(
            "nationalite",
            match (pays, categorie) {
                (Some(pays), Some(cat)) => Some(format!("{} ({})", pays, cat)),
                (Some(pays), None) => Some(pays),
                (None, cat) => cat.map(str::to_string),
            },
        );
        l.ot("naissance", f.date_naissance.dite().map(|j| jj(&j.iso()))).ot("sexe", sexe(f.nir.portee()));
        l.ot("emploi", c.emploi.clone()).ot("qualification", f.statut.dite().map(|s| s.to_string())).t("contrat_type", c.nature.clone()).ot("contrat", c.contrat.clone());
        l.t("entree", jj(&c.debut)).ot("sortie", c.fin.as_deref().map(jj));
        rangees.push(((c.debut.clone(), nom, c.contrat.clone()), l));
    }
    rangees.sort_by(|a, b| a.0.cmp(&b.0));
    let mut lignes: Vec<Ligne> = Vec::new();
    for (i, (_, mut l)) in rangees.into_iter().enumerate() {
        l.n("ordre", (i + 1) as i64);
        lignes.push(l);
    }
    avert.push("La nationalité est le pays de nationalité de la fiche, suivi de la catégorie déclarée en DSN (française, Union européenne, EEE hors Union, reste du monde) ; une fiche sans pays n'en montre que la catégorie. Pour un salarié étranger hors UE, le registre doit aussi porter le type et le numéro du titre de travail (art. D1221-24) — à compléter à la main.".to_string());
    avert.extend(a_preciser);
    avert.push("Registre unique du personnel : il porte des données personnelles et se conserve cinq ans après le départ de chaque salarié. Il se présente à l'inspection du travail sur demande.".to_string());
    let mut t = Tableau::nouveau("registre-du-personnel", "Registre unique du personnel", p.libelle.clone());
    t.mois = p.mois.clone();
    t.avertissements = avert;
    t.sections = vec![Section::nouvelle("", lignes)];
    t.avec_total = false;
    t.donnees_personnelles = true;
    t.colonnes = vec![
        col_t("ordre", "N°"),
        col_t("nom", "Nom"),
        col_t("prenom", "Prénom"),
        col_t("nationalite", "Nationalité"),
        col_t("naissance", "Date de naissance"),
        col_t("sexe", "Sexe"),
        col_t("emploi", "Emploi"),
        col_t("qualification", "Qualification"),
        col_t("contrat_type", "Type de contrat"),
        col_t("contrat", "Contrat"),
        col_t("entree", "Entrée"),
        col_t("sortie", "Sortie"),
    ];
    t.sources = vec!["Code du travail, art. L1221-13 (registre, ordre des embauches), D1221-23 (mentions : nationalité, date de naissance, sexe, emploi, qualification, dates d'entrée et de sortie), D1221-24 (type de contrat, titre de travail), Légifrance, relevé le 23/09/2026".to_string()];
    finir(ctx, &mut t, Some(&p), "registre-du-personnel", &d.format, d.ecrire)?;
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
        l.ot("code", f.code_salarie.remplie());
        l.ot("nom", f.nom_affichage.remplie().or(f.nom.remplie()));
        l.ot("prenom", f.prenom_affichage.remplie().or(f.prenom.remplie()));
        l.ot("nir", f.nir.remplie().map(|n| [Some(n), f.nir_cle.remplie()].into_iter().flatten().collect::<Vec<_>>().join(" ")));
        l.ot("naissance", f.date_naissance.dite().map(|j| jj(&j.iso())));
        l.ot("lieu", f.lieu_naissance_affichage.remplie().or(f.lieu_naissance.remplie()));
        l.ot("adresse", f.adresse_affichage.remplie().or(f.adresse.remplie()));
        l.ot("cp", f.code_postal.remplie());
        l.ot("commune", f.commune_affichage.remplie().or(f.commune.remplie()));
        l.ot("email", f.email.remplie());
        l.ot("emploi", f.emploi_affichage.remplie().or(f.emploi.remplie()));
        l.ot("statut", f.statut.dite().map(|s| s.to_string()));
        l.ot("cs", f.num_conges_spectacles.remplie());
        l.t("actif", if f.archivee() { "archivé" } else { "oui" });
        lignes.push(l);
    }
    if lignes.is_empty() {
        return Err(refus("Aucune fiche de salarié."));
    }
    let mut t = Tableau::nouveau("fichier-du-personnel", "Fichier du personnel", "fiches au jour de l'export");
    t.avec_total = false;
    t.donnees_personnelles = true;
    t.sections = vec![Section::nouvelle("", lignes)];
    t.avertissements = vec!["Ce fichier porte le NIR, la date de naissance et l'adresse des salariés : il reste dans prive/, et ne s'envoie qu'à qui en a besoin.".to_string()];
    t.colonnes = vec![
        col_t("code", "Code"),
        col_t("nom", "Nom"),
        col_t("prenom", "Prénom"),
        col_t("nir", "NIR"),
        col_t("naissance", "Né(e) le"),
        col_t("lieu", "à"),
        col_t("adresse", "Adresse"),
        col_t("cp", "Code postal"),
        col_t("commune", "Commune"),
        col_t("email", "Courriel"),
        col_t("emploi", "Emploi"),
        col_t("statut", "Statut"),
        col_t("cs", "N° Congés Spectacles"),
        col_t("actif", "Fiche"),
    ];
    t.sources = vec![droit::contexte::SALARIES.to_string()];
    finir(ctx, &mut t, None, "fichier-du-personnel", &d.format, d.ecrire)?;
    Ok(t)
}

// ===================================== 16. le journal des declarations

/// `ACTES_LIBELLES`.
fn acte_lisible(quoi: &str) -> Option<&'static str> {
    Some(match quoi {
        "dsn" => "DSN",
        "dpae" => "DPAE",
        "documents" => "Documents au salarié",
        "contrat-signature" => "Contrat envoyé à la signature",
        "contrat-contresigne" => "Contrat contresigné renvoyé",
        "regularisation-casc" => "Régularisation CASC-SVP",
        "dossier-salarie" => "Dossier envoyé au salarié",
        _ => return None,
    })
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
    pub quand: String,
    /// Sa sorte, telle que le journal l'ecrit (`dsn`, `contrat-signature`...).
    pub quoi: String,
    pub main_propre: bool,
    /// Le mois qu'il declare (`AAAA-MM`), s'il en declare un.
    pub mois: Option<String>,
    pub mode: String,
    pub accepte: bool,
    /// Le fichier parti, s'il y en a un.
    pub fichier: Option<String>,
    pub destinataire: String,
    pub reponse: String,
    pub origine: String,
}

pub fn journal_des_declarations(ctx: &Contexte, d: &Demande, journal: &[ActeDuJournal]) -> R<Tableau> {
    let p = Periode::de(ctx, d, false)?;
    let mut lignes = Vec::new();
    for l in journal {
        // Un acte porte le mois qu'il declare ; un acte annuel se range au mois ou il a eu lieu.
        let m: String = match &l.mois {
            Some(m) => m.clone(),
            None => l.quand.chars().take(7).collect(),
        };
        if m.chars().count() != 7 || !p.contient(&m) {
            continue;
        }
        let mut x = Ligne::nouvelle();
        x.t("quand", l.quand.replace('T', " ").chars().take(16).collect::<String>());
        let main_propre = l.quoi == "contrat-signature" && l.main_propre;
        // Un acte que la table ne nomme pas se dit par sa sorte, telle que le journal l'ecrit.
        let acte = match (main_propre, acte_lisible(&l.quoi)) {
            (true, _) => "Contrat imprimé, à remettre en main propre".to_string(),
            (false, Some(a)) => a.to_string(),
            (false, None) => l.quoi.clone(),
        };
        x.t("acte", acte);
        x.t("mois", m).t("mode", l.mode.clone()).t("issue", if l.accepte { "accepté" } else { "refusé ou échoué" });
        x.ot("fichier", l.fichier.clone()).t("destinataire", l.destinataire.clone()).t("reponse", reponse(&l.reponse)).t("origine", l.origine.clone());
        lignes.push(x);
    }
    if lignes.is_empty() {
        return Err(refus(format!("Aucun acte au journal sur la période ({}).", p.libelle)));
    }
    let mut t = Tableau::nouveau("journal-des-declarations", "Journal des déclarations et envois", p.libelle.clone());
    t.mois = lignes.iter().map(|l| l.texte("mois")).collect::<BTreeSet<_>>().into_iter().collect();
    t.avec_total = false;
    t.sections = sections_de(lignes, |l| l.texte("mois"), section_du_mois, Some(&|l| vec![l.texte("quand")]))?;
    t.colonnes = vec![
        col_t("quand", "Date"),
        col_t("acte", "Acte"),
        col_t("mois", "Mois"),
        col_t("mode", "Mode"),
        col_t("issue", "Issue"),
        col_t("fichier", "Fichier"),
        col_t("destinataire", "Destinataire"),
        col_t("reponse", "Réponse"),
        col_t("origine", "Reconstitué depuis"),
    ];
    t.sources = vec!["journal des actes de PuppetPaie : une ligne écrite après chaque acte, jamais avant".to_string()];
    finir(ctx, &mut t, Some(&p), "journal-des-declarations", &d.format, d.ecrire)?;
    Ok(t)
}
