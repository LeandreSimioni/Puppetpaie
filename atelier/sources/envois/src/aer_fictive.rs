//! L'AER d'une COMPAGNIE FICTIVE : ce que France Travail renverrait.
//!
//! Chez une vraie compagnie, France Travail edite l'« Attestation employeur pour les activites relevant des annexes 8
//! et 10 » (AER Cinema Spectacle) a partir des DSN deposees, et la publie en retour de nature 46 : une archive .zip d'un
//! PDF par salarie (`retours::aer_spectacle`). Le logiciel la range, en remplit et signe le cadre 5
//! (`aer::authentifier`), et elle part au salarie avec ses documents. Une compagnie fictive ne depose rien nulle part :
//! ce module lui fabrique son AER, pour que ses intermittents la recoivent.
//!
//! Ici, on FABRIQUE cette archive a la place de France Travail, depuis les DSN que la compagnie fictive a jouees -- ce
//! que France Travail lit, et rien d'autre : le salarie (bloc 30), l'employeur et ses contrats d'intermittent (bloc 40,
//! `cinema_spectacle`) dont la DSN du mois declare la fin (bloc 62), et pour chacun ses periodes d'emploi, mois par
//! mois, lues dans la DSN reelle de
//! chaque mois du contrat (heures, cachets, jours, brut, date de paiement). Une attestation par salarie et par mois ; un
//! contrat sur plusieurs mois y porte tous ses mois (« 1 AE CS éditée à la fin du dernier mois concerné par le contrat »,
//! `aer::due`, sources citees la) ; cinq periodes au plus par PDF (les colonnes 01 a 05 du formulaire). Puis l'archive
//! suit LE MEME CHEMIN qu'une vraie (`releve_retours::rattacher_simulation` : l'archive dans `retours/`, puis
//! `routes::documents::ranger_aer`, qui signe le cadre 5 et la pose ou l'envoi la prend).
//!
//! LE GABARIT est celui du formulaire de France Travail -- trois pages A4, les cadres 1 (employeur) a 5
//! (authentification), et les NOMS DE CHAMPS AcroForm qu'il porte : `ENTETE_*`, `EMP_*`, `ADR_EMP_*`, `SALARIE_*`,
//! `CON_LIB_EMPLOI` ; au cadre 4, par colonne `nn`, `CON_NUMERO_nn`, `CON_NUM_DT_DEB_nn`, `CON_NUM_DT_FIN_nn`,
//! `CON_OBJET_nn` (le contrat, dans la colonne de sa premiere periode), `SAL_DT_DEB_nn`, `SAL_DT_FIN_nn`,
//! `SAL_NB_HEURES_nn`, `SAL_NB_CACHET_nn`, `SAL_TPS_TRA_nn`, `SAL_MONTANT_nn`, `SAL_DT_PAI_nn` (la periode) ; au cadre 5,
//! les `EMPLOYEUR_*` que remplit `aer.rs`. Relevé le 23/10/2026 sur une vraie attestation (octobre 2026), gardée dans le
//! depot prive (`reference-privee/exemples/`, une piece reelle : regle 9) ; d'elle, on ne reprend que ces noms et la
//! place des cadres, aucune valeur. Les champs imprimes sont en lecture seule, ceux du cadre 5 vides. Le PDF est un
//! formulaire de la fabrique (`fabrique_pdf::fabriquer`, puis `fabrique_pdf::formulaire::ajouter_champs`) ; il dit en
//! tete et en pied de chaque page, en rouge, qu'il est le document d'une compagnie fictive et qu'aucun organisme ne l'a
//! emis. Les cases a cocher du formulaire (licence, civilite, nationalite, cadre) ne sont pas reprises : la fabrique
//! n'ajoute que des champs texte ; ce qu'elles diraient s'ecrit en clair quand la DSN le dit.

use droit::fiche::Lue as _;
use std::io::Write;

use fabrique_pdf::forme::{AjoutDeChamps, Alignement, Bloc, ChampAjoute, Description, Document, Marges, Mesure, Page, Police, PoliceFixe, TexteFixe, Trait};
use fabrique_pdf::mesure::mesure as m;

use declarations::dsn_lue::{ContratDeclare, DsnLue, IndividuDeclare, Mesures};
use droit::contexte::Contexte;
use droit::fiche::Nir;
use socle::date::{Jour, Periode};
use socle::echec::{refus, R};
use socle::grandeurs::{Cachets, Heures, Jours};

/// Le « code » du nom d'archive (`AER_<siret>__<MMAAAA>_<code>_<JJMMAAAA>_<NOM>_<rang>_<id>.pdf`) : celui d'une fictive.
const CODE: &str = "FICTIVE";
/// La nature de contrat de la DSN d'un CDD de droit prive (S21.G00.40.007 ; `contrats-base.json`).
const NATURE_CDD: &str = "02";
/// Les colonnes du cadre 4 : 01 a 05 (`aer::lire_quantites`).
const COLONNES: usize = 5;

const ROUGE: &str = "#B22222";
const NOIR: &str = "#1F2A33";
const GRIS: &str = "#7A848C";
const BLANC: &str = "#FFFFFF";

// ------------------------------------------------------------------ ce que la DSN dit

/// Une rubrique que la DSN doit porter (`quoi` la dit en mots, `rubrique` va a part).
fn exigee<'a, T>(v: &'a Option<T>, quoi: &str, rubrique: droit::natures::RubriqueDeLaNorme) -> R<&'a T> {
    v.as_ref().ok_or_else(|| socle::echec::cle(format!("La DSN ne dit pas {}.", quoi), &rubrique.to_string()))
}

/// Le contrat releve-t-il de l'attestation Cinema Spectacle ? Ce que France Travail lit de la DSN : un CDD (40.007 = 02)
/// dont le code emploi (40.005) est de la nomenclature des emplois du spectacle, pas « 999SPT » (« Code complément PCS
/// ESE-S21.G00.40.005 doit être dans la table ART et <> « 999SPT » », `atelier/reference/sources/
/// net-entreprises-base-connaissances/net-entreprises-de-la-dsn-a-l-attestation-employeur-v3-2-20230328.txt`, l. 500-510).
fn cinema_spectacle(c: &ContratDeclare) -> bool {
    c.nature_dsn.as_deref() == Some(NATURE_CDD)
        && c.emploi.code_emploi_ft.as_ref().is_some_and(|e| !e.est_permanent())
}

// ------------------------------------------------------------------ les periodes

/// Une colonne du cadre 4 : une periode d'emploi d'un contrat, dans un mois ; `contrat` n'est porte que par la colonne
/// de sa premiere periode (comme sur le formulaire).
struct ColonneDuCadre4 {
    /// Le numero, le debut, la fin et le numero d'objet (s'il est dit) du contrat.
    contrat: Option<(engagement::numeros::NumeroContrat, String, String, Option<String>)>,
    /// La periode d'emploi : le contrat, borne au mois.
    periode: Periode,
    heures: Heures,
    cachets: Cachets,
    jours: Jours,
    /// Le brut declare ; sans remuneration declaree ce mois-ci, la case reste a blanc.
    brut: Option<socle::grandeurs::Montant>,
    paiement: Option<String>,
}

/// La DSN reelle deposee pour `m` (le journal), lue ; `None` si le mois n'en a pas. Un fichier depose qui ne se relit
/// pas refuse.
fn dsn_deposee(ctx: &Contexte, lignes: &[crate::journal::Ligne], m: socle::mois::Mois) -> R<Option<DsnLue>> {
    let Some(l) = crate::etats::etat_dsn_du_mois(lignes, m, None, ctx.entreprise.nic_principal()) else { return Ok(None) };
    let Some(fichier) = l.fichier else { return Ok(None) };
    let chemin = documents::fichier_depose(&ctx.racine, m, fichier.texte());
    declarations::dsn_lue::lire_une_dsn(&chemin)
}

/// Le jour du premier versement de l'individu `nir` dans la DSN `d`.
fn paiement_de(d: &DsnLue, nir: Option<&Nir>) -> Option<Jour> {
    d.versements_de(nir).next().and_then(|v| v.verse_le)
}

/// Les periodes d'un contrat de la DSN `d` qui finit ce mois-ci : une par mois du contrat dont la DSN reelle le porte.
fn periodes(ctx: &Contexte, lignes: &[crate::journal::Ligne], d: &DsnLue, le_mois: socle::mois::Mois, c: &ContratDeclare) -> R<Vec<ColonneDuCadre4>> {
    let numero = c.numero.as_ref().ok_or_else(|| socle::echec::cle("La DSN ne dit pas le numéro d'un contrat.", "S21.G00.40.009"))?;
    let du_contrat = c.periode.fermee().ok_or_else(|| socle::refus!("contrat {} : la DSN ne dit pas sa fin", numero))?;
    let mut out = Vec::new();
    // Les mois du contrat avant celui-ci.
    for m in du_contrat.debut().mois_civil().jusqu_a(le_mois.precedent()) {
        let Some(lue) = dsn_deposee(ctx, lignes, m)? else { continue };
        if lue.contrats_de(c.nir.as_ref()).any(|x| x.numero == c.numero) {
            let mesures = lue.mesures(c.nir.as_ref(), numero, false);
            out.push(colonne(c, numero, &mesures, &du_contrat, m, paiement_de(&lue, c.nir.as_ref()), out.is_empty())?);
        }
    }
    // Un contrat que ce fichier ne porte que pour le rappel d'un mois deja declare (`dsn_lue::Remuneration::rappel`) n'a
    // pas de periode ce mois-ci.
    let mesures = d.mesures(c.nir.as_ref(), numero, false);
    if mesures.remunere || out.is_empty() {
        out.push(colonne(c, numero, &mesures, &du_contrat, le_mois, paiement_de(d, c.nir.as_ref()), out.is_empty())?);
    }
    Ok(out)
}

fn colonne(c: &ContratDeclare, numero: &engagement::numeros::NumeroContrat, x: &Mesures, du_contrat: &Periode, m: socle::mois::Mois, paiement: Option<Jour>, premiere: bool) -> R<ColonneDuCadre4> {
    let periode = engagement::contrat_dans_le_mois(&c.periode, m, &format!("contrat {}", numero))?;
    Ok(ColonneDuCadre4 {
        contrat: premiere.then(|| (numero.clone(), du_contrat.debut().jj_mm_aaaa(), du_contrat.fin().jj_mm_aaaa(), c.objet.as_ref().map(|o| o.to_string()))),
        periode,
        heures: x.heures,
        cachets: x.cachets,
        jours: x.jours,
        brut: x.brut,
        paiement: paiement.map(|j| j.jj_mm_aaaa()),
    })
}

// ------------------------------------------------------------------ le dessin

// Le gabarit se releve en points : chaque position, chaque corps, chaque trait est une `Mesure`, ecrite par son texte.

/// Un rectangle `[x0, y0, x1, y1]`, en points.
fn boite(r: [&str; 4]) -> [Mesure; 4] {
    r.map(m)
}

/// Un texte pose en Helvetica, aligne par rapport a `x`.
fn texte_aligne(x: Mesure, y: Mesure, t: &str, taille: Mesure, gras: bool, couleur: &str, alignement: Alignement) -> Bloc {
    Bloc::TexteFixe(TexteFixe {
        texte: t.to_string(),
        x,
        y,
        taille: Some(taille),
        police: PoliceFixe::Helvetica,
        gras,
        couleur: Some(couleur.to_string()),
        alignement,
        largeur_max: None,
    })
}

fn texte(x: Mesure, y: Mesure, t: &str, taille: Mesure, gras: bool, couleur: &str) -> Bloc {
    texte_aligne(x, y, t, taille, gras, couleur, Alignement::Gauche)
}

fn centre(x: Mesure, y: Mesure, t: &str, taille: Mesure, gras: bool, couleur: &str) -> Bloc {
    texte_aligne(x, y, t, taille, gras, couleur, Alignement::Centre)
}

fn trait_en(x1: Mesure, y1: Mesure, x2: Mesure, y2: Mesure, ep: Mesure, couleur: &str) -> Bloc {
    Bloc::Trait(Trait { x1, y1, x2, y2, epaisseur: ep, couleur: couleur.to_string() })
}

fn case(b: &mut Vec<Bloc>, r: [Mesure; 4]) {
    let [x0, y0, x1, y1] = r;
    for (a, bb, c, d) in [(x0, y0, x1, y0), (x1, y0, x1, y1), (x1, y1, x0, y1), (x0, y1, x0, y0)] {
        b.push(trait_en(a, bb, c, d, m("0.4"), GRIS));
    }
}

/// Un filet sur toute la largeur du formulaire (de 30 a 565 points), a la hauteur `y`.
fn filet(y: Mesure, ep: Mesure, couleur: &str) -> Bloc {
    trait_en(m("30"), y, m("565"), y, ep, couleur)
}

/// Un cadre numerote : le numero dans un carre noir, le titre en capitales, un filet.
fn cadre(b: &mut Vec<Bloc>, y: Mesure, n: &str, titre: &str) {
    b.push(trait_en(m("30"), y - m("4"), m("48"), y - m("4"), m("16"), NOIR));
    b.push(centre(m("39"), y, n, m("10"), true, BLANC));
    b.push(texte(m("56"), y, titre, m("10"), true, NOIR));
    b.push(filet(y + m("6"), m("0.6"), NOIR));
}

fn etiquette(b: &mut Vec<Bloc>, x: Mesure, y: Mesure, t: &str) {
    b.push(texte(x, y, t, m("7.5"), false, NOIR));
}

/// La mention de la compagnie fictive, en tete et en pied de chaque page.
fn mention(b: &mut Vec<Bloc>, page: usize) {
    b.push(filet(m("48"), m("18"), ROUGE));
    b.push(centre(m("297.6"), m("51"), "COMPAGNIE FICTIVE — DOCUMENT DE DÉMONSTRATION : aucun organisme ne l'a émis, il n'a aucune valeur", m("8"), true, BLANC));
    b.push(filet(m("818"), m("0.5"), ROUGE));
    b.push(texte(m("30"), m("829"), "Compagnie fictive : imitation de l'attestation que France Travail édite d'après la DSN, pour la démonstration du logiciel.", m("6.5"), false, ROUGE));
    b.push(texte_aligne(m("565"), m("829"), &format!("{}/3", page), m("7"), false, NOIR, Alignement::Droite));
}

/// Les champs ajoutes au dessin.
#[derive(Default)]
struct Champs(Vec<ChampAjoute>);

impl Champs {
    /// Imprime (lecture seule) ; une valeur vide n'ecrit pas de champ (le formulaire la laisse a blanc).
    fn imprime(&mut self, page: usize, nom: &str, r: [Mesure; 4], v: &str, taille: Mesure, alignement: Alignement) {
        if !v.trim().is_empty() {
            self.0.push(ChampAjoute {
                nom: nom.to_string(),
                page,
                rect: r,
                valeur: Some(v.to_string()),
                lecture_seule: true,
                taille: Some(taille),
                alignement,
            });
        }
    }
    /// Une case de la fiche, quand elle est dite.
    fn imprime_dite(&mut self, page: usize, nom: &str, r: [Mesure; 4], v: Option<&str>, taille: Mesure, alignement: Alignement) {
        if let Some(v) = v {
            self.imprime(page, nom, r, v, taille, alignement);
        }
    }
    /// A remplir par l'employeur (le cadre 5), a gauche.
    fn vide(&mut self, nom: &str, r: [Mesure; 4]) {
        self.0.push(ChampAjoute { nom: nom.to_string(), page: 3, rect: r, valeur: None, lecture_seule: false, taille: Some(m("9")), alignement: Alignement::Gauche });
    }
}

/// Une quantite telle que la DSN la declare (bloc 53), ecrite en francais : « 151, 67 », « 12 » ; zero (`zero`), a
/// blanc.
fn quantite<G: socle::grandeurs::Grandeur + PartialEq>(g: G, zero: G) -> String {
    if g == zero {
        String::new()
    } else {
        socle::texte::virgule(g)
    }
}

/// Le PDF d'une attestation : le salarie `ind` (et ce que la DSN dit de plus, `plus`), ses periodes (cinq au plus),
/// l'emploi et le motif de rupture de son contrat, l'employeur `ent`, edite le `edite_le`.
fn attestation(ent: &droit::fiche::ChampsEntreprise, d: &DsnLue, ind: &IndividuDeclare, periodes: &[&ColonneDuCadre4], emploi: Option<&str>, motif: droit::natures::MotifDeRupture, idcc: Option<&droit::fiche::Idcc>, edite_le: Jour, remplace: Option<droit::natures::NumeroDOrdre>) -> R<Vec<u8>> {
    if periodes.is_empty() || periodes.len() > COLONNES {
        return Err(socle::refus!("une attestation porte de 1 a {} periodes ({} donnees)", COLONNES, periodes.len()));
    }
    let premier_contrat = d.contrats_de(ind.identite.nir.as_ref()).find(|c| cinema_spectacle(c));
    let (nom, prenoms) = (exigee(&ind.identite.nom, "le nom d'un salarié", droit::rubrique!("S21.G00.30.002"))?, exigee(&ind.identite.prenoms,"le prénom d'un salarié", droit::rubrique!("S21.G00.30.004"))?);
    let mut f = Champs::default();
    let (mut p1, mut p2, mut p3): (Vec<Bloc>, Vec<Bloc>, Vec<Bloc>) = (Vec::new(), Vec::new(), Vec::new());

    // L'en-tete de chaque page : la provenance, le salarie, le logiciel.
    // UNE DSN QUI ANNULE ET REMPLACE (type 03 ou 05, S20.G00.05.002) : France Travail reedite, et l'en-tete le dit, avec
    // l'ordre de la declaration (comme la vraie : « Attestation annule et remplace éditée … N° d'ordre … »).
    let provenance = match remplace {
        Some(ordre) => format!("Attestation annule et remplace éditée pour la démonstration (compagnie fictive) le {}. N° d'ordre {} — imitation : France Travail n'a rien édité", edite_le.jj_mm_aaaa(), ordre),
        None => format!("Attestation éditée pour la démonstration (compagnie fictive) le {} — imitation : France Travail n'a rien édité", edite_le.jj_mm_aaaa()),
    };
    let qui = match &ind.identite.nir {
        Some(nir) => format!("{}, {} {}", nir, nom, prenoms),
        None => format!("{} {}", nom, prenoms),
    };
    f.imprime(1, "ENTETE_PROVENANCE", boite(["7", "5", "588", "16"]), &provenance, m("6.5"), Alignement::Centre);
    f.imprime(1, "ENTETE_SALARIE", boite(["88", "16", "571", "26"]), &qui, m("6.5"), Alignement::Centre);
    // Un nom de champ n'est porte qu'une fois : les pages 2 et 3 redisent l'en-tete en texte.
    for b in [&mut p2, &mut p3] {
        b.push(centre(m("297.6"), m("13"), &provenance, m("6.5"), false, NOIR));
        b.push(centre(m("297.6"), m("23.5"), &qui, m("6.5"), false, NOIR));
    }
    f.imprime(1, "ENTETE_MENTION_LOGICIEL", boite(["88", "26", "571", "37"]), "Logiciel utilisé PUPPETPAIE - reconstituée à partir des déclarations DSN de la compagnie fictive", m("6.5"), Alignement::Centre);
    for (b, n) in [(&mut p1, 1), (&mut p2, 2), (&mut p3, 3)] {
        mention(b, n);
    }

    // Page 1 : le titre, a qui elle est destinee, les cadres 1 a 3.
    p1.push(texte(m("55"), m("92"), "Attestation employeur pour les activités relevant des", m("14"), true, NOIR));
    p1.push(texte(m("55"), m("109"), "annexes 8 et 10 au règlement d'assurance chômage", m("14"), true, NOIR));
    p1.push(texte(m("55"), m("123"), "Issue du processus de déclaration prévue par l'article R.133-14 II 2° du code de la sécurité sociale", m("8"), false, NOIR));
    p1.push(texte(m("55"), m("148"), "À SIGNER PAR L'EMPLOYEUR ET À REMETTRE AU SALARIÉ", m("9"), true, NOIR));
    p1.push(texte(m("55"), m("159"), "Articles L. 5422-14 et R. 5422-6 du code du travail", m("7.5"), false, NOIR));
    for (i, l) in [
        "Cette attestation est destinée au salarié ayant conclu",
        "plusieurs contrats à durée déterminée d'usage d'une durée",
        "inférieure ou égale à un mois pour la réalisation d'activités",
        "relevant des annexes VIII et X au règlement d'assurance",
        "chômage et dont le motif de fin de contrat de travail est une",
        "fin de CDD. En cas de contrat d'une durée supérieure à 1 mois,",
        "une attestation est remise au salarié pour chaque contrat.",
    ]
    .iter()
    .enumerate()
    {
        p1.push(texte(m("55"), m("180") + i * m("10"), l, m("7.5"), false, NOIR));
    }
    for (i, l) in [
        "ATTENTION, cette attestation doit être transmise par le salarié",
        "à France Travail : en la téléchargeant sur son espace personnel si,",
        "au terme de sa demande d'allocations dématérialisée, il lui est",
        "demandé de la transmettre.",
    ]
    .iter()
    .enumerate()
    {
        p1.push(texte(m("320"), m("180") + i * m("10"), l, m("7.5"), i == 0, NOIR));
    }

    cadre(&mut p1, m("315"), "1", "EMPLOYEUR");
    etiquette(&mut p1, m("30"), m("343"), "N° SIRET");
    f.imprime_dite(1, "EMP_SIRET", boite(["75", "334", "253", "346"]), ent.siret.portee(), m("8.5"), Alignement::Gauche);
    etiquette(&mut p1, m("300"), m("343"), "Code APE/NAF");
    f.imprime_dite(1, "EMP_APE", boite(["368", "333", "432", "345"]), ent.naf.portee(), m("8.5"), Alignement::Gauche);
    etiquette(&mut p1, m("470"), m("343"), "N° IDCC");
    f.imprime_dite(1, "EMP_IDCC", boite(["515", "333", "566", "345"]), idcc.map(|i| &**i), m("8.5"), Alignement::Gauche);
    etiquette(&mut p1, m("30"), m("362"), "Nom ou raison sociale");
    f.imprime_dite(1, "ENSEIGNE", boite(["146", "352", "567", "365"]), ent.raison_sociale.portee(), m("8.5"), Alignement::Gauche);
    etiquette(&mut p1, m("30"), m("380"), "Adresse");
    let adresse = ent.adresse.portee().map(|a| droit::fiche::avec_complement(a.to_string(), ent.complement_adresse.remplie()));
    f.imprime_dite(1, "ADR_EMP_CONCATENEE", boite(["74", "370", "567", "383"]), adresse.as_deref(), m("8.5"), Alignement::Gauche);
    etiquette(&mut p1, m("30"), m("397"), "Code postal");
    f.imprime_dite(1, "ADR_EMP_CP", boite(["90", "388", "155", "399"]), ent.code_postal.portee(), m("8.5"), Alignement::Gauche);
    etiquette(&mut p1, m("165"), m("397"), "Commune");
    f.imprime_dite(1, "ADR_EMP_LOCALITE", boite(["224", "387", "567", "399"]), ent.commune.portee(), m("8.5"), Alignement::Gauche);
    etiquette(&mut p1, m("30"), m("425"), "Êtes-vous titulaire d'une licence spectacle ou d'un n° de récépissé de déclaration d'entrepreneur");
    etiquette(&mut p1, m("30"), m("435"), "de spectacles vivants valant licence ?");
    let licence = premier_contrat.and_then(|c| c.licence.as_deref()).filter(|l| !l.is_empty());
    etiquette(&mut p1, m("200"), m("446"), if licence.is_none() { "Non" } else { "Oui, n°" });
    f.imprime_dite(1, "EMP_LICENCE_1", boite(["236", "437", "494", "448"]), licence, m("8.5"), Alignement::Gauche);
    etiquette(&mut p1, m("30"), m("485"), "Numéro d'affiliation à la caisse de congé spectacle :");
    let caisse = premier_contrat.and_then(|c| c.caisse_conges.as_deref()).filter(|c| !c.is_empty());
    etiquette(&mut p1, m("30"), m("503"), if caisse.is_none() { "Non" } else { "Oui, N°" });
    f.imprime_dite(1, "EMP_CAISSE_CP_ART_CODE", boite(["78", "493", "331", "505"]), caisse, m("8.5"), Alignement::Gauche);

    cadre(&mut p1, m("548"), "2", "SALARIÉ");
    etiquette(&mut p1, m("30"), m("590"), "Nom de famille (nom de naissance)");
    f.imprime(1, "SALARIE_NOM_FAMILLE", boite(["209", "580", "563", "594"]), nom, m("9"), Alignement::Gauche);
    etiquette(&mut p1, m("30"), m("605"), "Nom d'usage (nom d'époux (se))");
    etiquette(&mut p1, m("30"), m("624"), "Prénom(s)");
    f.imprime(1, "SALARIE_PRENOMS", boite(["86", "614", "563", "627"]), prenoms, m("9"), Alignement::Gauche);
    etiquette(&mut p1, m("30"), m("639"), "Adresse");
    f.imprime_dite(1, "SALARIE_ADRESSE", boite(["73", "630", "563", "642"]), ind.adresse.as_deref(), m("8.5"), Alignement::Gauche);
    etiquette(&mut p1, m("30"), m("652"), "Code postal");
    f.imprime_dite(1, "SALARIE_CP", boite(["93", "643", "158", "655"]), ind.code_postal.as_deref(), m("8.5"), Alignement::Gauche);
    etiquette(&mut p1, m("170"), m("652"), "Commune");
    f.imprime_dite(1, "SALARIE_ADR_LOCALITE", boite(["237", "643", "563", "655"]), ind.commune.as_deref(), m("8.5"), Alignement::Gauche);
    etiquette(&mut p1, m("30"), m("667"), "NIR (n° de Sécurité sociale)");
    f.imprime_dite(1, "SALARIE_NIR", boite(["165", "658", "331", "670"]), ind.identite.nir.as_deref(), m("8.5"), Alignement::Gauche);
    etiquette(&mut p1, m("30"), m("681"), "Date de naissance");
    // AAAA-MM-JJ (`NaissanceDeclaree::iso`), le jour ou le mois tels que declares s'il est inconnu.
    if let Some(n) = ind.identite.naissance.map(|n| n.iso()) {
        f.imprime(1, "SALARIE_DT_NAISSANCE_J", boite(["123", "672", "149", "683"]), &n[8..], m("8.5"), Alignement::Centre);
        f.imprime(1, "SALARIE_DT_NAISSANCE_M", boite(["155", "672", "181", "683"]), &n[5..7], m("8.5"), Alignement::Centre);
        f.imprime(1, "SALARIE_DT_NAISSANCE_A", boite(["186", "672", "238", "683"]), &n[..4], m("8.5"), Alignement::Centre);
    }
    etiquette(&mut p1, m("280"), m("681"), "Lieu de naissance");
    f.imprime_dite(1, "SALARIE_LIEU_NAISSANCE", boite(["361", "672", "561", "684"]), ind.lieu_naissance.as_deref(), m("8.5"), Alignement::Gauche);

    cadre(&mut p1, m("745"), "3", "CONTRAT DE TRAVAIL");
    etiquette(&mut p1, m("30"), m("773"), "Emploi occupé");
    f.imprime_dite(1, "CON_LIB_EMPLOI", boite(["109", "764", "562", "776"]), emploi, m("8.5"), Alignement::Gauche);
    let motif_lu = if motif == droit::natures::MotifDeRupture::FinDeCdd { "Fin de CDD".to_string() } else { format!("motif {} (S21.G00.62.002)", motif) };
    etiquette(&mut p1, m("30"), m("792"), &format!("Motif de la rupture : {}", motif_lu));

    // Page 2 : le cadre 4, une colonne par periode.
    cadre(&mut p2, m("70"), "4", "PRESTATIONS DE TRAVAIL ET SALAIRES");
    let xs: [(&str, &str); COLONNES] = [("147", "227"), ("230", "310"), ("315", "396"), ("397", "478"), ("482", "563")];
    let lignes_4: [(&str, &str, &str); 11] = [
        ("82", "100", "Numéro de contrat"),
        ("105", "120", "Début du contrat (du)"),
        ("125", "140", "Fin du contrat (au)"),
        ("145", "172", "Numéro d'objet"),
        ("184", "206", "N° IDCC de la prestation (si différent)"),
        ("218", "233", "Début de période (du)"),
        ("236", "252", "Fin de période (au)"),
        ("258", "273", "Heures"),
        ("282", "298", "Cachets*"),
        ("306", "322", "Nombre de jours travaillés"),
        ("339", "360", "Salaire brut (non plafonné)"),
    ];
    for (y0, y1, t) in lignes_4 {
        p2.push(texte_aligne(m("32"), (m(y0) + m(y1)).moitie() + m("2.5"), t, m("7.5"), false, NOIR, Alignement::Gauche));
        p2.push(filet(m(y1) + m("1.5"), m("0.3"), GRIS));
    }
    etiquette(&mut p2, m("32"), m("392"), "Date de paiement");
    p2.push(filet(m("402"), m("0.3"), GRIS));
    p2.push(texte(m("32"), m("414"), "* Uniquement pour les artistes", m("7"), false, GRIS));
    for (i, p) in periodes.iter().enumerate() {
        let n = format!("{:02}", i + 1);
        let (x0, x1) = xs[i];
        let r = |y0: &str, y1: &str| boite([x0, y0, x1, y1]);
        if let Some((numero, deb, fin, objet)) = &p.contrat {
            f.imprime(2, &format!("CON_NUMERO_{}", n), r("82", "100"), numero.as_str(), m("8.5"), Alignement::Centre);
            f.imprime(2, &format!("CON_NUM_DT_DEB_{}", n), r("105", "120"), deb, m("8.5"), Alignement::Centre);
            f.imprime(2, &format!("CON_NUM_DT_FIN_{}", n), r("125", "140"), fin, m("8.5"), Alignement::Centre);
            f.imprime_dite(2, &format!("CON_OBJET_{}", n), r("145", "172"), objet.as_deref(), m("8"), Alignement::Centre);
            let idcc_contrat = d.contrats_de(ind.identite.nir.as_ref()).find(|c| c.numero.as_ref() == Some(numero)).and_then(|c| c.idcc.as_deref()).filter(|i| !i.is_empty());
            if let Some(idcc_contrat) = idcc_contrat.filter(|i| Some(*i) != idcc.map(|j| &**j)) {
                f.imprime(2, &format!("CON_IDCC_{}", n), r("184", "206"), idcc_contrat, m("8.5"), Alignement::Centre);
            }
        }
        f.imprime(2, &format!("SAL_DT_DEB_{}", n), r("218", "233"), &p.periode.debut().jj_mm_aaaa(), m("8.5"), Alignement::Centre);
        f.imprime(2, &format!("SAL_DT_FIN_{}", n), r("236", "252"), &p.periode.fin().jj_mm_aaaa(), m("8.5"), Alignement::Centre);
        f.imprime(2, &format!("SAL_NB_HEURES_{}", n), r("258", "273"), &quantite(p.heures, Heures::ZERO), m("8.5"), Alignement::Centre);
        f.imprime(2, &format!("SAL_NB_CACHET_{}", n), r("282", "298"), &quantite(p.cachets, Cachets::ZERO), m("8.5"), Alignement::Centre);
        f.imprime(2, &format!("SAL_TPS_TRA_{}", n), r("306", "322"), &quantite(p.jours, Jours::ZERO), m("8.5"), Alignement::Centre);
        let montant = p.brut.map(socle::grandeurs::ecrit::norme::montant);
        f.imprime_dite(2, &format!("SAL_MONTANT_{}", n), r("339", "360"), montant.as_deref(), m("8.5"), Alignement::Centre);
        f.imprime_dite(2, &format!("SAL_DT_PAI_{}", n), r("379", "400"), p.paiement.as_deref(), m("8.5"), Alignement::Centre);
    }
    // Un tableau a remplir a la main : son titre, quatre en-tetes, `lignes` lignes numerotees.
    let tableau = |b: &mut Vec<Bloc>, titre: &str, y: Mesure, entetes: [&str; 4], lignes: usize| {
        b.push(texte(m("32"), y, titre, m("9"), true, NOIR));
        for (i, e) in entetes.iter().enumerate() {
            b.push(texte(m("45") + i * m("130"), y + m("16"), e, m("7"), false, GRIS));
        }
        for k in 0..lignes {
            let yy = y + m("36") + k * m("20");
            b.push(texte(m("32"), yy, &format!("{:02}", k + 1), m("7.5"), false, GRIS));
            b.push(filet(yy + m("5"), m("0.3"), GRIS));
        }
    };
    for (titre, y, entetes) in [
        ("Primes et indemnités", m("460"), ["Numéro de contrat", "Intitulé de la prime", "Date de paiement", "Montant brut (non plafonné)"]),
        ("Activité partielle", m("660"), ["Numéro de contrat", "Du (JJ/MM/AA) au (JJ/MM/AA)", "Nombre d'heures", "Indemnité d'activité partielle"]),
    ] {
        tableau(&mut p2, titre, y, entetes, 5);
    }

    // Page 3 : les absences, les autres remunerations, le cadre 5.
    for (titre, y, entetes, n) in [
        ("Périodes d'absence du salarié (congés maladie, maternité, paternité, d'adoption)", m("90"), ["Numéro de contrat", "Motif d'absence", "", "Du (JJ/MM/AA) au (JJ/MM/AA)"], 8),
        ("Si présence d'autres rémunérations :", m("340"), ["Du (JJ/MM/AA) au (JJ/MM/AA)", "Date de paiement", "", "Autres rémunérations brutes"], 6),
    ] {
        tableau(&mut p3, titre, y, entetes, n);
    }
    cadre(&mut p3, m("525"), "5", "AUTHENTIFICATION PAR L'EMPLOYEUR");
    // Une case a remplir par l'employeur : son trait, et son champ vide.
    let mut a_remplir = |b: &mut Vec<Bloc>, nom: &str, r: [&str; 4]| {
        case(b, boite(r));
        f.vide(nom, boite(r));
    };
    etiquette(&mut p3, m("35"), m("554"), "Je soussigné(e), (Nom)");
    a_remplir(&mut p3, "EMPLOYEUR_NOM", ["131", "542", "569", "558"]);
    etiquette(&mut p3, m("35"), m("571"), "(Prénom)");
    a_remplir(&mut p3, "EMPLOYEUR_PRENOM", ["74", "560", "569", "575"]);
    etiquette(&mut p3, m("35"), m("591"), "agissant en qualité de");
    a_remplir(&mut p3, "EMPLOYEUR_QUALITE_LIB", ["130", "581", "551", "594"]);
    etiquette(&mut p3, m("35"), m("610"), "certifie que les renseignements indiqués sur la présente attestation sont exacts et notamment, en cas de");
    etiquette(&mut p3, m("35"), m("620"), "cessation du contrat, que le motif de la rupture est une fin de contrat à durée déterminée.");
    a_remplir(&mut p3, "EMPLOYEUR_LIEU_SIGN", ["38", "630", "290", "646"]);
    etiquette(&mut p3, m("318"), m("642"), "Le");
    for (nom, r) in [
        ("EMPLOYEUR_DATE_SIGN_J", ["333", "632", "359", "645"]),
        ("EMPLOYEUR_DATE_SIGN_M", ["363", "632", "389", "645"]),
        ("EMPLOYEUR_DATE_SIGN_A", ["393", "632", "444", "645"]),
    ] {
        a_remplir(&mut p3, nom, r);
    }
    etiquette(&mut p3, m("35"), m("676"), "Signature de l'employeur");
    a_remplir(&mut p3, "EMPLOYEUR_SIGNATURE", ["31", "683", "152", "764"]);
    etiquette(&mut p3, m("199"), m("664"), "Personne à joindre concernant cette attestation");
    a_remplir(&mut p3, "EMPLOYEUR_CORRES_NOM", ["199", "671", "392", "694"]);
    etiquette(&mut p3, m("160"), m("706"), "Téléphone");
    a_remplir(&mut p3, "EMPLOYEUR_CORRES_TEL", ["206", "697", "386", "709"]);
    etiquette(&mut p3, m("160"), m("722"), "E-mail");
    a_remplir(&mut p3, "EMPLOYEUR_CORRES_EMAIL", ["192", "711", "392", "733"]);
    for (i, l) in [
        "Toute fausse déclaration est passible de deux ans",
        "d'emprisonnement et d'une amende pouvant aller",
        "jusqu'à 30 000 euros (Art. L. 5429-1 du code du travail)",
        "Vous devez conserver cette attestation",
    ]
    .iter()
    .enumerate()
    {
        p3.push(texte(m("199"), m("745") + i * m("9"), l, m("7"), i == 3, NOIR));
    }
    etiquette(&mut p3, m("430"), m("678"), "Cachet de l'entreprise");
    a_remplir(&mut p3, "EMPLOYEUR_CACHET", ["401", "685", "562", "765"]);

    let nulle = Mesure::ZERO;
    let description = Description {
        format: fabrique_pdf::FORMAT,
        page: Some(Page {
            largeur: fabrique_pdf::forme::A4_LARGEUR,
            hauteur: fabrique_pdf::forme::A4_HAUTEUR,
            marges: Marges { haut: nulle, droite: nulle, bas: nulle, gauche: nulle },
        }),
        // Chaque texte dit son corps et sa couleur : la police n'en est que la base.
        police: Some(Police { taille: m("10"),interligne: m("1.25"), couleur: "#000000".to_string() }),
        style: None,
        logo: None,
        titre: Some(format!("Attestation employeur annexes 8 et 10 (compagnie fictive) — {} {}", nom, prenoms)),
        documents: vec![Document::Blocs(p1), Document::Blocs(p2), Document::Blocs(p3)],
        entete_page: None,
        pied_page: None,
    };
    let dessin = documents::fabrique::fabriquer_octets(&description)?;
    let champs = AjoutDeChamps { format: fabrique_pdf::FORMAT, champs: f.0 };
    fabrique_pdf::formulaire::ajouter_champs(&dessin, &champs).map_err(documents::fabrique::echec)
}

/// Le nom d'un PDF dans l'archive, au motif de France Travail (`retours::nom_aer`) : le SIRET, la periode `MMAAAA`, le
/// jour d'edition, le nom et les prenoms du salarie (que `retours::rattacher` rapproche de sa fiche).
fn nom_dans_l_archive(siret: &str, mois: socle::mois::Mois, edite_le: Jour, nom: &str, prenoms: &str, rang: usize, id: usize) -> String {
    let periode: String = format!("{:02}{:04}", mois.numero(), mois.annee());
    let qui: String = format!("{} {}", nom, prenoms).split_whitespace().collect::<Vec<_>>().join("_");
    let siret: String = siret.chars().filter(char::is_ascii_digit).collect();
    format!(
        "AER_{}__{}_{}_{}_{}_{}_{}.pdf",
        if siret.is_empty() { "0".to_string() } else { siret },
        periode,
        CODE,
        edite_le.jjmmaaaa(),
        qui,
        rang,
        id
    )
}

/// L'archive de nature 46 que France Travail publierait pour la DSN `dsn` du mois `mois` : un PDF par salarie dont la
/// DSN declare la fin d'un contrat d'usage (cinq periodes par PDF, les mois anterieurs du contrat lus dans les DSN
/// reelles deposees). `None` : aucune AER Cinema Spectacle n'est due ce mois-ci, France Travail ne publie rien.
pub fn archive(ctx: &Contexte, dsn: &[u8], mois: socle::mois::Mois, edite_le: Jour) -> R<Option<Vec<u8>>> {
    let d = declarations::dsn_lue::lire(dsn, None)?;
    let le_mois = d.mois.ok_or_else(|| refus("la DSN ne dit pas le mois qu'elle déclare (S20.G00.05.005)"))?;
    // Le type et l'ordre de la declaration (S20.G00.05.002 et .004) : 03 et 05 annulent et remplacent.
    let remplace = match d.type_declaration {
        Some(t) if t.remplace() => Some(*exigee(&d.ordre, "l'ordre de la déclaration qu'elle remplace", droit::rubrique!("S20.G00.05.004"))?),
        _ => None,
    };
    let lignes = crate::journal::lu(&ctx.racine).map(|j| j.lignes)?;
    let ent = &ctx.entreprise;
    let siret = ent.siret.ecrite("siret")?;
    let idcc = ent.idcc.dite();
    let mut pdfs: Vec<(String, Vec<u8>)> = Vec::new();
    for ind in &d.individus {
        let mut finis: Vec<&ContratDeclare> = d.contrats_de(ind.identite.nir.as_ref()).filter(|c| cinema_spectacle(c) && c.fin_declaree.is_some()).collect();
        finis.sort_by_key(|c| c.periode);
        let Some(dernier) = finis.last() else { continue };
        let motif = dernier.motif_fin.ok_or_else(|| socle::echec::cle("La DSN ne dit pas le motif de fin d'un contrat.", "S21.G00.62.002"))?;
        let mut toutes: Vec<ColonneDuCadre4> = Vec::new();
        for c in &finis {
            toutes.extend(periodes(ctx, &lignes, &d, le_mois, c)?);
        }
        let emploi = finis.iter().find_map(|c| c.emploi.emploi.as_deref().filter(|e| !e.is_empty()));
        let (nom, prenoms) = (exigee(&ind.identite.nom, "le nom d'un salarié", droit::rubrique!("S21.G00.30.002"))?, exigee(&ind.identite.prenoms,"le prénom d'un salarié", droit::rubrique!("S21.G00.30.004"))?);
        let refs: Vec<&ColonneDuCadre4> = toutes.iter().collect();
        for (rang, lot) in refs.chunks(COLONNES).enumerate() {
            let pdf = attestation(ent, &d, ind, lot, emploi, motif, idcc, edite_le, remplace)?;
            let id = 900_001 + pdfs.len();
            pdfs.push((nom_dans_l_archive(siret, mois, edite_le, nom, prenoms, rang + 1, id), pdf));
        }
    }
    if pdfs.is_empty() {
        return Ok(None);
    }
    let mut z = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (nom, octets) in &pdfs {
        z.start_file(nom.as_str(), options).map_err(|e| refus(e.to_string()))?;
        z.write_all(octets).map_err(|e| refus(e.to_string()))?;
    }
    Ok(Some(z.finish().map_err(|e| refus(e.to_string()))?.into_inner()))
}

