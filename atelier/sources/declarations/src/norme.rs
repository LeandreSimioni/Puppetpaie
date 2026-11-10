//! La version de la norme DSN (NEODeS) qu'un fichier porte, `S10.G00.00.006`.
//!
//! Elle ne vient plus d'une constante : `logiciel/donnees/norme-dsn-base.json` dit, par version, ce qui la declenche
//! (le mois principal declare d'une DSN mensuelle, le jour de constitution d'un signalement), ses tables d'usages et de
//! formats, les valeurs de liste qu'elle n'admet plus et le code de la CET patronale. Le moteur applique la donnee ;
//! ce qu'elle ne couvre pas refuse. LA REGLE DE BASCULE EST UNE INFERENCE (la `note` de la donnee) : le cahier
//! technique 2027.1.1 ne la donne pas (p. 129, une seule valeur par cahier ).
//!
//! Le fichier en cours de fabrication porte sa norme (`dsn::Fichier::en_norme`) : « ce fichier est-il en P27V01 ? »
//! se demande a lui, par la table de sa version (`Norme::admet`), en un seul endroit.

use socle::date::Jour;
use socle::echec::APart;
use socle::num::Resultat;
use crate::usages::{lignes, rubrique_valeur};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::Arc;

const BASE: &str = "norme-dsn-base.json";
pub const RUBRIQUE: &str = "S10.G00.00.006";

/// Les natures de declaration (S20.G00.05.001) d'un signalement que le logiciel envoie : arret de travail (04), reprise
/// (05), fin du contrat de travail unique (07), amorcage des donnees variables (08) ; la mensuelle est 01
/// (`atelier/reference/norme-dsn/texte/bloc-S20.G00.05.txt`).
pub const SIGNALEMENTS: [&str; 4] = ["04", "05", "07", "08"];

/// UNE DONNEE LIVREE (`logiciel/donnees/<nom>`) EN SA FORME `T`, lue une fois par contenu (`socle::forme::partagee`) et
/// notee au registre d'un calcul suivi : absente ou fausse, rien que la compagnie puisse corriger.
pub(crate) fn livree<T: serde::de::DeserializeOwned + Send + Sync + 'static>(nom: &str) -> Resultat<Arc<T>> {
    let chemin = socle::fichiers::dossier_du_code().join(nom);
    let lue = socle::forme::partagee::<T>(&chemin, "Une donnée livrée avec le logiciel ne se lit pas : le signaler au développeur.")?;
    socle::fichiers::noter_si_besoin(&chemin);
    lue.ok_or_else(|| socle::echec::avec("Une donnée livrée avec le logiciel manque : le signaler au développeur.", APart { fichier: Some(nom.to_string()), ..APart::default() }))
}

/// `norme-dsn-base.json`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Base {
    #[allow(dead_code)]
    quoi: String,
    #[allow(dead_code)]
    note: String,
    /// Les versions, dans l'ordre de la donnee.
    #[serde(deserialize_with = "socle::forme::en_ordre")]
    versions: Vec<(String, Version)>,
}

/// Une version de la norme dans la base.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Version {
    #[allow(dead_code)]
    libelle: String,
    /// Les mois principaux declares (`AAAA-MM`) d'une DSN mensuelle en cette version.
    mensuelle: Plage,
    /// Les jours de constitution (`AAAA-MM-JJ`) d'un signalement en cette version.
    signalements: Plage,
    /// La table des usages (un fichier de `logiciel/donnees/`).
    usages: String,
    /// La table des formats.
    formats: String,
    /// Par rubrique, les valeurs que la version n'admet plus dans sa liste, et pourquoi.
    valeurs_retirees: BTreeMap<String, BTreeMap<String, String>>,
    /// Les rubriques que le tableau des usages admet et que la description dit « a ne pas renseigner ».
    rubriques_a_ne_pas_renseigner: Option<ANePasRenseigner>,
    /// Les regles d'ecriture que la version ajoute a la precedente : le nom de la regle, le texte qui la porte.
    regles: BTreeMap<String, String>,
    /// Le code de cotisation (S21.G00.81.001) de la CET patronale.
    cotisation_cet_patronale: Option<String>,
    note: String,
}

/// Une plage de mois ou de jours, bornes ouvertes si absentes ; la comparaison de textes de meme forme suffit.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Plage {
    du: Option<String>,
    au: Option<String>,
    #[allow(dead_code)]
    note: String,
}

impl Plage {
    fn couvre(&self, v: &str) -> bool {
        self.du.as_deref().is_none_or(|d| d <= v) && self.au.as_deref().is_none_or(|a| v <= a)
    }

    /// « du 2027-01 au 2027-12 », « ... » pour une borne ouverte.
    fn dite(&self) -> String {
        let borne = |b: &Option<String>| match b {
            Some(b) => b.clone(),
            None => "...".to_string(),
        };
        format!("du {} au {}", borne(&self.du), borne(&self.au))
    }
}

/// `rubriques_a_ne_pas_renseigner` : sa note, et le texte qui retire chaque rubrique.
struct ANePasRenseigner(BTreeMap<String, String>);

impl<'de> Deserialize<'de> for ANePasRenseigner {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let (_note, rubriques): (Option<String>, Vec<(String, String)>) = socle::forme::fiches_et_note(d, "note")?;
        Ok(ANePasRenseigner(rubriques.into_iter().collect()))
    }
}

/// Une version de la norme que le moteur sait ecrire (`P26V01`, `P27V01`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Norme {
    pub version: String,
}

fn base() -> Resultat<Arc<Base>> {
    livree(BASE)
}

/// Les versions livrees, dans l'ordre de la donnee.
pub fn versions() -> Resultat<Vec<Norme>> {
    Ok(base()?.versions.iter().map(|(k, _)| Norme { version: k.clone() }).collect())
}

fn liste_des_versions() -> Resultat<String> {
    Ok(versions()?.iter().map(|n| n.version.clone()).collect::<Vec<_>>().join(", "))
}

impl Norme {
    /// La version nommee, si la donnee la connait.
    pub fn connue(version: &str) -> Resultat<Norme> {
        if let Some(n) = Norme::livree(version)? {
            return Ok(n);
        }
        Err(socle::echec::avec(
            format!("Version de la norme DSN « {} » inconnue du logiciel (versions livrées : {}). Rien n'est écrit.", version, liste_des_versions()?),
            APart { fichier: Some(BASE.to_string()), ..APart::default() },
        ))
    }

    /// La version nommee si le logiciel la livre, `None` sinon (une DSN d'avant 2026).
    pub fn livree(version: &str) -> Resultat<Option<Norme>> {
        Ok(base()?.versions.iter().any(|(k, _)| k == version).then(|| Norme { version: version.to_string() }))
    }

    /// Ce que l'entree de cette version dans la base partagee rend (`lire`) : rien n'est recopie.
    fn entree<R>(&self, lire: impl FnOnce(&Version) -> R) -> Resultat<R> {
        let b = base()?;
        match b.versions.iter().find(|(k, _)| *k == self.version) {
            Some((_, v)) => Ok(lire(v)),
            None => Err(socle::echec::livree(BASE, &format!("versions.{}", self.version)).into()),
        }
    }

    /// La table des usages de cette version (un fichier de `logiciel/donnees/`).
    pub fn table_usages(&self) -> Resultat<String> {
        self.entree(|e| e.usages.clone())
    }

    /// La table des formats de cette version.
    pub fn table_formats(&self) -> Resultat<String> {
        self.entree(|e| e.formats.clone())
    }

    /// La rubrique est-elle admise (O ou C) pour cette nature dans le tableau des usages de CETTE version ? C'est la
    /// question que les generateurs posent avant d'ecrire une rubrique qu'une version a retiree (06.003, 11.002 en
    /// P27V01) ou ajoutee.
    pub fn admet(&self, rubrique: &str, nature: &str) -> Resultat<bool> {
        Ok(matches!(crate::usages::usage(self, rubrique, nature)?, Some(crate::usages::Usage::O | crate::usages::Usage::C)))
    }

    /// Pourquoi cette valeur n'est plus dans la liste de la rubrique pour cette version (le texte de la donnee), ou
    /// `None`.
    pub fn valeur_retiree(&self, rubrique: &str, valeur: &str) -> Resultat<Option<String>> {
        self.entree(|e| e.valeurs_retirees.get(rubrique).and_then(|r| r.get(valeur)).cloned())
    }

    /// Pourquoi cette rubrique ne se renseigne pas dans cette version alors que son tableau des usages l'admet (le texte
    /// de la donnee, `rubriques_a_ne_pas_renseigner`), ou `None`.
    pub fn rubrique_a_ne_pas_renseigner(&self, rubrique: &str) -> Resultat<Option<String>> {
        self.entree(|e| e.rubriques_a_ne_pas_renseigner.as_ref().and_then(|r| r.0.get(rubrique)).cloned())
    }

    /// Le texte du cahier qui porte la regle d'ecriture `nom` (`regles` de la donnee) si cette version l'applique, `None`
    /// sinon : ce que la version ajoute a la precedente et qui ne se lit pas dans ses tables d'usages et de formats (une
    /// valeur a ne pas depasser, une datation, la definition d'un bloc).
    pub fn regle(&self, nom: &str) -> Resultat<Option<String>> {
        self.entree(|e| e.regles.get(nom).cloned())
    }

    /// Le code de cotisation (S21.G00.81.001) sous lequel la CET patronale se declare dans cette version ; `Err` avec
    /// la raison si la donnee ne le dit pas.
    pub fn cotisation_cet_patronale(&self) -> Resultat<String> {
        self.entree(|e| match &e.cotisation_cet_patronale {
            Some(code) => Ok(code.clone()),
            None => Err(socle::echec::avec(
                format!(
                    "Contribution d'equilibre technique (CET) patronale dans un fichier en norme {} : le code de cotisation \
(S21.G00.81.001) sous lequel elle se declare n'est pas tranche. {} Rien n'est ecrit.",
                    self.version, e.note
                ),
                APart { fichier: Some(BASE.to_string()), cle: Some("cotisation_cet_patronale".to_string()), ..APart::default() },
            )),
        })?
    }
}

/// La plage d'une version que `quelle` choisit (la mensuelle, les signalements).
type QuellePlage = fn(&Version) -> &Plage;

fn choisir(quelle: QuellePlage, valeur: &str) -> Resultat<Option<Norme>> {
    let b = base()?;
    let siennes: Vec<&String> = b.versions.iter().filter(|(_, e)| quelle(e).couvre(valeur)).map(|(k, _)| k).collect();
    match siennes.as_slice() {
        [] => Ok(None),
        [k] => Ok(Some(Norme { version: (*k).clone() })),
        _ => Err(format!(
            "{} : {} versions couvrent {} ({}) -- la donnee se contredit, rien n'est ecrit.",
            BASE,
            siennes.len(),
            valeur,
            siennes.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
        )),
    }
}

fn plages(quelle: QuellePlage) -> Resultat<String> {
    Ok(base()?.versions.iter().map(|(k, e)| format!("{} {}", k, quelle(e).dite())).collect::<Vec<_>>().join(" ; "))
}

/// La version d'une DSN mensuelle, d'apres son mois principal declare (`AAAA-MM`).
pub fn de_la_mensuelle(mois: &str) -> Resultat<Norme> {
    match choisir(|e| &e.mensuelle, mois)? {
        Some(n) => Ok(n),
        None => Err(socle::echec::avec(
            format!(
                "DSN de {} : aucune version de la norme DSN livree avec le logiciel ne couvre ce mois principal declare. \
La norme d'une annee se livre avec une mise a jour du logiciel. Rien n'est ecrit.",
                mois
            ),
            APart { fichier: Some(BASE.to_string()), detail: Some(plages(|e| &e.mensuelle)?), ..APart::default() },
        )),
    }
}

/// La version d'un signalement (arret, reprise, fin de contrat unique, amorcage), d'apres le jour ou il est constitue
/// (S20.G00.05.007) : un signalement n'a pas de mois principal declare.
pub fn du_signalement(constitution: &Jour) -> Resultat<Norme> {
    match choisir(|e| &e.signalements, &constitution.iso())? {
        Some(n) => Ok(n),
        None => Err(socle::echec::avec(
            format!(
                "Signalement constitue le {} : aucune version de la norme DSN livree avec le logiciel ne couvre ce jour. \
La norme d'une annee se livre avec une mise a jour du logiciel. Rien n'est ecrit.",
                constitution.jj_mm_aaaa()
            ),
            APart { fichier: Some(BASE.to_string()), detail: Some(plages(|e| &e.signalements)?), ..APart::default() },
        )),
    }
}

/// La version que le fichier porte lui-meme (S10.G00.00.006) ; refus si elle manque ou si le logiciel ne la connait pas
/// -- ses tables d'usages et de formats sont celles d'une version.
pub fn du_fichier(texte: &str) -> Resultat<Norme> {
    for ligne in lignes(texte) {
        let (r, v) = rubrique_valeur(ligne);
        if r == RUBRIQUE {
            return Norme::connue(v).map_err(|e| {
                format!("{} = '{}' : le fichier ne se controle pas, ses tables d'usages et de formats sont celles d'une version. {}", RUBRIQUE, v, e)
            });
        }
    }
    Err(format!(
        "Le fichier ne porte pas {} (numero de version de la norme) : ses tables d'usages et de formats ne se choisissent \
pas. Rien n'est ecrit.",
        RUBRIQUE
    ))
}

/// Les valeurs que la version du fichier n'admet plus dans leur liste (`valeurs_retirees`), et les rubriques que son
/// tableau des usages admet mais que sa description dit « a ne pas renseigner » (`rubriques_a_ne_pas_renseigner`) :
/// refus, avec le texte de la donnee.
pub fn verifier_les_valeurs(texte: &str, norme: &Norme) -> Resultat<()> {
    let mut refus = Vec::new();
    let mut a_ne_pas = Vec::new();
    for ligne in lignes(texte) {
        let (r, v) = rubrique_valeur(ligne);
        if let Some(pourquoi) = norme.valeur_retiree(r, v)? {
            let dit = format!("    {} = '{}' : {}", r, v, pourquoi);
            if !refus.contains(&dit) {
                refus.push(dit);
            }
        }
        if let Some(pourquoi) = norme.rubrique_a_ne_pas_renseigner(r)? {
            let dit = format!("    {} : {}", r, pourquoi);
            if !a_ne_pas.contains(&dit) {
                a_ne_pas.push(dit);
            }
        }
    }
    if !a_ne_pas.is_empty() {
        return Err(socle::echec::avec(
            format!("Rubrique(s) que la norme {} dit « à ne pas renseigner » :\n{}\n  Rien n'est écrit.", norme.version, a_ne_pas.join("\n")),
            APart { fichier: Some(BASE.to_string()), cle: Some("rubriques_a_ne_pas_renseigner".to_string()), ..APart::default() },
        ));
    }
    if refus.is_empty() {
        return Ok(());
    }
    Err(socle::echec::avec(
        format!(
            "Valeur(s) que la norme {} n'admet plus dans leur liste -- la plate-forme refuserait le fichier :\n{}\n  Rien n'est écrit.",
            norme.version,
            refus.join("\n")
        ),
        APart { fichier: Some(BASE.to_string()), cle: Some("valeurs_retirees".to_string()), ..APart::default() },
    ))
}
