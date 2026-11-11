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
use socle::date::Jour;
use socle::echec::{refus, R};
use socle::num::Decimal;

/// Le « code » du nom d'archive (`AER_<siret>__<MMAAAA>_<code>_<JJMMAAAA>_<NOM>_<rang>_<id>.pdf`) : celui d'une fictive.
const CODE: &str = "FICTIVE";
/// La nature de contrat de la DSN d'un CDD de droit prive (S21.G00.40.007 ; `contrats-base.json`).
const NATURE_CDD: &str = "02";
/// Le code emploi d'un emploi permanent du spectacle (S21.G00.40.005, CCH-18).
const EMPLOI_PERMANENT: &str = "999SPT";
/// Les colonnes du cadre 4 : 01 a 05 (`aer::lire_quantites`).
const COLONNES: usize = 5;

const ROUGE: &str = "#B22222";
const NOIR: &str = "#1F2A33";
const GRIS: &str = "#7A848C";
const BLANC: &str = "#FFFFFF";

// ------------------------------------------------------------------ ce que la DSN dit

/// Une rubrique que la DSN doit porter (`quoi` la dit en mots, `rubrique` va a part).
fn exigee<'a>(v: &'a Option<String>, quoi: &str, rubrique: &str) -> R<&'a str> {
    v.as_deref().ok_or_else(|| socle::echec::cle(format!("La DSN ne dit pas {}.", quoi), rubrique))
}

/// Le contrat releve-t-il de l'attestation Cinema Spectacle ? Ce que France Travail lit de la DSN : un CDD (40.007 = 02)
/// dont le code emploi (40.005) est de la nomenclature des emplois du spectacle, pas « 999SPT » (« Code complément PCS
/// ESE-S21.G00.40.005 doit être dans la table ART et <> « 999SPT » », `atelier/reference/sources/
/// net-entreprises-base-connaissances/net-entreprises-de-la-dsn-a-l-attestation-employeur-v3-2-20230328.txt`, l. 500-510).
fn cinema_spectacle(c: &ContratDeclare) -> bool {
    c.nature_dsn.as_deref() == Some(NATURE_CDD)
        && c.emploi.code_emploi_ft.as_deref().is_some_and(|e| !e.is_empty() && !e.eq_ignore_ascii_case(EMPLOI_PERMANENT))
}

// ------------------------------------------------------------------ les periodes

/// Une colonne du cadre 4 : une periode d'emploi d'un contrat, dans un mois ; `contrat` n'est porte que par la colonne
/// de sa premiere periode (comme sur le formulaire).
struct Periode {
    /// Le numero, le debut, la fin et le numero d'objet (s'il est dit) du contrat.
    contrat: Option<(String, String, String, Option<String>)>,
    debut: Jour,
    fin: Jour,
    heures: Decimal,
    cachets: Decimal,
    jours: Decimal,
    /// Le brut declare ; sans remuneration declaree ce mois-ci, la case reste a blanc.
    brut: Option<Decimal>,
    paiement: Option<String>,
}

/// La DSN reelle deposee pour `m` (le journal), lue ; `None` si le mois n'en a pas. Un fichier depose qui ne se relit
/// pas refuse.
fn dsn_deposee(ctx: &Contexte, lignes: &[crate::journal::Ligne], m: socle::mois::Mois) -> R<Option<DsnLue>> {
    let Some(l) = crate::etats::etat_dsn_du_mois(lignes, m, None, ctx.entreprise.nic_principal()) else { return Ok(None) };
    let Some(fichier) = l.fichier.filter(|f| !f.is_empty()) else { return Ok(None) };
    let p = std::path::Path::new(&fichier);
    let chemin = if p.is_absolute() { p.to_path_buf() } else { ctx.racine.join("sorties").join(m.to_string()).join(&fichier) };
    declarations::dsn_lue::lire_une_dsn(&chemin)
}

/// Le jour du premier versement de l'individu `nir` dans la DSN `d`.
fn paiement_de(d: &DsnLue, nir: &str) -> Option<Jour> {
    d.versements_de(nir).next().and_then(|v| v.verse_le)
}

/// Les periodes d'un contrat de la DSN `d` qui finit ce mois-ci : une par mois du contrat dont la DSN reelle le porte.
fn periodes(ctx: &Contexte, lignes: &[crate::journal::Ligne], d: &DsnLue, le_mois: socle::mois::Mois, c: &ContratDeclare) -> R<Vec<Periode>> {
    let numero = exigee(&c.numero, "le numéro d'un contrat", "S21.G00.40.009")?;
    let (Some(debut), Some(fin)) = (c.debut, c.fin) else {
        return Err(socle::refus!("contrat {} : dates illisibles dans la DSN", numero));
    };
    let mut out = Vec::new();
    // Les mois du contrat avant celui-ci.
    for m in debut.mois_civil().jusqu_a(le_mois.precedent()) {
        let Some(lue) = dsn_deposee(ctx, lignes, m)? else { continue };
        if lue.contrats_de(&c.nir).any(|x| x.numero == c.numero) {
            let mesures = lue.mesures(&c.nir, numero, false);
            out.push(periode(c, numero, &mesures, debut, fin, m, paiement_de(&lue, &c.nir), out.is_empty()));
        }
    }
    // Un contrat que ce fichier ne porte que pour le rappel d'un mois deja declare (`dsn_lue::Remuneration::rappel`) n'a
    // pas de periode ce mois-ci.
    let mesures = d.mesures(&c.nir, numero, false);
    if mesures.remunere || out.is_empty() {
        out.push(periode(c, numero, &mesures, debut, fin, le_mois, paiement_de(d, &c.nir), out.is_empty()));
    }
    Ok(out)
}

fn periode(c: &ContratDeclare, numero: &str, x: &Mesures, debut: Jour, fin: Jour, m: socle::mois::Mois, paiement: Option<Jour>, premiere: bool) -> Periode {
    let (d, f) = (m.premier_jour(), m.dernier_jour());
    Periode {
        contrat: premiere.then(|| (numero.to_string(), debut.jj_mm_aaaa(), fin.jj_mm_aaaa(), c.objet.clone())),
        debut: if debut > d { debut } else { d },
        fin: if fin < f { fin } else { f },
        heures: x.heures,
        cachets: x.cachets,
        jours: x.jours,
        brut: x.brut,
        paiement: paiement.map(|j| j.jj_mm_aaaa()),
    }
}

// ------------------------------------------------------------------ le dessin

// Le gabarit se releve en points entiers (`i64`) ; un corps, un trait, une position au demi-point pres sont des
// `Mesure`.

/// `n` points.
fn pt(n: i64) -> Mesure {
    Mesure::entier(n)
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

fn texte(x: i64, y: i64, t: &str, taille: Mesure, gras: bool, couleur: &str) -> Bloc {
    texte_aligne(pt(x), pt(y), t, taille, gras, couleur, Alignement::Gauche)
}

fn centre(x: Mesure, y: Mesure, t: &str, taille: Mesure, gras: bool, couleur: &str) -> Bloc {
    texte_aligne(x, y, t, taille, gras, couleur, Alignement::Centre)
}

fn trait_en(x1: Mesure, y1: Mesure, x2: Mesure, y2: Mesure, ep: Mesure, couleur: &str) -> Bloc {
    Bloc::Trait(Trait { x1, y1, x2, y2, epaisseur: ep, couleur: couleur.to_string() })
}

fn trait_(x1: i64, y1: i64, x2: i64, y2: i64, ep: Mesure, couleur: &str) -> Bloc {
    trait_en(pt(x1), pt(y1), pt(x2), pt(y2), ep, couleur)
}

fn case(b: &mut Vec<Bloc>, r: [i64; 4]) {
    let [x0, y0, x1, y1] = r;
    for (a, bb, c, d) in [(x0, y0, x1, y0), (x1, y0, x1, y1), (x1, y1, x0, y1), (x0, y1, x0, y0)] {
        b.push(trait_(a, bb, c, d, m("0.4"), GRIS));
    }
}

/// Un cadre numerote : le numero dans un carre noir, le titre en capitales, un filet.
fn cadre(b: &mut Vec<Bloc>, y: i64, n: &str, titre: &str) {
    b.push(trait_(30, y - 4, 48, y - 4, pt(16), NOIR));
    b.push(centre(pt(39), pt(y), n, pt(10), true, BLANC));
    b.push(texte(56, y, titre, pt(10), true, NOIR));
    b.push(trait_(30, y + 6, 565, y + 6, m("0.6"), NOIR));
}

fn etiquette(b: &mut Vec<Bloc>, x: i64, y: i64, t: &str) {
    b.push(texte(x, y, t, m("7.5"), false, NOIR));
}

/// La mention de la compagnie fictive, en tete et en pied de chaque page.
fn mention(b: &mut Vec<Bloc>, page: usize) {
    b.push(trait_(30, 48, 565, 48, pt(18), ROUGE));
    b.push(centre(m("297.6"), pt(51), "COMPAGNIE FICTIVE — DOCUMENT DE DÉMONSTRATION : aucun organisme ne l'a émis, il n'a aucune valeur", pt(8), true, BLANC));
    b.push(trait_(30, 818, 565, 818, m("0.5"), ROUGE));
    b.push(texte(30, 829, "Compagnie fictive : imitation de l'attestation que France Travail édite d'après la DSN, pour la démonstration du logiciel.", m("6.5"), false, ROUGE));
    b.push(texte_aligne(pt(565), pt(829), &format!("{}/3", page), pt(7), false, NOIR, Alignement::Droite));
}

/// Les champs ajoutes au dessin.
#[derive(Default)]
struct Champs(Vec<ChampAjoute>);

impl Champs {
    /// Imprime (lecture seule) ; une valeur vide n'ecrit pas de champ (le formulaire la laisse a blanc).
    fn imprime(&mut self, page: usize, nom: &str, r: [i64; 4], v: &str, taille: Mesure, alignement: Alignement) {
        if !v.trim().is_empty() {
            self.0.push(ChampAjoute {
                nom: nom.to_string(),
                page,
                rect: r.map(pt),
                valeur: Some(v.to_string()),
                lecture_seule: true,
                taille: Some(taille),
                alignement,
            });
        }
    }
    /// Une case de la fiche, quand elle est dite.
    fn imprime_dite(&mut self, page: usize, nom: &str, r: [i64; 4], v: Option<&str>, taille: Mesure, alignement: Alignement) {
        if let Some(v) = v {
            self.imprime(page, nom, r, v, taille, alignement);
        }
    }
    /// A remplir par l'employeur (le cadre 5), a gauche.
    fn vide(&mut self, nom: &str, r: [i64; 4]) {
        self.0.push(ChampAjoute { nom: nom.to_string(), page: 3, rect: r.map(pt), valeur: None, lecture_seule: false, taille: Some(Mesure::entier(9)), alignement: Alignement::Gauche });
    }
}

/// Une quantite telle que la DSN la declare (bloc 53), ecrite en francais : « 151, 67 », « 12 » ; zero, a blanc.
fn quantite(d: Decimal) -> String {
    if d.is_zero() {
        String::new()
    } else {
        d.normalize().to_string().replace('.', ",")
    }
}

/// Le PDF d'une attestation : le salarie `ind` (et ce que la DSN dit de plus, `plus`), ses periodes (cinq au plus),
/// l'emploi et le motif de rupture de son contrat, l'employeur `ent`, edite le `edite_le`.
fn attestation(ent: &droit::fiche::ChampsEntreprise, d: &DsnLue, ind: &IndividuDeclare, periodes: &[&Periode], emploi: Option<&str>, motif: droit::natures::MotifDeRupture, idcc: Option<&str>, edite_le: Jour, remplace: Option<&str>) -> R<Vec<u8>> {
    if periodes.is_empty() || periodes.len() > COLONNES {
        return Err(socle::refus!("une attestation porte de 1 a {} periodes ({} donnees)", COLONNES, periodes.len()));
    }
    let premier_contrat = d.contrats_de(&ind.nir).find(|c| cinema_spectacle(c));
    let (nom, prenoms) = (exigee(&ind.nom, "le nom d'un salarié", "S21.G00.30.002")?, exigee(&ind.prenom, "le prénom d'un salarié", "S21.G00.30.004")?);
    let mut f = Champs::default();
    let (mut p1, mut p2, mut p3): (Vec<Bloc>, Vec<Bloc>, Vec<Bloc>) = (Vec::new(), Vec::new(), Vec::new());

    // L'en-tete de chaque page : la provenance, le salarie, le logiciel.
    // UNE DSN QUI ANNULE ET REMPLACE (type 03 ou 05, S20.G00.05.002) : France Travail reedite, et l'en-tete le dit, avec
    // l'ordre de la declaration (comme la vraie : « Attestation annule et remplace éditée … N° d'ordre … »).
    let provenance = match remplace {
        Some(ordre) => format!("Attestation annule et remplace éditée pour la démonstration (compagnie fictive) le {}. N° d'ordre {} — imitation : France Travail n'a rien édité", edite_le.jj_mm_aaaa(), ordre),
        None => format!("Attestation éditée pour la démonstration (compagnie fictive) le {} — imitation : France Travail n'a rien édité", edite_le.jj_mm_aaaa()),
    };
    let qui = format!("{}, {} {}", ind.nir, nom, prenoms);
    f.imprime(1, "ENTETE_PROVENANCE", [7, 5, 588, 16], &provenance, m("6.5"), Alignement::Centre);
    f.imprime(1, "ENTETE_SALARIE", [88, 16, 571, 26], &qui, m("6.5"), Alignement::Centre);
    // Un nom de champ n'est porte qu'une fois : les pages 2 et 3 redisent l'en-tete en texte.
    for b in [&mut p2, &mut p3] {
        b.push(centre(m("297.6"), pt(13), &provenance, m("6.5"), false, NOIR));
        b.push(centre(m("297.6"), m("23.5"), &qui, m("6.5"), false, NOIR));
    }
    f.imprime(1, "ENTETE_MENTION_LOGICIEL", [88, 26, 571, 37], "Logiciel utilisé PUPPETPAIE - reconstituée à partir des déclarations DSN de la compagnie fictive", m("6.5"), Alignement::Centre);
    for (b, n) in [(&mut p1, 1), (&mut p2, 2), (&mut p3, 3)] {
        mention(b, n);
    }

    // Page 1 : le titre, a qui elle est destinee, les cadres 1 a 3.
    p1.push(texte(55, 92, "Attestation employeur pour les activités relevant des", pt(14), true, NOIR));
    p1.push(texte(55, 109, "annexes 8 et 10 au règlement d'assurance chômage", pt(14), true, NOIR));
    p1.push(texte(55, 123, "Issue du processus de déclaration prévue par l'article R.133-14 II 2° du code de la sécurité sociale", pt(8), false, NOIR));
    p1.push(texte(55, 148, "À SIGNER PAR L'EMPLOYEUR ET À REMETTRE AU SALARIÉ", pt(9), true, NOIR));
    p1.push(texte(55, 159, "Articles L. 5422-14 et R. 5422-6 du code du travail", m("7.5"), false, NOIR));
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
        p1.push(texte(55, 180 + 10 * i as i64, l, m("7.5"), false, NOIR));
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
        p1.push(texte(320, 180 + 10 * i as i64, l, m("7.5"), i == 0, NOIR));
    }

    cadre(&mut p1, 315, "1", "EMPLOYEUR");
    etiquette(&mut p1, 30, 343, "N° SIRET");
    f.imprime_dite(1, "EMP_SIRET", [75, 334, 253, 346], ent.siret.portee(), m("8.5"), Alignement::Gauche);
    etiquette(&mut p1, 300, 343, "Code APE/NAF");
    f.imprime_dite(1, "EMP_APE", [368, 333, 432, 345], ent.naf.portee(), m("8.5"), Alignement::Gauche);
    etiquette(&mut p1, 470, 343, "N° IDCC");
    f.imprime_dite(1, "EMP_IDCC", [515, 333, 566, 345], idcc, m("8.5"), Alignement::Gauche);
    etiquette(&mut p1, 30, 362, "Nom ou raison sociale");
    f.imprime_dite(1, "ENSEIGNE", [146, 352, 567, 365], ent.raison_sociale.portee(), m("8.5"), Alignement::Gauche);
    etiquette(&mut p1, 30, 380, "Adresse");
    let adresse = ent.adresse.portee().map(|a| droit::fiche::avec_complement(a.to_string(), ent.complement_adresse.remplie()));
    f.imprime_dite(1, "ADR_EMP_CONCATENEE", [74, 370, 567, 383], adresse.as_deref(), m("8.5"), Alignement::Gauche);
    etiquette(&mut p1, 30, 397, "Code postal");
    f.imprime_dite(1, "ADR_EMP_CP", [90, 388, 155, 399], ent.code_postal.portee(), m("8.5"), Alignement::Gauche);
    etiquette(&mut p1, 165, 397, "Commune");
    f.imprime_dite(1, "ADR_EMP_LOCALITE", [224, 387, 567, 399], ent.commune.portee(), m("8.5"), Alignement::Gauche);
    etiquette(&mut p1, 30, 425, "Êtes-vous titulaire d'une licence spectacle ou d'un n° de récépissé de déclaration d'entrepreneur");
    etiquette(&mut p1, 30, 435, "de spectacles vivants valant licence ?");
    let licence = premier_contrat.and_then(|c| c.licence.as_deref()).filter(|l| !l.is_empty());
    etiquette(&mut p1, 200, 446, if licence.is_none() { "Non" } else { "Oui, n°" });
    f.imprime_dite(1, "EMP_LICENCE_1", [236, 437, 494, 448], licence, m("8.5"), Alignement::Gauche);
    etiquette(&mut p1, 30, 485, "Numéro d'affiliation à la caisse de congé spectacle :");
    let caisse = premier_contrat.and_then(|c| c.caisse_conges.as_deref()).filter(|c| !c.is_empty());
    etiquette(&mut p1, 30, 503, if caisse.is_none() { "Non" } else { "Oui, N°" });
    f.imprime_dite(1, "EMP_CAISSE_CP_ART_CODE", [78, 493, 331, 505], caisse, m("8.5"), Alignement::Gauche);

    cadre(&mut p1, 548, "2", "SALARIÉ");
    etiquette(&mut p1, 30, 590, "Nom de famille (nom de naissance)");
    f.imprime(1, "SALARIE_NOM_FAMILLE", [209, 580, 563, 594], nom, pt(9), Alignement::Gauche);
    etiquette(&mut p1, 30, 605, "Nom d'usage (nom d'époux (se))");
    etiquette(&mut p1, 30, 624, "Prénom(s)");
    f.imprime(1, "SALARIE_PRENOMS", [86, 614, 563, 627], prenoms, pt(9), Alignement::Gauche);
    etiquette(&mut p1, 30, 639, "Adresse");
    f.imprime_dite(1, "SALARIE_ADRESSE", [73, 630, 563, 642], ind.adresse.as_deref(), m("8.5"), Alignement::Gauche);
    etiquette(&mut p1, 30, 652, "Code postal");
    f.imprime_dite(1, "SALARIE_CP", [93, 643, 158, 655], ind.code_postal.as_deref(), m("8.5"), Alignement::Gauche);
    etiquette(&mut p1, 170, 652, "Commune");
    f.imprime_dite(1, "SALARIE_ADR_LOCALITE", [237, 643, 563, 655], ind.commune.as_deref(), m("8.5"), Alignement::Gauche);
    etiquette(&mut p1, 30, 667, "NIR (n° de Sécurité sociale)");
    f.imprime(1, "SALARIE_NIR", [165, 658, 331, 670], &ind.nir, m("8.5"), Alignement::Gauche);
    etiquette(&mut p1, 30, 681, "Date de naissance");
    // AAAA-MM-JJ (`NaissanceDeclaree::iso`), le jour ou le mois tels que declares s'il est inconnu.
    if let Some(n) = ind.date_naissance.map(|n| n.iso()) {
        f.imprime(1, "SALARIE_DT_NAISSANCE_J", [123, 672, 149, 683], &n[8..], m("8.5"), Alignement::Centre);
        f.imprime(1, "SALARIE_DT_NAISSANCE_M", [155, 672, 181, 683], &n[5..7], m("8.5"), Alignement::Centre);
        f.imprime(1, "SALARIE_DT_NAISSANCE_A", [186, 672, 238, 683], &n[..4], m("8.5"), Alignement::Centre);
    }
    etiquette(&mut p1, 280, 681, "Lieu de naissance");
    f.imprime_dite(1, "SALARIE_LIEU_NAISSANCE", [361, 672, 561, 684], ind.lieu_naissance.as_deref(), m("8.5"), Alignement::Gauche);

    cadre(&mut p1, 745, "3", "CONTRAT DE TRAVAIL");
    etiquette(&mut p1, 30, 773, "Emploi occupé");
    f.imprime_dite(1, "CON_LIB_EMPLOI", [109, 764, 562, 776], emploi, m("8.5"), Alignement::Gauche);
    let motif_lu = if motif == droit::natures::MotifDeRupture::FinDeCdd { "Fin de CDD".to_string() } else { format!("motif {} (S21.G00.62.002)", motif) };
    etiquette(&mut p1, 30, 792, &format!("Motif de la rupture : {}", motif_lu));

    // Page 2 : le cadre 4, une colonne par periode.
    cadre(&mut p2, 70, "4", "PRESTATIONS DE TRAVAIL ET SALAIRES");
    let xs: [(i64, i64); COLONNES] = [(147, 227), (230, 310), (315, 396), (397, 478), (482, 563)];
    let lignes_4: [(i64, i64, &str); 11] = [
        (82, 100, "Numéro de contrat"),
        (105, 120, "Début du contrat (du)"),
        (125, 140, "Fin du contrat (au)"),
        (145, 172, "Numéro d'objet"),
        (184, 206, "N° IDCC de la prestation (si différent)"),
        (218, 233, "Début de période (du)"),
        (236, 252, "Fin de période (au)"),
        (258, 273, "Heures"),
        (282, 298, "Cachets*"),
        (306, 322, "Nombre de jours travaillés"),
        (339, 360, "Salaire brut (non plafonné)"),
    ];
    for (y0, y1, t) in lignes_4 {
        p2.push(texte_aligne(pt(32), (pt(y0) + pt(y1)) / 2 + m("2.5"), t, m("7.5"), false, NOIR, Alignement::Gauche));
        p2.push(trait_en(pt(30), pt(y1) + m("1.5"), pt(565), pt(y1) + m("1.5"), m("0.3"), GRIS));
    }
    etiquette(&mut p2, 32, 392, "Date de paiement");
    p2.push(trait_(30, 402, 565, 402, m("0.3"), GRIS));
    p2.push(texte(32, 414, "* Uniquement pour les artistes", pt(7), false, GRIS));
    for (i, p) in periodes.iter().enumerate() {
        let n = format!("{:02}", i + 1);
        let (x0, x1) = xs[i];
        let r = |y0: i64, y1: i64| [x0, y0, x1, y1];
        if let Some((numero, deb, fin, objet)) = &p.contrat {
            f.imprime(2, &format!("CON_NUMERO_{}", n), r(82, 100), numero, m("8.5"), Alignement::Centre);
            f.imprime(2, &format!("CON_NUM_DT_DEB_{}", n), r(105, 120), deb, m("8.5"), Alignement::Centre);
            f.imprime(2, &format!("CON_NUM_DT_FIN_{}", n), r(125, 140), fin, m("8.5"), Alignement::Centre);
            f.imprime_dite(2, &format!("CON_OBJET_{}", n), r(145, 172), objet.as_deref(), pt(8), Alignement::Centre);
            let idcc_contrat = d.contrats_de(&ind.nir).find(|c| c.numero.as_deref() == Some(numero.as_str())).and_then(|c| c.idcc.as_deref()).filter(|i| !i.is_empty());
            if let Some(idcc_contrat) = idcc_contrat.filter(|i| Some(*i) != idcc) {
                f.imprime(2, &format!("CON_IDCC_{}", n), r(184, 206), idcc_contrat, m("8.5"), Alignement::Centre);
            }
        }
        f.imprime(2, &format!("SAL_DT_DEB_{}", n), r(218, 233), &p.debut.jj_mm_aaaa(), m("8.5"), Alignement::Centre);
        f.imprime(2, &format!("SAL_DT_FIN_{}", n), r(236, 252), &p.fin.jj_mm_aaaa(), m("8.5"), Alignement::Centre);
        f.imprime(2, &format!("SAL_NB_HEURES_{}", n), r(258, 273), &quantite(p.heures), m("8.5"), Alignement::Centre);
        f.imprime(2, &format!("SAL_NB_CACHET_{}", n), r(282, 298), &quantite(p.cachets), m("8.5"), Alignement::Centre);
        f.imprime(2, &format!("SAL_TPS_TRA_{}", n), r(306, 322), &quantite(p.jours), m("8.5"), Alignement::Centre);
        let montant = p.brut.map(|b| format!("{:.2}", b.round_dp(2)));
        f.imprime_dite(2, &format!("SAL_MONTANT_{}", n), r(339, 360), montant.as_deref(), m("8.5"), Alignement::Centre);
        f.imprime_dite(2, &format!("SAL_DT_PAI_{}", n), r(379, 400), p.paiement.as_deref(), m("8.5"), Alignement::Centre);
    }
    for (titre, y, entetes) in [
        ("Primes et indemnités", 460, ["Numéro de contrat", "Intitulé de la prime", "Date de paiement", "Montant brut (non plafonné)"]),
        ("Activité partielle", 660, ["Numéro de contrat", "Du (JJ/MM/AA) au (JJ/MM/AA)", "Nombre d'heures", "Indemnité d'activité partielle"]),
    ] {
        p2.push(texte(32, y, titre, pt(9), true, NOIR));
        for (i, e) in entetes.iter().enumerate() {
            p2.push(texte(45 + 130 * i as i64, y + 16, e, pt(7), false, GRIS));
        }
        for k in 0..5 {
            let yy = y + 36 + 20 * k as i64;
            p2.push(texte(32, yy, &format!("{:02}", k + 1), m("7.5"), false, GRIS));
            p2.push(trait_(30, yy + 5, 565, yy + 5, m("0.3"), GRIS));
        }
    }

    // Page 3 : les absences, les autres remunerations, le cadre 5.
    for (titre, y, entetes, n) in [
        ("Périodes d'absence du salarié (congés maladie, maternité, paternité, d'adoption)", 90, ["Numéro de contrat", "Motif d'absence", "", "Du (JJ/MM/AA) au (JJ/MM/AA)"], 8),
        ("Si présence d'autres rémunérations :", 340, ["Du (JJ/MM/AA) au (JJ/MM/AA)", "Date de paiement", "", "Autres rémunérations brutes"], 6),
    ] {
        p3.push(texte(32, y, titre, pt(9), true, NOIR));
        for (i, e) in entetes.iter().enumerate() {
            p3.push(texte(45 + 130 * i as i64, y + 16, e, pt(7), false, GRIS));
        }
        for k in 0..n {
            let yy = y + 36 + 20 * k as i64;
            p3.push(texte(32, yy, &format!("{:02}", k + 1), m("7.5"), false, GRIS));
            p3.push(trait_(30, yy + 5, 565, yy + 5, m("0.3"), GRIS));
        }
    }
    cadre(&mut p3, 525, "5", "AUTHENTIFICATION PAR L'EMPLOYEUR");
    etiquette(&mut p3, 35, 554, "Je soussigné(e), (Nom)");
    case(&mut p3, [131, 542, 569, 558]);
    f.vide("EMPLOYEUR_NOM", [131, 542, 569, 558]);
    etiquette(&mut p3, 35, 571, "(Prénom)");
    case(&mut p3, [74, 560, 569, 575]);
    f.vide("EMPLOYEUR_PRENOM", [74, 560, 569, 575]);
    etiquette(&mut p3, 35, 591, "agissant en qualité de");
    case(&mut p3, [130, 581, 551, 594]);
    f.vide("EMPLOYEUR_QUALITE_LIB", [130, 581, 551, 594]);
    etiquette(&mut p3, 35, 610, "certifie que les renseignements indiqués sur la présente attestation sont exacts et notamment, en cas de");
    etiquette(&mut p3, 35, 620, "cessation du contrat, que le motif de la rupture est une fin de contrat à durée déterminée.");
    case(&mut p3, [38, 630, 290, 646]);
    f.vide("EMPLOYEUR_LIEU_SIGN", [38, 630, 290, 646]);
    etiquette(&mut p3, 318, 642, "Le");
    for (nom, r) in [
        ("EMPLOYEUR_DATE_SIGN_J", [333, 632, 359, 645]),
        ("EMPLOYEUR_DATE_SIGN_M", [363, 632, 389, 645]),
        ("EMPLOYEUR_DATE_SIGN_A", [393, 632, 444, 645]),
    ] {
        case(&mut p3, r);
        f.vide(nom, r);
    }
    etiquette(&mut p3, 35, 676, "Signature de l'employeur");
    case(&mut p3, [31, 683, 152, 764]);
    f.vide("EMPLOYEUR_SIGNATURE", [31, 683, 152, 764]);
    etiquette(&mut p3, 199, 664, "Personne à joindre concernant cette attestation");
    case(&mut p3, [199, 671, 392, 694]);
    f.vide("EMPLOYEUR_CORRES_NOM", [199, 671, 392, 694]);
    etiquette(&mut p3, 160, 706, "Téléphone");
    case(&mut p3, [206, 697, 386, 709]);
    f.vide("EMPLOYEUR_CORRES_TEL", [206, 697, 386, 709]);
    etiquette(&mut p3, 160, 722, "E-mail");
    case(&mut p3, [192, 711, 392, 733]);
    f.vide("EMPLOYEUR_CORRES_EMAIL", [192, 711, 392, 733]);
    for (i, l) in [
        "Toute fausse déclaration est passible de deux ans",
        "d'emprisonnement et d'une amende pouvant aller",
        "jusqu'à 30 000 euros (Art. L. 5429-1 du code du travail)",
        "Vous devez conserver cette attestation",
    ]
    .iter()
    .enumerate()
    {
        p3.push(texte(199, 745 + 9 * i as i64, l, pt(7), i == 3, NOIR));
    }
    etiquette(&mut p3, 430, 678, "Cachet de l'entreprise");
    case(&mut p3, [401, 685, 562, 765]);
    f.vide("EMPLOYEUR_CACHET", [401, 685, 562, 765]);

    let nulle = Mesure::ZERO;
    let description = Description {
        format: fabrique_pdf::FORMAT,
        page: Some(Page {
            largeur: fabrique_pdf::forme::A4_LARGEUR,
            hauteur: fabrique_pdf::forme::A4_HAUTEUR,
            marges: Marges { haut: nulle, droite: nulle, bas: nulle, gauche: nulle },
        }),
        // Chaque texte dit son corps et sa couleur : la police n'en est que la base.
        police: Some(Police { taille: pt(10), interligne: m("1.25"), couleur: "#000000".to_string() }),
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
        "AER_{}__{}_{}_{:02}{:02}{:04}_{}_{}_{}.pdf",
        if siret.is_empty() { "0".to_string() } else { siret },
        periode,
        CODE,
        edite_le.jour,
        edite_le.mois,
        edite_le.an,
        qui,
        rang,
        id
    )
}

/// L'archive de nature 46 que France Travail publierait pour la DSN `dsn` du mois `mois` : un PDF par salarie dont la
/// DSN declare la fin d'un contrat d'usage (cinq periodes par PDF, les mois anterieurs du contrat lus dans les DSN
/// reelles deposees). `None` : aucune AER Cinema Spectacle n'est due ce mois-ci, France Travail ne publie rien.
pub fn archive(ctx: &Contexte, dsn: &[u8], mois: socle::mois::Mois, edite_le: Jour) -> R<Option<Vec<u8>>> {
    let d = declarations::dsn_lue::lire(dsn, "")?;
    let le_mois = d.mois.ok_or_else(|| refus("la DSN ne dit pas le mois qu'elle déclare (S20.G00.05.005)"))?;
    // Le type et l'ordre de la declaration (S20.G00.05.002 et .004) : 03 et 05 annulent et remplacent.
    let remplace = match d.type_declaration {
        Some(t) if t.remplace() => Some(exigee(&d.ordre, "l'ordre de la déclaration qu'elle remplace", "S20.G00.05.004")?),
        _ => None,
    };
    let lignes = crate::journal::lu(&ctx.racine).map(|j| j.lignes)?;
    let ent = &ctx.entreprise;
    let siret = ent.siret.ecrite("siret")?;
    let idcc = ent.idcc.portee();
    let mut pdfs: Vec<(String, Vec<u8>)> = Vec::new();
    for ind in &d.individus {
        let mut finis: Vec<&ContratDeclare> = d.contrats_de(&ind.nir).filter(|c| cinema_spectacle(c) && c.fin_declaree.is_some()).collect();
        finis.sort_by_key(|c| (c.debut, c.fin));
        let Some(dernier) = finis.last() else { continue };
        let motif = dernier.motif_fin.ok_or_else(|| socle::echec::cle("La DSN ne dit pas le motif de fin d'un contrat.", "S21.G00.62.002"))?;
        let mut toutes: Vec<Periode> = Vec::new();
        for c in &finis {
            toutes.extend(periodes(ctx, &lignes, &d, le_mois, c)?);
        }
        let emploi = finis.iter().find_map(|c| c.emploi.emploi.as_deref().filter(|e| !e.is_empty()));
        let (nom, prenoms) = (exigee(&ind.nom, "le nom d'un salarié", "S21.G00.30.002")?, exigee(&ind.prenom, "le prénom d'un salarié", "S21.G00.30.004")?);
        let refs: Vec<&Periode> = toutes.iter().collect();
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

