//! Le questionnaire de la fiche salarie remplie par la personne elle-meme, par courriel
//! Les `QUESTIONS`, `questionnaire`, `code_fiche`, `lire_reponses`, `fiche_depuis_reponses`.
//!
//! Les textes (questions, objet, corps, HTML) et les deux tables (pays, nationalites) sont ecrits mot pour mot du questionnaire. Les deux tables ne portent que le code
//! ISO : la codification UE (quatre valeurs) se deduit dans `codification_ue`. Les pays se
//! lisent dans la liste livree (`pays.rs`, COG de l'INSEE) ; seule la table des nationalites (adjectifs) reste ici. Ce que la personne a repondu se lit, et ce qu'on ne sait pas lire
//! laisse la case VIDE et le dit (`illisible`) : rien n'est complete au hasard. Le departement et le pays de
//! naissance se lisent dans le NIR.
//!
//! Lecture des chiffres : seuls se lisent
//! les chiffres ASCII, pour les dates ; la decomposition NFD des accents est celle de `contexte::sans_accent` (latin
//! courant).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use socle::texte::{blanc as espace, strip};
use socle::date::Jour;
use socle::echec::R;

pub const ALPHABET_CODE: &str = "ABCDEFGHJKMNPQRSTUVWXYZ23456789";

/// Un code a six signes derriere son prefixe (`SIG-` d'une signature, `FIC-` d'une fiche demandee a la personne).
/// Le systeme qui ne donne pas d'aleas fait refuser : un code devinable ne protege rien.
pub fn code_au_hasard(prefixe: &str) -> R<String> {
    let alphabet = ALPHABET_CODE.as_bytes();
    let limite = 256 - 256 % alphabet.len();
    let mut code = String::from(prefixe);
    // UN TIRAGE PAR CODE, jamais un par signe : un octet rejete ne decale pas la suite des aleas qui suivent (au rejeu,
    // `socle::temps::aleas` est une suite fixe, que chaque tirage fait avancer).
    while code.len() < prefixe.len() + 6 {
        let mut b = [0u8; 32];
        socle::temps::aleas(&mut b)?;
        for x in b.iter().filter(|x| (**x as usize) < limite).take(prefixe.len() + 6 - code.len()) {
            code.push(alphabet[*x as usize % alphabet.len()] as char);
        }
    }
    Ok(code)
}

/// `code_fiche` : `FIC-` puis six signes tires au hasard dans `ALPHABET_CODE`.
pub fn code_fiche() -> R<String> {
    code_au_hasard("FIC-")
}

/// Les questions que le questionnaire sait lire : (cle de la reponse, libelle, case de la fiche salarie qu'elle remplit).
/// Ne se posent que celles dont la case sert a un acte (`posees`) : la liste de `champs-base.json` (`exige_par`, `sinon`).
pub const QUESTIONS: [(&str, &str, &str); 15] = [
    ("prenom", "Prénom", "prenom_affichage"),
    ("nom", "Nom de famille (de naissance)", "nom_affichage"),
    ("naissance", "Date de naissance (JJ/MM/AAAA)", "date_naissance"),
    ("lieu", "Commune de naissance", "lieu_naissance_affichage"),
    ("pays", "Pays de naissance", "pays_naissance"),
    ("nationalite", "Nationalité", "code_nationalite_dsn"),
    ("nir", "Numéro de sécurité sociale (15 chiffres)", "nir"),
    ("sexe", "Sans numéro de sécurité sociale : femme ou homme ?", "sexe"),
    ("adresse", "Adresse (numéro et rue)", "adresse_affichage"),
    ("code_postal", "Code postal", "code_postal"),
    ("commune", "Commune", "commune_affichage"),
    ("conges", "Numéro Congés Spectacles", "num_conges_spectacles"),
    ("anniversaire", "Date anniversaire de vos droits France Travail (JJ/MM/AAAA)", "anniversaire_droits"),
    ("iban", "IBAN (le compte où verser le salaire, sur le RIB)", "banque"),
    ("bic", "BIC (sur le même RIB)", "banque"),
];

/// Les questions posees : celles dont la case sert a un acte (`utile`, que l'appelant lit dans `champs-base.json`).
pub fn posees(utile: &dyn Fn(&str) -> bool) -> Vec<(&'static str, &'static str)> {
    QUESTIONS.iter().filter(|(_, _, case)| utile(case)).map(|(cle, lib, _)| (*cle, *lib)).collect()
}

pub const SUJET: &str = "Ta fiche pour {compagnie}";

pub const CORPS: &str = "Bonjour{virgule_prenom},\n\nPour préparer ton contrat, {compagnie} a besoin de quelques informations sur toi.\n\nLe plus simple : appuie sur le bouton « Remplir ma fiche » de ce message. Une réponse s'ouvre avec les questions ; écris chaque réponse après les deux-points, puis envoie-la. Une question dont tu n'as pas la réponse reste vide.\n\nSi ta messagerie n'affiche pas le bouton, réponds simplement à ce message en recopiant les lignes ci-dessous, avec tes réponses :\n\n{questionnaire}\n\nCes informations servent à tes contrats, à tes bulletins et aux déclarations obligatoires (Urssaf, France Travail, Audiens). Elles restent chez {compagnie}.\n\nBien à toi,\n{signataire}\n{compagnie}\n";

pub const HTML: &str = "<div style=\"font-family:Arial,Helvetica,sans-serif;font-size:15px;line-height:1.5;color:#2C3E50\">\n<p>Bonjour{virgule_prenom},</p>\n<p>Pour préparer ton contrat, {compagnie} a besoin de quelques informations sur toi.</p>\n<p>Appuie sur le bouton : une réponse s'ouvre avec les questions. Écris chaque réponse après les deux-points, puis envoie-la. Une question dont tu n'as pas la réponse reste vide.</p>\n<p style=\"margin:26px 0\">\n<a href=\"{mailto}\" style=\"display:inline-block;background:#B22222;color:#ffffff;padding:14px 26px;border-radius:6px;text-decoration:none;font-weight:bold;font-size:16px\">Remplir ma fiche</a>\n</p>\n<p style=\"font-size:13px;color:#555\">Si le bouton ne fait rien, réponds simplement à ce message en recopiant ces lignes avec tes réponses :</p>\n<pre style=\"font-family:Arial,Helvetica,sans-serif;font-size:14px;background:#f4f4f4;padding:12px\">{questionnaire}</pre>\n<p>Ces informations servent à tes contrats, à tes bulletins et aux déclarations obligatoires (Urssaf, France Travail, Audiens). Elles restent chez {compagnie}.</p>\n<p>Bien à toi,<br>{signataire}<br>{compagnie}</p>\n</div>\n";

/// Les nationalites (adjectifs) : (mot sans accent, code ISO du pays). La fiche garde le pays (`pays_nationalite`) ;
/// la codification UE s'en deduit (`codification_ue::code_du_pays`).
const NATIONALITES: [(&str, &str); 60] = [
    ("francaise", "FR"),
    ("francais", "FR"),
    ("italienne", "IT"),
    ("italien", "IT"),
    ("espagnole", "ES"),
    ("espagnol", "ES"),
    ("belge", "BE"),
    ("allemande", "DE"),
    ("allemand", "DE"),
    ("portugaise", "PT"),
    ("portugais", "PT"),
    ("polonaise", "PL"),
    ("polonais", "PL"),
    ("roumaine", "RO"),
    ("roumain", "RO"),
    ("grecque", "GR"),
    ("grec", "GR"),
    ("neerlandaise", "NL"),
    ("neerlandais", "NL"),
    ("irlandaise", "IE"),
    ("irlandais", "IE"),
    ("luxembourgeoise", "LU"),
    ("luxembourgeois", "LU"),
    ("autrichienne", "AT"),
    ("autrichien", "AT"),
    ("suedoise", "SE"),
    ("suedois", "SE"),
    ("danoise", "DK"),
    ("danois", "DK"),
    ("finlandaise", "FI"),
    ("finlandais", "FI"),
    ("norvegienne", "NO"),
    ("norvegien", "NO"),
    ("islandaise", "IS"),
    ("islandais", "IS"),
    ("britannique", "GB"),
    ("anglaise", "GB"),
    ("anglais", "GB"),
    ("suisse", "CH"),
    ("americaine", "US"),
    ("americain", "US"),
    ("canadienne", "CA"),
    ("canadien", "CA"),
    ("algerienne", "DZ"),
    ("algerien", "DZ"),
    ("marocaine", "MA"),
    ("marocain", "MA"),
    ("tunisienne", "TN"),
    ("tunisien", "TN"),
    ("bresilienne", "BR"),
    ("bresilien", "BR"),
    ("argentine", "AR"),
    ("argentin", "AR"),
    ("chilienne", "CL"),
    ("chilien", "CL"),
    ("ukrainienne", "UA"),
    ("ukrainien", "UA"),
    ("russe", "RU"),
    ("liechtensteinoise", "LI"),
    ("liechtensteinois", "LI"),
];

/// Les nationalites d'un pays que la liste livree n'a pas (`pays-base.json`, COG de l'INSEE : le Kosovo n'y a pas de
/// code ISO) : (mot sans accent, nom du pays). Le nom va dans `pays_nationalite_hors_liste` ; la zone
/// (`code_nationalite_dsn`) ne s'en deduit pas, l'administrateur la choisit.
const NATIONALITES_HORS_LISTE: [(&str, &str); 3] = [("kosovare", "Kosovo"), ("kosovar", "Kosovo"), ("kosovo", "Kosovo")];

/// `questionnaire` : ce que le bouton prepare. Les questions `posees`, chacune suivie de sa reponse (vide pour la
/// demande ; la personne fictive y met la sienne), puis le code, en DERNIER, sur sa ligne ; la releve le cherche dans
/// ce que la personne a ecrit, avant la citation.
pub fn questionnaire(code: &str, posees: &[(&str, &str)], reponse: &dyn Fn(&str) -> String) -> String {
    let mut lignes: Vec<String> = posees.iter().map(|(cle, lib)| format!("{} : {}", lib, reponse(cle))).collect();
    lignes.push(String::new());
    lignes.push(format!("Code (ne pas modifier) : {}", code));
    lignes.join("\n")
}

// ---------------------------------------------------------------- la lecture

/// Sans accents, en minuscules, sans blancs aux bords.
fn sans_accents(t: &str) -> String {
    strip(&socle::texte::sans_accent(t).to_lowercase()).to_string()
}

/// Sans accents ni casse, les parentheses (des consignes) retirees, les mots de `a-z` seulement.
fn cle_libelle(texte: &str) -> String {
    let t: Vec<char> = socle::texte::sans_accent(texte).to_lowercase().chars().collect();
    // Chaque parenthèse, de l'ouvrante à la première fermante sans passer à la ligne, devient un espace.
    let mut sans = String::new();
    let mut i = 0;
    while i < t.len() {
        if t[i] == '(' {
            let fin = t[i + 1..].iter().position(|c| *c == ')' || *c == '\n');
            if let Some(f) = fin.filter(|f| t[i + 1 + f] == ')') {
                sans.push(' ');
                i += f + 2;
                continue;
            }
        }
        sans.push(t[i]);
        i += 1;
    }
    // `" ".join(re.findall(r"[a-z]+", t))`.
    let mut mots: Vec<String> = Vec::new();
    let mut mot = String::new();
    for c in sans.chars() {
        if c.is_ascii_lowercase() {
            mot.push(c);
        } else if !mot.is_empty() {
            mots.push(std::mem::take(&mut mot));
        }
    }
    if !mot.is_empty() {
        mots.push(mot);
    }
    mots.join(" ")
}

/// `lire_reponses` : `{cle: reponse brute}` pour chaque ligne « Libelle : reponse » reconnue.
///
/// Une ligne se reconnait si ce qui precede le premier deux-points COMMENCE comme un libelle -- le plus long qui
/// convienne gagne (« Commune de naissance » avant « Commune »). Une reponse vide n'est pas une reponse. La premiere
/// ligne reconnue d'une cle l'emporte.
pub fn lire_reponses(texte: &str) -> HashMap<&'static str, String> {
    let libelles: Vec<(String, &'static str)> = QUESTIONS.iter().map(|(cle, lib, _)| (cle_libelle(lib), *cle)).collect();
    let mut sortie: HashMap<&'static str, String> = HashMap::new();
    // Un libelle qui porte lui-meme un deux-points (« Sans numero de securite sociale : femme ou homme ? ») : la reponse
    // est derriere le dernier (« femme ou homme ? : homme » a pour reponse « homme », pas « femme »).
    let avec_deux_points: Vec<(usize, String, &'static str)> =
        QUESTIONS.iter().filter(|(_, lib, _)| lib.contains(':')).map(|(cle, lib, _)| (lib.matches(':').count(), cle_libelle(lib), *cle)).collect();
    for ligne in socle::texte::lignes_du_texte(texte) {
        let coupes: Vec<usize> = ligne.match_indices(':').map(|(i, _)| i).collect();
        if let Some((n, _, cle)) = avec_deux_points.iter().find(|(n, lib, _)| {
            coupes.get(*n).is_some_and(|i| cle_libelle(ligne[..*i].trim_start_matches(['*', '>', ' '])) == *lib)
        }) {
            let valeur = strip(strip(&ligne[coupes[*n] + 1..]).trim_matches('*'));
            if !valeur.is_empty() {
                sortie.entry(*cle).or_insert_with(|| valeur.to_string());
            }
            continue;
        }
        let Some((tete, valeur)) = ligne.split_once(':') else { continue };
        // Une messagerie peut mettre la ligne en citation ou en gras.
        let tete = cle_libelle(tete.trim_start_matches(['*', '>', ' ']));
        let valeur = strip(strip(valeur).trim_matches('*'));
        if tete.is_empty() || valeur.is_empty() {
            continue;
        }
        // Le libelle exact d'abord : « Commune » n'est pas le debut tronque de « Commune de naissance ».
        let exact = libelles.iter().find(|(lib, _)| *lib == tete).map(|(_, cle)| *cle);
        let candidat = libelles
            .iter()
            .filter(|(lib, _)| !lib.is_empty() && (tete.starts_with(&format!("{} ", lib)) || (lib.starts_with(&tete) && tete.chars().count() >= 6)))
            .map(|(lib, cle)| (lib.chars().count(), *cle))
            .max();
        if let Some(cle) = exact.or(candidat.map(|(_, c)| c)) {
            sortie.entry(cle).or_insert_with(|| valeur.to_string());
        }
    }
    sortie
}

/// Le premier chiffre (le zero) de chaque ecriture decimale d'Unicode (categorie `Nd`). Chaque ecriture compte dix
/// chiffres consecutifs.
const ZEROS: [u32; 68] = [
    0x30, 0x660, 0x6F0, 0x7C0, 0x966, 0x9E6, 0xA66, 0xAE6, 0xB66, 0xBE6, 0xC66, 0xCE6, 0xD66, 0xDE6, 0xE50, 0xED0, 0xF20, 0x1040, 0x1090, 0x17E0,
    0x1810, 0x1946, 0x19D0, 0x1A80, 0x1A90, 0x1B50, 0x1BB0, 0x1C40, 0x1C50, 0xA620, 0xA8D0, 0xA900, 0xA9D0, 0xA9F0, 0xAA50, 0xABF0, 0xFF10,
    0x104A0, 0x10D30, 0x11066, 0x110F0, 0x11136, 0x111D0, 0x112F0, 0x11450, 0x114D0, 0x11650, 0x116C0, 0x11730, 0x118E0, 0x11950, 0x11C50,
    0x11D50, 0x11DA0, 0x11F50, 0x16A60, 0x16AC0, 0x16B50, 0x1D7CE, 0x1D7D8, 0x1D7E2, 0x1D7EC, 0x1D7F6, 0x1E140, 0x1E2F0, 0x1E4F0, 0x1E950,
    0x1FBF0,
];

/// La valeur d'un chiffre decimal d'Unicode (`unicodedata.decimal`), ou `None`.
fn valeur_chiffre(c: char) -> Option<u32> {
    let n = c as u32;
    ZEROS.iter().find(|z| (**z..**z + 10).contains(&n)).map(|z| n - z)
}

/// Un chiffre pour `\d` : la categorie `Nd` d'Unicode.
fn chiffre(c: char) -> bool {
    valeur_chiffre(c).is_some()
}

/// Le nombre d'un groupe de chiffres décimaux (toutes écritures), ou `None`.
fn nombre(t: &str) -> Option<u32> {
    if t.is_empty() {
        return None;
    }
    let mut v: u32 = 0;
    for c in t.chars() {
        v = v.checked_mul(10)?.checked_add(valeur_chiffre(c)?)?;
    }
    Some(v)
}

/// Une date lue `AAAA-M-J` ou `J/M/AAAA` (separateurs `/`, `.`, `-`, espace), en ISO ; `None` si elle ne se
/// lit pas ou n'existe pas.
pub fn date_lue(texte: &str) -> Option<Jour> {
    let t = strip(texte);
    let c: Vec<char> = t.chars().collect();
    // Les trois groupes de chiffres, separes par `sep`, de largeurs permises.
    let groupes = |sep: &dyn Fn(char) -> bool, largeurs: [(usize, usize); 3]| -> Option<[String; 3]> {
        let mut morceaux: Vec<String> = vec![String::new()];
        for x in &c {
            if sep(*x) {
                morceaux.push(String::new());
            } else if chiffre(*x) {
                morceaux.last_mut()?.push(*x);
            } else {
                return None;
            }
        }
        if morceaux.len() != 3 || morceaux.iter().zip(largeurs).any(|(m, (a, b))| !(a..=b).contains(&m.chars().count())) {
            return None;
        }
        let mut it = morceaux.into_iter();
        Some([it.next()?, it.next()?, it.next()?])
    };
    let (a, mo, j) = if let Some([a, mo, j]) = groupes(&|x| x == '-', [(4, 4), (1, 2), (1, 2)]) {
        (a, mo, j)
    } else {
        let [j, mo, a] = groupes(&|x| matches!(x, '/' | '.' | '-' | ' '), [(1, 2), (1, 2), (4, 4)])?;
        (a, mo, j)
    };
    Jour::nouveau(nombre(&a)? as i32, nombre(&mo)?, nombre(&j)?)
}

// LES PAYS QUE LA RELEVE SAIT NOMMER : ceux de la liste livree (`pays.rs`, Code officiel geographique de l'INSEE),
// par leur nom (celui du COG ou le nom officiel, sans accents) ou leur code ISO a deux lettres. Un nom ou un
// code que la liste ne connait pas laisse la case vide et la note le dit.

/// (code ISO, codification UE) d'un pays de la liste nomme en francais, ou de son code a deux lettres ;
/// `None` pour ce que la liste ne connait pas.
pub fn pays_lu(texte: &str) -> R<Option<(String, Option<&'static str>)>> {
    let t = sans_accents(texte).replace('\u{2019}', "'");
    let lu = |p: socle::pays::Pays| (p.code.clone(), Some(socle::pays::codification(&p)));
    if t.chars().count() == 2 && t.chars().all(|c| c.is_ascii_lowercase()) {
        return Ok(socle::pays::du_code(&t)?.map(lu));
    }
    match socle::pays::du_nom(&t)? {
        Some(p) => Ok(Some(lu(p))),
        None => Ok(socle::pays::du_nom(&t.replace(' ', "-"))?.map(lu)),
    }
}

/// CE QUE LES REPONSES DISENT DE LA FICHE DU SALARIE : chaque case lue, dans l'ordre du fichier de la fiche ; une case
/// que les reponses ne disent pas, ou pas lisiblement, ne s'ecrit pas.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FicheLue {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prenom_affichage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nom_affichage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lieu_naissance_affichage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adresse_affichage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commune_affichage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_conges_spectacles: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_naissance: Option<Jour>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anniversaire_droits: Option<Jour>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_postal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nir_cle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sexe: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pays_naissance: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code_nationalite_dsn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pays_nationalite: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pays_nationalite_hors_liste: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub banque: Option<BanqueLue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub departement_naissance: Option<String>,
}

/// Le compte bancaire que les reponses donnent : l'IBAN et le BIC, chacun s'il est lisible.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BanqueLue {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iban: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bic: Option<String>,
}

/// `fiche_depuis_reponses` : (les cases lues de la fiche, ce qui n'a pas pu se lire). Rien n'est suppose. Une liste
/// livree (les pays) qui ne se lit pas refuse.
pub fn fiche_depuis_reponses(rep: &HashMap<&'static str, String>) -> R<(FicheLue, Vec<String>)> {
    let mut d = FicheLue::default();
    let mut illisible: Vec<String> = Vec::new();
    // Une reponse dite : une question sans reponse, ou a la reponse vide, ne dit rien.
    let reponse = |cle: &str| rep.get(cle).map(String::as_str).filter(|r| !r.is_empty());
    let dite = |t: &str| Some(t.to_string()).filter(|t| !t.is_empty());
    d.prenom_affichage = reponse("prenom").and_then(|r| dite(strip(r)));
    d.nom_affichage = reponse("nom").and_then(|r| dite(strip(r)));
    d.lieu_naissance_affichage = reponse("lieu").and_then(|r| dite(strip(r)));
    d.adresse_affichage = reponse("adresse").and_then(|r| dite(strip(r)));
    d.commune_affichage = reponse("commune").and_then(|r| dite(strip(r)));
    d.num_conges_spectacles = reponse("conges").and_then(|r| dite(&r.replace(' ', "")));
    for (q, cle) in [("naissance", "date_naissance"), ("anniversaire", "anniversaire_droits")] {
        if let Some(r) = reponse(q) {
            let v = date_lue(r);
            if v.is_none() {
                illisible.push(format!("{} : « {} »", cle.replace('_', " "), r));
            }
            match q {
                "naissance" => d.date_naissance = v,
                _ => d.anniversaire_droits = v,
            }
        }
    }
    if let Some(r) = reponse("code_postal") {
        let cp: String = r.chars().filter(|c| !espace(*c)).collect();
        if cp.chars().count() == 5 && cp.chars().all(chiffre) {
            d.code_postal = Some(cp);
        } else {
            illisible.push(format!("code postal : « {} »", r));
        }
    }
    if let Some(r) = reponse("nir") {
        let n: Vec<char> = r.chars().filter(|c| !espace(*c) && !matches!(c, '.' | '-')).collect();
        let tout_chiffres = n.iter().all(|c| chiffre(*c));
        if tout_chiffres && n.len() == 15 {
            d.nir = Some(n[..13].iter().collect::<String>());
            d.nir_cle = Some(n[13..].iter().collect::<String>());
        } else if tout_chiffres && n.len() == 13 {
            d.nir = Some(n.iter().collect::<String>());
        } else {
            illisible.push(format!("numéro de sécurité sociale : « {} »", r));
        }
    }
    if let Some(r) = reponse("sexe").filter(|_| d.nir.is_none()) {
        let s = sans_accents(r);
        match s.chars().next() {
            Some('f') => d.sexe = Some("2".into()),
            Some('h') | Some('m') => d.sexe = Some("1".into()),
            _ => illisible.push(format!("sexe : « {} »", r)),
        }
    }
    if let Some(r) = reponse("pays") {
        // Le pays de naissance se lit aussi dans la table « Pays » de la DSN (`pays::naissance_du_nom`) : le
        // Kosovo, l'URSS, que le COG n'a pas.
        let lu = match pays_lu(r)? {
            Some((code, _)) => Some(code),
            None => {
                let t = sans_accents(r).replace('\u{2019}', "'");
                if t.chars().count() == 2 && t.chars().all(|c| c.is_ascii_lowercase()) {
                    socle::pays::de_la_table_dsn(&t)?.map(|p| p.code)
                } else {
                    socle::pays::naissance_du_nom(&t)?
                }
            }
        };
        match lu {
            Some(code) => d.pays_naissance = Some(code),
            None => illisible.push(format!("pays de naissance : « {} »", r)),
        }
    }
    // LA NATIONALITE : le pays se garde (`pays_nationalite`), la codification UE s'en deduit.
    if let Some(r) = reponse("nationalite") {
        let t = sans_accents(r);
        let pays = match NATIONALITES.iter().find(|(n, _)| *n == t) {
            Some((_, c)) => Some(c.to_string()),
            None => pays_lu(&t)?.and_then(|(c, ue)| ue.map(|_| c)),
        };
        match pays {
            Some(pays) => {
                // La codification se lit dans la liste (un territoire suit son pays de rattachement), `pays.rs`.
                match socle::pays::du_code(&pays)? {
                    Some(p) => {
                        d.code_nationalite_dsn = Some(socle::pays::codification(&p).to_string());
                        d.pays_nationalite = Some(p.code);
                    }
                    None => illisible.push(format!("nationalité : « {} »", r)),
                }
            }
            None => match NATIONALITES_HORS_LISTE.iter().find(|(n, _)| *n == t) {
                // Un pays hors de la liste se garde dans sa case ; la zone reste a choisir (la fiche le dit).
                Some((_, nom)) => d.pays_nationalite_hors_liste = Some(nom.to_string()),
                None => illisible.push(format!("nationalité : « {} »", r)),
            },
        }
    }
    // LE COMPTE BANCAIRE : l'IBAN compact (sans blancs, en majuscules) ; un IBAN dont la forme ou la cle est fausse ne
    // s'ecrit pas, il se dit a completer avec sa raison (`socle::iban::faute_iban`) ; le BIC de meme (`faute_bic`).
    let mut banque = BanqueLue::default();
    for (q, cle, faute) in [("iban", "IBAN", socle::iban::faute_iban as fn(&str) -> Option<String>), ("bic", "BIC", socle::iban::faute_bic)] {
        let Some(brut) = reponse(q).map(strip).filter(|b| !b.is_empty()) else { continue };
        match faute(brut) {
            None => {
                let compact = Some(socle::iban::iban_compact(brut));
                match q {
                    "iban" => banque.iban = compact,
                    _ => banque.bic = compact,
                }
            }
            Some(f) => illisible.push(format!("{} : « {} » ({})", cle, brut, f)),
        }
    }
    if banque != BanqueLue::default() {
        d.banque = Some(banque);
    }
    // Ce que le NIR ecrit de la naissance complete ce que les reponses ne disent pas.
    if let Some((departement, pays)) = d.nir.as_deref().and_then(naissance_depuis_nir) {
        d.departement_naissance = Some(departement);
        if d.pays_naissance.is_none() {
            d.pays_naissance = pays;
        }
    }
    Ok((d, illisible))
}

/// Le departement de naissance que le NIR ECRIT (les 6e et 7e signes ; 99 pour une naissance a l'etranger), et le pays
/// de naissance qu'il en deduit (la France, hors 99). Deux cas se taisent : la Corse d'avant 1976 (« 20 ») et un NIR
/// incomplet.
fn naissance_depuis_nir(nir: &str) -> Option<(String, Option<String>)> {
    let c: Vec<char> = strip(nir).chars().collect();
    if c.len() != 13 || !c.iter().all(|x| chiffre(*x)) || !matches!(c[0], '1' | '2') {
        return None;
    }
    let dep: String = c[5..7].iter().collect();
    if dep == "20" {
        return None;
    }
    let pays = (dep != "99").then(|| "FR".to_string());
    Some((dep, pays))
}
