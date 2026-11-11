//! LA FABRIQUE DE BULLETINS FICTIFS d'un logiciel precedent, a la maniere de deux logiciels de paie : CoolPaie (A)
//! et sPAIEctacle (B), leurs positions relevees sur leurs bulletins. Une page se decrit (les
//! structures `A` et `B`), la fabrique la dessine (`texte_fixe`, positions du haut-gauche de la page), et le lecteur
//! (`bulletin_lu`) la relit comme un vrai bulletin.
//!
//! UN SEUL CODE POUR DEUX USAGES : les tests du moteur (`bulletin_lu`, `routes::import`) l'incluent tel quel
//! (`moteur/tests/bulletins_fictifs/mod.rs`, par `#[path]`), et l'outil d'atelier `moteur-atelier fabriquer-bulletins`
//! (`fabriquer_bulletins.rs`, a cote) le compile. Ce fichier ne depend donc que de `fabrique_pdf` et `std` (rien de
//! `crate::`), et le binaire livre ne le contient pas.
//!
//! Les textes sont generiques (`S: AsRef<str>`, `&'static str` par defaut : les exemples des tests s'ecrivent en
//! litteraux, l'outil en `String`). Rien de reel : noms, adresses, NIR, Siret, numeros d'objet, titres et montants
//! sont ceux qu'on lui donne, inventes (regle 9 a).

use fabrique_pdf::forme::{Alignement, Bloc, Description, Document, Marges, Mesure, Page, Police, PoliceFixe, TexteFixe};
use fabrique_pdf::mesure::mesure;
use std::path::Path;

// ------------------------------------------------------------------------------------------- la fabrique

/// Une position ou un corps releve sur les bulletins, en points : `m("135.9")`.
pub const fn m(t: &str) -> Mesure {
    mesure(t)
}

/// Un texte pose, maigre, en Helvetica, de la couleur de la page.
fn texte_fixe(x: Mesure, y: Mesure, texte: &str, taille: Mesure, alignement: Alignement) -> Bloc {
    Bloc::TexteFixe(TexteFixe {
        texte: texte.to_string(),
        x,
        y,
        taille: Some(taille),
        police: PoliceFixe::Helvetica,
        gras: false,
        couleur: None,
        alignement,
        largeur_max: None,
    })
}

pub fn t(b: &mut Vec<Bloc>, x: Mesure, y: Mesure, texte: &str) {
    b.push(texte_fixe(x, y, texte, Mesure::entier(9), Alignement::Gauche));
}

/// Un nombre, aligne a droite sur `x`.
pub fn r(b: &mut Vec<Bloc>, x: Mesure, y: Mesure, texte: &str) {
    b.push(texte_fixe(x, y, texte, Mesure::entier(9), Alignement::Droite));
}

pub fn grand(b: &mut Vec<Bloc>, x: Mesure, y: Mesure, texte: &str, taille: Mesure) {
    b.push(texte_fixe(x, y, texte, taille, Alignement::Gauche));
}

/// Les octets d'un PDF d'une page par liste de blocs, A4.
pub fn pdf_octets(pages: Vec<Vec<Bloc>>) -> socle::echec::R<Vec<u8>> {
    let nulle = Mesure::ZERO;
    let d = Description {
        format: 3,
        page: Some(Page {
            largeur: mesure("595.22"),
            hauteur: Mesure::entier(842),
            marges: Marges { haut: nulle, droite: nulle, bas: nulle, gauche: nulle },
        }),
        police: Some(Police { taille: Mesure::entier(9), interligne: mesure("1.25"), couleur: "#000000".to_string() }),
        style: None,
        logo: None,
        titre: None,
        documents: pages.into_iter().map(Document::Blocs).collect(),
        entete_page: None,
        pied_page: None,
    };
    fabrique_pdf::fabriquer(&d).map_err(|r| socle::refus!("la fabrique refuse : {}", r.phrase))
}

/// Un PDF d'une page par liste de blocs, A4 (les tests : un refus arrete le test).
pub fn pdf(pages: Vec<Vec<Bloc>>, chemin: &Path) {
    let octets = match pdf_octets(pages) {
        Ok(o) => o,
        Err(m) => panic!("{}", m.phrase),
    };
    std::fs::write(chemin, octets).unwrap();
}

fn s<S: AsRef<str>>(x: &S) -> &str {
    x.as_ref()
}

// ------------------------------------------------------------------------------------------- l'identite

/// Ce qu'un bulletin imprime de l'entreprise et du salarie.
#[derive(Clone, Debug)]
pub struct Identite<S = &'static str> {
    /// La raison sociale, la rue, « code postal commune ».
    pub entreprise: [S; 3],
    /// Quatorze chiffres, sans espace (sPAIEctacle les espace lui-meme).
    pub siret: S,
    pub naf: S,
    pub urssaf: S,
    /// La convention, sur deux rangs, telle qu'elle s'imprime.
    pub convention: [S; 2],
    pub licence: S,
    /// Le nom (CoolPaie l'ecrit en capitales avant le prenom ; sPAIEctacle, en capitales apres).
    pub nom: S,
    pub prenom: S,
    /// La rue, « code postal commune ».
    pub adresse: [S; 2],
    /// JJ/MM/AAAA.
    pub naissance: S,
    /// Quinze caracteres, sans espace.
    pub nir: S,
    /// Le code (ou matricule) du salarie dans l'ancien logiciel.
    pub code: S,
    pub conges_spectacles: Option<S>,
}

/// Le Siret espace comme sPAIEctacle l'imprime : 3, 3, 3, 5.
fn siret_espace(siret: &str) -> String {
    if siret.len() != 14 || !siret.is_ascii() {
        return siret.to_string();
    }
    format!("{} {} {} {}", &siret[..3], &siret[3..6], &siret[6..9], &siret[9..])
}

/// Le NIR espace comme sPAIEctacle l'imprime : 1, 2, 2, 2, 3, 3, 2.
fn nir_espace(nir: &str) -> String {
    if nir.len() != 15 || !nir.is_ascii() {
        return nir.to_string();
    }
    let mut out = Vec::new();
    let mut k = 0;
    for n in [1, 2, 2, 2, 3, 3, 2] {
        out.push(&nir[k..k + n]);
        k += n;
    }
    out.join(" ")
}

// ------------------------------------------------------------------------------------------- gabarit A

/// Une ligne du tableau des cotisations : un titre de famille (`famille` sans nombres), ou une cotisation.
/// (x du libelle, libelle, famille attendue, base, taux employeur, montant employeur, taux salarie, montant salarie)
/// Une base `B` s'imprime comme le brut, `C` comme la base de la CSG.
pub type RangA<S = &'static str> = (Mesure, S, &'static str, S, S, S, S, S);

pub struct A<S = &'static str> {
    pub id: Identite<S>,
    pub numero: S,
    /// Numero d'objet et titre du spectacle.
    pub objet: Option<(S, S)>,
    pub emploi: S,
    pub statut: S,
    pub periode: S,
    /// Les rangs des dates travaillees.
    pub dates: Vec<S>,
    /// (quantite, libelle, taux, montant)
    pub lignes: Vec<(S, S, S, S)>,
    pub brut: S,
    pub csg: S,
    /// Les cotisations, dans l'ordre du bulletin clarifie.
    pub cotisations: Vec<RangA<S>>,
    /// Le total des cotisations : (employeur, salarie).
    pub total: [S; 2],
    /// Le net social, qui est aussi le net a payer avant impot (CoolPaie n'a pas de ligne non soumise).
    pub net: S,
    /// (base, taux, montant, cumul)
    pub pas: [S; 4],
    /// (periode, cumul)
    pub net_imposable: [S; 2],
    pub net_a_payer: S,
    pub regle: S,
    /// « mode de reglement » (absent avant 2023).
    pub mode: bool,
    /// « Cachets: » ou « Heures: »
    pub unite: S,
    /// (unite, brut, plafond, cout employeur) x (periode, cumul)
    pub cumuls: [[S; 2]; 4],
    pub total_verse: S,
}

pub fn page_a<S: AsRef<str>>(a: &A<S>) -> Vec<Bloc> {
    let mut b = Vec::new();
    let b_ = &mut b;
    let id = &a.id;
    grand(b_, m("320"), m("45"), &format!("Bulletin de paie n° {}", s(&a.numero)), m("11"));
    if let Some((objet, titre)) = &a.objet {
        t(b_, m("354.3"), m("65"), &format!("numéro d'objet : {}", s(objet)));
        t(b_, m("354.3"), m("76"), &format!("Spectacle : {}", s(titre)));
    }
    grand(b_, m("28.4"), m("120"), s(&id.entreprise[0]), m("11"));
    t(b_, m("283.5"), m("120"), &format!("{} {}", s(&id.nom).to_uppercase(), s(&id.prenom).to_uppercase()));
    t(b_, m("28.4"), m("132"), s(&id.entreprise[1]));
    t(b_, m("283.5"), m("132"), &s(&id.adresse[0]).to_uppercase());
    t(b_, m("28.4"), m("143"), s(&id.entreprise[2]));
    t(b_, m("283.5"), m("143"), &s(&id.adresse[1]).to_uppercase());
    for (y, gauche, valeur_g, droite, valeur_d) in [
        (m("172"), "N° Siret :", s(&id.siret), "Date de naissance :", s(&id.naissance)),
        (m("183"), "NAF(APE) :", s(&id.naf), "N° Sécurité Sociale :", s(&id.nir)),
        (m("194"), "Urssaf :", s(&id.urssaf), "Code Salarié :", s(&id.code)),
        (m("205"), "Conv. collective :", s(&id.convention[0]), "Emploi :", s(&a.emploi)),
        (m("227"), "Licence spec. N° :", s(&id.licence), "Abattement :", "0 %"),
    ] {
        t(b_, m("28.4"), y, gauche);
        if !valeur_g.is_empty() {
            t(b_, m("127.5"), y, valeur_g);
        }
        t(b_, m("283.5"), y, droite);
        t(b_, m("436.4"), y, valeur_d);
    }
    if !s(&id.convention[1]).is_empty() {
        t(b_, m("127.5"), m("216"), s(&id.convention[1]));
    }
    t(b_, m("283.5"), m("217.5"), "Statut :");
    t(b_, m("436.4"), m("217.5"), s(&a.statut));
    t(b_, m("283.5"), m("238"), "N° de Congés Spectacles :");
    if let Some(c) = &id.conges_spectacles {
        t(b_, m("436.4"), m("238"), s(c));
    }
    t(b_, m("79.5"), m("262"), "Périodes :");
    t(b_, m("135.9"), m("262"), s(&a.periode));
    t(b_, m("42.8"), m("273"), "Dates travaillées :");
    for (k, d) in a.dates.iter().enumerate() {
        t(b_, m("135.9"), m("273") + m("11") * k as i64, s(d));
    }
    let mut y = m("308");
    t(b_, m("104.9"), y, "LIBELLE");
    t(b_, m("324.3"), y, "Base");
    t(b_, m("382.6"), y, "Part Employeur");
    t(b_, m("504.6"), y, "Part Salarié");
    for (q, libelle, taux, montant) in &a.lignes {
        y += m("11.2");
        t(b_, m("56.7"), y, &format!("{} {}", s(q), s(libelle)));
        r(b_, m("346"), y, s(taux));
        r(b_, m("453.7"), y, s(montant));
    }
    y += m("11.2");
    t(b_, m("56.7"), y, "SALAIRE BRUT");
    r(b_, m("453.7"), y, s(&a.brut));
    y += m("14.2");
    for (x, texte) in [(m("394.1"), "Taux"), (m("453.9"), "Montant"), (m("504.8"), "Taux"), (m("561.6"), "Montant")] {
        r(b_, x, y, texte);
    }
    for (x, libelle, _, base, te, me, ts, ms) in a.cotisations.iter() {
        y += m("11.2");
        t(b_, *x, y, s(libelle));
        let base = match s(base) {
            "B" => s(&a.brut),
            "C" => s(&a.csg),
            autre => autre,
        };
        for (xd, v) in [(m("345.9"), base), (m("394.2"), s(te)), (m("453.7"), s(me)), (m("504.9"), s(ts)), (m("561.5"), s(ms))] {
            if !v.is_empty() {
                r(b_, xd, y, v);
            }
        }
    }
    y += m("22");
    t(b_, m("36.8"), y, "Total des Cotisations et Contributions");
    r(b_, m("434"), y, s(&a.total[0]));
    r(b_, m("561.4"), y, s(&a.total[1]));
    y += m("20.3");
    t(b_, m("33.9"), y, "NET SOCIAL");
    r(b_, m("561.6"), y, s(&a.net));
    y += m("11");
    t(b_, m("33.9"), y, "NET A PAYER AVANT IMPOT SUR LE REVENU");
    r(b_, m("561.5"), y, s(&a.net));
    // Le pied.
    y += m("11.5");
    t(b_, m("33.9"), y, "Impôt sur le revenu");
    t(b_, m("310.9"), y, "base");
    t(b_, m("396.3"), y, "taux personnalisé");
    t(b_, m("486.8"), y, "montant");
    t(b_, m("543"), y, "cumul");
    y += m("14");
    t(b_, m("33.9"), y, "Impôt sur le revenu prélevé à la source");
    r(b_, m("331.7"), y, s(&a.pas[0]));
    r(b_, m("466.8"), y, s(&a.pas[1]));
    t(b_, m("471.6"), y, "%");
    r(b_, m("516"), y, s(&a.pas[2]));
    r(b_, m("567.2"), y, s(&a.pas[3]));
    y += m("8.5");
    t(b_, m("33.9"), y, "Net Imposable");
    r(b_, m("518.9"), y, s(&a.net_imposable[0]));
    r(b_, m("567.2"), y, s(&a.net_imposable[1]));
    y += m("8.5");
    t(b_, m("33.9"), y, "Montant net des heures compl / suppl exonérées");
    r(b_, m("518.9"), y, "0,00");
    r(b_, m("567.2"), y, "0,00");
    y += m("13");
    t(b_, m("131.8"), y, "Période");
    t(b_, m("203.6"), y, "Cumul");
    t(b_, m("435.7"), y, "NET A PAYER AU SALARIE");
    t(b_, m("255.2"), y + m("2"), &format!("Réglement le : {}", s(&a.regle)));
    t(b_, m("460.3"), y + m("10"), "(EN EUROS)");
    for (k, libelle) in [s(&a.unite), "Salaire brut:", "Plafond Urssaf:", "Cout employeur:"].iter().enumerate() {
        let yk = y + m("14") + m("11.2") * k as i64;
        t(b_, m("36.8"), yk, libelle);
        r(b_, m("170.2"), yk, s(&a.cumuls[k][0]));
        r(b_, m("235.5"), yk, s(&a.cumuls[k][1]));
    }
    if a.mode {
        t(b_, m("255.2"), y + m("14"), "mode de règlement : Virement");
    }
    grand(b_, m("469.5"), y + m("32"), &format!("{} €", s(&a.net_a_payer)), m("14")); // forme imposée : l'imprimé du logiciel imité
    t(b_, m("439.7"), y + m("51"), "Total versé par l'employeur");
    grand(b_, m("478.9"), y + m("72"), &format!("{} €", s(&a.total_verse)), m("11")); // forme imposée : l'imprimé du logiciel imité
    grand(b_, m("31"), m("810"), "coolpaie 4.0.0.8.312", m("7"));
    b
}

// ------------------------------------------------------------------------------------------- gabarit B

/// (x, libelle, famille attendue, base, taux salarial, part salarie, part employeur)
pub type RangB<S = &'static str> = (Mesure, S, &'static str, S, S, S, S);

pub struct B<S = &'static str> {
    /// L'identite : l'entreprise sur trois rangs, la convention sur deux, le code du salarie en haut a droite.
    pub id: Identite<S>,
    pub periode: S,
    pub periode_courte: S,
    pub reglement: S,
    pub objet: S,
    pub analytique: S,
    pub emploi: S,
    /// « Artiste », « Technicien », « Non cadre »...
    pub statut: S,
    /// (quantite, libelle, base, montant)
    pub lignes: Vec<(S, S, S, S)>,
    pub brut: (S, S),
    pub cotisations: Vec<RangB<S>>,
    pub total: (S, S, S),
    pub apres_total: Vec<RangB<S>>,
    pub net_social: Option<S>,
    pub non_soumis: Vec<(S, S, S, S)>,
    pub net_avant_impot: S,
    /// L'en-tete du PAS : (x, mot) ; puis la ligne du PAS (libelle, [(x droite, valeur)]).
    pub entete_pas: Vec<(Mesure, &'static str)>,
    pub pas: (S, Vec<(Mesure, S)>),
    pub net_imposable: Option<(S, S)>,
    /// Les cumuls : (libelle, periode, cumul).
    pub cumuls: Vec<(S, S, S)>,
    pub net_paye: S,
    /// « Total verse par » : avec le cadre de l'allegement (2020-2022), la valeur dessous ; sinon a droite.
    pub allegement: bool,
    pub total_verse: S,
}

pub fn page_b<S: AsRef<str>>(p: &B<S>) -> Vec<Bloc> {
    let mut b = Vec::new();
    let b_ = &mut b;
    let id = &p.id;
    grand(b_, m("39"), m("36"), s(&id.entreprise[0]), m("10"));
    grand(b_, m("375"), m("38"), "Bulletin de paie", m("14"));
    t(b_, m("545.6"), m("36"), s(&id.code));
    t(b_, m("39"), m("54"), s(&id.entreprise[1]));
    t(b_, m("375"), m("55"), "Période");
    t(b_, m("443"), m("55"), s(&p.periode));
    t(b_, m("39"), m("76"), s(&id.entreprise[2]));
    t(b_, m("375"), m("67.5"), s(&p.periode_courte));
    t(b_, m("39"), m("93"), "Siret");
    t(b_, m("87"), m("93"), &siret_espace(s(&id.siret)));
    t(b_, m("39"), m("102"), "Code APE");
    t(b_, m("87"), m("102"), s(&id.naf));
    t(b_, m("375"), m("107"), "Règlement");
    t(b_, m("443"), m("107"), s(&p.reglement));
    t(b_, m("375"), m("121"), "Numéro d'objet");
    if !s(&p.objet).is_empty() {
        t(b_, m("443"), m("121"), s(&p.objet));
    }
    t(b_, m("39"), m("126.5"), &format!("Convention Collective : {}", s(&id.convention[0])));
    t(b_, m("375"), m("133"), "Analytique");
    if !s(&p.analytique).is_empty() {
        t(b_, m("443"), m("133"), s(&p.analytique));
    }
    if !s(&id.convention[1]).is_empty() {
        t(b_, m("39"), m("135.7"), s(&id.convention[1]));
    }
    t(b_, m("39"), m("145"), "Durée congés payés : code du travail, art L3141-3 à 11");
    grand(b_, m("314"), m("175"), &format!("{} {}", s(&id.prenom), s(&id.nom).to_uppercase()), m("10"));
    t(b_, m("39"), m("176.5"), "Emploi");
    t(b_, m("108"), m("176.5"), s(&p.emploi));
    t(b_, m("39"), m("187.5"), "Statut");
    t(b_, m("108"), m("187.5"), s(&p.statut));
    t(b_, m("314"), m("200"), s(&id.adresse[0]));
    t(b_, m("39"), m("209.5"), "NIR");
    t(b_, m("108"), m("209.5"), &nir_espace(s(&id.nir)));
    if let Some(c) = &id.conges_spectacles {
        t(b_, m("39"), m("220.5"), "N° Congés Sp.");
        t(b_, m("108"), m("220.5"), s(c));
    }
    match s(&id.adresse[1]).split_once(' ') {
        Some((cp, commune)) => {
            t(b_, m("314"), m("222"), cp);
            if !commune.is_empty() {
                t(b_, m("369"), m("222"), commune);
            }
        }
        None => t(b_, m("314"), m("222"), s(&id.adresse[1])),
    }
    t(b_, m("314"), m("233"), "FRANCE");
    t(b_, m("499"), m("264"), "XYZ");
    t(b_, m("399.8"), m("279.5"), "Taux");
    t(b_, m("458.2"), m("279.5"), "Part");
    t(b_, m("523.2"), m("279.5"), "Part");
    t(b_, m("126.2"), m("286.5"), "libellé");
    t(b_, m("339.7"), m("286.5"), "base");
    t(b_, m("394.5"), m("290"), "salarial");
    t(b_, m("452.7"), m("290"), "salarié");
    t(b_, m("509.2"), m("290"), "employeur");
    let mut y = m("308.5");
    let ligne = |b_: &mut Vec<Bloc>, y: Mesure, (q, libelle, base, montant): &(S, S, S, S)| {
        t(b_, m("59.5"), y, &format!("{} ", s(q)));
        t(b_, m("74"), y, s(libelle));
        r(b_, m("374.4"), y, s(base));
        r(b_, m("494.4"), y, s(montant));
    };
    for l in &p.lignes {
        ligne(b_, y, l);
        y += m("11");
    }
    t(b_, m("74"), y, s(&p.brut.0));
    r(b_, m("494.4"), y, s(&p.brut.1));
    let cotisation = |b_: &mut Vec<Bloc>, y: Mesure, c: &RangB<S>| {
        t(b_, c.0, y, s(&c.1));
        if !s(&c.3).is_empty() {
            r(b_, m("374.4"), y, s(&c.3));
        }
        if !s(&c.4).is_empty() {
            r(b_, m("419.7"), y, s(&c.4));
            t(b_, m("421.9"), y, "%");
        }
        if !s(&c.5).is_empty() {
            r(b_, m("495"), y, s(&c.5));
        }
        if !s(&c.6).is_empty() {
            r(b_, m("559"), y, s(&c.6));
        }
    };
    y += m("9");
    for c in &p.cotisations {
        y += if c.0 < m("50") { m("13") } else { m("10") };
        cotisation(b_, y, c);
    }
    y += m("13");
    t(b_, m("74"), y, s(&p.total.0));
    r(b_, m("494.4"), y, s(&p.total.1));
    r(b_, m("558.3"), y, s(&p.total.2));
    for c in &p.apres_total {
        y += m("12");
        cotisation(b_, y, c);
    }
    if let Some(n) = &p.net_social {
        y += m("18");
        t(b_, m("74"), y, "MONTANT NET SOCIAL");
        r(b_, m("494.2"), y, s(n));
    }
    for l in &p.non_soumis {
        y += m("18");
        ligne(b_, y, l);
    }
    let mut y = m("655");
    t(b_, m("39"), y, "NET A PAYER AVANT IMPOT SUR LE REVENU");
    r(b_, m("494.2"), y, s(&p.net_avant_impot));
    y += m("19");
    t(b_, m("99.6"), y, "Impôt sur le revenu");
    for (x, mot) in &p.entete_pas {
        t(b_, *x, y, mot);
    }
    if let Some((n, c)) = &p.net_imposable {
        y += m("17");
        t(b_, m("39"), y, "Montant net imposable");
        r(b_, m("486.2"), y, s(n));
        r(b_, m("563"), y, s(c));
        y += m("13");
        t(b_, m("39"), y, "Montant net des HC/HS/RTT exonérées");
        r(b_, m("563"), y, "0,00");
    }
    y += m("16");
    t(b_, m("39"), y, s(&p.pas.0));
    for (x, v) in &p.pas.1 {
        r(b_, *x, y + m("0.5"), s(v));
    }
    let mut y = m("737");
    t(b_, m("39"), y, "BASE");
    t(b_, m("131"), y, "Période");
    t(b_, m("189"), y, "Cumul");
    t(b_, m("475"), y + m("3"), "Net payé");
    for (libelle, periode, cumul) in &p.cumuls {
        y += m("11");
        t(b_, m("39"), y, s(libelle));
        if !s(periode).is_empty() {
            r(b_, m("165"), y, s(periode));
            r(b_, m("220"), y, s(cumul));
        }
    }
    grand(b_, m("493.3"), m("765"), &format!("{} €", s(&p.net_paye)), m("14")); // forme imposée : l'imprimé du logiciel imité
    t(b_, m("445"), m("787"), "Total versé par");
    t(b_, m("450.2"), m("795"), "l'employeur");
    if p.allegement {
        t(b_, m("510"), m("787"), "Allègement de");
        t(b_, m("506"), m("795"), "cotisations empl.");
        grand(b_, m("460"), m("812"), &format!("{} €", s(&p.total_verse)), m("11")); // forme imposée : l'imprimé du logiciel imité
        grand(b_, m("532"), m("812"), "0,00 €", m("11"));
    } else {
        grand(b_, m("524.7"), m("792"), &format!("{} €", s(&p.total_verse)), m("11")); // forme imposée : l'imprimé du logiciel imité
    }
    grand(b_, m("504.7"), m("828"), "GHS - sPAIEctacle ©", m("6.5"));
    b
}
