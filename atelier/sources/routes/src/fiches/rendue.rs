//! LA FICHE RENDUE (`fiche`) : une fiche telle qu'on la saisit, rendue comme la route d'une carte rend la sienne
//! (`routes::mois::CarteRendue`). Ses cases ouvertes, dans l'ordre de `champs-base.json`, chacune son libelle, sa forme
//! (`catalogue::Forme`), sa place (de base ou sous « Avancé », `engagement::carte::PlaceDeLaCase`), son aide, sa valeur,
//! ses choix, ses temps et sa faute ; l'en-tete de la fiche, l'encart de ses erreurs et son historique, en mots. La page
//! les dessine par le dessinateur commun (`js/avance.js`) et ne decide rien. L'ecriture reste `enregistrer-fiche`.

use serde::{Deserialize, Serialize};

use crate::catalogue::Forme;
use crate::enveloppe::{Dite, Phrase};
use crate::ouverture::Choix;
use crate::temps::{CaseDatee, TempsDeLaCase};
use declarations::champs::{self, Anomalie, Champ, CheminDeFiche, FicheAJuger, Gravite};
use droit::contexte::{Contexte, IdSalarie, IdSpectacle};
use droit::fiche::{CaseDeFiche, Changements, ChampsEntreprise, FicheEntrepriseDatee, FicheSaisie, FicheSalarieDatee, FicheSpectacleDatee, MotEcrit, NomDeFiche, ValeurDeCase};
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

/// UNE CASE OUVERTE D'UNE FICHE, telle que la page la dessine : sa cle, son libelle, sa forme, sa place, son aide (la
/// bulle au survol), sa valeur (celle de la fiche, ou ce que le moteur propose d'y ecrire : « Pour démarrer »), les choix
/// d'une case qui se choisit (code et libelle), ses temps (la valeur d'avant et celle a venir, en mots) et sa faute.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct CaseDeLaFiche {
    cle: CaseDeFiche,
    libelle: String,
    forme: Forme,
    place: PlaceDeLaCase,
    aide: String,
    valeur: Option<ValeurDeCase>,
    #[serde(skip_serializing_if = "Option::is_none")]
    choix: Option<Vec<Choix>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temps: Option<TempsDeLaCase<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    anomalie: Option<Anomalie>,
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
    let entreprise: &ChampsEntreprise = match F::a_juger(&plate) {
        FicheAJuger::Entreprise(e) => e,
        FicheAJuger::Salarie(_) | FicheAJuger::Spectacle(_) => &ctx.entreprise,
    };
    // Les permanents du jour ne se lisent que pour la fiche qui en a l'usage (`fiches::montres`).
    let dus = || Ok(!engagement::permanents::Permanents::de(ctx, &socle::date::Periode::du_jour(jour))?.dus.is_empty());
    let super::Montres { cases: montres, .. } = super::montres(ctx, F::a_juger(&plate), cle.as_ref().and_then(F::salarie), entreprise, &dus)?;
    let table = champs::fiche(F::NOM)?;
    let historique = historique::<F>(ctx, cle.as_ref(), &table, &jour)?;
    let mut cases = Vec::new();
    for c in table.iter().filter(|c| montres.contains(&c.cle)) {
        let Some(mut case) = case_du_champ(c, plate.valeur_ecrite(c.cle)) else { continue };
        case.anomalie = anomalies.iter().find(|a| a_l_ecran(&table, a.champ) == c.cle).cloned();
        case.temps = historique.iter().find(|h| h.temps.champ == CaseDatee::Case(c.cle)).map(|h| h.temps.clone());
        cases.push(case);
    }
    let (erreurs_titre, erreurs_note) = crate::ouverture::erreurs_dites(anomalies.iter().filter(|a| a.gravite == Gravite::Erreur).count());
    Ok(FicheRendue { phrase, phrases, entete: F::entete(cle.as_ref(), &enregistree), erreurs_titre, erreurs_note, cases, historique })
}

/// LA CASE QUE LE MOTEUR PROPOSE DE REMPLIR (« Pour démarrer » : `routes::rappels`), sur la fiche `nom`, de valeur
/// `valeur` : dessinee comme dans sa fiche. Aucune pour une case d'une structure, une case qui ne se saisit pas, ou une
/// cle que la table ne connait pas.
pub(crate) fn case_a_remplir(nom: NomDeFiche, chemin: &CheminDeFiche, valeur: Option<ValeurDeCase>) -> R<Option<CaseDeLaFiche>> {
    let Some(k) = chemin.seul() else { return Ok(None) };
    Ok(champs::champ(nom, k)?.filter(|c| c.ecran && !c.calcule()).and_then(|c| case_du_champ(&c, valeur)))
}

/// LA CASE A SAISIR DE NOUVEAU (`crate::redire`) : la case `k` de la fiche `nom`, dessinee comme dans sa fiche, vide.
/// Aucune pour une cle que la table ne connait pas, ou pour la ligne de l'histoire.
pub(crate) fn case_a_redire(nom: NomDeFiche, k: CaseDeFiche) -> R<Option<CaseDeLaFiche>> {
    Ok(champs::champ(nom, k)?.and_then(|c| case_du_champ(&c, None)))
}

/// La case que la fiche montre pour la cle `k` : la forme lisible dont elle se deduit, ou elle-meme.
fn a_l_ecran(table: &[Champ], k: CaseDeFiche) -> CaseDeFiche {
    match table.iter().find(|c| c.cle == k) {
        Some(c) => c.a_l_ecran(),
        None => k,
    }
}

/// LA CASE DU CHAMP `c` de valeur `valeur`, sans temps ni faute ; aucune pour la ligne de l'histoire, qui n'est pas une
/// case.
fn case_du_champ(c: &Champ, valeur: Option<ValeurDeCase>) -> Option<CaseDeLaFiche> {
    let forme = forme_de(c.forme)?;
    let place = match c.avance {
        Some(_) => PlaceDeLaCase::Avance,
        None => PlaceDeLaCase::Base,
    };
    Some(CaseDeLaFiche { cle: c.cle, libelle: c.libelle.clone(), forme, place, aide: c.aide.clone(), valeur, choix: choix_du_champ(c), temps: None, anomalie: None })
}

/// LA FORME QU'ON SAISIT D'UNE CASE DE FICHE (`champs::Forme`, ce qu'on en verifie) : un texte (de toute ecriture, un code
/// risque, un pays par son code ISO), un jour, un montant, une valeur d'une liste fermee que la route rend a cote
/// (`choix`), un oui-non, une valeur composee ; l'histoire n'est pas une case.
fn forme_de(f: champs::Forme) -> Option<Forme> {
    use champs::Forme as F;
    match f {
        F::Texte | F::Officiel | F::Affichage | F::Chiffres | F::Email | F::CodeAtmp | F::Pays => Some(Forme::Texte),
        F::Date => Some(Forme::Date),
        F::Montant => Some(Forme::Montant),
        F::Choix => Some(Forme::ChoixRendu),
        F::Booleen => Some(Forme::OuiNon),
        F::Structure => Some(Forme::Objet),
        F::Histoire => None,
    }
}

/// CE QU'OFFRE UNE CASE QUI SE CHOISIT : chacune de ses valeurs (`choix` de `champs-base.json`), dans l'ordre de la table,
/// avec le libelle que la table lui donne (`libelles`), ou telle qu'elle s'ecrit ; la ligne vide en tete quand la table
/// la nomme. Aucun choix pour une autre forme.
fn choix_du_champ(c: &Champ) -> Option<Vec<Choix>> {
    if c.forme != champs::Forme::Choix {
        return None;
    }
    let libelle = |code: &str| match c.libelles.as_ref().and_then(|l| l.de(code)) {
        Some(l) => l.to_string(),
        None => code.to_string(),
    };
    let vide = c.libelles.as_ref().and_then(|l| l.de("")).filter(|_| !c.choix.iter().any(String::is_empty)).map(|l| Choix::de("", l));
    Some(vide.into_iter().chain(c.choix.iter().map(|code| Choix::de(code, libelle(code)))).collect())
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
