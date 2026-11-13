//! Le numero technique temporaire : declarer quelqu'un qui n'a pas de NIR.
//!
//! Il se fabrique (sexe + SIREN + matricule), il ne se saisit pas. Les
//! regles de forme CSL-11, CSL-12, CCH-13 et la longueur [11,40] sont ici.

use crate::dsn::Fichier;
use droit::fiche::Lue as _;
use socle::fichiers::repr;
use crate::formats;
use socle::echec::R;
use crate::usages::strip;
use droit::fiche::{ChampsSalarie, FicheSalarie, Nir, Sexe};
use droit::natures::RubriqueDeLaNorme;

pub const RUBRIQUE: RubriqueDeLaNorme = RubriqueDeLaNorme::du_moteur("S21.G00.30.020");
pub const RUBRIQUE_NIR: RubriqueDeLaNorme = RubriqueDeLaNorme::du_moteur("S21.G00.30.001");

/// UN NUMERO TECHNIQUE TEMPORAIRE (S21.G00.30.020) : l'identifiant d'un individu declare sans NIR, « X [11,40] », le
/// code sexe (CSL-11), sans espace (CSL-12), le SIREN de l'entreprise en 2e a 10e position (CCH-13)
/// (`atelier/reference/norme-dsn/texte/bloc-S21.G00.30.txt` l. 338-354). Il se fabrique pour un salarie (`numero`, qui
/// le controle entier) ou se lit dans une DSN (`Ntt::lu`, sa forme seule : le SIREN de l'entreprise n'y est pas en main).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
pub struct Ntt(String);

/// CE QUI IDENTIFIE UN INDIVIDU DANS UNE DSN : son NIR (S21.G00.30.001), ou, sans NIR, son NTT (S21.G00.30.020 :
/// « En l'absence de NIR ou de NIA, il est obligatoire de remplir la rubrique », `norme-dsn/texte/bloc-S21.G00.30.txt`,
/// l. 348-354). Il ne s'ecrit dans aucun fichier ; il se trie NIR d'abord, chacun comme son texte.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IdentifiantDIndividu {
    Nir(Nir),
    Ntt(Ntt),
}

impl IdentifiantDIndividu {
    /// Ce qu'il est, pour une phrase : « NIR » ou « NTT ».
    pub fn quoi(&self) -> &'static str {
        match self {
            IdentifiantDIndividu::Nir(_) => "NIR",
            IdentifiantDIndividu::Ntt(_) => "NTT",
        }
    }
}

impl std::fmt::Display for IdentifiantDIndividu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IdentifiantDIndividu::Nir(n) => n.fmt(f),
            IdentifiantDIndividu::Ntt(n) => n.fmt(f),
        }
    }
}

impl Ntt {
    /// Le NTT qu'une DSN lue ecrit, s'il en a la forme ; sinon aucun.
    pub fn lu(valeur: &str) -> R<Option<Ntt>> {
        Ok(faute(valeur, "")?.is_none().then(|| Ntt(valeur.to_string())))
    }

    /// Le NTT qu'un retour de la norme ecrit (RO.B009.R002 de NEOReS) : hors de sa forme, il se refuse.
    pub fn lire(valeur: &str) -> R<Ntt> {
        match faute(valeur, "")? {
            None => Ok(Ntt(valeur.to_string())),
            Some(f) => Err(socle::refus!("« {} » n'est pas un numéro technique temporaire : {}.", valeur, f)),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Ntt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Le code sexe d'une fiche EN COURS DE SAISIE : celui que dit son NIR (`du_nir` : `Sexe::du_nir` du texte saisi), sinon
/// la case `sexe` ; `None` si ni l'un ni l'autre ne le dit. Une fiche enregistree le dit par `ChampsSalarie::sexe`.
pub fn sexe_de(du_nir: Option<Sexe>, sexe: Option<&str>) -> Option<&'static str> {
    let un_ou_deux = |t: &str| match t {
        "1" => Some("1"),
        "2" => Some("2"),
        _ => None,
    };
    match du_nir {
        Some(s) => Some(s.as_str()),
        None => sexe.map(strip).and_then(un_ou_deux),
    }
}

/// Le NTT de ce salarie dans l'entreprise de SIREN `siren`.
pub fn numero(siren: &str, salarie: &FicheSalarie) -> R<Ntt> {
    let Some(s) = salarie.sexe().map(|s| s.as_str()) else {
        let qui = match salarie.nom.remplie() {
            Some(nom) => nom.to_string(),
            None => salarie.id.to_string(),
        };
        return Err(socle::refus!(
            "Le numero technique temporaire ({}) de {} ne peut pas etre fabrique : sa fiche ne dit pas le sexe, et le NTT commence par lui (1 ou 2). D'ordinaire il se lit sur le NIR ; sans NIR, il faut le renseigner dans la fiche du salarie.",
            RUBRIQUE, qui
        ));
    };
    let siren = strip(siren).to_string();
    if siren.chars().count() != 9 || !siren.chars().all(|c| c.is_ascii_digit()) {
        return Err(socle::echec::a_la_cle(
            &format!(
                "Le numero technique temporaire ({}) porte le SIREN de l'entreprise en 2e a 10e position (CCH-13), et celui de la fiche entreprise n'a pas neuf chiffres : le corriger dans Configuration › Entreprise.",
                RUBRIQUE
            ),
            "siren",
            repr(&siren),
        ));
    }
    let Some(matricule) = salarie.code_salarie.portee().map(strip) else {
        return Err(socle::refus!(
            "Le numero technique temporaire ({}) finit par l'identifiant perenne du salarie dans l'entreprise -- son matricule --, et la fiche n'en porte pas.",
            RUBRIQUE
        ));
    };
    let v = format!("{}{}{}", s, siren, matricule);
    if let Some(pourquoi) = faute(&v, &siren)? {
        return Err(socle::refus!(
            "Le numero technique temporaire fabrique pour cette fiche, {}, n'est pas declarable : {}.",
            repr(&v),
            pourquoi
        ));
    }
    Ok(Ntt(v))
}

/// Ce que la norme reproche a ce NTT -- None s'il passe.
pub fn faute(valeur: &str, siren: &str) -> R<Option<String>> {
    let c: Vec<char> = valeur.chars().collect();
    if c.is_empty() {
        return Ok(Some("il est vide".into()));
    }
    if c[0] != '1' && c[0] != '2' {
        return Ok(Some(format!(
            "il doit commencer par 1 ou 2, le code sexe de la personne (CSL-11) ; celui-ci commence par \u{ab} {} \u{bb}",
            c[0]
        )));
    }
    if let Some(i) = c.iter().position(|&x| x == ' ') {
        return Ok(Some(format!(
            "le caractere espace y est interdit (CSL-12) ; il porte un espace en position {}",
            i + 1
        )));
    }
    if !siren.is_empty() {
        let milieu: String = c.iter().skip(1).take(9).collect();
        if milieu != siren {
            return Ok(Some(format!(
                "ses caracteres 2 a 10 doivent etre le SIREN declare, {} (CCH-13) ; il porte {}",
                siren,
                repr(&milieu)
            )));
        }
    }
    formats::mauvaise(RUBRIQUE, valeur)
}

/// S21.G00.30.001, quand il y en a un -- et rien du tout sinon. La DSN ecrit le NIR de la fiche tel quel (sa saisie l'a
/// range, `Nir::rangee`).
pub fn poser_nir(f: &mut Fichier, salarie: &ChampsSalarie) -> R<()> {
    if let Some(nir) = salarie.nir.dite().cloned() {
        f.add(RUBRIQUE_NIR, nir);
    }
    Ok(())
}

/// S21.G00.30.020, apres le matricule, dans toute DSN (mensuelle, signalement, demande de taux). Sans NIR, le NTT du
/// salarie (« En l'absence de NIR ou de NIA, il est obligatoire de remplir la rubrique ») ; avec un NIR, le NTT sous lequel
/// la derniere DSN partie l'a declare quand elle ne portait pas de NIR, quel que soit le jour ou le NIR est arrive dans la
/// fiche (« Le NTT doit etre renseigne sur la premiere DSN ou le NIR est attribue. Ceci permettra de faire le lien entre
/// les deux identites declarees par l'employeur », et « le NTT le plus ancien en cours de validite » :
/// `norme-dsn/texte/bloc-S21.G00.30.txt`, l. 348-354) ; sinon rien. `declaree` : l'identite que la derniere DSN a
/// declaree pour lui (`changements_individu::Declarations::derniere`, ou `derniere_declaree`).
pub fn poser_ntt(f: &mut Fichier, ent: &droit::fiche::ChampsEntreprise, salarie: &FicheSalarie, declaree:Option<&crate::changements_individu::Declaree>) -> R<()> {
    if salarie.nir.dite().is_none() {
        f.add(RUBRIQUE, numero(ent.siren.ecrite("siren")?, salarie)?);
        return Ok(());
    }
    if let Some(n) = declaree.filter(|d| d.nir.is_none()).and_then(|d| d.ntt.as_ref()) {
        f.add(RUBRIQUE, n);
    }
    Ok(())
}

/// CCH-12 et CCH-14, verifies sur le fichier seul, relu (`dsn_lue::lire`).
pub fn verifier_fichier(texte: &str) -> R<()> {
    refuser_les_doublons(&crate::dsn_lue::lire(texte.as_bytes(), None)?.individus)
}

fn refuser_les_doublons(individus: &[crate::dsn_lue::IndividuDeclare]) -> R<()> {
    // Un NIR et un NTT ne sont jamais egaux : une seule liste.
    let mut vus: Vec<(IdentifiantDIndividu, &str)> = Vec::new();
    for i in individus {
        let nom = match i.nom.as_deref() {
            Some(n) => n,
            None => "Un individu sans nom (S21.G00.30.002)",
        };
        if i.nir.is_none() && i.ntt.is_none() {
            return Err(socle::refus!(
                "{} est declare sans numero d'inscription au repertoire et sans numero technique temporaire. La norme exige l'un ou l'autre (CCH-12, S21.G00.30.020) : sans cela l'individu n'est identifie par rien.",
                nom
            ));
        }
        for cle in [i.nir.clone().map(IdentifiantDIndividu::Nir), i.ntt.clone().map(IdentifiantDIndividu::Ntt)] {
            let Some(cle) = cle else { continue };
            if let Some((_, autre)) = vus.iter().find(|(c, _)| *c == cle) {
                return Err(socle::refus!(
                    "Le meme {} ({}) identifie deux individus de la meme declaration, {} et {}. La norme l'interdit (CCH-14) : un identifiant qui designe deux personnes rend la declaration intracable.",
                    cle.quoi(), cle, autre, nom
                ));
            }
            vus.push((cle, nom));
        }
    }
    Ok(())
}
