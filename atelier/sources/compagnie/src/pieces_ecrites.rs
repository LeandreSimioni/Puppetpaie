//! LE JOURNAL DES PIECES ECRITES (`journal/dsn-ecrites.json`) : quand le moteur a ecrit chaque piece, ce qu'elle
//! declare, et le refus de recrire une piece partie.

use socle::echec::Classe;
use std::path::Path;

use socle::echec::erreur;
use socle::echec::Echec;
use socle::echec::R;
use socle::ecriture::ecrire_json;

fn os(chemin: &Path, e: std::io::Error) -> Echec {
    erreur(Classe::Disque, socle::echec::du_disque(chemin, &e))
}

/// Le journal des pieces ecrites, sous la racine de la compagnie.
const DSN_ECRITES: &str = "journal/dsn-ecrites.json";

/// LE JOURNAL DES PIECES ECRITES (`journal/dsn-ecrites.json`) : chaque piece que le moteur ecrit -- une DSN, une
/// DPAE, un export compta -- y note QUAND, au temps du moteur (`temps.rs`, par `socle::paris::maintenant`). C'est ce que
/// l'ecran dit (« ecrit le ... ») : la date d'un fichier n'est pas celle de son ecriture (une fictive livree est
/// decompressee chez le visiteur, un banc simule le temps). `cle` : `<mois>/<fichier>`, ou `exercices/<fichier>` ;
/// `empreinte_saisie` : la paie que la piece declare -- pour une DSN, celle de ses paies validees ; pour l'export compta
/// de l'annee, celles de ses saisies (`exercice_ecrit::empreintes_des_saisies`) ; `versions_portees` : pour une DSN
/// mensuelle, la version de chaque engagement d'un mois passe dont elle porte la correction
/// (`produire_regularisations::versions_portees`), que la ligne du journal de son depot emporte (`versions_portees_de`).
pub fn noter_piece_ecrite(racine: &Path, cle: &str, empreinte_saisie: Option<EmpreinteNotee>, versions_portees: Option<Vec<envois::journal::Version>>) -> R<()> {
    let p = racine.join(DSN_ECRITES);
    let mut d = journal_des_ecritures(racine)?;
    d.insert(cle.to_string(), PieceNotee { quand: socle::paris::maintenant(), empreinte_saisie, versions_portees });
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| os(parent, e))?;
    }
    ecrire_json(&p, &d, 2).map_err(|m| erreur(Classe::Disque, m))
}

/// UNE PIECE PARTIE NE SE RECRIT JAMAIS (regle 4 de `atelier/AGENTS.md` : ce qui est depose ne se regenere pas) : le
/// fichier `cible` dont le contenu est celui d'un depot reel accepte, qu'aucun retour n'a refuse (`journal/depots.jsonl`,
/// par son empreinte), ne s'ecrit pas par-dessus ; `Some(le jour de son depot)`. Chaque ecriture d'une declaration sous
/// un nom qui ne depend pas de son contenu (un signalement, la DPAE du mois, la DSN mensuelle) le lit avant d'ecrire.
pub fn partie_le(racine: &Path, cible: &Path) -> R<Option<String>> {
    if !cible.is_file() {
        return Ok(None);
    }
    let empreinte = envois::journal::empreinte_fichier(cible)?;
    let lignes = envois::journal::lu(racine)?.lignes;
    Ok(lignes
        .iter()
        .filter(|l| l.reel() && l.accepte() && l.empreinte() == Some(empreinte.as_str()))
        .find(|l| !envois::etats::refuse_par_un_retour(l))
        .map(|l| l.quand().chars().take(10).collect()))
}

/// `partie_le`, en refus : `cible` est parti, rien n'est ecrit.
pub fn refuser_si_partie(racine: &Path, cible: &Path) -> R<()> {
    match partie_le(racine, cible)? {
        None => Ok(()),
        Some(le) => Err(erreur(
            Classe::Refuse,
            format!(
                "{} est parti le {} : ce qui est parti ne se réécrit pas. Rien n'est écrit.",
                socle::fichiers::nom_du(cible),
                match socle::date::Jour::lire(&le) {
                    Ok(j) => j.jj_mm_aaaa(),
                    Err(e) => return Err(Echec { classe: Some(Classe::Illisible), ..e }),
                }
            ),
        )),
    }
}

/// La cle d'une piece au journal des ecritures : `<son dossier>/<son nom>` (`2026-11/DPAE_Novembre2026.txt`,
/// `exercices/Compta_2026.csv`) : les deux dernieres parties de son chemin.
pub fn cle_de_piece(fichier: &Path) -> String {
    let mut parties: Vec<String> = fichier.components().rev().take(2).map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    parties.reverse();
    parties.join("/")
}

/// Le journal des pieces ecrites : chaque piece sous sa cle `<mois>/<fichier>` (`PieceNotee`). Un journal absent est
/// vide ; un journal qui ne se lit pas se refuse.
pub fn journal_des_ecritures(racine: &Path) -> R<std::collections::BTreeMap<String, PieceNotee>> {
    let p = racine.join(DSN_ECRITES);
    if !p.exists() {
        return Ok(std::collections::BTreeMap::new());
    }
    socle::forme::lire_fichier(&p, "Le journal des pièces écrites ne se lit pas.").map_err(|e| Echec { classe: Some(Classe::Illisible), ..e })
}

/// QUAND LE MOTEUR A ECRIT CETTE PIECE (`AAAA-MM-JJTHH:MM`), lu au journal des ecritures (`noter_piece_ecrite`) ; `None`
/// s'il ne le dit pas. Jamais la date du fichier.
pub fn ecrit_le(racine: &Path, fichier: &Path) -> R<Option<String>> {
    Ok(piece_notee(racine, fichier)?.map(|n| n.quand.chars().take(16).collect()))
}

/// UNE PIECE NOTEE AU JOURNAL DES ECRITURES, telle que `noter_piece_ecrite` l'ecrit : quand, la paie qu'elle declare, et
/// ce qu'une DSN mensuelle corrige.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PieceNotee {
    pub quand: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub empreinte_saisie: Option<EmpreinteNotee>,
    #[serde(skip_serializing_if = "Option::is_none")]
    versions_portees: Option<Vec<envois::journal::Version>>,
}

/// La paie qu'une piece declare : celle d'une DSN (l'empreinte de ses paies validees), ou celles des saisies d'une annee
/// pour l'export compta (`exercice_ecrit::empreintes_des_saisies`).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum EmpreinteNotee {
    Saisie(String),
    Annee(std::collections::BTreeMap<String, String>),
}

/// La note de la piece `fichier` au journal des ecritures (`PieceNotee`) ; aucune si le moteur ne l'a pas ecrite.
pub fn piece_notee(racine: &Path, fichier: &Path) -> R<Option<PieceNotee>> {
    Ok(journal_des_ecritures(racine)?.remove(&cle_de_piece(fichier)))
}

/// CE QUE CETTE DSN MENSUELLE CORRIGE, tel que sa fabrication l'a note (`noter_piece_ecrite`) ; `None` si le moteur ne l'a
/// pas ecrite.
pub fn versions_portees_de(racine: &Path, fichier: &Path) -> R<Option<Vec<envois::journal::Version>>> {
    Ok(piece_notee(racine, fichier)?.and_then(|n| n.versions_portees))
}
