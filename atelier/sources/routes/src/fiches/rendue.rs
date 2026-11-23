//! LA FICHE RENDUE (`fiche`) : une fiche telle qu'on la saisit, rendue comme la route d'une carte rend la sienne
//! (`routes::mois::CarteRendue`). Ses cases ouvertes, dans l'ordre de `champs-base.json` (bloc par bloc), chacune son bloc,
//! sa largeur, son libelle, sa forme (`catalogue::Forme`), sa place (de base ou sous « Avancé »,
//! `engagement::carte::PlaceDeLaCase`), son aide, son exemple, sa valeur, ses choix, ses parties (une case faite de cases,
//! des rangees), ce que le moteur dit sous elle, ses temps et sa faute ; l'en-tete de la fiche, l'encart de ses erreurs et
//! son historique, en mots. La page les dessine par le dessinateur commun (`js/avance.js`) et ne decide rien. L'ecriture
//! reste `enregistrer-fiche`.

use serde::{Deserialize, Serialize};

use crate::catalogue::Forme;
use crate::enveloppe::{Dite, Phrase};
use crate::ouverture::{Choix, ExempleDuChoix};
use crate::temps::{CaseDatee, TempsDeLaCase};
use declarations::champs::{self, Anomalie, Case, Champ, CheminDeFiche, FicheAJuger, Gravite, Largeur, Libelles};
use droit::contexte::{Contexte, IdSalarie, IdSpectacle};
use droit::fiche::{CaseDeFiche, CaseEntreprise, Changements, ChampsEntreprise, FauteDeCase, FicheDatee as _, FicheEntrepriseDatee, FicheSaisie, FicheSalarieDatee, FicheSpectacleDatee, Lue as _, MotEcrit, NomDeFiche, ValeurDeCase};
use crate::imprevus::GesteDeRoute;
use engagement::carte::PlaceDeLaCase;
use socle::echec::{Classe, R};

use super::ecriture::{anomalies_enregistrees, anomalies_saisies, comblee, verdict, SorteDeFiche, Tapee};

/// LA FICHE A RENDRE (`fiche`) : `{"nom": …}`, sa cle pour une fiche enregistree (aucune : une fiche neuve, ou
/// l'entreprise), et ce qui est saisi (`changements`, comme a l'ecriture `enregistrer-fiche` et comme la carte : chacun
/// lu seul dans le type de sa case, pose sur la fiche enregistree ; absents : la fiche telle qu'elle est enregistree), et
/// les cases que la saisie vient de changer (`changees`, de la plus ancienne a la plus recente : ce qu'une case decide
/// d'autres cases ne se fait que quand c'est elle qu'on vient de changer ; absentes : aucune).
#[derive(Deserialize)]
#[serde(tag = "nom", deny_unknown_fields)]
pub(crate) enum FicheARendre {
    #[serde(rename = "entreprise")]
    Entreprise { changements: Option<socle::forme::Liste>, changees: Option<Vec<CaseDeFiche>> },
    #[serde(rename = "salarie")]
    Salarie { cle: Option<IdSalarie>, changements: Option<socle::forme::Liste>, changees: Option<Vec<CaseDeFiche>> },
    #[serde(rename = "spectacle")]
    Spectacle { cle: Option<IdSpectacle>, changements: Option<socle::forme::Liste>, changees: Option<Vec<CaseDeFiche>> },
}

/// CE QUE L'EN-TETE D'UNE FICHE RENDUE DIT D'ELLE (`SorteDeFiche::entete`) : son titre, ce qui suit le titre, une note
/// dessous, le cout de l'annee qu'elle montre (`cout`), et si « Annuler » ramene a la liste (`annuler` : l'entreprise
/// n'a pas de liste) ; celle d'un salarie enregistre, ce qui se fait depuis elle (`salarie`), celle d'un salarie neuf, la
/// demande qui la fait remplir par la personne (`invitation`). Une fiche qui n'en dit rien n'a que son « Annuler ».
#[derive(Debug, Clone, Serialize)]
pub(crate) struct EnteteDeLaFiche {
    titre: Option<String>,
    sous_titre: Option<String>,
    note: Option<&'static str>,
    cout: Option<CoutDeLaFiche>,
    annuler: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    salarie: Option<SalarieDeLEntete>,
    #[serde(skip_serializing_if = "Option::is_none")]
    invitation: Option<InvitationDeLaFiche>,
}

/// LE COUT DE L'ANNEE QU'UNE FICHE MONTRE EN TETE : ou le lire dans ce que rend la route `couts` (le cote, et la cle sous
/// laquelle elle range cette fiche : un spectacle par son titre, un salarie par sa cle). La route `fiche` ne le calcule
/// pas : `couts` lit l'annee entiere, une fois par annee et par seance (la page le garde), et la fiche se rend a chaque
/// frappe.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "cote")]
pub(crate) enum CoutDeLaFiche {
    #[serde(rename = "spectacles")]
    Spectacle { de: String },
    #[serde(rename = "salaries")]
    Salarie { de: IdSalarie },
}

/// CE QUI SE FAIT DEPUIS LA FICHE D'UN SALARIE ENREGISTRE (son en-tete) : sa cle, dont la page montre les imprevus (route
/// `imprevus`) et les droits (route `conseils-salarie`), et le libelle du geste qui mene a ses documents (Exports, ce
/// salarie choisi).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct SalarieDeLEntete {
    cle: IdSalarie,
    exporter: &'static str,
}

/// LA FICHE D'UN SALARIE NEUF A FAIRE REMPLIR PAR LA PERSONNE (son en-tete) : le titre de l'encart, ce que la demande
/// fait, le geste qui l'envoie (`fiche-demander`, ses cases : le courriel et le prenom) quand la boite courriel est liee,
/// sinon ce qui lui manque, et la demande du mot de passe du logiciel avant l'envoi.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct InvitationDeLaFiche {
    titre: &'static str,
    aide: &'static str,
    geste: Option<GesteDeRoute<crate::SansCharge>>,
    sans_boite: Option<&'static str>,
    demande: compagnie::demande::DemandeDuMotDePasse,
}

/// OU UNE CASE RENVOIE, que la route rend sous elle (`renvoi`) : le sous-onglet des metiers, ou la case d'une autre fiche
/// (l'option de la deduction forfaitaire, dans la fiche de l'entreprise), chacun son libelle.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "vers", rename_all = "snake_case")]
pub(crate) enum RenvoiDeLaFiche {
    Metiers { libelle: &'static str },
    Case { libelle: &'static str, fiche: NomDeFiche, case: CaseDeFiche },
}

impl EnteteDeLaFiche {
    /// Un en-tete qui ne dit rien, « Annuler » ramenant a la liste.
    pub(crate) fn aucun() -> EnteteDeLaFiche {
        EnteteDeLaFiche { titre: None, sous_titre: None, note: None, cout: None, annuler: true, salarie: None, invitation: None }
    }

    /// Un en-tete qui ne dit rien, sans « Annuler » : une fiche sans liste ou revenir.
    pub(crate) fn sans_liste() -> EnteteDeLaFiche {
        EnteteDeLaFiche { annuler: false, ..EnteteDeLaFiche::aucun() }
    }

    /// Un titre, ce qui le suit, sa note et le cout de l'annee qu'il montre ; « Annuler » ramene a la liste.
    pub(crate) fn de(titre: String, sous_titre: Option<String>, note: Option<&'static str>, cout: Option<CoutDeLaFiche>) -> EnteteDeLaFiche {
        EnteteDeLaFiche { titre: Some(titre), sous_titre, note, cout, ..EnteteDeLaFiche::aucun() }
    }
}

/// L'EN-TETE DE LA FICHE D'UN SALARIE `p` (`cle` : enregistree ; aucune, neuve). Enregistree : son nom (le prenom puis le
/// nom tels qu'on les lit, sinon sa cle), son matricule, le cout de son annee, ses imprevus, ses droits et ses documents.
/// Neuve : « Nouveau salarié », et la demande qui la fait remplir par la personne, par courriel (`fiche-demander` : sa
/// boite liee, sinon ce qui manque).
pub(super) fn entete_du_salarie(ctx: &Contexte, cle: Option<&IdSalarie>, p: &droit::fiche::ChampsSalarie) -> R<EnteteDeLaFiche> {
    let Some(cle) = cle else {
        let geste = crate::ouverture::boite_liee(ctx)?.then(|| {
            GesteDeRoute::avec_cases(
                crate::catalogue::Route::FicheDemander,
                crate::SansCharge {},
                crate::catalogue::Affiche::new("Envoyer la demande"),
                vec!["adresse", "prenom"],
                crate::imprevus::ValeursDeDepart::default(),
                crate::imprevus::ChoixDesCases::default(),
            )
        });
        let invitation = InvitationDeLaFiche {
            titre: "À faire remplir",
            aide: "Elle reçoit un courriel avec un bouton : une réponse s'ouvre avec les questions (identité, naissance, numéro de sécurité sociale, adresse, compte bancaire), elle y répond et l'envoie. Sa fiche se crée seule à l'arrivée de la réponse ; le métier se choisit ensuite à chaque engagement.",
            sans_boite: geste.is_none().then_some("Il faut d'abord lier la boîte courriel de la compagnie (Configuration › Mots de passe)."),
            geste,
            demande: crate::coffre::demande(&ctx.racine, super::GESTE_DEMANDER_FICHE)?,
        };
        let note = Some("Ou remplissez la fiche vous-même ci-dessous. Le matricule se donne à l'enregistrement.");
        return Ok(EnteteDeLaFiche { invitation: Some(invitation), ..EnteteDeLaFiche::de("Nouveau salarié".to_string(), None, note, None) });
    };
    let lu = |a: Option<&str>, b: Option<&str>| [a, b].into_iter().flatten().map(str::trim).find(|t| !t.is_empty()).map(str::to_string);
    let nom: Vec<String> = [lu(p.prenom_affichage.portee(), p.prenom.portee()), lu(p.nom_affichage.portee(), p.nom.portee())].into_iter().flatten().collect();
    let titre = if nom.is_empty() { cle.to_string() } else { nom.join(" ") };
    let matricule = p.code_salarie.portee().map(str::trim).filter(|m| !m.is_empty()).map(|m| format!("matricule {}", m));
    Ok(EnteteDeLaFiche {
        salarie: Some(SalarieDeLEntete { cle: cle.clone(), exporter: "Exporter ses documents" }),
        ..EnteteDeLaFiche::de(titre, matricule, None, Some(CoutDeLaFiche::Salarie { de: cle.clone() }))
    })
}

/// UNE CASE OUVERTE D'UNE FICHE, telle que la page la dessine : sa cle, son bloc (le titre sous lequel elle se range,
/// `""` : aucun) et sa largeur, son libelle, sa forme, sa place, son aide (la bulle au survol), l'exemple d'une case vide,
/// sa valeur (celle de la fiche, ou ce que le moteur propose d'y ecrire : « Pour démarrer »), ce que le moteur prend pour
/// elle vide (`prerempli`), les choix d'une case qui se choisit (code et libelle), s'ils se montrent en boutons a cocher,
/// chacun avec le document qui le montre quand il en a un (`boutons`, le modele du bulletin), ses parties (une case faite de cases,
/// ou chaque rangee d'une liste), le bouton qui ajoute une rangee, ce que le moteur dit sous elle (`dit`) et ou elle
/// renvoie (`renvoi`), ses temps (la valeur d'avant et celle a venir, en mots) et sa faute.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct CaseDeLaFiche {
    cle: CaseDeFiche,
    bloc: String,
    largeur: Largeur,
    libelle: String,
    forme: Forme,
    place: PlaceDeLaCase,
    aide: String,
    valeur: Option<ValeurDeCase>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exemple: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prerempli: Option<ValeurDeCase>,
    #[serde(skip_serializing_if = "Option::is_none")]
    choix: Option<Vec<Choix>>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    boutons: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    parties: Option<Vec<PartieDeLaFiche>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ajouter: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    dit: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    renvoi: Option<RenvoiDeLaFiche>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temps: Option<TempsDeLaCase<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    anomalie: Option<Anomalie>,
}

/// UNE PARTIE D'UNE CASE FAITE DE CASES (le signataire, le compte bancaire, le taux AT/MP, une rangee d'un autre
/// etablissement) : son chemin dans la valeur (`taux.t1_cadre`), son libelle, sa forme, son aide, son exemple, ce que le
/// moteur prend pour elle vide, ses choix, s'ils se montrent en boutons a cocher (chacun une phrase entiere), et si elle
/// est ouverte (une partie fermee est vide : elle n'a pas de sens pour ce qu'on saisit). Une cle de la valeur que la
/// route ne rend pas en partie (une forme declaree qui se deduit, une note, d'ou vient un taux) se garde telle quelle.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct PartieDeLaFiche {
    cle: String,
    libelle: String,
    forme: Forme,
    #[serde(skip_serializing_if = "Option::is_none")]
    aide: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exemple: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    prerempli: Option<ValeurDeCase>,
    #[serde(skip_serializing_if = "Option::is_none")]
    choix: Option<Vec<Choix>>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    boutons: bool,
    ouverte: bool,
}

/// LES VALEURS D'UNE CASE DATEE, en mots, pour l'historique replie de la fiche : son libelle et ses temps.
#[derive(Debug, Clone, Serialize)]
struct HistoireDUneCase {
    libelle: String,
    #[serde(flatten)]
    temps: TempsDeLaCase<String>,
}

/// CE QUE REND `fiche` : la phrase de sa verification, son en-tete, l'encart de ses erreurs (son titre, aucun sans
/// erreur, et sa note : `ouverture::erreurs_dites`), ses cases ouvertes dans l'ordre, si « Avancé » s'ouvre (une de ses
/// cases s'ecarte du cas ordinaire : `champs::Champ::ecart`), les cases dont la route a recrit la valeur saisie (le
/// numero de securite sociale colle d'un coup, ce que le metier choisi remplit : la page les montre telles que la route
/// les rend), et l'historique de ses cases datees.
#[derive(Serialize)]
pub(crate) struct FicheRendue {
    phrase: Phrase,
    phrases: Vec<String>,
    #[serde(flatten)]
    entete: EnteteDeLaFiche,
    erreurs_titre: Option<String>,
    erreurs_note: &'static str,
    cases: Vec<CaseDeLaFiche>,
    avance_ouvert: bool,
    reecrites: Vec<CaseDeFiche>,
    historique: Vec<HistoireDUneCase>,
}

impl Dite for FicheRendue {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

/// `fiche` : la fiche nommee, rendue (`rendre`). Une lecture : elle ne refuse que ce qui ne se lit pas (une cle
/// inconnue) ; une case que son type refuse est sa faute, et le reste se rend.
pub(crate) fn fiche(ctx: &Contexte, charge: FicheARendre) -> R<FicheRendue> {
    match charge {
        FicheARendre::Entreprise { changements, changees } => rendre::<FicheEntrepriseDatee>(ctx, None, changements, changees),
        FicheARendre::Salarie { cle, changements, changees } => rendre::<FicheSalarieDatee>(ctx, cle, changements, changees),
        FicheARendre::Spectacle { cle, changements, changees } => rendre::<FicheSpectacleDatee>(ctx, cle, changements, changees),
    }
}

/// LA FICHE `cle` RENDUE : l'enregistree (neuve sans cle), sa forme lisible comblee (`comblee`), et ce qui est saisi pose
/// dessus (`changements`, comme a l'ecriture : une case qu'on ne nomme pas garde sa valeur, une case videe est vide) ;
/// ses anomalies (sans saisie, la verification de sa pastille dans la liste, `anomalies_enregistrees` ; avec, comme a
/// l'ecriture, `anomalies_saisies`), les cases que la fiche montre au jour de l'ecran (`fiches::montres`), dans l'ordre
/// de la table, et ses temps. Ce que la route recrit de la saisie suit les cases qui viennent de changer (`changees`).
fn rendre<F: SorteDeFiche>(ctx: &Contexte, cle: Option<F::Cle>, changements: Option<socle::forme::Liste>, changees: Option<Vec<CaseDeFiche>>) -> R<FicheRendue> {
    let jour = ctx.aujourdhui;
    let enregistree = match (F::montree(ctx, cle.as_ref()), &cle) {
        (Some(p), _) => p,
        (None, None) => F::retenue(F::Plate::default()),
        (None, Some(c)) => return Err(socle::echec::erreur(Classe::Manque, format!("Aucune fiche {} « {} ».", F::NOM, F::cle_dite(c)))),
    };
    let depart = comblee::<F>(&enregistree)?;
    let mut plate = depart.clone();
    let (mut reecrites, fautes, saisie) = match &changements {
        None => (Vec::new(), Vec::new(), false),
        Some(liste) => {
            let (lus, mut fautes) = Changements::<F::Changement>::lire(liste).map_err(|e| e.reclasser(Classe::MalFormee))?;
            for ch in lus.0 {
                F::appliquer(&mut plate, ch);
            }
            // Aucune case nommee : aucune ne vient de changer.
            let changees: &[CaseDeFiche] = match &changees {
                Some(l) => l,
                None => &[],
            };
            (F::reprendre_la_saisie(ctx, liste, changees, &mut plate, &mut fautes)?, fautes, true)
        }
    };
    // A l'ouverture, aucune liste ; a une relecture, les cases qui viennent de changer (aucune nommee : aucune).
    let venues: Option<&[CaseDeFiche]> = match (&changements, &changees) {
        (None, _) => None,
        (Some(_), Some(l)) => Some(l),
        (Some(_), None) => Some(&[]),
    };
    reecrites.extend(F::proposer(&mut plate, venues)?);
    // Sans saisie ni proposition, la verification de la pastille ; sinon celle de la fiche telle qu'elle se montre.
    let anomalies = if saisie || !reecrites.is_empty() {
        let tapee = Tapee { fiche: plate, fautes };
        let anomalies = anomalies_saisies::<F>(ctx, &tapee)?;
        plate = tapee.fiche;
        anomalies
    } else {
        anomalies_enregistrees::<F>(&enregistree)?
    };
    let (phrase, phrases) = verdict(&anomalies);
    // L'entreprise d'une fiche de salarie ou de spectacle est celle de la compagnie ; celle de la fiche entreprise, la
    // fiche elle-meme, telle qu'on la saisit.
    let entreprise: &ChampsEntreprise = match F::a_juger(&plate) {
        FicheAJuger::Entreprise(e) => e,
        FicheAJuger::Salarie(_) | FicheAJuger::Spectacle(_) => &ctx.entreprise,
    };
    // Les permanents du jour ne se lisent que pour la fiche qui en a l'usage (`fiches::montres`).
    let dus = || Ok(!engagement::permanents::Permanents::de(ctx, &socle::date::Periode::du_jour(jour))?.dus.is_empty());
    let super::Montres { cases: montres, .. } = super::montres(ctx, F::a_juger(&plate), cle.as_ref().and_then(F::salarie), entreprise, &dus)?;
    let table = champs::fiche(F::NOM)?;
    let historique = historique::<F>(ctx, cle.as_ref(), &table, &jour)?;
    let mobilite = crate::ouverture::versement_mobilite_des_etablissements(ctx);
    // L'organisme Urssaf que le code postal de la compagnie donne : le cas ordinaire du code Urssaf (`Avance::DuCodePostal`).
    let du_code_postal = match entreprise.code_postal.portee() {
        Some(cp) => declarations::urssaf::base()?.du_code_postal(cp),
        None => None,
    };
    let listes = std::cell::OnceCell::new();
    let completer = Completer::de(ctx, F::a_juger(&plate), &|k| montres.contains(&k), &listes)?;
    let mut cases = Vec::new();
    let mut avance_ouvert = false;
    for c in table.iter().filter(|c| montres.contains(&c.cle) || completer.en_lecture(c.cle)) {
        let ecrite = |chemin: &[&str]| plate.ecrite_au_chemin(c.cle, chemin);
        let Some(mut case) = case_du_champ(c, plate.valeur_ecrite(c.cle), &ecrite, mobilite)? else { continue };
        if !completer.case(ctx, &mut case)? {
            continue;
        }
        let ouverte = montres.contains(&c.cle);
        if !ouverte {
            case.forme = Forme::Objet;
            case.choix = None;
        }
        avance_ouvert |= ouverte && case.place == PlaceDeLaCase::Avance && c.ecart(&plate, du_code_postal.as_ref().map(|c| c.as_str()));
        case.anomalie = anomalies.iter().find(|a| a_l_ecran(&table, a.champ) == c.cle).cloned();
        case.temps = historique.iter().find(|h| h.temps.champ == CaseDatee::Case(c.cle)).map(|h| h.temps.clone());
        cases.push(case);
    }
    let (erreurs_titre, erreurs_note) = crate::ouverture::erreurs_dites(anomalies.iter().filter(|a| a.gravite == Gravite::Erreur).count());
    Ok(FicheRendue { phrase, phrases, entete: F::entete(ctx, cle.as_ref(), &enregistree)?, erreurs_titre, erreurs_note, cases, avance_ouvert, reecrites, historique })
}

/// CE QUE LE MOTEUR SAIT DE PLUS SUR LES CASES D'UNE FICHE, telle qu'on la saisit, qu'il rend avec elles : celles de
/// l'entreprise (`completer_l_entreprise`), celles d'un salarie (`completer_le_salarie`, avec les listes lues une fois
/// par reponse, et sa complementaire sante quand sa case s'ouvre) ; un spectacle n'en a pas.
enum Completer<'a> {
    Entreprise(&'a ChampsEntreprise),
    Salarie(&'a droit::fiche::ChampsSalarie, &'a ListesDuSalarie, Option<SanteDuSalarie>),
    Rien,
}

impl<'a> Completer<'a> {
    /// Pour la fiche `fiche`, dont les cases `montree` s'ouvrent ; `listes` : celles d'un salarie, lues a la premiere
    /// fiche de salarie de la reponse qui les demande, puis gardees.
    fn de(ctx: &Contexte, fiche: FicheAJuger<'a>, montree: &dyn Fn(CaseDeFiche) -> bool, listes: &'a std::cell::OnceCell<ListesDuSalarie>) -> R<Completer<'a>> {
        Ok(match fiche {
            FicheAJuger::Entreprise(e) => Completer::Entreprise(e),
            FicheAJuger::Salarie(s) => {
                let lues = match listes.get() {
                    Some(l) => l,
                    None => {
                        let l = ListesDuSalarie::lire(ctx)?;
                        listes.get_or_init(|| l)
                    }
                };
                let sante = if montree(droit::fiche::CasePieceSalarie::SanteSvp.into()) { Some(SanteDuSalarie::lire(ctx)?) } else { None };
                Completer::Salarie(s, lues, sante)
            }
            FicheAJuger::Spectacle(_) => Completer::Rien,
        })
    }

    /// LA CASE QUE LA FICHE FERME ET QUE LA ROUTE REND POURTANT, EN LECTURE : la profession de la deduction forfaitaire
    /// d'un salarie quand la compagnie n'exerce pas l'option (rien ne s'y saisit ; elle porte la phrase qui le dit et le
    /// renvoi a la fiche de l'entreprise, ou l'option se pose).
    fn en_lecture(&self, k: CaseDeFiche) -> bool {
        match self {
            Completer::Salarie(_, listes, _) => !listes.deduction_forfaitaire_exercee && k == CaseDeFiche::from(droit::fiche::CaseSalarie::DeductionForfaitaire),
            Completer::Entreprise(_) | Completer::Rien => false,
        }
    }

    /// La case completee ; faux : la route ne l'ouvre pas (`completer_le_salarie`).
    fn case(&self, ctx: &Contexte, case: &mut CaseDeLaFiche) -> R<bool> {
        match self {
            Completer::Entreprise(e) => completer_l_entreprise(ctx, e, case).map(|_| true),
            Completer::Salarie(s, listes, sante) => completer_le_salarie(ctx, s, listes, sante.as_ref(), case),
            Completer::Rien => Ok(true),
        }
    }
}

/// LA CASE QUE LE MOTEUR PROPOSE DE REMPLIR (« Pour démarrer » : `routes::rappels`), sur la fiche `fiche`, de valeur
/// `valeur` (`ecrite` : ce qu'ecrit la fiche, case et chemin) : dessinee comme dans sa fiche. La case d'une structure
/// n'a que la partie qui manque (les autres se gardent, la route ne les rendant pas). Aucune pour une case qui ne se
/// saisit pas, que la fiche n'ouvre pas, ou une cle que la table ne connait pas. `listes` : celles d'un salarie, lues
/// une fois pour toutes les lignes de la reponse.
pub(crate) fn case_a_remplir(
    ctx: &Contexte,
    listes: &std::cell::OnceCell<ListesDuSalarie>,
    fiche: FicheAJuger<'_>,
    chemin: &CheminDeFiche,
    valeur: Option<ValeurDeCase>,
    ecrite: &dyn Fn(CaseDeFiche, &[&str]) -> MotEcrit,
) -> R<Option<CaseDeLaFiche>> {
    let Some(c) = champs::champ(fiche.nom(), chemin.champ)?.filter(|c| c.ecran && !c.calcule()) else { return Ok(None) };
    let mobilite = crate::ouverture::versement_mobilite_des_etablissements(ctx);
    let Some(mut case) = case_du_champ(&c, valeur, &|ch: &[&str]| ecrite(c.cle, ch), mobilite)? else { return Ok(None) };
    if !Completer::de(ctx, fiche, &|k| k == c.cle, listes)?.case(ctx, &mut case)? {
        return Ok(None);
    }
    if let (Some(dans), Some(parties)) = (&chemin.dans, case.parties.as_mut()) {
        let voulue = dans.to_string();
        parties.retain(|p| p.cle == voulue);
        parties.iter_mut().for_each(|p| p.ouverte = true);
    }
    Ok(Some(case))
}

/// LA CASE A SAISIR DE NOUVEAU (`crate::redire`) : la case `k` de la fiche `nom`, dessinee comme dans sa fiche, vide.
/// Aucune pour une cle que la table ne connait pas, ou pour la ligne de l'histoire.
pub(crate) fn case_a_redire(ctx: &Contexte, nom: NomDeFiche, k: CaseDeFiche) -> R<Option<CaseDeLaFiche>> {
    let Some(c) = champs::champ(nom, k)? else { return Ok(None) };
    let mobilite = crate::ouverture::versement_mobilite_des_etablissements(ctx);
    case_du_champ(&c, None, &|_: &[&str]| MotEcrit::Vide, mobilite)
}

/// La case que la fiche montre pour la cle `k` : la forme lisible dont elle se deduit, ou elle-meme.
fn a_l_ecran(table: &[Champ], k: CaseDeFiche) -> CaseDeFiche {
    match table.iter().find(|c| c.cle == k) {
        Some(c) => c.a_l_ecran(),
        None => k,
    }
}

/// LA CASE DU CHAMP `c` de valeur `valeur`, sans temps ni faute (`ecrite` : ce qu'ecrit la fiche au bout d'un chemin
/// dans la case ; `mobilite` : les autres etablissements disent leur versement mobilite) ; aucune pour la ligne de
/// l'histoire, qui n'est pas une case. Une case faite de cases rend ses parties ; une liste de cases, ses rangees.
fn case_du_champ(c: &Champ, valeur: Option<ValeurDeCase>, ecrite: &dyn Fn(&[&str]) -> MotEcrit, mobilite: bool) -> R<Option<CaseDeLaFiche>> {
    let parties_decrites = c.champs.as_deref().filter(|l| !l.is_empty());
    // UNE LISTE DE VALEURS D'UNE LISTE FERMEE (les jours travailles) : sa forme les declare (`Declaree::valeurs`), elle se
    // coche valeur par valeur.
    let valeurs = c.declaree.as_ref().map(|d| d.valeurs).filter(|v| !v.is_empty());
    let forme = match (c.forme, parties_decrites, &c.ligne, valeurs) {
        (champs::Forme::Histoire, _, _, _) => return Ok(None),
        (champs::Forme::Structure, Some(_), Some(_), _) => Forme::Rangees,
        (champs::Forme::Structure, Some(_), None, _) => Forme::Parties(&[]),
        (champs::Forme::Structure, None, _, Some(_)) => Forme::Liste,
        (f, _, _, _) => forme_de(f),
    };
    let choix = match (c.forme, valeurs) {
        (champs::Forme::Choix, _) => Some(choix_de(&c.choix, c.libelles.as_ref())),
        (champs::Forme::Structure, Some(v)) if parties_decrites.is_none() => Some(v.iter().map(|x| Choix::de(x, x)).collect()),
        _ => None,
    };
    let parties = match parties_decrites {
        Some(l) => Some(l.iter().filter(|s| s.ecran && !s.calcule()).map(|s| partie_de(s, ecrite, mobilite)).collect::<R<Vec<_>>>()?),
        None => None,
    };
    let place = match c.avance {
        Some(_) => PlaceDeLaCase::Avance,
        None => PlaceDeLaCase::Base,
    };
    // Du HTML colle se rend nettoye, comme il s'ecrit et comme il part (`envois::signature_courriel::nettoyer`).
    let valeur = match (c.forme, valeur) {
        (champs::Forme::Html, Some(v)) => match v.ecrite() {
            MotEcrit::Mot(t) => Some(ValeurDeCase::texte(envois::signature_courriel::nettoyer(&t))),
            _ => Some(v),
        },
        (_, v) => v,
    };
    Ok(Some(CaseDeLaFiche {
        cle: c.cle,
        bloc: c.bloc.clone(),
        largeur: c.largeur,
        libelle: c.libelle.clone(),
        forme,
        place,
        aide: c.aide.clone(),
        valeur,
        exemple: Some(c.exemple.clone()).filter(|e| !e.is_empty()),
        prerempli: None,
        choix,
        boutons: false,
        parties,
        ajouter: c.ligne.as_ref().map(|l| format!("+ Ajouter un {}", l)),
        dit: Vec::new(),
        renvoi: None,
        temps: None,
        anomalie: None,
    }))
}

/// LA PARTIE `s` D'UNE CASE FAITE DE CASES : ouverte quand les autres cases de sa structure l'ouvrent (`si`, sur ce que la
/// fiche ecrit dans la structure) et que la compagnie en a l'usage (`si_configuration` : le versement mobilite des autres
/// etablissements).
fn partie_de(s: &Case, ecrite: &dyn Fn(&[&str]) -> MotEcrit, mobilite: bool) -> R<PartieDeLaFiche> {
    let configuration = match s.si_configuration.as_deref() {
        None => true,
        Some("versement_mobilite_des_etablissements") => mobilite,
        Some(autre) => return Err(socle::echec::livree(&champs::BASE, &format!("{} : la configuration ne dit pas « {} »", s.cle, autre))),
    };
    let ouverte = configuration && s.si.as_ref().is_none_or(|si| si.remplie_par(&|n| ecrite(&[n.nom()])));
    let forme = match s.forme {
        champs::Forme::Choix => Forme::ChoixRendu,
        champs::Forme::Booleen => Forme::OuiNon,
        f => forme_de(f),
    };
    Ok(PartieDeLaFiche {
        cle: s.cle.to_string(),
        libelle: s.libelle.clone(),
        forme,
        aide: s.aide.clone().filter(|a| !a.is_empty()),
        exemple: s.exemple.clone().filter(|e| !e.is_empty()),
        prerempli: None,
        choix: s.choix.as_deref().map(|l| choix_de(l, s.libelles.as_ref())),
        boutons: s.boutons == Some(true),
        ouverte,
    })
}

/// LA FORME QU'ON SAISIT D'UNE CASE SIMPLE DE FICHE (`champs::Forme`, ce qu'on en verifie) : un texte (de toute ecriture,
/// un code risque, un pays par son code ISO), un jour, un montant, une valeur d'une liste fermee que la route rend a cote
/// (`choix`), un oui-non, du HTML colle, une valeur composee que la table ne decrit pas case par case (en lecture).
fn forme_de(f: champs::Forme) -> Forme {
    use champs::Forme as F;
    match f {
        F::Texte | F::Officiel | F::Affichage | F::Chiffres | F::Email | F::CodeAtmp | F::Pays => Forme::Texte,
        F::Date => Forme::Date,
        F::Montant => Forme::Montant,
        F::Choix => Forme::ChoixRendu,
        F::Booleen => Forme::OuiNon,
        F::Html => Forme::Html,
        F::Structure | F::Histoire => Forme::Objet,
    }
}

/// Le choix vide d'une liste qui n'en dit pas : rien n'est choisi.
const A_CHOISIR: &str = "— à choisir —";

/// CE QU'OFFRE UNE CASE QUI SE CHOISIT : chacune de ses valeurs (`choix` de `champs-base.json`), dans l'ordre de la table,
/// avec le libelle que la table lui donne (`libelles`), ou telle qu'elle s'ecrit ; la ligne vide en tete, avec le libelle
/// que la table lui donne (ce que vaut le vide), sinon « — à choisir — ».
fn choix_de(choix: &[String], libelles: Option<&Libelles>) -> Vec<Choix> {
    let libelle = |code: &str| match libelles.and_then(|l| l.de(code)) {
        Some(l) => l.to_string(),
        None => code.to_string(),
    };
    let vide = match libelles.and_then(|l| l.de("")) {
        Some(l) => l.to_string(),
        None => A_CHOISIR.to_string(),
    };
    let tete = (!choix.iter().any(String::is_empty)).then(|| Choix::de("", &vide));
    tete.into_iter().chain(choix.iter().map(|code| Choix::de(code, if code.is_empty() { vide.clone() } else { libelle(code) }))).collect()
}

/// LES LISTES QUE LA FICHE D'UN SALARIE OFFRE, lues une fois par reponse (la fiche rendue, ou toutes les lignes de
/// « Pour démarrer ») : les pays de naissance (la table « Pays » de la DSN), les pays de nationalite (le COG de l'INSEE,
/// chacun avec la nationalite DSN qui s'en deduit), les metiers qu'une fiche choisit (`metiers::metiers_d_une_fiche`), et
/// ce que la fiche de l'entreprise dit de la deduction forfaitaire specifique (sa phrase, et si la compagnie l'exerce).
pub(crate) struct ListesDuSalarie {
    pays_naissance: Vec<Choix>,
    pays: Vec<droit::nationalite::PaysDeNationalite>,
    metiers: Vec<Choix>,
    deduction_forfaitaire: String,
    deduction_forfaitaire_exercee: bool,
}

/// LA COMPLEMENTAIRE SANTE D'UN SALARIE AU MOIS, telle que la fiche la fait dire : la convention ne la fait pas choisir
/// sur la fiche (d'ou elle vient : `bareme::sante_hors_fiche`), le bareme du jour ne se compose pas (ce qui manque), ou
/// les niveaux de garantie du bareme, ou l'on choisit. Elle ne se lit que quand sa case s'ouvre : elle compose le bareme
/// du jour.
enum SanteDuSalarie {
    HorsFiche(String),
    PourPayer(String),
    Niveaux(Vec<Choix>),
}

impl SanteDuSalarie {
    fn lire(ctx: &Contexte) -> R<SanteDuSalarie> {
        Ok(match droit::bareme::sante_hors_fiche(&ctx.entreprise)? {
            Some(d) => SanteDuSalarie::HorsFiche(d),
            None => match crate::ouverture::niveaux_sante(ctx)? {
                Ok(niveaux) => SanteDuSalarie::Niveaux(std::iter::once(Choix::de("", "— pas encore dite —")).chain(niveaux.iter().map(|n| Choix::de(&n.niveau, &n.libelle))).collect()),
                Err(manque) => SanteDuSalarie::PourPayer(manque),
            },
        })
    }
}

/// Ce que dit une case que le moteur remplit depuis le numero de securite sociale, tant qu'elle est vide.
const LU_DANS_LE_NIR: &str = "Rempli pour vous — lu dans le numéro de sécurité sociale.";

/// Ce que dit le pays de naissance d'une personne que le numero de securite sociale dit nee en metropole.
const NE_EN_METROPOLE: &str = "Le numéro de sécurité sociale dit une naissance en France métropolitaine.";

/// LE NUMERO DE SECURITE SOCIALE DE `s` DIT-IL UNE NAISSANCE EN FRANCE METROPOLITAINE ? Son lieu de naissance (6e et 7e
/// caracteres, `champs::departement_du_nir`) est, pour une personne nee en metropole, le departement du code officiel
/// geographique, 2A et 2B pour la Corse depuis 1976 ; trois chiffres en 97 pour les DOM ; 99 a l'etranger
/// (`atelier/reference/sources/insee-nir-siret/insee-schema-nir-20261028.txt`, l. 125). Le departement de naissance de
/// la DSN admet 20 avant 1976, 98 pour les TOM, 96 avant 1968 seulement
/// (`atelier/reference/norme-dsn/texte-2027/bloc-S21.G00.30.txt`, S21.G00.30.014). Les departements de metropole du COG
/// sont 01 a 19, 2A, 2B, 21 a 95 (`atelier/reference/sources/insee-cog-2015-departements.zip`). Oui donc pour 01 a 90,
/// 2A, 2B, et 20 (la Corse d'avant 1976) ; pour 91 a 95, seulement d'une personne nee a partir de 1968, la date de
/// naissance de la fiche le disant (INFERENCE : la DSN date de 1968 le changement de 96, aucun texte depose ne dit ce que
/// 91 a 95 nommaient avant) ; non pour 96, 97, 98, 99. La table « Pays » de la DSN a ses propres codes pour les DOM et
/// les TOM (`logiciel/donnees/pays-dsn-base.json`) : un 97 ou un 98 ne dit donc pas FR.
fn ne_en_metropole(s: &droit::fiche::ChampsSalarie) -> bool {
    let Some(dep) = s.nir.dite().and_then(|n| champs::departement_du_nir(n.as_ref())) else { return false };
    match dep.as_str() {
        "2A" | "2B" => true,
        d => match d.parse::<u8>() {
            Ok(1..=90) => true,
            Ok(91..=95) => s.date_naissance.dite().is_some_and(|j| socle::mois::Annee::du_jour(j) >= socle::annee!("1968")),
            _ => false,
        },
    }
}

/// UN PAYS QUE LA LISTE N'OFFRE PAS (une fiche d'avant la liste, une piece reprise) reste offert dans la case, dit
/// « CODE — inconnu de la liste » : sa valeur se garde, et l'on voit qu'elle est a choisir de nouveau.
fn garder_hors_liste(case: &mut CaseDeLaFiche) {
    let Some(MotEcrit::Mot(code)) = case.valeur.as_ref().map(ValeurDeCase::ecrite) else { return };
    if let Some(choix) = case.choix.as_mut().filter(|l| !l.iter().any(|x| x.code() == code.as_str())) {
        choix.push(Choix::de(&code, format!("{} — inconnu de la liste", code)));
    }
}

/// CE QUE LA ROUTE PROPOSE DANS LA FICHE D'UN SALARIE `plate` (`SorteDeFiche::proposer`) : France dans un pays de
/// naissance vide quand le numero de securite sociale dit une naissance en metropole (`ne_en_metropole`), « FR » de la
/// table « Pays » de la DSN, a l'ouverture de la fiche et quand on vient de taper le numero (`changees` le nomme) : un
/// pays qu'on vide ensuite reste vide. Rien ne l'ecrit d'autre que « Enregistrer ».
pub(super) fn proposer_au_salarie(plate: &mut droit::fiche::ChampsSalarie, changees: Option<&[CaseDeFiche]>) -> R<Vec<CaseDeFiche>> {
    use droit::fiche::CaseSalarie as S;
    let a_l_instant = changees.is_none_or(|l| l.contains(&S::Nir.into()));
    if !a_l_instant || plate.pays_naissance.is_some() || !ne_en_metropole(plate) {
        return Ok(Vec::new());
    }
    plate.pays_naissance = Some(socle::pays::CodePays::france());
    Ok(vec![S::PaysNaissance.into()])
}

impl ListesDuSalarie {
    fn lire(ctx: &Contexte) -> R<ListesDuSalarie> {
        let a_choisir = || Choix::de("", A_CHOISIR);
        let pays_naissance = std::iter::once(a_choisir()).chain(socle::pays::servir_naissance()?.into_iter().map(|p| Choix::de(&p.code, &p.nom))).collect();
        let metiers = std::iter::once(a_choisir())
            .chain(
                crate::metiers::metiers_d_une_fiche(&crate::metiers::metiers(ctx)?)
                    .into_iter()
                    .map(|m| Choix::de(&m.code, format!("{}{}", m.libelle, if m.cadre_par_defaut { " (cadre)" } else { "" })).sous(Some(m.famille.to_string()))),
            )
            .collect();
        Ok(ListesDuSalarie {
            pays_naissance,
            pays: droit::nationalite::servir()?,
            metiers,
            deduction_forfaitaire: crate::ouverture::deduction_forfaitaire_dite(ctx)?,
            deduction_forfaitaire_exercee: ctx.entreprise.deduction_forfaitaire_option.code().is_some(),
        })
    }
}

/// CE QUE LE MOTEUR SAIT DE PLUS SUR UNE CASE DE LA FICHE D'UN SALARIE `s` (telle qu'on la saisit), qu'il rend avec elle ;
/// faux : il ne l'ouvre pas. Les pays se choisissent dans leurs listes ; le departement de naissance qu'une case vide
/// prend du numero de securite sociale (le moteur l'ecrit a l'enregistrement, `SorteDeFiche::deriver`) ; sous le pays de
/// naissance, ce que le numero dit d'une naissance en metropole (la route y propose France : `proposer_au_salarie`) ; avec un
/// pays de nationalite de la liste, la nationalite DSN s'en deduit et ne s'ouvre pas, ce qui s'en deduit se dit sous le
/// pays ; le metier se choisit dans les metiers d'une fiche et renvoie a l'onglet ou l'on en ajoute ; la complementaire
/// sante dit ses niveaux, ou pourquoi elle ne se choisit pas ici (ses cases ne s'ouvrent pas, sa valeur se garde) ; la
/// deduction forfaitaire dit ce qu'en dit la fiche de l'entreprise, et y renvoie.
fn completer_le_salarie(ctx: &Contexte, s: &droit::fiche::ChampsSalarie, listes: &ListesDuSalarie, sante: Option<&SanteDuSalarie>, case: &mut CaseDeLaFiche) -> R<bool> {
    use droit::fiche::CaseSalarie as S;
    let est = |k: S| case.cle == CaseDeFiche::from(k);
    let du_nir = s.nir.dite().and_then(|n| champs::departement_du_nir(n.as_ref()));
    let vide = case.valeur.is_none();
    let pays_de_nationalite = s.pays_nationalite.dite().and_then(|code| listes.pays.iter().find(|p| p.code == *code));
    if est(S::PaysNaissance) {
        case.forme = Forme::ChoixRendu;
        case.choix = Some(listes.pays_naissance.clone());
        garder_hors_liste(case);
        if ne_en_metropole(s) {
            case.dit.push(NE_EN_METROPOLE.to_string());
        }
    } else if est(S::DepartementNaissance) {
        if let (true, Some(dep)) = (vide, du_nir) {
            case.prerempli = Some(ValeurDeCase::texte(dep));
            case.dit.push(LU_DANS_LE_NIR.to_string());
        }
    } else if est(S::PaysNationalite) {
        case.forme = Forme::ChoixRendu;
        case.choix = Some(std::iter::once(Choix::de("", A_CHOISIR)).chain(listes.pays.iter().map(|p| Choix::de(&p.code, &p.nom))).collect());
        garder_hors_liste(case);
        if let Some(p) = pays_de_nationalite {
            let table = champs::champ(NomDeFiche::Salarie, S::CodeNationaliteDsn)?;
            let code = p.codification_ue.to_string();
            let libelle = table.as_ref().and_then(|c| c.libelles.as_ref()).and_then(|l| l.de(&code)).map(str::to_string);
            let nationalite = match libelle {
                Some(l) => l,
                None => code,
            };
            case.dit.push(format!("Nationalité déclarée : {} — {}.", nationalite, p.deduit_dit));
        }
    } else if est(S::CodeNationaliteDsn) {
        return Ok(pays_de_nationalite.is_none());
    } else if est(S::CodeEmploiFt) {
        case.forme = Forme::ChoixRendu;
        let mut choix = listes.metiers.clone();
        // Un code que la liste n'offre pas (le metier d'une autre convention) reste offert, nomme par son metier.
        if let Some(code) = s.code_emploi_ft.as_ref().filter(|c| !choix.iter().any(|x| x.code() == c.as_str())) {
            if let Some(emploi) = droit::emplois::CodeEmploi::du_code_declare(code)? {
                choix.push(Choix::de(code.as_str(), emploi.metier(&ctx.racine)?.libelle()));
            }
        }
        case.choix = Some(choix);
        case.renvoi = Some(RenvoiDeLaFiche::Metiers { libelle: "Ajouter un métier" });
    } else if case.cle == CaseDeFiche::from(droit::fiche::CasePieceSalarie::SanteSvp) {
        match sante {
            None => {}
            Some(SanteDuSalarie::Niveaux(niveaux)) => {
                if let Some(p) = case.parties.iter_mut().flatten().find(|p| p.cle == "niveau") {
                    p.choix = Some(niveaux.clone());
                }
                case.dit.push("Obligatoire pour un salarié au mois (art. L911-7) : sans elle, le logiciel refuse de le payer plutôt que de cotiser moins que le contrat. Elle se lit sur son bulletin d'adhésion.".to_string());
            }
            Some(SanteDuSalarie::HorsFiche(dit) | SanteDuSalarie::PourPayer(dit)) => {
                // Rien a y choisir ici : la valeur de la fiche se lit et se garde telle quelle.
                case.forme = Forme::Objet;
                case.parties = None;
                case.dit.push(dit.clone());
            }
        }
    } else if est(S::DeductionForfaitaire) {
        case.dit.push(listes.deduction_forfaitaire.clone());
        case.renvoi = Some(RenvoiDeLaFiche::Case { libelle: "Fiche de l'entreprise", fiche: NomDeFiche::Entreprise, case: CaseEntreprise::DeductionForfaitaireOption.into() });
    }
    Ok(true)
}

/// CE QUE LA ROUTE RECRIT DE LA SAISIE D'UN SALARIE (`fiche`, a chaque relecture), posee sur `plate` ; les cases recrites.
/// LES QUINZE CHIFFRES DE LA CARTE VITALE, collees dans « Numéro de sécurité sociale », se repartissent entre le numero
/// (les treize premiers) et sa cle (les deux derniers) : la fiche les garde en deux cases, la cle verifiant le numero
/// (S21.G00.30.001, `atelier/reference/norme-dsn/texte/bloc-S21.G00.30.txt`). LE METIER QU'ON VIENT DE CHOISIR (le code
/// de la nomenclature est dans `changees`) remplit les cases que le metier decide (`emplois::ChampsDuMetier::CASES`) :
/// l'intitule de l'emploi, le PCS-ESE, le statut et les classements, comme sur la carte (`emplois::resoudre`, le cadre
/// du metier dans la convention de la compagnie), au feminin pour une salariee ; une case changee apres lui dans la meme
/// saisie garde ce qu'on y a tape, et une case que le metier ne dit pas (pas de PCS-ESE, pas de classement) se vide :
/// vide, la DSN lit le metier du catalogue, ou refuse en le nommant. Sans metier choisi a l'instant, ces cases gardent ce qu'elles disent. Un metier que le moteur ne resout pas le
/// dit sur sa case.
pub(super) fn reprendre_le_salarie(
    ctx: &Contexte,
    liste: &socle::forme::Liste,
    changees: &[CaseDeFiche],
    plate: &mut droit::fiche::ChampsSalarie,
    fautes: &mut Vec<FauteDeCase>,
) -> R<Vec<CaseDeFiche>> {
    use droit::fiche::{CaseSalarie as S, ChangementSalarie};
    let mut reecrites: Vec<CaseDeFiche> = Vec::new();
    for lu in liste.chacun::<CaseTapee>("valeur") {
        let Ok(CaseTapee { case, valeur }) = lu else { continue };
        let chiffres: String = valeur.chars().filter(|c| !matches!(c, ' ' | '.' | '-')).collect();
        if case != CaseDeFiche::from(S::Nir) || chiffres.len() != 15 || !chiffres.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let (numero, cle) = chiffres.split_at(13);
        fautes.retain(|f| f.case != CaseDeFiche::from(S::Nir) && f.case != CaseDeFiche::from(S::NirCle));
        for (k, t) in [(S::Nir, numero), (S::NirCle, cle)] {
            match FauteDeCase::pose::<ChangementSalarie>(k.into(), t) {
                Ok(ch) => FicheSalarieDatee::appliquer(plate, ch),
                Err(f) => fautes.push(f),
            }
            reecrites.push(k.into());
        }
    }
    let Some(rang) = changees.iter().rposition(|k| *k == CaseDeFiche::from(S::CodeEmploiFt)) else { return Ok(reecrites) };
    let tapees_apres = &changees[rang + 1..];
    let Some(code) = plate.code_emploi_ft.clone() else { return Ok(reecrites) };
    let Some(emploi) = droit::emplois::CodeEmploi::du_code_declare(&code)? else { return Ok(reecrites) };
    let feminin = declarations::ntt::sexe_de(plate.nir.dite().and_then(|n| droit::fiche::Sexe::du_nir(n.as_ref())), plate.sexe.code()) == Some("2");
    let m = match droit::emplois::resoudre(&emploi.metier(&ctx.racine)?, None, feminin, ctx.entreprise.idcc.dite()) {
        Ok(m) => m,
        Err(e) => {
            fautes.push(FauteDeCase { case: S::CodeEmploiFt.into(), chemin: None, phrase: e.phrase });
            return Ok(reecrites);
        }
    };
    macro_rules! remplir {
        ($champ:ident, $k:expr, $v:expr) => {
            let k: CaseDeFiche = $k.into();
            let v = $v;
            if !tapees_apres.contains(&k) && plate.$champ != v {
                plate.$champ = v;
                reecrites.push(k);
            }
        };
    }
    remplir!(emploi_affichage, S::EmploiAffichage, Some(m.emploi_affichage));
    remplir!(pcs_ese, S::PcsEse, m.pcs_ese);
    remplir!(statut, S::Statut, Some(m.statut));
    remplir!(statut_conventionnel, S::StatutConventionnel, m.statut_conventionnel);
    remplir!(statut_categoriel_rc, S::StatutCategorielRc, Some(m.statut_categoriel_rc));
    Ok(reecrites)
}

/// UN CHANGEMENT TAPE DONT LA VALEUR EST UN TEXTE : sa case et ce texte, lus tels quels (les quinze chiffres d'un NIR que
/// le type de sa case refuse).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CaseTapee {
    case: CaseDeFiche,
    valeur: String,
}

/// CE QUE LE MOTEUR SAIT DE PLUS SUR UNE CASE DE LA FICHE ENTREPRISE `e` (telle qu'on la saisit), qu'il rend avec elle :
/// les conventions qu'il sait servir (l'IDCC se choisit dans leur liste) ; les organismes Urssaf (le code se choisit, et
/// le vide dit celui que le code postal tape donne) ; sous le taux AT/MP, ce que `taux-atmp` dit, et la date qu'une ligne
/// neuve propose ; pour la taxe sur les salaires, les questions que sa conclusion ouvre, l'aide et les reponses de la
/// franchise chiffrees au bareme de l'annee, et ce qui en decoule (`fiches::regime_de_la_taxe`, CGI art. 231, 1) ; les
/// deux modeles du bulletin en boutons, chacun avec son bulletin d'exemple, et sans choix ecrit celui que le moteur
/// applique.
fn completer_l_entreprise(ctx: &Contexte, e: &ChampsEntreprise, case: &mut CaseDeLaFiche) -> R<()> {
    let est = |k: CaseEntreprise| case.cle == CaseDeFiche::from(k);
    if est(CaseEntreprise::Idcc) {
        case.forme = Forme::ChoixRendu;
        let servies = ctx.conventions_servies()?;
        let liste = servies.into_iter().map(|(idcc, c)| {
            let libelle = if c.libelle.trim().is_empty() { idcc.to_string() } else { c.libelle };
            Choix::de(&idcc, format!("IDCC {} — {}{}", idcc, libelle, if c.applicable { "" } else { " (taux non relevés)" }))
        });
        case.choix = Some(std::iter::once(Choix::de("", A_CHOISIR)).chain(liste).collect());
    } else if est(CaseEntreprise::CodeUrssaf) {
        case.forme = Forme::ChoixRendu;
        let urssaf = declarations::urssaf::base()?;
        let deduit = e.code_postal.portee().and_then(|cp| urssaf.du_code_postal(cp));
        let vide = match deduit.as_ref().and_then(|d| urssaf.organismes.iter().find(|o| o.code == *d)) {
            Some(o) => format!("— selon le code postal : {} — {} —", o.code, o.nom),
            None => "— code postal : aucun organisme ne s'en déduit —".to_string(),
        };
        let liste = urssaf.organismes.iter().map(|o| Choix::de(&o.code, format!("{} — {}", o.code, o.nom)));
        case.choix = Some(std::iter::once(Choix::de("", vide)).chain(liste).collect());
    } else if est(CaseEntreprise::TauxAtmp) {
        let (dit, propose) = crate::atmp::dit_sous_la_case(ctx)?;
        case.dit = dit;
        if let Some(p) = case.parties.iter_mut().flatten().find(|p| p.cle == "depuis") {
            p.prerempli = ValeurDeCase::de(&propose);
        }
    } else if est(CaseEntreprise::TaxeSurLesSalaires) {
        let (q, r) = super::regime_de_la_taxe(ctx, e, ctx.aujourdhui.annee())?;
        for p in case.parties.iter_mut().flatten() {
            p.ouverte = r.champs.contains(&p.cle.as_str());
            if p.cle == "sous_franchise" {
                p.aide = Some(q.aide.clone());
                p.choix = Some(vec![Choix::de("true", &q.sous), Choix::de("false", &q.dessus)]);
            }
        }
        case.dit = vec![r.consequence.to_string()];
    } else if est(CaseEntreprise::ModeleBulletin) {
        // Les deux modeles en boutons, chacun avec son bulletin d'exemple ; sans choix ecrit, celui que le moteur applique
        // (`documents::bulletin::modele_de_la_compagnie`) se montre choisi, et rien ne s'ecrit tant qu'on n'en change pas.
        case.boutons = true;
        case.choix = Some(
            droit::fiche::ModeleBulletin::TOUS
                .iter()
                .map(|m| Choix::de(m, m.nom()).montre_par(ExempleDuChoix::du_modele("voir l'exemple", *m)))
                .collect(),
        );
        if e.modele_bulletin.dite().is_none() {
            case.prerempli = Some(ValeurDeCase::texte(documents::bulletin::modele_de_la_compagnie(e).to_string()));
        }
    }
    Ok(())
}

/// L'HISTORIQUE DE LA FICHE `cle` a `jour` : chaque case datee qui a plus d'une valeur (`ouverture::temps_de`), nommee par
/// son libelle, chaque valeur en mots (`en_mots`).
fn historique<F: SorteDeFiche>(ctx: &Contexte, cle: Option<&F::Cle>, table: &[Champ], jour: &socle::date::Jour) -> R<Vec<HistoireDUneCase>> {
    let mut out = Vec::new();
    for t in crate::ouverture::temps_de(F::brute(ctx, cle), jour)? {
        let CaseDatee::Case(k) = t.champ else { continue };
        let champ = table.iter().find(|c| c.cle == k);
        let libelle = match champ {
            Some(c) => c.libelle.clone(),
            None => k.to_string(),
        };
        out.push(HistoireDUneCase { libelle, temps: t.dits(&|v| Some(en_mots(champ, v))) });
    }
    Ok(out)
}

/// UNE VALEUR D'UNE CASE, EN MOTS : le libelle d'une valeur d'une liste de choix, « oui » ou « non » d'une case a
/// cocher, le texte tel qu'il s'ecrit (`ValeurDeCase::en_mots`) ; une case vide, un tiret.
fn en_mots(champ: Option<&Champ>, v: Option<ValeurDeCase>) -> String {
    let Some(v) = v else { return "—".to_string() };
    if let (Some(libelles), MotEcrit::Mot(code)) = (champ.and_then(|c| c.libelles.as_ref()), v.ecrite()) {
        if let Some(l) = libelles.de(&code) {
            return l.to_string();
        }
    }
    // Une case faite de cases se dit par les libelles de celles que la fiche montre, jamais par leurs cles ; celles
    // qu'elle ne montre pas (la provenance d'un taux) ne se disent pas.
    if let (Some(cases), Some(valeurs)) = (champ.and_then(|c| c.champs.as_ref()), v.cases()) {
        let mut dits = Vec::new();
        for c in cases.iter().filter(|c| c.ecran) {
            let cle = c.cle.to_string();
            let Some((_, Some(x))) = valeurs.iter().find(|(k, _)| *k == cle) else { continue };
            let mot = match (c.libelles.as_ref(), x.ecrite()) {
                (Some(libelles), MotEcrit::Mot(code)) => match libelles.de(&code) {
                    Some(l) => l.to_string(),
                    None => x.en_mots(),
                },
                _ => x.en_mots(),
            };
            dits.push(format!("{} : {}", c.libelle, mot));
        }
        return dits.join(" · ");
    }
    v.en_mots()
}
