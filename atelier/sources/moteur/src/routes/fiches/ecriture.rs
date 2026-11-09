//! UNE FICHE SE SAISIT, SE VERIFIE ET S'ECRIT PAR UN SEUL CHEMIN. La fiche est sa forme a un jour (`droit::fiche` :
//! `ChampsEntreprise`, `ChampsSalarie`, `ChampsSpectacle`) ; ce que l'ecran change, ce sont des cases
//! (`ChangementEntreprise`…), posees sur la fiche telle qu'il l'a montree. Sa saisie (`champs::Saisie`, chaque case dans la
//! forme que sa ligne de `champs-base.json` lui donne) est la lentille par laquelle on la derive (la forme declaree se
//! fabrique de la forme lisible : `deriver`), on la verifie (`champs::verifier_fiche`) et on la compare (ce qui change,
//! ce qui se date) ; ce que la derivation pose revient dans la fiche case par case (`reporter`). Puis chaque case qui
//! change s'ecrit datee dans la fiche telle qu'ecrite (`ecrire_datee`, la seule qui date une valeur de fiche).

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use declarations::champs::{self, Anomalie, Apercu, Gravite, Saisie, Valeur};
use droit::contexte::{Contexte, IdSalarie, IdSpectacle};
use droit::fiche::{ChampsEntreprise, ChampsSalarie, ChampsSpectacle, FicheDatee, FicheEntrepriseDatee, FicheSalarieDatee, FicheSpectacleDatee, Lue as _};
use socle::date::Jour;
use socle::echec::{erreur, Classe, Echec, R};
use socle::forme::NombreLu;

use super::{date_proposee, ecrire, maintenant, plain, refus, DejaDeclare};
use crate::import::completer::A_COMPLETER;
use crate::routes::mois::sauver_avant;

// ------------------------------------------------------------------ les trois fiches

/// UNE SORTE DE FICHE : sa forme datee, son nom (celui de `champs-base.json`), son fichier, et ce qui lui est propre.
pub(crate) trait SorteDeFiche: FicheDatee {
    const NOM: &'static str;
    const FICHIER: &'static str;
    /// Les cases qui s'ecrivent a part (`ecrire_a_part`).
    const A_PART: &'static [&'static str] = &[];
    /// Le fichier qui la porte.
    type Fichier: Clone + Default + PartialEq + Serialize + serde::de::DeserializeOwned;

    /// La fiche `cle` du fichier, si elle existe.
    fn existante<'a>(f: &'a mut Self::Fichier, cle: Option<&str>) -> Option<&'a mut Self>;
    /// La fiche `cle`, neuve si le fichier ne la porte pas encore.
    fn ou_neuve<'a>(f: &'a mut Self::Fichier, cle: Option<&str>) -> R<&'a mut Self>;
    /// Le fichier porte-t-il la fiche `cle` ?
    fn contient(f: &Self::Fichier, cle: &str) -> bool;
    /// La fiche telle que l'ecran l'a montree (`ctx.lue_au`), si elle existe.
    fn montree(ctx: &Contexte, cle: Option<&str>) -> Option<Self::Plate>;
    /// Les cases qui different d'une fiche a l'autre.
    fn differentes(a: &Self::Plate, b: &Self::Plate) -> Vec<&'static str>;
    /// Les cases de la fiche, sans ses notes (ce que l'ecran montre).
    fn sans_notes(p: Self::Plate) -> Self::Plate;

    /// Ce que le moteur pose dans la fiche changee `apres`, d'apres celle que l'ecran a montree (`avant`), avant tout
    /// controle (la forme d'un code risque, la notification d'un taux AT/MP que la saisie change).
    fn ajuster(_avant: &Self::Plate, _apres: &mut Self::Plate) {}
    /// Ce qu'une ecriture refuse, `forcer` ou non (un NIR, un matricule, un numero d'objet deja pris).
    fn refuser(_ctx: &Contexte, _cle: Option<&str>, _avant: &Self::Plate, _apres: &Self::Plate) -> R<()> {
        Ok(())
    }
    /// Ce qu'une fiche neuve recoit du moteur (le matricule suivant).
    fn completer_neuve(_ctx: &Contexte, _plate: &mut Self::Plate) -> R<()> {
        Ok(())
    }
    /// La phrase a confirmer avant d'ecrire, sans date (le jour fixe du versement des intermittents).
    fn usage(_avant: &Self::Plate, _apres: &Self::Plate) -> R<Option<String>> {
        Ok(None)
    }
    /// Ce qui se date a part (`A_PART`), pour la boite « Ce qui change ».
    fn a_dater_a_part(_avant: &Self::Plate, _apres: &Self::Plate) -> Vec<ChangementDit> {
        Vec::new()
    }
    /// Les cases `A_PART`, ecrites dans `brute` ; rend vrai si elles ont change.
    fn ecrire_a_part(_ctx: &Contexte, _brute: &mut Self, _apres: &Self::Plate, _depuis: Option<Jour>, _note: Option<&str>) -> R<bool> {
        Ok(false)
    }
}

impl SorteDeFiche for FicheEntrepriseDatee {
    const NOM: &'static str = "entreprise";
    const FICHIER: &'static str = droit::contexte::ENTREPRISE;
    const A_PART: &'static [&'static str] = &["etablissements"];
    type Fichier = FicheEntrepriseDatee;

    fn existante<'a>(f: &'a mut Self, _cle: Option<&str>) -> Option<&'a mut Self> {
        Some(f)
    }
    fn ou_neuve<'a>(f: &'a mut Self, _cle: Option<&str>) -> R<&'a mut Self> {
        Ok(f)
    }
    fn contient(_f: &Self, _cle: &str) -> bool {
        false
    }
    fn montree(ctx: &Contexte, _cle: Option<&str>) -> Option<ChampsEntreprise> {
        Some(crate::routes::etablissements::entreprise_montree(ctx))
    }
    fn differentes(a: &ChampsEntreprise, b: &ChampsEntreprise) -> Vec<&'static str> {
        a.cases_differentes(b)
    }
    fn sans_notes(p: ChampsEntreprise) -> ChampsEntreprise {
        p.sans_notes()
    }
    /// LE TAUX AT/MP SAISI (`taux_atmp`) : le code risque s'ecrit sans point ni blanc (S21.G00.40.040, « 923AD ») ; une
    /// notification dit le code, le taux et l'annee qu'elle a notifies : la saisie qui en change un ne vient plus d'elle.
    fn ajuster(avant: &ChampsEntreprise, apres: &mut ChampsEntreprise) {
        let Some(t) = apres.taux_atmp.as_mut() else { return };
        t.code_risque = t.code_risque.as_deref().map(|c| c.trim_matches(socle::texte::blanc).replace(['.', ' '], "").to_uppercase()).filter(|c| !c.is_empty());
        if !avant.taux_atmp.as_ref().is_some_and(|a| droit::bareme::meme_taux_atmp(a, t)) {
            t.notification = None;
        }
    }
    /// Le taux AT/MP qui ne se lit pas comme la DSN le declare (`droit::bareme::taux_atmp_refuse`).
    fn refuser(_ctx: &Contexte, _cle: Option<&str>, _avant: &ChampsEntreprise, apres: &ChampsEntreprise) -> R<()> {
        match apres.taux_atmp.as_ref().map(droit::bareme::taux_atmp_refuse).transpose().map_err(refus)? {
            Some(Some(m)) => Err(refus(format!("Accidents du travail (AT/MP) : {} Rien n'a été enregistré.", m))),
            _ => Ok(()),
        }
    }
    /// Le jour fixe du versement des intermittents ; le taux AT/MP recu par la DSN que la saisie change
    /// (`droit::bareme::taux_atmp_a_confirmer`).
    fn usage(avant: &ChampsEntreprise, apres: &ChampsEntreprise) -> R<Option<String>> {
        let versement = apres
            .date_de_versement_intermittents
            .remplie()
            .map(str::trim)
            .filter(|neuf| Some(*neuf) != avant.date_de_versement_intermittents.remplie().map(str::trim))
            .and_then(droit::versement::Versement::a_confirmer_pour_les_intermittents);
        let atmp = droit::bareme::taux_atmp_a_confirmer(avant.taux_atmp.as_ref(), apres.taux_atmp.as_ref()).map_err(refus)?;
        let phrases: Vec<String> = versement.into_iter().chain(atmp).collect();
        Ok((!phrases.is_empty()).then(|| phrases.join(" ")))
    }
    fn a_dater_a_part(avant: &ChampsEntreprise, apres: &ChampsEntreprise) -> Vec<ChangementDit> {
        let liste = |e: &ChampsEntreprise| e.etablissements.iter().flatten().cloned().collect::<Vec<_>>();
        crate::routes::etablissements::a_dater(&liste(avant), &liste(apres)).iter().map(crate::routes::etablissements::Changement::dit).collect()
    }
    fn ecrire_a_part(ctx: &Contexte, brute: &mut Self, apres: &ChampsEntreprise, depuis: Option<Jour>, note: Option<&str>) -> R<bool> {
        let avant = brute.etablissements.clone();
        let liste: Vec<droit::fiche::Etablissement> = apres.etablissements.iter().flatten().cloned().collect();
        crate::routes::etablissements::ecrire(ctx, brute, apres, &liste, depuis, note)?;
        Ok(brute.etablissements != avant)
    }
}

impl SorteDeFiche for FicheSalarieDatee {
    const NOM: &'static str = "salarie";
    const FICHIER: &'static str = droit::contexte::SALARIES;
    type Fichier = droit::fiche::Salaries;

    fn existante<'a>(f: &'a mut droit::fiche::Salaries, cle: Option<&str>) -> Option<&'a mut Self> {
        f.fiches.get_mut(cle?)
    }
    fn ou_neuve<'a>(f: &'a mut droit::fiche::Salaries, cle: Option<&str>) -> R<&'a mut Self> {
        let cle = cle.ok_or_else(|| refus("Une fiche de salarié se nomme par sa clé."))?;
        Ok(f.fiches.entry(IdSalarie::lire(cle).map_err(plain)?).or_default())
    }
    fn contient(f: &droit::fiche::Salaries, cle: &str) -> bool {
        f.fiches.contains_key(cle)
    }
    fn montree(ctx: &Contexte, cle: Option<&str>) -> Option<ChampsSalarie> {
        ctx.salaries.get(cle?).map(|f| f.champs.clone())
    }
    fn differentes(a: &ChampsSalarie, b: &ChampsSalarie) -> Vec<&'static str> {
        a.cases_differentes(b)
    }
    fn sans_notes(p: ChampsSalarie) -> ChampsSalarie {
        p.sans_notes()
    }
    fn refuser(ctx: &Contexte, cle: Option<&str>, avant: &ChampsSalarie, apres: &ChampsSalarie) -> R<()> {
        // LE MATRICULE D'UN SALARIE SANS NIR NE SE CHANGE PAS : il finit son numero technique temporaire.
        if let (Some(_), Some(a), Some(b)) = (cle, avant.code_salarie.remplie(), apres.code_salarie.remplie()) {
            if a != b && apres.nir.portee().is_none() && avant.nir.portee().is_none() {
                return Err(refus(format!(
                    "Le matricule de cette fiche passerait de {} a {}, et le salarie n'a pas de NIR : il est declare sous un numero \
                     technique temporaire qui FINIT par son matricule. Le changer ferait apparaitre un deuxieme individu dans la DSN, \
                     que rien ne rattacherait au premier. Saisir d'abord le NIR notifie par la caisse.",
                    a, b
                )));
            }
        }
        // UN NIR, UNE PERSONNE : `forcer` ne leve pas ce refus.
        if let Some(nir) = apres.nir.portee().map(str::trim).filter(|n| !n.is_empty()) {
            let autres: Vec<&str> =
                ctx.salaries.iter().filter(|(k, s)| Some(k.as_str()) != cle && s.nir.portee().is_some_and(|n| n.trim() == nir)).map(|(k, _)| k.as_str()).collect();
            if !autres.is_empty() {
                return Err(refus(format!(
                    "Le NIR {} est déjà celui de la fiche {}. Un NIR désigne une personne et une seule : complétez ou réactivez cette \
                     fiche-là plutôt que d'en créer une seconde.",
                    nir,
                    autres.join(", ")
                )));
            }
        }
        // UN MATRICULE, UN SALARIE : la DSN rapproche chaque salarie de ses declarations par son matricule (S21.G00.30.019,
        // dont « le choix de la valeur revient a l'employeur », `norme-dsn/texte/bloc-S21.G00.30.txt`, l. 327-329) ; deux
        // fiches au meme matricule s'y confondraient. `forcer` ne leve pas ce refus.
        if let Some(matricule) = apres.code_salarie.remplie().map(str::trim).filter(|m| !m.is_empty()) {
            let porteur = ctx.salaries.iter().find(|(k, s)| Some(k.as_str()) != cle && s.code_salarie.portee().is_some_and(|m| m.trim() == matricule));
            if let Some((k, s)) = porteur {
                let qui = match s.nomme() {
                    Some(n) => n,
                    None => k.to_string(),
                };
                return Err(refus(format!(
                    "Le matricule {} est déjà celui de {} (fiche {}). Un matricule désigne un salarié et un seul : la DSN \
rapproche chacun de ses déclarations par lui. En choisir un autre, ou laisser la case vide pour prendre le suivant.",
                    matricule, qui, k
                )));
            }
        }
        Ok(())
    }
    fn completer_neuve(ctx: &Contexte, plate: &mut ChampsSalarie) -> R<()> {
        // Le matricule d'une fiche neuve : le suivant. Un matricule saisi est garde tel quel. L'ANCIENNETE PART DE
        // L'EMBAUCHE, pas du jour de la fiche : la validation du premier engagement la pose (`poser_anciennete_a_l_embauche`).
        if plate.code_salarie.remplie().map(str::trim).is_none_or(str::is_empty) {
            plate.code_salarie = Some(matricule_suivant(&lire_fichier::<FicheSalarieDatee>(ctx)?)?.to_string());
        }
        Ok(())
    }
}

impl SorteDeFiche for FicheSpectacleDatee {
    const NOM: &'static str = "spectacle";
    const FICHIER: &'static str = droit::contexte::SPECTACLES;
    type Fichier = droit::fiche::Spectacles;

    fn existante<'a>(f: &'a mut droit::fiche::Spectacles, cle: Option<&str>) -> Option<&'a mut Self> {
        f.fiches.get_mut(cle?)
    }
    fn ou_neuve<'a>(f: &'a mut droit::fiche::Spectacles, cle: Option<&str>) -> R<&'a mut Self> {
        let cle = cle.ok_or_else(|| refus("Une fiche de spectacle se nomme par sa clé."))?;
        Ok(f.fiches.entry(IdSpectacle::lire(cle).map_err(plain)?).or_default())
    }
    fn contient(f: &droit::fiche::Spectacles, cle: &str) -> bool {
        f.fiches.contains_key(cle)
    }
    fn montree(ctx: &Contexte, cle: Option<&str>) -> Option<ChampsSpectacle> {
        ctx.spectacles.get(cle?).cloned()
    }
    fn differentes(a: &ChampsSpectacle, b: &ChampsSpectacle) -> Vec<&'static str> {
        a.cases_differentes(b)
    }
    fn sans_notes(p: ChampsSpectacle) -> ChampsSpectacle {
        p.sans_notes()
    }
    fn refuser(ctx: &Contexte, cle: Option<&str>, _avant: &ChampsSpectacle, apres: &ChampsSpectacle) -> R<()> {
        // UN NUMERO D'OBJET, UN SPECTACLE : France Travail l'attribue « pour chaque spectacle ou chaque production » et il
        // « permet d'identifier toutes les personnes ayant travaille » dessus (S21.G00.40.050,
        // `norme-dsn/texte/bloc-S21.G00.40.txt`, l. 1379-1385) ; deux fiches au meme numero se confondraient dans la DSN.
        // `forcer` ne leve pas ce refus.
        let Some(numero) = apres.numero_objet.remplie().map(str::trim).filter(|n| !n.is_empty()) else { return Ok(()) };
        let porteur = ctx.spectacles.iter().find(|(k, s)| Some(k.as_str()) != cle && s.numero_objet.portee().is_some_and(|n| n.trim().eq_ignore_ascii_case(numero)));
        if let Some((k, s)) = porteur {
            let qui = match s.titre.remplie().filter(|t| !t.trim().is_empty() && !t.trim().eq_ignore_ascii_case(A_COMPLETER)) {
                Some(t) => format!("du spectacle « {} » (fiche {})", t, k),
                None => format!("de la fiche {}", k),
            };
            return Err(refus(format!(
                "Le numéro d'objet {} est déjà celui {}. Un numéro d'objet désigne un spectacle et un seul : complétez \
cette fiche-là, ou corrigez le numéro.",
                numero, qui
            )));
        }
        Ok(())
    }
}

/// Le fichier de la fiche `F` de la compagnie, lu par sa forme ; absent, il est vide (une compagnie sans fiche encore).
pub(crate) fn lire_fichier<F: SorteDeFiche>(ctx: &Contexte) -> R<F::Fichier> {
    let chemin = ctx.racine.join(F::FICHIER);
    if !chemin.exists() {
        return Ok(F::Fichier::default());
    }
    droit::fiche::lire_fichier(&chemin, F::FICHIER).map_err(plain)
}

/// Le fichier ecrit, l'ancien sauvegarde.
fn ecrire_fichier<F: SorteDeFiche>(ctx: &Contexte, fichier: &F::Fichier) -> R<()> {
    let chemin = ctx.racine.join(F::FICHIER);
    sauver_avant(&chemin)?;
    ecrire(&chemin, fichier)
}

/// Le prochain matricule (S21.G00.30.019, que l'employeur choisit : `norme-dsn/texte-2027/bloc-S21.G00.30.txt`) : le
/// plus grand code salarie numerique que le fichier ait jamais porte, plus un. Chaque valeur datee compte.
fn matricule_suivant(salaries: &droit::fiche::Salaries) -> R<i64> {
    let codes = salaries.fiches.values().flat_map(|s| s.code_salarie.iter().flatten()).filter_map(|e| e.valeur.portee().map(str::trim));
    let numeriques: Vec<i64> = codes
        .filter(|c| !c.is_empty() && c.chars().all(|x| x.is_ascii_digit()))
        .map(|c| c.parse::<i64>().map_err(|e| refus(format!("Matricule illisible : {} ({}).", c, e))))
        .collect::<R<_>>()?;
    Ok(numeriques.into_iter().fold(0, i64::max) + 1)
}

// ------------------------------------------------------------------ la saisie

/// La saisie de la fiche `F` qu'est `plate` (`champs::Saisie::de_la_fiche`).
pub(crate) fn vue<F: SorteDeFiche>(plate: &F::Plate) -> R<Saisie> {
    Saisie::de_la_fiche(F::NOM, plate)
}

/// Une case telle qu'un changement l'ecrit, lue par la forme de la fiche (`droit::fiche::lire_saisie`).
#[derive(Serialize)]
struct CaseEcrite<'a> {
    case: &'a str,
    valeur: Option<&'a Valeur>,
}

/// CE QUE LA SAISIE `apres` CHANGE DE `avant` REVIENT DANS LA FICHE `plate`, case par case, chacune lue par la forme de la
/// fiche : une valeur qu'elle ne lit pas se refuse en se nommant.
pub(crate) fn reporter<F: SorteDeFiche>(plate: &mut F::Plate, avant: &Saisie, apres: &Saisie) -> R<()> {
    let cles: Vec<&str> = apres.cases().iter().chain(avant.cases()).map(|(k, _)| k.as_str()).collect();
    for k in cles {
        if avant.case(k) != apres.case(k) {
            let ch: F::Changement = droit::fiche::lire_saisie(&CaseEcrite { case: k, valeur: apres.case(k) }, F::FICHIER).map_err(refus)?;
            F::appliquer(plate, ch);
        }
    }
    Ok(())
}

/// LES SAISIES QUE L'ECRAN ENVOIE, chacune lue par la table de sa fiche (`champs::lecture`) : une case qui n'a pas sa
/// forme se refuse avec son chemin ; une case inconnue se garde pour etre dite (`champs::cases_inconnues`).
macro_rules! saisie_de {
    ($($t:ident = $nom:literal),*) => {
        $(
            pub(crate) struct $t(Saisie);
            impl<'de> Deserialize<'de> for $t {
                fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<$t, D::Error> {
                    use serde::de::{DeserializeSeed, Error};
                    champs::lecture($nom).map_err(|e| D::Error::custom(e.message))?.deserialize(d).map($t)
                }
            }
        )*
    };
}
saisie_de!(SaisieEntreprise = "entreprise", SaisieSalarie = "salarie", SaisieSpectacle = "spectacle");

/// UNE FICHE EN COURS DE SAISIE (`verifier-fiche`, `permanents`) : `{"nom": …, "donnees": {…}}`.
#[derive(Deserialize)]
#[serde(tag = "nom", content = "donnees", deny_unknown_fields)]
pub(crate) enum SaisieDeFiche {
    #[serde(rename = "entreprise")]
    Entreprise(SaisieEntreprise),
    #[serde(rename = "salarie")]
    Salarie(SaisieSalarie),
    #[serde(rename = "spectacle")]
    Spectacle(SaisieSpectacle),
}

impl SaisieDeFiche {
    /// Le nom de la fiche, et sa saisie.
    pub(crate) fn ouverte(self) -> (&'static str, Saisie) {
        match self {
            SaisieDeFiche::Entreprise(s) => ("entreprise", s.0),
            SaisieDeFiche::Salarie(s) => ("salarie", s.0),
            SaisieDeFiche::Spectacle(s) => ("spectacle", s.0),
        }
    }
}

// ------------------------------------------------------------------ la derivation

/// Le texte dit de la case `cle`, sans ses blancs autour.
fn texte<'a>(s: &'a Saisie, cle: &str) -> Option<&'a str> {
    s.texte(cle).map(str::trim).filter(|t| !t.is_empty())
}

/// Le texte d'une case qui en a un : un texte, ou un nombre tel qu'il s'ecrit.
fn mot_de(v: Option<&Valeur>) -> Option<String> {
    v.and_then(Valeur::mot)
}

/// Sans accents, en capitales, espaces resserrees.
fn capitales(t: &str) -> String {
    let t: String = champs::sans_accents(t).chars().flat_map(char::to_uppercase).collect();
    t.split(socle::texte::blanc).filter(|m| !m.is_empty()).collect::<Vec<_>>().join(" ")
}

/// Un nombre saisi (`12,5`, `1.5 %`), ecrit en nombre ; vide, rien.
fn nombre_saisi(v: Option<&Valeur>, quoi: &str) -> R<Option<NombreLu>> {
    let Some(brut) = mot_de(v) else { return Ok(None) };
    let brut = brut.replace(',', ".").replace('%', "");
    let brut = brut.trim();
    if brut.is_empty() {
        return Ok(None);
    }
    let d = socle::num::decimal_lu(brut).ok_or_else(|| erreur(Classe::Refuse, format!("{} illisible : {}.", quoi, brut)))?;
    NombreLu::de_f64(socle::num::flottant(d)).map(Some).ok_or_else(|| erreur(Classe::Refuse, format!("{} illisible : {}.", quoi, brut)))
}

/// La forme declaree qui ne respecte pas sa norme (`champs::faute_de_norme`) se refait ; ce qui n'est pas un texte se
/// refuse.
fn fautif(c: &champs::Champ, actuel: &Valeur) -> R<bool> {
    match actuel {
        Valeur::Texte(s) => Ok(champs::faute_de_norme(c, s)?.is_some()),
        _ => Err(erreur(Classe::MalFormee, socle::fichiers::attendu("un texte", "une autre valeur"))),
    }
}

/// FABRIQUE LES FORMES DECLAREES DEPUIS LES FORMES LISIBLES, dans la saisie `d` de la fiche `nom`, de ce que les cases
/// `source` disent (a l'ecriture : celles que l'ecran change ; a la verification : toutes). Sans `forcer`, seul ce qui
/// est vide ou ne respecte pas sa norme se remplit. Le departement de naissance et les initiales du contrat d'un salarie
/// ne s'ecrivent que dans une case vide : ils se remplissent quelles que soient les cases `source`.
pub(crate) fn deriver(nom: &str, d: &mut Saisie, forcer: bool, source: &dyn Fn(&str) -> bool) -> R<()> {
    for c in champs::fiche(nom)? {
        let Some(src) = c.derive_de.as_deref().filter(|s| !s.is_empty() && source(s)) else { continue };
        let Some(Valeur::Texte(lisible)) = d.case(src) else { continue };
        if lisible.is_empty() {
            continue;
        }
        let attendu = champs::normaliser(&c, lisible)?;
        match d.case(&c.cle) {
            Some(Valeur::Texte(t)) if *t == attendu => continue,
            Some(a) if a.dite() && !forcer && !fautif(&c, a)? => continue,
            _ => {}
        }
        d.poser(&c.cle, Valeur::Texte(attendu));
    }
    if nom == "entreprise" {
        // Le numero d'adherent Conges Spectacles s'ecrit dans sa forme (`conges_spectacles::forme_adherent`) : « 0123
        // 450001-f » est la meme valeur que « 0123450001F ».
        let cle = documents::conges_spectacles::CLE_ADHERENT;
        if let (true, Some(brut)) = (source(cle), d.texte(cle)) {
            let forme = documents::conges_spectacles::forme_adherent(brut);
            if forme != brut {
                d.poser(cle, Valeur::Texte(forme));
            }
        }
        deriver_personnes(d, forcer, source);
        deriver_siren_nic(d, source);
        if source("etablissements") {
            deriver_etablissements(d)?;
        }
    }
    // LE COMPTE BANCAIRE S'ECRIT COMPACT : l'IBAN et le BIC sans blancs, en majuscules -- « FR76 3000 6000 … » est le
    // meme compte que « FR763000 6000… » (`socle::iban::iban_compact`, communique IBAN de la Banque centrale de Turquie,
    // annexe 3 : `reference/sources/iban/`).
    if source("banque") {
        if let Some(Valeur::Bloc(b)) = d.case_mut("banque") {
            for k in ["iban", "bic"] {
                if let Some(brut) = b.texte(k) {
                    let forme = socle::iban::iban_compact(brut);
                    if forme != brut {
                        b.poser(k, Valeur::Texte(forme));
                    }
                }
            }
        }
    }
    if nom == "salarie" {
        // LE DEPARTEMENT DE NAISSANCE SE LIT DANS LE NIR (regle 2 : la page le montre, le moteur l'ecrit) : ses 6e et 7e
        // caracteres sont le departement de naissance, 99 pour une personne nee a l'etranger (S21.G00.30.001 et .014,
        // `atelier/reference/norme-dsn/texte-2027/bloc-S21.G00.30.txt`). Il ne s'ecrit que dans une case vide : une
        // valeur saisie qui le contredit se dit (`champs::controles_croises`), elle ne se remplace pas.
        if texte(d, "departement_naissance").is_none() {
            if let Some(dep) = texte(d, "nir").and_then(champs::departement_du_nir) {
                d.poser("departement_naissance", Valeur::Texte(dep));
            }
        }
        // Les initiales du contrat se lisent dans le nom declare, lui-meme derive du nom saisi.
        if texte(d, "initiales_contrat").is_none() {
            let lettres: String = texte(d, "nom").into_iter().flat_map(str::chars).filter(|c| c.is_ascii_uppercase()).take(2).collect();
            if !lettres.is_empty() {
                d.poser("initiales_contrat", Valeur::Texte(lettres));
            }
        }
        // LA NATIONALITE DSN SE DEDUIT DU PAYS : avec un pays de nationalite de la liste, la case 30.013 ne se choisit
        // plus, le moteur l'ecrit (`pays::codification`). Sans pays, ou avec un pays inconnu (qui refuse sur sa case), la
        // case reste ce qu'elle est.
        if let (true, Some(code)) = (source("pays_nationalite"), texte(d, "pays_nationalite")) {
            if let Some(p) = socle::pays::du_code(code).map_err(Echec::from)? {
                let attendu = socle::pays::codification(&p).to_string();
                if d.texte("code_nationalite_dsn") != Some(attendu.as_str()) {
                    d.poser("code_nationalite_dsn", Valeur::Texte(attendu));
                }
            }
        }
    }
    Ok(())
}

/// Les formes declarees du signataire et des contacts. LE CONTACT DSN ET CELUI DE LA MEDECINE DU TRAVAIL INTERNE : la
/// norme les ecrit en « nom et prenom » en une seule rubrique (S20.G00.07.001, `norme-dsn/texte/bloc-S20.G00.07.txt`
/// l. 63-71) ; la fiche les saisit en une case (`nom_prenom_affichage`), dont la forme declaree est la meme en capitales
/// sans accents.
fn deriver_personnes(d: &mut Saisie, forcer: bool, source: &dyn Fn(&str) -> bool) {
    let personnes: [(&str, &[&str]); 3] = [("signataire", &["nom", "prenom"]), ("declarant_dsn", &["nom_prenom"]), ("contact_sante_travail", &["nom_prenom"])];
    for (cle, officiels) in personnes {
        if !source(cle) {
            continue;
        }
        let Some(Valeur::Bloc(p)) = d.case_mut(cle) else { continue };
        for k in officiels {
            let Some(lisible) = texte(p, &format!("{}_affichage", k)).map(capitales) else { continue };
            if forcer || p.dite(k).is_none() {
                p.poser(k, Valeur::Texte(lisible));
            }
        }
    }
}

/// LE SIREN ET LE NIC SE DEDUISENT DU SIRET (regle 2 : la page ne les pose pas) : le SIRET est le SIREN (9 chiffres) suivi
/// du NIC (5 chiffres) -- c'est sa definition (INSEE), aucune valeur ne se devine. Un SIRET qui n'a pas 14 chiffres ne
/// deduit rien : sa case le dit (`champs::verifier_fiche`).
fn deriver_siren_nic(d: &mut Saisie, source: &dyn Fn(&str) -> bool) {
    let Some(siret) = texte(d, "siret").filter(|s| source("siret") && s.chars().count() == 14 && s.chars().all(|c| c.is_ascii_digit())).map(str::to_string) else { return };
    for (cle, valeur) in [("siren", &siret[..9]), ("nic", &siret[9..])] {
        if d.texte(cle) != Some(valeur) {
            d.poser(cle, Valeur::Texte(valeur.to_string()));
        }
    }
}

/// LES AUTRES ETABLISSEMENTS S'ECRIVENT COMME LE SIEGE : le NIC se lit dans le SIRET (ses cinq derniers chiffres),
/// l'adresse et la commune declarees se fabriquent depuis leur forme lisible comme celles du siege, le code risque
/// s'ecrit sans point ni blanc (S21.G00.40.040, « 923AD »), le taux saisi en nombre ; le versement mobilite de sa commune
/// est un taux (zero : aucun) et le code INSEE de la commune que la DSN declare.
fn deriver_etablissements(d: &mut Saisie) -> R<()> {
    let Some(Valeur::Lignes(lignes)) = d.case("etablissements").cloned() else { return Ok(()) };
    let fiche = champs::fiche("entreprise")?;
    let mut sortie = Vec::new();
    for mut o in lignes {
        if let Some(siret) = o.texte("siret").map(|s| s.chars().filter(|c| !c.is_whitespace()).collect::<String>()) {
            if siret.chars().count() == 14 && siret.chars().all(|c| c.is_ascii_digit()) {
                o.poser("nic", Valeur::Texte(siret[9..].to_string()));
            }
            o.poser("siret", Valeur::Texte(siret));
        }
        for (declare, lisible) in [("adresse", "adresse_affichage"), ("commune", "commune_affichage")] {
            let Some(t) = texte(&o, lisible).map(str::to_string) else { continue };
            if let Some(c) = fiche.iter().find(|c| c.cle == declare) {
                o.poser(declare, Valeur::Texte(champs::normaliser(c, &t)?));
            }
        }
        if let Some(Valeur::Bloc(t)) = o.case_mut("taux_atmp") {
            match texte(t, "code_risque").map(|c| c.trim_matches(socle::texte::blanc).replace(['.', ' '], "").to_uppercase()).filter(|c| !c.is_empty()) {
                Some(code) => t.poser("code_risque", Valeur::Texte(code)),
                None => t.retirer("code_risque"),
            }
            match nombre_saisi(t.case("valeur"), "Taux AT/MP")? {
                Some(n) => t.poser("valeur", Valeur::Nombre(n)),
                None => t.retirer("valeur"),
            }
        }
        if let Some(Valeur::Bloc(vm)) = o.case_mut("versement_mobilite") {
            match nombre_saisi(vm.case("taux"), "Taux du versement mobilité")? {
                Some(n) => vm.poser("taux", Valeur::Nombre(n)),
                None => vm.retirer("taux"),
            }
            match texte(vm, "code_insee").map(str::to_uppercase) {
                Some(insee) => vm.poser("code_insee", Valeur::Texte(insee)),
                None => vm.retirer("code_insee"),
            }
        }
        if sans_valeur(o.case("versement_mobilite")) {
            o.retirer("versement_mobilite");
        }
        sortie.push(o);
    }
    d.poser("etablissements", Valeur::Lignes(sortie));
    Ok(())
}

// ------------------------------------------------------------------ ce qui change

/// Une case vide : absente, un texte blanc ou « A COMPLETER » (le titre d'un spectacle importe sans nom), une liste ou
/// une structure vides. La remplir n'est pas un changement a dater.
pub(crate) fn vide(v: Option<&Valeur>) -> bool {
    v.is_none_or(|v| match v {
        Valeur::Texte(t) => t.trim().is_empty() || t.trim().eq_ignore_ascii_case(A_COMPLETER),
        Valeur::Mots(l) => l.is_empty(),
        Valeur::Lignes(l) => l.is_empty(),
        Valeur::Bloc(b) => !b.est_dite(),
        Valeur::Nombre(_) | Valeur::OuiNon(_) => false,
    })
}

/// Sans valeur : une case vide (`vide`), une case a cocher non cochee, une structure dont aucune case n'a de valeur. La
/// fiche les montre vides ; un enregistrement qui les laisse ainsi ne change rien.
pub(crate) fn sans_valeur(v: Option<&Valeur>) -> bool {
    vide(v)
        || match v {
            Some(Valeur::OuiNon(b)) => !b,
            Some(Valeur::Bloc(b)) => b.cases().iter().all(|(_, x)| sans_valeur(Some(x))),
            _ => false,
        }
}

/// Le nombre que dit une valeur simple (un nombre, ou un texte qui en est un).
fn nombre_de(v: &Valeur) -> Option<f64> {
    match v {
        Valeur::Nombre(n) => Some(n.en_f64()),
        Valeur::Texte(t) => t.trim().parse::<f64>().into_iter().next(),
        _ => None,
    }
}

/// LA MEME VALEUR, a la forme pres : un nombre saisi en texte (100 et « 100 »), une structure sans ses formes lisibles
/// (les cases `_affichage` que l'ecran comble ne se declarent pas) ni ses cases laissees vides.
fn meme(a: &Valeur, b: &Valeur) -> bool {
    if a == b {
        return true;
    }
    match (a, b) {
        (Valeur::Bloc(x), Valeur::Bloc(y)) => {
            let dites = |s: &Saisie| -> Vec<(String, Valeur)> { s.cases().iter().filter(|(k, v)| !k.ends_with("_affichage") && !sans_valeur(Some(v))).cloned().collect() };
            let (x, y) = (dites(x), dites(y));
            x.len() == y.len() && x.iter().all(|(k, v)| y.iter().any(|(l, w)| l == k && meme(v, w)))
        }
        (Valeur::Bloc(_) | Valeur::Lignes(_) | Valeur::Mots(_), _) | (_, Valeur::Bloc(_) | Valeur::Lignes(_) | Valeur::Mots(_)) => false,
        _ => match (nombre_de(a), nombre_de(b)) {
            (Some(x), Some(y)) => x == y,
            _ => a.mot().is_some() && a.mot() == b.mot(),
        },
    }
}

fn meme_case(a: Option<&Valeur>, b: Option<&Valeur>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => meme(a, b),
        (a, b) => a.is_none() && b.is_none(),
    }
}

/// La structure `apres` garde chaque case de `avant` qui a une valeur (elle n'en remplit que des vides).
fn seulement_completee(avant: &Valeur, apres: &Valeur) -> bool {
    match (avant, apres) {
        (Valeur::Bloc(a), Valeur::Bloc(b)) => a.cases().iter().filter(|(_, x)| !sans_valeur(Some(x))).all(|(k, x)| b.case(k).is_some_and(|y| meme(x, y))),
        _ => false,
    }
}

/// La case avait une valeur : la changer est un changement, a dater. Une case a cocher que la fiche dit a la sienne,
/// cochee ou non.
fn avait_une_valeur(avant: Option<&Valeur>) -> bool {
    matches!(avant, Some(Valeur::OuiNon(_))) || !sans_valeur(avant)
}

/// LES FAITS QUI NE SE DATENT PAS : une case qui dit un fait du passe, vrai de tout temps, et non une valeur qui vaut a
/// partir d'un jour. La date de la premiere embauche de la compagnie (CSS L. 130-1, II, l'effectif d'une entreprise de
/// moins de cinq ans) : la changer la corrige, elle ne change pas a une date. De meme l'anciennete d'un salarie, le jour
/// ou il est entre dans l'entreprise (« Dans la compagnie depuis », d'ou se compte S21.G00.86.003 a la fin de chaque mois
/// declare : `norme-dsn/texte-2027/bloc-S21.G00.86.txt`, l. 91-95). Elles s'ecrivent sans date d'effet.
const FAITS_SANS_DATE: [(&str, &str); 2] = [("entreprise", "premiere_embauche"), ("salarie", "date_anciennete")];

/// LES CASES `cles` QUI CHANGENT LA FICHE `avant` (telle que l'ecran l'a montree) en `apres` ET DEMANDENT LEUR DATE
/// D'EFFET (modele, « Le temps » : toute la configuration est datee, et une valeur sans date rend le passe faux des
/// qu'elle change) : une valeur deja dite qui change, toujours ; une case que la fiche ne dit pas est vide. UNE CASE VIDE
/// QU'ON REMPLIT NE SE DATE PAS : elle comble un manque, sa premiere valeur vaut de tout temps ; ce qui est parti ne se
/// refait pas (regle 4), seul ce qui reste a faire la lit. Sauf `premiere_datee` : une valeur qui, par nature, ne vaut
/// qu'a partir d'un jour (le taux de PAS recu, « le taux personnalise ne s'applique que pour l'avenir »,
/// BOI-IR-PAS-30-10-50 § 20, `bofip/BOI-IR-PAS-30-10-50-20190227.html`). Une fiche neuve n'a rien a dater.
fn changements_a_dater(nom: &str, avant: &Saisie, apres: &Saisie, cles: &[&'static str], premiere_datee: bool) -> R<Vec<&'static str>> {
    if avant.cases().is_empty() {
        return Ok(Vec::new());
    }
    // Une case que la fiche ne dit pas se montre a ce que le moteur lit alors : une case a cocher a son defaut
    // (`booleen_defaut`), les jours travailles du lundi au vendredi (`droit::conges::JoursTravailles`). La laisser ainsi ne
    // change rien.
    let mut defauts: HashMap<String, Valeur> =
        champs::fiche(nom)?.into_iter().filter(|c| c.forme == champs::Forme::Booleen).map(|c| (c.cle, Valeur::OuiNon(c.booleen_defaut))).collect();
    defauts.insert("jours_travailles".into(), Valeur::Mots(droit::conges::JOURS_DE_LA_SEMAINE[..5].iter().map(|j| j.to_string()).collect()));
    Ok(cles
        .iter()
        .copied()
        .filter(|k| {
            let dite = avant.case(k).filter(|a| !vide(Some(a)));
            let a = dite.or(defauts.get(*k));
            let b = apres.case(k);
            !FAITS_SANS_DATE.contains(&(nom, k))
                && !(a.is_some() && b.is_some() && meme_case(a, b))
                && !(sans_valeur(a) && sans_valeur(b))
                && (premiere_datee || avait_une_valeur(dite))
                // Une structure dont on remplit des cases vides sans changer celles qui ont une valeur : c'est leur
                // premiere valeur, comme une case vide.
                && (premiere_datee || !matches!((a, b), (Some(a), Some(b)) if seulement_completee(a, b)))
        })
        .collect())
}

/// ECRIRE UNE FICHE DATEE, la seule fonction qui date une valeur d'une fiche : chaque case qui change de `avant` (la
/// fiche a `lue_au`, celle que l'ecran a montree) a `apres` s'ecrit dans la fiche datee `brute` (`FicheDatee::changer`,
/// `droit::fiche::dater`). Ce qui change une valeur existante vaut a partir de `depuis`, et sans lui se refuse : ce qui
/// precede garde l'ancienne valeur (`changements_a_dater`). Une case vide qu'on remplit prend sa valeur de tout temps,
/// `depuis` ou non ; une valeur `premiere_datee` (le taux de PAS recu) vaut a partir de `depuis` meme dans une case vide :
/// avant lui, la fiche ne disait rien. Les notes s'ecrivent telles quelles. Les cases `A_PART` s'ecrivent a part. Rend
/// les cases ecrites.
pub(crate) fn ecrire_datee<F: SorteDeFiche>(
    brute: &mut F,
    avant: &F::Plate,
    apres: &F::Plate,
    premiere_datee: bool,
    depuis: Option<Jour>,
    note: Option<&str>,
    lue_au: &Jour,
) -> R<Vec<&'static str>> {
    let (vue_avant, vue_apres) = (vue::<F>(avant)?, vue::<F>(apres)?);
    let cles: Vec<&'static str> = F::differentes(avant, apres).into_iter().filter(|k| !F::A_PART.contains(k)).collect();
    let a_dater = changements_a_dater(F::NOM, &vue_avant, &vue_apres, &cles, premiere_datee)?;
    if depuis.is_none() && !a_dater.is_empty() {
        return Err(refus(format!(
            "Changer {} demande sa date d'effet (« a partir du ») : ce qui precede garde l'ancienne valeur, et la date ne \
             se devine pas. Rien n'a ete enregistre.",
            libelles_a_dater(F::NOM, &a_dater)?.join(", ")
        )));
    }
    let mut ecritures: Vec<(F::Changement, Option<Jour>, Option<&str>)> = Vec::new();
    let mut ecrites = Vec::new();
    for k in cles {
        let (a, b) = (vue_avant.case(k), vue_apres.case(k));
        let rien = b.is_none_or(|v| matches!(v, Valeur::Texte(t) if t.is_empty()) || matches!(v, Valeur::Bloc(_) if sans_valeur(Some(v))));
        if (vide(a) && rien) || (a.is_some() && b.is_some() && meme_case(a, b)) {
            continue;
        }
        let jours = brute.depuis(k);
        let datee = depuis.is_some() && a_dater.contains(&k);
        let jour = if datee { depuis } else { jours.iter().copied().filter(|j| j.is_none_or(|j| j <= *lue_au)).last().flatten() };
        let case = |p: &F::Plate| F::changement(p, k).ok_or_else(|| plain(format!("La case {} n'est pas de la fiche {}.", k, F::NOM)));
        if datee && jours.is_empty() {
            // Avant `depuis`, la fiche ne disait rien : la case, vide.
            ecritures.push((case(&F::Plate::default())?, None, None));
        }
        ecritures.push((case(apres)?, jour, note));
        ecrites.push(k);
    }
    for (ch, jour, note) in ecritures {
        droit::fiche::FicheDatee::changer(brute, ch, jour.as_ref(), note);
    }
    brute.noter(apres);
    Ok(ecrites)
}

/// Les libelles des cases a dater, sans un champ calcule qui change avec la case d'ou il derive (le meme fait).
fn libelles_a_dater(nom: &str, a_dater: &[&str]) -> R<Vec<String>> {
    let fiche = champs::fiche(nom)?;
    let derive_d_un_change = |k: &str| fiche.iter().find(|c| c.cle == k).and_then(|c| c.derive_de.as_deref()).is_some_and(|s| a_dater.contains(&s));
    Ok(a_dater
        .iter()
        .filter(|k| !derive_d_un_change(k))
        .map(|k| match fiche.iter().find(|c| c.cle == *k) {
            Some(c) => format!("« {} »", c.libelle),
            None => format!("« {} »", k),
        })
        .collect())
}

// ------------------------------------------------------------------ poser des cases

/// POSER DES CASES D'UNE FICHE QUI EXISTE (un geste qui ne touche que quelques cases : le modele de bulletin, la case des
/// permanents, le compte des DPAE, l'anciennete d'un salarie, une fiche archivee, ce que l'import complete) : face a la
/// fiche telle qu'elle est sur le disque a `lue_au` (`ecrire_datee`), l'ancien fichier sauvegarde. Ni derivation ni
/// verification : chaque case est deja dans sa forme. `cle` : celle d'un salarie ou d'un spectacle, `None` pour
/// l'entreprise.
pub(crate) fn poser<F: SorteDeFiche>(ctx: &Contexte, cle: Option<&str>, changements: Vec<F::Changement>, depuis: Option<Jour>, note: Option<&str>) -> R<()> {
    poser_au::<F>(ctx, cle, changements, false, depuis, note, &ctx.lue_au)
}

/// `poser`, face a la fiche lue a `lue_au` ; `premiere_datee` comme `ecrire_datee`.
pub(crate) fn poser_au<F: SorteDeFiche>(ctx: &Contexte, cle: Option<&str>, changements: Vec<F::Changement>, premiere_datee: bool, depuis: Option<Jour>, note: Option<&str>, lue_au: &Jour) -> R<()> {
    let mut contenu = lire_fichier::<F>(ctx)?;
    let Some(brute) = F::existante(&mut contenu, cle) else {
        return Err(refus(format!("La fiche {} {} n'existe pas : rien n'est écrit.", F::NOM, cle.into_iter().collect::<String>())));
    };
    let avant = brute.au(lue_au);
    let mut apres = avant.clone();
    for ch in changements {
        F::appliquer(&mut apres, ch);
    }
    ecrire_datee::<F>(brute, &avant, &apres, premiere_datee, depuis, note, lue_au)?;
    ecrire_fichier::<F>(ctx, &contenu)
}

// ------------------------------------------------------------------ enregistrer une fiche

/// CE QUE L'ENREGISTREMENT D'UNE FICHE PASSE OUTRE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Forcer {
    /// Rien : une erreur refuse la fiche.
    Rien,
    /// Ce que l'ecran peut forcer (« Enregistrer quand meme ») : une erreur de forme (`forcable`), jamais ce que le type
    /// d'une case refuse.
    Forme,
    /// Tout : l'import et le questionnaire du salarie posent une fiche a completer, qui le dit (« A COMPLETER »).
    Tout,
}

impl Forcer {
    /// Ces anomalies refusent-elles la fiche ?
    fn refuse(self, anomalies: &[Anomalie]) -> bool {
        let mut erreurs = anomalies.iter().filter(|a| a.gravite == Gravite::Erreur);
        match self {
            Forcer::Rien => erreurs.next().is_some(),
            Forcer::Forme => erreurs.any(|a| !a.forcable),
            Forcer::Tout => false,
        }
    }
}

/// Ce qu'un refus de fiche porte a part : ses anomalies, et leur nombre en mots.
#[derive(Serialize)]
struct FicheRefusee<'a> {
    anomalies: &'a [Anomalie],
    erreurs_dit: String,
}

/// La fiche porte au moins une erreur : elle n'est pas ecrite.
fn fiche_refusee(nom: &str, anomalies: &[Anomalie]) -> R<Echec> {
    let mut phrases = Vec::new();
    for a in anomalies.iter().filter(|a| a.gravite == Gravite::Erreur) {
        phrases.push(match champs::champ(nom, &a.champ)?.map(|c| c.libelle).filter(|l| !l.is_empty()) {
            Some(l) => format!("« {} » : {}", l, a.message),
            None => a.message.clone(),
        });
    }
    let erreurs_dit = if phrases.len() > 1 { format!("{} erreurs", phrases.len()) } else { "Une erreur".to_string() };
    let message = format!("La fiche n'est pas écrite : {}", phrases.join(" ; "));
    Ok(Echec { classe: Some(Classe::FicheRefusee), message: socle::echec::avec(message, FicheRefusee { anomalies, erreurs_dit }) })
}

/// Pose sur chaque anomalie le libelle de sa case, celui que la fiche montre.
pub(crate) fn avec_libelles(nom: &str, anomalies: &mut [Anomalie]) -> R<()> {
    let champs = champs::fiche(nom)?;
    for a in anomalies.iter_mut() {
        // Le code PCS-ESE ne se tape pas : la fiche le remplit du metier choisi (`emplois.rs`).
        if a.champ == "pcs_ese" {
            a.libelle = Some("Métier (il donne le code PCS-ESE)".to_string());
        } else if let Some(c) = champs.iter().find(|c| c.cle == a.champ) {
            a.libelle = Some(c.libelle.clone());
        }
    }
    Ok(())
}

/// UNE FICHE PRETE A S'ECRIRE : telle que l'ecran l'a montree (`avant`), et telle qu'elle sera, ses changements poses
/// et derivee (`apres`), avec les cases que l'ecran a nommees.
pub(crate) struct Preparee<F: SorteDeFiche> {
    cle: Option<String>,
    avant: F::Plate,
    apres: F::Plate,
    vue_avant: Saisie,
    vue_apres: Saisie,
}

/// PREPARER L'ECRITURE de la fiche `cle` (`None` : une fiche neuve, ou l'entreprise) : la fiche telle que l'ecran l'a
/// montree, ce que `changer` y pose (il rend les cases qu'il nomme), ce que la fiche refuse quoi qu'on force, puis la
/// derivation des formes declarees de ce que ces cases disent.
pub(crate) fn preparer<F: SorteDeFiche>(ctx: &Contexte, cle: Option<&str>, changer: impl FnOnce(&mut F::Plate) -> Vec<&'static str>) -> R<Preparee<F>> {
    let mut avant = F::Plate::default();
    if let Some(p) = F::montree(ctx, cle) {
        avant = p;
    }
    let mut apres = avant.clone();
    let nommees = changer(&mut apres);
    F::ajuster(&avant, &mut apres);
    F::refuser(ctx, cle, &avant, &apres)?;
    if cle.is_none() {
        F::completer_neuve(ctx, &mut apres)?;
    }
    let vue_avant = vue::<F>(&avant)?;
    let vue_changee = vue::<F>(&apres)?;
    let mut vue_apres = vue_changee.clone();
    deriver(F::NOM, &mut vue_apres, true, &|c| nommees.contains(&c))?;
    reporter::<F>(&mut apres, &vue_changee, &vue_apres)?;
    let vue_apres = vue::<F>(&apres)?;
    Ok(Preparee { cle: cle.map(str::to_string), avant, apres, vue_avant, vue_apres })
}

/// Ce que l'ecriture d'une fiche a fait : sa cle (celle d'une fiche neuve est fabriquee ici), les anomalies, si le
/// fichier a change (une fiche enregistree telle qu'elle est n'ecrit rien), et les cases ecrites.
pub(crate) struct FicheEcrite {
    pub(crate) cle: Option<String>,
    pub(crate) anomalies: Vec<Anomalie>,
    pub(crate) ecrit: bool,
    pub(crate) champs: Vec<&'static str>,
}

/// ECRIRE UNE FICHE PREPAREE : verifiee entiere, telle qu'elle sera (une erreur la refuse, sauf ce que `forcer` passe
/// outre), puis chaque case qui change datee (`ecrire_datee`), et l'ancien fichier sauvegarde.
pub(crate) fn ecrire_fiche<F: SorteDeFiche>(ctx: &Contexte, p: Preparee<F>, forcer: Forcer, depuis: Option<Jour>, note: Option<&str>) -> R<FicheEcrite> {
    let anomalies = champs::verifier_fiche(F::NOM, &p.vue_apres)?;
    if forcer.refuse(&anomalies) {
        return Err(fiche_refusee(F::NOM, &anomalies)?);
    }
    let mut contenu = lire_fichier::<F>(ctx)?;
    let avant_fichier = contenu.clone();
    let cle = match (F::NOM != "entreprise", p.cle) {
        // UNE FICHE NEUVE : la cle se fabrique ici et nulle part ailleurs.
        (true, None) => Some(cle_neuve::<F>(&p.vue_apres, &contenu)?),
        (_, cle) => cle,
    };
    let brute = F::ou_neuve(&mut contenu, cle.as_deref())?;
    let mut champs = ecrire_datee::<F>(brute, &p.avant, &p.apres, false, depuis, note, &ctx.lue_au)?;
    if F::A_PART.iter().any(|k| F::differentes(&p.avant, &p.apres).contains(k)) && F::ecrire_a_part(ctx, brute, &p.apres, depuis, note)? {
        champs.extend(F::A_PART);
    }
    let ecrit = contenu != avant_fichier;
    if ecrit {
        ecrire_fichier::<F>(ctx, &contenu)?;
    }
    Ok(FicheEcrite { cle, anomalies, ecrit, champs })
}

/// La cle d'une fiche neuve, lisible et unique dans son fichier : le nom du salarie (puis son prenom), le titre du
/// spectacle (ou son numero d'objet, quand il n'en a pas encore).
fn cle_neuve<F: SorteDeFiche>(d: &Saisie, existants: &F::Fichier) -> R<String> {
    let propre = |texte: &str, bas: bool| -> String {
        let t = champs::sans_accents(texte).replace(['\'', '\u{2019}'], "");
        let t: String = t.chars().map(|c| if envois::etats::lettre_ou_chiffre(c) { c } else { '-' }).collect();
        let t = t.split('-').filter(|m| !m.is_empty()).collect::<Vec<_>>().join("-");
        if bas {
            t.to_lowercase()
        } else {
            t.to_uppercase()
        }
    };
    let dit = |k: &str| d.mot(k).map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
    let candidats: Vec<String> = if F::NOM == "salarie" {
        let base = dit("nom").or_else(|| dit("nom_affichage")).map(|n| propre(&n, false));
        let prenom = dit("prenom").or_else(|| dit("prenom_affichage")).map(|p| propre(&p, false));
        let ensemble = base.as_ref().map(|b| format!("{}-{}", b, prenom.iter().cloned().collect::<String>()));
        base.into_iter().chain(ensemble).collect()
    } else {
        let titre = dit("titre").filter(|t| !t.eq_ignore_ascii_case(A_COMPLETER));
        match (titre, dit("numero_objet")) {
            (Some(t), _) => vec![propre(&t, true)],
            (None, Some(n)) => vec![format!("objet-{}", n)],
            (None, None) => Vec::new(),
        }
    };
    let candidats: Vec<String> = candidats.into_iter().filter(|c| !c.is_empty() && !c.starts_with('-') && !c.ends_with('-')).collect();
    let Some(dernier) = candidats.last() else {
        return Err(refus(format!("Impossible de nommer cette fiche {} : il lui faut au moins un {}.", F::NOM, if F::NOM == "salarie" { "nom" } else { "titre" })));
    };
    if let Some(c) = candidats.iter().find(|c| !F::contient(existants, c)) {
        return Ok(c.clone());
    }
    let mut n = 2;
    while F::contient(existants, &format!("{}-{}", dernier, n)) {
        n += 1;
    }
    Ok(format!("{}-{}", dernier, n))
}

// ------------------------------------------------------------------ l'avertissement

/// UNE VALEUR QUI CHANGE, telle que la boite « Ce qui change » la montre : la case, son libelle, avant et apres ; ce
/// que la valeur sortante a deja servi a declarer ; la forme lisible d'une forme declaree.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ChangementDit {
    champ: String,
    libelle: String,
    avant: Option<Valeur>,
    apres: Option<Valeur>,
    #[serde(skip_serializing_if = "Option::is_none")]
    deja_declare: Option<DejaDeclare>,
    #[serde(skip_serializing_if = "Option::is_none")]
    avant_lisible: Option<Valeur>,
    #[serde(skip_serializing_if = "Option::is_none")]
    apres_lisible: Option<Valeur>,
}

impl ChangementDit {
    /// Un changement dit en mots (une ligne d'un autre etablissement).
    pub(crate) fn en_mots(champ: &str, libelle: String, avant: String, apres: String) -> ChangementDit {
        ChangementDit { champ: champ.to_string(), libelle, avant: Some(Valeur::Texte(avant)), apres: Some(Valeur::Texte(apres)), deja_declare: None, avant_lisible: None, apres_lisible: None }
    }
}

/// L'AVERTISSEMENT D'UN ENREGISTREMENT DE FICHE : ce qu'il ferait changer (chaque valeur existante qui change demande sa
/// date d'effet), et la date que le moteur propose (`date_proposee`, `depuis` et `pourquoi`) ; et ce qui se confirme sans
/// se dater (`usage`).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Avertissement {
    changements: Vec<ChangementDit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    depuis: Option<Jour>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pourquoi: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    usage: Option<String>,
}

/// L'avertissement d'une fiche preparee ; `None` : rien n'est a dater ni a confirmer. N'ecrit rien.
fn avertissement<F: SorteDeFiche>(ctx: &Contexte, p: &Preparee<F>) -> R<Option<Avertissement>> {
    let usage = F::usage(&p.avant, &p.apres)?;
    let cles: Vec<&'static str> = F::differentes(&p.avant, &p.apres).into_iter().filter(|k| !F::A_PART.contains(k)).collect();
    let a_dater = changements_a_dater(F::NOM, &p.vue_avant, &p.vue_apres, &cles, false)?;
    let mut changements = changements_dits(ctx, F::NOM, &p.vue_avant, &p.vue_apres, &a_dater)?;
    if !p.vue_avant.cases().is_empty() {
        changements.extend(F::a_dater_a_part(&p.avant, &p.apres));
    }
    if changements.is_empty() {
        return Ok(usage.map(|u| Avertissement { changements, depuis: None, pourquoi: None, usage: Some(u) }));
    }
    let (depuis, pourquoi) = date_proposee(ctx, F::NOM, p.cle.as_deref())?;
    Ok(Some(Avertissement { changements, depuis: Some(depuis), pourquoi: Some(pourquoi), usage }))
}

/// Chaque valeur a dater, telle que la boite la montre ; un champ declare dit aussi ce que sa valeur sortante a deja
/// servi a declarer.
fn changements_dits(ctx: &Contexte, nom: &str, avant: &Saisie, apres: &Saisie, a_dater: &[&'static str]) -> R<Vec<ChangementDit>> {
    let fiche = champs::fiche(nom)?;
    let comble = avant.comblee(nom)?;
    let mut sortie = Vec::new();
    for k in a_dater {
        let c = fiche.iter().find(|c| c.cle == *k);
        // Un champ calcule qui change avec la case d'ou il derive est le meme fait : la case le dit seule.
        if c.filter(|c| c.calcule()).and_then(|c| c.derive_de.as_deref()).is_some_and(|s| a_dater.contains(&s)) {
            continue;
        }
        let libelle = match c {
            Some(c) => c.libelle.clone(),
            None => k.to_string(),
        };
        let mut dit = ChangementDit { champ: k.to_string(), libelle, avant: avant.case(k).cloned(), apres: apres.case(k).cloned(), deja_declare: None, avant_lisible: None, apres_lisible: None };
        if let Some(c) = c.filter(|c| c.declare) {
            let valeur = mot_de(avant.case(k));
            dit.deja_declare = Some(super::usages_declares(ctx, c.rubrique.as_deref(), valeur.as_deref())?);
        }
        // La forme lisible d'une forme declaree : c'est elle que la boite montre.
        if let Some(source) = c.filter(|c| c.calcule()).and_then(|c| c.derive_de.as_deref()) {
            if let Some(l) = fiche.iter().find(|x| x.cle == source) {
                dit.libelle = l.libelle.clone();
            }
            dit.avant_lisible = comble.case(source).cloned();
            dit.apres_lisible = apres.case(source).cloned();
        }
        sortie.push(dit);
    }
    Ok(sortie)
}

/// Une valeur en mots, pour une phrase : vide, un texte, oui ou non, des mots, les cases d'une structure.
fn en_mots(v: Option<&Valeur>) -> String {
    match v {
        None => "vide".to_string(),
        Some(Valeur::Texte(t)) if t.trim().is_empty() => "vide".to_string(),
        Some(Valeur::Texte(t)) => t.clone(),
        Some(Valeur::Nombre(n)) => n.to_string(),
        Some(Valeur::OuiNon(b)) => (if *b { "oui" } else { "non" }).to_string(),
        Some(Valeur::Mots(l)) => l.join(", "),
        Some(Valeur::Bloc(b)) => b.cases().iter().map(|(k, x)| format!("{} {}", k, en_mots(Some(x)))).collect::<Vec<_>>().join(", "),
        Some(Valeur::Lignes(l)) => format!("{} lignes", l.len()),
    }
}

/// L'avertissement en phrases : ce qui se confirme, chaque valeur qui change, puis la date proposee et ce qui l'ecrit.
fn avertissement_en_phrases(a: &Avertissement) -> Vec<String> {
    let mut lignes: Vec<String> = a.usage.iter().cloned().collect();
    lignes.extend(a.changements.iter().map(|c| {
        let (avant, apres) = match (&c.avant_lisible, &c.apres_lisible) {
            (None, None) => (&c.avant, &c.apres),
            _ => (&c.avant_lisible, &c.apres_lisible),
        };
        format!("{} : {} devient {}.", c.libelle, en_mots(avant.as_ref()), en_mots(apres.as_ref()))
    }));
    match (&a.depuis, &a.pourquoi) {
        (Some(depuis), Some(pourquoi)) => {
            lignes.push(format!("À partir du {}. {} La même charge avec « confirme » l'écrit (« depuis » pour une autre date).", depuis.jj_mm_aaaa(), pourquoi))
        }
        _ => lignes.push("La même charge avec « confirme » l'écrit.".to_string()),
    }
    lignes
}

// ------------------------------------------------------------------ la route enregistrer-fiche

/// La charge de `enregistrer-fiche`, pour une fiche : sa cle (absente : une fiche neuve, ou l'entreprise), ses
/// changements, la date d'effet de ce qui change et pourquoi, et ce qui est lu et passe outre.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "C: serde::de::DeserializeOwned"))]
pub(crate) struct Enregistrement<C> {
    cle: Option<String>,
    changements: droit::fiche::Changements<C>,
    depuis: Option<Jour>,
    note: Option<String>,
    forcer: Option<bool>,
    confirme: Option<bool>,
}

/// La charge de `enregistrer-fiche` : la fiche nommee (`nom`) et son enregistrement.
#[derive(Deserialize)]
#[serde(tag = "nom", deny_unknown_fields)]
pub(crate) enum ChargeEnregistrer {
    #[serde(rename = "entreprise")]
    Entreprise(Enregistrement<droit::fiche::ChangementEntreprise>),
    #[serde(rename = "salarie")]
    Salarie(Enregistrement<droit::fiche::ChangementSalarie>),
    #[serde(rename = "spectacle")]
    Spectacle(Enregistrement<droit::fiche::ChangementSpectacle>),
}

/// Ce que rend `enregistrer-fiche` : l'avertissement (rien n'est ecrit), ou ce qui l'est.
#[derive(Serialize)]
#[serde(untagged)]
pub(crate) enum FicheEnregistree {
    Avertie { avertissement: Avertissement, phrase: String, phrases: Vec<String> },
    Ecrite { cle: Option<String>, ecrits: Vec<&'static str>, enregistre_a: Option<String>, enregistre_dit: Option<String>, phrase: String },
}

impl crate::routes::enveloppe::Dite for FicheEnregistree {
    fn phrase(&self) -> &str {
        match self {
            FicheEnregistree::Avertie { phrase, .. } | FicheEnregistree::Ecrite { phrase, .. } => phrase,
        }
    }
}

/// `enregistrer-fiche` : une fiche de l'ecran, ecrite (`ecrire_fiche`). Une valeur existante qui change demande sa date
/// d'effet : sans `confirme`, la route rend l'avertissement (`avertissement`) et n'ecrit rien ; la meme charge avec
/// `confirme` ecrit, a partir de `depuis` (la date proposee, s'il manque).
pub(crate) fn enregistrer_fiche(ctx: &Contexte, charge: ChargeEnregistrer) -> R<FicheEnregistree> {
    match charge {
        ChargeEnregistrer::Entreprise(e) => enregistrer::<FicheEntrepriseDatee>(ctx, e),
        ChargeEnregistrer::Salarie(e) => enregistrer::<FicheSalarieDatee>(ctx, e),
        ChargeEnregistrer::Spectacle(e) => enregistrer::<FicheSpectacleDatee>(ctx, e),
    }
}

fn enregistrer<F: SorteDeFiche>(ctx: &Contexte, e: Enregistrement<F::Changement>) -> R<FicheEnregistree> {
    let Enregistrement { cle, changements, depuis, note, forcer, confirme } = e;
    // L'entreprise est le fichier entier : elle n'a pas de cle.
    let cle = cle.filter(|_| F::NOM != "entreprise");
    let p = preparer::<F>(ctx, cle.as_deref(), |plate| {
        changements
            .0
            .into_iter()
            .map(|ch| {
                let k = F::case_changee(&ch);
                F::appliquer(plate, ch);
                k
            })
            .collect()
    })?;
    let forcer = forcer == Some(true);
    let mut depuis = depuis;
    if let Some(a) = avertissement(ctx, &p)? {
        if confirme != Some(true) {
            let phrase = if a.depuis.is_some() { "Ce qui change se date : rien n'est écrit." } else { "À confirmer : rien n'est écrit." };
            return Ok(FicheEnregistree::Avertie { phrases: avertissement_en_phrases(&a), avertissement: a, phrase: phrase.to_string() });
        }
        depuis = depuis.or(a.depuis);
    }
    let neuve = F::NOM != "entreprise" && cle.is_none();
    let note = note.as_deref().map(str::trim).filter(|n| !n.is_empty());
    let e = ecrire_fiche::<F>(ctx, p, if forcer { Forcer::Forme } else { Forcer::Rien }, depuis, note)?;
    let en_erreur = e.anomalies.iter().any(|a| a.gravite == Gravite::Erreur);
    // Une fiche creee se nomme ensuite par sa cle (la saisie d'une carte, `cle` d'une autre ecriture) : la reponse la rend.
    let phrase = match (e.ecrit, neuve, forcer && en_erreur) {
        (false, _, _) => "Rien n'a changé : rien n'est écrit.",
        (true, _, true) => "Enregistré malgré les erreurs.",
        (true, true, false) => "Fiche créée.",
        (true, false, false) => "Enregistré.",
    };
    // « enregistré à HH:MM » : l'heure du moteur (`temps.rs`), pas celle du navigateur (au banc, le temps est simule).
    let enregistre_a = e.ecrit.then(maintenant);
    let enregistre_dit = enregistre_a.as_ref().map(|a| format!("enregistré à {}", a.chars().skip(11).take(5).collect::<String>()));
    Ok(FicheEnregistree::Ecrite { cle: e.cle, ecrits: e.champs, enregistre_a, enregistre_dit, phrase: phrase.to_string() })
}

// ------------------------------------------------------------------ la route verifier-fiche

/// Ce que le moteur PROPOSERAIT pour une case encore vide, et pourquoi.
#[derive(Debug, Clone, Serialize)]
struct Suggestion {
    valeur: String,
    pourquoi: &'static str,
}

/// Ce que rend `verifier-fiche` : les anomalies de la fiche telle qu'elle SERAIT derivee, ce qui serait declare, ce que
/// le moteur proposerait pour les cases vides.
#[derive(Serialize)]
pub(crate) struct FicheVerifiee {
    phrase: String,
    phrases: Vec<String>,
    anomalies: Vec<Anomalie>,
    declare: Apercu,
    suggestions: BTreeMap<&'static str, Suggestion>,
}

impl crate::routes::enveloppe::Dite for FicheVerifiee {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

/// `verifier-fiche` : la saisie derivee (`deriver`, sans forcer : seul ce qui est vide ou faux se refait), verifiee comme
/// a l'ecriture, une case que la fiche ne decrit pas comprise.
pub(crate) fn verifier_fiche(s: SaisieDeFiche) -> R<FicheVerifiee> {
    let (nom, saisie) = s.ouverte();
    let mut derivee = saisie.clone();
    deriver(nom, &mut derivee, false, &|_| true)?;
    let mut anomalies = champs::verifier_fiche(nom, &derivee)?;
    anomalies.extend(champs::cases_inconnues(nom, &derivee, true)?);
    let phrases: Vec<String> = anomalies.iter().map(|a| format!("{} : {}", a.champ, a.message)).collect();
    let phrase = if phrases.is_empty() { "La fiche est juste.".to_string() } else { format!("Anomalies de la fiche : {}.", phrases.len()) };
    Ok(FicheVerifiee { phrase, phrases, anomalies, declare: champs::apercu_derive(nom, &saisie)?, suggestions: suggestions(nom, &saisie)? })
}

/// Ce que le moteur PROPOSERAIT pour une case encore vide.
fn suggestions(nom: &str, d: &Saisie) -> R<BTreeMap<&'static str, Suggestion>> {
    let mut propose = BTreeMap::new();
    if nom == "salarie" {
        if let Some(dep) = texte(d, "nir").and_then(champs::departement_du_nir) {
            let mut trouvees = vec![("departement_naissance", dep.clone())];
            if dep != "99" {
                trouvees.push(("pays_naissance", "FR".to_string()));
            }
            for (cle, valeur) in trouvees {
                if texte(d, cle).is_none() {
                    propose.insert(cle, Suggestion { valeur, pourquoi: "lu dans le numéro de sécurité sociale" });
                }
            }
        }
    } else if nom == "entreprise" && texte(d, "code_urssaf").is_none() {
        if let Some(code) = texte(d, "code_postal").map(|cp| declarations::urssaf::base().map(|b| b.du_code_postal(cp))).transpose().map_err(Echec::from)?.flatten() {
            propose.insert(
                "code_urssaf",
                Suggestion {
                    valeur: code,
                    pourquoi: "l'organisme du département, d'après votre code postal — à corriger si le vôtre diffère (versement en lieu unique, pôle grandes entreprises, GUSO)",
                },
            );
        }
    }
    Ok(propose)
}
