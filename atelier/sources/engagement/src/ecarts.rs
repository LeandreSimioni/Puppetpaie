//! LES ECARTS DE CALCUL D'UN ENGAGEMENT IMPORTE : ce que ce moteur aurait calcule de la saisie qu'un autre logiciel a
//! payee, compare ligne a ligne a ce que son bulletin imprime. Le but est de voir ou deux logiciels lisent les textes
//! autrement ; l'engagement importe ne change pas (il se lit, il ne se recalcule pas : `importe`), et la paie calculee
//! ici ne s'ecrit nulle part dans la compagnie (l'atelier la lit, `calculee`, pour les bulletins d'une fictive).
//!
//! LA SAISIE REFAITE : le salarie (sa fiche, a la date de la periode ; le statut, la qualite de cadre et l'emploi que
//! le bulletin imprime passent devant les siens), le spectacle que le bulletin nomme, la nature du contrat importe (`importe::Importe::nature`), les lignes de remuneration que le
//! bulletin imprime, par leur rubrique du catalogue (quantite, taux, heures), aux jours qu'il dit travailles (a defaut,
//! autant de jours de sa periode qu'il en compte : `refaire`), payee le
//! jour du versement. UN CONTRAT PAYE AU MOIS a ses dates (`Importe::contrat_de_la_paie`), son motif de recours
//! (`Importe::motif_recours`), et son salaire mensuel et ses heures par mois dans les cases de sa carte : le salaire que
//! sa ligne de salaire imprime, les heures que la DSN declare pour le contrat, sinon celles que le bulletin imprime
//! (`salaire_et_heures`, d'un mois paye entier : `contrat_au_mois`) ; ses autres lignes, hors repas et nuitees, ne se
//! lisent pas sur un bulletin, et sa paie ne se refait pas. Elle se calcule comme la validation la calculerait (`charger_donnees`), avec les baremes et la
//! configuration a la date de la periode ; une compagnie qui n'a pas encore pose son taux AT/MP de l'annee lit celui que
//! le bulletin applique (`Contexte::avec_le_taux_atmp_lu`).
//!
//! LA COMPARAISON, PAR COTISATION : chaque ligne de cotisation du bulletin importe porte la cotisation que l'import lui a
//! donnee (`cotisation`, par son libelle) ; chaque ligne de la paie calculee, la sienne (`cotisation_de`).
//! Plusieurs lignes d'une meme cotisation s'additionnent (les tranches de l'Apec, les contributions de « Autres charges
//! dues par l'employeur ») ; leur base et leurs taux ne se comparent que quand chaque cote n'en a qu'une. Un bulletin au
//! modele de l'arrete du 25/02/2016 (art. 3) range la caisse des Conges Spectacles dans « Autres charges dues par
//! l'employeur » et la CET dans « Retraite complementaire, CEG et CET T1 » : quand le bulletin importe ne les imprime
//! pas a part, les notres y entrent aussi (`REPLIS`). Un montant compare a l'arrondi de chaque ligne pres (1 centime par
//! ligne additionnee), un taux a la milliemes de point pres (le bulletin en imprime trois decimales).
//!
//! CE QUI NE SE COMPARE PAS LE DIT (regle 1) : une periode sans bareme, une paie sans bulletin (la DSN seule ne porte
//! pas les lignes du bulletin), une ligne de remuneration soumise sans rubrique, une saisie que le calcul refuse (une
//! convention sans couche, une fiche incomplete) ; et, dans une paie comparee, les lignes de cotisation que la table ne
//! rattache a aucune cotisation.
//!
//! CE QUI SE REND NE DESIGNE PERSONNE : la cotisation, les libelles, la grandeur, les deux taux et l'ecart RELATIF d'une
//! base ou d'un montant (jamais le montant), la periode, la nature et le statut.

use std::collections::BTreeMap;

use droit::fiche::Lue as _;
use droit::natures::NatureDeContrat;
use std::path::Path;


use crate::calcul::LigneCotisation;
use droit::forme_bareme::{
    IdDeCotisation, APEC_T1, APEC_T2, ATMP, AUTRES_1, AUTRES_2, AUTRES_3, AUTRES_4, CCN, CET, CONGES_SPEC, FCAP, PARITARISME, PREVOYANCE, RETRAITE_T1, VERSEMENT_MOBILITE,
};
use crate::importe::{CotisationImprimee, Importe, LogicielDUnBulletin, PaieVersee};
use droit::contexte::Contexte;
use socle::echec::{refus, Echec, R};
use socle::fichiers::{Empreinte, EmpreinteCourte};
use socle::mois::Mois;
use socle::grandeurs::{Coefficient, Heures, Montant, MontantUnitaire, Taux};

/// Ce que l'on compare d'une ligne, et qui differe (`MesureDeLEcart::ce_qui_differe`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CeQuiDiffere {
    Base,
    TauxSalarie,
    TauxEmployeur,
    MontantSalarie,
    MontantEmployeur,
    /// Le bulletin importe n'imprime pas une cotisation que ce moteur calcule.
    AbsenteDuBulletin,
    /// Le bulletin importe imprime une cotisation que ce moteur ne calcule pas.
    AbsenteDuCalcul,
}

impl CeQuiDiffere {
    pub fn code(self) -> &'static str {
        match self {
            CeQuiDiffere::Base => "base",
            CeQuiDiffere::TauxSalarie => "taux_salarie",
            CeQuiDiffere::TauxEmployeur => "taux_employeur",
            CeQuiDiffere::MontantSalarie => "montant_salarie",
            CeQuiDiffere::MontantEmployeur => "montant_employeur",
            CeQuiDiffere::AbsenteDuBulletin => "absente_du_bulletin",
            CeQuiDiffere::AbsenteDuCalcul => "absente_du_calcul",
        }
    }

    /// En francais, pour un resume.
    pub fn dite(self) -> &'static str {
        match self {
            CeQuiDiffere::Base => "base",
            CeQuiDiffere::TauxSalarie => "taux salarial",
            CeQuiDiffere::TauxEmployeur => "taux employeur",
            CeQuiDiffere::MontantSalarie => "montant salarial",
            CeQuiDiffere::MontantEmployeur => "montant employeur",
            CeQuiDiffere::AbsenteDuBulletin => "calculée ici, absente du bulletin importé",
            CeQuiDiffere::AbsenteDuCalcul => "sur le bulletin importé, que ce moteur ne calcule pas",
        }
    }
}

/// CE QUE LA COMPARAISON D'UN BULLETIN IMPORTE NOMME, au grain ou un bulletin l'imprime : une cotisation du bareme, ou
/// un regroupement de cotisations que le modele du bulletin imprime sur une ligne (les contributions que l'employeur
/// paie seul, `autres` ; celles de la convention, `convention` ; les tranches de l'Apec, `apec`), ou un total du
/// bulletin. La cle de `cotisations` de la table de lecture des bulletins ; elle s'ecrit par son texte et se trie comme
/// lui (`as_str`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RegroupementDeComparaison {
    Cotisation(IdDeCotisation),
    Autres,
    Convention,
    Apec,
    Total(TotalDuBulletin),
}

/// Un total que le bulletin importe imprime et que la comparaison confronte au calcul.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TotalDuBulletin {
    CotisationsSalariales,
    CotisationsPatronales,
    NetSocial,
    NetImposable,
}

impl TotalDuBulletin {
    pub const TOUS: [TotalDuBulletin; 4] = [TotalDuBulletin::CotisationsSalariales, TotalDuBulletin::CotisationsPatronales, TotalDuBulletin::NetSocial, TotalDuBulletin::NetImposable];

    /// Son nom ecrit.
    pub fn cle(self) -> &'static str {
        match self {
            TotalDuBulletin::CotisationsSalariales => "cotisations_salariales",
            TotalDuBulletin::CotisationsPatronales => "cotisations_patronales",
            TotalDuBulletin::NetSocial => "net_social",
            TotalDuBulletin::NetImposable => "net_imposable",
        }
    }
}

impl RegroupementDeComparaison {
    /// Les contributions que l'employeur paie seul et que le modele du bulletin agrege dans « Autres charges dues par
    /// l'employeur » (arrete du 25/02/2016, art. 3), quand la ligne n'a pas de part salariale : `Autres`. La caisse des
    /// Conges Spectacles y va aussi au bulletin ; la comparaison ne l'y range que si le bulletin importe ne l'imprime pas
    /// a part (`REPLIS`).
    pub const DE_L_EMPLOYEUR_SEUL: [IdDeCotisation; 5] = [AUTRES_1, AUTRES_2, AUTRES_3, AUTRES_4, VERSEMENT_MOBILITE];
    /// Les cotisations de la convention (FNAS, FCAP, paritarisme), sur une ligne du modele : `Convention`.
    pub const DE_LA_CONVENTION: [IdDeCotisation; 3] = [CCN, FCAP, PARITARISME];
    /// Les tranches de l'Apec, sur une ligne du modele : `Apec`.
    pub const DE_L_APEC: [IdDeCotisation; 2] = [APEC_T1, APEC_T2];

    /// Son nom ecrit : la cle de la cotisation, ou celle du regroupement ou du total.
    pub fn as_str(&self) -> &str {
        match self {
            RegroupementDeComparaison::Cotisation(id) => id.as_str(),
            RegroupementDeComparaison::Autres => "autres",
            RegroupementDeComparaison::Convention => "convention",
            RegroupementDeComparaison::Apec => "apec",
            RegroupementDeComparaison::Total(t) => t.cle(),
        }
    }

    /// Celui qu'ecrit `texte` : un regroupement ou un total par son nom, sinon la cotisation de cette cle (vide, elle se
    /// refuse).
    pub fn lire(texte: &str) -> R<RegroupementDeComparaison> {
        Ok(match texte {
            "autres" => RegroupementDeComparaison::Autres,
            "convention" => RegroupementDeComparaison::Convention,
            "apec" => RegroupementDeComparaison::Apec,
            _ => match TotalDuBulletin::TOUS.iter().copied().find(|t| t.cle() == texte) {
                Some(t) => RegroupementDeComparaison::Total(t),
                None => RegroupementDeComparaison::Cotisation(IdDeCotisation::lire(texte)?),
            },
        })
    }
}

impl Ord for RegroupementDeComparaison {
    fn cmp(&self, autre: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(autre.as_str())
    }
}

impl PartialOrd for RegroupementDeComparaison {
    fn partial_cmp(&self, autre: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(autre))
    }
}

impl std::fmt::Display for RegroupementDeComparaison {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl serde::Serialize for RegroupementDeComparaison {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for RegroupementDeComparaison {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let texte = <String as serde::Deserialize>::deserialize(d)?;
        RegroupementDeComparaison::lire(&texte).map_err(|e| serde::de::Error::custom(e.phrase))
    }
}

/// D'OU VIENT LE TAUX AT/MP DU CALCUL d'une comparaison : celui que la compagnie a pose pour l'annee, ou celui que le
/// bulletin importe applique (l'AT/MP n'est alors pas compare). Une liste du logiciel, ecrite `compagnie`, `bulletin`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OrigineDuTauxAtmp {
    Compagnie,
    Bulletin,
}

/// Un ecart sur une cotisation (ou un total : `TotalDuBulletin`).
#[derive(Clone, Debug, PartialEq)]
pub struct Ecart {
    pub cotisation: RegroupementDeComparaison,
    /// Les libelles du bulletin importe pour cette cotisation (les notres quand il ne l'imprime pas).
    pub libelles: Vec<String>,
    pub mesure: MesureDeLEcart,
}

/// CE QU'UN ECART MESURE, selon ce qui differe (sa variante le dit) : une base ou un montant, son ecart relatif, (le leur
/// - le notre) / le notre, a 4 decimales, `None` quand le notre est nul ; un taux, le notre et le leur ; une cotisation
/// d'un seul cote, rien.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MesureDeLEcart {
    Base(Option<Coefficient>),
    MontantSalarie(Option<Coefficient>),
    MontantEmployeur(Option<Coefficient>),
    TauxSalarie { notre: Taux, leur: Taux },
    TauxEmployeur { notre: Taux, leur: Taux },
    /// Le bulletin importe n'imprime pas une cotisation que ce moteur calcule.
    AbsenteDuBulletin,
    /// Le bulletin importe imprime une cotisation que ce moteur ne calcule pas.
    AbsenteDuCalcul,
}

impl MesureDeLEcart {
    /// Ce qui differe.
    pub fn ce_qui_differe(self) -> CeQuiDiffere {
        match self {
            MesureDeLEcart::Base(_) => CeQuiDiffere::Base,
            MesureDeLEcart::MontantSalarie(_) => CeQuiDiffere::MontantSalarie,
            MesureDeLEcart::MontantEmployeur(_) => CeQuiDiffere::MontantEmployeur,
            MesureDeLEcart::TauxSalarie { .. } => CeQuiDiffere::TauxSalarie,
            MesureDeLEcart::TauxEmployeur { .. } => CeQuiDiffere::TauxEmployeur,
            MesureDeLEcart::AbsenteDuBulletin => CeQuiDiffere::AbsenteDuBulletin,
            MesureDeLEcart::AbsenteDuCalcul => CeQuiDiffere::AbsenteDuCalcul,
        }
    }
}

/// La comparaison d'une paie importee (un mois de l'engagement).
#[derive(Clone, Debug)]
pub struct Comparaison {
    /// L'empreinte de la paie (`empreinte`).
    pub empreinte: EmpreinteCourte,
    pub mois: Mois,
    /// Le logiciel que le lecteur a reconnu au bulletin ; `None` : un autre.
    pub logiciel: Option<LogicielDUnBulletin>,
    pub nature: NatureDeContrat,
    pub statut: Option<droit::fiche::Statut>,
    /// Cadre (`true`) ou non cadre, quand le bulletin ou la fiche le dit.
    pub cadre: Option<bool>,
    /// D'ou vient le taux AT/MP du calcul.
    pub taux_atmp: Option<OrigineDuTauxAtmp>,
    /// Les ecarts ; `Err` : la paie ne se compare pas, et pourquoi.
    pub ecarts: R<Vec<Ecart>>,
    /// La paie ne s'est pas comparee parce que le calcul a refuse la saisie (une configuration a completer, l'effectif
    /// de l'entreprise...) : elle se compare de nouveau quand la compagnie s'ouvre. Les autres raisons tiennent aux
    /// pieces importees et ne changent pas.
    pub a_refaire: bool,
    /// Les libelles de cotisation du bulletin que la table ne rattache a aucune cotisation : non compares.
    pub non_rattachees: Vec<String>,
}

/// Quand le bulletin importe n'imprime pas la premiere a part, elle se compare dans la seconde (le modele de l'arrete
/// du 25/02/2016, art. 3 : « Autres charges dues par l'employeur » ; la ligne « Retraite complementaire, CEG et CET »).
const REPLIS: [(IdDeCotisation, RegroupementDeComparaison); 2] =
    [(CONGES_SPEC, RegroupementDeComparaison::Autres), (CET, RegroupementDeComparaison::Cotisation(RETRAITE_T1))];

/// La cotisation d'une ligne de la paie calculee, au grain ou un bulletin l'imprime : les contributions que
/// l'employeur paie seul et que le modele agrege (`autres`), les cotisations de la convention (`convention`), les
/// tranches de l'Apec et les garanties de prevoyance de la paie (`garanties`, `calcul::garanties_de_prevoyance`) ; les
/// autres gardent leur identifiant du bareme.
pub fn cotisation_de(l: &LigneCotisation, garanties: &[IdDeCotisation]) -> RegroupementDeComparaison {
    let seule_employeur = l.taux_sal.is_none() && l.montant_sal.est_nul();
    if garanties.contains(&l.id) {
        return RegroupementDeComparaison::Cotisation(PREVOYANCE);
    }
    if RegroupementDeComparaison::DE_L_APEC.contains(&l.id) {
        return RegroupementDeComparaison::Apec;
    }
    if seule_employeur && RegroupementDeComparaison::DE_L_EMPLOYEUR_SEUL.contains(&l.id) {
        return RegroupementDeComparaison::Autres;
    }
    if RegroupementDeComparaison::DE_LA_CONVENTION.contains(&l.id) {
        return RegroupementDeComparaison::Convention;
    }
    RegroupementDeComparaison::Cotisation(l.id.clone())
}

/// Un centime : l'ecart de montant qu'un arrondi fait sur une ligne, et qui n'est pas un ecart.
const UN_CENTIME: Montant = Montant::UN_CENTIME;

/// Cinq dix-milliemes de point : l'ecart de taux qui n'en est pas un.
const ECART_DE_TAUX_TOLERE: Taux = Taux::CINQ_DIX_MILLIEMES;

/// Le taux AT/MP notifie d'un pour cent auquel la paie importee se calcule d'abord : sa ligne AT/MP y porte le facteur
/// que le bareme applique au contrat, d'ou se lit le taux notifie que le bulletin a applique.
const TAUX_ATMP_D_ESSAI: Taux = Taux::points(1);

/// Les comparaisons de chaque paie de l'engagement importe `imp`, mois par mois. `deja` : la paie de cette empreinte (`empreinte`) est
/// deja comparee, elle ne se compare pas deux fois.
pub fn comparer(racine: &Path, imp: &Importe, deja: &dyn Fn(&EmpreinteCourte) -> bool) -> Vec<Comparaison> {
    saisies_refaites(imp)
        .into_iter()
        .filter_map(|(mois, p, nature, jours_comptes)| {
            let empreinte = empreinte(imp, mois, p);
            if deja(&empreinte) {
                return None;
            }
            let logiciel = p.bulletin.as_ref().and_then(|b| b.logiciel.clone());
            let mut suivi = Suivi::default();
            let ecarts = refaire(racine, imp, mois, p, nature, jours_comptes, &mut suivi).map(|r| confronter(&r, p, &mut suivi));
            let Suivi { statut, cadre, taux_atmp, a_refaire, non_rattachees } = suivi;
            Some(Comparaison { empreinte, mois, logiciel, nature, statut, cadre, taux_atmp, ecarts, a_refaire, non_rattachees })
        })
        .collect()
}

/// LA PAIE QUE CE MOTEUR CALCULE de la paie importee du mois `mois` de l'engagement importe `imp` : la saisie que la
/// comparaison refait (`comparer`), calculee par le meme chemin. Un refus dit pourquoi elle ne se calcule pas.
pub fn calculee(racine: &Path, imp: &Importe, mois: Mois) -> R<crate::calcul::Paie> {
    let Some((_, p, nature, jours_comptes)) = saisies_refaites(imp).into_iter().find(|(m, ..)| *m == mois) else {
        return Err(match imp.paies().contains_key(&mois) {
            true => refus("la nature de son contrat reste à dire : la paie ne se refait pas avant (règle 6)"),
            false => socle::refus!("l'engagement importé n'a pas de paie en {}", mois),
        });
    };
    refaire(racine, imp, mois, p, nature, jours_comptes, &mut Suivi::default()).map(|r| r.paie)
}

/// Les paies de l'engagement importe `imp` que ce moteur refait : chacune avec son mois, la nature du contrat et le
/// nombre de jours travailles que son bulletin compte (son cumul « Jours ») : celui que la paie du mois porte (le mois
/// d'un permanent), sinon celui du contrat quand il n'a qu'une paie. UN CONTRAT DONT LA NATURE RESTE A DIRE (sans
/// `nature`, celui d'un permanent : `import::nature`) n'en a aucune : une nature ne se devine pas (regle 6) ; ses paies
/// se refont une fois sa nature dite.
fn saisies_refaites(imp: &Importe) -> Vec<(Mois, &PaieVersee, NatureDeContrat, Option<socle::grandeurs::Jours>)> {
    let paies = imp.paies();
    let du_contrat = imp.contrat().and_then(|c| c.jours).filter(|_| paies.len() == 1);
    imp.nature().into_iter().flat_map(|nature| paies.iter().map(move |(&mois, p)| (mois, p, nature, p.jours.or(du_contrat)))).collect()
}

/// Ce que la saisie refaite dit de la paie, et ce que la comparaison note en chemin (`Comparaison`).
#[derive(Default)]
struct Suivi {
    statut: Option<droit::fiche::Statut>,
    cadre: Option<bool>,
    taux_atmp: Option<OrigineDuTauxAtmp>,
    a_refaire: bool,
    non_rattachees: Vec<String>,
}

/// La paie refaite (`refaire`), et ce que la comparaison en lit.
struct Refaite<'a> {
    paie: crate::calcul::Paie,
    garanties: Vec<IdDeCotisation>,
    /// Le taux AT/MP du calcul est celui que le bulletin applique : l'AT/MP ne se compare pas.
    atmp_lu: bool,
    /// Les lignes de cotisation que le bulletin imprime.
    cotisations: &'a [CotisationImprimee],
}

/// LA SAISIE REFAITE de la paie importee `p` (l'en-tete du module), calculee comme la validation la calculerait ; `c`
/// note ce qu'elle lit du statut, de la qualite de cadre et du taux AT/MP, et si un refus du calcul la fera refaire.
fn refaire<'a>(
    racine: &Path,
    imp: &Importe,
    mois: Mois,
    p: &'a PaieVersee,
    nature: NatureDeContrat,
    jours_comptes: Option<socle::grandeurs::Jours>,
    c: &mut Suivi,
) -> R<Refaite<'a>> {
    let jours_comptes = match jours_comptes.map(socle::grandeurs::entiere) {
        None => None,
        Some(Some(n)) => Some(n),
        Some(None) => return Err(refus("le bulletin compte des jours travaillés qui ne sont pas un nombre entier")),
    };
    let fin = mois.dernier_jour();
    let annee = mois.annee();
    let mut ctx = Contexte::nouveau(racine, Some(fin)).map_err(|m| a_refaire(c, m))?;
    if !ctx.code.join("baremes").join(format!("{}.json", annee)).exists() {
        return Err(socle::refus!("aucun barème livré pour {} : la période ne se calcule pas ici", annee));
    }
    // La convention fait la couche du bareme (regle 8) : sans elle, le calcul n'est pas celui de la compagnie.
    if !ctx.entreprise.idcc.est_dite() {
        c.a_refaire = true;
        return Err(refus("la convention collective de l'entreprise n'est pas renseignée : le calcul n'aurait pas sa couche"));
    }
    let brut = imp.objet().carte();
    let salarie = brut.salarie.as_ref().ok_or_else(|| refus("l'engagement importé ne nomme pas son salarié"))?;
    let fiche = ctx.fiche_de(salarie).map_err(|m| a_refaire(c, m))?;
    // LE STATUT ET L'EMPLOI DE CETTE PAIE sont ceux que son bulletin imprime (`emploi_imprime`) ; ce qu'il ne dit pas
    // se lit dans la fiche. Un salarie artiste sur un contrat, non cadre au regime general sur un autre, se calcule
    // chaque fois sous le sien.
    let dit = |v: &Option<String>| v.clone().filter(|x| !x.is_empty());
    let imprimes = p.emploi_imprime.as_ref().map(|i| &i.ecrit);
    c.statut = imprimes.and_then(|i| i.statut).or_else(|| fiche.statut.dite().copied());
    c.cadre = imprimes.and_then(|i| i.statut_categoriel_rc).map(|s| s == droit::fiche::StatutCategorielRc::Cadre).or_else(|| fiche.statut_categoriel_rc.dite().map(|s| *s == droit::fiche::StatutCategorielRc::Cadre));
    let Some(cotisations) = &p.cotisations else {
        return Err(refus("la paie importée n'a pas de bulletin : la DSN seule ne porte pas ses lignes de cotisation"));
    };
    // LE TAUX AT/MP est celui de l'etablissement, notifie par l'Assurance Maladie. Une compagnie qui n'a pas encore pose
    // le sien pour l'annee lit celui que le bulletin applique (son taux employeur, sinon son montant sur sa base ; un
    // bulletin qui n'imprime que le montant, sur l'assiette de la ligne AT/MP calculee ici), ramene au taux notifie par
    // le facteur que le bareme applique a ce contrat (`droit::bareme::bareme_atmp`), au centieme de point comme un taux
    // notifie. L'AT/MP n'est alors pas compare : il est l'entree.
    let an = annee;
    let mut atmp_imprime: Option<(Option<Taux>, Option<Montant>, Option<Montant>)> = None;
    if !ctx.taux_atmp_dit(an) {
        let ligne = cotisations.iter().find(|l| l.cotisation == Some(RegroupementDeComparaison::Cotisation(ATMP)));
        atmp_imprime = ligne
            .map(|l| (l.taux_employeur, l.montant_employeur, l.base.and_then(crate::calcul::BaseDeCotisation::montant)))
            .filter(|(t, m, _)| t.is_some() || m.is_some_and(|m| !m.est_nul()));
        if atmp_imprime.is_some() {
            ctx = ctx.avec_le_taux_atmp_lu(an, TAUX_ATMP_D_ESSAI);
        }
    }
    let atmp_lu = atmp_imprime.is_some();
    c.taux_atmp = Some(if atmp_lu { OrigineDuTauxAtmp::Bulletin } else { OrigineDuTauxAtmp::Compagnie });
    // Les jours travailles que le bulletin dit, chacun sur chaque ligne (les jours de l'engagement en sont la reunion).
    let mut jours: Vec<crate::DateDeLigne> = p.dates.iter().flat_map(|d| d.jours()).map(crate::DateDeLigne::Nue).collect();
    // UN BULLETIN QUI N'IMPRIME PAS SES JOURS TRAVAILLES en dit la periode et le nombre (`jours_comptes`) :
    // - autant de jours que la periode : ce sont tous les siens ;
    // - moins, dans une periode qui tient dans un mois : le calcul ne lit des jours que leur nombre, leur mois et le
    //   premier. Le plafond d'un intermittent compte les jours remuneres mois par mois (BOSS, assiette generale, § 1090)
    //   ou 12 heures du plafond horaire de l'annee par journee (arrete du 24/01/1975, art. 3, al. 2 : `droit::plafond`),
    //   et l'engagement court se juge sur le debut et la fin du contrat ; le minimum conventionnel et le bareme se
    //   lisent au premier jour, qui est le debut de la periode. Le debut et les jours qui le suivent en tiennent lieu.
    // Sans ce nombre, une periode de plus d'un jour ne dit pas combien : la paie ne se compare pas.
    let mensuelle = nature.proprietes()?.mensuelle();
    if jours.is_empty() && !mensuelle {
        if let Some(contrat) = p.periode.and_then(|o| o.fermee()) {
            // Les jours de la periode, du premier au dernier.
            let de_la_periode: Vec<socle::date::Jour> = contrat.jours().collect();
            let periode = socle::grandeurs::Jours::de_dates(de_la_periode.len());
            let un = socle::grandeurs::Jours::UN;
            let un_mois = contrat.mois().len() == 1;
            let n = if periode == un { Some(un) } else { jours_comptes.filter(|n| *n >= un && *n <= periode && (*n == periode || un_mois)) };
            let Some(n) = n else {
                return Err(refus(match jours_comptes {
                    None => "le bulletin imprime sa période sans ses jours travaillés ni leur nombre",
                    Some(n) if n > periode => "le bulletin compte plus de jours travaillés que sa période n'en a",
                    Some(_) => "le bulletin ne dit pas lesquels de ses jours travaillés tombent dans chacun des mois de sa période",
                }));
            };
            // Ses `n` premiers jours en tiennent lieu.
            for (k, j) in de_la_periode.into_iter().enumerate() {
                if socle::grandeurs::Jours::de_dates(k + 1) > n {
                    break;
                }
                jours.push(crate::DateDeLigne::Nue(j));
            }
        }
    }
    // La ligne de salaire d'un contrat paye au mois (`contrat_au_mois` en lit son salaire mensuel et ses heures par mois).
    let mut au_mois: Option<&crate::importe::LigneImprimee> = None;
    let mut saisies = Vec::new();
    for l in &p.lignes {
        let libelle = &l.libelle;
        let Some(r) = &l.rubrique else {
            if !l.soumis {
                continue;
            }
            return Err(socle::refus!("la ligne de rémunération « {} » ne se rattache à aucune rubrique du catalogue", libelle));
        };
        use crate::ligne::{self, Cle, FormeSaisie, LigneDonnee, LignePaie, PaiementDeLigne, TauxDeLigne};
        let lue = droit::catalogue::rubrique(&ctx.racine, r, &fin)?;
        // UN CONTRAT PAYE AU MOIS ne porte en ligne que ses repas et ses nuitees (`crate::a_une_limite`) : son salaire se
        // dit dans les cases de sa carte (`conges::lignes_du_mois` le compose), lues sur sa ligne de salaire.
        if mensuelle && !crate::a_une_limite(&lue.code) {
            if lue.code != droit::catalogue::HEURE && lue.code != crate::conges::MOIS_PAYE {
                return Err(socle::refus!(
                    "le bulletin d'un contrat payé au mois imprime « {} » : la comparaison n'en lit que la ligne de son salaire mensuel, ses repas et ses nuitées",
                    libelle
                ));
            }
            if au_mois.is_some() {
                return Err(socle::refus!("le bulletin d'un contrat payé au mois imprime une seconde ligne de salaire (« {} ») : son salaire mensuel ne s'y lit pas", libelle));
            }
            au_mois = Some(l);
            continue;
        }
        let paiement = l.paiement.ok_or_else(|| socle::refus!("la ligne « {} » n'imprime pas sa quantité et son taux", libelle))?;
        // Une ligne d'une rubrique que le moteur pose (des heures en plus) : telle que le moteur la compose.
        let Some(forme) = FormeSaisie::de(&lue)? else {
            saisies.push(LignePaie::Composee(ligne::Composee { rubrique: lue.code, paiement, heures: l.heures, precision: None, dates: jours.clone() }));
            continue;
        };
        // LA LIGNE DANS LA FORME QUE LE CATALOGUE DEMANDE DE SA RUBRIQUE, rangee par la porte d'une ligne saisie
        // (`LigneDonnee::ranger`) : elle dit les cles de cette forme, de ce que la ligne imprimee paie. Sa quantite est dans
        // l'unite de sa rubrique, comme l'import l'a rattachee ; son taux, un montant unitaire (une rubrique qui n'est pas
        // en pourcentage) ; une ligne en pourcentage, sa base et la part qu'elle en paie, en pourcent ; une ligne au
        // montant, sa quantite fois son taux, comme le bulletin l'imprime.
        let voulue = |c: Cle| forme.cles().contains(&c);
        let par_unite = || match paiement.taux() {
            TauxDeLigne::ParUnite(u) => Ok(u),
            TauxDeLigne::PartDeLaBase(_) => Err(socle::refus!("la ligne « {} » paie une part de sa base, et sa rubrique ne la saisit pas ainsi : le signaler au développeur", libelle)),
        };
        let (base, part) = match (voulue(Cle::Base), paiement) {
            (false, _) => (None, None),
            (true, PaiementDeLigne::Base(base, part)) => (Some(Some(base)), Some(Some(part.en_taux()))),
            (true, _) => return Err(socle::refus!("la ligne « {} » est d'une rubrique en pourcentage, et l'import ne lui a pas donné de base : le signaler au développeur", libelle)),
        };
        let donnee = LigneDonnee {
            rubrique: Some(lue.code.clone()),
            quantite: voulue(Cle::Quantite).then_some(paiement.quantite()),
            taux: if voulue(Cle::Taux) { Some(Some(par_unite()?)) } else { None },
            heures: voulue(Cle::Heures).then_some(l.heures),
            base,
            taux_majoration: part,
            montant: voulue(Cle::Montant).then_some(Some(paiement.montant())),
            dates: voulue(Cle::Dates).then(|| jours.clone()),
            ..LigneDonnee::default()
        };
        saisies.push(donnee.ranger(&lue)?);
    }
    // Le spectacle que le bulletin nomme ; sans titre, celui dont la fiche porte son numero d'objet.
    let spectacle = match (mensuelle, dit(&p.spectacle)) {
        (true, _) => None,
        (false, Some(titre)) => Some(ctx.spectacle(&titre).map_err(|m| a_refaire(c, m))?.cle),
        (false, None) => p.numero_objet.as_ref().and_then(|n| ctx.spectacles.iter().find(|(_, f)| f.numero_objet.dite() == Some(n)).map(|(k, _)| k.clone())),
    };
    // LE CONTRAT : celui d'un contrat d'usage, la periode de la paie ; celui d'un contrat paye au mois, ses dates
    // (`Importe::contrat_de_la_paie` : sa fin, dite ou declaree, n'est jamais celle du mois que paie un bulletin), son
    // salaire et ses heures (`contrat_au_mois`).
    let (debut, fin_du_contrat, salaire_mensuel, quotite_contrat) = match mensuelle {
        false => (p.periode.map(|o| o.debut()), p.periode.and_then(|o| o.fin()), None, None),
        true => {
            let (contrat, salaire, heures) = contrat_au_mois(imp, p, mois, au_mois)?;
            (Some(contrat.debut()), contrat.fin(), Some(salaire), Some(heures))
        }
    };
    let e = crate::saisie::Carte {
        salarie: Some(salarie.clone()),
        spectacle,
        nature: Some(nature),
        debut,
        fin: fin_du_contrat,
        salaire_mensuel,
        quotite_contrat,
        // Le motif d'un CDD, que l'engagement importe dit ou que sa DSN declare (`Importe::motif_recours`).
        motif_recours: imp.motif_recours().filter(|_| mensuelle),
        // L'emploi que le bulletin imprime, tel quel ; son libelle lisible, l'engagement le resout de son metier.
        emploi: p.emploi_imprime.as_ref().map(|i| droit::emplois::EmploiEcrit { emploi: dit(&i.ecrit.emploi), ..i.ecrit.clone() }).filter(|e| *e != droit::emplois::EmploiEcrit::default()),
        lignes: Some(saisies),
        // Le jour ou l'ancien logiciel a verse : un fait (`Carte::importe`).
        date_paiement: p.verse_le,
        importe: Some(true),
        ..Default::default()
    };
    // Une case que la nature d'un contrat paye au mois exige, et que ni son bulletin ni l'engagement importe ne disent :
    // la paie ne se refait pas (un CDD, son motif de recours).
    if mensuelle {
        let manquent: Vec<String> = crate::carte::CarteValide::cases_de(nature)
            .iter()
            .filter(|(k, x)| *x == crate::carte::Exige::Obligatoire && !e.dit(*k))
            .map(|(k, _)| format!("« {} »", crate::carte::libelle(*k)))
            .collect();
        if !manquent.is_empty() {
            return Err(socle::refus!(
                "ni le bulletin ni l'engagement importé ne disent {} de ce contrat payé au mois : sa paie ne se refait pas",
                manquent.join(", ")
            ));
        }
    }
    let data = crate::saisie::VueDuMois { mois, taux_cachet: None, engagements: vec![e] };
    let mut saisie = crate::charger_donnees(&ctx, &data).map_err(|m| a_refaire(c, m))?;
    if let Some((taux, montant, base)) = atmp_imprime {
        // Calcule au taux notifie de 1 %, la ligne AT/MP porte le facteur du contrat, et son assiette.
        let ligne = saisie.paies.first().and_then(|p| p.lignes.iter().find(|l| l.est(&droit::forme_bareme::ATMP)));
        let facteur = ligne.and_then(|l| l.taux_pat).filter(|f| *f > Taux::ZERO);
        let applique = taux.or_else(|| {
            let b = base.or(ligne.and_then(|l| l.base.montant())).filter(|b| !b.est_nul())?;
            Some(montant?.rapport(b)?.en_taux().a_deux_decimales())
        });
        // Au taux d'essai, la ligne calculee porte ce taux par le facteur : le taux notifie est le taux d'essai par le
        // rapport du taux applique au sien.
        if let Some(rapport) = facteur.zip(applique).and_then(|(f, applique)| applique.rapport(f)) {
            let notifie = (TAUX_ATMP_D_ESSAI * rapport).a_deux_decimales();
            ctx = ctx.avec_le_taux_atmp_lu(an, notifie);
            saisie = crate::charger_donnees(&ctx, &data).map_err(|m| a_refaire(c, m))?;
        }
    }
    let paie = saisie.paies.into_iter().next().ok_or_else(|| refus("le calcul ne rend aucune paie"))?;
    let garanties = crate::calcul::garanties_de_prevoyance(&ctx, &paie)?;
    Ok(Refaite { paie, garanties, atmp_lu, cotisations })
}

/// LE SALAIRE MENSUEL ET LES HEURES PAR MOIS DU CONTRAT, lus sur la ligne de salaire `l` (de la rubrique `heure` ou
/// `mois`) de la paie importee `p`, d'un mois que le contrat et la paie couvrent entier (`contrat_au_mois`).
///
/// LES HEURES PAR MOIS : la quotite que la DSN deposee declare pour le contrat quand elle la declare (S21.G00.40.013,
/// « la duree contractuelle de travail applicable au salarie », `norme-dsn/texte/bloc-S21.G00.40.txt` l. 595-597 :
/// `PaieVersee::quotite`) ; sinon celles que le bulletin imprime, « le nombre d'heures de travail auxquels se rapporte le
/// salaire » (C. trav. R. 3243-1, 5°, `atelier/reference/sources/code-du-travail/articles-20261028.md` l. 442) : sur
/// une ligne en heures (« Heures », « Heure de travail »), ses heures ; sous une ligne « Salaire mensuel de base »
/// (`rubriques-base.json`, `mois`), qui ne les imprime pas, la duree du travail de l'en-tete (« Durée du travail »,
/// `PaieVersee::duree_du_travail`). Que les heures imprimees soient celles du contrat est une inference, qui ne vaut que
/// d'un mois paye entier, et seulement sans DSN. Que la quotite de la DSN compte des heures PAR MOIS est une INFERENCE
/// aussi : la norme ne fixe pas sa periode (elle en donne l'unite, S21.G00.40.011, et la quotite de reference de
/// l'entreprise, S21.G00.40.012). Un logiciel qui y declare la duree de la semaine fait dire « non comparee » a une paie
/// qui pourrait se comparer ; aucune paie n'en est faussee.
///
/// LE SALAIRE MENSUEL : le montant de la ligne, d'un mois paye entier. Une ligne « Salaire mensuel de base » paie le mois
/// entier quand sa quantite, si elle en imprime une, est un mois. Une ligne en heures paie les heures du mois : quand la
/// DSN declare une autre quotite, le bulletin ne paie pas le mois entier du contrat (une absence, des conges, qui se
/// disent en cases), et son montant n'est pas le salaire mensuel. Ce que le bulletin n'imprime pas ne se devine pas, ni
/// ne se prend dans la table de lecture : le refus dit ce qui manque.
fn salaire_et_heures(l: &crate::importe::LigneImprimee, p: &PaieVersee) -> R<(MontantUnitaire, Heures)> {
    use crate::ligne::PaiementDeLigne;
    let libelle = &l.libelle;
    let montant = l.montant.ok_or_else(|| socle::refus!("le bulletin imprime « {} » sans son montant : le salaire mensuel du contrat ne s'y lit pas", libelle))?;
    let declarees = p.quotite.map(|q| q.heures("la quotité que la DSN déclare pour le contrat (S21.G00.40.013)")).transpose()?;
    let dit = socle::grandeurs::ecrit::sans_zero::dit;
    let heures = match (l.paiement, declarees) {
        (Some(PaiementDeLigne::Heures(imprimees, _)), Some(contrat)) if imprimees != contrat => {
            return Err(socle::refus!(
                "le bulletin imprime « {} » pour {} h, et la DSN déclare un contrat de {} h par mois (S21.G00.40.013) : il ne paie pas le mois entier du contrat, son salaire mensuel ne s'y lit pas",
                libelle,
                dit(imprimees),
                dit(contrat)
            ));
        }
        (Some(PaiementDeLigne::Heures(imprimees, _)), _) => imprimees,
        (Some(PaiementDeLigne::PartDuMois(part, _)), _) if part != Coefficient::UN => {
            return Err(socle::refus!("le bulletin imprime « {} » avec une quantité qui n'est pas un mois entier : le salaire mensuel du contrat ne s'y lit pas", libelle));
        }
        (Some(PaiementDeLigne::PartDuMois(..)) | None, Some(contrat)) => contrat,
        (Some(PaiementDeLigne::PartDuMois(..)) | None, None) => p.duree_du_travail.ok_or_else(|| {
            socle::refus!(
                "le bulletin imprime « {} » sans le nombre d'heures de travail auquel le salaire se rapporte (C. trav. R. 3243-1, 5°), ni sur la ligne ni en en-tête (« Durée du travail »), et aucune DSN ne déclare la quotité du contrat : les heures par mois du contrat ne se lisent pas",
                libelle
            )
        })?,
        (Some(_), _) => return Err(socle::refus!("le bulletin imprime « {} » dans une autre unité que des heures ou le mois : le salaire mensuel du contrat ne s'y lit pas", libelle)),
    };
    Ok((montant.pour_une_unite(), heures))
}

/// Les dates `o` couvrent-elles le mois `mois` entier ? Le mois paye entier se juge ainsi, sur le contrat et sur la
/// paie (`contrat_au_mois`, `effectif_repris`).
pub(crate) fn couvre_le_mois(o: Option<socle::date::PeriodeOuverte>, mois: Mois) -> bool {
    let du_mois = socle::date::Periode::du_mois(mois);
    o.and_then(|o| o.croise(&du_mois)) == Some(du_mois)
}

/// LE CONTRAT PAYE AU MOIS QUE PAIE `p`, le mois `mois` : ses dates (`Importe::contrat_de_la_paie`), et le salaire mensuel
/// et les heures par mois que sa ligne de salaire `ligne` et sa DSN disent (`salaire_et_heures`). Le mois ou le contrat
/// finit, son solde de tout compte se compose de cases (le motif de la rupture, le preavis) qu'aucun bulletin n'imprime :
/// la paie ne se refait pas.
///
/// LE MOIS PAYE ENTIER : le contrat le couvre entier ; et la paie aussi. Une paie qu'un bulletin seul declare a la periode
/// de ce bulletin (`routes::importation::bulletins`) : elle couvre le mois entier. Une paie que la DSN declare a les dates
/// du contrat, que la DSN porte, et non celles du bulletin : le mois qu'elle paie se lit sur sa ligne de salaire, des
/// heures que la DSN declare pour le contrat, ou le mois (`salaire_et_heures`).
fn contrat_au_mois(imp: &Importe, p: &PaieVersee, mois: Mois, ligne: Option<&crate::importe::LigneImprimee>) -> R<(socle::date::PeriodeOuverte, MontantUnitaire, Heures)> {
    let Some(l) = ligne else {
        return Err(refus("le bulletin d'un contrat payé au mois n'imprime pas la ligne de son salaire mensuel"));
    };
    let Some(contrat) = imp.contrat_de_la_paie(p)? else {
        return Err(refus("ni l'engagement importé ni le bulletin ne disent le début du contrat"));
    };
    let du_mois = socle::date::Periode::du_mois(mois);
    if contrat.fin().is_some_and(|f| du_mois.contient(&f)) {
        return Err(refus("le contrat payé au mois finit ce mois-ci : son solde de tout compte se dit en cases (le motif de la rupture, le préavis) qu'un bulletin n'imprime pas"));
    }
    if !couvre_le_mois(Some(contrat), mois) {
        return Err(refus("le contrat ne couvre qu'une partie du mois : le salaire mensuel du contrat ne s'y lit pas"));
    }
    if p.contrat.d_un_bulletin() && !couvre_le_mois(p.periode, mois) {
        return Err(refus("le bulletin paie une partie du mois : le salaire mensuel du contrat ne s'y lit pas"));
    }
    let (salaire, heures) = salaire_et_heures(l, p)?;
    Ok((contrat, socle::grandeurs::courte(salaire), socle::grandeurs::courte(heures)))
}

/// Les ecarts de la paie refaite `r` a ce que le bulletin de la paie importee `p` imprime ; `c` note les lignes de
/// cotisation que la table ne rattache a aucune cotisation.
fn confronter(r: &Refaite, p: &PaieVersee, c: &mut Suivi) -> Vec<Ecart> {
    let (paie, garanties, atmp_lu, cotisations) = (&r.paie, &r.garanties, r.atmp_lu, r.cotisations);
    // Les deux cotes, par cotisation.
    let mut leurs: BTreeMap<RegroupementDeComparaison, Vec<&CotisationImprimee>> = BTreeMap::new();
    // Les cotisations que le bulletin imprime apres son total (les facultatives, les exonerations) : hors de ce total.
    let mut hors_total: Vec<RegroupementDeComparaison> = Vec::new();
    for l in cotisations {
        if l.montant_salarie.is_none() && l.montant_employeur.is_none() {
            continue;
        }
        match l.cotisation.clone() {
            Some(code) => {
                if l.hors_total && !hors_total.contains(&code) {
                    hors_total.push(code.clone());
                }
                leurs.entry(code).or_default().push(l)
            }
            None => {
                if !c.non_rattachees.contains(&l.libelle) {
                    c.non_rattachees.push(l.libelle.clone());
                }
            }
        }
    }
    let mut nos: BTreeMap<RegroupementDeComparaison, Vec<&LigneCotisation>> = BTreeMap::new();
    for l in &paie.lignes {
        nos.entry(cotisation_de(l, &garanties)).or_default().push(l);
    }
    for (fin, gros) in REPLIS {
        let fin = RegroupementDeComparaison::Cotisation(fin);
        if !leurs.contains_key(&fin) {
            if let Some(v) = nos.remove(&fin) {
                nos.entry(gros).or_default().extend(v);
            }
        }
    }
    let mut ecarts = Vec::new();
    let mut codes: Vec<&RegroupementDeComparaison> = leurs.keys().chain(nos.keys()).collect();
    codes.sort();
    codes.dedup();
    let somme = |montants: &mut dyn Iterator<Item = Montant>| montants.fold(Montant::ZERO, |a, m| a + m.abs());
    for code in codes {
        if atmp_lu && *code == RegroupementDeComparaison::Cotisation(ATMP) {
            continue;
        }
        let eux: &[&CotisationImprimee] = match leurs.get(code) {
            Some(v) => v,
            None => &[],
        };
        let nous: &[&LigneCotisation] = match nos.get(code) {
            Some(v) => v,
            None => &[],
        };
        let leur_sal = somme(&mut eux.iter().filter_map(|l| l.montant_salarie));
        let leur_pat = somme(&mut eux.iter().filter_map(|l| l.montant_employeur));
        let notre_sal = somme(&mut nous.iter().map(|l| l.montant_sal));
        let notre_pat = somme(&mut nous.iter().map(|l| l.montant_pat));
        let mut libelles: Vec<String> = Vec::new();
        for l in eux.iter().map(|l| l.libelle.clone()).chain(eux.is_empty().then(|| nous.iter().map(|l| l.libelle.clone())).into_iter().flatten()) {
            if !libelles.contains(&l) {
                libelles.push(l);
            }
        }
        let ecart = |mesure| Ecart { cotisation: code.clone(), libelles: libelles.clone(), mesure };
        let (chez_eux, chez_nous) = (leur_sal + leur_pat > Montant::ZERO, notre_sal + notre_pat > Montant::ZERO);
        if chez_nous && !chez_eux {
            ecarts.push(ecart(MesureDeLEcart::AbsenteDuBulletin));
            continue;
        }
        if chez_eux && !chez_nous {
            ecarts.push(ecart(MesureDeLEcart::AbsenteDuCalcul));
            continue;
        }
        if !chez_eux {
            continue;
        }
        // Un centime par ligne, du cote qui en a le plus.
        let tolerance = UN_CENTIME.fois(eux.len().max(nous.len()));
        let montants: [(fn(Option<Coefficient>) -> MesureDeLEcart, Montant, Montant); 2] =
            [(MesureDeLEcart::MontantSalarie, leur_sal, notre_sal), (MesureDeLEcart::MontantEmployeur, leur_pat, notre_pat)];
        for (mesure, leur, notre) in montants {
            if (leur - notre).abs() > tolerance {
                ecarts.push(ecart(mesure(relatif(leur, notre))));
            }
        }
        let ([l], [n]) = (eux, nous) else { continue };
        // Une base en heures (la deduction des heures supplementaires) est masquee au bulletin : elle ne se compare pas.
        if let (Some(b), crate::calcul::BaseDeCotisation::Montant(notre)) = (l.base.and_then(crate::calcul::BaseDeCotisation::montant).filter(|_| !n.masquer_base_et_taux), n.base) {
            if (b - notre).abs() > UN_CENTIME {
                ecarts.push(ecart(MesureDeLEcart::Base(relatif(b, notre))));
            }
        }
        let taux: [(fn(Taux, Taux) -> MesureDeLEcart, Option<Taux>, Option<Taux>); 2] = [
            (|notre, leur| MesureDeLEcart::TauxSalarie { notre, leur }, l.taux_salarie, n.taux_sal),
            (|notre, leur| MesureDeLEcart::TauxEmployeur { notre, leur }, l.taux_employeur, n.taux_pat),
        ];
        for (mesure, leur, notre) in taux {
            // Un taux se compare quand chaque cote en a un : une ligne calculee sans taux (la reduction generale, un
            // montant fixe) n'en a pas, et ce que le bulletin imprime a sa place n'en est pas un ; le montant compare.
            let (Some(leur), Some(notre)) = (leur, notre) else { continue };
            if (leur - notre).abs() > ECART_DE_TAUX_TOLERE {
                ecarts.push(ecart(mesure(notre, leur)));
            }
        }
    }
    // Les totaux du bulletin, sans ce qu'il imprime apres eux : l'ecart relatif seulement.
    let hors = |f: &dyn Fn(&LigneCotisation) -> Montant| -> Montant {
        paie.lignes.iter().filter(|l| hors_total.contains(&cotisation_de(l, &garanties))).fold(Montant::ZERO, |a, l| a + f(l))
    };
    for (cle, notre, leur) in [
        (TotalDuBulletin::CotisationsSalariales, paie.total_sal - hors(&|l| l.montant_sal), p.cotisations_salariales),
        (TotalDuBulletin::CotisationsPatronales, paie.total_pat - hors(&|l| l.montant_pat), p.cotisations_patronales),
        (TotalDuBulletin::NetSocial, paie.net_social, p.net_social),
        (TotalDuBulletin::NetImposable, paie.net_imposable, p.nets.net_imposable),
    ] {
        let Some(leur) = leur else { continue };
        if (leur - notre).abs() > UN_CENTIME {
            let r = relatif(leur, notre);
            let mesure = if cle == TotalDuBulletin::CotisationsPatronales { MesureDeLEcart::MontantEmployeur(r) } else { MesureDeLEcart::MontantSalarie(r) };
            ecarts.push(Ecart { cotisation: RegroupementDeComparaison::Total(cle), libelles: Vec::new(), mesure });
        }
    }
    ecarts
}

/// (leur - notre) / notre, a 4 decimales ; `None` quand le notre est nul.
fn relatif(leur: Montant, notre: Montant) -> Option<Coefficient> {
    (leur - notre).rapport(notre).map(|e| e.a_decimales(4))
}

/// Une raison de non-comparaison venue du calcul : la paie se comparera de nouveau.
fn a_refaire(c: &mut Suivi, raison: Echec) -> Echec {
    c.a_refaire = true;
    raison
}

/// L'empreinte de la paie `mois` de l'engagement importe `imp` : ce qui la reconnait d'une comparaison a l'autre (son
/// salarie, ses bornes, son numero et ce que les pieces en disent, sans la date de l'import).
pub fn empreinte(imp: &Importe, mois: Mois, p: &PaieVersee) -> EmpreinteCourte {
    let brut = imp.objet().carte();
    let paie = PaieVersee { importe_le: None, ..p.clone() };
    let graine = socle::ecriture::json_en_ligne(&(&brut.salarie, &brut.debut, &brut.fin, &brut.numero, mois, paie));
    Empreinte::des_octets(graine.as_bytes()).courte()
}
