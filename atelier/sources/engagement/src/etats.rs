//! LES TROIS ETATS D'UN ENGAGEMENT, pour un mois :
//!
//! - EN SAISIE (`EnSaisie`) : il se calcule a mesure pour l'ecran ; rien ne peut partir de lui ;
//! - VALIDE (`Valide`) : « Valider l'engagement » a ecrit, dans l'engagement, `valide` : la date, le numero de sa
//!   version et sa saisie telle qu'elle etait ; et, dans le fichier de chaque mois qu'il couvre (`paies`), sa paie
//!   calculee, avec l'empreinte de ce dont son calcul depend (`empreinte`). Tout ce qui part part d'elles
//!   (`validees_du_mois`, `Valide::paie`) ;
//! - DOCUMENTS PARTIS (`Partis`) : les documents du mois (bulletin, certificat Conges Spectacles, AER) sont partis au
//!   salarie ; ce mois ne se recalcule plus de lui-meme, sauf par une decision de l'editeur. La couche des envois le dit
//!   (`departs`).
//!
//! Trois types, et seul un engagement valide (ou parti, qui l'est) donne une paie a qui fabrique un document : un
//! engagement en saisie n'a pas de `paie`.
//!
//! LES VERSIONS DATEES. Tant que rien n'est parti d'un engagement, il se modifie librement, sans historique. Des qu'un
//! envoi est parti sur sa version (`departs`), une revalidation qui le change garde dans l'engagement la version d'avant
//! (`versions` : sa saisie telle qu'elle etait, la date de sa validation et celle ou elle a ete remplacee ; ses paies se
//! gardent sous son numero dans le fichier de leur mois, `paies::de_la_version`), et le present prend le numero suivant. Une
//! revalidation qui ne change rien ne fait pas de version. Une
//! version passee ne se reecrit jamais. Ce qui est a refaire (`a_refaire`) se tire de la comparaison entre le present et
//! la version sur laquelle chaque envoi est parti.
//!
//! LE RECALCUL DE LUI-MEME (`rafraichir`) : a chaque lecture d'un mois (`du_mois`, par ou passe tout ce qui lit une
//! paie validee : l'ecran, les exports, les declarations, les documents, les envois), la paie de chaque engagement
//! valide se compare a son empreinte. Si elle ne tient plus, la paie se recalcule et se reecrit ; si un chiffre a change,
//! elle dit pourquoi (`raison`) : « taux de PAS reçu » (seul le prelevement change), « règle du JJ/MM/AAAA » (une regle
//! datee s'applique : du bareme, ou une valeur datee de la compagnie, son taux AT/MP de l'annee), « calcul corrigé » (le code qui calcule a change), « un autre engagement du salarié a changé » (un
//! engagement de son salarie dans le mois), « saisie du mois modifiée » (le taux du cachet, la date de paiement), « un arrêt
//! ou une absence a changé » (un imprevu du salarie que le calcul lit), « la paie de
//! <mois> a changé » (une paie validee d'un autre mois que son calcul lit : conges, cumuls), « configuration de la
//! compagnie modifiée » (une fiche corrigee sans date nouvelle, les contrats d'organismes). Un changement qui ne change aucun chiffre ne laisse aucune trace.
//! Un mois dont les documents sont partis ne se recalcule que par une decision de l'editeur (« calcul corrigé », une
//! regle datee) : le recalcul fait alors une version datee, comme une modification, et la paie refaite rectifie le
//! bulletin parti.
//!
//! LE PRESENT SEUL. Un mois est EN COURS ou FINI (`fini` : ses documents et sa DSN sont partis). Seul un mois en cours se
//! recalcule a sa lecture ; un mois fini se lit tel qu'il est ecrit, et une decision de l'editeur ne l'atteint qu'au
//! passage de la version qui la porte (`corriger_les_mois_finis`). Aucune paie ne s'ecrit pour un mois d'apres le present
//! (`mois_present`). Ce qu'une paie lit d'un autre mois (le compteur de conges que la paie d'avant tient pour elle) se
//! lit tel qu'il est ecrit ; une validation qui refait un mois remet a jour le report des paies parties qui le suivent
//! (`reporter`).
//!
//! MODIFIER UN ENGAGEMENT VALIDE se fait d'un geste, dans une seule ecriture de la route : il repasse en saisie
//! (`modifier`), sa validation mise de cote (`precedente`), prend sa saisie nouvelle (`stock::poser_la_saisie`) et se
//! revalide aussitot ; la validation reprend telles quelles les paies des mois dont les documents sont partis tant que la
//! saisie de leur mois n'a pas change, et fait une version si quelque chose est parti sur elle. Il ne reste jamais
//! rouvert.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use droit::contexte::IdSalarie;

use crate::calcul::Paie;
use crate::departs::{Depart, Departs, Envoi};
use crate::paie_ecrite::{FormeDePaie, PaieEcrite, Recalcul};
use crate::Avis;
use crate::saisie::{Carte, Champ, RangDansLeMois};
use crate::stock::{self, Objet, VersionDeLEngagement};
use crate::{empreinte, relire};
use droit::contexte::{CleLue, Contexte};
use socle::date::Jour;
use socle::fichiers::{LectureDatee, ValeurLue};
use socle::mois::{Graphie, Mois};
use socle::paris::{InstantOuJour, Moment};
use socle::grandeurs::Montant;
use socle::echec::{refus, Echec, R};

/// POURQUOI UNE PAIE SE RECALCULE (`raison`). Elle s'ecrit par sa phrase (`Display`), que la paie ecrite garde
/// (`Recalcul::raison`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RaisonDuRecalcul {
    /// Le taux de prelevement a la source a change.
    TauxDePas,
    /// Une regle datee nouvelle s'applique : une regle du code, ou une valeur datee de la configuration de la compagnie.
    RegleDu(Jour),
    /// Le code qui calcule a change (le moteur, ou les baremes et catalogues qu'il livre).
    CalculCorrige,
    /// Un autre engagement valide du meme salarie dans le mois, que le calcul lit, a change (la deduction forfaitaire
    /// specifique deja retenue, `calcul::dfs_deja_deduite`).
    AutreEngagement,
    /// Ce que la saisie du mois pose pour tous ses engagements (taux du cachet, date de paiement) a change.
    SaisieDuMois,
    /// Un arret ou une suspension du salarie (le magasin des imprevus, `imprevus::DOSSIER`), que le calcul lit, a change.
    Imprevu,
    /// La paie d'un autre mois que le calcul lit (conges, cumuls, salaire de reference), validee ou reprise d'un ancien
    /// logiciel, a change.
    PaieDUnAutreMois(Mois),
    /// La configuration de la compagnie a change sans valeur datee nouvelle (une fiche corrigee, les contrats
    /// d'organismes).
    Configuration,
}

impl RaisonDuRecalcul {
    /// Une decision de l'editeur : le code qui calcule, ou une regle datee.
    fn decision_de_l_editeur(self) -> bool {
        matches!(self, RaisonDuRecalcul::CalculCorrige | RaisonDuRecalcul::RegleDu(_))
    }
}

impl std::fmt::Display for RaisonDuRecalcul {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RaisonDuRecalcul::TauxDePas => f.write_str("taux de PAS reçu"),
            RaisonDuRecalcul::RegleDu(jour) => write!(f, "règle du {}", jour.jj_mm_aaaa()),
            RaisonDuRecalcul::CalculCorrige => f.write_str("calcul corrigé"),
            RaisonDuRecalcul::AutreEngagement => f.write_str("un autre engagement du salarié a changé"),
            RaisonDuRecalcul::SaisieDuMois => f.write_str("saisie du mois modifiée"),
            RaisonDuRecalcul::Imprevu => f.write_str("un arrêt ou une absence a changé"),
            RaisonDuRecalcul::PaieDUnAutreMois(m) => write!(f, "la paie {} a changé", socle::texte::de(&m.libelle(socle::mois::Graphie::Accentuee))),
            RaisonDuRecalcul::Configuration => f.write_str("configuration de la compagnie modifiée"),
        }
    }
}


// ------------------------------------------------------------------ ce qui est parti

/// Ce qui est parti de l'engagement `o` (`departs`), d'apres la cle de son salarie et son numero.

pub(crate) fn ses_departs<'a>(o: &Objet, departs: &'a Departs) -> &'a [Depart] {
    match (&o.carte().salarie, &o.carte().numero) {
        (Some(c), Some(n)) => departs.de(c, n),
        _ => &[],
    }
}

/// LE MOIS DU PRESENT : celui de l'horloge (`Contexte::aujourdhui` : la date du geste, la veille ou le jour). Aucune paie
/// ne s'ecrit pour un mois d'apres : elle se calcule quand il devient le present, avec les regles qui le regissent.
pub fn mois_present(ctx: &Contexte) -> Mois {
    ctx.aujourdhui.mois_civil()
}

/// UN MOIS FINI, pour l'engagement dont `siens` est ce qui est parti : ses documents de `mois` sont partis au salarie et
/// une DSN partie l'a declare pour ce mois (une DSN refusee au depot ou au bilan n'est pas partie). Sa paie se lit telle
/// qu'elle est ecrite ; seule une decision de l'editeur la refait, une fois, au passage de la version qui la porte
/// (`corriger_les_mois_finis`). Un mois qui n'est pas fini est EN COURS : il se recalcule de lui-meme a sa lecture.
pub fn fini(siens: &[Depart], mois: Mois) -> bool {
    documents_partis(siens, mois).is_some() && siens.iter().any(|d| d.envoi == Envoi::Dsn && d.mois == Some(mois))
}

/// LE PASSAGE D'UNE VERSION SUR LES MOIS FINIS : chaque mois qui a des paies, du plus ancien au plus recent, se relit
/// une fois comme un mois dont les documents sont partis (`rafraichir`) : une decision de l'editeur (le code qui calcule,
/// une regle datee) refait sa paie, en rectificatif, avec une version ; le reste ne bouge pas. La couche du dessus
/// l'appelle une fois par version installee. Rend les mois dont une paie s'est refaite.
pub fn corriger_les_mois_finis(ctx: &Contexte, departs: &Departs) -> R<Vec<Mois>> {
    let mut refaits = Vec::new();
    for mois in crate::paies::mois_payes(&ctx.racine)? {
        let avant: Vec<Option<Arc<PaieEcrite>>> = tels_qu_ecrits(ctx, mois, departs)?.iter().map(|e| e.paie_ecrite(mois)).collect::<R<_>>()?;
        let apres: Vec<Option<Arc<PaieEcrite>>> = lu_et_rafraichi(ctx, mois, departs, true)?.iter().map(|e| e.paie_ecrite(mois)).collect::<R<_>>()?;
        let change = |a: &Option<Arc<PaieEcrite>>, b: &Option<Arc<PaieEcrite>>| match (a, b) {
            (Some(x), Some(y)) => !chiffres_egaux(x, y),
            (x, y) => x.is_some() != y.is_some(),
        };
        if avant.len() != apres.len() || avant.iter().zip(&apres).any(|(a, b)| change(a, b)) {
            refaits.push(mois);
        }
    }
    Ok(refaits)
}

/// Quand les documents de `mois` de cet engagement sont partis (le premier envoi) ; `None` s'ils ne le sont pas.
fn documents_partis(siens: &[Depart], mois: Mois) -> Option<InstantOuJour> {
    siens.iter().find(|d| d.envoi == Envoi::Documents && d.mois == Some(mois)).map(|d| d.le)
}

// ------------------------------------------------------------------ les trois etats

/// Un engagement en saisie.
#[derive(Debug, Clone)]
pub struct EnSaisie {
    pub objet: Objet,
}

/// Un engagement valide : ses paies sont ecrites. Seul ce module en fait un.
#[derive(Debug, Clone)]
pub struct Valide {
    objet: Objet,
    le: Moment,
    /// Pourquoi la paie du mois lu ne s'est pas ecrite : le refus du recalcul (`du_mois`), quand elle manque, ou quand il
    /// dit une saisie a finir (`saisie_incomplete`) : la paie ecrite ne tient plus, et elle ne se lit pas.
    refus: Option<Echec>,
    /// La paie d'un mois qu'une validation en cours vient de calculer (`valides_lues`) : elle se lit en place de l'ecrite.
    en_cours: Option<(Mois, Arc<PaieEcrite>)>,
}

/// Un engagement valide dont les documents du mois `mois` sont partis le `le`.
#[derive(Debug, Clone)]
pub struct Partis {
    valide: Valide,
    pub mois: Mois,
    pub le: InstantOuJour,
}

/// Ce qu'est un engagement pour un mois.
#[derive(Debug, Clone)]
pub enum Etat {
    EnSaisie(EnSaisie),
    Valide(Valide),
    Partis(Partis),
}

impl Valide {
    fn de(objet: Objet) -> Option<Valide> {
        let le = objet.brut.valide.as_ref()?.le;
        Some(Valide { objet, le, refus: None, en_cours: None })
    }

    /// L'engagement tel qu'il est ecrit.
    pub fn objet(&self) -> &Objet {
        &self.objet
    }

    /// Quand il a ete valide.
    pub fn le(&self) -> Moment {
        self.le
    }

    /// Le numero de sa version (1 tant qu'aucune modification n'a suivi un envoi) ; `None` pour une validation qui ne le
    /// porte pas (`a_revalider`).
    pub fn version(&self) -> Option<VersionDeLEngagement> {
        self.objet.brut.valide.as_ref()?.version
    }

    /// CE QUI PART SE LIT DANS LES VERSIONS : une validation sans son numero de version ou sans sa saisie ne dit pas ce
    /// qu'un envoi emporterait. Rien ne part d'elle et rien ne s'y relit : la phrase du refus ; `None` quand elle dit tout.
    pub fn a_revalider(&self, ctx: &Contexte) -> Option<String> {
        a_revalider(ctx, &self.objet, stock::Version::Presente(self.objet.brut.valide.as_ref()?))
    }

    /// Ses versions passees, de la plus ancienne a la plus recente.
    pub fn versions(&self) -> &[stock::VersionPassee] {
        &self.objet.brut.versions
    }

    /// La version `n`, passee ou presente, telle que l'engagement l'ecrit (sa saisie) ; ses paies se lisent par
    /// `paie_ecrite_de_la_version`.
    pub fn version_numero(&self, n: VersionDeLEngagement) -> Option<stock::Version<'_>> {
        match &self.objet.brut.valide {
            Some(v) if v.version == Some(n) => Some(stock::Version::Presente(v)),
            _ => self.versions().iter().find(|v| v.version == n).map(stock::Version::Passee),
        }
    }

    /// La paie ecrite de `mois` (`{"paie", "fiches", "pas"?, "empreinte", "recalcul"?}`), si elle l'est ; une paie ecrite
    /// qui ne se lit pas se refuse.
    pub fn paie_ecrite(&self, mois: Mois) -> R<Option<Arc<PaieEcrite>>> {
        if self.refus.as_ref().is_some_and(crate::saisie_incomplete) {
            return Ok(None);
        }
        match &self.en_cours {
            Some((m, p)) if *m == mois => Ok(Some(p.clone())),
            _ => Ok(crate::paies::presente(&self.objet, mois)?),
        }
    }

    /// Pourquoi la paie de `mois` s'est recalculee d'elle-meme, et quand.
    pub fn recalcul(&self, mois: Mois) -> R<Option<Recalcul>> {
        Ok(self.paie_ecrite(mois)?.and_then(|p| p.recalcul.clone()))
    }

    /// LA PAIE DE `mois`, TELLE QU'ELLE A ETE VALIDEE : ce que lit ce qui fabrique un document. `data` : la saisie du mois
    /// (`stock::vue_du_mois`), `i` : le rang de l'engagement dans cette saisie.
    pub fn paie(&self, ctx: &Contexte, data: &crate::saisie::VueDuMois, i: RangDansLeMois) -> R<Paie> {
        let mois = data.mois;
        let x = self.paie_ecrite(mois)?.ok_or_else(|| match &self.refus {
            Some(r) => r.clone().reformuler(|p| format!("{} : la paie de {} ne se calcule pas. {}", qui_de(ctx, &self.objet), mois.libelle(Graphie::Accentuee), p)),
            None => socle::refus!(
                "{} : la paie de {} n'est pas calculée. Revalidez l'engagement : ouvrez sa carte, puis « Modifier l'engagement ».",
                qui_de(ctx, &self.objet),
                mois.libelle(Graphie::Accentuee)
            ),
        })?;
        let une = relire::saisie_d_un_engagement(data, i)?;
        Ok(paie_sur_son_ecrite(relire::paies(ctx, &une, &[&x.paie])?.pop().ok_or_else(|| socle::echec::cle(socle::echec::PAIE_ILLISIBLE, "paie"))?, &x))
    }
}

/// La paie relue `paie`, avec ce que sa paie ecrite `x` porte a cote d'elle : son prelevement, ses compteurs de conges,
/// le numero de son bulletin.
fn paie_sur_son_ecrite(mut paie: Paie, x: &PaieEcrite) -> Paie {
    let p = paie.ecrire();
    p.pas = x.pas.clone();
    p.conges = x.conges.clone();
    p.conges_avant = x.conges_avant.clone();
    p.numero_bulletin = x.bulletin.clone();
    paie
}

/// LES NUMEROS DES DOCUMENTS QUI PARTENT, ecrits dans la paie de `mois` de l'engagement du salarie `cle` au contrat
/// `numero` : le numero de son bulletin, le premier N CE de son certificat. Ils se relisent ensuite (`numeros_ecrits`) :
/// un document reparti a l'identique garde ses numeros. Une paie deja numerotee ne change pas de
/// numero.
pub fn poser_numeros_des_documents(ctx: &Contexte, mois: Mois, cle: &IdSalarie, numero: &crate::numeros::NumeroContrat, bulletin: Option<&crate::numeros::NumeroDeBulletin>, ce: Option<crate::numeros::NumeroDeCertificat>) -> R<()> {
    let o = stock::objets_du_mois(&ctx.racine, mois)?
        .into_iter()
        .find(|o| o.carte().salarie.as_ref() == Some(cle) && o.carte().numero.as_ref() == Some(numero))
        .ok_or_else(|| socle::refus!("{} : pas d'engagement de {} au contrat {}", mois, cle, numero))?;
    let Some(mut p) = crate::paies::presente(&o, mois)?.filter(|_| o.est_valide()).map(|p| (*p).clone()) else {
        return Err(socle::refus!("{} : l'engagement de {} au contrat {} n'a pas de paie validee", mois, cle, numero));
    };
    let mut change = false;
    if let Some(b) = bulletin.filter(|_| p.bulletin.is_none()) {
        p.bulletin = Some(b.clone());
        change = true;
    }
    if let Some(n) = ce.filter(|_| p.ce.is_none()) {
        p.ce = Some(n);
        change = true;
    }
    if change {
        crate::paies::poser(&o, mois, Some(p))?;
    }
    Ok(())
}

impl Partis {
    /// L'engagement valide dont les documents sont partis.
    pub fn valide(&self) -> &Valide {
        &self.valide
    }
}

impl Etat {
    /// L'engagement tel qu'il est ecrit.
    pub fn objet(&self) -> &Objet {
        match self {
            Etat::EnSaisie(e) => &e.objet,
            Etat::Valide(v) => &v.objet,
            Etat::Partis(p) => &p.valide.objet,
        }
    }

    /// L'engagement valide, qu'il soit parti ou non ; `None` en saisie.
    pub fn valide(&self) -> Option<&Valide> {
        match self {
            Etat::EnSaisie(_) => None,
            Etat::Valide(v) => Some(v),
            Etat::Partis(p) => Some(&p.valide),
        }
    }

    /// La paie ecrite de `mois` de l'engagement valide (`Valide::paie_ecrite`) ; `None` en saisie.
    pub fn paie_ecrite(&self, mois: Mois) -> R<Option<Arc<PaieEcrite>>> {
        match self.valide() {
            Some(v) => v.paie_ecrite(mois),
            None => Ok(None),
        }
    }
}

/// Le contrat de l'engagement `o` a-t-il ete signe hors du logiciel (`crate::signe_ailleurs_de`) ? Ce qui ne se lit pas
/// se refuse.
pub fn signe_ailleurs(ctx: &Contexte, o: &Objet) -> R<bool> {
    let c = o.carte();
    let nat = c.nature_lue()?;
    let salarie = c.salarie.as_ref().ok_or_else(|| socle::fichiers::absent("salarie"))?;
    // Un engagement sans debut n'a pas de contrat.
    let signe = crate::debut_brut(&o.brut).map(|debut| crate::signe_ailleurs_de(&ctx.racine, &nat, salarie, debut, c.numero.as_ref(), o.fichier));
    Ok(signe.transpose()?.is_some_and(|oui| oui))
}

/// Qui est l'engagement `o`, pour une phrase de l'ecran : « MARTIN Lou (contrat du 20/11/2026) » (`crate::salarie_nomme`).
pub fn qui_de(ctx: &Contexte, o: &Objet) -> String {
    let debut: String = crate::debut_brut(&o.brut).map(|j| j.jj_mm_aaaa()).into_iter().collect();
    format!("{} (contrat du {})", crate::salarie_nomme(ctx, o.carte().salarie.as_ref()), debut)
}

/// Ce qu'est l'engagement `o` pour le mois `mois`, tel qu'il est ecrit.
fn etat(o: Objet, mois: Mois, departs: &Departs) -> Etat {
    let parti = documents_partis(ses_departs(&o, departs), mois);
    match Valide::de(o.clone()) {
        None => Etat::EnSaisie(EnSaisie { objet: o }),
        Some(v) => match parti {
            Some(le) => Etat::Partis(Partis { valide: v, mois, le }),
            None => Etat::Valide(v),
        },
    }
}

/// L'etat de chaque engagement que `mois` montre, tel qu'il est ecrit, sans le recalcul de lui-meme, d'apres ce qui est
/// parti (`departs`, que la couche du dessus lit) : ce que lit un bilan qui ne lit aucune paie.
pub fn tels_qu_ecrits(ctx: &Contexte, mois: Mois, departs: &Departs) -> R<Vec<Etat>> {
    Ok(stock::objets_du_mois(&ctx.racine, mois)?.into_iter().map(|o| etat(o, mois, departs)).collect())
}

/// Ce qui manque a la validation (ou a la version ecrite) `v` de `o` pour qu'un envoi en parte et qu'on y relise ce qui
/// est parti, dit en une phrase ; `None` quand rien ne manque.
fn a_revalider(ctx: &Contexte, o: &Objet, v: stock::Version) -> Option<String> {
    let mut manque = Vec::new();
    if v.numero().is_none() {
        manque.push("son numero de version".to_string());
    }
    if v.saisie().is_none() {
        manque.push("sa saisie".to_string());
    }
    (!manque.is_empty()).then(|| {
        format!(
            "{} : sa validation ne porte pas {}. Cet engagement est a revalider : ouvrir sa carte, puis « Modifier l'engagement » ; rien n'est ecrit.",
            qui_de(ctx, o),
            manque.join(", ni ")
        )
    })
}

/// LA LECTURE DE L'ENGAGEMENT VALIDE : l'etat de chaque engagement que `mois` montre, dans l'ordre de l'ecran, apres le
/// recalcul de lui-meme (`rafraichir`). C'est le seul chemin vers une paie validee : l'ecran, les exports, les
/// declarations, les documents et les envois lisent tous par ici, et lisent donc une paie juste.
///
/// UN RECALCUL QUE LE MOTEUR REFUSE n'est pas l'affaire de l'utilisateur : ce qui releve de lui (une saisie incomplete)
/// se refuse a la validation, avant tout calcul. L'engagement garde la paie validee, et le refus va au temoin que la
/// couche du dessus a pose (`poser_temoin`), qui le note pour l'editeur. Une paie que ce refus empeche d'ecrire (le mois
/// qu'un contrat sans terme n'avait pas encore) le porte : ce qui la lit dit pourquoi elle manque (le taux AT/MP de
/// l'annee, un bareme), et quoi faire.
pub fn du_mois(ctx: &Contexte, mois: Mois, departs: &Departs) -> R<Vec<Etat>> {
    lu_et_rafraichi(ctx, mois, departs, false)
}

/// Ce que `rafraichir` a fait : les etats lus, quand rien ne s'est reecrit ; et chaque engagement dont le recalcul a
/// refuse (son rang dans l'ecran), avec son refus.
struct Rafraichi {
    etats: Option<Vec<Etat>>,
    refus: Vec<(RangDansLeMois, Echec)>,
}

fn lu_et_rafraichi(ctx: &Contexte, mois: Mois, departs: &Departs, passage: bool) -> R<Vec<Etat>> {
    let refus: Vec<(Option<RangDansLeMois>, Echec)> = match rafraichir(ctx, mois, departs, passage) {
        // Rien ne s'est reecrit : les etats lus pour la verification sont ceux du mois.
        Ok(Rafraichi { etats: Some(etats), .. }) => return Ok(etats),
        Ok(Rafraichi { refus, .. }) => refus.into_iter().map(|(i, m)| (Some(i), m)).collect(),
        // Ce qui refuse avant tout calcul refuse pour tout le mois.
        Err(m) => vec![(None, m)],
    };
    let mut etats = tels_qu_ecrits(ctx, mois, departs)?;
    let mut temoins: Vec<(&str, Option<String>)> = Vec::new();
    for (_, m) in &refus {
        // Une saisie a finir est l'affaire de l'utilisateur : ce qui lit la paie la lui dit, l'editeur n'en est pas averti.
        let vu = (m.phrase.as_str(), m.a_part.as_ref().map(socle::ecriture::json_en_ligne));
        if !temoins.contains(&vu) && !crate::saisie_incomplete(m) {
            if let Some(temoin) = TEMOIN.get() {
                temoin(&ctx.racine, mois, m);
            }
            temoins.push(vu);
        }
    }
    // Une paie qui manque (un contrat sans terme qui entre dans le mois) ne s'est pas ecrite a cause de son refus : qui la
    // lit le dit, avec sa cause (`Valide::paie`).
    // Une saisie a finir que le recalcul attend (les indemnites journalieres d'un arret, `maintien`) : la paie ecrite ne
    // tient plus, et ce qui la lit dit quoi saisir plutot que de lire une paie fausse.
    for (i, e) in RangDansLeMois::de_chacun(etats.iter_mut()) {
        let (v, parti) = match e {
            Etat::Valide(v) => (v, false),
            Etat::Partis(p) => (&mut p.valide, true),
            Etat::EnSaisie(_) => continue,
        };
        let sien = refus.iter().find(|(r, _)| r.is_none_or(|r| r == i));
        let Some((_, m)) = sien else { continue };
        if v.paie_ecrite(mois)?.is_none() || (!parti && crate::saisie_incomplete(m)) {
            v.refus = Some(m.clone());
        }
    }
    Ok(etats)
}

/// Qui recoit un recalcul refuse : `(racine de la compagnie, mois, refus du moteur)`.
pub type Temoin = fn(&Path, Mois, &Echec);

static TEMOIN: std::sync::OnceLock<Temoin> = std::sync::OnceLock::new();

/// Pose le temoin des recalculs refuses (`du_mois`), une fois pour le processus.
pub fn poser_temoin(t: Temoin) {
    TEMOIN.get_or_init(|| t);
}

/// Pour chaque engagement que `mois` montre, dans l'ordre de l'ecran : est-il valide ? Ne lit aucune paie, donc ne
/// recalcule rien : ce que comptent l'accueil et le bilan d'un mois.
pub fn valides_du_mois(ctx: &Contexte, mois: Mois) -> R<Vec<bool>> {
    Ok(stock::objets_du_mois(&ctx.racine, mois)?.iter().map(Objet::est_valide).collect())
}

/// La paie ecrite de `mois` de l'engagement du salarie `cle` (la cle de sa fiche) au contrat `numero`, lue par
/// `du_mois` ; `None` s'il n'est pas valide ou n'a pas de paie ecrite pour ce mois.
pub fn paie_ecrite_de(ctx: &Contexte, mois: Mois, cle: &IdSalarie, numero: &crate::numeros::NumeroContrat, departs: &Departs) -> R<Option<PaieEcrite>> {
    for e in du_mois(ctx, mois, departs)? {
        let o = e.objet();
        if o.carte().salarie.as_ref() == Some(cle) && o.carte().numero.as_ref() == Some(numero) {
            if let Some(p) = e.paie_ecrite(mois)? {
                return Ok(Some((*p).clone()));
            }
        }
    }
    Ok(None)
}

/// CE QUI PEUT PARTIR DE `mois` : sa saisie reduite aux engagements valides (dans l'ordre de l'ecran), et la paie ecrite
/// de chacun, au meme rang. Un engagement en saisie n'y est pas : aucun document ne part de lui. Un mois d'apres le
/// present n'a pas encore de paie ecrite : celle de chaque engagement valide s'y calcule, sans s'ecrire, pour ce qui part
/// avant lui (la DPAE, le contrat). `None` : aucune saisie.
pub fn validees_du_mois(ctx: &Contexte, mois: Mois, departs: &Departs) -> R<Option<(crate::saisie::VueDuMois, Vec<Paie>)>> {
    validees_du_mois_parmi(ctx, mois, departs, &|_| true)
}

/// `validees_du_mois`, reduite aux engagements valides que `garder` retient : la paie d'un engagement que l'appelant ne
/// lit pas ne se relit pas.
pub fn validees_du_mois_parmi(ctx: &Contexte, mois: Mois, departs: &Departs, garder: &dyn Fn(&Valide) -> bool) -> R<Option<(crate::saisie::VueDuMois, Vec<Paie>)>> {
    let etats = du_mois(ctx, mois, departs)?;
    let Some(data) = stock::vue_du_mois(&ctx.racine, mois)? else { return Ok(None) };
    let mut gardes = Vec::new();
    let mut paies = Vec::new();
    let mut a_venir: Vec<RangDansLeMois> = Vec::new();
    if mois > mois_present(ctx) {
        for (i, e) in RangDansLeMois::de_chacun(&etats) {
            if e.valide().is_some_and(garder) && e.paie_ecrite(mois)?.is_none() {
                a_venir.push(i);
            }
        }
    }
    let mut calculees = if a_venir.is_empty() { Vec::new() } else { calculer_les_rangs(ctx, mois, &a_venir)? }.into_iter();
    for (i, e) in RangDansLeMois::de_chacun(&etats) {
        let Some(v) = e.valide().filter(|v| garder(v)) else { continue };
        let paie = match a_venir.contains(&i).then(|| calculees.next()).flatten() {
            Some(p) => Valide { en_cours: Some((mois, Arc::new(p))), ..v.clone() }.paie(ctx, &data, i)?,
            None => v.paie(ctx, &data, i)?,
        };
        paies.push(paie);
        gardes.push(i.dans(&data.engagements).cloned().ok_or_else(|| socle::echec::cle(socle::echec::PAIE_ILLISIBLE, "engagements"))?);
    }
    Ok(Some((crate::saisie::VueDuMois { engagements: gardes, ..data }, paies)))
}

/// LES PAIES DE `mois`, CE QUE LISENT LES ETATS : la paie ecrite de chaque engagement valide de la saisie qui en a une
/// (`validees_du_mois`), puis celle de chaque engagement importe paye ce mois (`importe::paies_du_mois` : ce que les
/// pieces de l'ancien logiciel en disent, dans les mots du moteur). Un engagement en saisie n'est pas paye ; rien ne se
/// calcule. Ce qui part (DSN, documents) ne lit que la saisie (`validees_du_mois`) : un mois importe a deja ete declare.
pub fn paies_du_mois(ctx: &Contexte, mois: Mois, departs: &Departs) -> R<Vec<PaieDuMois>> {
    let mut paies: Vec<PaieDuMois> =
        validees_du_mois(ctx, mois, departs)?.into_iter().flat_map(|(_, p)| p).filter(|p| !p.engagement.sans_paie()).map(PaieDuMois::Validee).collect();
    paies.extend(crate::importe::paies_du_mois(ctx, mois)?.into_iter().map(PaieDuMois::Importee));
    Ok(paies)
}

/// UNE PAIE DU MOIS, CE QUE LIT UN ETAT : la paie ecrite d'un engagement valide, ou celle d'un engagement importe, que
/// ses pieces disent (`importe::PaieImportee` : un montant qu'elles taisent est vide). Ce qu'elles ont en commun se lit
/// par les memes noms ; un montant se lit `Option` : toujours dit pour une paie validee, dit ou non pour une importee.
#[derive(Debug, Clone)]
pub enum PaieDuMois {
    Validee(Paie),
    Importee(crate::importe::PaieImportee),
}

/// UNE LIGNE DE REMUNERATION D'UNE PAIE DU MOIS, telle qu'un etat la lit, sans recopie : la ligne de remuneration que le
/// calcul rend (`LigneBrut`, elle dit ce qu'elle paie) ; d'un bulletin importe, celle que l'import a rattachee a sa
/// rubrique (`LigneRattachee`, ce qu'elle paie quand le bulletin l'imprime), ou une ligne qu'il n'a pas rattachee,
/// telle qu'il l'imprime (`importe::LigneImprimee` : son libelle, son montant).
#[derive(Debug, Clone, Copy)]
pub enum RemunerationDuMois<'a> {
    Calculee(&'a crate::calcul::LigneBrut),
    Rattachee(&'a crate::calcul::LigneRattachee),
    Imprimee(&'a crate::importe::LigneImprimee),
}

/// UNE LIGNE DE COTISATION D'UNE PAIE DU MOIS, telle qu'un etat la lit, sans recopie : celle du calcul
/// (`LigneCotisation`, tout y est dit, et elle compte dans les totaux de la paie), ou celle que le bulletin importe
/// imprime (`importe::CotisationImprimee`, ce qu'il ne dit pas est vide).
#[derive(Debug, Clone, Copy)]
pub enum CotisationDuMois<'a> {
    Calculee(&'a crate::calcul::LigneCotisation),
    Imprimee(&'a crate::importe::CotisationImprimee),
}

impl CotisationDuMois<'_> {
    /// Sa cotisation (ou le regroupement que l'import a donne a la ligne d'un bulletin) ; `None` : une ligne d'un bulletin
    /// importe que l'import n'a pas qualifiee.
    pub fn cotisation(&self) -> Option<crate::ecarts::RegroupementDeComparaison> {
        match self {
            CotisationDuMois::Calculee(l) => Some(crate::ecarts::RegroupementDeComparaison::Cotisation(l.id.clone())),
            CotisationDuMois::Imprimee(l) => l.cotisation.clone(),
        }
    }

    pub fn libelle(&self) -> &str {
        match self {
            CotisationDuMois::Calculee(l) => &l.libelle,
            CotisationDuMois::Imprimee(l) => &l.libelle,
        }
    }

    /// Sa base, dans la grandeur que sa cotisation compte ; `None` : un bulletin importe qui ne l'imprime pas.
    pub fn base(&self) -> Option<crate::calcul::BaseDeCotisation> {
        match self {
            CotisationDuMois::Calculee(l) => Some(l.base),
            CotisationDuMois::Imprimee(l) => l.base,
        }
    }

    pub fn taux_salarie(&self) -> Option<socle::grandeurs::Taux> {
        match self {
            CotisationDuMois::Calculee(l) => l.taux_sal,
            CotisationDuMois::Imprimee(l) => l.taux_salarie,
        }
    }

    pub fn taux_employeur(&self) -> Option<socle::grandeurs::Taux> {
        match self {
            CotisationDuMois::Calculee(l) => l.taux_pat,
            CotisationDuMois::Imprimee(l) => l.taux_employeur,
        }
    }

    /// Sa part salariale ; `None` : la colonne qu'un bulletin importe laisse vide.
    pub fn montant_salarie(&self) -> Option<Montant> {
        match self {
            CotisationDuMois::Calculee(l) => Some(l.montant_sal),
            CotisationDuMois::Imprimee(l) => l.montant_salarie,
        }
    }

    /// Sa part patronale ; `None` : la colonne qu'un bulletin importe laisse vide.
    pub fn montant_employeur(&self) -> Option<Montant> {
        match self {
            CotisationDuMois::Calculee(l) => Some(l.montant_pat),
            CotisationDuMois::Imprimee(l) => l.montant_employeur,
        }
    }
}

impl PaieDuMois {
    pub fn engagement(&self) -> &crate::Engagement {
        match self {
            PaieDuMois::Validee(p) => &p.engagement,
            PaieDuMois::Importee(p) => &p.engagement,
        }
    }

    pub fn salarie(&self) -> &droit::fiche::SalarieDuContrat {
        match self {
            PaieDuMois::Validee(p) => &p.salarie,
            PaieDuMois::Importee(p) => &p.salarie,
        }
    }

    pub fn spectacle(&self) -> Option<&droit::fiche::Spectacle> {
        match self {
            PaieDuMois::Validee(p) => p.spectacle.as_ref(),
            PaieDuMois::Importee(p) => p.spectacle.as_ref(),
        }
    }

    /// SANS PAIE : un engagement valide sans ligne (`Engagement::sans_paie`), ou une paie importee dont aucune ligne n'est
    /// rattachee a une rubrique.
    pub fn sans_paie(&self) -> bool {
        match self {
            PaieDuMois::Validee(p) => p.engagement.sans_paie(),
            PaieDuMois::Importee(p) => p.remuneration.iter().all(|(_, r)| r.is_none()),
        }
    }

    /// La paie importee, ou `None` pour une paie validee.
    pub fn importee(&self) -> Option<&crate::importe::PaieImportee> {
        match self {
            PaieDuMois::Validee(_) => None,
            PaieDuMois::Importee(p) => Some(p),
        }
    }

    /// La paie validee, ou `None` pour une paie importee.
    pub fn validee(&self) -> Option<&Paie> {
        match self {
            PaieDuMois::Validee(p) => Some(p),
            PaieDuMois::Importee(_) => None,
        }
    }

    fn montant(&self, validee: fn(&Paie) -> Montant, importee: fn(&crate::importe::PaieImportee) -> Option<Montant>) -> Option<Montant> {
        match self {
            PaieDuMois::Validee(p) => Some(validee(p)),
            PaieDuMois::Importee(p) => importee(p),
        }
    }

    pub fn brut(&self) -> Option<Montant> {
        self.montant(|p| p.brut, |p| p.brut)
    }

    pub fn total_sal(&self) -> Option<Montant> {
        self.montant(|p| p.total_sal, |p| p.total_sal)
    }

    pub fn total_pat(&self) -> Option<Montant> {
        self.montant(|p| p.total_pat, |p| p.total_pat)
    }

    pub fn net_a_payer(&self) -> Option<Montant> {
        self.montant(|p| p.net_a_payer, |p| p.net_a_payer)
    }

    pub fn net_imposable(&self) -> Option<Montant> {
        self.montant(|p| p.net_imposable, |p| p.net_imposable)
    }

    pub fn cout_employeur(&self) -> Option<Montant> {
        self.montant(|p| p.cout_employeur, |p| p.cout_employeur)
    }

    pub fn plafond(&self) -> Option<Montant> {
        self.montant(|p| p.plafond, |p| p.plafond)
    }

    /// L'assiette de la CSG : celle du calcul, ou celle que les pieces disent ; une assiette en heures se refuse.
    pub fn base_csg(&self) -> R<Option<Montant>> {
        match self {
            PaieDuMois::Validee(p) => Ok(Some(p.base_csg)),
            PaieDuMois::Importee(p) => p.bases.get(&droit::assiette::Assiette::Csg).map(|b| b.en_euros(droit::assiette::Assiette::Csg)).transpose(),
        }
    }

    /// Les assiettes, chacune dans ce qu'elle compte : celles du calcul, ou celles que les pieces disent.
    pub fn bases(&self) -> &BTreeMap<droit::assiette::Assiette, crate::calcul::BaseDeCotisation> {
        match self {
            PaieDuMois::Validee(p) => &p.bases,
            PaieDuMois::Importee(p) => &p.bases,
        }
    }

    /// Les lignes de cotisation, telles que le bulletin les imprime : celles du calcul, ou celles du bulletin importe.
    pub fn cotisations(&self) -> Vec<CotisationDuMois<'_>> {
        match self {
            PaieDuMois::Validee(p) => p.lignes.iter().map(CotisationDuMois::Calculee).collect(),
            PaieDuMois::Importee(p) => p.lignes.iter().map(CotisationDuMois::Imprimee).collect(),
        }
    }

    /// LES CACHETS DE LA PAIE : ceux que son engagement compte (la somme de ses lignes de cachets, ce que la DSN
    /// declare en activite) ; `None` pour une paie importee dont aucune ligne n'est rattachee a une rubrique
    /// (ses heures, `heures_annexes`, ne sont pas dites).
    pub fn cachets(&self) -> Option<socle::grandeurs::Cachets> {
        self.heures_annexes().map(|_| self.engagement().cachets)
    }

    /// LES HEURES DE LA PAIE POUR LES ANNEXES 8 ET 10 (un cachet compte pour les heures que sa rubrique declare) : celles
    /// que le calcul a sommees (`heures_travaillees`), ou celles des lignes rattachees du bulletin importe
    /// (`PaieImportee::heures_annexes`) ; `None` pour une paie importee dont aucune ligne n'est rattachee a une rubrique.
    /// Ce ne sont pas les heures de son engagement (`Engagement::heures`, celles que la DSN declare : sans les cachets).
    pub fn heures_annexes(&self) -> Option<socle::grandeurs::Heures> {
        match self {
            PaieDuMois::Validee(p) => Some(p.heures_travaillees),
            PaieDuMois::Importee(p) => p.heures_annexes,
        }
    }

    /// Les lignes de remuneration, dans l'ordre du bulletin : celles du calcul, ou celles du bulletin importe.
    pub fn remuneration(&self) -> Vec<RemunerationDuMois<'_>> {
        match self {
            PaieDuMois::Validee(p) => p.remuneration.iter().map(RemunerationDuMois::Calculee).collect(),
            PaieDuMois::Importee(p) => p
                .remuneration
                .iter()
                .map(|(imprimee, rattachee)| match rattachee {
                    Some(r) => RemunerationDuMois::Rattachee(r),
                    None => RemunerationDuMois::Imprimee(imprimee),
                })
                .collect(),
        }
    }
}

/// LES PAIES VALIDEES DU SALARIE `salarie` (la cle de sa fiche) EN `mois` QU'UN CALCUL LIT (le compteur de conges, et
/// ce que l'employeur lui a verse, `paies_du_salarie`) : la paie de chacun de ses
/// engagements valides du mois, dans l'ordre de l'ecran -- ecrite, ou calculee sans s'ecrire quand son mois ne l'a pas
/// encore ecrite (`valides_lues`). Un engagement en saisie n'y est pas : ce qui n'est pas valide n'est pas paye. Rien ne
/// se reecrit. Pendant une validation, les mois que son engagement a deja calcules s'y lisent comme ecrits
/// (`EnValidation`).
///
/// Le calcul qui les lit note leur empreinte (`paies <mois> <salarie>`, `empreinte_des_paies_validees`), et elle seule :
/// une paie validee de ce salarie qui change fait recalculer ses mois qui l'ont lue ; celle d'un autre salarie, non.
pub fn paies_validees(ctx: &Contexte, mois: Mois, salarie: &IdSalarie) -> R<Vec<Paie>> {
    paies_validees_dont(ctx, mois, salarie, LectureDatee::Paies { mois, salarie: salarie.clone() }, &|_| Ok(true))
}

/// LES PAIES VALIDEES DE `mois` DU SALARIE `salarie` DONT LE CONTRAT EST PAYE AU MOIS (`Carte::mensuelle`), telles que
/// les relit la regularisation du plafond (`conges::plafonds_des_mois_anterieurs`) : seules elles peuvent se regulariser
/// (`droit::forme_bareme::RegularisationDuPlafond::regularise`), et une paie d'artiste ou de technicien intermittent,
/// qui n'entre pas dans le cumul, ne se lit ni ne se calcule : son refus reste le sien et ne ferme pas la paie d'un CDI
/// du meme salarie (« Le logiciel est doux »). La lecture notee est la leur (`paies-au-mois <mois> <salarie>`,
/// `empreinte_des_paies_validees_au_mois`).
pub fn paies_validees_au_mois(ctx: &Contexte, mois: Mois, salarie: &IdSalarie) -> R<Vec<Paie>> {
    paies_validees_dont(ctx, mois, salarie, LectureDatee::PaiesAuMois { mois, salarie: salarie.clone() }, &au_mois)
}

/// LES PAIES VALIDEES DE `mois` DU SALARIE `salarie` DONT LE CONTRAT N'EST PAS PAYE AU MOIS (ses contrats d'intermittent),
/// telles que les relit l'annee de sa retraite complementaire (`conges::retraite_des_mois_anterieurs`) : une paie d'un
/// CDI du meme salarie n'y entre pas, ne se lit ni ne se calcule, et son refus ne ferme pas celle d'un intermittent. La
/// lecture notee est la leur (`paies-intermittentes <mois> <salarie>`, `empreinte_des_paies_validees_intermittentes`).
pub fn paies_validees_intermittentes(ctx: &Contexte, mois: Mois, salarie: &IdSalarie) -> R<Vec<Paie>> {
    paies_validees_dont(ctx, mois, salarie, LectureDatee::PaiesIntermittentes { mois, salarie: salarie.clone() }, &intermittent)
}

/// Un engagement dont le contrat est paye au mois.
fn au_mois(o: &Objet) -> R<bool> {
    o.carte().mensuelle()
}

/// Un engagement d'intermittent : son contrat n'est pas paye au mois.
fn intermittent(o: &Objet) -> R<bool> {
    Ok(!o.carte().mensuelle()?)
}

/// Les paies validees de `mois` du salarie, des seuls engagements que `garder` retient, filtres avant tout calcul
/// (`valides_lues`) ; la lecture `lecture` se note avec leur empreinte (`empreinte_des_paies`).
fn paies_validees_dont(
    ctx: &Contexte, mois: Mois, salarie: &IdSalarie, lecture: LectureDatee<IdSalarie, droit::missions::LieuDeMission>, garder: &dyn Fn(&Objet) -> R<bool>,
) -> R<Vec<Paie>> {
    // Un mois sans saisie n'a aucune paie validee.
    let lire = |saisie: std::sync::Arc<(crate::saisie::VueDuMois, socle::fichiers::EmpreinteDeSaisie)>| -> R<Vec<Paie>> {
        let data = &saisie.0;
        let valides = valides_lues(ctx, mois, Some(salarie), garder)?;
        socle::fichiers::noter_lecture(&CleLue::Datee(lecture.clone()), ValeurLue::Empreinte(empreinte_des_paies(&valides, mois)?));
        let mut paies = Vec::new();
        for (i, v) in &valides {
            paies.push(v.paie(ctx, data, *i)?);
        }
        Ok(paies)
    };
    Ok(stock::vue_et_empreinte(&ctx.racine, mois)?.map(lire).transpose()?.into_iter().flatten().collect())
}

/// UNE PAIE D'UN AUTRE MOIS, TELLE QU'UN CALCUL LA LIT (`paies_du_salarie`) : la paie ecrite d'un engagement valide,
/// ou une paie reprise d'un ancien logiciel telle que ses pieces la disent (`importe::PaieReprise`), sans rien ranger
/// ni calculer d'elle. Chaque calcul y lit sa grandeur, et une grandeur que les pieces taisent est vide.
#[derive(Debug, Clone)]
pub enum PaieAnterieure {
    Validee(Paie),
    Reprise(crate::importe::PaieReprise),
}

impl PaieAnterieure {
    /// Son brut ; `None` : une paie reprise dont les pieces ne le disent pas.
    pub fn brut(&self) -> Option<Montant> {
        match self {
            PaieAnterieure::Validee(p) => Some(p.brut),
            PaieAnterieure::Reprise(r) => r.paie.brut,
        }
    }

    /// Le numero du contrat qu'elle paie, s'il est dit.
    fn numero(&self) -> Option<&crate::numeros::NumeroContrat> {
        match self {
            PaieAnterieure::Validee(p) => p.engagement.numero.as_ref(),
            PaieAnterieure::Reprise(r) => r.numero.as_ref(),
        }
    }

    /// La cle de la reprise des cumuls de l'annee (`droit::fiche::RepriseDesCumuls`), que seule une paie reprise donne.
    pub fn reprise_des_cumuls(&self) -> Option<droit::fiche::RepriseDesCumuls> {
        match self {
            PaieAnterieure::Validee(_) => None,
            PaieAnterieure::Reprise(r) => Some(r.paie.reprise_des_cumuls()),
        }
    }
}

/// CE QUE L'EMPLOYEUR A VERSE AU SALARIE `salarie` (la cle de sa fiche) EN `mois`, TEL QU'UN CALCUL D'UN AUTRE MOIS LE
/// LIT (`PaieAnterieure`) : ses paies validees (`paies_validees`), puis ses paies reprises d'un ancien logiciel, telles
/// que leurs pieces les disent (`importe::reprises_du_salarie`). Les textes que ces calculs appliquent parlent de la
/// remuneration versee, quel que soit le logiciel qui l'a calculee : les paies de l'ancien logiciel y comptent comme les
/// siennes. Le lisent, par `conges::paies_versees` : le salaire des indemnites journalieres (`maintien::ij_de_principe`),
/// le salaire de reference et la remuneration du contrat du solde de tout compte (`solde`), les cumuls de l'annee civile
/// (`conges::cumul_de_l_annee`). Le compteur de conges ne lit que les validees : ce que l'ancien logiciel en avait, la
/// fiche le reprend (`conges_reprise`).
///
/// Le calcul qui les lit note l'empreinte des paies reprises (`reprises <mois> <salarie>`,
/// `importe::empreinte_des_reprises`) comme celle des validees (`paies <mois> <salarie>`) : une paie reprise qui change
/// (un reimport, un bulletin qui complete une paie de DSN, une ligne rattachee) fait recalculer ses mois qui l'ont lue.
///
/// UNE PAIE NE SE COMPTE PAS DEUX FOIS : une paie reprise et une paie validee du meme contrat le meme mois refusent, en
/// nommant le mois et le contrat. L'import ne reprend pas un bulletin d'un mois que la saisie paie, mais une DSN de
/// l'ancien logiciel qui declare un mois que le contrat continue ici se reprend telle qu'elle a ete deposee.
pub fn paies_du_salarie(ctx: &Contexte, mois: Mois, salarie: &IdSalarie) -> R<Vec<PaieAnterieure>> {
    let validees: Vec<PaieAnterieure> = paies_validees(ctx, mois, salarie)?.into_iter().map(PaieAnterieure::Validee).collect();
    let reprises = crate::importe::reprises_du_salarie(&ctx.racine, mois, salarie)?;
    let lecture = CleLue::Datee(LectureDatee::Reprises { mois, salarie: salarie.clone() });
    socle::fichiers::noter_lecture(&lecture, ValeurLue::Empreinte(crate::importe::empreinte_de(&reprises)));
    let mut paies = validees.clone();
    for r in reprises {
        if let Some(n) = r.numero.as_ref().filter(|n| validees.iter().any(|p| p.numero() == Some(*n))) {
            return Err(socle::refus!(
                "Le contrat {} de {} est payé en {} deux fois : par l'ancien logiciel, dont la paie de ce mois est reprise, et \
                 par une paie validée ici. Un mois que l'ancien logiciel a payé ne se paie pas ici : modifier l'engagement \
                 d'ici pour qu'il commence après ce mois. Rien n'est calculé.",
                n,
                salarie,
                mois.libelle(socle::mois::Graphie::Accentuee)
            ));
        }
        paies.push(PaieAnterieure::Reprise(r));
    }
    Ok(paies)
}

/// L'empreinte des paies validees de `mois` telles qu'un calcul les lit (`paies_validees`) : celles du salarie `salarie`
/// (la cle de sa fiche), ou toutes avec `None` (ce que la DSN du mois declare) ; ce qui ne se lit pas se refuse.
pub fn empreinte_des_paies_validees(ctx: &Contexte, mois: Mois, salarie: Option<&IdSalarie>) -> R<socle::fichiers::Empreinte> {
    empreinte_des_paies(&valides_lues(ctx, mois, salarie, &|_| Ok(true))?, mois)
}

/// Ce que vaut la lecture `paies-au-mois <mois> <salarie>` (`paies_validees_au_mois`).
pub fn empreinte_des_paies_validees_au_mois(ctx: &Contexte, mois: Mois, salarie: &IdSalarie) -> R<socle::fichiers::Empreinte> {
    empreinte_des_paies(&valides_lues(ctx, mois, Some(salarie), &au_mois)?, mois)
}

/// Ce que vaut la lecture `paies-intermittentes <mois> <salarie>` (`paies_validees_intermittentes`).
pub fn empreinte_des_paies_validees_intermittentes(ctx: &Contexte, mois: Mois, salarie: &IdSalarie) -> R<socle::fichiers::Empreinte> {
    empreinte_des_paies(&valides_lues(ctx, mois, Some(salarie), &intermittent)?, mois)
}

/// Ce qu'un calcul lit d'une paie validee d'un autre mois : la paie, et le compteur de conges avant son mois.
fn empreinte_des_paies(valides: &[(RangDansLeMois, Valide)], mois: Mois) -> R<socle::fichiers::Empreinte> {
    let paies: Vec<Option<Arc<PaieEcrite>>> = valides.iter().map(|(_, v)| v.paie_ecrite(mois)).collect::<R<_>>()?;
    let lues: Vec<Option<(&FormeDePaie<Avis>, &Option<crate::conges::Avant>)>> = paies.iter().map(|p| p.as_ref().map(|p| (&p.paie, &p.conges_avant))).collect();
    Ok(socle::fichiers::Empreinte::des_octets(socle::ecriture::json_en_ligne(&lues).as_bytes()))
}

thread_local! {
    /// Les engagements en cours de validation (`fichier`, `rang`) et les paies deja calculees de leurs mois.
    static EN_VALIDATION: RefCell<Option<Vec<(Mois, usize, BTreeMap<Mois, Arc<PaieEcrite>>)>>> = const { RefCell::new(None) };
}

/// UNE VALIDATION EN COURS : les mois d'un engagement se calculent du premier au dernier, et chacun lit ceux d'avant
/// (`paies_validees`) tels qu'elle vient de les calculer. Elle se ferme quand elle tombe.
struct EnValidation(Option<Vec<(Mois, usize, BTreeMap<Mois, Arc<PaieEcrite>>)>>);

impl EnValidation {
    fn ouvrir(o: &Objet) -> EnValidation {
        EnValidation(EN_VALIDATION.with(|e| e.replace(Some(vec![(o.fichier, o.rang, BTreeMap::new())]))))
    }

    fn calculee(&self, mois: Mois, paie: &PaieEcrite) {
        noter_calculee(mois, paie);
    }
}

/// La paie de `mois` que le dernier engagement ouvert dans la validation en cours vient de calculer.
fn noter_calculee(mois: Mois, paie: &PaieEcrite) {
    EN_VALIDATION.with(|e| {
        if let Some((_, _, paies)) = e.borrow_mut().as_mut().and_then(|v| v.last_mut()) {
            paies.insert(mois, Arc::new(paie.clone()));
        }
    });
}

impl Drop for EnValidation {
    fn drop(&mut self) {
        let avant = self.0.take();
        EN_VALIDATION.with(|e| *e.borrow_mut() = avant);
    }
}

/// UN ENGAGEMENT EN SAISIE SE CALCULE POUR L'ECRAN COMME SA VALIDATION LE CALCULERA : le calcul de `mois` y lit les mois
/// d'avant de chaque engagement en saisie qui les couvre tels que sa validation les calculerait (`calculer`, du premier
/// au dernier, comme `paies_a_valider`), si bien que l'indemnite de conges montree est celle qui sera retenue. Rien ne
/// s'ecrit. Pendant une validation, rien de plus : elle tient deja ses mois. Un mois d'avant qui ne se lit ou ne se
/// calcule pas refuse le calcul, avec sa raison : sans lui, l'indemnite montree ne serait pas celle de la validation. Un
/// engagement valide se calcule de meme dans les mois d'apres le present, qui n'ont pas encore sa paie ; ses mois ecrits
/// se lisent tels qu'ils sont (`valides_lues`).
pub(crate) struct CommeALaValidation {
    _garde: Option<EnValidation>,
}

impl CommeALaValidation {
    pub(crate) fn ouvrir(ctx: &Contexte, mois: Mois, salaries: &[&IdSalarie]) -> R<CommeALaValidation> {
        if EN_VALIDATION.with(|e| e.borrow().is_some()) {
            return Ok(CommeALaValidation { _garde: None });
        }
        let objets = stock::objets_du_mois(&ctx.racine, mois)?;
        let present = mois_present(ctx);
        // Une paie ne lit que les paies de son salarie (`valides_lues`) : seuls les engagements des salaries `salaries`
        // (ceux que le calcul en cours paie) se calculent ; un autre salarie du mois ne leur fait rien refuser.
        let du_calcul = |o: &Objet| o.carte().salarie.as_ref().is_some_and(|s| salaries.contains(&s));
        let a_calculer: Vec<Objet> = objets.into_iter().filter(|o| o.fichier < mois && (!o.est_valide() || mois > present) && du_calcul(o)).collect();
        if a_calculer.is_empty() {
            return Ok(CommeALaValidation { _garde: None });
        }
        let garde = EnValidation(EN_VALIDATION.with(|e| e.replace(Some(Vec::new()))));
        for o in &a_calculer {
            EN_VALIDATION.with(|e| {
                if let Some(v) = e.borrow_mut().as_mut() {
                    v.push((o.fichier, o.rang, BTreeMap::new()));
                }
            });
            for m in o.mois_couverts(mois)?.into_iter().filter(|m| *m < mois) {
                if o.est_valide() && (m <= present || crate::paies::presente(o, m)?.is_some()) {
                    continue;
                }
                let p = calculer(ctx, o, m)?;
                noter_calculee(m, &p);
            }
        }
        Ok(CommeALaValidation { _garde: Some(garde) })
    }
}

/// Ce qui distingue l'etat de la validation en cours (les engagements et les mois deja calcules) : ce que lit
/// `paies_validees` en depend.
pub(crate) fn marque_de_validation() -> String {
    EN_VALIDATION.with(|e| e.borrow().iter().map(|v| v.iter().map(|(f, r, paies)| format!("{}#{}#{}", f, r, paies.len())).collect::<Vec<_>>().join(";")).collect())
}

/// Les engagements valides de `mois` (de `salarie`, la cle de sa fiche, s'il est dit) et leur paie de ce mois, avec leur
/// rang a l'ecran : la paie ecrite, ou celle qui se calcule quand son mois ne l'a pas encore ecrite ; un engagement en
/// cours de validation y porte les paies qu'elle a deja calculees, et seulement elles (un engagement deja valide, ses
/// paies ecrites a cote). Ce qui est parti n'y change rien (une paie partie se lit comme une paie validee) : le journal
/// des envois ne se lit pas, et un calcul qui lit ces paies ne depend que d'elles (`paies <mois> <salarie>`). Seuls les
/// engagements que `garder` retient y entrent, avant tout calcul : celui d'un autre ne refuse rien ici.
fn valides_lues(ctx: &Contexte, mois: Mois, salarie: Option<&IdSalarie>, garder: &dyn Fn(&Objet) -> R<bool>) -> R<Vec<(RangDansLeMois, Valide)>> {
    // Hors d'une validation, aucun engagement n'est en cours.
    let en_cours: Vec<_> = EN_VALIDATION.with(|e| e.borrow().clone()).into_iter().flatten().collect();
    let mut out = Vec::new();
    let mut a_calculer: Vec<(RangDansLeMois, Valide)> = Vec::new();
    for (i, o) in RangDansLeMois::de_chacun(stock::objets_du_mois(&ctx.racine, mois)?) {
        if salarie.is_some_and(|s| o.carte().salarie.as_ref() != Some(s)) || !garder(&o)? {
            continue;
        }
        let v = match en_cours.iter().find(|(f, r, paies)| o.fichier == *f && o.rang == *r && (paies.contains_key(&mois) || !o.est_valide())) {
            Some((_, _, paies)) => {
                let Some(p) = paies.get(&mois) else { continue };
                let mut lue = o.clone();
                lue.brut_mut().valide = Some(stock::Validation { le: maintenant(), version: None, saisie: None });
                Valide::de(lue).map(|v| Valide { en_cours: Some((mois, p.clone())), ..v })
            }
            None => Valide::de(o),
        };
        let Some(v) = v else { continue };
        match v.paie_ecrite(mois)? {
            Some(_) => out.push((i, v)),
            None => a_calculer.push((i, v)),
        }
    }
    // UNE PAIE VALIDEE QUE SON MOIS N'A PAS ENCORE ECRITE (personne n'a lu ce mois) se calcule ici, sans s'ecrire : ce
    // qui la lit (la remuneration du contrat d'un mois suivant, ses conges) depend de sa saisie, et son calcul note ce
    // qu'il a lu dans le registre de celui qui la demande (`socle::fichiers::rendre`). Ecrite plus tard, elle porte les
    // memes chiffres, et la meme empreinte (`empreinte_des_paies`).
    if !a_calculer.is_empty() {
        let rangs: Vec<RangDansLeMois> = a_calculer.iter().map(|(i, _)| *i).collect();
        for ((i, v), p) in a_calculer.into_iter().zip(calculer_les_rangs(ctx, mois, &rangs)?) {
            out.push((i, Valide { en_cours: Some((mois, Arc::new(p))), ..v }));
        }
        out.sort_by_key(|(i, _)| *i);
    }
    Ok(out)
}

/// Les numeros des documents de chaque engagement valide de `mois`, au meme rang que `validees_du_mois` : le numero de
/// son bulletin, le premier N CE de son certificat, et le bulletin que sa paie rectifie, tels que la paie ecrite les porte.
pub fn numeros_des_documents(ctx: &Contexte, mois: Mois, departs: &Departs) -> R<Vec<NumerosEcrits>> {
    tels_qu_ecrits(ctx, mois, departs)?
        .iter()
        .filter_map(Etat::valide)
        .map(|v| {
            let p = v.paie_ecrite(mois)?;
            Ok(NumerosEcrits { bulletin: p.as_ref().and_then(|p| p.bulletin.clone()), premier_ce: p.as_ref().and_then(|p| p.ce), rectifie: p.as_ref().and_then(|p| p.rectifie.clone()) })
        })
        .collect()
}

/// Ce qu'une paie ecrite porte des documents partis d'elle.
#[derive(Debug, Clone, Default)]
pub struct NumerosEcrits {
    pub bulletin: Option<crate::numeros::NumeroDeBulletin>,
    pub premier_ce: Option<crate::numeros::NumeroDeCertificat>,
    pub rectifie: Option<crate::numeros::NumeroDeBulletin>,
}

// ------------------------------------------------------------------ calculer et ecrire

/// Le rang de l'engagement `o` parmi ceux que `mois` montre.
fn rang_dans(ctx: &Contexte, o: &Objet, mois: Mois) -> R<RangDansLeMois> {
    RangDansLeMois::du_premier(&stock::objets_du_mois(&ctx.racine, mois)?, |x| x.fichier == o.fichier && x.rang == o.rang)
        .ok_or_else(|| socle::refus!("{} ne touche pas {}", qui_de(ctx, o), mois))
}

/// Le prelevement a la source de la paie `p`, au taux que la DGFiP a transmis et qui vaut le jour de son paiement
/// (`droit::pas::taux_dgfip`), sinon a la grille de l'article 204 H ; `None` pour un mois sans paie. Un bareme sans
/// grille refuse le calcul : un prelevement ne se devine pas (regle 1), et aucun document ne le calcule a la place.
pub(crate) fn prelevement(ctx: &Contexte, p: &Paie) -> R<Option<droit::pas::Versement>> {
    let e = &p.engagement;
    if e.sans_paie() {
        return Ok(None);
    }
    let cle = &p.salarie.id;
    let fin = e.periode_payee()?.fin();
    let taux = droit::pas::taux_dgfip(ctx, cle, Some(e.date_du_versement()?), Some(e.mois))?;
    let bareme = ctx.bareme(&fin.mois_civil().premier_jour())?;
    let court = droit::natures::contrat_court(&*e.nat()?, &e.contrat(), fin)?;
    Ok(Some(droit::pas::du_versement(taux.as_ref(), &bareme, assiette_du_pas(p)?, Some(fin), court, droit::pas::ZONE_DEFAUT)?))
}

/// L'ASSIETTE DU PRELEVEMENT A LA SOURCE de la paie `p` : son net imposable, plus l'avance des indemnites journalieres
/// subrogees. « Dans le cas d'une avance faite par un employeur en cas de subrogation des indemnites journalieres : le
/// montant de l'avance s'ajoute a la RNF pour constituer l'assiette du PAS » (S21.G00.50.013,
/// `norme-dsn/texte/bloc-S21.G00.50.txt`) ; cette avance-la, ce que le BOFiP en soumet (`maintien`,
/// `Assiette::IjSubrogeesAssiettePas`). Sans subrogation, aucune avance.
fn assiette_du_pas(p: &Paie) -> R<Montant> {
    let avance = p.bases.get(&droit::assiette::Assiette::IjSubrogeesAssiettePas).map(|b| b.en_euros(droit::assiette::Assiette::IjSubrogeesAssiettePas)).transpose()?;
    Ok(avance.into_iter().fold(p.net_imposable, |a, m| a + m))
}

/// L'IMPOT PRELEVE SUR LA PAIE `p` AU TAUX `taux` que son bulletin imprime (une paie importee, que ce moteur refait :
/// `ecarts::calculee`) : le taux transmis par la DGFiP s'applique tel quel a l'assiette de la paie
/// (`droit::pas::au_taux_transmis`), comme celui d'une paie validee.
pub fn prelevement_au_taux_imprime(p: &Paie, taux: droit::pas::TauxDePas) -> R<Montant> {
    Ok(droit::pas::au_taux_transmis(taux, assiette_du_pas(p)?).1)
}

/// Le prelevement a la source du versement d'une correction, verse avec le mois `mois_du_versement` : la difference de
/// net imposable entre la paie corrigee `apres` et ce qui a ete declare (`net_imposable_declare`).
///
/// BOFiP, BOI-IR-PAS-30-10-50 (`atelier/reference/sources/bofip/BOI-IR-PAS-30-10-50-extrait-regularisation.md`) :
/// - § 1 : un rappel de paie « ne donne pas lieu a regularisation, mais doit etre soumis au prelevement a la source et
///   porte dans la declaration souscrite au titre du mois au cours duquel est intervenu le versement du rappel » : la
///   difference qui monte se preleve comme un versement de ce mois-la, datee de sa fin (S21.G00.50.001), au taux DGFiP
///   qui vaut alors, sinon a la grille de l'article 204 H, avec l'abattement d'un contrat court ;
/// - § 90 : un trop-verse qui ne se compense pas (le versement de la correction est negatif) se regularise « au taux de
///   prelevement initialement utilise » : `taux_origine`, le taux du versement declare (S21.G00.56.006). Sans lui, pas
///   de prelevement (`None`) : la DSN refuse, le bulletin le dit. Son versement porte un montant soumis nul (fiche
///   1810, `droit::pas::du_versement`).
///
/// Un bareme sans grille refuse, comme pour une paie (regle 1).
pub fn prelevement_de_la_correction(
    ctx: &Contexte, apres: &Paie, net_imposable_declare: Montant, taux_origine: Option<&droit::pas::TauxDePas>, versement: Mois,
) -> R<PrelevementDeLaCorrection> {
    let (debut, fin) = (versement.premier_jour(), versement.dernier_jour());
    let difference = apres.net_imposable - net_imposable_declare;
    let e = &apres.engagement;
    let taux = droit::pas::taux_dgfip(ctx, &apres.salarie.id, Some(fin), Some(versement))?;
    let bareme = ctx.bareme(&debut)?;
    let court = droit::natures::contrat_court(&*e.nat()?, &e.contrat(), fin)?;
    let versement = droit::pas::du_versement(taux.as_ref(), &bareme, difference, Some(fin), court, droit::pas::ZONE_DEFAUT)?;
    let prelevement = if difference < Montant::ZERO {
        taux_origine.map(|t| (*t, t.prelevement(difference).au_centime()))
    } else {
        Some((versement.taux, versement.prelevement))
    };
    Ok(PrelevementDeLaCorrection { difference, versement, prelevement })
}

/// Ce que `prelevement_de_la_correction` rend.
pub struct PrelevementDeLaCorrection {
    /// La difference de net imposable (S21.G00.50.002 du versement de la correction).
    pub difference: Montant,
    /// Le taux, son type, son identifiant, le montant soumis et le prelevement du versement (S21.G00.50.006 a .009, .013).
    pub versement: droit::pas::Versement,
    /// Le taux applique et ce que la correction preleve (positif) ou rend (negatif) : celui du versement, ou pour un
    /// trop-verse le taux d'origine (S21.G00.56.006, .007) ; `None` : un trop-verse sans taux d'origine.
    pub prelevement: Option<(droit::pas::TauxDePas, Montant)>,
}

impl PrelevementDeLaCorrection {
    /// Un trop-verse : la difference baisse.
    pub fn indu(&self) -> bool {
        self.difference < Montant::ZERO
    }
}

/// La paie de l'engagement `o` pour `mois`, calculee maintenant : `{"paie", "fiches", "pas", "empreinte"}`.
pub(crate) fn calculer(ctx: &Contexte, o: &Objet, mois: Mois) -> R<PaieEcrite> {
    let i = rang_dans(ctx, o, mois)?;
    calculer_les_rangs(ctx, mois, &[i])?.pop().ok_or_else(|| refus("le calcul du mois n'a pas cette paie"))
}

/// Ce que la saisie `data` d'un mois pose pour tous ses engagements (comme `stock::empreinte_de_saisie` la separe).
fn mois_lu(data: &crate::saisie::VueDuMois) -> crate::saisie::CeQueLeMoisDit {
    crate::saisie::CeQueLeMoisDit { taux_cachet: data.taux_cachet.clone() }
}

/// Les paies des engagements de rangs `rangs` dans l'ecran de `mois`, d'un seul calcul du mois, au contexte du mois :
/// chacune `{"paie", "fiches", "saisie_du_mois", "pas", "empreinte"}`, dans l'ordre de `rangs`, avec l'empreinte de ce
/// dont son calcul depend.
///
/// UNE PAIE ECRITE NE LIT QUE DES ENGAGEMENTS VALIDES : le calcul se fait sur les engagements valides du mois et ceux
/// de `rangs`, comptes comme valides (`stock::empreinte_de_saisie` ne compte que ceux-la) ; un engagement en saisie
/// n'entre ni dans le calcul ni dans l'empreinte d'une paie ecrite. DES AUTRES ENGAGEMENTS DU MOIS, ELLE NE LIT QUE CEUX
/// DE SON SALARIE : le calcul ne prend qu'eux, et ce qui refuse la paie d'un salarie n'empeche pas celle d'un autre.
fn calculer_les_rangs(ctx: &Contexte, mois: Mois, rangs: &[RangDansLeMois]) -> R<Vec<PaieEcrite>> {
    let ctx = &ctx.du_mois(mois)?;
    let mut data = stock::vue_du_mois(&ctx.racine, mois)?.ok_or_else(|| socle::refus!("{} : aucune saisie", mois))?;
    let tous = std::mem::take(&mut data.engagements);
    let salaries: Vec<IdSalarie> =
        rangs.iter().map(|i| i.dans(&tous).and_then(|e| e.salarie.clone()).ok_or_else(|| socle::fichiers::absent("salarie"))).collect::<R<_>>()?;
    let mut gardes = Vec::new();
    // La place, dans le calcul, de chaque engagement du mois qu'il prend.
    let mut places: BTreeMap<RangDansLeMois, usize> = BTreeMap::new();
    for (i, mut e) in RangDansLeMois::de_chacun(tous) {
        if rangs.contains(&i) && e.valide.is_none() {
            e.valide = Some(crate::saisie::Validee::default());
        }
        let des_siens = e.salarie.as_ref().is_some_and(|s| salaries.contains(s));
        if des_siens && e.valide.is_some() {
            places.insert(i, gardes.len());
            gardes.push(e);
        }
    }
    data.engagements = gardes;
    let (calculees, mut entrees) = empreinte::suivre(ctx, || {
        let s = crate::charger_donnees(ctx, &data)?;
        rangs
            .iter()
            .map(|i| {
                let p = places.get(i).and_then(|j| s.paies.get(*j)).ok_or_else(|| refus("le calcul du mois n'a pas cette paie"))?;
                let conges = crate::conges::compteur_ecrit(ctx, p, mois)?;
                let (salarie, spectacle) = (&p.engagement.salarie, p.engagement.spectacle.as_ref());
                let fin = p.engagement.periode_payee()?.fin();
                let config = empreinte::configuration(ctx, &fin, salarie, spectacle.map(|s| s.as_str()))?;
                let entree_fiches = empreinte::fiches(ctx, mois, salarie, spectacle)?;
                Ok((relire::ecrire(p)?, relire::FichesDuCalcul::de(p), prelevement(ctx, p)?, conges, fin, config, entree_fiches))
            })
            .collect::<R<Vec<_>>>()
    })?;
    // DES AUTRES ENGAGEMENTS DE SON MOIS, UNE PAIE NE LIT QUE CEUX DE SON SALARIE (`calcul::dfs_deja_deduite`, sur
    // `precedentes`) ; `un_numero_un_salarie` refuse sans changer un chiffre. Des autres mois, elle ne lit que les paies
    // de son salarie (`paies <mois> <salarie>`, `reprises <mois> <salarie>`) : le calcul du mois les a lues pour tous.
    entrees.remove(&stock::cle_de_saisie(mois, None));
    let mois_lu = mois_lu(&data);
    calculees
        .into_iter()
        .zip(salaries)
        .map(|((paie, fiches, pas, conges, fin, config, (cle_fiches, entree_fiches)), salarie)| {
            let regles = empreinte::regles(ctx, &fin)?;
            let mut siennes = entrees.clone();
            // Les paies, les mensuels et le taux de PAS d'un autre salarie ne sont pas de son calcul.
            siennes.retain(|k, _| !matches!(k, CleLue::Datee(LectureDatee::Paies { salarie: autre, .. } | LectureDatee::PaiesAuMois { salarie: autre, .. } | LectureDatee::PaiesIntermittentes { salarie: autre, .. } | LectureDatee::Reprises { salarie: autre, .. } | LectureDatee::Mensuels { salarie: autre, .. } | LectureDatee::Pas { salarie: autre, .. }) if *autre != salarie));
            siennes.insert(stock::cle_de_saisie(mois, Some(&salarie)), ValeurLue::Saisie(stock::empreinte_de_saisie(&data, Some(&salarie))));
            siennes.insert(cle_fiches, entree_fiches);
            let (conges, conges_avant) = conges.unzip();
            Ok(PaieEcrite {
                paie,
                fiches,
                saisie_du_mois: mois_lu.clone(),
                empreinte: empreinte::ecrite(&siennes, &regles, &config),
                pas,
                conges,
                conges_avant,
                bulletin: None,
                ce: None,
                rectifie: None,
                recalcul: None,
            })
        })
        .collect()
}

/// Deux paies ecrites disent-elles les memes chiffres : tout sauf ce qu'elles disent en mots ? Rien ne se recopie : ce
/// qui est a refaire se compare a chaque lecture de l'etat d'un mois, pour chaque envoi parti.
fn chiffres_egaux(a: &PaieEcrite, b: &PaieEcrite) -> bool {
    egales_sans_avis(&a.paie, &b.paie) && a.pas == b.pas && a.conges_sans_avis() == b.conges_sans_avis()
}

/// Deux paies egales hors de leurs avertissements, ceux de leurs periodes compris, et hors de la part que l'abattement de
/// la CSG a abattue (`calcul::Contenu::part_abattue_csg`) : elle n'est ni payee, ni cotisee, ni declaree, et une paie ecrite
/// avant qu'elle s'ecrive ne la porte pas ; ce qu'elle change dans la paie (l'assiette de la CSG) se compare, lui. Une paie
/// dont aucun montant ne change ne se rectifie pas, et sa paie recalculee s'ecrit avec elle (`rafraichir`). Une ligne de
/// cotisation dont la base et les deux montants sont nuls ne porte aucun chiffre : qu'une paie l'imprime et l'autre non
/// (la tranche 2 ou la contribution d'equilibre technique qui ne sont pas dues, `calcul::calculer`) n'est pas un ecart.
/// Une assiette qu'une seule des deux paies ecrit (`Paie::bases`) ne se compare pas non plus : un calcul plus recent en
/// ecrit une de plus (les tranches de l'annee de la retraite d'un intermittent, `calcul::calculer`, 7.6), et ce qu'elle
/// change se lit dans les lignes et les montants, qui se comparent.
fn egales_sans_avis(a: &FormeDePaie<Avis>, b: &FormeDePaie<Avis>) -> bool {
    // `None` : les deux paies n'ont pas le meme nombre de periodes, elles different.
    fn sans_avis(p: &FormeDePaie<Avis>, autre: &FormeDePaie<Avis>) -> Option<FormeDePaie<Avis>> {
        if p.periodes.len() != autre.periodes.len() {
            return None;
        }
        Some(FormeDePaie {
            avertissements: Vec::new(),
            part_abattue_csg: None,
            lignes: p.lignes.iter().filter(|l| !(l.base.est_nulle() && l.montant_pat.est_nul() && l.montant_sal.est_nul())).cloned().collect(),
            bases: p.bases.iter().filter(|(a, _)| autre.bases.contains_key(*a)).map(|(a, b)| (*a, b.clone())).collect(),
            periodes: p.periodes.iter().zip(&autre.periodes).map(|(q, o)| sans_avis(q, o)).collect::<Option<_>>()?,
            ..p.clone()
        })
    }
    matches!((sans_avis(a, b), sans_avis(b, a)), (Some(x), Some(y)) if x == y)
}

/// Pourquoi la paie a change de `avant` a `apres` : son prelevement seul, une regle datee nouvelle, la saisie d'un autre
/// engagement ou du mois, la paie d'un autre mois, une valeur datee de la compagnie, le code qui calcule (le moteur ou ce
/// qu'il livre), ou la configuration de la compagnie.
///
/// UNE SEULE RAISON, LA MEME QUELLE QUE SOIT L'HEURE DE LA LECTURE. Une paie dont l'empreinte ne tient plus sans qu'un
/// chiffre change renouvelle son empreinte sans rien dire (`rafraichir`) : selon que l'ecran a lu le mois entre deux
/// gestes ou non, l'empreinte ecrite porte deja le code present ou encore l'ancien. Ce que la compagnie a fait (une
/// saisie, une paie d'un autre mois, une valeur datee) dit donc pourquoi avant le code, qui ne se nomme que lorsque rien
/// d'autre n'a change. Une paie partie (`parti`) ne bouge que par une decision de l'editeur : le code et les regles
/// datees s'y nomment d'abord, puisque ce sont eux qui la refont.
fn raison(avant: &PaieEcrite, apres: &PaieEcrite, parti: bool) -> R<RaisonDuRecalcul> {
    if egales_sans_avis(&avant.paie, &apres.paie) && avant.conges_sans_avis() == apres.conges_sans_avis() && avant.pas != apres.pas {
        return Ok(RaisonDuRecalcul::TauxDePas);
    }
    let vieilles = &avant.empreinte.regles;
    if let Some(r) = apres.empreinte.regles.iter().filter(|r| !vieilles.contains(r)).max() {
        return Ok(RaisonDuRecalcul::RegleDu(*r));
    }
    let (ea, eb) = (&avant.empreinte.entrees, &apres.empreinte.entrees);
    let changees: Vec<&CleLue> = ea.keys().chain(eb.keys()).filter(|k| ea.get(*k) != eb.get(*k)).collect();
    // Le code a change : le moteur, ou un fichier livre que les deux calculs ont lu. Un fichier livre que seul l'un des
    // deux a lu dit que le calcul a pris un autre chemin (un arret saisi lit le bareme des arrets), pas que le code a change.
    let code = changees.iter().any(|k| matches!(k, CleLue::Moteur) || (empreinte::est_du_code(k) && ea.contains_key(*k) && eb.contains_key(*k)));
    if code && parti {
        return Ok(RaisonDuRecalcul::CalculCorrige);
    }
    // La saisie dont la paie depend : ce que le mois pose et les engagements de son salarie (`stock::empreinte_de_saisie`),
    // lue des deux cotes ; une saisie que l'une des deux paies n'a pas lue ne dit pas qu'un engagement a change.
    let saisies: Vec<&CleLue> = changees.iter().copied().filter(|k| matches!(k, CleLue::Datee(LectureDatee::Saisie { .. })) && ea.contains_key(*k) && eb.contains_key(*k)).collect();
    let engagements = |e: &BTreeMap<CleLue, ValeurLue>, k: &CleLue| match e.get(k) {
        Some(ValeurLue::Saisie(s)) => Some(s.engagements.clone()),
        _ => None,
    };
    if saisies.iter().any(|k| engagements(ea, k) != engagements(eb, k)) || changees.iter().any(|k| matches!(k, CleLue::Datee(LectureDatee::Mensuels { .. }))) {
        return Ok(RaisonDuRecalcul::AutreEngagement);
    }
    if !saisies.is_empty() {
        return Ok(RaisonDuRecalcul::SaisieDuMois);
    }
    if changees.iter().any(|k| du_magasin_des_imprevus(k)) {
        return Ok(RaisonDuRecalcul::Imprevu);
    }
    // L'indemnite de mission d'un jour a l'etranger que la table livree dit autrement : le code qui calcule.
    if changees.iter().any(|k| matches!(k, CleLue::Datee(LectureDatee::Mission { .. }))) {
        return Ok(RaisonDuRecalcul::CalculCorrige);
    }
    // Les paies d'un autre mois que le calcul lit (conges, cumuls, salaire de reference) : `paies <le mois> <son
    // salarie>`, les validees ; `reprises <le mois> <son salarie>`, celles d'un ancien logiciel.
    let premier = changees.iter().filter_map(|k| match k {
        CleLue::Datee(LectureDatee::Paies { mois, .. } | LectureDatee::PaiesAuMois { mois, .. } | LectureDatee::PaiesIntermittentes { mois, .. } | LectureDatee::Reprises { mois, .. }) => Some(*mois),
        _ => None,
    }).min();
    if let Some(m) = premier {
        return Ok(RaisonDuRecalcul::PaieDUnAutreMois(m));
    }
    // La configuration de la compagnie : une valeur datee de ses fiches qui s'applique, ou son bareme de l'annee
    // (`droit::contexte::surcouche` : ses choix de bareme, qui valent pour toute l'annee). Une paie peut lire celle d'une
    // autre annee que son mois (une ligne de frais datee de decembre dans la paie de janvier).
    let config_avant = &avant.empreinte.configuration;
    let config_nouvelle = apres.empreinte.configuration.iter().filter(|r| !config_avant.contains(r)).max().cloned();
    let annee_du_bareme = changees
        .iter()
        .find_map(|k| match k.chemin() {
            Some(socle::fichiers::CheminLu::Racine(Some(c))) => droit::contexte::annee_de_la_surcouche(c),
            _ => None,
        })
        .map(|an| an.premier_jour());
    if let Some(jour) = config_nouvelle.or(annee_du_bareme) {
        return Ok(RaisonDuRecalcul::RegleDu(jour));
    }
    if code {
        return Ok(RaisonDuRecalcul::CalculCorrige);
    }
    Ok(RaisonDuRecalcul::Configuration)
}

/// Le moment d'un geste, a l'heure du moteur.
fn maintenant() -> Moment {
    socle::temps::secondes().a_la_seconde()
}


/// La saisie d'un engagement telle qu'une version la garde.
fn saisie_de(o: &Objet) -> stock::Saisie {
    o.brut.saisie.clone()
}

/// Ce qu'une paie ecrite a lu hors de la saisie de son engagement : la fiche du salarie (`fiches`) et la saisie du mois
/// (`MOIS_LU`). Ce qui a ete declare se relit dessus (`paie_declaree`) : qu'il change, et la version change.
fn lu(p: &PaieEcrite) -> (&relire::FichesDuCalcul, &crate::saisie::CeQueLeMoisDit) {
    (&p.fiches, &p.saisie_du_mois)
}

/// Les paies `b` (`{<mois>: <paie ecrite>}`) disent-elles, pour chaque mois de `a`, les memes chiffres, lus sur les memes
/// fiches et la meme saisie du mois ? Un mois que `b` ajoute (un contrat sans terme qui entre dans un mois de plus) ne
/// change rien de ce que disait `a`.
fn memes_chiffres(a: &BTreeMap<Mois, PaieEcrite>, b: &BTreeMap<Mois, PaieEcrite>) -> bool {
    a.iter().all(|(m, p)| b.get(m).is_some_and(|q| chiffres_egaux(p, q) && lu(p) == lu(q)))
}

/// LA VERSION QUE PREND LE PRESENT quand la validation `actuelle` de `o` est remplacee le `le` : si un envoi est parti
/// sur elle (`siens`), elle se garde, datee, dans les versions de l'engagement, ses paies sous son numero dans le fichier
/// de leur mois (`paies::garder_la_version`), et le present prend le numero suivant ; sinon le present garde son numero
/// `n`, sans historique.
fn nouvelle_version(o: &mut Objet, actuelle: &stock::Validation, n: VersionDeLEngagement, siens: &[Depart], le: Moment) -> R<VersionDeLEngagement> {
    if !siens.iter().any(|d| d.version == n) {
        return Ok(n);
    }
    crate::paies::garder_la_version(o, n)?;
    let passee = stock::VersionPassee { version: n, valide_le: actuelle.le, remplacee_le: le, saisie: actuelle.saisie.clone() };
    o.brut_mut().versions.push(passee);
    Ok(n.suivante())
}

/// « VALIDER L'ENGAGEMENT » : l'engagement de rang `rang` dans l'ecran de `mois` ecrit sa saisie telle qu'elle est, et
/// ses paies, une par mois qu'il couvre jusqu'au present (`mois_present`), ou court un contrat sans terme ; un mois
/// d'apres aura la sienne quand il sera le present. Chacune avec son
/// empreinte, dans le fichier de son mois (`paies`) ; une paie de la validation d'avant dont le mois n'est plus couvert
/// s'efface. La paie d'un mois dont les documents sont partis se reprend de la validation d'avant (`precedente`) si la
/// saisie de son mois n'a pas change. Une validation qui change ce qu'un envoi a emporte fait une version
/// (`nouvelle_version`) ; une validation qui ne change rien garde le numero. Refus : un mois qui ne se calcule pas ; rien
/// n'est ecrit.
pub fn valider(ctx: &Contexte, mois: Mois, rang: RangDansLeMois, departs: &Departs) -> R<Valide> {
    let mut o = stock::objet_du_mois(&ctx.racine, mois, rang)?.ok_or_else(|| socle::refus!("{} : pas d'engagement de rang {}", mois, rang))?;
    // Le numero du contrat d'abord : la paie de chaque mois le porte, et le calcul lit l'engagement ecrit. Un calcul
    // refuse rend l'engagement tel qu'il etait.
    let avant = o.clone();
    let le = maintenant();
    crate::numeros::a_la_validation(ctx, &mut o, le, departs)?;
    let numerote = o.brut != avant.brut;
    if numerote {
        stock::reecrire(&ctx.racine, &o)?;
    }
    let siens = ses_departs(&o, departs).to_vec();
    // Une validation qui ne dit pas sa version ou sa saisie (`a_revalider`) ne se reprend pas : celle-ci s'ecrit entiere,
    // en version 1.
    let deja = o.brut.precedente.clone().or_else(|| o.brut.valide.clone());
    let precedente = deja.clone().filter(|p| a_revalider(ctx, &o, stock::Version::Presente(p)).is_none());
    // Les paies de la validation d'avant, sous le numero qu'elle portait.
    let anciennes = if deja.is_some() { crate::paies::presentes(&avant)? } else { BTreeMap::new() };
    let gardees = if precedente.is_some() { anciennes.clone() } else { BTreeMap::new() };
    let paies = match paies_a_valider(ctx, &o, mois, &gardees, &siens) {
        Ok(p) => p,
        Err(e) => {
            if numerote {
                stock::reecrire(&ctx.racine, &avant)?;
            }
            return Err(e);
        }
    };
    let saisie = saisie_de(&o);
    // Une validation qui ne change rien garde sa version et la date de celle-ci : la version est la meme.
    let (version, le) = match precedente.as_ref().and_then(|p| Some((p, p.version?))) {
        Some((p, n)) if p.saisie.as_ref() != Some(&saisie) || !memes_chiffres(&gardees, &paies) => (nouvelle_version(&mut o, p, n, &siens, le)?, le),
        Some((p, n)) => (n, p.le),
        None => (VersionDeLEngagement::premiere(), le),
    };
    // Sous un autre numero, la validation d'avant n'a plus de paies ; sous le meme, ses mois que celle-ci ne couvre plus.
    if deja.is_some() && avant.carte().numero != o.carte().numero {
        crate::paies::effacer(&avant)?;
    } else {
        for m in anciennes.keys().filter(|m| !paies.contains_key(*m)) {
            crate::paies::poser(&o, *m, None)?;
        }
    }
    // Le premier mois que la validation a refait (ou ecrit) : ce qui le suit reprend son report.
    let premier_refait = paies.iter().find(|(m, p)| anciennes.get(*m) != Some(*p)).map(|(m, _)| *m);
    for (m, p) in paies {
        crate::paies::poser(&o, m, Some(p))?;
    }
    let e = o.brut_mut();
    e.precedente = None;
    e.valide = Some(stock::Validation { le, version: Some(version), saisie: Some(saisie) });
    stock::reecrire(&ctx.racine, &o)?;
    if let Some(premier) = premier_refait {
        reporter(ctx, &o, premier, departs)?;
    }
    Valide::de(o).ok_or_else(|| refus("validation illisible"))
}

/// Les paies que la validation de `o` ecrit, une par mois qu'il couvre jusqu'au present (`mois_present`). `anciennes` : celles de la
/// validation qu'elle remplace ; `siens` : ce qui est parti de l'engagement.
fn paies_a_valider(ctx: &Contexte, o: &Objet, mois: Mois, anciennes: &BTreeMap<Mois, PaieEcrite>, siens: &[Depart]) -> R<BTreeMap<Mois, PaieEcrite>> {
    let mut paies = BTreeMap::new();
    let en_cours = EnValidation::ouvrir(o);
    let present = mois_present(ctx);
    // Un contrat sans terme court jusqu'au present au moins.
    for m in o.mois_couverts(present.max(mois))?.into_iter().filter(|m| *m <= present) {
        let partis = documents_partis(siens, m).is_some();
        let garde = match anciennes.get(&m) {
            Some(f) if partis => {
                let salarie = o.carte().salarie.clone();
                let saisie = saisie_comptee_valide(ctx, o, m, salarie.as_ref())?.map(ValeurLue::Saisie);
                let ecrite = f.empreinte.entrees.get(&stock::cle_de_saisie(m, salarie.as_ref())).cloned();
                (saisie.is_some() && saisie == ecrite && imprevus_tiennent(ctx, f)?).then_some(f)
            }
            _ => None,
        };
        let p = match garde {
            Some(f) => f.clone(),
            None => {
                let mut p = calculer(ctx, o, m)?;
                match anciennes.get(&m).filter(|_| partis) {
                    // Une paie partie, refaite aux memes chiffres : le bulletin parti reste juste, elle garde ses numeros.
                    Some(f) if chiffres_egaux(f, &p) => garder_les_numeros(f, &mut p),
                    // Une paie partie, refaite : son bulletin sera un rectificatif de celui qui est parti.
                    Some(f) => {
                        if let Some(parti) = f.bulletin.clone().or_else(|| f.rectifie.clone()) {
                            p.rectifie = Some(parti);
                        }
                    }
                    None => {}
                }
                p
            }
        };
        en_cours.calculee(m, &p);
        paies.insert(m, p);
    }
    Ok(paies)
}

/// Une paie refaite aux memes chiffres que la paie partie `f` garde ses numeros : son bulletin, son premier N CE, le
/// bulletin qu'elle rectifie.
fn garder_les_numeros(f: &PaieEcrite, p: &mut PaieEcrite) {
    if f.bulletin.is_some() {
        p.bulletin = f.bulletin.clone();
    }
    if f.ce.is_some() {
        p.ce = f.ce;
    }
    if f.rectifie.is_some() {
        p.rectifie = f.rectifie.clone();
    }
}

/// Les imprevus que la paie ecrite `f` a lus (le magasin, `imprevus::DOSSIER`) valent-ils encore ce qu'ils valaient ?
/// Un arret saisi ou corrige apres le depart du mois les change : la paie partie se refait alors a la revalidation, et
/// l'ecart passe en regularisation (`apres_un_imprevu`).
fn imprevus_tiennent(ctx: &Contexte, f: &PaieEcrite) -> R<bool> {
    let entrees: BTreeMap<CleLue, ValeurLue> = f.empreinte.entrees.iter().filter(|(k, _)| du_magasin_des_imprevus(k)).map(|(k, v)| (k.clone(), v.clone())).collect();
    empreinte::tient(ctx, &entrees, &mut empreinte::Relues::default())
}

/// Une lecture du magasin des imprevus : les imprevus d'un salarie, ou un chemin du magasin.
fn du_magasin_des_imprevus(k: &CleLue) -> bool {
    matches!(k, CleLue::Imprevus { .. }) || k.chemin().is_some_and(|c| c.dans_la_compagnie(&crate::imprevus::DOSSIER))
}

/// Ce que la revalidation d'un engagement, apres un imprevu change (`apres_un_imprevu`), a donne.
pub enum Revalidation {
    /// Une paie a change : l'ecart d'une paie partie se regularise
    /// (`a_refaire` : le bulletin rectificatif, la DSN qui le porte).
    Changee(Valide),
    /// La paie partie ne se calcule pas encore (une indemnite a dire...) : qui, et le refus du calcul. Rien n'est ecrit
    /// pour lui ; la revalidation se refait au geste suivant sur l'imprevu.
    Attend(String, Echec),
}

/// UN IMPREVU CHANGE APRES LE DEPART DU MOIS (un arret saisi, ses indemnites corrigees, une absence) : chaque engagement
/// valide de `salarie` dont des documents sont partis se revalide (`valider`), comme une modification, dans son
/// brouillon. Une paie partie dont les imprevus ont change se refait (`imprevus_tiennent`) : si un chiffre change,
/// l'engagement prend une version nouvelle, la version d'avant garde ce qui est parti, et l'ecart suit le chemin de
/// toute correction d'un mois parti (un bulletin rectificatif verse avec une paie suivante ; la DSN du mois tant que
/// l'annule-et-remplace est admis, sinon la DSN qui porte la correction). Le mois parti ne se redepose pas. Rend les
/// engagements dont une paie a change, et ceux qui attendent.
pub fn apres_un_imprevu(ctx: &Contexte, salarie: &IdSalarie, departs: &Departs) -> R<Vec<Revalidation>> {
    let mut out = Vec::new();
    for o in stock::objets(&ctx.racine)? {
        if !o.est_valide() || o.est_non_actif() || o.est_importe() || o.carte().salarie.as_ref() != Some(salarie) {
            continue;
        }
        if !ses_departs(&o, departs).iter().any(|d| d.envoi == Envoi::Documents) {
            continue;
        }
        let mois = o.premier_mois();
        let rang = rang_dans(ctx, &o, mois)?;
        let avant = crate::paies::presentes(&o)?;
        let brouillon = stock::Brouillon::ouvrir(&ctx.racine);
        match valider(ctx, mois, rang, departs) {
            Ok(v) => {
                brouillon.ecrire()?;
                if !memes_chiffres(&avant, &crate::paies::presentes(v.objet())?) {
                    out.push(Revalidation::Changee(v));
                }
            }
            Err(m) => out.push(Revalidation::Attend(qui_de(ctx, &o), m)),
        }
    }
    Ok(out)
}

/// L'empreinte de la saisie de `mois` dont la paie de l'engagement `o` (de `salarie`) depend, `o` compte comme valide,
/// comme son calcul le compte (`calculer_les_rangs`) : celle qu'une paie ecrite de `o` porte si ce que le mois dit d'elle
/// n'a pas change.
fn saisie_comptee_valide(ctx: &Contexte, o: &Objet, mois: Mois, salarie: Option<&IdSalarie>) -> R<Option<socle::fichiers::EmpreinteDeSaisie>> {
    let Some(mut data) = stock::vue_du_mois(&ctx.racine, mois)? else { return Ok(None) };
    let i = rang_dans(ctx, o, mois)?;
    let Some(e) = i.dans_mut(&mut data.engagements) else { return Ok(None) };
    if e.valide.is_none() {
        e.valide = Some(crate::saisie::Validee::default());
    }
    Ok(Some(stock::empreinte_de_saisie(&data, salarie)))
}

/// L'engagement de rang `rang` de l'ecran de `mois`, rouvert (`modifier`), est-il a revalider ? Sa saisie dit autre
/// chose que sa validation mise de cote, ou celle-ci ne dit pas tout ce qu'un envoi emporte (`a_revalider`). Faux pour un
/// engagement qui n'a pas ete rouvert.
pub fn a_revalider_rouvert(ctx: &Contexte, mois: Mois, rang: RangDansLeMois) -> R<bool> {
    Ok(stock::objet_du_mois(&ctx.racine, mois, rang)?.is_some_and(|o| {
        o.brut.precedente.as_ref().is_some_and(|p| p.saisie.as_ref() != Some(&saisie_de(&o)) || a_revalider(ctx, &o, stock::Version::Presente(p)).is_some())
    }))
}

/// LE MOIS OU LE CONTRAT CHANGE entre la version `passee` d'un engagement valide (sa saisie gardee) et son present : le
/// premier mois dont les clauses (`CLAUSES`) different. C'est de ce mois que l'avenant prend effet (R1221-40 : « au plus
/// tard a la date de prise d'effet de cette modification », `atelier/reference/sources/code-du-travail/
/// articles-20261029.md`, l. 856). `None` : aucun mois ne change.
pub fn mois_du_changement(v: &Valide, passee: stock::Version) -> R<Option<Mois>> {
    let Some(saisie) = passee.saisie() else { return Ok(None) };
    let ancien = v.objet.d_apres_la_saisie(saisie);
    let mensuel = v.objet.mensuel()?;
    // Les mois ou l'une des versions a une paie (`paies::mois_ecrits`), et ceux que l'une des saisies a touches.
    let mut mois: Vec<Mois> = crate::paies::mois_ecrits(&v.objet)?.into_iter().chain(v.objet.mois_saisis()).chain(ancien.mois_saisis()).collect();
    mois.sort();
    mois.dedup();
    let garder = |x: Option<Carte>| x.map(|c| clauses_de(&c, mensuel));
    for m in mois {
        if garder(v.objet.vue(m)?) != garder(ancien.vue(m)?) {
            return Ok(Some(m));
        }
    }
    Ok(None)
}

/// « MODIFIER… » : l'engagement repasse en saisie. Sa validation se garde (`precedente`) pour la suivante.
pub fn modifier(ctx: &Contexte, mois: Mois, rang: RangDansLeMois) -> R<EnSaisie> {
    let mut o = stock::objet_du_mois(&ctx.racine, mois, rang)?.ok_or_else(|| socle::refus!("{} : pas d'engagement de rang {}", mois, rang))?;
    if let Some(v) = o.brut_mut().valide.take() {
        o.brut_mut().precedente = Some(v);
        stock::reecrire(&ctx.racine, &o)?;
    }
    Ok(EnSaisie { objet: o })
}

// ------------------------------------------------------------------ supprimer, ou ne plus etre actif

/// CE QUI EST PARTI de l'engagement `o`, dit en mots (« la DPAE, le 02/03/2026 ») : ce que la couche des envois dit
/// (`departs`), et une version gardee parce qu'un envoi l'avait emportee. Vide : rien n'est parti, il se supprime.
pub fn ce_qui_est_parti(o: &Objet, departs: &Departs) -> Vec<String> {
    let mut out: Vec<String> = ses_departs(o, departs)
        .iter()
        .map(|d| format!("{}, le {}", crate::versions::quoi_du_depart(d), d.le.jour().jj_mm_aaaa()))
        .collect();
    out.dedup();
    if out.is_empty() && !o.brut.versions.is_empty() {
        out.push("une version qu'un envoi a emportee".to_string());
    }
    out
}

/// SUPPRIMER L'ENGAGEMENT `o` (celui qu'une carte montre, en saisie ou valide) : tant que rien n'est parti de lui
/// (`ce_qui_est_parti` vide), il s'efface, meme valide, et rend son numero (`numeros` : le suivant se tire de ce que portent les
/// engagements et ce qui est parti). Refus des qu'un envoi est parti : il ne se supprime plus, il peut ne plus etre
/// actif (`desactiver`) ; rien n'est ecrit.
pub fn supprimer(ctx: &Contexte, o: &Objet, departs: &Departs) -> R<()> {
    let partis = ce_qui_est_parti(o, departs);
    if !partis.is_empty() {
        return Err(socle::refus!(
            "{} : déjà parti : {}. Il ne se supprime plus ; il peut devenir non actif : il reste dans la compagnie, avec ses versions, et ne compte plus dans aucune paie ni aucune déclaration. Rien n'est effacé.",
            qui_de(ctx, o),
            partis.join(" ; "),
        ));
    }
    crate::paies::effacer(o)?;
    stock::effacer(&ctx.racine, o)?;
    Ok(())
}

/// Un engagement qui n'est plus actif (`stock::NON_ACTIF`) : il garde ses versions, ne se calcule plus, n'entre dans
/// aucun document ni aucune declaration ; il se lit.
#[derive(Debug, Clone)]
pub struct NonActif {
    objet: Objet,
    le: Moment,
    version: VersionDeLEngagement,
}

impl NonActif {
    /// L'engagement non actif `o`, tel qu'il est ecrit ; `None` s'il est actif.
    pub fn de(objet: Objet) -> Option<NonActif> {
        let na = objet.brut.non_actif.clone()?;
        Some(NonActif { objet, le: na.le, version: na.version })
    }

    /// Sa version : celle qui suit la derniere validee. Son present est : rien de paye ; une correction qui le declare
    /// part sur elle.
    pub fn version(&self) -> VersionDeLEngagement {
        self.version
    }

    pub fn objet(&self) -> &Objet {
        &self.objet
    }

    /// Depuis quand.
    pub fn le(&self) -> Moment {
        self.le
    }

    /// CE QUE SA DESACTIVATION REND FAUX, tire de ce qui est parti (`departs`) comparé a son present, qui est : rien.
    /// Chaque mois que la derniere DSN qui l'a declare a declare sur une autre version que la sienne est a corriger
    /// (`Dsn`) : la couche des declarations dit comment (l'annule-et-remplace du mois qui ne le porte plus, ou la
    /// correction qui annule ce qui a ete declare). Un contrat, des documents, un signalement partis ne demandent rien.
    /// Une DPAE partie non plus : aucun texte ne l'annule, une DPAE se corrige par une nouvelle declaration (fiche
    /// net-entreprises 3300, `atelier/reference/sources/net-entreprises-base-connaissances/
    /// net-entreprises-fiche3300-dpae-en-dsn-20261015.txt`, l. 451 ; guide de l'API DPAE,
    /// `atelier/reference/norme-dpae/5491-API-DPAE-Fonctionnement.pdf`, p. 3 : pas d'annule-et-remplace).
    pub fn a_refaire(&self, departs: &Departs) -> Vec<ARefaire> {
        let siens = ses_departs(&self.objet, departs);
        let mut out = Vec::new();
        let mut mois: BTreeMap<Mois, VersionDeLEngagement> = BTreeMap::new();
        for d in siens.iter().filter(|d| d.envoi == Envoi::Dsn) {
            if let Some(m) = d.mois {
                mois.insert(m, d.version);
            }
        }
        out.extend(mois.into_iter().filter(|(_, depuis)| *depuis != self.version).map(|(mois, depuis)| ARefaire::Dsn { mois, depuis }));
        out
    }

    /// LA CORRECTION de `mois` que la DSN de `porteur` doit porter pour lui : ce qui a ete declare pour ce mois par les
    /// DSN anterieures a `porteur` (`paies_declarees`), quand la derniere ne l'a pas declare non actif ; `None` sinon.
    pub fn correction_due(&self, ctx: &Contexte, departs: &Departs, mois: Mois, porteur: Mois) -> R<Option<Vec<Declaree>>> {
        let declarees = paies_declarees(ctx, &self.objet, departs, mois, Some(porteur))?;
        let due = declarees.last().is_some_and(|d| d.version != self.version);
        Ok(due.then_some(declarees))
    }
}

/// « NE PLUS ETRE ACTIF » : l'engagement de rang `rang` dans l'ecran de `mois`, dont quelque chose est parti, garde sa
/// validation parmi ses versions (datee du `le` de la desactivation) et devient non actif. Refus quand rien n'est parti :
/// il se supprime (`supprimer`) ; rien n'est ecrit.
pub fn desactiver(ctx: &Contexte, mois: Mois, rang: RangDansLeMois, departs: &Departs) -> R<NonActif> {
    let mut o = stock::objet_du_mois(&ctx.racine, mois, rang)?.ok_or_else(|| socle::refus!("{} : pas d'engagement de rang {}", mois, rang))?;
    if ce_qui_est_parti(&o, departs).is_empty() {
        return Err(socle::refus!("{} : rien n'est parti de cet engagement ; il se supprime, il ne devient pas non actif. Rien n'est écrit.", qui_de(ctx, &o)));
    }
    let le = maintenant();
    // La validation presente : la derniere version, gardee telle quelle.
    let derniere = o.brut_mut().valide.take();
    // Sa version suit la derniere validee : son present, rien de paye, est une version de plus.
    let version = match o.brut.versions.iter().map(|v| v.version).chain(derniere.as_ref().and_then(|v| v.version)).max() {
        Some(v) => v.suivante(),
        None => VersionDeLEngagement::premiere(),
    };
    if let Some((v, n)) = derniere.and_then(|v| Some((v.clone(), v.version?))) {
        crate::paies::garder_la_version(&o, n)?;
        o.brut_mut().versions.push(stock::VersionPassee { version: n, valide_le: v.le, remplacee_le: le.clone(), saisie: v.saisie });
    }
    // Rien de paye : il n'a plus de paie presente.
    for m in crate::paies::mois_ecrits(&o)? {
        crate::paies::poser(&o, m, None)?;
    }
    o.brut_mut().non_actif = Some(stock::NonActif { le, version });
    stock::reecrire(&ctx.racine, &o)?;
    NonActif::de(o).ok_or_else(|| refus("desactivation illisible"))
}

/// Les engagements non actifs que `mois` montrerait, tels qu'ils sont ecrits.
pub fn non_actifs_du_mois(ctx: &Contexte, mois: Mois) -> R<Vec<NonActif>> {
    Ok(stock::non_actifs_du_mois(&ctx.racine, mois)?.into_iter().filter_map(NonActif::de).collect())
}

/// LE RECALCUL DE LUI-MEME, a la lecture de `mois` (`du_mois`) : chaque engagement valide du mois dont la paie de ce mois
/// n'est plus juste (son empreinte ne tient plus, ou elle manque : un contrat sans terme qui entre dans un nouveau mois)
/// se recalcule et se reecrit. Un chiffre qui change note pourquoi, et quand ; rien d'autre ne change. Si un envoi a
/// emporte la paie de ce mois (sa DSN, ses documents), le chiffre qui change fait une version datee (`nouvelle_version`) :
/// la version d'avant garde ce qui est parti.
///
/// Un mois dont les documents sont partis ne bouge que par une decision de l'editeur (le code qui calcule, une regle
/// datee : `empreinte::code_ou_regle_change`, puis la raison du recalcul), et la paie refaite rectifie le bulletin parti.
/// Un taux de PAS recu, une configuration, une autre saisie ne touchent pas un mois parti : un premier taux personnalise
/// recu apres la grille « ne s'applique que pour l'avenir », sans regularisation (BOI-IR-PAS-30-10-50, § 20,
/// `atelier/reference/sources/bofip/BOI-IR-PAS-30-10-50-20190227.html`). Un mois fini (`fini`) ne se regarde qu'au
/// passage d'une version (`passage`, `corriger_les_mois_finis`) ; un mois d'apres le present n'a pas de paie
/// (`mois_present`).
fn rafraichir(ctx: &Contexte, mois: Mois, departs: &Departs, passage: bool) -> R<Rafraichi> {
    // Au contexte du mois, quel que soit le jour de celui qui lit : ce que l'empreinte relit (les fiches) vaut au mois paye.
    let present = mois_present(ctx);
    let ctx = &ctx.du_mois(mois)?;
    // Ce qui ne tient plus : le rang dans l'ecran, l'engagement, sa paie ecrite du mois, et si ses documents sont partis.
    let mut a_refaire: Vec<(RangDansLeMois, Objet, Option<PaieEcrite>, bool)> = Vec::new();
    let mut relues = empreinte::Relues::default();
    let mut reecrit = false;
    // CE DONT UNE PAIE DEPEND SE LIT TEL QU'IL EST ECRIT : la paie d'un mois d'avant qu'elle a lue (`paies <mois>
    // <salarie>`) se compare a son empreinte sans se recalculer d'abord. Qu'elle se reecrive, et l'empreinte de celle-ci
    // ne tient plus : elle se refait a sa prochaine lecture.
    let etats = tels_qu_ecrits(ctx, mois, departs)?;
    for (i, e) in RangDansLeMois::de_chacun(&etats) {
        let (v, parti) = match e {
            Etat::Valide(v) => (v, false),
            Etat::Partis(p) => (p.valide(), true),
            Etat::EnSaisie(_) => continue,
        };
        let ancienne = v.paie_ecrite(mois)?;
        // Une paie qui manque (un contrat sans terme qui entre dans le mois) s'ecrit.
        let parti = parti && ancienne.is_some();
        // UN MOIS FINI NE SE RELIT PAS : sa paie se lit telle qu'elle est ecrite (`fini`), sauf au passage d'une version
        // (`corriger_les_mois_finis`), ou il est un mois parti comme un autre.
        if ancienne.is_some() && !passage && fini(ses_departs(&v.objet, departs), mois) {
            continue;
        }
        // AUCUNE PAIE POUR UN MOIS QUI N'EST PAS ENCORE LA : elle s'ecrira quand il sera le present, avec ses regles.
        if ancienne.is_none() && mois > present {
            continue;
        }
        let empreinte_ecrite = ancienne.as_ref().map(|a| &a.empreinte);
        let change = empreinte_ecrite.filter(|_| parti).map(|e| empreinte::code_ou_regle_change(ctx, e, mois, &mut relues)).transpose()?.is_some_and(|c| c);
        // Partie, elle ne bouge que par une decision de l'editeur : on la cherche d'abord (quelques entrees), et
        // l'empreinte entiere ne se verifie que si elle a lieu.
        if parti && !change {
            // Partie, elle ne bouge pas ; le compteur avant le mois, que le mois suivant reprend, suit les mois d'avant.
            if let Some(a) = ancienne.as_ref().filter(|a| a.conges_avant.is_some()) {
                if let Some(nouvelle) = conges_avant_a_jour(ctx, &v.objet, a, mois, &mut relues)? {
                    crate::paies::poser(&v.objet, mois, Some(nouvelle))?;
                    reecrit = true;
                }
            }
            continue;
        }
        let tient = empreinte_ecrite.map(|en| empreinte::tient(ctx, &en.entrees, &mut relues)).transpose()?.is_some_and(|t| t);
        if tient {
            continue;
        }
        a_refaire.push((i, v.objet.clone(), ancienne.as_deref().cloned(), parti));
    }
    if a_refaire.is_empty() {
        return Ok(Rafraichi { etats: (!reecrit).then_some(etats), refus: Vec::new() });
    }
    // UN CALCUL PAR SALARIE (`calculer_les_rangs`) : le refus de l'un est le sien, les paies des autres s'ecrivent.
    let mut salaries: Vec<IdSalarie> = Vec::new();
    for (_, o, _, _) in &a_refaire {
        let s = o.carte().salarie.clone().ok_or_else(|| socle::fichiers::absent("salarie"))?;
        if !salaries.contains(&s) {
            salaries.push(s);
        }
    }
    let mut refus: Vec<(RangDansLeMois, Echec)> = Vec::new();
    let mut calculees: Vec<((RangDansLeMois, Objet, Option<PaieEcrite>, bool), PaieEcrite)> = Vec::new();
    for s in &salaries {
        let siens: Vec<(RangDansLeMois, Objet, Option<PaieEcrite>, bool)> =
            a_refaire.iter().filter(|(_, o, _, _)| o.carte().salarie.as_ref() == Some(s)).cloned().collect();
        let rangs: Vec<RangDansLeMois> = siens.iter().map(|(i, _, _, _)| *i).collect();
        match calculer_les_rangs(ctx, mois, &rangs) {
            Ok(nouvelles) => calculees.extend(siens.into_iter().zip(nouvelles)),
            Err(m) => refus.extend(rangs.into_iter().map(|i| (i, m.clone()))),
        }
    }
    for ((_, mut o, ancienne, parti), mut nouvelle) in calculees {
        let raison = match &ancienne {
            Some(a) if !chiffres_egaux(a, &nouvelle) => Some(raison(a, &nouvelle, parti)?),
            Some(a) => {
                if a.recalcul.is_some() {
                    nouvelle.recalcul = a.recalcul.clone();
                }
                None
            }
            None => None,
        };
        let le = maintenant();
        if let Some(r) = &raison {
            nouvelle.recalcul = Some(Recalcul { le: le.clone(), raison: r.to_string() });
        }
        match (&raison, parti, &ancienne) {
            // Partie, sans chiffre qui change : l'empreinte se renouvelle, la paie garde ses numeros.
            (None, true, Some(a)) => garder_les_numeros(a, &mut nouvelle),
            // Partie, et une decision de l'editeur la change : un bulletin rectificatif.
            (Some(r), true, a) if r.decision_de_l_editeur() => {
                if let Some(b) = a.as_ref().and_then(|a| a.bulletin.clone().or_else(|| a.rectifie.clone())) {
                    nouvelle.rectifie = Some(b);
                }
            }
            // Partie, et autre chose que l'editeur la changerait : elle ne bouge pas.
            (Some(_), true, _) => continue,
            (_, _, a) => {
                if let Some(r) = a.as_ref().and_then(|a| a.rectifie.clone()) {
                    nouvelle.rectifie = Some(r);
                }
            }
        }
        // Un chiffre, ou ce que la paie a lu (la fiche, la saisie du mois), qui change sur une paie qu'un envoi a emportee
        // (la DSN du mois, ses documents) : la version sur laquelle il est parti se garde, datee, avec ce qu'il a emporte.
        if raison.is_some() || ancienne.as_ref().map(lu) != Some(lu(&nouvelle)) {
            let emportee: Vec<Depart> = ses_departs(&o, departs)
                .iter()
                .filter(|d| matches!(d.envoi, Envoi::Documents | Envoi::Dsn) && d.mois == Some(mois))
                .cloned()
                .collect();
            // Une validation a revalider (`a_revalider`) n'a rien emporte : sa paie se refait, sans version.
            let actuelle = o.brut.valide.clone().filter(|v| a_revalider(ctx, &o, stock::Version::Presente(v)).is_none());
            if let Some((actuelle, n)) = actuelle.as_ref().and_then(|a| Some((a, a.version?))) {
                let suivante = nouvelle_version(&mut o, actuelle, n, &emportee, le)?;
                if suivante != n {
                    if let Some(v) = o.brut_mut().valide.as_mut() {
                        v.version = Some(suivante);
                    }
                    stock::reecrire(&ctx.racine, &o)?;
                }
            }
        }
        crate::paies::poser(&o, mois, Some(nouvelle))?;
    }
    Ok(Rafraichi { etats: None, refus })
}

/// LE REPORT SUIT CE QUI LE PRECEDE, A L'ECRITURE : la validation de l'engagement `o` a refait sa paie de `premier` ;
/// chaque paie partie de son salarie dans les mois d'apres, jusqu'au present, du plus ancien au plus recent, remet a jour
/// l'etat qu'elle tient pour le mois suivant (`conges_avant_a_jour`) : un mois fini ne se relit pas, et le premier mois
/// en cours reprend un etat juste. Une paie qui n'est pas partie se refait a sa lecture (son empreinte ne tient plus).
fn reporter(ctx: &Contexte, o: &Objet, premier: Mois, departs: &Departs) -> R<()> {
    let Some(cle) = &o.carte().salarie else { return Ok(()) };
    let present = mois_present(ctx);
    for mois in crate::paies::mois_payes(&ctx.racine)?.into_iter().filter(|m| *m > premier && *m <= present) {
        let ctx = &ctx.du_mois(mois)?;
        let mut relues = empreinte::Relues::default();
        for e in tels_qu_ecrits(ctx, mois, departs)? {
            let Etat::Partis(p) = e else { continue };
            let v = p.valide();
            if v.objet.carte().salarie.as_ref() != Some(cle) {
                continue;
            }
            if let Some(a) = v.paie_ecrite(mois)?.filter(|a| a.conges_avant.is_some()) {
                if let Some(nouvelle) = conges_avant_a_jour(ctx, &v.objet, &a, mois, &mut relues)? {
                    crate::paies::poser(&v.objet, mois, Some(nouvelle))?;
                }
            }
        }
    }
    Ok(())
}

/// LE COMPTEUR AVANT LE MOIS D'UNE PAIE PARTIE, a jour des mois d'avant : la paie partie ne bouge pas, mais l'etat
/// qu'elle tient pour le mois suivant (`CONGES_AVANT`) suit ce dont il depend (les paies validees des mois qu'il lit,
/// les fiches). `None` : il tient. Sinon, la paie ecrite avec l'etat refait, et ce qu'il a lu dans son empreinte.
fn conges_avant_a_jour(ctx: &Contexte, o: &Objet, ancienne: &PaieEcrite, mois: Mois, relues: &mut empreinte::Relues) -> R<Option<PaieEcrite>> {
    let entrees = &ancienne.empreinte.entrees;
    let de_lui = |k: &CleLue| matches!(k, CleLue::Datee(LectureDatee::Paies { .. }) | CleLue::Fiches { .. });
    let siennes: BTreeMap<CleLue, ValeurLue> = entrees.iter().filter(|(k, _)| de_lui(k)).map(|(k, v)| (k.clone(), v.clone())).collect();
    if empreinte::tient(ctx, &siennes, relues)? {
        return Ok(None);
    }
    let cle = o.carte().salarie.clone().ok_or_else(|| socle::fichiers::absent("salarie"))?;
    let fin = mois.dernier_jour();
    // Un compteur avant le mois n'est tenu que pour un engagement paye au mois (`conges::compteur_ecrit`).
    let Some(au) = o.au_mois_depuis()?.and_then(|(_, nature)| nature.au_mois()) else { return Ok(None) };
    let (avant, mut lues) = empreinte::suivre(ctx, || crate::conges::avant_le_mois(ctx, &au, &cle, &fin, mois))?;
    lues.extend([empreinte::fiches(ctx, mois, &cle, o.carte().spectacle.as_ref())?]);
    let mut nouvelle = ancienne.clone();
    nouvelle.conges_avant = Some(avant);
    nouvelle.empreinte.entrees.retain(|k, _| !de_lui(k));
    nouvelle.empreinte.entrees.extend(lues.into_iter().filter(|(k, _)| de_lui(k)));
    Ok(Some(nouvelle))
}

// ------------------------------------------------------------------ ce qui est a refaire

/// Ce que dit le contrat, dans la saisie d'un engagement : ses parties, son objet, ses dates, sa remuneration, sa nature
/// et son emploi. INFERENCE : la liste suit les marqueurs du modele de contrat (`donnees/modeles/`) ; les lignes d'un
/// contrat paye au mois se composent chaque mois et ne sont pas des clauses.
pub(crate) const CLAUSES: [Champ; 17] = [
    Champ::Salarie, Champ::Spectacle, Champ::Debut, Champ::Fin, Champ::Numero, Champ::Lignes, Champ::Lieu, Champ::Nature,
    Champ::MotifRecours, Champ::QuotiteContrat, Champ::QuotiteReference, Champ::ModaliteTempsTravail, Champ::SalaireMensuel,
    Champ::Emploi, Champ::Metier, Champ::Cadre, Champ::Statut,
];

/// Les clauses d'une carte (`CLAUSES`), sans les lignes d'un contrat paye au mois.
fn clauses_de(c: &Carte, mensuel: bool) -> Carte {
    let mut out = Carte::default();
    for k in CLAUSES.into_iter().filter(|k| !(mensuel && *k == Champ::Lignes)) {
        out.prendre(c, k);
    }
    out
}

/// Les clauses d'une saisie gardee (`saisie_de`) : celles de son premier mois, chaque changement d'une clause, et
/// celles que les faits de chaque mois d'apres disent. Une version sans saisie n'en dit aucune.
fn clauses(saisie: Option<&stock::Saisie>, mensuel: bool) -> Option<stock::Saisie> {
    saisie.map(|s| clauses_de_la_saisie(s, mensuel))
}

fn clauses_de_la_saisie(s: &stock::Saisie, mensuel: bool) -> stock::Saisie {
    let avenants = s
        .avenants
        .iter()
        .map(|a| stock::Avenant { depuis: a.depuis, termes: clauses_de(&a.termes, mensuel), retires: a.retires.iter().copied().filter(|c| CLAUSES.contains(c)).collect() })
        .filter(|a| !a.retires.is_empty() || !a.termes.muette())
        .collect();
    stock::Saisie { carte: clauses_de(&s.carte, mensuel), avenants, faits: s.faits.iter().map(|(m, f)| (*m, clauses_de(f, mensuel))).collect() }
}

/// Ce qu'un envoi parti sur une version passee demande de refaire, le present ayant change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ARefaire {
    /// Le contrat (ou son dernier avenant) est parti sur la version `depuis`, et ses clauses ont change : un avenant a
    /// faire signer. `None` : le contrat a ete signe hors du logiciel (`crate::signe_ailleurs`), aucun avenant n'est
    /// parti, et un de ses termes a change depuis le premier mois (`stock::DATES`).
    Avenant { depuis: Option<VersionDeLEngagement> },
    /// La DPAE est partie sur la version `depuis` avec la date d'embauche `debut_declare`, qui a change.
    Dpae { depuis: VersionDeLEngagement, debut_declare: Option<Jour> },
    /// Les documents de `mois` sont partis sur la version `depuis`, et sa paie a change : un bulletin rectificatif.
    Bulletin { mois: Mois, depuis: VersionDeLEngagement },
    /// La DSN de `mois` est partie sur la version `depuis`, et sa paie a change : un annule-et-remplace, ou la correction
    /// dans une DSN suivante (la couche des declarations tranche, a l'echeance).
    Dsn { mois: Mois, depuis: VersionDeLEngagement },
}

/// LA VERSION EN VIGUEUR AU DEPART de chaque envoi, comparee au present de l'engagement valide `v` : ce qui est a
/// refaire. Pour chaque sorte d'envoi (et chaque mois des documents et des DSN), le dernier parti fait foi ; parti sur la
/// version presente, il est a jour.
pub fn a_refaire(ctx: &Contexte, v: &Valide, departs: &Departs) -> R<Vec<ARefaire>> {
    a_refaire_parmi(ctx, v, departs, &|_| true)
}

/// `a_refaire`, pour les seules sortes d'envoi que `sorte` retient : ce qui ne se demande pas ne se compare pas (les paies
/// des documents et des DSN partis, pour qui ne cherche que l'avenant d'un contrat).
pub fn a_refaire_parmi(ctx: &Contexte, v: &Valide, departs: &Departs, sorte: &dyn Fn(Envoi) -> bool) -> R<Vec<ARefaire>> {
    let siens = ses_departs(&v.objet, departs);
    let present = v.version();
    let actuelle = v.objet.brut.valide.as_ref();
    let presente = actuelle.and_then(|a| a.saisie.as_ref());
    let mut derniers: BTreeMap<(u8, Option<Mois>), &Depart> = BTreeMap::new();
    for d in siens.iter().filter(|d| sorte(d.envoi)) {
        let rang = match d.envoi {
            Envoi::Contrat => 0,
            Envoi::Dpae => 1,
            Envoi::Documents => 2,
            Envoi::Dsn => 3,
            Envoi::Signalement => 4,
        };
        derniers.insert((rang, d.mois), d);
    }
    let mensuel = v.objet.mensuel()?;
    let mut out = Vec::new();
    // UN CONTRAT SIGNE AILLEURS, dont aucun avenant n'est parti : ce qu'il dit est ce que dit le premier mois ; un terme
    // date apres lui (`stock::DATES`) est un changement que le salarie n'a pas signe.
    let contrat_parti = derniers.keys().any(|(sorte, _)| *sorte == 0);
    if sorte(Envoi::Contrat) && !contrat_parti && signe_ailleurs(ctx, &v.objet)? {
        if clauses(presente, mensuel).is_some_and(|c| !c.avenants.is_empty()) {
            out.push(ARefaire::Avenant { depuis: None });
        }
    }
    for d in derniers.into_values() {
        if Some(d.version) == present {
            continue;
        }
        let Some(passee) = v.version_numero(d.version) else { continue };
        let paie = |x: stock::Version, m: Mois| paie_ecrite_de_la_version(&v.objet, x, m);
        // Le debut que la version a valide ; une version sans saisie n'en dit pas.
        let debut = |s: Option<&stock::Saisie>| s.and_then(|s| s.carte.debut_lu());
        match d.envoi {
            Envoi::Contrat => {
                let partie = passee.saisie();
                if clauses(partie, mensuel) != clauses(presente, mensuel) && !partie.zip(presente).is_some_and(|(partie, presente)| seule_la_rupture(partie, presente)) {
                    out.push(ARefaire::Avenant { depuis: Some(d.version) });
                }
            }
            Envoi::Dpae => {
                let (avant, maintenant) = (debut(passee.saisie()), debut(presente));
                if avant != maintenant {
                    out.push(ARefaire::Dpae { depuis: d.version, debut_declare: avant });
                }
            }
            // Un signalement (arret, reprise, fin de contrat) part sur sa version ; il ne porte pas de paie.
            Envoi::Signalement => {}
            Envoi::Documents | Envoi::Dsn => {
                let Some(m) = d.mois else { continue };
                let presente = match actuelle {
                    Some(a) => paie(stock::Version::Presente(a), m)?,
                    None => None,
                };
                let change = match (paie(passee, m)?, presente) {
                    (Some(a), Some(b)) => !Arc::ptr_eq(&a, &b) && !chiffres_egaux(&a, &b),
                    (a, b) => a.is_some() != b.is_some(),
                };
                if change {
                    out.push(if d.envoi == Envoi::Documents {
                        ARefaire::Bulletin { mois: m, depuis: d.version }
                    } else {
                        ARefaire::Dsn { mois: m, depuis: d.version }
                    });
                }
            }
        }
    }
    Ok(out)
}

/// Ce qu'une rupture ecrit dans la saisie d'un engagement : sa fin, son motif, son preavis et ses dates.
const DE_LA_RUPTURE: [Champ; 10] = [
    Champ::Debut, Champ::Fin, Champ::MotifRupture, Champ::Preavis, Champ::NotificationRupture, Champ::SignatureConvention,
    Champ::EngagementLicenciement, Champ::Transaction, Champ::Numero, Champ::Lignes,
];

/// UN CONTRAT ROMPU N'A PAS D'AVENANT. Code du travail L. 1243-1 (`atelier/reference/sources/code-du-travail/
/// articles-20261028.md`) : le CDD se rompt avant l'echeance de son terme (accord des parties, faute grave, force
/// majeure, inaptitude) ; la rupture met fin au contrat signe, elle ne le modifie pas, et rien ne se refait signer. Vrai
/// quand la saisie `presente` ne differe de la saisie `partie` (celle sur laquelle le contrat est parti) que par la rupture
/// qu'elle porte : son motif (`motif_rupture`, absent de la partie), la fin et le preavis qu'il appelle, et les jours de
/// travail d'apres la fin retires (une ligne garde ses jours jusqu'a la fin, et sa quantite ne croit pas).
fn seule_la_rupture(partie: &stock::Saisie, presente: &stock::Saisie) -> bool {
    let motif_dit = |s: &stock::Saisie| s.carte.motif_rupture.is_some();
    if !motif_dit(presente) || motif_dit(partie) {
        return false;
    }
    let reste = |s: &stock::Saisie| {
        let mut s = s.clone();
        for k in DE_LA_RUPTURE {
            s.carte.ote(k);
        }
        s
    };
    if reste(partie) != reste(presente) {
        return false;
    }
    presente.carte.fin.is_some_and(|fin| crate::lignes_coupees_a_la_fin(partie.carte.lignes(), presente.carte.lignes(), fin))
}

#[cfg(test)]
mod tests_rupture {
    use super::*;

    /// Code du travail L. 1243-1 (`atelier/reference/sources/code-du-travail/articles-20261028.md`) : rompre un CDD avant
    /// son terme ne modifie pas le contrat signe, rien ne se refait signer ; changer en meme temps ce qu'il paie, si.
    #[test]
    fn un_contrat_rompu_n_a_pas_d_avenant() {
        let jours = |n: usize| ["2026-05-04", "2026-05-05", "2026-05-06", "2026-05-07", "2026-05-08"][..n].iter().map(|j| format!(r#""{}""#, j)).collect::<Vec<_>>().join(", ");
        let ligne = |n: usize, taux: &str| format!(r#"{{"forme": "unites", "rubrique": "cachet", "quantite": {{"cachets": {}}}, "taux": {}, "lieu": "Salle", "dates": [{}], "planning": []}}"#, n, taux, jours(n));
        let carte = |texte: String| crate::saisie::Carte {
            salarie: Some(IdSalarie::lire("X").unwrap()),
            spectacle: Some(droit::contexte::IdSpectacle::lire("s").unwrap()),
            numero: Some(crate::numeros::NumeroContrat::lire("X1.26.01").unwrap()),
            lignes: Some(vec![socle::forme::lire_texte(&texte, "ligne").unwrap()]),
            ..Default::default()
        };
        let saisie = |carte: crate::saisie::Carte| stock::Saisie { carte, ..Default::default() };
        let partie = saisie(carte(ligne(5, "150")));
        let rompu = |taux: &str| {
            saisie(crate::saisie::Carte {
                debut: Some(socle::forme::lire_texte("\"2026-05-04\"", "le jour du test").unwrap()),
                fin: Some(socle::forme::lire_texte("\"2026-05-06\"", "le jour du test").unwrap()),
                motif_rupture: Some(droit::natures::MotifDeRupture::CommunAccord),
                preavis: Some(crate::saisie::Preavis { genre: droit::preavis::TypeDePreavis::SansClause, debut: None, fin: None }),
                ..carte(ligne(3, taux))
            })
        };
        assert!(seule_la_rupture(&partie, &rompu("150")));
        assert!(!seule_la_rupture(&partie, &rompu("180")), "un taux change avec la rupture se refait signer");
        let raccourci = saisie(carte(ligne(3, "150")));
        assert!(!seule_la_rupture(&partie, &raccourci), "sans motif de rupture, des jours retires sont un avenant");
    }
}

/// La version (`saisie` ; ses paies : `paie_ecrite_de_la_version`) sur laquelle le dernier envoi `envoi` de l'engagement
/// `v` est parti, telle qu'elle est ecrite ; `None` si rien de tel n'est parti.
pub fn version_au_depart<'a>(v: &'a Valide, departs: &Departs, envoi: Envoi) -> Option<stock::Version<'a>> {
    let d = ses_departs(&v.objet, departs).iter().rev().find(|d| d.envoi == envoi)?;
    v.version_numero(d.version)
}

/// La version `n` de l'engagement `o`, telle qu'elle est ecrite (`saisie`) : le present, la validation que
/// « Modifier… » a mise de cote, ou une version passee ; `None` si l'engagement n'a pas de version `n`. Refus : une
/// version qui ne dit pas tout ce qu'un envoi a emporte (`a_revalider`).
fn version_ecrite<'a>(ctx: &Contexte, o: &'a Objet, n: VersionDeLEngagement) -> R<Option<stock::Version<'a>>> {
    let Some(v) = o.brut.version(n) else {
        return Ok(None);
    };
    match a_revalider(ctx, o, v) {
        Some(phrase) => Err(refus(phrase)),
        None => Ok(Some(v)),
    }
}

/// La version de l'engagement `o` sur laquelle la derniere DSN qui a declare `mois` est partie (celle du mois, ou celle
/// d'un mois suivant qui a porte sa correction), telle qu'elle est ecrite ; `None` si aucune DSN n'a declare ce mois
/// pour lui.
fn version_de_la_dsn<'a>(ctx: &Contexte, o: &'a Objet, departs: &Departs, mois: Mois) -> R<Option<stock::Version<'a>>> {
    match ses_departs(o, departs).iter().rev().find(|d| d.envoi == Envoi::Dsn && d.mois == Some(mois)) {
        Some(d) => version_ecrite(ctx, o, d.version),
        None => Ok(None),
    }
}

/// Ce que la version `version` dit de `mois` (`Objet::vue` de sa saisie).
fn vue_de_la_version(o: &Objet, version: stock::Version, mois: Mois) -> R<Option<crate::saisie::Carte>> {
    let saisie = version.saisie().ok_or_else(|| socle::echec::cle(socle::echec::PAIE_ILLISIBLE, "saisie"))?;
    o.d_apres_la_saisie(saisie).vue(mois)
}

/// LA PAIE ECRITE DE `mois` DE LA VERSION `version` de l'engagement `o` (une validation, celle qu'une modification va
/// remplacer, ou une version passee, telles que l'engagement les ecrit) : la paie presente du mois pour la version presente,
/// celle que la version passee a gardee (`paies::de_la_version`) sinon ; `None` si elle n'en a pas pour ce mois.
pub fn paie_ecrite_de_la_version(o: &Objet, version: stock::Version, mois: Mois) -> R<Option<Arc<PaieEcrite>>> {
    let n = version.numero();
    let presente = o.brut.valide.iter().chain(o.brut.precedente.iter()).any(|v| v.version == n);
    match n {
        Some(n) if !presente => Ok(crate::paies::de_la_version(o, n, mois)?.map(Arc::new)),
        _ => Ok(crate::paies::presente(o, mois)?),
    }
}

/// La paie de `mois` que la version `version` de `o` a ecrite, relue (`relire`) sur la saisie de cette version, la fiche
/// du salarie et la saisie du mois telles que son calcul les a lues (`fiches`, `MOIS_LU`) ; `None` si cette version n'a
/// pas de paie pour ce mois. Elle porte le numero de son bulletin quand ses documents sont partis.
fn paie_de_la_version(ctx: &Contexte, o: &Objet, version: stock::Version, mois: Mois) -> R<Option<Paie>> {
    let Some(ecrite) = paie_ecrite_de_la_version(o, version, mois)? else { return Ok(None) };
    let Some(vue) = vue_de_la_version(o, version, mois)? else { return Ok(None) };
    let data = crate::saisie::VueDuMois { mois, taux_cachet: ecrite.saisie_du_mois.taux_cachet.clone(), engagements: vec![vue] };
    let eng = relire::engagements_ecrits(ctx, &data, &[&ecrite.paie])?.pop().ok_or_else(|| socle::echec::cle(socle::echec::PAIE_ILLISIBLE, "paie"))?;
    let lue = relire::paie_sur_ses_fiches(ctx, eng, &ecrite.paie, &ecrite.fiches)?;
    Ok(Some(paie_sur_son_ecrite(lue, &ecrite)))
}

/// LA PAIE DE `mois` QUE LA VERSION `version` DE `o` A ECRITE, relue de ce qu'elle a ecrit et de lui seul : son
/// engagement compris (`Engagement::de_sa_forme`), sur les fiches que son calcul a lues ; `None` si cette version n'a
/// pas de paie pour ce mois. C'est ce que le contrat disait sur cette version (l'ancien texte d'un avenant) : rien ne s'y
/// recalcule, rien ne s'y relit d'aujourd'hui (ni la saisie, ni le jour du versement, ni l'emploi).
pub fn paie_ecrite_relue(ctx: &Contexte, o: &Objet, version: stock::Version, mois: Mois) -> R<Option<Paie>> {
    let Some(ecrite) = paie_ecrite_de_la_version(o, version, mois)? else { return Ok(None) };
    let eng = crate::Engagement::de_sa_forme(&ecrite.paie.engagement, crate::qui_de(&ecrite.fiches.paie.fiche.champs)?);
    relire::paie_sur_ses_fiches(ctx, eng, &ecrite.paie, &ecrite.fiches).map(Some)
}

/// LA PAIE DECLAREE de `mois` : celle que la version sur laquelle la derniere DSN qui a declare ce mois est partie a
/// ecrite (`paie_de_la_version`) (regle 4 : ce qui est declare se lit, il ne se recalcule pas) ; `None` si aucune DSN
/// n'a declare ce mois pour l'engagement `o`.
pub fn paie_declaree(ctx: &Contexte, o: &Objet, departs: &Departs, mois: Mois) -> R<Option<Paie>> {
    match version_de_la_dsn(ctx, o, departs, mois)? {
        Some(v) => paie_de_la_version(ctx, o, v, mois),
        None => Ok(None),
    }
}

/// La paie de `mois` que la version n° `n` de l'engagement `o` a ecrite (`version_ecrite`, `paie_de_la_version`) ; `None`
/// si `o` n'a pas cette version ou qu'elle n'a pas de paie pour ce mois.
pub fn paie_de_la_version_n(ctx: &Contexte, o: &Objet, n: VersionDeLEngagement, mois: Mois) -> R<Option<Paie>> {
    match version_ecrite(ctx, o, n)? {
        Some(v) => paie_de_la_version(ctx, o, v, mois),
        None => Ok(None),
    }
}

/// LA PAIE PARTIE de `mois` : celle que la version sur laquelle les derniers documents de ce mois sont partis au salarie
/// a ecrite (`paie_de_la_version`), avec le numero de son bulletin ; `None` si les documents de ce mois ne sont pas partis
/// pour l'engagement `o`. C'est ce que son bulletin a paye.
pub fn paie_partie(ctx: &Contexte, o: &Objet, departs: &Departs, mois: Mois) -> R<Option<Paie>> {
    let Some(d) = ses_departs(o, departs).iter().rev().find(|d| d.envoi == Envoi::Documents && d.mois == Some(mois)) else {
        return Ok(None);
    };
    match version_ecrite(ctx, o, d.version)? {
        Some(v) => paie_de_la_version(ctx, o, v, mois),
        None => Ok(None),
    }
}

/// Une paie declaree pour un mois, et le mois de la DSN qui l'a portee quand ce n'est pas la sienne (une correction).
pub struct Declaree {
    pub par: Option<Mois>,
    /// La version de l'engagement sur laquelle la DSN est partie.
    pub version: VersionDeLEngagement,
    pub paie: Paie,
}

/// CE QUI A ETE DECLARE POUR `mois`, DANS L'ORDRE, par les DSN des mois anterieurs a `avant` (`None` : par toutes les
/// DSN parties) : la paie de la version sur
/// laquelle la DSN de ce mois est partie (la derniere : un annule-et-remplace remplace la precedente), puis celle de
/// chaque correction qu'une DSN d'un mois suivant a portee (la derniere DSN de ce mois-la fait foi). Vide si aucune DSN
/// n'a declare ce mois pour l'engagement `o`. Refus : une correction portee par la DSN d'un mois posterieur a `avant`
/// (une correction se porte apres la derniere qui a declare le contrat), ou une version qui n'a pas de paie de ce mois.
pub fn paies_declarees(ctx: &Contexte, o: &Objet, departs: &Departs, mois: Mois, avant: Option<Mois>) -> R<Vec<Declaree>> {
    let mut out: Vec<Declaree> = Vec::new();
    let non_actif = o.brut.non_actif.as_ref().map(|n| n.version);
    for (par, n) in versions_declarees(ctx, o, departs, mois, avant)? {
        // Une correction partie sur la version d'un non actif a declare la paie d'avant, annulee.
        if Some(n) == non_actif {
            if let Some(avant) = out.last().map(|d| d.paie.annulee()) {
                out.push(Declaree { par, version: n, paie: avant });
                continue;
            }
        }
        let paie = version_ecrite(ctx, o, n)?
            .map(|v| paie_de_la_version(ctx, o, v, mois))
            .transpose()?
            .flatten()
            .ok_or_else(|| socle::refus!("{} : la version {} sur laquelle une DSN a declare {} n'a pas de paie de ce mois", qui_de(ctx, o), n, mois))?;
        out.push(Declaree { par, version: n, paie });
    }
    Ok(out)
}

/// Les versions que `paies_declarees` relit, dans le meme ordre, chacune avec le mois de la DSN qui l'a portee : ce qui
/// a ete declare, sans en relire les paies.
fn versions_declarees(ctx: &Contexte, o: &Objet, departs: &Departs, mois: Mois, avant: Option<Mois>) -> R<Vec<(Option<Mois>, VersionDeLEngagement)>> {
    let mut chaine: Vec<(Option<Mois>, VersionDeLEngagement)> = Vec::new();
    for d in ses_departs(o, departs).iter().filter(|d| d.envoi == Envoi::Dsn && d.mois == Some(mois)) {
        match (d.par, avant) {
            (None, _) => chaine = vec![(None, d.version)],
            (Some(p), Some(avant)) if p > avant => {
                return Err(socle::refus!(
                    "{} : une correction de {} est deja partie dans la DSN de {}, posterieure a {}. Une correction se porte dans un mois posterieur au dernier qui a declare le contrat.",
                    qui_de(ctx, o),
                    mois,
                    p,
                    avant
                ))
            }
            (Some(p), Some(avant)) if p == avant => {}
            (Some(p), _) => match chaine.last_mut() {
                Some((Some(q), v)) if *q == p => *v = d.version,
                _ => chaine.push((Some(p), d.version)),
            },
        }
    }
    Ok(chaine)
}

/// LA CORRECTION DUE pour `mois` de l'engagement valide `v`, dans la DSN de `avant` (`None` : apres toutes les DSN
/// parties) : ce qui a ete declare pour ce mois par les DSN anterieures a `avant` (`paies_declarees`) quand la paie presente de `mois` n'est plus celle que la
/// derniere a declaree (la paie elle-meme, pas le prelevement ni le compteur de conges, que la DSN ne corrige pas) ; vide
/// quand aucune DSN n'a declare ce mois pour lui. `None` : rien a corriger, ou pas de paie presente de ce mois. La couche
/// des declarations dit si le mois se corrige ainsi (sa DSN partie, son annule-et-remplace passe). Les paies declarees ne
/// se relisent que quand la correction est due.
pub fn correction_due(ctx: &Contexte, v: &Valide, departs: &Departs, mois: Mois, avant: Option<Mois>) -> R<Option<Vec<Declaree>>> {
    let Some(presente) = v.paie_ecrite(mois)? else { return Ok(None) };
    let Some(&(_, derniere)) = versions_declarees(ctx, &v.objet, departs, mois, avant)?.last() else { return Ok(Some(Vec::new())) };
    if Some(derniere) == v.version() {
        return Ok(None);
    }
    let ecrite = version_ecrite(ctx, &v.objet, derniere)?.map(|x| paie_ecrite_de_la_version(&v.objet, x, mois)).transpose()?.flatten();
    if ecrite.is_some_and(|e| egales_sans_avis(&e.paie, &presente.paie)) {
        return Ok(None);
    }
    Ok(Some(paies_declarees(ctx, &v.objet, departs, mois, avant)?))
}

/// LA DERNIERE DATE DE DEBUT DECLAREE de l'engagement `o` (AAAA-MM-JJ) par une DSN d'un mois anterieur a `avant` : celle
/// de la saisie de la version sur laquelle la derniere est partie, quel que soit le mois qu'elle declarait (la correction
/// d'un autre mois d'un contrat sur plusieurs mois a pu la changer). `None` : aucune.
pub fn debut_declare(ctx: &Contexte, o: &Objet, departs: &Departs, avant: Mois) -> R<Option<Jour>> {
    let Some(d) = ses_departs(o, departs).iter().rev().find(|d| d.envoi == Envoi::Dsn && d.par.or(d.mois).is_some_and(|m| m < avant)) else {
        return Ok(None);
    };
    Ok(version_ecrite(ctx, o, d.version)?.and_then(|v| v.saisie()).and_then(|s| s.carte.debut_lu()))
}

/// La phrase d'un recalcul, pour l'ecran : « Recalculé le JJ/MM/AAAA : taux de PAS reçu. »
pub fn phrase_du_recalcul(r: &Recalcul) -> Option<String> {
    // Un instant se dit par son jour.
    Some(format!("Recalculé le {} : {}.", r.le.jour_de_paris().jj_mm_aaaa(), r.raison))
}

