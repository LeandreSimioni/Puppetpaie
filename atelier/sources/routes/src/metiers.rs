//! Configuration › Métiers : les metiers que la compagnie coche, ceux de sa convention et ceux qu'elle prend aux autres.

use droit::fiche::Lue as _;
use droit::fiche::Idcc;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::catalogue::{Acte, Affiche, Champ, Choix, Forme, Ecran, Fiche, Place, FORME, MANQUE, OPERATION};
use crate::catalogue::Route;
use crate::enveloppe::{en_json, Charge, Dite, Phrase, Porte, Rendu};
use crate::fiches::{creer_dossier, ecrire, refus};
use crate::imprevus::GesteDeRoute;
use compagnie::avant::sauver_avant;
use droit::contexte::Contexte;
use droit::emplois::{self, FamilleDeMetier, MetierDuDeroulant};
use socle::echec::R;

const METIERS_REND: &str = "emplois (les métiers proposés) et emplois_autres (ceux des autres conventions)";

/// Ou ces routes se voient a l'ecran (une place par module).
const ECRAN: Ecran = Ecran::DessineAvance(Place::Metiers);

/// Les routes de ce module, et leurs fiches.
pub const ROUTES: &[Fiche] = &[
    Fiche {
        route: Route::MetiersCatalogue,
        geste: "Dire les métiers du catalogue, ceux de la convention et les autres.",
        charge: &[],
        exemple: r#"{}"#,
        rend: "metiers (le catalogue, chacun dit retenu ou non)",
        refus: &[OPERATION, FORME],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Lit,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::MetierRetenu,
        geste: "Proposer un métier du catalogue dans les listes, ou ne plus le proposer.",
        charge: &[Champ { nom: "code", libelle: Affiche::new("Métier"), forme: Forme::Texte, sens: "le code du métier au catalogue", avance: false }, Champ { nom: "retenu", libelle: Affiche::new("Le proposer"), forme: Forme::OuiNon, sens: "vrai : le proposer", avance: false }],
        exemple: r#"{"code":"CHE350","retenu":true}"#,
        rend: METIERS_REND,
        refus: &[OPERATION, MANQUE, FORME],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: ECRAN,
    },
    Fiche {
        route: Route::MetierCree,
        geste: "Créer un métier en copiant un métier : son nom, et s'il est cadre.",
        charge: &[COPIE_DE, LIBELLE, CADRE_A_LA_CREATION],
        exemple: r#"{"copie_de":"MAR010","libelle":"Marionnettiste cadre","cadre":"cadre"}"#,
        rend: "code (celui du métier créé), emplois et emplois_autres",
        refus: &[OPERATION, MANQUE, FORME],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: Ecran::Geste(Place::Metiers),
    },
    Fiche {
        route: Route::MetierModifie,
        geste: "Modifier un métier créé ; une case absente ne change pas.",
        charge: &[
            Champ { nom: "code", libelle: Affiche::new("Métier"), forme: Forme::Texte, sens: "le code du métier créé (CIE-1…)", avance: false },
            LIBELLE,
            CADRE,
            Champ { nom: "feminin", libelle: Affiche::new("Au féminin"), forme: Forme::Texte, sens: "son nom au féminin", avance: true },
            Champ { nom: "emploi", libelle: Affiche::new("Emploi déclaré"), forme: Forme::Texte, sens: "le libellé de l'emploi que la DSN déclare (S21.G00.40.006)", avance: true },
            Champ { nom: "pcs_ese", libelle: Affiche::new("PCS-ESE"), forme: Forme::Texte, sens: "le code PCS-ESE, quatre caractères (S21.G00.40.004)", avance: true },
            Champ {
                nom: "code_declare",
                libelle: Affiche::new("Code France Travail"),
                forme: Forme::Texte,
                sens: "le code de la nomenclature des emplois du spectacle qu'il déclare (S21.G00.40.005) ; l'annexe et le statut le suivent",
                avance: true,
            },
            Champ {
                nom: "statut_conventionnel",
                libelle: Affiche::new("Classement conventionnel"),
                forme: Forme::ChoixRendu,
                sens: "le classement que la DSN déclare quand il n'est pas cadre (S21.G00.40.002) : 05, 06 ou 07",
                avance: true,
            },
        ],
        exemple: r#"{"code":"CIE-1","feminin":"Marionnettiste principale"}"#,
        rend: "emplois et emplois_autres",
        refus: &[OPERATION, MANQUE, FORME],
        pendant_la_demande: crate::catalogue::PendantLaDemande::Refusee,
        acte: Acte::Ecrit,
        phrase: 0,
        ecran: Ecran::GesteAvance(Place::Metiers),
    },
];

/// Le metier qu'on copie (`metier-cree`).
const COPIE_DE: Champ = Champ { nom: "copie_de", libelle: Affiche::new("Copie de"), forme: Forme::ChoixRendu, sens: "le code du métier copié, du catalogue ou déjà créé (MAR010)", avance: false };
/// Le nom d'un metier cree.
const LIBELLE: Champ = Champ { nom: "libelle", libelle: Affiche::new("Nom"), forme: Forme::Texte, sens: "son nom, tel que la liste et le contrat le disent", avance: false };
/// Cadre ou non, a la creation : la charge le dit (`emplois::Encadrement`).
const CADRE_A_LA_CREATION: Champ = Champ {
    nom: "cadre",
    libelle: Affiche::new("Cadre"),
    forme: Forme::Choix(&[Choix { code: "cadre", libelle: Affiche::new("Cadre") }, Choix { code: "non_cadre", libelle: Affiche::new("Non cadre") }]),
    sens: "cadre, ou non_cadre",
    avance: false,
};
/// Cadre ou non, pour un metier deja cree.
const CADRE: Champ = Champ { nom: "cadre", libelle: Affiche::new("Cadre"), forme: Forme::OuiNon, sens: "vrai : cadre ; faux : non cadre ; absent : inchangé", avance: false };

/// LES CLASSEMENTS QU'UN METIER CREE OFFRE (S21.G00.40.002) : ceux d'un metier non cadre, les seuls que la DSN lit d'un
/// metier (`StatutConventionnel::NON_CADRES`, `emplois::Metier::statut_non_cadre` ; un metier cadre se declare 04),
/// chacun sous le libelle que la fiche du salarie lui donne (`champs-base.json`, `statut_conventionnel`).
fn classements_offerts() -> R<Vec<crate::ouverture::Choix>> {
    let cle: droit::fiche::CaseDeFiche = droit::fiche::CaseSalarie::StatutConventionnel.into();
    let description = declarations::champs::description(droit::fiche::NomDeFiche::Salarie)?;
    let libelles = description.iter().find(|c| c.cle == cle).and_then(|c| c.libelles);
    droit::fiche::StatutConventionnel::NON_CADRES
        .iter()
        .map(|s| {
            let libelle = libelles.and_then(|l| l.de(s.as_str())).ok_or_else(|| socle::echec::livree(&declarations::champs::BASE, &format!("statut_conventionnel.libelles.{}", s)))?;
            Ok(crate::ouverture::Choix::de(s, libelle))
        })
        .collect()
}

/// Une route de ce module, branchee sur l'enveloppe.
pub fn route(porte: &Porte, charge: &Charge) -> R<Rendu> {
    match porte.route {
        Route::MetiersCatalogue => {
            let super::SansCharge {} = charge.lire()?;
            en_json(metiers_catalogue(porte.ctx)?)
        }
        Route::MetierRetenu => en_json(metier_retenu(porte.ctx, charge.lire()?)?),
        Route::MetierCree => en_json(metier_cree(porte.ctx, charge.lire()?)?),
        Route::MetierModifie => en_json(metier_modifie(porte.ctx, charge.lire()?)?),
        autre => Err(super::enveloppe::route_inconnue(autre.nom())),
    }
}

impl Dite for MetierCreeRendu {
    fn phrase(&self) -> &Phrase {
        &self.emplois.phrase
    }
}

impl Dite for MetiersDuCatalogue {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

impl Dite for EmploisRendus {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

/// `config/metiers-ajoutes.json` : les metiers que la compagnie a coches ou decoches dans Configuration > Metiers.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MetiersAjoutes {
    quoi: String,
    /// Pris a une autre convention collective.
    pub(crate) codes: Vec<emplois::CodeEmploi>,
    /// Decoches dans la sienne.
    pub(crate) retires: Vec<emplois::CodeEmploi>,
}

const QUOI_METIERS: &str = "Les metiers que cette compagnie a coches ou decoches dans Configuration > Metiers : `codes`, pris a une autre convention collective ; `retires`, decoches dans la sienne. Preference d'affichage : aucun calcul ne change. Le catalogue est livre avec le logiciel (donnees/emplois-base.json).";

/// Les metiers que la compagnie a ajoutes ou retires, sous sa racine.
pub(crate) const METIERS_AJOUTES: socle::fichiers::CheminRelatif = socle::fichiers::CheminRelatif::du_moteur("config/metiers-ajoutes.json");

impl MetiersAjoutes {
    /// Aucun metier coche ni decoche : ceux de la convention, et eux seuls.
    fn aucun() -> MetiersAjoutes {
        MetiersAjoutes { quoi: QUOI_METIERS.into(), codes: Vec::new(), retires: Vec::new() }
    }

    /// LA SEULE LECTURE de `config/metiers-ajoutes.json` ; rien de pose s'il n'existe pas.
    pub(crate) fn lire(racine: &Path) -> R<MetiersAjoutes> {
        let p = METIERS_AJOUTES.dans(racine);
        if !p.exists() {
            return Ok(MetiersAjoutes::aucun());
        }
        socle::forme::lire_fichier(&p, "Les métiers retenus par la compagnie ne se lisent pas : effacez leur fichier, puis cochez-les de nouveau dans Configuration › Métiers.")
            .map_err(|m| m.reclasser(socle::echec::Classe::Illisible))
    }

    fn ecrire(&self, ctx: &Contexte) -> R<()> {
        let p = METIERS_AJOUTES.dans(&ctx.racine);
        if let Some(parent) = p.parent() {
            creer_dossier(parent)?;
        }
        sauver_avant(&p)?;
        ecrire(&p, &MetiersAjoutes { quoi: QUOI_METIERS.into(), codes: self.codes.clone(), retires: self.retires.clone() })
    }

    /// Ce metier est-il dans le deroulant de cette compagnie ? Ceux de sa convention le sont d'office ; elle en ajoute
    /// d'une autre, elle en retire des siens (Configuration > Metiers).
    fn visible(&self, m: &MetierDuDeroulant, idcc: Option<&Idcc>) -> bool {
        !self.retires.contains(&m.code) && (de_la_convention(m, idcc) || self.codes.contains(&m.code))
    }
}

/// Le metier est-il de la convention `idcc` ? Sans IDCC (une compagnie qui s'installe), ou sans convention dite, rien ne
/// se cache.
fn de_la_convention(m: &MetierDuDeroulant, idcc: Option<&Idcc>) -> bool {
    m.conventions.is_empty() || idcc.is_none_or(|i| m.conventions.iter().any(|c| c == i))
}

/// Les conventions d'un metier en mots : « Spectacle vivant privé (3090) » quand `idcc` est dit.
/// Une convention que le catalogue cite sans la nommer est un defaut de la donnee livree.
fn conventions_dites(m: &MetierDuDeroulant, noms: &BTreeMap<Idcc, String>, avec_l_idcc: bool) -> R<String> {
    let dites = m
        .conventions
        .iter()
        .map(|c| match noms.get(c) {
            Some(nom) if avec_l_idcc => Ok(format!("{} ({})", nom, c)),
            Some(nom) => Ok(nom.clone()),
            None => Err(socle::echec::livree(&droit::emplois::BASE, &format!("conventions.{}", c)).reclasser(socle::echec::Classe::Illisible)),
        })
        .collect::<R<Vec<_>>>()?;
    Ok(dites.join(", "))
}

/// Les familles, et l'ordre ou les montrer : les artistes en tete.
fn rang_des_familles(familles: &[FamilleDeMetier]) -> HashMap<FamilleDeMetier, usize> {
    familles.iter().enumerate().map(|(i, f)| (*f, i)).collect()
}

/// CE QUE LA COMPAGNIE A DIT DE SES METIERS, lu pour une lecture : ceux qu'elle a coches ou decoches (`MetiersAjoutes`)
/// et ceux qu'elle a crees (`emplois::metiers_crees`). Regarder n'est jamais refuse (« le logiciel est doux ») : un de
/// ces fichiers qui ne se lit pas (`Classe::Illisible`) ne ferme ni la route d'etat, ni la liste des metiers d'une carte ;
/// la lecture se fait sans lui, et sa faute se dit (`fautes`, dans la phrase de l'onglet Metiers). Une carte qui nomme un
/// metier cree garde sa faute sur elle (`CodeEmploi::metier`), et une ecriture de ces fichiers les lit en strict, et
/// refuse : rien ne s'ecrit par-dessus un fichier qu'on n'a pas lu.
struct DitsDeLaCompagnie {
    retenus: MetiersAjoutes,
    crees: Vec<emplois::MetierCree>,
    fautes: Vec<String>,
}

impl DitsDeLaCompagnie {
    fn lire(racine: &Path) -> R<DitsDeLaCompagnie> {
        let mut fautes = Vec::new();
        let retenus = match lu_ou_dit(MetiersAjoutes::lire(racine), &mut fautes)? {
            Some(r) => r,
            None => MetiersAjoutes::aucun(),
        };
        let crees = match lu_ou_dit(emplois::metiers_crees(racine), &mut fautes)? {
            Some(c) => c,
            None => Vec::new(),
        };
        Ok(DitsDeLaCompagnie { retenus, crees, fautes })
    }
}

/// Le fichier lu, ou, s'il ne se lit pas, rien et sa faute notee ; une autre panne remonte.
fn lu_ou_dit<T>(lu: R<T>, fautes: &mut Vec<String>) -> R<Option<T>> {
    match lu {
        Ok(t) => Ok(Some(t)),
        Err(e) if e.classe == Some(socle::echec::Classe::Illisible) => {
            fautes.push(e.phrase);
            Ok(None)
        }
        Err(e) => Err(e),
    }
}

/// `metiers` : le catalogue des metiers que le deroulant de cette compagnie montre, groupe par famille (les artistes en
/// tete), chaque convention voyant ses metiers et seulement eux.
pub(crate) fn metiers(ctx: &Contexte) -> R<Vec<MetierDuDeroulant>> {
    let ordre = rang_des_familles(&emplois::familles()?);
    let DitsDeLaCompagnie { retenus, crees, .. } = DitsDeLaCompagnie::lire(&ctx.racine)?;
    let idcc = ctx.entreprise.idcc.dite();
    let mut liste: Vec<MetierDuDeroulant> = emplois::catalogue(idcc, &crees)?.into_iter().filter(|m| retenus.visible(m, idcc)).collect();
    // Chaque famille du catalogue a son rang (`emplois::familles`).
    liste.sort_by_cached_key(|m| (ordre.get(&m.famille).copied(), m.libelle.to_lowercase()));
    Ok(liste)
}

/// LES METIERS QUE LA FICHE D'UN SALARIE CHOISIT : ceux du deroulant `emplois`, sans ceux que la compagnie a crees (la
/// route `emploi` les refuse, `mois::emploi` : un metier cree se choisit sur la carte d'un engagement).
pub(crate) fn metiers_d_une_fiche(emplois: &[MetierDuDeroulant]) -> Vec<MetierDuDeroulant> {
    emplois.iter().filter(|m| m.code.numero_cree().is_none()).cloned().collect()
}

/// Un metier d'une autre convention, que la compagnie peut ajouter a son deroulant : `de` dit lesquelles.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct MetierAutre {
    #[serde(flatten)]
    metier: MetierDuDeroulant,
    de: String,
}

/// `metiers_autres_conventions` : les metiers des AUTRES conventions, ceux que la compagnie peut ajouter a son deroulant.
pub(crate) fn metiers_autres_conventions(ctx: &Contexte) -> R<Vec<MetierAutre>> {
    let noms = emplois::noms_des_conventions()?;
    let DitsDeLaCompagnie { retenus, crees, .. } = DitsDeLaCompagnie::lire(&ctx.racine)?;
    let idcc = ctx.entreprise.idcc.dite();
    let mut out: Vec<MetierAutre> =
        emplois::catalogue(idcc, &crees)?.into_iter().filter(|m| m.copie_de.is_none() && !retenus.visible(m, idcc)).map(|m| Ok(MetierAutre { de: conventions_dites(&m, &noms, true)?, metier: m })).collect::<R<_>>()?;
    out.sort_by(|a, b| (&a.de, a.metier.libelle.to_lowercase()).cmp(&(&b.de, b.metier.libelle.to_lowercase())));
    Ok(out)
}

/// Un metier de l'onglet Metiers : s'il est retenu, s'il est de la convention de la compagnie, et sinon de laquelle ; un
/// metier que la compagnie a cree porte le geste qui le modifie (`metier-modifie`).
#[derive(Debug, Serialize)]
struct MetierDeLOnglet {
    #[serde(flatten)]
    metier: MetierDuDeroulant,
    retenu: bool,
    de_la_convention: bool,
    /// Les conventions d'un metier qui n'est pas de celle de la compagnie.
    de: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    modifier: Option<GesteDuMetier<MetierDesigne>>,
}

/// UN GESTE DE L'ONGLET METIERS (`imprevus::GesteDeRoute`) : creer un metier, en modifier un cree.
type GesteDuMetier<C> = GesteDeRoute<C, ValeursDuMetier, ChoixDuMetier>;

/// Ce qui designe un metier cree a `metier-modifie` : son code.
#[derive(Debug, Clone, Serialize)]
struct MetierDesigne {
    code: emplois::CodeEmploi,
}

/// La charge du geste qui cree un metier : rien ne le designe, ses cases disent tout.
#[derive(Debug, Clone, Serialize)]
struct AucunMetier {}

/// Ce que les cases du geste qui modifie un metier cree montrent d'abord : ce qu'il dit.
#[derive(Debug, Clone, Default, Serialize)]
struct ValeursDuMetier {
    #[serde(skip_serializing_if = "Option::is_none")]
    libelle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cadre: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    feminin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    emploi: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pcs_ese: Option<droit::fiche::CodePcsEse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    code_declare: Option<emplois::CodeEmploi>,
    #[serde(skip_serializing_if = "Option::is_none")]
    statut_conventionnel: Option<droit::fiche::StatutConventionnel>,
}

/// Ce qu'offrent les cases d'un geste qui se choisissent dans les donnees : le metier a copier, sous sa famille ; le
/// classement d'un metier cree (`classements_offerts`).
#[derive(Debug, Clone, Default, Serialize)]
struct ChoixDuMetier {
    #[serde(skip_serializing_if = "Option::is_none")]
    copie_de: Option<Vec<crate::ouverture::Choix>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    statut_conventionnel: Option<Vec<crate::ouverture::Choix>>,
}

const CREER: Affiche = Affiche::new("Créer un métier");
const MODIFIER: Affiche = Affiche::new("Modifier");

/// Ce que rend `metiers-catalogue` : le catalogue et les metiers crees, et le geste qui en cree un.
#[derive(Debug, Serialize)]
struct MetiersDuCatalogue {
    phrase: Phrase,
    metiers: Vec<MetierDeLOnglet>,
    familles: Vec<FamilleDeMetier>,
    gestes: Vec<GesteDuMetier<AucunMetier>>,
}

/// L'onglet Metiers : tout le catalogue et les metiers crees, chaque metier coche ou non. Creer un metier demande d'emblee
/// le metier copie, son nom et s'il est cadre ; le reste se modifie ensuite, sous « Avancé » (Leandre, 20/11/2026 :
/// « Simple pour les simples, compliqué pour ceux qui veulent »).
fn metiers_catalogue(ctx: &Contexte) -> R<MetiersDuCatalogue> {
    let familles = emplois::familles()?;
    let ordre = rang_des_familles(&familles);
    let noms = emplois::noms_des_conventions()?;
    let DitsDeLaCompagnie { retenus, crees, fautes } = DitsDeLaCompagnie::lire(&ctx.racine)?;
    let classements = classements_offerts()?;
    let idcc = ctx.entreprise.idcc.dite();
    let mut metiers: Vec<MetierDeLOnglet> = emplois::catalogue(idcc, &crees)?
        .into_iter()
        .map(|m| {
            let conv = de_la_convention(&m, idcc);
            let de = if conv { None } else { Some(conventions_dites(&m, &noms, false)?) };
            let modifier = crees.iter().find(|c| c.code == m.code).map(|c| geste_qui_modifie(c, &classements));
            Ok(MetierDeLOnglet { retenu: retenus.visible(&m, idcc), de_la_convention: conv, de, modifier, metier: m })
        })
        .collect::<R<_>>()?;
    metiers.sort_by_cached_key(|m| (ordre.get(&m.metier.famille).copied(), m.metier.libelle.to_lowercase()));
    let copiables = metiers.iter().map(|m| crate::ouverture::Choix::de(&m.metier.code, &m.metier.libelle).sous(Some(m.metier.famille.to_string()))).collect();
    let creer = GesteDeRoute::avec_cases(Route::MetierCree, AucunMetier {}, CREER, vec![COPIE_DE.nom, LIBELLE.nom, CADRE_A_LA_CREATION.nom], ValeursDuMetier::default(), ChoixDuMetier { copie_de: Some(copiables), statut_conventionnel: None });
    let fautes: String = fautes.iter().map(|f| format!(" {}", f)).collect();
    let phrase = socle::phrase!(
        "Métiers : {}, dont {} créés par la compagnie. Ceux qui sont cochés se proposent dans la fiche d'un salarié et dans un engagement ; ceux de votre convention et ceux que vous créez le sont d'office. Un métier qui manque se crée en en copiant un.{}",
        metiers.len(),
        crees.len(),
        fautes
    );
    Ok(MetiersDuCatalogue { phrase, metiers, familles, gestes: vec![creer] })
}

/// Le geste qui modifie le metier cree `m`, ses cases montrant ce qu'il dit, son classement parmi `classements`
/// (`classements_offerts`).
fn geste_qui_modifie(m: &emplois::MetierCree, classements: &[crate::ouverture::Choix]) -> GesteDuMetier<MetierDesigne> {
    let f = &m.fiche;
    let valeurs = ValeursDuMetier {
        libelle: Some(f.libelle.clone()),
        cadre: Some(f.cadre_par_defaut),
        feminin: Some(f.feminin.clone()),
        emploi: Some(f.emploi.clone()),
        pcs_ese: Some(f.pcs_ese.clone()),
        code_declare: Some(m.code_declare.clone()),
        // Un classement de cadre (la copie d'un metier cadre) n'est pas de ceux qui s'offrent : la case se montre vide.
        statut_conventionnel: Some(f.statut_conventionnel).filter(|s| !s.cadre()),
    };
    let cases = vec!["libelle", "cadre", "feminin", "emploi", "pcs_ese", "code_declare", "statut_conventionnel"];
    let choix = ChoixDuMetier { copie_de: None, statut_conventionnel: Some(classements.to_vec()) };
    GesteDeRoute::avec_cases(Route::MetierModifie, MetierDesigne { code: m.code.clone() }, MODIFIER, cases, valeurs, choix)
}

// ---------------------------------------------------------------- les metiers

/// La charge de `metier-retenu`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MetierARetenir {
    code: emplois::CodeEmploi,
    retenu: bool,
}

/// Ce que rend `metier-retenu` : les metiers proposes et ceux des autres conventions.
#[derive(Debug, Serialize)]
struct EmploisRendus {
    phrase: Phrase,
    emplois: Vec<MetierDuDeroulant>,
    emplois_autres: Vec<MetierAutre>,
}

impl EmploisRendus {
    /// La phrase, et les metiers proposes tels qu'ils sont maintenant.
    fn de(ctx: &Contexte, phrase: Phrase, emplois: Vec<MetierDuDeroulant>) -> R<EmploisRendus> {
        Ok(EmploisRendus { phrase, emplois, emplois_autres: metiers_autres_conventions(ctx)? })
    }
}

fn metier_retenu(ctx: &Contexte, c: MetierARetenir) -> R<EmploisRendus> {
    let _verrou = crate::mois::une_ecriture_a_la_fois();
    let idcc = ctx.entreprise.idcc.dite();
    let catalogue = emplois::catalogue(idcc, &emplois::metiers_crees(&ctx.racine)?)?;
    let Some(m) = catalogue.iter().find(|m| m.code == c.code) else {
        return Err(refus(format!("Metier hors catalogue : {}.", c.code)));
    };
    let conv = de_la_convention(m, idcc);
    let mut retenus = MetiersAjoutes::lire(&ctx.racine)?;
    retenus.codes.retain(|x| *x != c.code);
    retenus.retires.retain(|x| *x != c.code);
    if c.retenu && !conv {
        retenus.codes.push(c.code.clone());
    }
    if !c.retenu && conv {
        retenus.retires.push(c.code.clone());
    }
    retenus.ecrire(ctx)?;
    let emplois = metiers(ctx)?;
    EmploisRendus::de(ctx, socle::phrase!("Métiers retenus par la compagnie : {}.", emplois.len()), emplois)
}

// ---------------------------------------------------------------- les metiers crees

/// La charge de `metier-cree` : le metier copie, le nom de la copie, et s'il est cadre (la charge le dit : rien ne se
/// suppose).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MetierACreer {
    copie_de: emplois::CodeEmploi,
    libelle: socle::forme::TexteDit,
    cadre: emplois::Encadrement,
}

/// Ce que rend `metier-cree` : le code du metier cree, et les metiers proposes.
#[derive(Debug, Serialize)]
struct MetierCreeRendu {
    code: emplois::CodeEmploi,
    #[serde(flatten)]
    emplois: EmploisRendus,
}

/// UN METIER SE CREE EN COPIANT UN METIER (Leandre, 20/11/2026 : « On copie et on modifie ») : celui du catalogue, ou un
/// que la compagnie a deja cree ; il prend le numero qui suit le plus haut des metiers crees (`CodeEmploi::cree`), et se
/// propose aussitot dans les cartes. Le metier copie se lit par le seul chemin des metiers (`CodeEmploi::metier`).
fn metier_cree(ctx: &Contexte, c: MetierACreer) -> R<MetierCreeRendu> {
    let _verrou = crate::mois::une_ecriture_a_la_fois();
    let de = c.copie_de.metier(&ctx.racine)?;
    let mut crees = emplois::metiers_crees(&ctx.racine)?;
    nom_libre(ctx, &crees, None, &c.libelle)?;
    let numero = match crees.iter().filter_map(|m| m.code.numero_cree()).max() {
        Some(n) => n + 1,
        None => 1,
    };
    let cadre = c.cadre == emplois::Encadrement::Cadre;
    let neuf = emplois::MetierCree::copie(emplois::CodeEmploi::cree(numero), &de, c.libelle.to_string(), cadre)?;
    neuf.classement_qui_se_declare()?;
    let code = neuf.code.clone();
    crees.push(neuf);
    ecrire_les_crees(ctx, crees)?;
    let phrase = socle::phrase!(
        "Métier créé : {} ({}), copie de {}, {}. Il se propose dans les cartes ; le reste se modifie sous « Avancé ».",
        c.libelle,
        code,
        de.libelle(),
        if cadre { "cadre" } else { "non cadre" }
    );
    let emplois = metiers(ctx)?;
    Ok(MetierCreeRendu { code, emplois: EmploisRendus::de(ctx, phrase, emplois)? })
}

/// La charge de `metier-modifie` : le metier cree, et ce qui change ; une case absente ne change pas. Chaque valeur se lit
/// dans sa forme, comme celle du catalogue : la PCS-ESE (`CodePcsEse`), le code de la nomenclature (`CodeEmploi`,
/// `MetierCree::declarer`), le classement (`StatutConventionnel`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MetierAModifier {
    code: emplois::CodeEmploi,
    libelle: Option<socle::forme::TexteDit>,
    cadre: Option<bool>,
    feminin: Option<socle::forme::TexteDit>,
    emploi: Option<socle::forme::TexteDit>,
    pcs_ese: Option<droit::fiche::CodePcsEse>,
    code_declare: Option<emplois::CodeEmploi>,
    statut_conventionnel: Option<droit::fiche::StatutConventionnel>,
}

/// UN NOM QUE LA LISTE D'UNE CARTE NE MONTRE QU'UNE FOIS : le nom `libelle` d'un metier cree (`sauf` : celui qu'on
/// renomme) n'est ni celui d'un metier du catalogue, ni celui d'un autre metier cree, sans egard a la casse ; sinon la
/// liste des metiers montrerait deux fois le meme, et le refus dit le nom deja pris.
fn nom_libre(ctx: &Contexte, crees: &[emplois::MetierCree], sauf: Option<&emplois::CodeEmploi>, libelle: &socle::forme::TexteDit) -> R<()> {
    let cherche = libelle.to_string().trim().to_lowercase();
    let idcc = ctx.entreprise.idcc.dite();
    match emplois::catalogue(idcc, crees)?.into_iter().find(|m| Some(&m.code) != sauf && m.libelle.trim().to_lowercase() == cherche) {
        Some(pris) => Err(refus(format!("Le nom « {} » est déjà celui du métier {} ({}) : choisissez-en un autre. Rien n'est écrit.", libelle, pris.libelle, pris.code))),
        None => Ok(()),
    }
}

/// UN METIER CREE SE MODIFIE, case par case ; un metier du catalogue ne se modifie pas : il se copie.
fn metier_modifie(ctx: &Contexte, c: MetierAModifier) -> R<EmploisRendus> {
    let _verrou = crate::mois::une_ecriture_a_la_fois();
    let mut crees = emplois::metiers_crees(&ctx.racine)?;
    if let Some(v) = &c.libelle {
        nom_libre(ctx, &crees, Some(&c.code), v)?;
    }
    let Some(m) = crees.iter_mut().find(|m| m.code == c.code) else {
        return Err(refus(match c.code.numero_cree() {
            Some(_) => format!("Le métier {} n'est pas dans les métiers de la compagnie : créez-le d'abord dans Configuration › Métiers.", c.code),
            None => format!("Le métier {} est celui du catalogue : il ne se modifie pas ; créez-en une copie (« {} »), puis modifiez-la.", c.code, CREER.texte()),
        }));
    };
    if let Some(v) = c.libelle {
        m.fiche.libelle = v.to_string();
    }
    if let Some(v) = c.feminin {
        m.fiche.feminin = v.to_string();
    }
    if let Some(v) = c.emploi {
        m.fiche.emploi = v.to_string();
    }
    if let Some(v) = c.pcs_ese {
        m.fiche.pcs_ese = v;
    }
    if let Some(v) = c.statut_conventionnel {
        m.fiche.statut_conventionnel = v;
    }
    if let Some(v) = c.cadre {
        m.tenir_pour_cadre(v);
    }
    if let Some(v) = c.code_declare {
        m.declarer(v)?;
    }
    m.classement_qui_se_declare()?;
    let phrase =socle::phrase!("Métier modifié : {} ({}), {}.", m.fiche.libelle, m.code, if m.fiche.cadre_par_defaut { "cadre" } else { "non cadre" });
    ecrire_les_crees(ctx, crees)?;
    let emplois = metiers(ctx)?;
    EmploisRendus::de(ctx, phrase, emplois)
}

/// LA SEULE ECRITURE de `config/metiers-crees.json` (`emplois::METIERS_CREES`), la version d'avant gardee.
fn ecrire_les_crees(ctx: &Contexte, metiers: Vec<emplois::MetierCree>) -> R<()> {
    let p = emplois::METIERS_CREES.dans(&ctx.racine);
    if let Some(parent) = p.parent() {
        creer_dossier(parent)?;
    }
    sauver_avant(&p)?;
    ecrire(&p, &emplois::MetiersCrees { quoi: emplois::QUOI_METIERS_CREES.into(), metiers })
}
