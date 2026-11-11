//! Le bloc « Changements contrat - S21.G00.41 » d'une DSN mensuelle : ce qui a change d'un contrat deja declare.
//!
//! Cahier technique DSN 2026.1 (`atelier/reference/norme-dsn/texte/`) :
//! - le bloc porte l'ANCIENNE valeur, « Les rubriques doivent etre renseignees avec la valeur avant changement »
//!   (`bloc-S21.G00.41.txt`, p. 215), datee en 41.001, « date de survenance du changement » (p. 216) ;
//! - « Le changement d'une des modalites visees doit imperativement etre declare via un bloc « Changements Contrat -
//!   S21.G00.41 » », un bloc par modalite, « meme si plusieurs changements sont concomitants », dans « la DSN du mois
//!   ou le service prend connaissance de ces modifications » (`chapitre-2-principes-de-constitution.txt`, § 2.1.2.5.2,
//!   p. 25) ; CCH-13 et § 4.4.14.2 a (`chapitre-4-controles.txt`, p. 103) ;
//! - jamais un identifiant et une modalite dans le meme bloc (CCH-16, principe b) ; la profondeur de recalcul 41.028
//!   sur le bloc le plus ancien d'une modalite, au premier jour d'un mois civil ou a la date de debut du contrat
//!   (principe c, CCH-11 a 15 de 41.028, p. 226 ; § 2.1.2.5.2, p. 26), jamais pour un identifiant (principe d) ;
//! - un identifiant (date de debut 41.021, SIRET 41.012) se change « obligatoirement des la premiere DSN mensuelle dans
//!   laquelle est utilisee la nouvelle identite » (§ 2.1.2.3.1, p. 20) ; sa date « peut correspondre a toute date »,
//!   sauf celle du SIRET, « date d'effet du changement d'employeur » (p. 21) ;
//! - ce qui n'a pas de rubrique en bloc 41 (libelle de l'emploi, date de fin, taux AT) se change en reecrivant le bloc
//!   40 (§ 2.1.2.4, p. 21-22) : rien a faire ici.
//!
//! CE QUI A ETE DECLARE SE LIT, IL NE SE RECALCULE PAS (regle 4) : l'ancienne valeur est celle du bloc 40 du meme contrat
//! dans la DSN du mois le plus recent qui est partie pour lui, ecrit de la paie de ce mois telle que la version de
//! l'engagement sur laquelle elle est partie l'a ecrite (`dsn::bloc_40_declare`, `engagement::etats::paie_declaree`).
//! Un mois produit et pas encore depose n'est pas un declare.
//!
//! LA DATE SE LIT AUSSI, elle ne se pose jamais au 1er du mois par defaut : le lendemain de la fenetre de l'historique
//! de la fiche (salarie ou entreprise) qui a change le champ, la `date_modification` de la saisie du mois (un avenant),
//! le 1er janvier d'un bareme annuel (le code risque AT), ou le premier jour de la periode payee pour ce que le moteur
//! calcule periode par periode (les emplois multiples, 40.036). La date de debut, identifiant que la norme laisse dater
//! de tout jour, se date sans cela du dernier jour du mois declare (`blocs_41`). Un changement sans date refuse ; un changement anterieur
//! au mois (des mois deja declares avec l'ancienne valeur, que la paie n'a pas recalcules) refuse ; un changement en
//! cours de mois refuse sauf pour ce qui ne change pas la paie (statut BOETH), pour les identifiants, et pour ce que la
//! paie decoupee en periodes elementaires porte au jour du changement (`periodes`).




use engagement::calcul::Paie;
use droit::contexte::Contexte;
use droit::fiche::{CaseEntreprise, CaseSalarie, NomDeCase};
use socle::date::Jour;
use Origine::{Entreprise as E, Etablissement, Salarie as S};
use socle::mois::{Graphie, Mois};
use socle::echec::R;

/// Une modalite du contrat qui a sa rubrique en bloc 41 : d'ou son changement tient sa date.
struct Modalite {
    r40: &'static str,
    r41: &'static str,
    /// La saisie du mois la porte (`date_modification` de l'engagement la date).
    saisie: bool,
    /// D'ou elle vient, hors de la saisie : les cases des fiches dont l'historique la date, et l'etablissement de la
    /// carte.
    origines: &'static [Origine],
    /// Elle vient du bareme de l'annee : elle change au 1er janvier.
    annuelle: bool,
    /// Elle se lit periode de paie par periode de paie (le moteur la calcule chaque mois) : elle change au premier
    /// jour de la periode payee.
    periodique: bool,
    /// Elle peut changer en cours de mois : elle ne change pas la paie.
    en_cours_de_mois: bool,
    /// Elle peut changer en cours de mois quand la paie se decoupe au jour du changement (`periodes`) : le
    /// statut, la categorie, la PCS et le code emploi que la fiche date, la duree du travail que l'avenant date avec ses
    /// termes d'avant (`avant`), et la proratisation du plafond qui la suit.
    decoupable: bool,
    /// Sa valeur est un code d'une liste de la norme (`droit::natures::ListeDeLaNorme`) : l'ancienne se relit dans son
    /// `enum` et ne s'ecrit en bloc 41 que si la liste de la version du fichier l'a (`relue`).
    liste: Option<Relue>,
}

/// D'ou vient une modalite, hors de la saisie du mois.
#[derive(Clone, Copy)]
enum Origine {
    /// Une case de la fiche du salarie, que son historique date.
    Salarie(CaseSalarie),
    /// Une case de la fiche entreprise, que son historique date.
    Entreprise(CaseEntreprise),
    /// L'etablissement que la carte de l'engagement dit : il change par un avenant, que la saisie date.
    Etablissement,
}

/// L'ancienne valeur d'une modalite a liste, relue dans son `enum`, et le code que le fichier de cette norme en ecrit.
type Relue = fn(&crate::norme::Norme, &str) -> R<&'static str>;

/// L'ancienne valeur `avant` (le bloc 40 declare) dans la liste `T` ; un code qu'elle n'a pas refuse, et le code ne
/// s'ecrit que si la liste de la version du fichier l'a (`dsn::code_admis`).
fn relue<T: droit::natures::ListeDeLaNorme>(norme: &crate::norme::Norme, avant: &str) -> R<&'static str> {
    let v = T::du_code_de_la_norme(avant).ok_or_else(|| {
        socle::refus!(
            "{} « {} » ({}), declare : ce n'est pas un code de sa liste dans la norme ; le bloc Changements contrat ne le \
recopie pas. Rien n'est ecrit.",
            T::quoi(),
            avant,
            T::RUBRIQUE
        )
    })?;
    crate::dsn::code_admis(norme, v)
}

const fn m(r40: &'static str, r41: &'static str, saisie: bool, origines: &'static [Origine]) -> Modalite {
    Modalite { r40, r41, saisie, origines, annuelle: false, periodique: false, en_cours_de_mois: false, decoupable: false, liste: None }
}

/// `m`, une modalite que le decoupage de la paie porte en cours de mois.
const fn d(r40: &'static str, r41: &'static str, saisie: bool, origines: &'static [Origine]) -> Modalite {
    Modalite { decoupable: true, ..m(r40, r41, saisie, origines) }
}

/// Les modalites que le modele fait changer (fiche N8, « Les rubriques 41.0xx, une par une »), chacune avec sa jumelle
/// du bloc 40 (la liste des types, § 4.4.14.1, p. 101-103). Le reste du bloc 41 (fonction publique, regimes speciaux,
/// dispositifs absents du modele, rubriques 40 non ecrites ou fixees par la nature) n'a pas de jumelle qui bouge : rien
/// ne peut en changer.
const MODALITES: [Modalite; 20] = [
    d("S21.G00.40.002", "S21.G00.41.002", true, &[S(CaseSalarie::statut_conventionnel), S(CaseSalarie::statut_categoriel_rc), S(CaseSalarie::code_emploi_ft)]),
    d("S21.G00.40.003", "S21.G00.41.003", true, &[S(CaseSalarie::statut_categoriel_rc), S(CaseSalarie::statut_conventionnel), S(CaseSalarie::code_emploi_ft)]),
    d("S21.G00.40.004", "S21.G00.41.019", true, &[S(CaseSalarie::pcs_ese), S(CaseSalarie::code_emploi_ft)]),
    d("S21.G00.40.005", "S21.G00.41.020", true, &[S(CaseSalarie::code_emploi_ft)]),
    m("S21.G00.40.007", "S21.G00.41.004", true, &[]),
    m("S21.G00.40.008", "S21.G00.41.005", true, &[]),
    m("S21.G00.40.011", "S21.G00.41.006", true, &[]),
    m("S21.G00.40.012", "S21.G00.41.022", true, &[]),
    d("S21.G00.40.013", "S21.G00.41.007", true, &[]),
    Modalite { liste: Some(relue::<droit::natures::ModaliteDuTempsDeTravail>), ..d("S21.G00.40.014", "S21.G00.41.008", true, &[]) },
    m("S21.G00.40.016", "S21.G00.41.010", false, &[Etablissement, E(CaseEntreprise::code_postal), E(CaseEntreprise::etablissements)]),
    m("S21.G00.40.017", "S21.G00.41.011", false, &[E(CaseEntreprise::idcc)]),
    // Le lieu de travail est le SIRET de l'etablissement du salarie (`dsn::bloc_contrat`) : il change avec lui, a la
    // date de la mutation (le SIRET, identifiant, va en 41.012 dans un autre bloc, CCH-16).
    m("S21.G00.40.019", "S21.G00.41.013", false, &[Etablissement]),
    Modalite { liste: Some(relue::<droit::natures::MotifDeRecours>), ..m("S21.G00.40.021", "S21.G00.41.016", true, &[]) },
    m("S21.G00.40.022", "S21.G00.41.023", false, &[E(CaseEntreprise::congesspectacles_adherent)]),
    // Le taux de la deduction forfaitaire specifique vient du bareme de l'annee (arrete du 4 septembre 2025, art. 9, II :
    // reduit « au 1er janvier de chaque annee ») et de la fiche (profession, accord du salarie, option de l'employeur).
    Modalite {
        annuelle: true,
        ..m(
            "S21.G00.40.023",
            "S21.G00.41.017",
            false,
            &[
                S(CaseSalarie::deduction_forfaitaire),
                S(CaseSalarie::deduction_forfaitaire_accord),
                E(CaseEntreprise::deduction_forfaitaire_option),
            ],
        )
    },
    // Le code risque vient du bareme de l'annee, celui de l'etablissement du salarie (`bareme::surcharger`) : il change
    // au 1er janvier, ou avec une mutation.
    Modalite { annuelle: true, ..m("S21.G00.40.040", "S21.G00.41.024", false, &[Etablissement]) },
    // « emplois multiples », « au cours d'une meme periode de paie » (p. 199) : `dsn::emplois_multiples` le calcule
    // chaque mois ; 41.061 est une modalite (§ 4.4.14.1, p. 103).
    Modalite { periodique: true, ..m("S21.G00.40.036", "S21.G00.41.061", false, &[]) },
    Modalite { en_cours_de_mois: true, ..m("S21.G00.40.072", "S21.G00.41.048", false, &[S(CaseSalarie::statut_boeth)]) },
    // Norme 2027 (P27V01) : « Ancienne application de la proratisation du Plafond de Securite Sociale a hauteur de la
    // quotite de travail » (cahier 2027.1.1, p. 242, `texte-2027/bloc-S21.G00.41.txt` l. 1439-1450 ; modalite, § 4.4.14.1,
    // `chapitre-4-controles.txt` l. 1047). 40.084 change avec le temps partiel (un avenant, date par la saisie), avec
    // la renonciation de l'etablissement (la fiche entreprise) ou avec le statut d'artiste, que la loi exclut de la
    // proratisation (`calcul::plafond_temps_partiel`). Absente des DSN de 2026 : rien de declare, pas de bloc 41.
    d("S21.G00.40.084", "S21.G00.41.066", true, &[S(CaseSalarie::statut), E(CaseEntreprise::plafond_temps_partiel)]),
];

/// Le bloc 40 d'un contrat tel qu'une DSN partie l'a declare (`dsn::bloc_40_declare`).
pub struct Declare {
    /// Le mois de cette DSN.
    pub mois: Mois,
    /// Le NIC de l'etablissement qui l'a deposee, et son SIRET (06.001 + 11.001).
    pub nic: String,
    pub siret: String,
    pub bloc: Vec<(String, String)>,
}


fn valeur<'a>(bloc: &'a [(String, String)], r: &str) -> Option<&'a str> {
    bloc.iter().find(|(k, _)| k == r).map(|(_, v)| v.as_str())
}

fn ddmmaaaa(v: &str) -> Option<Jour> {
    crate::dsn_lue::jour_dsn(v)
}

/// Les jours ou une case d'une fiche datee a change (`depuis` : les jours ou elle prend une valeur) : le `depuis` de
/// chaque valeur qui en suit une autre.
fn changements_dates(depuis: Vec<Option<Jour>>) -> impl Iterator<Item = Jour> {
    depuis.into_iter().skip(1).flatten()
}

/// « NOM Prénom, contrat <numéro> » : qui la phrase de refus désigne.
fn qui(p: &Paie) -> String {
    let nom = match p.salarie.nomme() {
        Some(n) => n,
        None => p.salarie.id.as_str().to_string(),
    };
    match p.engagement.numero.as_deref() {
        Some(n) => format!("{}, contrat {}", nom, n),
        None => format!("{}, contrat sans numero", nom),
    }
}

/// La date de la modification (41.001) du changement de `md`, declaree `avant` dans la DSN de `decl.mois`. Refus
/// quand elle ne se lit pas, ou qu'elle ne tombe pas dans le mois declare (voir l'en-tete du module).
fn date_du_changement(ctx: &Contexte, p: &Paie, md: &Modalite, decl: &Declare, mois: Mois, avant: &str, apres: Option<&str>) -> R<Jour> {
    let apres = match apres {
        Some(a) => a,
        None => "(rien)",
    };
    let (debut_m, fin_b) = (mois.premier_jour(), decl.mois.dernier_jour());
    let fin_p = p.engagement.fin_periode();
    let mut candidats: Vec<(Jour, String)> = Vec::new();
    if md.saisie {
        if let Some(j) = p.engagement.date_modification {
            candidats.push((j,"la date de modification de la saisie du mois (`date_modification`)".into()));
        }
    }
    // Sans fiche datee, aucun changement date.
    for origine in md.origines {
        let (depuis, ou, c): (Vec<Option<Jour>>, &str, NomDeCase) = match *origine {
            S(c) => (ctx.salarie_brut(p.salarie.id.as_str()).into_iter().flat_map(|b| b.depuis(c.into())).collect(), "la fiche du salarie", c.into()),
            E(c) => (ctx.entreprise_brute().depuis(c.into()), "la fiche entreprise", c.into()),
            // L'etablissement est de la carte : un avenant le change, que la saisie date (`date_modification`).
            Etablissement => continue,
        };
        candidats.extend(changements_dates(depuis).map(|j| (j, format!("{} (`{}`)", ou, c))));
    }
    if md.annuelle && decl.mois.annee() < mois.annee() {
        candidats.push((Jour { an: debut_m.an, mois: 1, jour: 1 }, "le bareme de l'annee (il vaut du 1er janvier)".into()));
    }
    if md.periodique {
        // INFERENCE : une valeur qui se lit periode de paie par periode de paie change au premier jour de la periode
        // ou elle differe. Elle n'a pu changer qu'au mois declare suivant : sinon, le mois intermediaire, non depose,
        // l'a peut-etre deja changee, et le jour ne se lit plus.
        if decl.mois != mois.precedent() {
            return Err(socle::refus!(
                "{} : {} passe de '{}' (declare en {}) a '{}'. Cette valeur se lit periode de paie par periode de paie : le changement date du premier jour de la periode ou il a eu lieu, et les mois entre {} et {}, non deposes, ne le disent pas. Deposer d'abord la DSN du mois precedent.",
                qui(p),
                md.r40,
                avant,
                decl.mois.libelle(Graphie::Accentuee),
                apres,
                decl.mois.libelle(Graphie::Accentuee),
                mois.libelle(Graphie::Accentuee)
            ));
        }
        candidats.push((p.engagement.debut_periode(), "la periode de paie (la valeur se lit periode par periode, p. 199)".into()));
    }
    candidats.retain(|(j, _)| *j <= fin_p);
    let mut apres_declare: Vec<&(Jour, String)> = candidats.iter().filter(|(j, _)| *j > fin_b).collect();
    apres_declare.sort_by_key(|(j, _)| *j);
    apres_declare.dedup_by_key(|(j, _)| *j);
    let quoi = format!("{} passe de '{}' (declare en {}) a '{}'", md.r40, avant, decl.mois.libelle(Graphie::Accentuee), apres);
    let ou_dater = {
        let mut o = Vec::new();
        if md.saisie {
            o.push("la date de l'avenant (a l'ecran : ouvrir la carte de l'engagement, « Modifier l'engagement », puis le mois a \
partir duquel le changement vaut, que l'avertissement demande ; l'avenant prend effet le premier jour de ce mois)"
                .to_string());
        }
        if !md.origines.is_empty() {
            o.push("la date du changement sur la fiche (« a partir du » de l'ecran)".to_string());
        }
        o.join(", ou ")
    };
    match apres_declare.as_slice() {
        [] => {
            if let Some((j, d)) = candidats.iter().filter(|(j, _)| *j <= fin_b).max_by_key(|(j, _)| *j) {
                return Err(socle::refus!(
                    "{} : {}. Le changement est date du {} ({}), donc deja en vigueur au mois de {}, que la DSN deposee a \
declare avec l'ancienne valeur. Ce n'est plus un changement du mois mais une correction de mois deja declares (cahier \
technique DSN, § 2.1.2.5.2 p. 25-26 : bloc S21.G00.41 et profondeur de recalcul), avec la regularisation de leurs paies, \
que le logiciel ne fait pas. Refus : verifier la date du changement.",
                    qui(p), quoi, j.jj_mm_aaaa(), d, decl.mois.libelle(Graphie::Accentuee)
                ));
            }
            Err(socle::refus!(
                "{} : {}, et rien ne dit depuis quand. La DSN doit declarer l'ancienne valeur dans un bloc Changements \
contrat (S21.G00.41) date du jour du changement (41.001, cahier technique DSN p. 216 et § 2.1.2.5.2 p. 25), et ce jour ne \
se devine pas. Le dater : {}.",
                qui(p), quoi, ou_dater
            ))
        }
        [(j, d)] => {
            if *j < debut_m {
                return Err(socle::refus!(
                    "{} : {}. Le changement est date du {} ({}), avant {} : il se declare dans la DSN de {}, qui n'est pas \
deposee. La deposer d'abord (elle porte le changement) ; si ces mois ont ete declares avec l'ancienne valeur, c'est une \
correction de mois deja declares, que le logiciel ne fait pas.",
                    qui(p), quoi, j.jj_mm_aaaa(), d, mois.libelle(Graphie::Accentuee), j.mois_civil().libelle(Graphie::Accentuee)
                ));
            }
            // En cours de mois : la paie decoupee au jour du changement le porte (; norme, § 2.1.2.5 p. 22-23 :
            // 41.001 au jour, 41.028 au premier du mois), pour ce que le decoupage sait calculer.
            if *j > debut_m && !md.en_cours_de_mois && !(md.decoupable && p.decoupee_le(j)) {
                let suivant = Jour { jour: 1, ..debut_m.plus_jours(31)? };
                let comment = if md.decoupable && md.saisie && md.origines.is_empty() {
                    " La paie se decoupe au jour d'un avenant quand la saisie dit les termes du contrat d'avant (`avant` : \
`salaire_mensuel` et `quotite_contrat`) ; sinon, le"
                } else if md.decoupable {
                    " Ce changement ne decoupe pas la paie : il vient d'ailleurs que de la fiche ou d'un avenant dont la saisie \
dit les termes d'avant (`avant`). Le"
                } else {
                    " Le logiciel ne decoupe la paie qu'au jour d'un changement de statut, de categorie, d'emploi ou de duree du \
travail. Le"
                };
                return Err(socle::refus!(
                    "{} : {}, le {} ({}), en cours de mois.{} dater au premier jour d'un mois : le {} ou le {}.",
                    qui(p), quoi, j.jj_mm_aaaa(), d, comment, debut_m.jj_mm_aaaa(), suivant.jj_mm_aaaa()
                ));
            }
            Ok(*j)
        }
        plusieurs => Err(socle::refus!(
            "{} : {}, et plusieurs changements sont dates depuis la DSN deposee de {} ({}). La DSN les declarerait en \
plusieurs blocs, chacun avec la valeur d'avant lui, que la fiche ne garde pas toutes. Refus : deposer les mois \
intermediaires, ou ne garder qu'un changement.",
            qui(p),
            quoi,
            decl.mois.libelle(Graphie::Accentuee),
            plusieurs.iter().map(|(j, d)| format!("{} par {}", j.jj_mm_aaaa(), d)).collect::<Vec<_>>().join(" ; ")
        )),
    }
}

/// Les blocs S21.G00.41 du contrat de `p`, dont le bloc 40 vient d'etre ecrit (`ecrit`), dans la DSN mensuelle de
/// `mois` de l'etablissement `nics[0]`, en norme `norme` ; `declare` : son bloc 40 declare (`dsn::bloc_40_declare`). Vide au
/// premier mois du contrat ou quand rien n'a encore ete declare.
fn blocs_41(ctx: &Contexte, p: &Paie, mois: Mois, norme: &crate::norme::Norme, nics: &[String], ecrit: &[(String, String)], declare: Option<&Declare>) -> R<Vec<Vec<(String, String)>>> {
    let e = &p.engagement;
    if e.numero.as_deref().is_none_or(str::is_empty) {
        return Ok(Vec::new());
    }
    if e.premier_mois() {
        return Ok(Vec::new());
    }
    let Some(decl) = declare else { return Ok(Vec::new()) };
    let fin_m = mois.dernier_jour();
    let mut identifiants = Vec::new();
    // L'IDENTIFIANT : la date de debut. Il ne change jamais en silence : l'ancienne se declare dans un bloc Changements
    // contrat (41.021), sans quoi les organismes verraient un nouveau contrat (§ 2.1.2.3, p. 20-21). Ce bloc se date du
    // jour que la saisie du mois donne a son avenant ; sinon du dernier jour du mois declare : pour un identifiant autre
    // que le SIRET, « la date de modification du bloc changement peut correspondre a toute date » (§ 2.1.2.3.2, p. 21,
    // `chapitre-2-principes-de-constitution.txt`), au plus tard ce jour-la (CCH-15 de 41.001, p. 217).
    fn debut_declare<'a>(bloc: &'a [(String, String)], ou: &str, p: &Paie) -> R<&'a str> {
        valeur(bloc, "S21.G00.40.001").ok_or_else(|| socle::refus!("{} : {} ne porte pas la date de debut du contrat (S21.G00.40.001).", qui(p), ou))
    }
    let (avant, apres) = (debut_declare(&decl.bloc, "la DSN deposee", p)?, debut_declare(ecrit, "le bloc contrat ecrit", p)?);
    if avant != apres {
        let date = match e.date_modification {
            Some(j) => j,
            // Sans avenant date : le dernier jour du mois declare (§ 2.1.2.3.2, ci-dessus).
            None => fin_m,
        };
        if date > fin_m {
            return Err(socle::echec::cle(format!(
                "{} : l'avenant prend effet le {}, après la fin du mois déclaré ; la norme l'interdit (CCH-15 de S21.G00.41.001, \
p. 217).",
                qui(p),
                date.jj_mm_aaaa()
            ), "date_modification"));
        }
        identifiants.push(vec![("S21.G00.41.001".to_string(), date.jjmmaaaa()), ("S21.G00.41.021".to_string(), avant.to_string())]);
    }
    // L'IDENTIFIANT : l'etablissement. Le contrat etait declare par un autre etablissement : son SIRET, date du jour de
    // la mutation (« la datation precise du changement de SIRET est impactante », p. 21).
    if decl.nic != nics[0] {
        let fin_b = decl.mois.dernier_jour();
        // L'etablissement est celui que l'engagement dit (`engagement::calcul::etablissement_du_contrat`) ; le jour ou il
        // change est celui de l'avenant (`date_modification`).
        let date = p.engagement.date_modification.filter(|j| *j > fin_b && *j <= fin_m);
        let Some(date) = date.as_ref() else {
            return Err(socle::refus!(
                "{} : le contrat etait declare par l'etablissement {} (DSN de {}) et l'est maintenant par l'etablissement {}. \
Le SIRET de l'ancien etablissement se declare dans un bloc Changements contrat (S21.G00.41.012) date du jour de la \
mutation (cahier technique DSN, p. 21) : ouvrir la carte de l'engagement, changer l'établissement, « Modifier l'engagement », et dire dans l'avertissement le mois et le jour où il change.",
                qui(p),
                decl.siret,
                decl.mois.libelle(Graphie::Accentuee),
                nics[0]
            ));
        };
        identifiants.push(vec![("S21.G00.41.001".to_string(), date.jjmmaaaa()), ("S21.G00.41.012".to_string(), decl.siret.clone())]);
    }
    // LES MODALITES : une par bloc, l'ancienne valeur, sa date, la profondeur de recalcul.
    let mut modalites = Vec::new();
    for md in &MODALITES {
        let Some(avant) = valeur(&decl.bloc, md.r40) else {
            // Rien de declare : il n'y a pas d'ancienne valeur (41.048 : un statut BOETH « precedemment declare »,
            // p. 234). INFERENCE, la meme pour toutes : une rubrique qui apparait ne fait pas de bloc 41.
            continue;
        };
        let apres = valeur(ecrit, md.r40);
        // A la casse pres : « 353b » (le catalogue, la nomenclature de l'Insee) et « 353B » (une fiche) sont un meme code.
        if apres.is_some_and(|a| avant.eq_ignore_ascii_case(a)) {
            continue;
        }
        let date = date_du_changement(ctx, p, md, &decl, mois, avant, apres)?;
        let debut = ddmmaaaa(debut_declare(ecrit, "le bloc contrat ecrit", p)?)
            .ok_or_else(|| socle::refus!("{} : la date de debut du bloc contrat ecrit (S21.G00.40.001) ne se lit pas.", qui(p)))?;
        let profondeur = Jour { jour: 1, ..date }.max(debut);
        let ancienne = match md.liste {
            Some(relue) => relue(norme, avant)?.to_string(),
            None => avant.to_string(),
        };
        modalites.push((
            md.r41,
            vec![
                ("S21.G00.41.001".to_string(), date.jjmmaaaa()),
                (md.r41.to_string(), ancienne),
                ("S21.G00.41.028".to_string(), profondeur.jjmmaaaa()),
            ],
        ));
    }
    modalites.sort_by_key(|(r, _)| *r);
    identifiants.extend(modalites.into_iter().map(|(_, b)| b));
    Ok(identifiants)
}

/// Pose les blocs 41 du contrat de `p` juste apres son bloc 40, qui commence a la ligne `debut` du fichier `f`.
pub fn poser(f: &mut crate::dsn::Fichier, debut: usize, ctx: &Contexte, p: &Paie, mois: Mois, nics: &[String], declare: Option<&Declare>) -> R<()> {
    let mut fin = debut;
    let mut ecrit = Vec::new();
    while let Some(l) = f.lignes.get(fin).filter(|l| l.starts_with("S21.G00.40.")) {
        if let Some((k, v)) = l.split_once(',') {
            ecrit.push((k.to_string(), v.trim_matches('\'').to_string()));
        }
        fin += 1;
    }
    let blocs = blocs_41(ctx, p, mois, f.norme()?, nics, &ecrit, declare)?;
    let neuves: Vec<String> = blocs.into_iter().flatten().map(|(k, v)| format!("{},'{}'", k, v)).collect();
    f.lignes.splice(fin..fin, neuves);
    Ok(())
}

/// LA DATE DE DEBUT D'UN CONTRAT DECLARE, CORRIGEE APRES LA LIMITE DE L'ANNULE-ET-REMPLACE, par une
/// regularisation (`dsn::Regularisation`) : fiche net-entreprises 1952 (`atelier/reference/sources/
/// net-entreprises-base-connaissances/net-entreprises-fiche1952-correction-date-debut-contrat-20250326.txt`), « Cette
/// donnee peut etre corrigee via un bloc " Changements Contrat - S21.G00.41 " en remplissant les rubriques " Date de la
/// modification - S21.G00.41.001 " et " Ancienne date de debut du contrat - S21.G00.41.021 ". La nouvelle valeur est
/// renseignee au niveau du bloc " Contrat - S21.G00.40 " ». Le bloc 40 vient d'etre ecrit a partir de la ligne `debut`
/// de `lignes`, a la date corrigee ; le bloc 41 se pose juste apres lui.
///
/// - 41.021 : la date declaree, LUE dans la version declaree (`versement_declare`, `debut_contrat` : regle 4), pas celle
///   d'une saisie ;
/// - 41.001 : le jour ou la correction a ete ecrite (« Cette erreur a ete corrigee en paie le 7 fevrier 2018 » ->
///   41.001 = 07022018, l'exemple de la fiche), au plus tard le dernier jour du mois principal declare (CCH-15 de
///   41.001, `bloc-S21.G00.41.txt` p. 217) ; pour un identifiant, « la date de modification du bloc changement peut
///   correspondre a toute date » (§ 2.1.2.3.2, `chapitre-2-principes-de-constitution.txt` p. 21) ;
/// - un identifiant seul dans son bloc, sans profondeur de recalcul (CCH-16, principe d, p. 23 ; fiche 1952 : 41.021
///   « Il n'est pas possible de la declarer avec la rubrique " Profondeur de recalcul de la paie - S21.G00.41.028 " »).
///
/// `r` : la correction du dernier mois qui a declare le contrat (pour un contrat sur plusieurs mois, celle du dernier) ;
/// `nouveau` : la date de debut du bloc 40 ecrit.
///
/// Rien a poser quand la date n'a pas change, ou pour un contrat jamais declare (un contrat ajoute). Refus quand la
/// date declaree n'est pas dite, ou que le jour de la correction n'est pas connu : le bloc ne se devine pas (regle 1).
pub fn ancienne_date_de_debut(lignes: &mut Vec<String>, debut: usize, r: &crate::dsn::Regularisation, nouveau: &Jour, fin_mois: Jour) -> R<()> {
    let e = &r.apres.engagement;
    let numero = match e.numero.as_deref() {
        Some(n) => n,
        None => "sans numero",
    };
    if r.declare.lignes.is_empty() && r.declare.fichier.is_empty() {
        return Ok(());
    }
    let Some(ancienne) = r.declare.debut_contrat else {
        if *nouveau == r.avant.engagement.debut {
            return Ok(());
        }
        return Err(socle::echec::refus_avec(
            format!(
                "Correction de {} (contrat {}) : la date de debut passe du {} au {}, et la DSN deposee ne dit pas la date \
declaree (S21.G00.40.001 du contrat). Le bloc Changements contrat (S21.G00.41.021, fiche net-entreprises 1952) porte la \
date declaree : elle se lit, elle ne se devine pas. Rien n'est ecrit.",
                r.mois,
                numero,
                r.avant.engagement.debut.jj_mm_aaaa(),
                nouveau.jj_mm_aaaa()
            ),
            socle::echec::Lieu { fichier: Some(r.declare.fichier.clone()), ..socle::echec::Lieu::default() },
        ));
    };
    if ancienne == *nouveau {
        return Ok(());
    }
    let Some(decidee) = r.decidee_le else {
        return Err(socle::echec::cle(format!(
            "Correction de {} (contrat {}) : la date de debut passe du {} au {} ; le bloc Changements contrat se date du \
jour de la correction (S21.G00.41.001, fiche net-entreprises 1952), et la correction ne le dit pas. Rien \
n'est ecrit.",
            r.mois,
            numero,
            ancienne.jj_mm_aaaa(),
            nouveau.jj_mm_aaaa()
        ), "decidee_le"));
    };
    let date = decidee.min(fin_mois);
    let mut fin = debut;
    while lignes.get(fin).is_some_and(|l| l.starts_with("S21.G00.40.")) {
        fin += 1;
    }
    let bloc = [format!("S21.G00.41.001,'{}'", date.jjmmaaaa()), format!("S21.G00.41.021,'{}'", ancienne.jjmmaaaa())];
    lignes.splice(fin..fin, bloc);
    Ok(())
}
