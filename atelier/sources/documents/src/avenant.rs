//! L'AVENANT D'UN CONTRAT PARTI : ce que le contrat dirait aujourd'hui, compare a ce qu'il disait sur la version de
//! l'engagement sur laquelle il est parti (`engagement::etats::version_au_depart`). L'ancien texte n'est pas relu dans le
//! PDF signe : il s'ecrit du modele avec les valeurs de la version d'avant, telles qu'elles etaient ecrites (regle 4), et
//! chaque paragraphe des articles du modele qui differe d'une version a l'autre est une modification (`modifications`).
//!
//! Ici, les valeurs que le modele de contrat lit dans une paie (`valeurs_de_la_paie`, `contrat.marqueurs`), le contrat
//! parti que le journal connait (`contrat_parti`) et ses avenants. Le texte de l'avenant est celui du modele du contrat
//! (`contrat_ecrit::ModeleDuContrat::avenant`) : la base de sa nature (`contrat_ecrit.avenant` de `contrats-base.json`)
//! ou celui d'un modele special.


use droit::fiche::Lue as _;

use crate::marqueurs::{pour_la_fabrique, Marqueur, Valeurs};
use fabrique_pdf::modele::{self, Modele};
use serde::Serialize;

use crate::bulletin::{convention_du_contrat, fmt_eur};
use droit::contexte::Contexte;
use droit::fiche::ChampsEntreprise;
use engagement::paie_ecrite::{FormeDEngagement, FormeDePaie, FormeDeRemuneration};
use engagement::saisie::Valeur;
use socle::date::Jour;
use socle::mois::Mois;
use droit::emplois::{emploi_de, EmploiEngagement};
use socle::nfkc::nfkc;
use socle::echec::{refus, R};


/// Ce qui remplace une valeur qui n'est pas un texte : elle ne se refuse que si le modele s'en
/// sert. Une phrase qui la porte est un refus.
const INVALIDE: &str = "\u{0}\u{0}VALEUR-SANS-TEXTE\u{0}\u{0}";


// ------------------------------------------------------ le texte qu'on compare

use socle::texte::blanc as est_blanc;

/// Ce qui se compare. Ligatures defaites (NFKC : « ﬀ » -> « ff », espaces
/// insecables -> espaces), une coupure apres un trait d'union recollee, les blancs ramenes a un seul.
pub fn normaliser(texte: &str) -> String {
    let t: Vec<char> = nfkc(texte).chars().collect();
    let mut recolle = String::with_capacity(t.len());
    let mut i = 0;
    while i < t.len() {
        if t[i] == '-' && i + 2 < t.len() && t[i + 1] == '\n' && !est_blanc(t[i + 2]) {
            recolle.push('-');
            i += 2;
        } else {
            recolle.push(t[i]);
            i += 1;
        }
    }
    let mut sortie = String::with_capacity(recolle.len());
    let mut dans_blanc = false;
    for c in recolle.chars() {
        if est_blanc(c) {
            if !dans_blanc {
                sortie.push(' ');
            }
            dans_blanc = true;
        } else {
            sortie.push(c);
            dans_blanc = false;
        }
    }
    sortie.trim_matches(' ').to_string()
}

// ------------------------------------------------------ les valeurs du modele

/// Une case texte de la fiche du salarie, que le modele exige : vide, elle se refuse en se nommant.
fn case<T: AsRef<str>>(c: &Option<T>, cle: &str) -> R<String> {
    c.ecrite(cle).map(str::to_string)
}

/// La forme lisible d'une case de la fiche du salarie si elle est dite, la forme declaree sinon (`case`).
fn lisible<T: AsRef<str>>(affichage: &Option<String>, c: &Option<T>, cle: &str) -> R<String> {
    match affichage.remplie() {
        Some(a) => Ok(a.to_string()),
        None => case(c, cle),
    }
}

/// « d' » devant une voyelle ou un h, « de » sinon.
pub(crate) fn de_devant(mot: &str) -> &'static str {
    let t: String = mot.chars().take(1).flat_map(char::to_lowercase).collect();
    if t.is_empty() || "aeiouyhâàéèêîôûœ".contains(&t) {
        "d'"
    } else {
        "de "
    }
}

/// « a, b et c ».
pub(crate) fn et(morceaux: &[String]) -> String {
    match morceaux.len() {
        0 => String::new(),
        1 => morceaux[0].clone(),
        n => format!("{} et {}", morceaux[..n - 1].join(", "), morceaux[n - 1]),
    }
}

/// Ce que `contrat.marqueurs` lit de l'engagement : sa forme ecrite (`paie_ecrite::FormeDEngagement`, dans la paie
/// calculee ou ecrite), et le numero que la paie porte vraiment.
pub(crate) struct Eng<'a> {
    f: &'a FormeDEngagement,
    numero: Option<engagement::numeros::NumeroContrat>,
}

impl<'a> Eng<'a> {
    pub(crate) fn de(f: &'a FormeDEngagement) -> Eng<'a> {
        Eng { f, numero: f.numero.clone() }
    }

    /// `Engagement.dates_du_contrat` : les jours du mois, puis le planning.
    fn dates_du_contrat(&self) -> Vec<&Jour> {
        self.f.dates_travail.iter().chain(self.f.planning.iter()).collect()
    }

    /// `Engagement.lieu_de` : le lieu que la date dit l'emporte sur celui de l'engagement.
    fn lieu_de(&self, jour: &Jour) -> &str {
        match [self.f.lieux.get(jour), self.f.planning_lieux.get(jour)].into_iter().flatten().find(|l| !l.is_empty()) {
            Some(l) => l.as_str(),
            None => self.f.lieu.as_str(),
        }
    }

    /// `Engagement.lieux_joues` : dans l'ordre des dates, sans repetition.
    fn lieux_joues(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for j in self.dates_du_contrat() {
            let ou = self.lieu_de(j);
            if !ou.is_empty() && !out.contains(&ou) {
                out.push(ou);
            }
        }
        out
    }

    /// `Engagement.dates_par_lieu()` : [(lieu, [dates])] dans l'ordre des dates, sans retrier.
    fn dates_par_lieu(&self) -> Vec<(&str, Vec<&Jour>)> {
        let mut out: Vec<(&str, Vec<&Jour>)> = Vec::new();
        for j in self.dates_du_contrat() {
            let ou = self.lieu_de(j);
            match out.last_mut() {
                Some((dernier, dates)) if *dernier == ou => dates.push(j),
                _ => out.push((ou, vec![j])),
            }
        }
        out
    }
}

/// « 2 cachets à 130,00 € » : une ligne de remuneration telle que le contrat la cite.
pub(crate) fn ligne_citee(l: &FormeDeRemuneration) -> String {
    let designation = &l.designation;
    // La designation ouvre une phrase : « 2 Cachets » y devient « 2 cachets ». On ne touche
    // qu'a la premiere majuscule.
    let mut texte = String::new();
    let mut fait = false;
    for c in designation.chars() {
        if !fait && c.is_uppercase() {
            texte.extend(c.to_lowercase());
            fait = true;
        } else {
            texte.push(c);
        }
    }
    format!("{} à {}", texte, socle::texte::euros(l.taux))
}

/// La ligne fait-elle le brut ? (`soumis` de la rubrique.)
pub(crate) fn fait_le_brut(l: &FormeDeRemuneration) -> bool {
    l.soumis
}

/// « 2 cachets à 130,00 € », ou « 1 journée de travail à
/// 200,00 € et 6 h de masterclass à 40,00 € ». Le modele la dit « brut » : elle ne cite que les
/// lignes qui font le brut ; les autres (les defraiements...) sont dans `remuneration_hors_brut`.
fn remuneration_detaillee(remuneration: &[FormeDeRemuneration], e: &Eng) -> R<String> {
    let morceaux: Vec<String> = remuneration.iter().filter(|l| fait_le_brut(l)).map(ligne_citee).collect();
    let mut phrase = et(&morceaux);
    if !e.f.dernier_mois {
        let quand = e.f.mois.libelle(socle::mois::Graphie::Accentuee);
        if !e.f.planning.is_empty() {
            let n = e.f.planning.len();
            phrase += &format!(
                " pour {} et, pour {} du planning, payée{} dans {} mois, le même montant",
                quand,
                if n > 1 { format!("les {} dates suivantes", n) } else { "la date suivante".to_string() },
                if n > 1 { "s" } else { "" },
                if n > 1 { "leur" } else { "son" }
            );
        } else {
            phrase += &format!(" pour {} et, pour les dates suivantes du planning, payées dans leur mois, le même montant", quand);
        }
    }
    Ok(phrase)
}

/// Ce que le contrat verse EN PLUS du brut -- les lignes non soumises qui s'ajoutent (defraiements,
/// indemnites hors assiette), convenues comme le reste : « 3 défraiements au forfait à 20,20 € ». `None` sans elles :
/// l'alinea du modele qui les dit ne s'ecrit pas (`[si remuneration.hors_brut]`). Une ligne qui retranche (un acompte)
/// n'est pas une clause du contrat.
pub(crate) fn remuneration_hors_brut(remuneration: &[FormeDeRemuneration]) -> Option<String> {
    let morceaux: Vec<String> =
        remuneration.iter().filter(|l| !fait_le_brut(l) && l.montant > socle::num::Decimal::ZERO).map(ligne_citee).collect();
    (!morceaux.is_empty()).then(|| et(&morceaux))
}

/// « le mois d'octobre 2026 » quand le contrat se poursuit apres le mois : le total et le net sont ceux de ce mois.
fn periode_remuneration(e: &Eng) -> R<Option<String>> {
    if e.f.dernier_mois {
        return Ok(None);
    }
    let mois = e.f.mois;
    let nom = mois.nom(socle::mois::Graphie::Accentuee);
    let de = if nom.chars().next().is_some_and(|c| "aeiouyâàéèêîôû".contains(c)) { "d'" } else { "de " };
    Ok(Some(format!("le mois {}{} {}", de, nom, mois.annee())))
}

/// Les jours joues, et OU chacun se joue quand ils different.
fn dates_travail(e: &Eng) -> R<String> {
    let mut texte = if e.lieux_joues().len() < 2 {
        e.dates_du_contrat().into_iter().map(Jour::jj_mm_aaaa).collect::<Vec<_>>().join(", ")
    } else {
        let mut groupes = Vec::new();
        for (ou, jours) in e.dates_par_lieu() {
            let dates = et(&jours.into_iter().map(Jour::jj_mm_aaaa).collect::<Vec<_>>());
            groupes.push(if ou.is_empty() { dates } else { format!("{} à {}", dates, ou) });
        }
        groupes.join(", ")
    };
    if !e.f.dernier_mois && e.f.planning.is_empty() {
        let fin = e.f.fin.ok_or_else(|| refus("contrat sans fin"))?;
        texte += &format!(", puis les dates du planning jusqu'au {}", fin.jj_mm_aaaa());
    }
    Ok(texte)
}

/// L'article « Lieu de travail », qui suit ce que les dates ont dit.
fn lieu_du_contrat(e: &Eng) -> String {
    let lieux = e.lieux_joues();
    match lieux.len() {
        0 => "au lieu de la représentation".to_string(),
        1 => format!("à {}", lieux[0]),
        _ => "aux lieux indiqués ci-dessus".to_string(),
    }
}

/// Les valeurs offertes au modele. `remuneration`, `brut` et `net` sont ceux
/// de la paie calculee ; `fiche` celle du salarie telle qu'elle vaut pour cet engagement.
fn marqueurs<A>(ent: &ChampsEntreprise, fiche: &droit::fiche::ChampsSalarie, sp: &droit::fiche::Spectacle, e: &Eng, paie: &FormeDePaie<A>) -> R<Valeurs> {
    let remuneration = &paie.remuneration;
    let licence = ent.licence_spectacle.ecrite("licence_spectacle")?.to_string();
    let mut m = parties(ent, fiche, Some(sp), e)?;
    let mut poser = |k: Marqueur, v: String| {
        m.insert(k, v);
    };
    poser(Marqueur::EmployeurLicence, licence);
    // Le numero sous lequel la compagnie est affiliee a la Caisse : son numero d'adherent (`conges_spectacles::adherent`).
    poser(Marqueur::EmployeurCongesspectacles, crate::conges_spectacles::adherent(ent)?);
    poser(Marqueur::ContratFin, e.f.fin.ok_or_else(|| refus("contrat sans fin"))?.jj_mm_aaaa());
    poser(Marqueur::ContratDatesTravail, dates_travail(e)?);
    poser(Marqueur::ContratLieu, lieu_du_contrat(e));
    // Le jour du versement de la paie (`engagement::Engagement::date_du_versement`, que la paie ecrite porte).
    let paiement = e.f.date_paiement.ok_or_else(|| socle::echec::cle("Un engagement ne se relit pas : le revalider.", "date_paiement"))?;
    poser(Marqueur::ContratDatePaiement, paiement.jj_mm_aaaa());
    poser(Marqueur::RemunerationDetail, remuneration_detaillee(remuneration, e)?);
    if let Some(p) = periode_remuneration(e)? {
        poser(Marqueur::RemunerationPeriode, p);
    }
    if let Some(h) = remuneration_hors_brut(remuneration) {
        poser(Marqueur::RemunerationHorsBrut, h);
    }
    poser(Marqueur::RemunerationCachets, e.f.cachets.to_string());
    // Sans taux de cachet, pas de marqueur : un modele qui le cite se refuse (`contrat::refuser_ce_que_le_modele_tairait`).
    if let Some(t) = e.f.taux_cachet {
        poser(Marqueur::RemunerationTauxCachet, fmt_eur(t));
    }
    poser(Marqueur::RemunerationBrut, fmt_eur(paie.brut));
    poser(Marqueur::RemunerationNetEstime, fmt_eur(paie.net_a_payer));
    poser(Marqueur::SpectacleTitre, case(&sp.titre, "titre")?);
    // LE NUMERO D'OBJET se reporte sur le contrat « a chaque fois que cela est possible » (annexes VIII et X, art. 47 § 3 :
    // atelier/reference/sources/plafond-intermittents/unedic-annexe8-convention-20241115.txt l. 1183-1184, et l'annexe X,
    // l. 1161-1162) : un spectacle qui n'en a pas encore fait son contrat, qui n'en dit rien. Le bulletin, lui, l'exige
    // (`exige_par` de `champs-base.json`). Sans numero, pas de valeur : l'alinea qui le dit ne s'ecrit pas.
    if let Some(n) = sp.numero_objet.dite().map(|n| n.trim()).filter(|n| !n.is_empty()) {
        poser(Marqueur::SpectacleNumeroObjet, n.to_string());
    }
    Ok(m)
}

/// Les parties au contrat, quelle que soit sa nature : l'employeur, sa convention et son signataire, le salarie, le
/// numero et le premier jour du contrat. `sp` : le spectacle (`None` pour un contrat qui n'en a pas). La convention se
/// nomme comme le bareme du mois de la paie qui ecrit le document la nomme (`bareme::libelle_convention`) : le premier
/// mois d'un contrat pour le contrat, le mois de l'avenant pour un avenant -- un contrat commence avant le logiciel n'a
/// pas de bareme de son annee.
pub(crate) fn parties(ent: &ChampsEntreprise, fiche: &droit::fiche::ChampsSalarie, sp: Option<&droit::fiche::Spectacle>, e: &Eng) -> R<Valeurs> {
    let (idcc, convention) = convention_du_contrat(ent, sp, fiche, &e.f.mois.premier_jour())?;
    let sig = ent.signataire.exigee("signataire")?;
    let mut m = Valeurs::new();
    let mut poser = |k: Marqueur, v: String| {
        m.insert(k, v);
    };
    poser(Marqueur::EmployeurRaisonSociale, case(&ent.raison_sociale, "raison_sociale")?);
    let complement = ent.complement_adresse_affichage.remplie().or(ent.complement_adresse.remplie());
    poser(Marqueur::EmployeurAdresse, droit::fiche::avec_complement(lisible(&ent.adresse_affichage, &ent.adresse, "adresse")?, complement));
    poser(Marqueur::EmployeurCodePostal, case(&ent.code_postal, "code_postal")?);
    poser(Marqueur::EmployeurCommune, lisible(&ent.commune_affichage, &ent.commune, "commune")?);
    poser(Marqueur::EmployeurSiret, case(&ent.siret, "siret")?);
    poser(Marqueur::EmployeurNaf, case(&ent.naf, "naf")?);
    poser(Marqueur::EmployeurIdcc, idcc);
    poser(Marqueur::EmployeurConvention, convention);
    poser(
        Marqueur::EmployeurSignataire,
        format!("{} {}", lisible(&sig.prenom_affichage, &sig.prenom, "prenom")?, lisible(&sig.nom_affichage, &sig.nom, "nom")?),
    );
    let fonction = case(&sig.fonction, "fonction")?;
    // « en qualité d'administratrice », « de président » : l'élision devant la fonction de qui signe.
    poser(Marqueur::EmployeurQualiteDe, de_devant(&fonction).to_string());
    poser(Marqueur::EmployeurSignataireFonction, fonction);
    // La case du salarie au bas du contrat : « L'artiste » d'un engagement ; celui d'un permanent dit « Le salarié »
    // (`contrat_ecrit::marqueurs_au_mois`, qui suit).
    poser(Marqueur::SalarieCaseSignature, "L'artiste".into());
    let s = fiche;
    poser(Marqueur::SalarieNom, case(&s.nom, "nom")?);
    poser(Marqueur::SalariePrenom, lisible(&s.prenom_affichage, &s.prenom, "prenom")?);
    poser(Marqueur::SalarieNomComplet, format!("{} {}", lisible(&s.prenom_affichage, &s.prenom, "prenom")?, case(&s.nom, "nom")?));
    poser(Marqueur::SalarieDateNaissance, s.date_naissance.exigee("date_naissance")?.jj_mm_aaaa());
    poser(Marqueur::SalarieLieuNaissance, lisible(&s.lieu_naissance_affichage, &s.lieu_naissance, "lieu_naissance")?);
    poser(Marqueur::SalarieAdresse, lisible(&s.adresse_affichage, &s.adresse, "adresse")?);
    poser(Marqueur::SalarieCodePostal, case(&s.code_postal, "code_postal")?);
    poser(Marqueur::SalarieCommune, lisible(&s.commune_affichage, &s.commune, "commune")?);
    // Le numero technique temporaire n'a rien a faire ici : un salarie pas encore immatricule lit
    // ce qui est vrai.
    poser(
        Marqueur::SalarieNir,
        match s.nir.remplie() {
            Some(nir) => format!("{}{}", nir, s.nir_cle.ecrite("nir_cle")?),
            None => "en cours d'immatriculation".to_string(),
        },
    );
    let emploi_texte = lisible(&s.emploi_affichage, &s.emploi, "emploi")?;
    if emploi_texte == INVALIDE {
        return Err(refus("l'emploi n'est pas un texte"));
    }
    poser(Marqueur::SalarieEmploi, emploi_texte.to_lowercase());
    poser(Marqueur::SalarieQualiteDe, de_devant(&emploi_texte).to_string());
    // Un numero que le moteur ne porte pas (contrat pas encore numerote) n'est pas devine ni pris au compteur.
    poser(
        Marqueur::ContratNumero,
        match e.numero.as_deref() {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => INVALIDE.to_string(),
        },
    );
    poser(Marqueur::ContratDebut, e.f.debut.jj_mm_aaaa());
    Ok(m)
}

/// Le paragraphe tel qu'il se LIT sur le contrat imprime.
fn texte_brut(texte: &str, vals: &modele::Valeurs) -> R<String> {
    let t = modele::texte_brut(texte, vals).map_err(crate::fabrique::echec)?;
    if t.contains(INVALIDE) {
        return Err(refus("une valeur du modele n'est pas un texte"));
    }
    Ok(t)
}

// ------------------------------------------------- ce que l'avenant modifie

/// Un article du contrat qui a change depuis la version sur laquelle il est parti : son numero, son titre, et pour chaque
/// paragraphe change l'ancien texte, le nouveau et son rang dans l'article. Un alinea conditionnel qui n'etait pas ecrit
/// n'a pas d'ancien texte, celui qui ne l'est plus pas de nouveau : `avant` et `apres` n'ont pas toujours autant de textes.
pub struct Modification {
    pub numero: usize,
    pub article: String,
    pub avant: Vec<String>,
    pub apres: Vec<String>,
    pub blocs: Vec<usize>,
}

/// Les articles du modele `m` dont un paragraphe s'ecrit autrement avec les valeurs d'aujourd'hui (`apres`) qu'avec celles
/// de la version sur laquelle le contrat est parti (`avant`), dans l'ordre du modele.
pub fn modifications(m: &Modele, avant: &Valeurs, apres: &Valeurs) -> R<Vec<Modification>> {
    let (avant, apres) = (&pour_la_fabrique(avant), &pour_la_fabrique(apres));
    let mut out = Vec::new();
    for (i, (titre, corps)) in m.articles.iter().enumerate() {
        // Le numero que le contrat signe lui donne ; un paragraphe qu'il taisait prend celui du texte a jour.
        let n = if modele::s_imprime(corps, avant) { m.numero_imprime(i, avant) } else { m.numero_imprime(i, apres) };
        let (mut anciens, mut neufs, mut blocs): (Vec<String>, Vec<String>, Vec<usize>) = (Vec::new(), Vec::new(), Vec::new());
        for (i, bloc) in corps.iter().enumerate() {
            // Un alinea conditionnel ote ne s'ecrit pas (`modele::retenu`) : l'avenant ne le cite pas, ni avant s'il
            // n'y etait pas, ni apres s'il n'y est plus.
            let ecrit = |v: &modele::Valeurs| -> R<Option<String>> { modele::retenu(bloc, v).map(|t| texte_brut(t, v).map(|s| normaliser(&s))).transpose() };
            let (ancien, neuf) = (ecrit(avant)?, ecrit(apres)?);
            if ancien != neuf {
                anciens.extend(ancien);
                neufs.extend(neuf);
                blocs.push(i);
            }
        }
        if !blocs.is_empty() {
            out.push(Modification { numero: n, article: texte_brut(titre, apres)?, avant: anciens, apres: neufs, blocs });
        }
    }
    Ok(out)
}

/// LA MODIFICATION D'UN APERCU D'AVENANT : un modele d'avenant ne s'imprime qu'avec ce qui change. L'apercu cite le
/// premier article que le contrat `m` imprime avec les valeurs `v`, tel qu'il est, avant comme apres : il montre la forme
/// de l'avenant, rien ne change.
pub fn modification_d_apercu(m: &Modele, v: &Valeurs) -> R<Modification> {
    let v = &pour_la_fabrique(v);
    let i = m.articles.iter().position(|(_, a)| modele::s_imprime(a, v)).ok_or_else(|| refus("Le modèle du contrat n'imprime aucun article : l'avenant n'a rien à citer."))?;
    let (titre, corps) = &m.articles[i];
    let mut textes = Vec::new();
    for bloc in corps {
        if let Some(t) = modele::retenu(bloc, v) {
            textes.push(normaliser(&texte_brut(t, v)?));
        }
    }
    Ok(Modification { numero: m.numero_imprime(i, v), article: texte_brut(titre, v)?, blocs: (0..textes.len()).collect(), avant: textes.clone(), apres: textes })
}

/// La fiche entreprise, avec la licence du mois et les gabarits de la compagnie.
pub fn ent_du_mois(ctx: &Contexte, mois: Mois) -> droit::contexte::EntrepriseDuDocument {
    let mut ent = ctx.entreprise.clone();
    ent.licence_spectacle = ctx.licence(&mois.premier_jour());
    ctx.pour_un_modele(&ent)
}

/// `contrat.marqueurs(p, ent)` pour une paie (sa forme ecrite ou canonique, `paie_ecrite::FormeDePaie`). `numero` : le
/// numero de contrat que la paie porte VRAIMENT (lu dans l'archive, ou celui de l'engagement), quand il differe de celui
/// de la forme. Le contrat d'un spectacle lit ses dates, ses cachets et son spectacle ; celui d'un permanent son salaire
/// au mois et sa duree du travail (`contrat_ecrit::marqueurs_au_mois`) ; tous deux les mentions que la loi exige de tout
/// contrat ecrit (`contrat_ecrit::poser_les_mentions`).
pub fn valeurs_de_la_paie<A>(ctx: &Contexte, ent: &ChampsEntreprise, paie: &FormeDePaie<A>, numero: Option<&engagement::numeros::NumeroContrat>) -> R<Valeurs> {
    let eng = &paie.engagement;
    let nat = eng.nature.proprietes()?;
    let mut e = Eng::de(eng);
    if let Some(n) = numero {
        e.numero = Some(n.clone());
    }
    let texte = |c: &Option<String>| c.clone().filter(|s| !s.is_empty());
    let fiche = ctx.fiche_de(&eng.salarie)?;
    let emploi = EmploiEngagement {
        emploi: texte(&eng.emploi),
        emploi_affichage: texte(&eng.emploi_affichage),
        pcs_ese: eng.pcs_ese.clone().filter(|s| !s.is_empty()),
        code_emploi_ft: eng.code_emploi_ft.clone(),
        statut: eng.statut,
        statut_conventionnel: eng.statut_conventionnel,
        statut_categoriel_rc: eng.statut_categoriel_rc,
    };
    let lue = emploi_de(&ctx.salarie_au(&fiche.id, &eng.debut)?, &emploi)?;
    let mut m = match &eng.spectacle {
        Some(s) => {
            let sp = ctx.spectacle(s.as_str())?;
            marqueurs(ent, &lue, &sp, &e, paie)?
        }
        None => {
            let mut m = parties(ent, &lue, None, &e)?;
            crate::contrat_ecrit::marqueurs_au_mois(paie, &nat, &mut m)?;
            m
        }
    };
    crate::contrat_ecrit::poser_les_mentions(ctx, ent, &lue, eng, &nat, &mut m)?;
    Ok(m)
}


// ------------------------------------------- l'avenant d'un contrat signe ailleurs

/// Un terme du contrat, tel que l'avenant d'un contrat signe hors du logiciel le nomme : son titre, et son nom dans une
/// phrase.
fn terme_nomme(terme: &str) -> (&'static str, &'static str) {
    match terme {
        "salaire_mensuel" => ("Rémunération mensuelle brute", "la rémunération mensuelle brute"),
        "quotite_contrat" => ("Durée du travail", "la durée du travail"),
        "quotite_reference" => ("Durée de référence", "la durée de référence"),
        "modalite_temps_travail" => ("Temps de travail", "la modalité du temps de travail"),
        "emploi" => ("Emploi", "l'emploi"),
        "metier" => ("Métier", "le métier"),
        "cadre" => ("Statut de cadre", "le statut de cadre"),
        "statut" => ("Statut", "le statut"),
        "nature" => ("Nature du contrat", "la nature du contrat"),
        "motif_recours" => ("Motif de recours", "le motif de recours"),
        "lieu" => ("Lieu de travail", "le lieu de travail"),
        "spectacle" => ("Spectacle", "le spectacle"),
        _ => ("Clause du contrat", "une clause du contrat"),
    }
}

/// La valeur d'un terme en clair : un montant en euros, une duree en heures par mois (S21.G00.40.013), une nature, un
/// motif ou un spectacle par son nom.
fn valeur_en_clair(ctx: &Contexte, nat: &droit::natures::Nature, terme: &str, v: &Valeur) -> R<String> {
    let texte = match v {
        Valeur::Nature(n) => return Ok(n.proprietes()?.libelle.clone()),
        Valeur::Texte(s) => s.clone(),
        Valeur::Nombre(n) => engagement::Chiffre::Nombre(n.clone()).en_texte(),
        Valeur::Oui(true) => "True".into(),
        Valeur::Oui(false) => "False".into(),
    };
    // Un montant ou une duree : un nombre, ou un nombre ecrit en texte (la virgule decimale admise).
    let nombre = || -> R<socle::num::Decimal> {
        match v {
            Valeur::Nombre(n) => engagement::Chiffre::Nombre(n.clone()).dec(),
            Valeur::Texte(s) => socle::num::decimal_a_virgule(s).ok_or_else(|| refus(socle::num::illisible(s))),
            Valeur::Oui(_) | Valeur::Nature(_) => Err(socle::refus!("{} : un nombre est attendu, lu {}", terme, texte)),
        }
    };
    Ok(match terme {
        "salaire_mensuel" => socle::texte::euros(nombre()?),
        "quotite_contrat" | "quotite_reference" => format!("{} heures par mois", socle::texte::virgule(nombre()?)),
        "cadre" => {
            let cadre = match v {
                Valeur::Oui(b) => *b,
                Valeur::Nombre(n) => !n.est_nul(),
                Valeur::Texte(s) => !s.is_empty(),
                Valeur::Nature(_) => true,
            };
            if cadre { "cadre".into() } else { "non cadre".into() }
        }
        "motif_recours" => droit::natures::MotifDeRecours::du_code(&texte).and_then(|m| nat.libelle_motif_recours(m)).ok_or_else(|| socle::refus!("Motif de recours {} inconnu pour un {}.", texte, nat.libelle))?,
        "spectacle" => droit::fiche::Lue::ecrite(&ctx.spectacle(&texte)?.titre, "titre")?.to_string(),
        "emploi" => format!("« {} »", texte),
        _ => texte,
    })
}

/// UNE MODIFICATION DITE EN CLAIR (`changements_en_clair`) : le titre du terme, ses valeurs avant et apres (aucune quand
/// il n'en avait pas, ou n'en a plus), la phrase que l'avenant ecrit, le jour ; aucun article cite.
#[derive(Debug, Clone, Serialize)]
pub struct ChangementEnClair {
    pub article: String,
    pub phrase: String,
    pub jour: socle::date::Jour,
    pub avant: Vec<String>,
    pub apres: Vec<String>,
}

/// CE QUE DIT L'AVENANT D'UN CONTRAT SIGNE HORS DU LOGICIEL : il ne connait pas le texte du contrat, il n'en cite aucun
/// article ; il dit en clair, pour chaque terme qui change (`engagement::versions::termes_changes`), a partir de quand,
/// l'ancienne valeur et la nouvelle (code du travail R. 1221-40 : « un document indiquant ces modifications »,
/// `atelier/reference/sources/code-du-travail/xml/R1221-40.xml`).
pub fn changements_en_clair(ctx: &Contexte, nat: &droit::natures::Nature, changes: &[engagement::versions::TermeChange]) -> R<Vec<ChangementEnClair>> {
    changes
        .iter()
        .map(|c| {
            let (titre, nom) = terme_nomme(c.terme.nom());
            let dit = |v: &Option<Valeur>| v.as_ref().map(|x| valeur_en_clair(ctx, nat, c.terme.nom(), x)).transpose();
            let (avant, apres) = (dit(&c.avant)?, dit(&c.apres)?);
            let quand = format!("À compter du {}", c.jour.jj_mm_aaaa());
            let phrase = match (&avant, &apres) {
                (Some(a), Some(b)) => format!("{}, {} passe de {} à {}.", quand, nom, a, b),
                (None, Some(b)) => format!("{}, {} est : {}.", quand, nom, b),
                (Some(a), None) => format!("{}, {} n'est plus {}.", quand, nom, a),
                (None, None) => format!("{}, {} change.", quand, nom),
            };
            Ok(ChangementEnClair { article: titre.to_string(), phrase, jour: c.jour, avant: avant.into_iter().collect(), apres: apres.into_iter().collect() })
        })
        .collect()
}
