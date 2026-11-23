//! LE SALARIE REPRIS SANS NIR, ET LA DSN AJOUTEE ENSUITE. Un salarie ne se rattache a sa DSN que par son NIR
//! (`routes::import::apercu_lu`, `par_nir`) : la fiche d'un salarie qui n'a pas de NIR (saisie a la main, ou reprise de
//! pieces qui ne l'impriment pas) n'est reconnue par aucune DSN. Rapprocher par le nom serait deviner (regle 1) ; creer
//! en silence une seconde fiche doublerait la personne. L'import demande donc (Léandre, 18/11/2026) :
//!
//! - `questions` : chaque individu qu'une DSN declare sous un NIR qu'aucune fiche ne porte et qui ressemble au salarie
//!   d'une fiche sans NIR (`ressemble`) fait une question (`import::reponse::QuestionDIdentite`), une par fiche qui lui
//!   ressemble ; tant qu'elle est posee, l'import n'ecrit rien de cet individu (ni fiche, ni contrat, ni paie). Elles se
//!   disent dans la reponse d'`importer` et dans `importation-etat` (`en_attente`, avec leurs gestes) ;
//! - la route `importation-identite` `{"nir", "salarie", "reponse": "meme" | "autre"}` y repond : « la meme personne »,
//!   la fiche recoit le NIR que la DSN declare (`importation::fiches::completer_fiche` : une case vide se complete), et
//!   l'import rejoue la reconnait comme tout salarie rattache ; « une autre », la reponse se garde
//!   (`config/import-identites.json`, `PasLaMemePersonne`) et l'import rejoue cree sa fiche comme aujourd'hui. Une
//!   reponse ne se redemande pas : la fiche a le NIR, ou la paire est gardee.
//!
//! « Ressemble » ne sert qu'a poser la question, jamais a decider.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use droit::contexte::{Contexte, IdSalarie};
use droit::fiche::{ChampsSalarie, FicheNommee, Lue as _, Nir};
use import::dsn_aem::SalarieDeclare;
use import::proposee::FicheProposee;
use import::reponse::QuestionDIdentite;
use socle::echec::{erreur, Classe, R};
use socle::texte::Phrase;

use crate::catalogue::{Affiche, Route};

/// LES REPONSES « UNE AUTRE PERSONNE » DE LA COMPAGNIE, sous sa racine : ce qu'elle dit, avec sa configuration (comme
/// `config/casc-svp.json`), pas au journal, qui tient ce qui est parti.
pub(crate) const IDENTITES: socle::fichiers::CheminRelatif = socle::fichiers::CheminRelatif::du_moteur("config/import-identites.json");

/// Ce que dit le fichier (sa note `quoi`).
const QUOI: &str = "Les personnes que les DSN reprises déclarent et que la compagnie a dites autres que le salarié d'une fiche sans numéro de sécurité sociale.";

/// `config/import-identites.json` : sa note, et les reponses dans l'ordre ou elles ont ete donnees.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ReponsesDIdentite {
    quoi: String,
    autres: Vec<PasLaMemePersonne>,
}

fn fichier(racine: &Path) -> PathBuf {
    IDENTITES.dans(racine)
}

/// Ecrit les reponses gardees.
fn garder(racine: &Path, autres: &[PasLaMemePersonne]) -> R<()> {
    socle::ecriture::ecrire_json(&fichier(racine), &ReponsesDIdentite { quoi: QUOI.to_string(), autres: autres.to_vec() }, 0)
}

/// UNE REPONSE « CE N'EST PAS LA MEME PERSONNE », gardee : l'individu que les DSN declarent sous le NIR `nir` n'est pas
/// le salarie de la fiche `salarie`. « La meme personne » ne se garde pas ici : la fiche recoit le NIR, et les DSN la
/// reconnaissent.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PasLaMemePersonne {
    nir: Nir,
    salarie: IdSalarie,
}

/// Les reponses « une autre personne » ; aucune tant que le fichier n'existe pas. Un fichier qui ne se lit pas se refuse.
fn pas_la_meme(racine: &Path) -> R<Vec<PasLaMemePersonne>> {
    let p = fichier(racine);
    if !socle::fichiers::existe_lu(&p)? {
        return Ok(Vec::new());
    }
    let lues: ReponsesDIdentite = socle::forme::lire_fichier(&p, "Les réponses aux questions d'identité de l'import ne se lisent pas : le signaler au développeur.")?;
    Ok(lues.autres)
}

/// LA FICHE SANS NIR RESSEMBLE-T-ELLE A L'INDIVIDU QUE LA DSN DECLARE ? Ce que les deux portent en commun : le meme nom
/// de famille (S21.G00.30.002 ; la case `nom` de la fiche) et le meme premier prenom (S21.G00.30.004 ; la case `prenom`),
/// lus sans accents ni capitales ni tirets (`import::bulletin_lu::jetons`), et la meme date de naissance (S21.G00.30.006 ;
/// `date_naissance`) quand la fiche et la DSN la disent toutes deux en entier. Une fiche sans nom ou sans prenom ne
/// ressemble a personne.
fn ressemble(fiche: &ChampsSalarie, individu: &declarations::dsn_lue::IndividuDeclare) -> bool {
    use import::bulletin_lu::jetons;
    let i = &individu.identite;
    let (Some(nom_f), Some(nom_d), Some(prenom_f), Some(prenom_d)) = (fiche.nom.remplie(), i.nom.as_deref(), fiche.prenom.remplie(), i.prenoms.as_deref()) else {
        return false;
    };
    let premier = |t: &str| jetons(t).into_iter().next();
    let memes_noms = !jetons(nom_f).is_empty() && jetons(nom_f) == jetons(nom_d) && premier(prenom_f).is_some() && premier(prenom_f) == premier(prenom_d);
    let naissance = match (fiche.date_naissance.dite(), i.naissance.as_ref().and_then(|n| n.jour())) {
        (Some(f), Some(d)) => *f == d,
        _ => true,
    };
    memes_noms && naissance
}

/// Le salarie d'une fiche, tel que l'ecran le nomme ; sa cle quand la fiche ne nomme personne.
fn nom_de(cle: &IdSalarie, fiche: &ChampsSalarie) -> String {
    match fiche.nomme() {
        Some(n) => n,
        None => cle.to_string(),
    }
}

/// Les fiches sans NIR qui peuvent ressembler a quelqu'un : celles qui disent un nom et un prenom (`ressemble`).
fn sans_nir(ctx: &Contexte) -> Vec<(&IdSalarie, &ChampsSalarie)> {
    ctx.salaries
        .iter()
        .filter(|(_, s)| s.nir.dite().is_none() && s.champs.nom.remplie().is_some() && s.champs.prenom.remplie().is_some())
        .map(|(k, s)| (k, &s.champs))
        .collect()
}

/// LES QUESTIONS D'IDENTITE que posent les individus que les DSN declarent (`declares`) : sous un NIR qu'aucune fiche ne
/// porte, ni aucune piece de cet import qui en creera une (`a_venir` : les bulletins, les certificats), ils ressemblent
/// au salarie d'une fiche sans NIR (`ressemble`), et la compagnie n'a pas dit que ce n'etait pas lui. Une par individu et
/// par fiche qui lui ressemble. Aucune sans fiche sans NIR.
pub(crate) fn questions(ctx: &Contexte, declares: &[SalarieDeclare], a_venir: &HashSet<Nir>) -> R<Vec<QuestionDIdentite>> {
    let sans_nir = sans_nir(ctx);
    if sans_nir.is_empty() {
        return Ok(Vec::new());
    }
    let connus: HashSet<&Nir> = ctx.salaries.values().filter_map(|s| s.nir.dite()).collect();
    let repondues = pas_la_meme(&ctx.racine)?;
    let mut out = Vec::new();
    for d in declares {
        let Some(nir) = d.individu.identite.nir.as_ref() else { continue };
        if connus.contains(nir) || a_venir.contains(nir) {
            continue;
        }
        let qui = [d.individu.identite.nom.clone(), d.individu.identite.prenoms.as_deref().map(declarations::champs::titre)].into_iter().flatten().collect::<Vec<_>>().join(" ");
        for (cle, fiche) in &sans_nir {
            if !ressemble(fiche, &d.individu) || repondues.iter().any(|r| r.nir == *nir && r.salarie == **cle) {
                continue;
            }
            let nom = nom_de(cle, fiche);
            let question = socle::phrase!(
                "{}, déclaré dans la DSN, ressemble à la fiche de {}, qui n'a pas de numéro de sécurité sociale : est-ce la même personne ? Rien de lui n'est repris avant votre réponse.",
                qui,
                nom
            );
            out.push(QuestionDIdentite { nir: nir.clone(), qui: qui.clone(), salarie: (*cle).clone(), nom, question });
        }
    }
    Ok(out)
}

/// LES QUESTIONS QUI ATTENDENT LA COMPAGNIE, relues des DSN que l'import a reprises (leurs copies, sous
/// `historique/bulletins/`), chacune avec ses deux gestes : `importation-etat` les rend. Aucune sans fiche sans NIR qui
/// dise un nom et un prenom (rien alors ne se relit), ni sans DSN reprise. LA LECTURE REPOND (« Le logiciel est doux ») :
/// une DSN reprise qui ne se relit plus, ou des reponses gardees qui ne se lisent pas, ne ferment pas l'etat de l'import ;
/// aucune question ne se dit, et `non_relues` dit pourquoi. Une panne reste une panne.
pub(crate) fn en_attente(ctx: &Contexte) -> R<IdentitesEnAttente> {
    let aucune = IdentitesEnAttente { questions: Vec::new(), non_relues: None };
    if sans_nir(ctx).is_empty() {
        return Ok(aucune);
    }
    let dossier = import::mois::dossier_des_bulletins(&ctx.racine);
    if !socle::fichiers::existe_lu(&dossier)? {
        return Ok(aucune);
    }
    // Seule la DSN reprise qui ne se relit plus est un manque ; nos reponses gardees qui ne se lisent pas sont une panne.
    let lecture = match import::dsn_aem::lire_dsn(&dossier) {
        Ok(l) => l,
        Err(e) if e.dit_un_manque() => {
            return Ok(IdentitesEnAttente {
                questions: Vec::new(),
                non_relues: Some(socle::phrase!("Les questions d'identité des DSN reprises ne se relisent pas : {}", e.phrase)),
            })
        }
        Err(e) => return Err(e),
    };
    let q = questions(ctx, &lecture.salaries, &HashSet::new())?;
    Ok(IdentitesEnAttente { questions: q.into_iter().map(IdentiteADire::de).collect(), non_relues: None })
}

/// Les questions d'identite qui attendent la compagnie, et ce qui n'a pas pu se relire pour les poser.
#[derive(Debug, serde::Serialize)]
pub(crate) struct IdentitesEnAttente {
    questions: Vec<IdentiteADire>,
    #[serde(skip_serializing_if = "Option::is_none")]
    non_relues: Option<Phrase>,
}

/// UNE DSN DECLARE-T-ELLE SOUS `nir` UN INDIVIDU QUI RESSEMBLE A `fiche` (`ressemble`) : une DSN reprise (ses copies,
/// sous `historique/bulletins/`) ou une DSN de la boite de l'import (`import/`) ?
fn declare_et_ressemble(ctx: &Contexte, nir: &Nir, fiche: &ChampsSalarie) -> R<bool> {
    for dossier in [import::mois::dossier_des_bulletins(&ctx.racine), ctx.racine.join(import::BOITE_AUX_LETTRES)] {
        if !socle::fichiers::existe_lu(&dossier)? {
            continue;
        }
        let lecture = import::dsn_aem::lire_dsn(&dossier)?;
        if lecture.salaries.iter().any(|d| d.individu.identite.nir.as_ref() == Some(nir) && ressemble(fiche, &d.individu)) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Une question d'identite telle que la page la lit : la question, et ses deux gestes (la meme personne, une autre).
#[derive(Debug, serde::Serialize)]
pub(crate) struct IdentiteADire {
    #[serde(flatten)]
    question: QuestionDIdentite,
    gestes: Vec<crate::imprevus::GesteDeRoute<ChargeIdentite>>,
}

// Les libelles des deux gestes, en constantes : un texte fautif ne compile pas (`Affiche::new`).
const LA_MEME: Affiche = Affiche::new("C'est la même personne");
const UNE_AUTRE: Affiche = Affiche::new("Ce n'est pas la même personne");

impl IdentiteADire {
    /// La question et ses deux gestes, qui ne font rien saisir : ils partent avec leur charge.
    fn de(question: QuestionDIdentite) -> IdentiteADire {
        let geste = |reponse: ReponseDIdentite, libelle: Affiche| {
            let charge = ChargeIdentite { nir: question.nir.clone(), salarie: question.salarie.clone(), reponse };
            crate::imprevus::GesteDeRoute::sans_cases(Route::ImportationIdentite, charge, libelle)
        };
        let gestes = vec![geste(ReponseDIdentite::Meme, LA_MEME), geste(ReponseDIdentite::Autre, UNE_AUTRE)];
        IdentiteADire { question, gestes }
    }
}

/// LA REPONSE A UNE QUESTION D'IDENTITE : l'individu que la DSN declare est le salarie de la fiche (`meme`), ou une autre
/// personne (`autre`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ReponseDIdentite {
    Meme,
    Autre,
}

impl socle::liste::ListeFermee for ReponseDIdentite {
    const TOUTES: &'static [ReponseDIdentite] = &[ReponseDIdentite::Meme, ReponseDIdentite::Autre];

    fn code(&self) -> socle::echec::R<String> {
        socle::liste::code_ecrit(self)
    }

    fn libelle(&self) -> socle::echec::R<&'static str> {
        Ok(match self {
            ReponseDIdentite::Meme => "C'est le salarié de la fiche",
            ReponseDIdentite::Autre => "C'est une autre personne",
        })
    }
}

/// La charge d'`importation-identite` : le NIR que la DSN declare, la fiche sans NIR qui lui ressemble, la reponse.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ChargeIdentite {
    nir: Nir,
    salarie: IdSalarie,
    reponse: ReponseDIdentite,
}

/// Ce qu'`importation-identite` rend : ce que l'ecran en dit.
#[derive(Debug, serde::Serialize)]
pub(crate) struct IdentiteRepondue {
    phrase: Phrase,
}

impl crate::enveloppe::Dite for IdentiteRepondue {
    fn phrase(&self) -> &Phrase {
        &self.phrase
    }
}

fn refus(m: impl Into<String>) -> socle::echec::Echec {
    erreur(Classe::Refuse, m)
}

/// `importation-identite` : la reponse de la compagnie. « La meme personne » : la fiche recoit le NIR (et sa cle), une
/// reponse « une autre » gardee pour cette paire s'efface ; une fiche qui porte deja ce NIR n'a rien a recevoir.
/// « Une autre » : la paire se garde. Refus, et rien n'est ecrit : une fiche qui n'existe pas, « la meme » pour une fiche
/// qui a un autre NIR ou pour un NIR qu'une autre fiche porte, « une autre » pour la fiche qui porte ce NIR.
pub(crate) fn repondre(ctx: &Contexte, charge: &ChargeIdentite) -> R<IdentiteRepondue> {
    let (nir, cle) = (&charge.nir, &charge.salarie);
    let Some(fiche) = ctx.salaries.get(cle) else {
        return Err(refus(format!("Aucune fiche de salarié « {} » : rien n'a été écrit.", cle)));
    };
    let qui = nom_de(cle, &fiche.champs);
    let mut repondues = pas_la_meme(&ctx.racine)?;
    let paire = PasLaMemePersonne { nir: nir.clone(), salarie: cle.clone() };
    let phrase = match charge.reponse {
        ReponseDIdentite::Meme => {
            if fiche.nir.dite() == Some(nir) {
                Phrase::fixe("Cette fiche porte déjà ce numéro de sécurité sociale : rien ne change.")
            } else {
                if fiche.nir.dite().is_some() {
                    return Err(refus(format!("La fiche de {} a déjà un autre numéro de sécurité sociale : ce n'est pas la personne que la DSN déclare. Rien n'a été écrit.", qui)));
                }
                if let Some((autre, f)) = ctx.salaries.iter().find(|(_, s)| s.nir.dite() == Some(nir)) {
                    return Err(refus(format!("Ce numéro de sécurité sociale est déjà sur la fiche de {} : rien n'a été écrit.", nom_de(autre, &f.champs))));
                }
                // La reponse repond a une question : le NIR est celui d'un individu qu'une DSN reprise ou a reprendre
                // declare, et qui ressemble a la fiche.
                if !declare_et_ressemble(ctx, nir, &fiche.champs)? {
                    return Err(refus(format!(
                        "Aucune DSN reprise ni aucune DSN de la boîte de l'import ne déclare, sous ce numéro de sécurité sociale, une personne qui ressemble à {} : rien n'a été écrit.",
                        qui
                    )));
                }
                let champs = ChampsSalarie { nir: Some(nir.clone()), nir_cle: droit::fiche::CleNir::du_nir(nir), ..ChampsSalarie::default() };
                // La fiche ne recoit le NIR que si sa case est vide sur le disque (`completer_fiche`) : la phrase dit ce
                // qui s'est ecrit.
                let ecrit = crate::importation::fiches::completer_fiche(ctx, &FicheNommee::Salarie(cle.clone()), &FicheProposee::Salarie(champs))?;
                if !matches!(&ecrit, FicheProposee::Salarie(c) if c.nir.is_some()) {
                    return Err(refus(format!(
                        "La case du numéro de sécurité sociale de la fiche de {} n'est pas vide : rien n'a été écrit. Dites son numéro dans sa fiche, puis relancez l'import.",
                        qui
                    )));
                }
                if repondues.contains(&paire) {
                    repondues.retain(|r| *r != paire);
                    garder(&ctx.racine, &repondues)?;
                }
                socle::phrase!("La fiche de {} reçoit le numéro de sécurité sociale que la DSN déclare. Relancez l'import : il reprend ce que la DSN déclare de lui.", qui)
            }
        }
        ReponseDIdentite::Autre => {
            if fiche.nir.dite() == Some(nir) {
                return Err(refus(format!("La fiche de {} porte ce numéro de sécurité sociale : c'est la personne que la DSN déclare. Rien n'a été écrit.", qui)));
            }
            if !repondues.contains(&paire) {
                repondues.push(paire);
                garder(&ctx.racine, &repondues)?;
            }
            socle::phrase!("Noté : la personne que la DSN déclare n'est pas {}. Relancez l'import : il lui crée sa fiche.", qui)
        }
    };
    Ok(IdentiteRepondue { phrase })
}
