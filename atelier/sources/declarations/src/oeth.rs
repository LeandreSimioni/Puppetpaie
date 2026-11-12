//! L'obligation d'emploi des travailleurs handicapes (OETH) : la declaration
//! annuelle que la DSN d'avril porte pour l'annee precedente.
//! Les chiffres viennent de `config/oeth.json`, par
//! millesime ; le calcul applique D5212-2, -20 a -24.

use droit::contexte::Contexte;
use droit::cotisation_etablissement::CotisationDEtablissement;
use serde::Deserialize;
use socle::date::Jour;
use socle::echec::{refus_avec, Lieu, R};
use socle::fichiers::repr;
use socle::forme::Nombre;
use socle::texte::{euros, virgule};
use socle::num::{add, eur2, mul, sub, zero2, Decimal};
use std::collections::BTreeMap;

const SEUIL: i64 = 20;
/// Le fichier des chiffres OETH de la compagnie, un par millesime.
const OETH: &str = "config/oeth.json";
const MOIS_DE_DECLARATION: u32 = 4;
const CTP_CONTRIBUTION: &str = "730";
const CTP_ACCORD: &str = "740";

/// Les categories de D5212-23 et leur code en S21.G00.82.002.
const DEPENSES: [(&str, CotisationDEtablissement); 3] = [
    ("accessibilite", CotisationDEtablissement::DepenseAccessibilite),
    ("maintien", CotisationDEtablissement::DepenseMaintien),
    ("accompagnement", CotisationDEtablissement::DepenseAccompagnement),
];

/// S21.G00.13.002 : les types de beneficiaires mis a disposition.
const TYPES_EXTERNES: [&str; 6] = ["01", "02", "03", "04", "05", "06"];

fn taux() -> Decimal {
    Decimal::from(6)
}

/// Ce que la DSN d'avril porte pour un millesime.
#[derive(Clone, Debug)]
pub struct Declaration {
    pub millesime: i32,
    pub effectif: Decimal,
    pub obligation: Decimal,
    pub beneficiaires: Decimal,
    pub manquants: Decimal,
    pub coefficient: i64,
    pub smic: Decimal,
    pub brute: Decimal,
    pub nette: Decimal,
    /// Les deductions non plafonnees, telles qu'elles se declarent (code -> montant).
    pub deductions: BTreeMap<CotisationDEtablissement, Decimal>,
    /// [(type, nombre)] des beneficiaires mis a disposition.
    pub externes: Vec<(String, Decimal)>,
    /// Le code de l'accord agree, ou `None`.
    pub accord: Option<String>,
    /// Sous un accord agree, les depenses non realisees, si la compagnie en ecrit.
    pub non_realisees: Option<Decimal>,
    pub source: String,
}

impl Declaration {
    /// Les depenses non realisees sous un accord agree, quand il y en a.
    fn non_realisees(&self) -> Option<Decimal> {
        self.non_realisees.filter(|m| !m.is_zero())
    }

    /// La contribution reelle due (068), celle du CTP 730.
    pub fn due(&self) -> Decimal {
        self.nette
    }

    /// [(code, montant)] du bloc S21.G00.82, dans l'ordre des codes.
    pub fn blocs_82(&self) -> Vec<(CotisationDEtablissement, Decimal)> {
        if self.accord.is_some() {
            return self.non_realisees().map(|m| (CotisationDEtablissement::OethNonRealisees, m)).into_iter().collect();
        }
        let mut l: Vec<(CotisationDEtablissement, Decimal)> =
            self.deductions.iter().filter(|(_, m)| !m.is_zero()).map(|(c, m)| (*c, *m)).collect();
        l.push((CotisationDEtablissement::OethBrute, self.brute));
        l.push((CotisationDEtablissement::OethNette, self.nette));
        l.push((CotisationDEtablissement::OethReelleDue, self.due()));
        l.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        l
    }

    /// Ce que la declaration ajoute au bordereau Urssaf, en euros (`Declaration.a_verser`).
    pub fn a_verser(&self) -> Decimal {
        self.bordereau().iter().fold(zero2(), |a, (_, m)| a + *m)
    }

    /// `Declaration.resume` : la phrase que `produire` dit au mois d'avril.
    pub fn resume(&self) -> String {
        if let Some(accord) = &self.accord {
            let non: String = self.non_realisees().iter().map(|m| format!(", {} de dépenses non réalisées (CTP 740)", euros(*m))).collect();
            return format!("OETH {} : accord agréé {}{}.", self.millesime, accord, non);
        }
        format!(
            "OETH {} : effectif {}, {} bénéficiaire(s) dû(s), {} employé(s), {} manquant(s) x {} x SMIC {} = {} brute, {} nette (CTP 730).",
            self.millesime,
            virgule(self.effectif),
            virgule(self.obligation),
            virgule(self.beneficiaires),
            virgule(self.manquants),
            self.coefficient,
            euros(self.smic),
            euros(self.brute),
            euros(self.nette)
        )
    }

    /// [(ctp, montant)] a ajouter au bordereau Urssaf.
    pub fn bordereau(&self) -> Vec<(String, Decimal)> {
        if self.accord.is_some() {
            return self.non_realisees().map(|m| (CTP_ACCORD.to_string(), m)).into_iter().collect();
        }
        if self.due().is_zero() {
            vec![]
        } else {
            vec![(CTP_CONTRIBUTION.to_string(), self.due())]
        }
    }
}

/// `config/oeth.json` : les chiffres de chaque millesime, sous son annee (`null` : rien de saisi).
type FichierOeth = BTreeMap<String, Option<EntreeOeth>>;

/// Les chiffres OETH d'un millesime, tels que l'Urssaf les transmet avant le 15 mars (D5212-5), et ce que la compagnie
/// sait d'elle-meme : son accord agree, ses depenses, sa sous-traitance ; ou qu'elle n'est pas assujettie, et pourquoi.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntreeOeth {
    pub effectif_assujettissement: Option<Nombre>,
    pub boeth: Option<Nombre>,
    pub ecap: Option<Nombre>,
    /// D'ou viennent les chiffres.
    pub source: Option<String>,
    /// Les beneficiaires mis a disposition (S21.G00.13.002).
    pub boeth_externes: Option<Vec<BoethExterne>>,
    /// Le code de l'accord agree (table AAETH, S21.G00.13.001).
    pub accord: Option<String>,
    pub depenses_non_realisees: Option<Nombre>,
    /// Les beneficiaires dus que l'Urssaf transmet, confrontes au calcul (D5212-2).
    pub boeth_dus: Option<Nombre>,
    pub sous_traitance: Option<Nombre>,
    /// Les depenses deductibles de D5212-23.
    pub depenses: Option<Depenses>,
    /// Sans beneficiaire ni sous-traitance : depuis plus de trois ans (D5212-21) ?
    pub aucune_action_depuis_plus_de_trois_ans: Option<bool>,
    /// `false` : la compagnie n'est pas assujettie, et `pourquoi` le dit.
    pub assujetti: Option<bool>,
    pub pourquoi: Option<String>,
}

/// Un type de beneficiaire mis a disposition, et combien.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoethExterne {
    #[serde(rename = "type")]
    pub type_beneficiaire: String,
    pub nombre: Nombre,
}

/// Les trois categories de depenses de D5212-23 en vigueur (les codes 071 a 073 du cahier technique ne correspondent a
/// aucun texte lu).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Depenses {
    pub accessibilite: Option<Nombre>,
    pub maintien: Option<Nombre>,
    pub accompagnement: Option<Nombre>,
}

impl Depenses {
    fn de(&self, categorie: &str) -> Option<Nombre> {
        match categorie {
            "accessibilite" => self.accessibilite,
            "maintien" => self.maintien,
            _ => self.accompagnement,
        }
    }
}

/// Un nombre positif ou nul de `config/oeth.json`.
fn dec(v: Nombre, nom: &str, millesime: i32) -> R<Decimal> {
    let x = v.0;
    if x.is_sign_negative() && !x.is_zero() {
        return Err(socle::echec::a_la_cle(&format!("La déclaration OETH de {} porte un chiffre négatif.", millesime), nom, x.to_string()));
    }
    Ok(x)
}

/// Un montant de `config/oeth.json` que la compagnie n'ecrit que s'il y en a, au centime.
fn montant(v: Option<Nombre>, nom: &str, millesime: i32) -> R<Option<Decimal>> {
    v.map(|n| dec(n, nom, millesime).map(eur2)).transpose()
}

/// D5212-20 et D5212-21.
pub fn coefficient(effectif: Decimal, sans_action: bool) -> i64 {
    if sans_action {
        1500
    } else if effectif >= Decimal::from(750) {
        600
    } else if effectif >= Decimal::from(250) {
        500
    } else {
        400
    }
}

/// La declaration d'un millesime, depuis son entree de `config/oeth.json` et le
/// SMIC horaire brut au 31/12 de ce millesime.
pub fn calculer(millesime: i32, donnees: &EntreeOeth, smic: Decimal) -> R<Declaration> {
    let ou = format!("La déclaration OETH de {}", millesime);
    let source = donnees.source.as_deref().filter(|s| !s.is_empty());
    let manque: Vec<&str> = [
        ("effectif_assujettissement", donnees.effectif_assujettissement.is_none()),
        ("boeth", donnees.boeth.is_none()),
        ("ecap", donnees.ecap.is_none()),
        ("source", source.is_none()),
    ]
    .into_iter()
    .filter(|(_, manque)| *manque)
    .map(|(k, _)| k)
    .collect();
    let (Some(effectif), Some(boeth), Some(ecap), Some(source)) = (donnees.effectif_assujettissement, donnees.boeth, donnees.ecap, source) else {
        return Err(socle::echec::cle(
            format!(
                "{} : il manque des chiffres, ceux que l'Urssaf transmet avant le 15 mars (D5212-5) -- \
effectif d'assujettissement, beneficiaires employes, effectif ECAP -- et d'ou ils viennent. \
Rien n'est ecrit.",
                ou
            ),
            &manque.join(", "),
        ));
    };
    let eff = dec(effectif, "effectif_assujettissement", millesime)?;
    let mut d = Declaration {
        millesime,
        effectif: eff,
        obligation: Decimal::ZERO,
        beneficiaires: Decimal::ZERO,
        manquants: Decimal::ZERO,
        coefficient: 0,
        smic,
        brute: zero2(),
        nette: zero2(),
        deductions: BTreeMap::new(),
        externes: vec![],
        accord: None,
        non_realisees: None,
        source: source.to_string(),
    };

    let mut externes = vec![];
    for x in donnees.boeth_externes.iter().flatten() {
        let t = &x.type_beneficiaire;
        if !TYPES_EXTERNES.contains(&t.as_str()) {
            return Err(socle::refus!(
                "{} : type de beneficiaire externe {} inconnu (S21.G00.13.002 : {}).",
                ou,
                repr(t),
                TYPES_EXTERNES.join(", ")
            ));
        }
        let n = dec(x.nombre, "boeth_externes.nombre", millesime)?;
        if !n.is_zero() {
            externes.push((t.clone(), n));
        }
    }
    d.externes = externes;

    if let Some(accord) = donnees.accord.as_deref().map(str::trim).filter(|a| !a.is_empty()) {
        if accord.chars().count() != 12 {
            return Err(socle::refus!(
                "{} : le code d'accord agree {} n'a pas douze caracteres (table AAETH, S21.G00.13.001).",
                ou,
                repr(accord)
            ));
        }
        d.accord = Some(accord.to_string());
        d.non_realisees = montant(donnees.depenses_non_realisees, "depenses_non_realisees", millesime)?;
        return Ok(d);
    }

    d.obligation = div_cent(mul(eff, taux())).trunc();
    if let Some(dus) = donnees.boeth_dus {
        let x = dec(dus, "boeth_dus", millesime)?;
        if x != d.obligation {
            return Err(socle::refus!(
                "{} : l'Urssaf transmet {} beneficiaire(s) du(s), et 6 % de {} arrondi a l'entier \
inferieur (D5212-2) en donne {}. L'un des deux est mal recopie -- ou une regle que le moteur ne \
connait pas s'applique. Rien n'est ecrit.",
                ou,
                x,
                eff,
                d.obligation
            ));
        }
    }
    let ext_total = d.externes.iter().fold(Decimal::ZERO, |t, (_, n)| add(t, *n));
    d.beneficiaires = add(dec(boeth, "boeth", millesime)?, ext_total);
    d.manquants = if sub(d.obligation, d.beneficiaires) > Decimal::ZERO {
        sub(d.obligation, d.beneficiaires)
    } else {
        Decimal::ZERO
    };

    // La sous-traitance et les depenses (D5212-22 et -23) : celles que la compagnie ecrit, et elles seules.
    let sous_traitance = montant(donnees.sous_traitance, "sous_traitance", millesime)?;
    let mut depenses: Vec<(CotisationDEtablissement, Decimal)> = Vec::new();
    for (cle, code) in DEPENSES {
        if let Some(m) = montant(donnees.depenses.as_ref().and_then(|d| d.de(cle)), &format!("depenses.{}", cle), millesime)? {
            depenses.push((code, m));
        }
    }

    let sans_action = if d.beneficiaires.is_zero() && sous_traitance.is_none_or(|s| s.is_zero()) {
        let Some(sans_action) = donnees.aucune_action_depuis_plus_de_trois_ans else {
            return Err(refus_avec(
                "La déclaration OETH de l'année ne dit pas depuis quand la compagnie n'a ni bénéficiaire ni sous-traitance : le dire, depuis plus de trois ans ou non.",
                Lieu {
                    fichier: Some(OETH.into()),
                    detail: Some(format!(
                        "{} : aucun beneficiaire et aucune sous-traitance cette annee. La contribution passe a 1 500 \
fois le SMIC par manquant si cela dure depuis PLUS DE TROIS ANS (D5212-21), et 400 fois sinon : ecrire \
\"aucune_action_depuis_plus_de_trois_ans\": true ou false. Le logiciel ne connait pas assez le passe \
pour le dire. Rien n'est ecrit.",
                        ou
                    )),
                    ..Default::default()
                },
            ));
        };
        sans_action
    } else {
        false
    };
    d.coefficient = coefficient(eff, sans_action);
    d.brute = eur2(mul(mul(d.manquants, Decimal::from(d.coefficient)), smic));

    let ecap = eur2(mul(mul(dec(ecap, "ecap", millesime)?, Decimal::from(17)), smic));
    d.deductions.insert(CotisationDEtablissement::DeductionEcap, ecap);
    if let Some(s) = sous_traitance {
        d.deductions.insert(CotisationDEtablissement::DeductionSousTraitance, s);
    }
    for (code, m) in &depenses {
        d.deductions.insert(*code, *m);
    }

    // Les plafonds, sur la contribution brute (D5212-22 et -23).
    let moitie_du_taux = mul(d.beneficiaires, Decimal::ONE_HUNDRED) >= socle::num::div(mul(eff, taux()), Decimal::from(2));
    let part = if moitie_du_taux { Decimal::new(75, 2) } else { Decimal::new(50, 2) };
    let plafond_st = eur2(mul(d.brute, part));
    let plafond_dep = eur2(mul(d.brute, Decimal::new(10, 2)));
    let total_dep = depenses.iter().fold(zero2(), |t, (_, m)| add(t, *m));
    let sous_traitance_retenue = sous_traitance.iter().fold(zero2(), |t, s| add(t, (*s).min(plafond_st)));
    let retenu = add(add(ecap, sous_traitance_retenue), total_dep.min(plafond_dep));
    let reste = sub(d.brute, retenu);
    d.nette = if reste > zero2() { reste } else { zero2() };
    Ok(d)
}

fn div_cent(x: Decimal) -> Decimal {
    socle::num::div(x, Decimal::ONE_HUNDRED)
}

/// La declaration OETH que la DSN de `mois` doit porter, ou `None` : seulement
/// en avril, pour l'annee precedente, et si l'employeur est assujetti. Refuse si
/// l'effectif dit qu'il l'est et que les chiffres de l'Urssaf ne sont pas ecrits.
pub fn declaration(ctx: &Contexte, mois: socle::mois::Mois) -> R<Option<Declaration>> {
    if mois.numero() != MOIS_DE_DECLARATION {
        return Ok(None);
    }
    let millesime = mois.annee() - 1;
    let chemin = ctx.racine.join(OETH);
    let mut fichier: FichierOeth = if socle::fichiers::existe(&chemin) {
        socle::forme::lire_fichier(&chemin, "Les chiffres OETH de la compagnie (config/oeth.json) ne se lisent pas : les corriger.")?
    } else {
        FichierOeth::new()
    };
    let Some(donnees) = fichier.remove(&millesime.to_string()).flatten() else {
        if !assujetti_selon_l_effectif(ctx, millesime)? {
            return Ok(None);
        }
        return Err(refus_avec("La compagnie doit la déclaration OETH de l'année, et ses chiffres ne sont pas saisis : y écrire ceux que l'Urssaf transmet avant le 15 mars.", Lieu {
            fichier: Some(OETH.into()),
            detail: Some(format!(
                "DSN de {} : l'entreprise a atteint {} salaries cinq annees de suite, elle doit la declaration \
OETH de {} (L5212-1, D5212-8), et config/oeth.json ne porte rien pour {}. Y ecrire les chiffres que \
l'Urssaf transmet avant le 15 mars : \"{}\": {{\"effectif_assujettissement\": ..., \"boeth\": ..., \
\"ecap\": ..., \"source\": \"...\"}}. Rien n'est ecrit.",
                mois, SEUIL, millesime, millesime, millesime
            )),
            ..Default::default()
        }));
    };
    if donnees.assujetti == Some(false) {
        if donnees.pourquoi.as_deref().is_none_or(|p| p.trim().is_empty()) {
            return Err(refus_avec(
                "La déclaration OETH de l'année dit la compagnie non assujettie sans dire pourquoi : dire la raison.",
                Lieu {
                    fichier: Some(OETH.into()),
                    detail: Some(format!(
                        "config/oeth.json, {} : \"assujetti\": false sans \"pourquoi\". Dire la raison (delai de cinq \
ans d'une entreprise creee a vingt salaries, L5212-4 ; seuil pas encore franchi, L. 130-1).",
                        millesime
                    )),
                    ..Default::default()
                },
            ));
        }
        return Ok(None);
    }
    if let Some(eff) = donnees.effectif_assujettissement {
        let accord = donnees.accord.as_deref().is_some_and(|a| !a.is_empty());
        if eff.0 < Decimal::from(SEUIL) && !accord {
            return Ok(None);
        }
    }
    Ok(Some(calculer(millesime, &donnees, smic_au_31_decembre(ctx, millesime)?)?))
}

/// Le SMIC horaire brut au 31/12 du millesime (D5212-20, dernier alinea) : national,
/// lu dans le socle livre ; une annee sans socle se lit chez la compagnie.
pub fn smic_au_31_decembre(ctx: &Contexte, millesime: i32) -> R<Decimal> {
    let jour = Jour::nouveau(millesime, 12, 31).ok_or_else(|| socle::refus!("annee hors limites : {}", millesime))?;
    let socle = droit::contexte::socle_livre_lu(millesime)?;
    let liste = match socle.as_ref().map(|s| &s.smic_horaire) {
        Some(l) if !l.is_empty() => l,
        _ => return ctx.smic(&jour),
    };
    let mut entrees: Vec<&droit::forme_bareme::Datee> = liste.iter().collect();
    // Une valeur sans `depuis` est celle de tout temps (`droit::forme_bareme::Datee`) : elle vient avant les autres.
    entrees.sort_by_key(|e| e.depuis);
    let mut val = None;
    for e in entrees {
        if e.depuis.is_none_or(|d| d <= jour) {
            val = Some(e.valeur);
        }
    }
    val.ok_or_else(|| socle::refus!("Aucun SMIC au {} dans le socle {}.", jour.iso(), millesime))
}

/// L'effectif dit-il l'entreprise assujettie pour ce millesime ? Le code A04 du 118 l'emporte s'il est la et sous le
/// seuil ; sinon la regle des cinq ans (L. 130-1, II, CSS) : un effectif qu'elle ne departage pas refuse, en disant
/// quelles annees manquent (`Effectif::atteint`).
fn assujetti_selon_l_effectif(ctx: &Contexte, millesime: i32) -> R<bool> {
    if let Some(a04) = crate::csa::Effectifs::lus(ctx)?.du_118(millesime, "A04") {
        if a04 < Decimal::from(SEUIL) {
            return Ok(false);
        }
    }
    ctx.effectif()?.atteint(SEUIL, millesime + 1).map_err(|m| m.reformuler(|m| format!("Déclaration OETH de {} : {}", millesime, m)))
}
