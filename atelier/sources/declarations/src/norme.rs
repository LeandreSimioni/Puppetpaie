//! La version de la norme DSN (NEODeS) qu'un fichier porte, `S10.G00.00.006`.
//!
//! Elle ne vient plus d'une constante : `logiciel/donnees/norme-dsn-base.json` dit, par version, ce qui la declenche
//! (le mois principal declare d'une DSN mensuelle, le jour de constitution d'un signalement), ses tables d'usages et de
//! formats (dont les listes de valeurs qu'elle admet) et le code de la CET patronale. Le moteur applique la donnee ;
//! ce qu'elle ne couvre pas refuse. LA REGLE DE BASCULE EST UNE INFERENCE (la `note` de la donnee) : le cahier
//! technique 2027.1.1 ne la donne pas (p. 129, une seule valeur par cahier ).
//!
//! Le fichier en cours de fabrication porte sa norme (`dsn::Fichier::en_norme`) : « ce fichier est-il en P27V01 ? »
//! se demande a lui, par la table de sa version (`Norme::admet`), en un seul endroit.

use socle::date::Jour;
use socle::echec::{refus_avec, Lieu, R};
use socle::mois::Mois;
use crate::usages::{lignes, rubrique_valeur};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::Arc;

const BASE: &str = "norme-dsn-base.json";
pub const RUBRIQUE: &str = "S10.G00.00.006";

/// UNE DONNEE LIVREE (`logiciel/donnees/<nom>`) EN SA FORME `T`, lue une fois par contenu (`socle::forme::partagee`) et
/// notee au registre d'un calcul suivi : absente ou fausse, rien que la compagnie puisse corriger.
pub(crate) fn livree<T: serde::de::DeserializeOwned + Send + Sync + 'static>(nom: &str) -> R<Arc<T>> {
    let chemin = socle::fichiers::dossier_du_code().join(nom);
    let lue = socle::forme::partagee::<T>(&chemin, "Une donnée livrée avec le logiciel ne se lit pas : le signaler au développeur.")?;
    socle::fichiers::noter_si_besoin(&chemin);
    lue.ok_or_else(|| refus_avec("Une donnée livrée avec le logiciel manque : le signaler au développeur.", Lieu { fichier: Some(nom.to_string()), ..Lieu::default() }))
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
    /// Les mois principaux declares d'une DSN mensuelle en cette version.
    mensuelle: Plage<Mois>,
    /// Les jours de constitution d'un signalement en cette version.
    signalements: Plage<Jour>,
    /// La table des usages (un fichier de `logiciel/donnees/`).
    usages: String,
    /// La table des formats.
    formats: String,
    /// Les rubriques que le tableau des usages admet et que la description dit « a ne pas renseigner ».
    rubriques_a_ne_pas_renseigner: Option<ANePasRenseigner>,
    /// Les regles d'ecriture que la version ajoute a la precedente : le nom de la regle, le texte qui la porte.
    regles: BTreeMap<String, String>,
    /// Le code de cotisation (S21.G00.81.001) de la CET patronale.
    cotisation_cet_patronale: Option<String>,
    note: String,
}

/// Une plage de mois ou de jours, bornes ouvertes si absentes.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Plage<T> {
    du: Option<T>,
    au: Option<T>,
    #[allow(dead_code)]
    note: String,
}

/// Une borne d'une plage, telle que la donnee l'ecrit (`AAAA-MM`, `AAAA-MM-JJ`).
trait Borne: PartialOrd {
    fn ecrite(&self) -> String;
}

impl Borne for Mois {
    fn ecrite(&self) -> String {
        self.to_string()
    }
}

impl Borne for Jour {
    fn ecrite(&self) -> String {
        self.iso()
    }
}

impl<T: Borne> Plage<T> {
    fn couvre(&self, v: &T) -> bool {
        self.du.as_ref().is_none_or(|d| d <= v) && self.au.as_ref().is_none_or(|a| v <= a)
    }

    /// « du 2027-01 au 2027-12 », « ... » pour une borne ouverte.
    fn dite(&self) -> String {
        let borne = |b: &Option<T>| match b {
            Some(b) => b.ecrite(),
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

fn base() -> R<Arc<Base>> {
    livree(BASE)
}

/// Les versions livrees, dans l'ordre de la donnee.
pub fn versions() -> R<Vec<Norme>> {
    Ok(base()?.versions.iter().map(|(k, _)| Norme { version: k.clone() }).collect())
}

fn liste_des_versions() -> R<String> {
    Ok(versions()?.iter().map(|n| n.version.clone()).collect::<Vec<_>>().join(", "))
}

impl Norme {
    /// La version nommee, si la donnee la connait.
    pub fn connue(version: &str) -> R<Norme> {
        if let Some(n) = Norme::livree(version)? {
            return Ok(n);
        }
        Err(refus_avec(
            format!("Version de la norme DSN « {} » inconnue du logiciel (versions livrées : {}). Rien n'est écrit.", version, liste_des_versions()?),
            Lieu { fichier: Some(BASE.to_string()), ..Lieu::default() },
        ))
    }

    /// La version nommee si le logiciel la livre, `None` sinon (une DSN d'avant 2026).
    pub fn livree(version: &str) -> R<Option<Norme>> {
        Ok(base()?.versions.iter().any(|(k, _)| k == version).then(|| Norme { version: version.to_string() }))
    }

    /// Ce que l'entree de cette version dans la base partagee rend (`lire`) : rien n'est recopie.
    fn entree<T>(&self, lire: impl FnOnce(&Version) -> T) -> R<T> {
        let b = base()?;
        match b.versions.iter().find(|(k, _)| *k == self.version) {
            Some((_, v)) => Ok(lire(v)),
            None => Err(socle::echec::livree(BASE, &format!("versions.{}", self.version))),
        }
    }

    /// Le rang de la version `version` dans la donnee : refus si elle n'y est pas (une borne d'une liste de la norme qui
    /// nomme une version que le logiciel ne livre pas).
    fn rang(version: &str) -> R<usize> {
        base()?.versions.iter().position(|(k, _)| k == version).ok_or_else(|| socle::echec::livree(BASE, &format!("versions.{}", version)))
    }

    /// La liste de la norme que `v` remplit a-t-elle son code dans CETTE version ? Le code dit ses bornes
    /// (`droit::natures::ListeDeLaNorme::depuis`, `jusqu_a`), comparees dans l'ordre des versions de la donnee.
    pub fn a_dans_sa_liste<T: droit::natures::ListeDeLaNorme>(&self, v: T) -> R<bool> {
        let ici = Norme::rang(&self.version)?;
        // Une borne que le code ne dit pas ne le borne pas.
        if let Some(d) = v.depuis() {
            if Norme::rang(d)? > ici {
                return Ok(false);
            }
        }
        if let Some(j) = v.jusqu_a() {
            if ici > Norme::rang(j)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// La table des usages de cette version (un fichier de `logiciel/donnees/`).
    pub fn table_usages(&self) -> R<String> {
        self.entree(|e| e.usages.clone())
    }

    /// La table des formats de cette version.
    pub fn table_formats(&self) -> R<String> {
        self.entree(|e| e.formats.clone())
    }

    /// La rubrique est-elle admise (O ou C) pour cette nature dans le tableau des usages de CETTE version ? C'est la
    /// question que les generateurs posent avant d'ecrire une rubrique qu'une version a retiree (06.003, 11.002 en
    /// P27V01) ou ajoutee.
    pub fn admet(&self, rubrique: &str, nature: droit::nature_declaration::NatureDeLaDeclaration) -> R<bool> {
        Ok(matches!(crate::usages::usage(self, rubrique, nature)?, Some(crate::usages::Usage::O | crate::usages::Usage::C)))
    }

    /// Pourquoi cette rubrique ne se renseigne pas dans cette version alors que son tableau des usages l'admet (le texte
    /// de la donnee, `rubriques_a_ne_pas_renseigner`), ou `None`.
    pub fn rubrique_a_ne_pas_renseigner(&self, rubrique: &str) -> R<Option<String>> {
        self.entree(|e| e.rubriques_a_ne_pas_renseigner.as_ref().and_then(|r| r.0.get(rubrique)).cloned())
    }

    /// Le texte du cahier qui porte la regle d'ecriture `nom` (`regles` de la donnee) si cette version l'applique, `None`
    /// sinon : ce que la version ajoute a la precedente et qui ne se lit pas dans ses tables d'usages et de formats (une
    /// valeur a ne pas depasser, une datation, la definition d'un bloc).
    pub fn regle(&self, nom: &str) -> R<Option<String>> {
        self.entree(|e| e.regles.get(nom).cloned())
    }

    /// Le code de cotisation (S21.G00.81.001) sous lequel la CET patronale se declare dans cette version ; `Err` avec
    /// la raison si la donnee ne le dit pas.
    pub fn cotisation_cet_patronale(&self) -> R<String> {
        self.entree(|e| match &e.cotisation_cet_patronale {
            Some(code) => Ok(code.clone()),
            None => Err(refus_avec(
                format!(
                    "Contribution d'equilibre technique (CET) patronale dans un fichier en norme {} : le code de cotisation \
(S21.G00.81.001) sous lequel elle se declare n'est pas tranche. {} Rien n'est ecrit.",
                    self.version, e.note
                ),
                Lieu { fichier: Some(BASE.to_string()), cle: Some("cotisation_cet_patronale".to_string()), ..Lieu::default() },
            )),
        })?
    }
}

/// La plage d'une version que `quelle` choisit (la mensuelle, les signalements).
type QuellePlage<T> = fn(&Version) -> &Plage<T>;

fn choisir<T: Borne>(quelle: QuellePlage<T>, valeur: &T) -> R<Option<Norme>> {
    let b = base()?;
    let siennes: Vec<&String> = b.versions.iter().filter(|(_, e)| quelle(e).couvre(valeur)).map(|(k, _)| k).collect();
    match siennes.as_slice() {
        [] => Ok(None),
        [k] => Ok(Some(Norme { version: (*k).clone() })),
        _ => Err(socle::refus!(
            "{} : {} versions couvrent {} ({}) -- la donnee se contredit, rien n'est ecrit.",
            BASE,
            siennes.len(),
            valeur.ecrite(),
            siennes.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
        )),
    }
}

fn plages<T: Borne>(quelle: QuellePlage<T>) -> R<String> {
    Ok(base()?.versions.iter().map(|(k, e)| format!("{} {}", k, quelle(e).dite())).collect::<Vec<_>>().join(" ; "))
}

/// La version d'une DSN mensuelle, d'apres son mois principal declare.
pub fn de_la_mensuelle(mois: Mois) -> R<Norme> {
    match choisir(|e| &e.mensuelle, &mois)? {
        Some(n) => Ok(n),
        None => Err(refus_avec(
            format!(
                "DSN de {} : aucune version de la norme DSN livree avec le logiciel ne couvre ce mois principal declare. \
La norme d'une annee se livre avec une mise a jour du logiciel. Rien n'est ecrit.",
                mois
            ),
            Lieu { fichier: Some(BASE.to_string()), detail: Some(plages(|e| &e.mensuelle)?), ..Lieu::default() },
        )),
    }
}

/// La version d'un signalement (arret, reprise, fin de contrat unique, amorcage), d'apres le jour ou il est constitue
/// (S20.G00.05.007) : un signalement n'a pas de mois principal declare.
pub fn du_signalement(constitution: &Jour) -> R<Norme> {
    match choisir(|e| &e.signalements, constitution)? {
        Some(n) => Ok(n),
        None => Err(refus_avec(
            format!(
                "Signalement constitue le {} : aucune version de la norme DSN livree avec le logiciel ne couvre ce jour. \
La norme d'une annee se livre avec une mise a jour du logiciel. Rien n'est ecrit.",
                constitution.jj_mm_aaaa()
            ),
            Lieu { fichier: Some(BASE.to_string()), detail: Some(plages(|e| &e.signalements)?), ..Lieu::default() },
        )),
    }
}

/// La version que le fichier porte lui-meme (S10.G00.00.006) ; refus si elle manque ou si le logiciel ne la connait pas
/// -- ses tables d'usages et de formats sont celles d'une version.
pub fn du_fichier(texte: &str) -> R<Norme> {
    for ligne in lignes(texte) {
        let (r, v) = rubrique_valeur(ligne);
        if r == RUBRIQUE {
            return Norme::connue(v).map_err(|e| {
                e.reformuler(|e| format!("{} = '{}' : le fichier ne se controle pas, ses tables d'usages et de formats sont celles d'une version. {}", RUBRIQUE, v, e))
            });
        }
    }
    Err(socle::refus!(
        "Le fichier ne porte pas {} (numero de version de la norme) : ses tables d'usages et de formats ne se choisissent \
pas. Rien n'est ecrit.",
        RUBRIQUE
    ))
}

/// Pourquoi la valeur `v` de la rubrique `r` n'est pas dans la liste que la table des formats de cette version releve
/// pour elle (`valeurs` de `formats-dsn-*-base.json`), avec le libelle qu'elle a dans les autres versions livrees ;
/// `None` si elle y est, ou si la rubrique n'a pas de liste dans cette version. « Une rubrique assortie d'une liste de
/// valeurs ne peut contenir qu'une des valeurs indiquees dans cette liste » (cahier 2027.1.1, p. 86,
/// `atelier/reference/norme-dsn/texte-2027/chapitre-4-controles.txt`, l. 164).
fn hors_de_sa_liste(tables: &[(String, Arc<crate::formats::Table>)], r: &str, v: &str) -> Option<String> {
    let [(version, ici), autres @ ..] = tables else { return None };
    let liste = ici.formats.get(r)?.valeurs.as_ref()?;
    if liste.contains_key(v) {
        return None;
    }
    let ailleurs: Vec<String> = autres
        .iter()
        .filter_map(|(autre, t)| t.formats.get(r)?.valeurs.as_ref()?.get(v).map(|l| format!("{} : « {} - {} »", autre, v, l)))
        .collect();
    let ailleurs = if ailleurs.is_empty() { String::new() } else { format!(" (la liste de {})", ailleurs.join(" ; ")) };
    Some(format!("la liste de la rubrique relevée pour {} ne l'a pas{}", version, ailleurs))
}

/// Les valeurs que la liste de leur rubrique, dans la version du fichier, n'a pas (`hors_de_sa_liste`), et les
/// rubriques que son tableau des usages admet mais que sa description dit « a ne pas renseigner »
/// (`rubriques_a_ne_pas_renseigner`) : refus, avec le texte de la donnee.
pub fn verifier_les_valeurs(texte: &str, norme: &Norme) -> R<()> {
    let mut tables = vec![(norme.version.clone(), crate::formats::table_de(norme)?)];
    for autre in versions()?.into_iter().filter(|n| n != norme) {
        tables.push((autre.version.clone(), crate::formats::table_de(&autre)?));
    }
    let mut refus = Vec::new();
    let mut a_ne_pas = Vec::new();
    for ligne in lignes(texte) {
        let (r, v) = rubrique_valeur(ligne);
        if let Some(pourquoi) = hors_de_sa_liste(&tables, r, v) {
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
        return Err(refus_avec(
            format!("Rubrique(s) que la norme {} dit « à ne pas renseigner » :\n{}\n  Rien n'est écrit.", norme.version, a_ne_pas.join("\n")),
            Lieu { fichier: Some(BASE.to_string()), cle: Some("rubriques_a_ne_pas_renseigner".to_string()), ..Lieu::default() },
        ));
    }
    if refus.is_empty() {
        return Ok(());
    }
    Err(refus_avec(
        format!(
            "Valeur(s) que la norme {} n'admet plus dans leur liste -- la plate-forme refuserait le fichier :\n{}\n  Rien n'est écrit.",
            norme.version,
            refus.join("\n")
        ),
        Lieu { fichier: Some(norme.table_formats()?), cle: Some("valeurs".to_string()), ..Lieu::default() },
    ))
}
