//! LE NUMERO D'UN CONTRAT (regle 4 : il ne se recalcule pas, il se lit). Il identifie le contrat en DSN
//! (S21.G00.40.009), unique par individu et par etablissement dans une declaration (CCH-12,
//! `atelier/reference/norme-dsn/texte-2027/bloc-S21.G00.40.txt`) ; aucune suite continue n'est exigee.
//!
//! Il s'ecrit dans l'engagement a sa validation (`a_la_validation`, que seule `etats::valider` appelle), puis tout ce qui
//! part -- le contrat a la signature ou imprime, la DPAE, la DSN, le bulletin -- le lit la. Le numero donne est le suivant
//! du plus haut que portent, pour le meme salarie et la meme annee, les engagements et ce qui est parti (`departs`)
//! (`derniers_pris`) : un engagement supprime avant tout envoi rend le sien. Il n'est jamais celui d'un autre salarie
//! (`former`, `Pris`) : le numero est la cle de ce qui est parti.
//!
//! Une revalidation garde le numero. Tant que rien n'est parti sous lui, un engagement dont le salarie ou le debut a
//! change en recoit un nouveau ; des qu'un envoi l'a porte, il ne change plus. Un numero venu avec la saisie (un contrat
//! repris d'un autre logiciel) reste le sien, sauf sur la copie d'un engagement plus ancien qui le porte deja.

use std::collections::{HashMap, HashSet};

use droit::contexte::{Contexte, IdSalarie};
use socle::date::Jour;
use socle::echec::R;
use socle::mois::{Annee, RangDansLAnnee};

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
    /// numero d'une autre forme (un contrat repris d'un autre logiciel). Le rang ne sort pas d'ici : il ne sert qu'a
    /// trouver le plus haut numero pris (`derniers_pris`) et a donner le suivant (`former`).
    fn annee_et_rang(&self) -> Option<(Annee, RangDansLAnnee)> {
        use socle::piece::{format, Zone};
        let (tete, rang) = self.0.rsplit_once('.')?;
        let (_, yy) = tete.rsplit_once('.')?;
        Some((Zone::lue(yy, format::Aa).valeur()?, Zone::lue(rang, format::Rang).valeur()?))
    }

    /// Son rang dans l'annee `annee`, s'il est un numero que `former` a donne cette annee-la.
    fn rang_en(&self, annee: Annee) -> Option<RangDansLAnnee> {
        self.annee_et_rang().filter(|(a, _)| *a == annee).map(|(_, r)| r)
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

/// LE NUMERO D'UN CERTIFICAT CONGES SPECTACLES (N CE) : un nombre, le premier des certificats d'une paie partie
/// (`paie_ecrite::PaieEcrite::ce`), ecrit et lu tel quel ; les suivants le continuent. La suite commence a un : zero
/// n'en est pas un, et se refuse a la lecture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct NumeroDeCertificat(std::num::NonZeroU64);

impl NumeroDeCertificat {
    /// Le premier numero de la suite.
    pub const PREMIER: NumeroDeCertificat = NumeroDeCertificat(std::num::NonZeroU64::MIN);

    /// Le numero qui le suit dans la sequence continue des certificats.
    pub fn suivant(self) -> NumeroDeCertificat {
        NumeroDeCertificat(self.0.saturating_add(1))
    }

    /// La suite des numeros a partir de celui-ci, lui compris : elle se zippe avec les certificats qu'elle numerote.
    pub fn suite(self) -> impl Iterator<Item = NumeroDeCertificat> {
        std::iter::successors(Some(self), |n| Some(n.suivant()))
    }
}

impl std::fmt::Display for NumeroDeCertificat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

/// LE NUMERO D'UN BULLETIN DU LOGICIEL, `N/AAAA` : son rang dans la suite des bulletins de l'annee, puis l'annee. La
/// suite commence au premier de l'annee (`premier`) et chaque numero donne le suivant (`suivant`) : le rang ne sort pas
/// d'ici. La paie ecrite le garde, le compteur des bulletins (`config/compteurs.json`) aussi, et il se relit tel qu'il
/// est ecrit (`lire`, ou `serde` : la meme chaine). Celui qu'un bulletin d'autrui imprime est du texte
/// (`importe::ContratAtteste::numero_bulletin`). L'ordre est celui de l'annee, puis du rang.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(into = "String")]
pub struct NumeroDeBulletin {
    annee: Annee,
    rang: RangDansLAnnee,
}

impl<'de> serde::Deserialize<'de> for NumeroDeBulletin {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        NumeroDeBulletin::lire(&String::deserialize(d)?).map_err(|e| serde::de::Error::custom(e.phrase))
    }
}

impl NumeroDeBulletin {
    /// Le numero ecrit `texte` : des chiffres, `/`, l'annee en quatre chiffres ; toute autre forme se refuse.
    pub fn lire(texte: &str) -> R<NumeroDeBulletin> {
        use socle::piece::{format, Zone};
        let lu = texte.split_once('/').and_then(|(n, an)| {
            let rang = Zone::lue(n, format::Rang).valeur()?;
            let annee = if an.len() == 4 { Zone::lue(an, format::AnneeEnChiffres).valeur() } else { None }?;
            Some((rang, annee))
        });
        match lu {
            Some((rang, annee)) => Ok(NumeroDeBulletin { annee: annee?, rang }),
            None => Err(socle::refus!("« {} » n'est pas un numéro de bulletin : son rang, « / », puis l'année en quatre chiffres (12/2026)", texte)),
        }
    }

    /// Le premier bulletin de l'annee `annee` (`1/AAAA`).
    pub fn premier(annee: Annee) -> NumeroDeBulletin {
        NumeroDeBulletin { annee, rang: RangDansLAnnee::PREMIER }
    }

    /// Le bulletin qui le suit dans la suite de son annee.
    pub fn suivant(&self) -> NumeroDeBulletin {
        NumeroDeBulletin { annee: self.annee, rang: self.rang.suivant() }
    }

    /// Son annee.
    pub fn annee(&self) -> Annee {
        self.annee
    }
}

impl std::fmt::Display for NumeroDeBulletin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.rang, self.annee)
    }
}

impl From<NumeroDeBulletin> for String {
    fn from(n: NumeroDeBulletin) -> String {
        n.to_string()
    }
}

/// LE NUMERO D'UN AVENANT AU CONTRAT : 1 pour le premier, puis le suivant du dernier signe ; ecrit et lu comme un nombre.
/// Il ne se dit que dans une phrase ou un nom de fichier (`Display`). Aucun avenant n'est numéroté 0 : il ne se lit pas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct NumeroDAvenant(std::num::NonZeroU32);

impl NumeroDAvenant {
    /// Le premier avenant d'un contrat.
    pub fn premier() -> NumeroDAvenant {
        NumeroDAvenant(std::num::NonZeroU32::MIN)
    }

    /// L'avenant qui suit celui-ci.
    pub fn suivant(self) -> NumeroDAvenant {
        NumeroDAvenant(self.0.saturating_add(1))
    }
}

impl std::fmt::Display for NumeroDAvenant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
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

/// Le contrat d'un salarie dans un message : « le contrat VA5.26.01 de VARENNE », « le contrat de VARENNE » tant qu'il n'a
/// pas de numero.
pub fn le_contrat(numero: Option<&NumeroContrat>, salarie: &IdSalarie) -> String {
    numero_dit(numero, "le contrat ", &format!(" de {}", salarie), &format!("le contrat de {}", salarie))
}

/// LES NUMEROS DEJA PRIS d'une annee (`pris`) : le plus haut que chaque salarie a pris cette annee (la cle de sa fiche),
/// et tous ceux que portent les engagements, d'ici ou repris, ou sous lesquels quelque chose est parti, de qui que ce
/// soit -- ce que `former` ne redonne jamais.
pub struct Pris {
    annee: Annee,
    derniers: HashMap<IdSalarie, NumeroContrat>,
    tous: HashSet<NumeroContrat>,
}

/// Le numero de l'annee `pris.annee` pour le salarie `cle` de cette fiche : `<initiales><code>.<AA>.<rang>`, le rang
/// suivant le plus haut qu'il a pris cette annee (le premier sans aucun). Le seul qui fabrique un numero de contrat
/// (regle 4), et le seul calcul du rang suivant ; des initiales ou un code qui le sortiraient de la forme de la norme le
/// font refuser (`NumeroContrat::lire`). LE NUMERO FORME N'EST CELUI D'AUCUN AUTRE : les initiales et le matricule
/// d'aujourd'hui ont pu former, sous un autre salarie, un numero deja pris (un matricule change puis redonne, un numero
/// repris de l'ancien logiciel de la meme forme) ; le numero est la cle de ce qui est parti (regle 4), et le rang passe
/// alors au suivant que personne ne porte. Le numero donne entre dans `pris`.
pub fn former(fiche: &droit::fiche::ChampsSalarie, cle: &IdSalarie, pris: &mut Pris) -> R<NumeroContrat> {
    use droit::fiche::Lue;
    let annee = pris.annee;
    let mut rang = match pris.derniers.get(cle) {
        Some(d) => d.rang_en(annee).ok_or_else(|| socle::refus!("Défaut du logiciel : le contrat n° {} n'est pas un numéro de {} donné par le logiciel ; aucun numéro ne le suit.", d, annee))?.suivant(),
        None => RangDansLAnnee::PREMIER,
    };
    let an = annee.to_string();
    let tete = format!("{}{}.{}", fiche.initiales_contrat.ecrite("initiales_contrat")?, fiche.code_salarie.ecrite("code_salarie")?, &an[2.min(an.len())..]);
    let mut numero = NumeroContrat::lire(&format!("{}.{:02}", tete, rang))?;
    while pris.tous.contains(&numero) {
        rang = rang.suivant();
        numero = NumeroContrat::lire(&format!("{}.{:02}", tete, rang))?;
    }
    pris.derniers.insert(cle.clone(), numero.clone());
    pris.tous.insert(numero.clone());
    Ok(numero)
}

/// LES NUMEROS DE CONTRAT QUE LE MOTEUR A FORMES (`a_la_validation`, `numero_pour`), un par salarie pour qui il l'a
/// forme : chacun porte les initiales et le matricule que sa fiche avait alors (`former`). Le matricule d'un salarie qui
/// en a un est engage : le numero le dit, et il ne se recalcule pas (regle 4).
pub fn formes(racine: &std::path::Path) -> R<HashMap<IdSalarie, NumeroContrat>> {
    let mut out = HashMap::new();
    for o in stock::objets(racine)? {
        if let (Some(p), Some(n)) = (&o.brut.numero_pour, &o.carte().numero) {
            out.entry(p.salarie.clone()).or_insert_with(|| n.clone());
        }
    }
    Ok(out)
}

/// Les numeros pris en `annee` (`Pris`) : par salarie (la cle de sa fiche), le plus haut rang des numeros de cette annee
/// que portent ses engagements, ou sous lesquels quelque chose est parti pour lui (`departs`) ; et tous les numeros que
/// portent les engagements, ceux qu'une DSN de l'ancien logiciel a declares pour un contrat repris, et ceux sous lesquels
/// quelque chose est parti.
fn derniers_pris(ctx: &Contexte, annee: Annee, departs: &Departs) -> R<Pris> {
    derniers_pris_sauf(ctx, annee, None, departs)
}

/// La meme, sans l'engagement `sauf` (son fichier et son rang) : celui qui recoit un nouveau numero.
fn derniers_pris_sauf(ctx: &Contexte, annee: Annee, sauf: Option<(socle::mois::Mois, usize)>, departs: &Departs) -> R<Pris> {
    let mut pris = Pris { annee, derniers: HashMap::new(), tous: HashSet::new() };
    let mut noter = |cle: Option<IdSalarie>, numero: &NumeroContrat| {
        pris.tous.insert(numero.clone());
        let (Some(cle), Some(rang)) = (cle, numero.rang_en(annee)) else { return };
        if pris.derniers.get(&cle).and_then(|p| p.rang_en(annee)).is_none_or(|haut| rang > haut) {
            pris.derniers.insert(cle, numero.clone());
        }
    };
    for o in stock::objets(&ctx.racine)? {
        if sauf == Some((o.fichier, o.rang)) {
            continue;
        }
        if let Some(n) = &o.carte().numero {
            noter(o.carte().salarie.clone(), n);
        }
        for p in o.pieces().into_iter().flat_map(|p| p.paies.values()) {
            if let Some(n) = p.contrat.numero_declare() {
                noter(None, n);
            }
        }
    }
    for (cle, n) in departs.engagements() {
        noter(Some(cle.clone()), n);
    }
    Ok(pris)
}

/// LE PLUS HAUT NUMERO DE BULLETIN `N/AAAA` DE `annee` QU'UN BULLETIN IMPORTE IMPRIME (aucun sans aucun) : la suite des
/// bulletins de l'annee continue apres lui, et deux bulletins de la meme annee ne portent jamais le meme numero. Un
/// numero d'une autre forme n'est pas un numero de cette suite : il ne se lit pas comme tel.
pub fn dernier_bulletin_importe(racine: &std::path::Path, annee: Annee) -> R<Option<NumeroDeBulletin>> {
    let de_l_annee = |t: Option<&str>| -> Option<NumeroDeBulletin> {
        let n = match NumeroDeBulletin::lire(t?.trim()) {
            Ok(n) => n,
            Err(_) => return None,
        };
        (n.annee() == annee).then_some(n)
    };
    let mut haut = None;
    for i in crate::importe::tous(racine)? {
        let du_contrat = de_l_annee(i.contrat().and_then(|c| c.numero_bulletin.as_deref()));
        let des_paies = i.paies().values().filter_map(|p| de_l_annee(p.bulletin.as_ref().and_then(|b| b.numero.as_deref()))).max();
        haut = haut.into_iter().chain(du_contrat).chain(des_paies).max();
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
    let annee = debut.annee();
    let mut pris = derniers_pris_sauf(ctx, annee, Some((o.fichier, o.rang)), departs)?;
    let numero = former(&fiche, &cle, &mut pris)?;
    let e = o.brut_mut();
    e.saisie.carte.numero = Some(numero);
    e.numero_pour = Some(NumeroPour { salarie: cle.clone(), debut, le });
    Ok(())
}

/// Le numero de contrat que la paie `p`, sans numero, porterait : la suite des numeros `pris` pour son salarie dans
/// l'annee du debut du contrat (`derniers_pris`, lus une fois par annee), en memoire -- celui que sa validation lui
/// donnerait (`a_la_validation`).
fn proposer(ctx: &Contexte, p: &mut crate::calcul::Contenu, pris: &mut HashMap<Annee, Pris>, departs: &Departs) -> R<()> {
    let annee = p.engagement.contrat().debut().annee();
    let pris = match pris.entry(annee) {
        std::collections::hash_map::Entry::Occupied(x) => x.into_mut(),
        std::collections::hash_map::Entry::Vacant(x) => x.insert(derniers_pris(ctx, annee, departs)?),
    };
    let numero = former(&p.salarie, &p.engagement.salarie, pris)?;
    p.engagement.numero = Some(numero);
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
/// bulletin. Rend le dernier numero de bulletin donne, aucun quand aucune paie n'en a recu.
pub fn numeroter(ctx: &Contexte, paies: &mut [crate::calcul::Paie], prochain: NumeroDeBulletin, departs: &Departs) -> R<Option<NumeroDeBulletin>> {
    let mut n_bull = prochain;
    let mut donne = None;
    let mut pris = HashMap::new();
    for paie in paies.iter_mut() {
        let p = paie.ecrire();
        if p.engagement.numero.is_none() {
            proposer(ctx, p, &mut pris, departs)?;
        }
        if p.engagement.sans_paie() || p.numero_bulletin.is_some() {
            continue;
        }
        p.numero_bulletin = Some(n_bull);
        donne = Some(n_bull);
        n_bull = n_bull.suivant();
    }
    Ok(donne)
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
        assert_eq!(socle::ecriture::json_en_ligne(&n), r#""FA1.26.13""#.to_string());
        assert!(socle::forme::lire_texte::<NumeroContrat>(r#""C1""#, "numero").is_err());
        let fiche = |initiales: &str| -> droit::fiche::ChampsSalarie {
            socle::forme::lire_texte(&format!(r#"{{"initiales_contrat": "{}", "code_salarie": "1"}}"#, initiales), "fiche").unwrap()
        };
        let an: Annee = socle::forme::lire_texte("2026", "l'annee du test").unwrap();
        let cle: IdSalarie = socle::forme::lire_texte(r#""FAVART""#, "la cle du test").unwrap();
        let autre: IdSalarie = socle::forme::lire_texte(r#""FAGOTIN""#, "la cle du test").unwrap();
        let vide = || Pris { annee: an, derniers: HashMap::new(), tous: HashSet::new() };
        assert_eq!(former(&fiche("FA"), &cle, &mut vide()).ok().map(String::from), Some("FA1.26.01".to_string()));
        let mut pris = vide();
        pris.derniers.insert(cle.clone(), n.clone());
        assert_eq!(former(&fiche("FA"), &cle, &mut pris).ok().map(String::from), Some("FA1.26.14".to_string()));
        // Les numeros qu'un autre salarie porte deja ne se redonnent pas : le rang passe au suivant que personne ne porte.
        let mut pris = vide();
        pris.tous.extend(["FA1.26.01", "FA1.26.02"].map(|t| NumeroContrat::lire(t).unwrap()));
        assert_eq!(former(&fiche("FA"), &autre, &mut pris).ok().map(String::from), Some("FA1.26.03".to_string()));
        assert!(former(&fiche("F#"), &cle, &mut vide()).is_err());
    }
}
