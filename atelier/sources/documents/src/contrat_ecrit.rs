//! LE CONTRAT ECRIT, quelle que soit sa nature : ce que la loi exige qu'il dise, et les valeurs qui le disent.
//!
//! Le contrat a duree determinee « est etabli par ecrit » avec ses mentions (code du travail L. 1242-12,
//! `atelier/reference/sources/code-du-travail/xml/L1242-12.xml`) ; tout salarie recoit par ecrit les informations
//! principales de sa relation de travail (L. 1221-5-1 ; leur liste, R. 1221-34 ; le renvoi aux textes, R. 1221-35 :
//! `xml/L1221-5-1.xml`, `xml/R1221-34.xml`, `xml/R1221-35.xml`) ; le contrat a temps partiel dit sa duree et sa
//! repartition (L. 3123-6, `xml/L3123-6.xml`). Chaque nature de `contrats-base.json` dit son modele de base et ses
//! mentions obligatoires, chacune par son marqueur (`contrat_ecrit`) : un modele -- la base ou un modele special de la
//! compagnie -- qui n'en cite pas une ne fait pas de contrat (le verificateur, `manques`).
//!
//! Ici, les valeurs propres au contrat d'un permanent (`marqueurs_au_mois` : son salaire au mois, sa duree du travail,
//! son motif et son terme), celles que tout contrat ecrit porte (`poser_les_mentions` : la categorie d'emploi, la
//! periode d'essai, les organismes, les mentions de renvoi, les clauses particulieres), et la liste des champs qu'un
//! modele peut citer (`champs_des_modeles`), que l'ecran montre a qui ecrit le sien.

use droit::fiche::Lue as _;
use serde::Serialize;

use crate::marqueurs::{Marqueur, ModeleLu, Valeurs};

use crate::avenant::{de_devant, et, fait_le_brut, ligne_citee, remuneration_hors_brut};
use crate::bulletin::fmt_eur;
use socle::texte::{euros, virgule};
use droit::contexte::Contexte;
use droit::natures::{self, Nature};
use engagement::paie_ecrite::{FormeDEngagement, FormeDePaie};
use socle::num::{Decimal, Resultat};

/// Un texte de l'engagement, non vide (ses blancs ote).
fn texte(t: Option<&str>) -> Option<&str> {
    t.map(str::trim).filter(|s| !s.is_empty())
}

/// Ce que la saisie dit du contrat ecrit (`engagement::ContratEcrit`), non vide.
fn ecrit(eng: &FormeDEngagement, quoi: fn(&engagement::ContratEcrit) -> Option<&str>) -> Option<String> {
    texte(eng.contrat_ecrit.as_ref().and_then(quoi)).map(str::to_string)
}

/// La duree de la periode d'essai que la saisie dit, non nulle.
fn periode_essai(eng: &FormeDEngagement) -> Option<String> {
    let jours = eng.contrat_ecrit.as_ref().and_then(|c| c.periode_essai_jours.as_ref()).filter(|j| j.vrai())?;
    texte(Some(&jours.en_texte())).map(str::to_string)
}

/// Qui le contrat nomme, pour un refus : les parties le posent toujours (`avenant::parties`).
fn qui(m: &Valeurs) -> Resultat<String> {
    posee(m, Marqueur::SalarieNomComplet)
}

/// Une valeur que les parties ont posee (`avenant::parties`).
fn posee(m: &Valeurs, k: Marqueur) -> Resultat<String> {
    m.get(&k).cloned().ok_or_else(|| format!("le contrat n'a pas de valeur {}", k))
}

/// Les valeurs du contrat d'un permanent, paye au mois, que les parties (`avenant::parties`) ne portent pas : le motif
/// et le terme d'un CDD, le lieu, la remuneration, sa periodicite, la duree du travail ; la remuneration, le brut et le
/// net sont ceux de la paie `paie` du mois.
pub fn marqueurs_au_mois<A>(paie: &FormeDePaie<A>, nat: &Nature, m: &mut Valeurs) -> Resultat<()> {
    let eng = &paie.engagement;
    let nom = qui(m)?;
    // LE TERME (L1242-12, 2°) : un contrat a duree determinee le dit ; un CDI n'en a pas.
    match eng.fin {
        Some(f) => {
            m.insert(Marqueur::ContratFin, f.jj_mm_aaaa());
        }
        None if !nat.duree_indeterminee => return Err(format!("Le contrat de {} ({}) n'a pas de date de fin : un contrat à durée déterminée dit son terme (L. 1242-12, 2°). Rien n'est écrit.", nom, nat.libelle)),
        None => {}
    }
    // LE MOTIF (L1242-12, premier alinea) et, pour un remplacement, la personne remplacee (1°).
    if nat.motif_recours_exige() {
        let code = texte(eng.motif_recours.as_deref()).ok_or_else(|| format!("Le contrat de {} ne dit pas son motif de recours : un CDD « comporte la définition précise de son motif » (L. 1242-12). Le choisir dans l'engagement (Plus d'options › Motif de recours). Rien n'est écrit.", nom))?;
        let libelle = nat.libelle_motif_recours(code).ok_or_else(|| format!("Motif de recours {} inconnu pour un {}.", code, nat.libelle))?;
        m.insert(Marqueur::ContratMotif, minuscule_initiale(&libelle));
        if nat.motif_de_remplacement(code) {
            let qui_remplace = ecrit(eng, |c| c.personne_remplacee.as_deref()).ok_or_else(|| {
                format!("Le contrat de {} est conclu pour un remplacement : il comporte « le nom et la qualification professionnelle de la personne remplacée » (L. 1242-12, 1°). L'écrire dans l'engagement (Plus d'options › Personne remplacée). Rien n'est écrit.", nom)
            })?;
            m.insert(Marqueur::ContratPersonneRemplacee, qui_remplace);
        }
    }
    // LE LIEU DE TRAVAIL (R1221-34, 2°) : celui que l'engagement dit ; sans lui, le siege de l'employeur (INFERENCE : un
    // permanent travaille la ou la compagnie est etablie, sauf si son engagement dit autre chose).
    let lieu = match texte(Some(&eng.lieu)) {
        Some(l) => format!("à {}", l),
        None => format!(
            "au siège de l'employeur, {}, {} {}",
            posee(m, Marqueur::EmployeurAdresse)?,
            posee(m, Marqueur::EmployeurCodePostal)?,
            posee(m, Marqueur::EmployeurCommune)?
        ),
    };
    m.insert(Marqueur::ContratLieu, lieu);
    // LA REMUNERATION ET SES COMPOSANTES (L1242-12, 7° ; R1221-34, 11°) : le salaire mensuel de base (le taux de la ligne
    // de salaire de base, pas son montant du mois, que le prorata d'une entree en cours de mois reduit), puis ce qui
    // s'y ajoute et fait le brut. Un mois coupe par un avenant a une ligne de salaire de base par periode, dans l'ordre
    // des jours : le contrat dit celui de la derniere, ses termes a partir de l'avenant.
    let mut morceaux = Vec::new();
    let mut base = None;
    for l in paie.remuneration.iter().filter(|l| fait_le_brut(l)) {
        if l.conges_pris {
            continue;
        }
        if l.salaire_de_base {
            base = Some(format!("un salaire mensuel brut de {}", euros(l.taux)));
        } else if l.montant > Decimal::ZERO {
            morceaux.push(ligne_citee(l));
        }
    }
    if let Some(b) = base {
        morceaux.insert(0, b);
    }
    if morceaux.is_empty() {
        return Err(format!("Le contrat de {} ne dit aucune rémunération : la paie de son premier mois n'a pas de salaire de base. Rien n'est écrit.", nom));
    }
    m.insert(Marqueur::RemunerationDetail, et(&morceaux));
    if let Some(h) = remuneration_hors_brut(&paie.remuneration) {
        m.insert(Marqueur::RemunerationHorsBrut, h);
    }
    m.insert(Marqueur::RemunerationBrut, fmt_eur(paie.brut));
    m.insert(Marqueur::RemunerationNetEstime, fmt_eur(paie.net_a_payer));
    // LA PERIODICITE DU PAIEMENT (R1221-34, 11°) : une fois par mois (L3242-1). INFERENCE : par virement, comme le contrat
    // d'engagement le dit.
    m.insert(Marqueur::ContratPaiement, "chaque mois, par virement".into());
    m.insert(Marqueur::SalarieCaseSignature, "Le salarié".into());
    if let Some(p) = texte(eng.date_paiement.as_deref()) {
        m.insert(Marqueur::ContratDatePaiement, socle::date::Jour::lire(p)?.jj_mm_aaaa());
    }
    // LA DUREE DU TRAVAIL (R1221-34, 12°) : la quotite du contrat, en heures par mois ; sans elle, celle que la DSN
    // declare (S21.G00.40.013, `quotite`). Sous la duree de reference, c'est un temps partiel, dont le contrat dit la
    // repartition (L3123-6, 1°).
    let q = match eng.quotite_contrat {
        Some(q) => q,
        None => eng.quotite,
    };
    // Le temps partiel comme la DSN le declare (S21.G00.40.014, `Engagement::modalite_temps_travail`) : la modalite
    // saisie, sinon une quotite sous la reference de l'entreprise, plafonnee a la duree legale ; sans reference, le temps
    // plein.
    let legale = nat.quotite_reference_defaut().map(socle::num::dec_f64).transpose()?;
    let reference = match (eng.quotite_reference_saisie, legale) {
        (Some(r), Some(l)) => Some(r.min(l)),
        (r, l) => r.or(l),
    };
    let partiel = match texte(eng.modalite_temps_travail_saisie.as_deref()) {
        Some(m) => m == "20",
        None => reference.is_some_and(|r| q < r),
    };
    let mut duree = format!("{} heures par mois", virgule(q));
    if partiel {
        let plein = if let Some(r) = reference { format!(", pour {} à temps plein", virgule(r)) } else { String::new() };
        let repartition = ecrit(eng, |c| c.repartition_horaire.as_deref()).ok_or_else(|| {
            format!(
                "Le contrat de {} est à temps partiel ({} heures par mois{}) : il dit « la répartition de la durée du travail entre les jours de la semaine ou les semaines du mois » (L. 3123-6, 1°). L'écrire dans l'engagement (Plus d'options › Répartition des horaires). Rien n'est écrit.",
                nom,
                virgule(q),
                plein
            )
        })?;
        duree = format!("{}, à temps partiel, réparties ainsi : {}", duree, repartition);
    } else if q.round_dp(2) == Decimal::new(15167, 2) {
        duree.push_str(", soit la durée légale de trente-cinq heures par semaine (article L. 3121-27 du code du travail)");
    }
    m.insert(Marqueur::ContratDureeTravail, duree);
    Ok(())
}

/// « Accroissement… » -> « accroissement… » : un libelle qui suit « pour ».
fn minuscule_initiale(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(p) => p.to_lowercase().chain(c).collect(),
        None => s.to_string(),
    }
}

/// Un organisme que le contrat nomme : son nom, et son adresse quand la donnee la porte.
struct Organisme {
    nom: String,
    adresse: Option<String>,
}

impl Organisme {
    /// « Audiens Retraite, 74 rue Jean Bleuzen, 92177 Vanves Cedex ».
    fn nom_et_adresse(&self) -> Option<String> {
        self.adresse.as_ref().map(|a| format!("{}, {}", self.nom, a))
    }
}

/// Les organismes de la compagnie : ses versements et ses adhesions (`Contexte::organismes`), et la partie normative
/// livree (`organismes-base.json`), qui porte les adresses et les organismes que la branche designe.
struct Organismes {
    compagnie: droit::organismes::Organismes,
    idcc: Option<String>,
}

impl Organismes {
    fn lire(ctx: &Contexte, ent: &droit::fiche::ChampsEntreprise) -> Resultat<Organismes> {
        Ok(Organismes { compagnie: ctx.organismes()?, idcc: ent.idcc.en_texte() })
    }

    /// L'organisme de ce siret : son nom, et l'adresse que la donnee livree lui donne.
    fn par_siret(&self, siret: &str) -> Option<Organisme> {
        let v = self.compagnie.base.versement(siret)?;
        Some(Organisme { nom: v.nom.clone(), adresse: v.adresse.clone() })
    }

    /// La caisse de retraite complementaire : l'organisme a qui la compagnie verse la cotisation du regime unifie
    /// Agirc-Arrco (code 131 du bloc S21.G00.81, `organismes-base.json`).
    fn retraite(&self) -> Option<Organisme> {
        let v = self.compagnie.versements().iter().find(|v| v.codes81.iter().any(|c| c == "131"))?;
        self.par_siret(&v.siret)
    }

    /// L'organisme de l'adhesion `cle` (la prevoyance, la sante) : celui que la compagnie a enregistre, sinon celui que
    /// la branche designe (`organisme_fixe` des contrats de `organismes-base.json`). `None` : elle n'a pas ce contrat.
    fn adhesion(&self, cle: &str) -> Option<Organisme> {
        let enregistre = self.compagnie.adhesions_prevoyance.iter().find(|a| a.cle == cle).map(|a| a.delegataire.as_str());
        let designe = || {
            self.compagnie.base.contrats_de(self.idcc.as_deref()).filter(|c| c.cle == cle && c.organisme_fixe).find_map(|c| c.organisme.as_deref())
        };
        self.par_siret(enregistre.or_else(designe)?)
    }
}

/// Les valeurs que tout contrat ecrit porte, quelle que soit sa nature : la categorie d'emploi (R1221-34, 3°), le motif
/// d'un contrat d'usage, la periode d'essai (L1242-12, 6° ; R1221-34, 7°), les organismes de retraite complementaire et
/// de prevoyance (L1242-12, 8° ; R1221-34, 14°), les mentions de renvoi (R1221-34, 8° a 10° et 14°, que R1221-35 permet
/// de dire par un renvoi aux textes), et les clauses particulieres de l'engagement, s'il en dit (le modele les imprime
/// par un alinea `[si contrat.clauses_particulieres]`).
pub fn poser_les_mentions(ctx: &Contexte, ent: &droit::fiche::ChampsEntreprise, sal: &droit::fiche::ChampsSalarie, eng: &FormeDEngagement, nat: &Nature, m: &mut Valeurs) -> Resultat<()> {
    let nom = qui(m)?;
    // LA CATEGORIE D'EMPLOI : le classement conventionnel (S21.G00.40.002), sinon le statut cadre ou non cadre de la
    // retraite complementaire (S21.G00.40.003), sinon le statut du spectacle (artiste, technicien).
    let artiste = sal.statut.dite() == Some(&droit::fiche::Statut::Artiste);
    let categorie = match sal.statut_conventionnel.code().map(|c| natures::categorie_d_emploi(c, artiste)).transpose()?.flatten() {
        Some(c) => c,
        None => match sal.statut_categoriel_rc.code() {
            Some("01" | "02") => "cadre".to_string(),
            Some("04") => "non cadre".to_string(),
            _ => sal.statut.code().map(str::to_string).ok_or_else(|| {
                format!("La fiche de {} ne dit pas sa catégorie d'emploi, que le contrat écrit porte (R. 1221-34, 3°) : choisir son métier dans la liste, ou saisir son classement conventionnel sur sa fiche. Rien n'est écrit.", nom)
            })?,
        },
    };
    m.insert(Marqueur::SalarieCategorie, categorie);
    // LES PHRASES DE RENVOI de la nature (R. 1221-35) : une nature sans contrat ecrit n'en fait pas.
    let phrases = nat.phrases().ok_or_else(|| format!("La nature {} n'a pas de contrat écrit.", nat.libelle))?;
    if let Some(motif) = &phrases.motif {
        m.insert(Marqueur::ContratMotif, motif.clone());
    }
    m.insert(
        Marqueur::ContratPeriodeEssai,
        match periode_essai(eng) {
            Some(j) => format!("{} jour{}", j, if j == "1" { "" } else { "s" }),
            None => "aucune".into(),
        },
    );
    // LES ORGANISMES : le nom ET l'adresse (L1242-12, 8°) ; une adresse que la donnee ne porte pas ne se devine pas.
    let org = Organismes::lire(ctx, ent)?;
    let retraite = org.retraite().ok_or_else(|| socle::echec::avec(format!("Le contrat de {} nomme sa caisse de retraite complémentaire (L. 1242-12, 8°), et la table livrée des versements n'en nomme aucune : le signaler au développeur. Rien n'est écrit.", nom), socle::echec::APart { fichier: Some("organismes-base.json".into()), cle: Some("131".into()), ..Default::default() }))?;
    let manque_adresse = |o: &Organisme| format!("Le contrat de {} donne « le nom et l'adresse » de {} (L. 1242-12, 8°), et cette adresse n'est pas connue du logiciel. Rien n'est écrit.", nom, o.nom);
    m.insert(Marqueur::OrganismesRetraite, retraite.nom_et_adresse().ok_or_else(|| manque_adresse(&retraite))?);
    let (cle_prevoyance, cle_sante) = if nat.permanent() { ("prevoyance_permanents", "sante_permanents") } else { ("interbranche", "tsintermittents") };
    let prevoyance = org.adhesion(cle_prevoyance);
    m.insert(
        Marqueur::OrganismesPrevoyance,
        match &prevoyance {
            Some(p) => p.nom_et_adresse().ok_or_else(|| manque_adresse(p))?,
            None => "aucun".into(),
        },
    );
    m.insert(Marqueur::MentionsFormation, phrases.formation.clone());
    m.insert(Marqueur::MentionsRupture, phrases.rupture.clone());
    let conges = phrases.conges.clone();
    let conges = if conges.contains("{congesspectacles}") { conges.replace("{congesspectacles}", &crate::conges_spectacles::adherent(ent)?) } else { conges };
    m.insert(Marqueur::MentionsConges, conges);
    // LES REGIMES OBLIGATOIRES ET LA PROTECTION SOCIALE COMPLEMENTAIRE (R1221-34, 14°) : le regime de base que la nature
    // declare (S21.G00.40.018, 200 : le regime general), la retraite complementaire, et les contrats collectifs.
    let regime = match nat.code("S21.G00.40.018").as_deref() {
        Some("200") => "Le salarié est affilié au régime général de la sécurité sociale".to_string(),
        Some(autre) => return Err(format!("Le contrat de {} : le régime de base {} de la nature {} n'a pas de libellé pour le contrat écrit.", nom, autre, nat.libelle)),
        None => return Err(format!("Le contrat de {} : la nature {} ne dit pas son régime de base (S21.G00.40.018).", nom, nat.libelle)),
    };
    let aupres = |nom: &str| format!("auprès {}{}", de_devant(nom), nom);
    let mut protection = format!("{} ; sa retraite complémentaire est celle du régime Agirc-Arrco, {}", regime, aupres(&retraite.nom));
    let sante = org.adhesion(cle_sante);
    let collectifs: Vec<String> = match (&prevoyance, &sante) {
        (Some(p), Some(s)) if p.nom == s.nom => vec![format!("de prévoyance et de frais de santé {}", aupres(&p.nom))],
        _ => [prevoyance.as_ref().map(|p| format!("de prévoyance {}", aupres(&p.nom))), sante.as_ref().map(|s| format!("de frais de santé {}", aupres(&s.nom)))].into_iter().flatten().collect(),
    };
    if !collectifs.is_empty() {
        protection.push_str(&format!(". Il bénéficie des garanties collectives {}, prévues par la convention collective", et(&collectifs)));
    }
    protection.push('.');
    m.insert(Marqueur::MentionsProtectionSociale, protection);
    if let Some(c) = ecrit(eng, |c| c.clauses_particulieres.as_deref()) {
        m.insert(Marqueur::ContratClausesParticulieres, c);
    }
    Ok(())
}

/// UN MODELE DE CONTRAT TEL QU'IL SERT A UN CONTRAT : comment une phrase le nomme (« le modèle de base « … » », « le
/// modèle « Tournée » »), s'il est la base de sa nature, son texte lu, et celui de son avenant.
#[derive(Debug, Clone)]
pub struct ModeleDuContrat {
    pub nom: String,
    pub base: bool,
    pub lu: ModeleLu,
    pub avenant: ModeleLu,
}

/// La phrase de L. 1221-3 que le verificateur rappelle : il ne detecte aucune langue, il dit la regle
/// (`atelier/reference/sources/legifrance-versions-datees/legifrance-ct-L1221-3-20080501.txt`).
pub const EN_FRANCAIS: &str = "Le contrat de travail écrit est rédigé en français (code du travail, L. 1221-3) : une traduction peut s'ajouter au texte français, jamais le remplacer.";

/// Les valeurs qui peuvent etre vides : un modele ne les cite que dans un alinea qui commence par `[si ce.marqueur]`.
const PEUVENT_ETRE_VIDES: [Marqueur; 5] = [Marqueur::SpectacleNumeroObjet, Marqueur::RemunerationPeriode, Marqueur::RemunerationHorsBrut, Marqueur::ContratPersonneRemplacee, Marqueur::ContratClausesParticulieres];

/// Ce que le refus d'une mention inconnue porte a part : la table livree et le marqueur.
#[derive(Serialize)]
struct MentionAPart<'a> {
    fichier: &'static str,
    marqueur: &'a str,
}

/// Les mentions que la loi exige d'un contrat de cette nature (`Nature::mentions_obligatoires`), chacune par son
/// marqueur, le texte qui l'exige, et si elle n'est due que dite.
pub fn mentions(nat: &Nature) -> Result<Vec<(Marqueur, String, bool)>, String> {
    nat.mentions_obligatoires()
        .into_iter()
        .flatten()
        .map(|x| {
            let m = Marqueur::lire(&x.marqueur).ok_or_else(|| socle::echec::avec(format!("La table livrée des natures exige du {} une mention que le logiciel ne connaît pas : le signaler au développeur.", nat.nom_du_contrat()), MentionAPart { fichier: "contrats-base.json", marqueur: &x.marqueur }))?;
            Ok((m, x.texte.clone(), x.si_dite == Some(true)))
        })
        .collect()
}

/// LE VERIFICATEUR D'UN MODELE DE CONTRAT, paragraphe par paragraphe : sa grammaire ; dans chaque article (et le
/// preambule, le titre, les signatures), chaque champ cite connu du logiciel et permis a un contrat, chaque condition
/// `[si …]` sur un champ connu, chaque valeur qui peut etre vide dans un alinea conditionne par elle ; puis chaque mention
/// que la loi exige de sa nature (`contrat_ecrit.mentions`) citee dans un alinea qui s'ecrit (sans condition, ou
/// conditionne par elle-meme), et de meme les clauses particulieres de l'engagement (`CLAUSES_EXIGEES`). Une mention
/// `si_dite` (la personne remplacee, les clauses) n'est due que si sa valeur est dite :
/// `valeurs` celles d'un contrat ; `None`, un modele qu'on ecrit, ou chacune se demande comme si elle l'etait. Rend les
/// manques, chacun une phrase qui nomme le paragraphe ou la donnee ; vide : le modele est bon.
pub fn manques(nat: &Nature, texte: &str, valeurs: Option<&Valeurs>) -> Result<Vec<String>, String> {
    let m = match fabrique_pdf::modele::lire_modele(texte, "le modèle") {
        Ok(m) => m,
        Err(r) => return Ok(vec![format!("Ce modèle ne se lit pas : {}.", r.0)]),
    };
    let (mut dits, ecrits) = champs_cites(&m, &crate::marqueurs::DU_REMPLACEMENT, &[]);
    for (cle, pourquoi, si_dite) in mentions(nat)? {
        let due = !si_dite || valeurs.is_none_or(|v| v.get(&cle).is_some_and(|x| !x.trim().is_empty()));
        if due && !ecrits.iter().any(|(x, c)| *x == cle && c.is_none_or(|c| c == cle)) {
            dits.push(format!("Il manque {}, que la loi exige d'un {} ({}).", cle, nat.nom_du_contrat(), pourquoi));
        }
    }
    // Les clauses de l'engagement, exigees de tout modele comme une mention due si elle est dite.
    let cle = Marqueur::ContratClausesParticulieres;
    let dites = valeurs.is_none_or(|v| v.get(&cle).is_some_and(|x| !x.trim().is_empty()));
    if dites && !ecrits.iter().any(|(x, c)| *x == cle && c.is_none_or(|c| c == cle)) {
        dits.push(format!("Il manque {} : {}.", cle, CLAUSES_EXIGEES));
    }
    Ok(dits)
}

/// Pourquoi tout modele de contrat cite les clauses particulieres de l'engagement : la decision 8 de Leandre (ticket
/// #403 du prive, une seule facon de personnaliser), pas un texte de loi. Elles ne s'impriment que par lui ; un modele
/// qui ne le citerait pas les tairait sans le dire.
const CLAUSES_EXIGEES: &str = "les clauses particulières qu'un engagement dit ne s'impriment que par lui, dans un alinéa [si contrat.clauses_particulieres] ; un modèle qui ne le cite pas les perdrait sans le dire";

/// LES CHAMPS QU'UN MODELE CITE, lieu par lieu (le titre, le preambule, la phrase d'annulation, le bloc de modification,
/// chaque article, les signatures) : les manques de grammaire des champs (un champ inconnu, ou que ce modele ne porte
/// pas ; une condition sur un champ inconnu ; une valeur qui peut etre vide hors de son alinea `[si …]`), et chaque champ
/// cite avec la condition de son alinea. Un contrat porte les champs de `CHAMPS` ; `annulation` : ceux que sa phrase
/// d'annulation ajoute ; `partout` : ceux qu'un avenant ajoute.
fn champs_cites(m: &fabrique_pdf::modele::Modele, annulation: &[Marqueur], partout: &[Marqueur]) -> (Vec<String>, Vec<(Marqueur, Option<Marqueur>)>) {
    let permis = |c: &Marqueur| CHAMPS.iter().any(|(k, _, _)| k == c) || partout.contains(c);
    let mut lieux: Vec<(String, &[String], &[Marqueur])> = vec![
        ("le titre".into(), std::slice::from_ref(&m.titre), &[]),
        ("le sous-titre".into(), std::slice::from_ref(&m.soustitre), &[]),
        ("le préambule".into(), &m.preambule, &[]),
        ("la phrase d'annulation".into(), &m.annule, annulation),
        ("le bloc de modification".into(), &m.modification, &[]),
    ];
    for (n, (titre, alineas)) in m.articles.iter().enumerate() {
        lieux.push((format!("l'article {} « {} »", n + 1, titre), alineas, &[]));
    }
    lieux.push(("les signatures".into(), &m.signatures, &[]));
    let (mut dits, mut ecrits) = (Vec::new(), Vec::new());
    for (ou, alineas, en_plus) in &lieux {
        for a in alineas.iter() {
            let (condition, corps) = match fabrique_pdf::modele::condition(a) {
                Some((c, t)) => match Marqueur::lire(c).filter(|x| permis(x)) {
                    Some(x) => (Some(x), t),
                    None => {
                        dits.push(format!("Dans {}, la condition [si {}] ne nomme pas un champ du logiciel.", ou, c));
                        continue;
                    }
                },
                None => (None, a.as_str()),
            };
            for cite in crate::marqueurs::cites(corps) {
                match cite {
                    Ok(x) if permis(&x) || en_plus.contains(&x) => {
                        if PEUVENT_ETRE_VIDES.contains(&x) && condition != Some(x) {
                            dits.push(format!("Dans {}, {} peut être vide : l'écrire dans un alinéa qui commence par [si {}].", ou, x, x.nom()));
                        }
                        ecrits.push((x, condition));
                    }
                    Ok(x) => dits.push(format!("Dans {}, {} n'est pas un champ de ce modèle.", ou, x)),
                    Err(inconnu) => dits.push(format!("Dans {}, {{{{{}}}}} n'est pas un champ du logiciel : le corriger d'après la liste des champs, ou l'écrire en texte.", ou, inconnu)),
                }
            }
        }
    }
    (dits, ecrits)
}

/// LE VERIFICATEUR D'UN MODELE D'AVENANT, le meme que celui du contrat (`champs_cites`), ses champs en plus
/// (`CHAMPS_DE_L_AVENANT`) : il a un bloc MODIFICATION:, qui dit ce qui change (R. 1221-40 pour un permanent :
/// `atelier/reference/sources/code-du-travail/xml/R1221-40.xml`), et ce bloc porte `modification.phrase`, seul a s'ecrire
/// pour un contrat signe hors du logiciel, dont aucun article ne se cite. Rend les manques ; vide : le modele est bon.
pub fn manques_de_l_avenant(texte: &str) -> Vec<String> {
    let m = match fabrique_pdf::modele::lire_modele(texte, "le modèle d'avenant") {
        Ok(m) => m,
        Err(r) => return vec![format!("Ce modèle d'avenant ne se lit pas : {}.", r.0)],
    };
    let (mut dits, _) = champs_cites(&m, &[], &CHAMPS_DE_L_AVENANT.map(|(c, _)| c));
    if m.modification.is_empty() {
        dits.push("L'avenant n'a pas de bloc MODIFICATION: : c'est lui qui dit, article par article, ce qui change.".into());
    } else if !m.modification.iter().any(|b| crate::marqueurs::cites(b).contains(&Ok(Marqueur::ModificationPhrase))) {
        dits.push(format!("Le bloc de modification de l'avenant ne cite pas {} : un contrat signé hors du logiciel n'a pas d'article à citer, l'avenant y dit ce qui change en clair.", Marqueur::ModificationPhrase));
    }
    dits
}

/// LE MODELE `m` FAIT-IL CE CONTRAT ? (`manques`, avec ses valeurs). `Err` : la phrase du refus, qui dit chaque manque et
/// quoi faire.
pub fn refuser_un_modele_incomplet(m: &ModeleDuContrat, nat: &Nature, valeurs: &Valeurs) -> Result<(), String> {
    let dits = manques(nat, &m.lu.texte, Some(valeurs))?;
    if dits.is_empty() {
        return Ok(());
    }
    let quoi_faire = if m.base { "C'est un défaut du logiciel : le signaler." } else { "Corriger ce modèle (Configuration › Modèles de contrat), ou choisir un autre modèle pour ce contrat." };
    Err(format!("{} ne fait pas ce contrat : {} {} Rien n'est écrit.", m.nom, dits.join(" "), quoi_faire))
}

/// LES CHAMPS QU'UN MODELE PEUT CITER, et ce que chacun imprime : ce que l'ecran montre a qui ecrit le modele de sa
/// compagnie. `(marqueur, ce qu'il imprime, natures ou il vaut)` ; « tous » : toute nature.
pub const CHAMPS: &[(Marqueur, &str, &str)] = &[
    (Marqueur::EmployeurRaisonSociale, "la raison sociale de la compagnie", "tous"),
    (Marqueur::EmployeurAdresse, "son adresse (la rue)", "tous"),
    (Marqueur::EmployeurCodePostal, "son code postal", "tous"),
    (Marqueur::EmployeurCommune, "sa commune", "tous"),
    (Marqueur::EmployeurSiret, "son SIRET", "tous"),
    (Marqueur::EmployeurNaf, "son code APE", "tous"),
    (Marqueur::EmployeurLicence, "sa licence d'entrepreneur de spectacles", "cddu_spectacle"),
    (Marqueur::EmployeurIdcc, "le numéro (IDCC) de sa convention collective", "tous"),
    (Marqueur::EmployeurConvention, "l'intitulé de sa convention collective", "tous"),
    (Marqueur::EmployeurCongesspectacles, "son numéro d'adhérent aux Congés Spectacles", "cddu_spectacle"),
    (Marqueur::EmployeurSignataire, "le prénom et le nom de qui signe pour elle", "tous"),
    (Marqueur::EmployeurSignataireFonction, "sa fonction", "tous"),
    (Marqueur::EmployeurQualiteDe, "« d' » ou « de », selon la fonction qui suit", "tous"),
    (Marqueur::SalarieNomComplet, "le prénom et le nom du salarié", "tous"),
    (Marqueur::SalarieNom, "son nom", "tous"),
    (Marqueur::SalariePrenom, "son prénom", "tous"),
    (Marqueur::SalarieDateNaissance, "sa date de naissance", "tous"),
    (Marqueur::SalarieLieuNaissance, "son lieu de naissance", "tous"),
    (Marqueur::SalarieAdresse, "son adresse (la rue)", "tous"),
    (Marqueur::SalarieCodePostal, "son code postal", "tous"),
    (Marqueur::SalarieCommune, "sa commune", "tous"),
    (Marqueur::SalarieNir, "son numéro de sécurité sociale", "tous"),
    (Marqueur::SalarieEmploi, "son emploi (le métier de l'engagement, ou de sa fiche)", "tous"),
    (Marqueur::SalarieQualiteDe, "« d' » ou « de », selon l'emploi qui suit", "tous"),
    (Marqueur::SalarieCategorie, "sa catégorie d'emploi (son classement conventionnel)", "tous"),
    (Marqueur::ContratNumero, "le numéro du contrat", "tous"),
    (Marqueur::ContratDebut, "la date d'embauche", "tous"),
    (Marqueur::ContratFin, "la date du terme", "cddu_spectacle cdd"),
    (Marqueur::ContratMotif, "le motif du contrat à durée déterminée", "cddu_spectacle cdd"),
    (Marqueur::ContratPersonneRemplacee, "le nom et la qualification de la personne remplacée, pour un remplacement ; vide sinon", "cdd"),
    (Marqueur::ContratDatesTravail, "les dates de travail, et le lieu de chacune", "cddu_spectacle"),
    (Marqueur::ContratLieu, "le lieu de travail", "tous"),
    (Marqueur::ContratDatePaiement, "la date du paiement", "tous"),
    (Marqueur::ContratPaiement, "la périodicité et le mode de paiement", "cdd cdi"),
    (Marqueur::ContratDureeTravail, "la durée du travail (et, à temps partiel, sa répartition)", "cdd cdi"),
    (Marqueur::ContratPeriodeEssai, "la durée de la période d'essai, ou « aucune »", "tous"),
    (Marqueur::ContratClausesParticulieres, "les clauses particulières que l'engagement dit ; vide sinon", "tous"),
    (Marqueur::RemunerationDetail, "la rémunération et ses composantes", "tous"),
    (Marqueur::RemunerationHorsBrut, "ce qui est versé en plus du brut (défraiements) ; vide sans eux", "tous"),
    (Marqueur::RemunerationBrut, "le brut du (premier) mois", "tous"),
    (Marqueur::RemunerationNetEstime, "le net à payer estimé du (premier) mois", "tous"),
    (Marqueur::RemunerationPeriode, "« le mois de … » quand le contrat se poursuit après le mois ; vide sinon", "cddu_spectacle"),
    (Marqueur::RemunerationCachets, "le nombre de cachets", "cddu_spectacle"),
    (Marqueur::RemunerationTauxCachet, "le taux du cachet, quand le contrat paie des cachets", "cddu_spectacle"),
    (Marqueur::SpectacleTitre, "le titre du spectacle", "cddu_spectacle"),
    (Marqueur::SpectacleNumeroObjet, "son numéro d'objet France Travail ; vide sans numéro", "cddu_spectacle"),
    (Marqueur::OrganismesRetraite, "le nom et l'adresse de la caisse de retraite complémentaire", "tous"),
    (Marqueur::OrganismesPrevoyance, "le nom et l'adresse de l'organisme de prévoyance, ou « aucun »", "tous"),
    (Marqueur::MentionsFormation, "le droit à la formation (un renvoi au code du travail)", "tous"),
    (Marqueur::MentionsConges, "les congés payés", "tous"),
    (Marqueur::MentionsRupture, "comment le contrat prend fin", "tous"),
    (Marqueur::MentionsProtectionSociale, "les régimes obligatoires et la protection sociale complémentaire", "tous"),
];

/// Ce que l'avenant ajoute aux champs du contrat, et ce que chacun imprime : les siens, et ceux de chaque modification
/// (le bloc MODIFICATION: se repete pour chacune).
pub const CHAMPS_DE_L_AVENANT: [(Marqueur, &str); 10] = [
    (Marqueur::AvenantNumero, "le numéro de l'avenant (1, 2… par contrat)"),
    (Marqueur::AvenantDateContrat, "le jour où le contrat modifié a été signé"),
    (Marqueur::AvenantReference, "le contrat modifié : « n° … du … », ou « commencé le … » s'il a été signé ailleurs"),
    (Marqueur::AvenantEffet, "la date d'effet de l'avenant"),
    (Marqueur::AvenantDate, "le jour où l'avenant est établi"),
    (Marqueur::ModificationNumero, "le numéro de l'article modifié"),
    (Marqueur::ModificationArticle, "le titre de l'article modifié"),
    (Marqueur::ModificationAvant, "un alinéa du contrat signé, tel qu'il s'y lit"),
    (Marqueur::ModificationApres, "le même alinéa, à jour"),
    (Marqueur::ModificationPhrase, "ce qui change en clair, pour un contrat signé ailleurs"),
];

/// Les champs d'un modele de cette nature, chacun avec ce qu'il imprime et, s'il est obligatoire, le texte qui l'exige ;
/// ceux d'un avenant (`avenant`) : les memes, aucun obligatoire, et les siens.
pub fn champs_des_modeles(nat: &Nature, avenant: bool) -> Result<Vec<ChampDeModele>, String> {
    let mut mentions = if avenant { Vec::new() } else { mentions(nat)? };
    if !avenant {
        mentions.push((Marqueur::ContratClausesParticulieres, CLAUSES_EXIGEES.to_string(), true));
    }
    let du_contrat = CHAMPS.iter().filter(|(_, _, ou)| *ou == "tous" || ou.split(' ').any(|n| n == nat.cle)).map(|(cle, quoi, _)| (*cle, *quoi));
    let siens = CHAMPS_DE_L_AVENANT.iter().filter(|_| avenant).copied();
    Ok(du_contrat
        .chain(siens)
        .map(|(cle, quoi)| {
            let exige = mentions.iter().find(|(m, _, _)| *m == cle);
            ChampDeModele { marqueur: cle.to_string(), quoi, obligatoire: exige.is_some(), texte: exige.map(|(_, t, _)| t.clone()), si_dite: exige.is_some_and(|(_, _, s)| *s) }
        })
        .collect())
}

/// Un champ qu'un modele peut citer : son marqueur, ce qu'il imprime, s'il est obligatoire et le texte qui l'exige (nul
/// sinon), s'il n'est du que dit.
#[derive(Debug, Clone, Serialize)]
pub struct ChampDeModele {
    pub marqueur: String,
    pub quoi: &'static str,
    pub obligatoire: bool,
    pub texte: Option<String>,
    pub si_dite: bool,
}
