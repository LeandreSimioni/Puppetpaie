//! LE NUMERO D'UN CONTRAT (regle 4 : il ne se recalcule pas, il se lit). Il identifie le contrat en DSN
//! (S21.G00.40.009), unique par individu et par etablissement dans une declaration (CCH-12,
//! `atelier/reference/norme-dsn/texte-2027/bloc-S21.G00.40.txt`) ; aucune suite continue n'est exigee.
//!
//! Il s'ecrit dans l'engagement a sa validation (`a_la_validation`, que seule `etats::valider` appelle), puis tout ce qui
//! part -- le contrat a la signature ou imprime, la DPAE, la DSN, le bulletin -- le lit la. Le numero donne est le suivant
//! du plus haut que portent, pour le meme salarie et la meme annee, les engagements et ce qui est parti (`departs`)
//! (`derniers_pris`) : un engagement supprime avant tout envoi rend le sien.
//!
//! Une revalidation garde le numero. Tant que rien n'est parti sous lui, un engagement dont le salarie ou le debut a
//! change en recoit un nouveau ; des qu'un envoi l'a porte, il ne change plus. Un numero venu avec la saisie (un contrat
//! repris d'un autre logiciel) reste le sien, sauf sur la copie d'un engagement plus ancien qui le porte deja.

use std::collections::HashMap;

use droit::contexte::{Contexte, IdSalarie};
use socle::date::Jour;
use socle::echec::R;

use crate::departs::Departs;
use crate::stock::{self, Objet};

/// POUR QUI LE NUMERO A ETE DONNE (`stock::Ecrit::numero_pour`) : la cle du salarie et le debut du contrat a ce
/// moment-la, et quand.
#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct NumeroPour {
    pub salarie: IdSalarie,
    pub debut: Jour,
    #[serde(with = "socle::paris::ecrit::seconde")]
    pub le: socle::paris::Moment,
}

/// UN NUMERO DE CONTRAT, tel que la DSN le declare en S21.G00.40.009 : de 5 a 20 caracteres (`X [5,20]`,
/// `atelier/reference/norme-dsn/texte-2027/bloc-S21.G00.40.txt` l. 527-551), pris parmi [a-z], [A-Z], [0-9], le trait
/// d'union, l'espace, le point, l'arobase, le slash, l'underscore et les lettres accentuees de la liste (`ACCENTUES`),
/// sans espace au debut ni a la fin (§ 4.4.11, `chapitre-4-controles.txt` l. 760-780). Il ne se donne qu'ici
/// (`former`, regle 4) ; ailleurs il se lit tel qu'il est deja ecrit (`lire`, ou `serde` : la meme chaine), d'un
/// engagement, d'une charge, d'une DSN deposee ; sa saisie (une charge) se range d'abord sans ses blancs de bord
/// (`rangee`). Ni `From<&str>` ni `Default` : un texte quelconque n'en est pas un.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(into = "String")]
pub struct NumeroContrat(String);

impl<'de> serde::Deserialize<'de> for NumeroContrat {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        NumeroContrat::lire(&socle::forme::range_si_saisi(String::deserialize(d)?, NumeroContrat::rangee)).map_err(|e| serde::de::Error::custom(e.phrase))
    }
}

impl droit::fiche::FormeDeCase for NumeroContrat {
    const RANGER: Option<fn(&str) -> String> = Some(NumeroContrat::rangee);
}

/// Les caracteres accentues qu'un numero de contrat admet (§ 4.4.11).
const ACCENTUES: [char; 8] = ['à', 'é', 'è', 'ê', 'ë', 'ï', 'ô', 'ç'];

impl NumeroContrat {
    /// Un numero de contrat ecrit `texte` ; toute autre forme que celle de la norme se refuse.
    pub fn lire(texte: &str) -> R<NumeroContrat> {
        let n = texte.chars().count();
        let admis = |c: char| c.is_ascii_alphanumeric() || "- .@/_".contains(c) || ACCENTUES.contains(&c);
        if !(5..=20).contains(&n) || !texte.chars().all(admis) || texte.starts_with(' ') || texte.ends_with(' ') {
            return Err(socle::refus!(
                "« {} » n'est pas un numéro de contrat de la DSN (S21.G00.40.009) : de 5 à 20 lettres, chiffres, points, \
                 traits d'union, arobases, barres obliques, tirets bas ou espaces, sans espace au début ni à la fin.",
                texte
            ));
        }
        Ok(NumeroContrat(texte.to_string()))
    }

    /// Le numero saisi `texte` dans sa forme, avant d'etre lu : sans ses blancs de bord (`FormeDeCase::RANGER`).
    pub fn rangee(texte: &str) -> String {
        droit::fiche::sans_blancs_de_bord(texte)
    }

    /// LE NUMERO QU'UNE PIECE D'AUTRUI IMPRIME (un bulletin d'un autre logiciel) : sans ses blancs de bord ; a blanc ou
    /// hors de sa forme, la case reste vide, comme un code hors de nos listes (la piece d'un tiers se lit, elle ne se
    /// refuse pas pour lui).
    pub fn d_une_piece(texte: &str) -> Option<NumeroContrat> {
        match NumeroContrat::lire(&NumeroContrat::rangee(texte)) {
            Ok(n) => Some(n),
            Err(_) => None,
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// L'annee (2000 + les deux chiffres) et le rang d'un numero que `former` a donne (`FA1.26.16`) ; `None` pour un
    /// numero d'une autre forme (un contrat repris d'un autre logiciel).
    pub fn annee_et_rang(&self) -> Option<(i32, i64)> {
        let (tete, rang) = self.0.rsplit_once('.')?;
        let (_, yy) = tete.rsplit_once('.')?;
        if yy.len() != 2 {
            return None;
        }
        Some((2000 + socle::num::entier_de_chiffres(yy)? as i32, socle::num::entier_de_chiffres(rang)?))
    }
}

impl std::ops::Deref for NumeroContrat {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl std::borrow::Borrow<str> for NumeroContrat {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for NumeroContrat {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for NumeroContrat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<NumeroContrat> for String {
    fn from(n: NumeroContrat) -> String {
        n.0
    }
}

/// LE NUMERO D'UN BULLETIN DU LOGICIEL, `N/AAAA` : son rang dans la suite des bulletins de l'annee, puis l'annee. Il se
/// donne par `numeroter`, et par le depart des documents d'un mois sur la suite qu'il reserve
/// (`routes::gestes::depart::fabriquer_documents`) ; l'apercu d'un bulletin en montre un (`du_rang`). La paie ecrite le
/// garde, et il se relit tel qu'il est ecrit (`lire`, ou `serde` : la meme chaine). Celui qu'un bulletin d'autrui imprime
/// est du texte (`importe::ContratAtteste::numero_bulletin`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(into = "String")]
pub struct NumeroDeBulletin {
    texte: String,
    rang: i64,
    annee: i64,
}

impl<'de> serde::Deserialize<'de> for NumeroDeBulletin {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        NumeroDeBulletin::lire(&String::deserialize(d)?).map_err(|e| serde::de::Error::custom(e.phrase))
    }
}

impl NumeroDeBulletin {
    /// Le numero ecrit `texte` : des chiffres, `/`, l'annee en quatre chiffres ; toute autre forme se refuse.
    pub fn lire(texte: &str) -> R<NumeroDeBulletin> {
        let lu = texte.split_once('/').and_then(|(n, an)| {
            let rang = socle::num::entier_de_chiffres(n)?;
            let annee = if an.len() == 4 { socle::num::entier_de_chiffres(an) } else { None }?;
            Some((rang, annee))
        });
        match lu {
            Some((rang, annee)) => Ok(NumeroDeBulletin { texte: texte.to_string(), rang, annee }),
            None => Err(socle::refus!("« {} » n'est pas un numéro de bulletin : son rang, « / », puis l'année en quatre chiffres (12/2026)", texte)),
        }
    }

    /// Le bulletin de rang `rang` de l'annee `annee` : celui que `numeroter` donne, celui que le depart des documents
    /// d'un mois donne sur la suite qu'il reserve (`routes::gestes::depart::fabriquer_documents`), ou celui que l'apercu
    /// d'un bulletin montre.
    pub fn du_rang(rang: i64, annee: i32) -> NumeroDeBulletin {
        NumeroDeBulletin { texte: format!("{}/{}", rang, annee), rang, annee: i64::from(annee) }
    }

    /// Son rang dans la suite de son annee.
    pub fn rang(&self) -> i64 {
        self.rang
    }

    /// Son annee.
    pub fn annee(&self) -> i64 {
        self.annee
    }
}

impl std::fmt::Display for NumeroDeBulletin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.texte)
    }
}

impl From<NumeroDeBulletin> for String {
    fn from(n: NumeroDeBulletin) -> String {
        n.texte
    }
}

/// UN NUMERO (d'un contrat, d'un avenant) TEL QU'UNE PHRASE LE DIT, la seule fonction qui l'y ecrit : `avant`, le
/// numero puis `apres` quand il y en a un ; `sans` quand il n'y en a pas.
pub fn numero_dit(numero: Option<impl std::fmt::Display>, avant: &str, apres: &str, sans: &str) -> String {
    match numero {
        Some(n) => format!("{}{}{}", avant, n, apres),
        None => sans.to_string(),
    }
}

impl PartialEq<str> for NumeroContrat {
    fn eq(&self, autre: &str) -> bool {
        self.0 == autre
    }
}

impl PartialEq<&str> for NumeroContrat {
    fn eq(&self, autre: &&str) -> bool {
        self.0 == *autre
    }
}

impl PartialEq<String> for NumeroContrat {
    fn eq(&self, autre: &String) -> bool {
        self.0 == *autre
    }
}

impl PartialEq<NumeroContrat> for &str {
    fn eq(&self, autre: &NumeroContrat) -> bool {
        *self == autre.0
    }
}

impl PartialEq<NumeroContrat> for String {
    fn eq(&self, autre: &NumeroContrat) -> bool {
        *self == autre.0
    }
}

/// Le numero de rang `seq` de l'annee `annee`, pour le salarie de cette fiche : `<initiales><code>.<AA>.<rang>`. Le seul
/// qui fabrique un numero de contrat (regle 4) ; des initiales ou un code qui le sortiraient de la forme de la norme le
/// font refuser (`NumeroContrat::lire`).
pub fn former(fiche: &droit::fiche::ChampsSalarie, annee: i32, seq: i64) -> R<NumeroContrat> {
    use droit::fiche::Lue;
    let an = annee.to_string();
    NumeroContrat::lire(&format!("{}{}.{}.{:02}", fiche.initiales_contrat.ecrite("initiales_contrat")?, fiche.code_salarie.ecrite("code_salarie")?, &an[2.min(an.len())..], seq))
}

/// Le dernier rang pris en `annee`, par salarie (la cle de sa fiche) : le plus grand des numeros de cette annee que
/// portent ses engagements, ou sous lesquels quelque chose est parti pour lui (`departs`).
fn derniers_pris(ctx: &Contexte, annee: i32, departs: &Departs) -> R<HashMap<IdSalarie, i64>> {
    derniers_pris_sauf(ctx, annee, None, departs)
}

/// La meme, sans l'engagement `sauf` (son fichier et son rang) : celui qui recoit un nouveau numero.
fn derniers_pris_sauf(ctx: &Contexte, annee: i32, sauf: Option<(socle::mois::Mois, usize)>, departs: &Departs) -> R<HashMap<IdSalarie, i64>> {
    let mut pris: HashMap<IdSalarie, i64> = HashMap::new();
    let mut noter = |cle: IdSalarie, numero: &NumeroContrat| {
        if let Some((a, seq)) = numero.annee_et_rang() {
            if a == annee {
                let v = pris.entry(cle).or_insert(0);
                *v = (*v).max(seq);
            }
        }
    };
    for o in stock::objets(&ctx.racine)? {
        if sauf == Some((o.fichier, o.rang)) {
            continue;
        }
        let (Some(cle), Some(n)) = (o.carte().salarie.clone(), o.carte().numero.clone()) else { continue };
        noter(cle, &n);
    }
    for (cle, n) in departs.engagements() {
        noter(cle.clone(), n);
    }
    Ok(pris)
}

/// LE PLUS HAUT NUMERO DE BULLETIN `N/AAAA` DE `annee` QU'UN BULLETIN IMPORTE IMPRIME (0 sans aucun) : la suite des
/// bulletins de l'annee continue apres lui, et deux bulletins de la meme annee ne portent jamais le meme numero. Un
/// numero d'une autre forme n'est pas un rang de cette suite : il ne se lit pas comme tel.
pub fn dernier_bulletin_importe(racine: &std::path::Path, annee: i64) -> R<i64> {
    let rang = |t: Option<&str>| -> Option<i64> {
        let n = match NumeroDeBulletin::lire(t?.trim()) {
            Ok(n) => n,
            Err(_) => return None,
        };
        (n.annee() == annee).then_some(n.rang())
    };
    let mut haut = 0;
    for i in crate::importe::tous(racine)? {
        let du_contrat = rang(i.contrat().and_then(|c| c.numero_bulletin.as_deref()));
        let des_paies = i.paies().values().filter_map(|p| rang(p.bulletin.as_ref().and_then(|b| b.numero.as_deref()))).max();
        for r in du_contrat.into_iter().chain(des_paies) {
            haut = haut.max(r);
        }
    }
    Ok(haut)
}

/// Un envoi a-t-il porte le numero `n` de l'engagement `o` (du salarie `cle`) ? Ce qui est parti sous lui (`departs`),
/// ou une version de lui gardee parce qu'un envoi l'avait emportee.
fn porte_par_un_envoi(o: &Objet, cle: &IdSalarie, n: &NumeroContrat, departs: &Departs) -> bool {
    if !o.brut.versions.is_empty() {
        return true;
    }
    !departs.de(cle, n).is_empty()
}


/// Un engagement ecrit avant `o` (plus ancien fichier, ou plus haut dans le meme) porte-t-il deja le numero `n` ? Une
/// copie d'engagement emporte le numero de l'original : il reste a l'original.
/// Le contrat importe que `o` continue porte le sien : c'est le meme contrat, pas une copie -- le contrat d'usage qui
/// continue ici (`importe::continue_par`).
fn porte_par_un_autre(ctx: &Contexte, o: &Objet, n: &NumeroContrat) -> R<bool> {
    let continue_ = crate::importe::continue_par(&ctx.racine, o.carte())?.map(|i| (i.objet().fichier, i.objet().rang));
    Ok(stock::objets(&ctx.racine)?.into_iter().any(|x| {
        (x.fichier, x.rang) < (o.fichier, o.rang)
            && continue_ != Some((x.fichier, x.rang))
            && x.carte().numero.as_ref() == Some(n)
    }))
}

/// LE NUMERO A LA VALIDATION de l'engagement `o` (`etats::valider`), pose sur lui ; `le` : l'instant de la validation.
/// Il garde celui qu'il porte, sauf si son salarie ou son debut a change depuis qu'il l'a recu et qu'aucun envoi ne l'a
/// porte : il en recoit alors un nouveau, comme un engagement qui n'en a pas.
pub(crate) fn a_la_validation(ctx: &Contexte, o: &mut Objet, le: socle::paris::Moment, departs: &Departs) -> R<()> {
    let cle = o.carte().salarie.clone().ok_or_else(|| socle::fichiers::absent("salarie"))?;
    let fiche = crate::fiche_du_salarie(ctx, &cle)?;
    let debut = crate::debut_brut(&o.brut).ok_or_else(|| socle::fichiers::absent("debut"))?;
    let porte = o.carte().numero.clone();
    // UN NUMERO PARTI NE CHANGE PLUS : celui de la validation que la modification remplace, des qu'un envoi l'a porte ;
    // celui qu'une DSN de l'ancien logiciel a declare pour un contrat repris (`importe::numero_declare`).
    let parti = o.brut.precedente.as_ref().and_then(|p| p.saisie.as_ref()).and_then(|s| s.carte.numero.clone());
    let parti = match parti.filter(|p| porte_par_un_envoi(o, &cle, p, departs)) {
        Some(p) => Some(p),
        None => crate::importe::numero_declare(o)?,
    };
    if let Some(parti) = parti {
        if porte.as_ref().is_some_and(|n| *n != parti) {
            return Err(socle::refus!(
                "{} : ce contrat est parti sous le n° {} ; un numéro parti ne change plus (il identifie le contrat en DSN, \
                 S21.G00.40.009). Rien n'est écrit.",
                crate::salarie_nomme(ctx, o.carte().salarie.as_ref()),
                parti
            ));
        }
    }
    if let Some(n) = porte {
        let garde = match &o.brut.numero_pour {
            Some(p) => {
                let meme = p.salarie == cle && p.debut == debut;
                meme || porte_par_un_envoi(o, &cle, &n, departs)
            }
            // Un numero venu avec la saisie (un contrat repris d'un autre logiciel, sous la forme qu'il avait) : le sien,
            // sauf s'il est la copie d'un engagement plus ancien.
            None => !porte_par_un_autre(ctx, o, &n)?,
        };
        if garde {
            return Ok(());
        }
    }
    let annee = debut.an;
    // Aucun numero pris cette annee : le premier sera le 1.
    let pris = derniers_pris_sauf(ctx, annee, Some((o.fichier, o.rang)), departs)?;
    let dernier: i64 = pris.get(&cle).into_iter().copied().sum();
    let numero = former(&fiche, annee, dernier + 1)?;
    let e = o.brut_mut();
    e.saisie.carte.numero = Some(numero);
    e.numero_pour = Some(NumeroPour { salarie: cle.clone(), debut, le });
    Ok(())
}

/// Le numero de contrat que la paie `p`, sans numero, porterait : la suite des numeros `pris` pour son salarie dans
/// l'annee du debut du contrat (`derniers_pris`, lus une fois par annee), en memoire -- celui que sa validation lui
/// donnerait (`a_la_validation`).
fn proposer(ctx: &Contexte, p: &mut crate::calcul::Contenu, pris: &mut HashMap<i32, HashMap<IdSalarie, i64>>, departs: &Departs) -> R<()> {
    let annee = p.engagement.debut.an;
    let pris = match pris.entry(annee) {
        std::collections::hash_map::Entry::Occupied(x) => x.into_mut(),
        std::collections::hash_map::Entry::Vacant(x) => x.insert(derniers_pris(ctx, annee, departs)?),
    };
    let dernier = pris.entry(p.engagement.salarie.clone()).or_insert(0);
    *dernier += 1;
    p.engagement.numero = Some(former(&p.salarie, annee, *dernier)?);
    Ok(())
}

/// LES NUMEROS DE CONTRAT QUE DES PAIES PORTERAIENT, en memoire : un engagement qui porte deja le sien le garde ; un
/// engagement qui n'a pas encore le sien (pas encore valide) recoit celui que sa validation lui donnerait (`proposer`).
pub fn numeroter_les_contrats(ctx: &Contexte, paies: &mut [crate::calcul::Paie], departs: &Departs) -> R<()> {
    let mut pris = HashMap::new();
    for paie in paies.iter_mut() {
        if paie.engagement.numero.is_some() {
            continue;
        }
        proposer(ctx, paie.ecrire(), &mut pris, departs)?;
    }
    Ok(())
}

/// LES NUMEROS QUE DES PAIES PORTERAIENT, en memoire : les numeros de contrat (`numeroter_les_contrats`) ; une paie dont le bulletin est
/// parti garde son numero de bulletin, une autre recoit le `prochain` (`N/AAAA`), puis le suivant. Un mois sans paie n'a pas de
/// bulletin. Rend le prochain numero de bulletin libre.
pub fn numeroter(ctx: &Contexte, paies: &mut [crate::calcul::Paie], annee: i32, prochain: i64, departs: &Departs) -> R<i64> {
    let mut n_bull = prochain;
    let mut pris = HashMap::new();
    for paie in paies.iter_mut() {
        let p = paie.ecrire();
        if p.engagement.numero.is_none() {
            proposer(ctx, p, &mut pris, departs)?;
        }
        if p.engagement.sans_paie() || p.numero_bulletin.is_some() {
            continue;
        }
        p.numero_bulletin = Some(NumeroDeBulletin::du_rang(n_bull, annee));
        n_bull += 1;
    }
    Ok(n_bull)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un numero de contrat : de 5 a 20 caracteres (S21.G00.40.009, `X [5,20]`), du jeu du § 4.4.11, sans espace aux
    /// bords ; le JSON garde la meme chaine. `former` refuse des initiales qui l'en sortiraient.
    #[test]
    fn un_numero_de_contrat_a_la_forme_de_la_norme() {
        for bon in ["FA1.26.13", "C-001", "a b/c_d@é.ç", "12345678901234567890"] {
            assert_eq!(NumeroContrat::lire(bon).ok().map(String::from), Some(bon.to_string()));
        }
        for faux in ["", "C1", "FA1 ", " FA1.26", "123456789012345678901", "FA1.26#3", "FÀ1.26.13"] {
            assert!(NumeroContrat::lire(faux).is_err(), "{:?} n'est pas un numero de contrat", faux);
        }
        let n = NumeroContrat::lire("FA1.26.13").unwrap();
        assert_eq!((n.annee_et_rang(), socle::ecriture::json_en_ligne(&n)), (Some((2026, 13)), r#""FA1.26.13""#.to_string()));
        assert!(socle::forme::lire_texte::<NumeroContrat>(r#""C1""#, "numero").is_err());
        let fiche = |initiales: &str| -> droit::fiche::ChampsSalarie {
            socle::forme::lire_texte(&format!(r#"{{"initiales_contrat": "{}", "code_salarie": "1"}}"#, initiales), "fiche").unwrap()
        };
        assert_eq!(former(&fiche("FA"), 2026, 3).ok().map(String::from), Some("FA1.26.03".to_string()));
        assert!(former(&fiche("F#"), 2026, 3).is_err());
    }
}
