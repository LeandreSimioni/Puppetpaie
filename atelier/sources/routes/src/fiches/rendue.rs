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
use crate::ouverture::Choix;
use crate::temps::{CaseDatee, TempsDeLaCase};
use declarations::champs::{self, Anomalie, Case, Champ, CheminDeFiche, FicheAJuger, Gravite, Largeur, Libelles};
use droit::contexte::{Contexte, IdSalarie, IdSpectacle};
use droit::fiche::{CaseDeFiche, CaseEntreprise, Changements, ChampsEntreprise, FicheEntrepriseDatee, FicheSaisie, FicheSalarieDatee, FicheSpectacleDatee, Lue as _, MotEcrit, NomDeFiche, ValeurDeCase};
use engagement::carte::PlaceDeLaCase;
use socle::echec::{Classe, R};

use super::ecriture::{anomalies_enregistrees, anomalies_saisies, comblee, verdict, SorteDeFiche, Tapee};

/// LA FICHE A RENDRE (`fiche`) : `{"nom": …}`, sa cle pour une fiche enregistree (aucune : une fiche neuve, ou
/// l'entreprise), et ce qui est saisi (`changements`, comme a l'ecriture `enregistrer-fiche` et comme la carte : chacun
/// lu seul dans le type de sa case, pose sur la fiche enregistree ; absents : la fiche telle qu'elle est enregistree).
#[derive(Deserialize)]
#[serde(tag = "nom", deny_unknown_fields)]
pub(crate) enum FicheARendre {
    #[serde(rename = "entreprise")]
    Entreprise { changements: Option<socle::forme::Liste> },
    #[serde(rename = "salarie")]
    Salarie { cle: Option<IdSalarie>, changements: Option<socle::forme::Liste> },
    #[serde(rename = "spectacle")]
    Spectacle { cle: Option<IdSpectacle>, changements: Option<socle::forme::Liste> },
}

/// CE QUE L'EN-TETE D'UNE FICHE RENDUE DIT D'ELLE (`SorteDeFiche::entete`) : son titre, ce qui suit le titre, une note
/// dessous, le cout de l'annee qu'elle montre (`cout`), et si « Annuler » ramene a la liste (`annuler` : l'entreprise
/// n'a pas de liste) ; une fiche qui n'en dit rien n'a que son « Annuler ».
#[derive(Debug, Clone, Serialize)]
pub(crate) struct EnteteDeLaFiche {
    titre: Option<String>,
    sous_titre: Option<String>,
    note: Option<&'static str>,
    cout: Option<CoutDeLaFiche>,
    annuler: bool,
}

/// LE COUT DE L'ANNEE QU'UNE FICHE MONTRE EN TETE : ou le lire dans ce que rend la route `couts` (le cote, et la cle sous
/// laquelle elle range cette fiche : un spectacle par son titre). La route `fiche` ne le calcule pas : `couts` lit l'annee
/// entiere, une fois par annee et par seance (la page le garde), et la fiche se rend a chaque frappe.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "cote")]
pub(crate) enum CoutDeLaFiche {
    #[serde(rename = "spectacles")]
    Spectacle { de: String },
}

impl EnteteDeLaFiche {
    /// Un en-tete qui ne dit rien, « Annuler » ramenant a la liste.
    pub(crate) fn aucun() -> EnteteDeLaFiche {
        EnteteDeLaFiche { titre: None, sous_titre: None, note: None, cout: None, annuler: true }
    }

    /// Un en-tete qui ne dit rien, sans « Annuler » : une fiche sans liste ou revenir.
    pub(crate) fn sans_liste() -> EnteteDeLaFiche {
        EnteteDeLaFiche { annuler: false, ..EnteteDeLaFiche::aucun() }
    }

    /// Un titre, ce qui le suit, sa note et le cout de l'annee qu'il montre ; « Annuler » ramene a la liste.
    pub(crate) fn de(titre: String, sous_titre: Option<String>, note: Option<&'static str>, cout: Option<CoutDeLaFiche>) -> EnteteDeLaFiche {
        EnteteDeLaFiche { titre: Some(titre), sous_titre, note, cout, annuler: true }
    }
}

/// UNE CASE OUVERTE D'UNE FICHE, telle que la page la dessine : sa cle, son bloc (le titre sous lequel elle se range,
/// `""` : aucun) et sa largeur, son libelle, sa forme, sa place, son aide (la bulle au survol), l'exemple d'une case vide,
/// sa valeur (celle de la fiche, ou ce que le moteur propose d'y ecrire : « Pour démarrer »), ce que le moteur prend pour
/// elle vide (`prerempli`), les choix d'une case qui se choisit (code et libelle), ses parties (une case faite de cases,
/// ou chaque rangee d'une liste), le bouton qui ajoute une rangee, ce que le moteur dit sous elle (`dit`), ses temps (la
/// valeur d'avant et celle a venir, en mots) et sa faute.
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
    #[serde(skip_serializing_if = "Option::is_none")]
    parties: Option<Vec<PartieDeLaFiche>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ajouter: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    dit: Vec<String>,
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
/// erreur, et sa note : `ouverture::erreurs_dites`), ses
/// cases ouvertes dans l'ordre, et l'historique de ses cases datees.
#[derive(Serialize)]
pub(crate) struct FicheRendue {
    phrase: Phrase,
    phrases: Vec<String>,
    #[serde(flatten)]
    entete: EnteteDeLaFiche,
    erreurs_titre: Option<String>,
    erreurs_note: &'static str,
    cases: Vec<CaseDeLaFiche>,
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
        FicheARendre::Entreprise { changements } => rendre::<FicheEntrepriseDatee>(ctx, None, changements),
        FicheARendre::Salarie { cle, changements } => rendre::<FicheSalarieDatee>(ctx, cle, changements),
        FicheARendre::Spectacle { cle, changements } => rendre::<FicheSpectacleDatee>(ctx, cle, changements),
    }
}

/// LA FICHE `cle` RENDUE : l'enregistree (neuve sans cle), sa forme lisible comblee (`comblee`), et ce qui est saisi pose
/// dessus (`changements`, comme a l'ecriture : une case qu'on ne nomme pas garde sa valeur, une case videe est vide) ;
/// ses anomalies (sans saisie, la verification de sa pastille dans la liste, `anomalies_enregistrees` ; avec, comme a
/// l'ecriture, `anomalies_saisies`), les cases que la fiche montre au jour de l'ecran (`fiches::montres`), dans l'ordre
/// de la table, et ses temps.
fn rendre<F: SorteDeFiche>(ctx: &Contexte, cle: Option<F::Cle>, changements: Option<socle::forme::Liste>) -> R<FicheRendue> {
    let jour = ctx.aujourdhui;
    let enregistree = match (F::montree(ctx, cle.as_ref()), &cle) {
        (Some(p), _) => p,
        (None, None) => F::retenue(F::Plate::default()),
        (None, Some(c)) => return Err(socle::echec::erreur(Classe::Manque, format!("Aucune fiche {} « {} ».", F::NOM, F::cle_dite(c)))),
    };
    let mut plate = comblee::<F>(&enregistree)?;
    let anomalies = match changements {
        None => anomalies_enregistrees::<F>(&enregistree)?,
        Some(liste) => {
            let (lus, fautes) = Changements::<F::Changement>::lire(&liste).map_err(|e| e.reclasser(Classe::MalFormee))?;
            for ch in lus.0 {
                F::appliquer(&mut plate, ch);
            }
            let tapee = Tapee { fiche: plate, fautes };
            let anomalies = anomalies_saisies::<F>(ctx, &tapee)?;
            plate = tapee.fiche;
            anomalies
        }
    };
    let (phrase, phrases) = verdict(&anomalies);
    // L'entreprise d'une fiche de salarie ou de spectacle est celle de la compagnie ; celle de la fiche entreprise, la
    // fiche elle-meme, telle qu'on la saisit.
    let (entreprise, de_l_entreprise): (&ChampsEntreprise, bool) = match F::a_juger(&plate) {
        FicheAJuger::Entreprise(e) => (e, true),
        FicheAJuger::Salarie(_) | FicheAJuger::Spectacle(_) => (&ctx.entreprise, false),
    };
    // Les permanents du jour ne se lisent que pour la fiche qui en a l'usage (`fiches::montres`).
    let dus = || Ok(!engagement::permanents::Permanents::de(ctx, &socle::date::Periode::du_jour(jour))?.dus.is_empty());
    let super::Montres { cases: montres, .. } = super::montres(ctx, F::a_juger(&plate), cle.as_ref().and_then(F::salarie), entreprise, &dus)?;
    let table = champs::fiche(F::NOM)?;
    let historique = historique::<F>(ctx, cle.as_ref(), &table, &jour)?;
    let mobilite = crate::ouverture::versement_mobilite_des_etablissements(ctx);
    let mut cases = Vec::new();
    for c in table.iter().filter(|c| montres.contains(&c.cle)) {
        let ecrite = |chemin: &[&str]| plate.ecrite_au_chemin(c.cle, chemin);
        let Some(mut case) = case_du_champ(c, plate.valeur_ecrite(c.cle), &ecrite, mobilite)? else { continue };
        if de_l_entreprise {
            completer_l_entreprise(ctx, entreprise, &mut case)?;
        }
        case.anomalie = anomalies.iter().find(|a| a_l_ecran(&table, a.champ) == c.cle).cloned();
        case.temps = historique.iter().find(|h| h.temps.champ == CaseDatee::Case(c.cle)).map(|h| h.temps.clone());
        cases.push(case);
    }
    let (erreurs_titre, erreurs_note) = crate::ouverture::erreurs_dites(anomalies.iter().filter(|a| a.gravite == Gravite::Erreur).count());
    Ok(FicheRendue { phrase, phrases, entete: F::entete(cle.as_ref(), &enregistree), erreurs_titre, erreurs_note, cases, historique })
}

/// LA CASE QUE LE MOTEUR PROPOSE DE REMPLIR (« Pour démarrer » : `routes::rappels`), sur la fiche `nom`, de valeur
/// `valeur` (`ecrite` : ce qu'ecrit la fiche, case et chemin) : dessinee comme dans sa fiche. La case d'une structure
/// n'a que la partie qui manque (les autres se gardent, la route ne les rendant pas). Aucune pour une case qui ne se
/// saisit pas, ou une cle que la table ne connait pas.
pub(crate) fn case_a_remplir(ctx: &Contexte, nom: NomDeFiche, chemin: &CheminDeFiche, valeur: Option<ValeurDeCase>, ecrite: &dyn Fn(CaseDeFiche, &[&str]) -> MotEcrit) -> R<Option<CaseDeLaFiche>> {
    let Some(c) = champs::champ(nom, chemin.champ)?.filter(|c| c.ecran && !c.calcule()) else { return Ok(None) };
    let mobilite = crate::ouverture::versement_mobilite_des_etablissements(ctx);
    let Some(mut case) = case_du_champ(&c, valeur, &|ch: &[&str]| ecrite(c.cle, ch), mobilite)? else { return Ok(None) };
    if nom == NomDeFiche::Entreprise {
        completer_l_entreprise(ctx, &ctx.entreprise, &mut case)?;
    }
    if let (Some(dans), Some(parties)) = (&chemin.dans, case.parties.as_mut()) {
        let voulue = dans.to_string();
        parties.retain(|p| p.cle == voulue);
        parties.iter_mut().for_each(|p| p.ouverte = true);
    }
    Ok(Some(case))
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
    let forme = match (c.forme, parties_decrites, &c.ligne) {
        (champs::Forme::Histoire, _, _) => return Ok(None),
        (champs::Forme::Structure, Some(_), Some(_)) => Forme::Rangees,
        (champs::Forme::Structure, Some(_), None) => Forme::Parties(&[]),
        (f, _, _) => forme_de(f),
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
        choix: (c.forme == champs::Forme::Choix).then(|| choix_de(&c.choix, c.libelles.as_ref())),
        parties,
        ajouter: c.ligne.as_ref().map(|l| format!("+ Ajouter un {}", l)),
        dit: Vec::new(),
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

/// CE QUE LE MOTEUR SAIT DE PLUS SUR UNE CASE DE LA FICHE ENTREPRISE `e` (telle qu'on la saisit), qu'il rend avec elle :
/// les conventions qu'il sait servir (l'IDCC se choisit dans leur liste) ; les organismes Urssaf (le code se choisit, et
/// le vide dit celui que le code postal tape donne) ; sous le taux AT/MP, ce que `taux-atmp` dit, et la date qu'une ligne
/// neuve propose ; pour la taxe sur les salaires, les questions que sa conclusion ouvre, l'aide et les reponses de la
/// franchise chiffrees au bareme de l'annee, et ce qui en decoule (`fiches::regime_de_la_taxe`, CGI art. 231, 1).
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
    v.en_mots()
}
