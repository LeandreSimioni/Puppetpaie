//! POUR DEMARRER : ce qu'une compagnie neuve peut remplir d'avance, d'un coup.
//!
//! Une compagnie neuve ne decouvre pas ce qui lui manque un refus apres l'autre (le taux AT/MP, l'effectif, la medecine
//! du travail, le compte Urssaf...) : tout se dit d'un coup, ce que chaque case servira, OU elle vit, et ou la compagnie
//! TROUVE la valeur (l'aide du champ, `champs-base.json`). Une case y est exigee pour un geste ou utile pour un geste ; le refus reste a l'acte.
//!
//! Les regles ne sont pas recopiees : le bareme de l'annee se demande au contexte (`Contexte::bareme` : le taux AT/MP,
//! l'effectif, la premiere embauche), ce que chaque acte exige des fiches a `champs-base.json` (`exige_par`), que l'acte
//! lit aussi (`champs::refus_des_manques`) : un champ qu'on y marque sans qu'un acte le lise serait un faux manque.

use droit::fiche::Lue as _;
use serde::Serialize;

use droit::contexte::Contexte;
use droit::fiche::{CaseDeFiche, CaseEntreprise, ChampsEntreprise, NomDeFiche};
use crate::dsn_lue::PieceQuiLaPorte;
use socle::date::Jour;
use socle::echec::{Classe, R};

/// Le titre d'un spectacle que l'import n'a pas ecrit.
const TITRE_A_COMPLETER: &str = "A COMPLETER";

/// Où une ligne mène : ce que l'écran en dit (`ou`), et l'onglet de la Configuration qu'elle ouvre (`onglet`).
const ENTREPRISE: (&str, &str) = ("Configuration › Entreprise", "entreprise");

/// UNE LIGNE DE « POUR DEMARRER » : la case a remplir (`case`, son chemin dans la fiche, que l'ecran dessine ; `champ`, la
/// racine de ce chemin ; aucune pour ce qui ne se remplit pas dans une fiche), ou elle vit (`ou`, `onglet`), ou la
/// compagnie trouve la valeur (`trouver`). Elle dit ce qu'elle bloque (`bloque`), ou, quand rien ne se refuse sans elle,
/// ce qu'elle sert (`utile`) et ce qui se passe sinon (`sinon`). Celle d'un salarie ou d'un spectacle dit aussi qui
/// (`qui`), sous le nom duquel elle se range ; une structure dit ses cases (`cases`) ; une valeur que le moteur propose
/// se dit (`propose`). `route` y ajoute sa phrase entiere (`phrase`) et ce qu'elle sert (`pour`).
#[derive(Debug, Clone, Serialize)]
pub struct Manque {
    pub cle: String,
    pub libelle: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bloque: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub utile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sinon: Option<String>,
    pub ou: String,
    pub onglet: String,
    pub champ: Option<CaseDeFiche>,
    pub case: Option<crate::champs::CheminDeFiche>,
    pub trouver: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cases: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub salarie: Option<droit::contexte::IdSalarie>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spectacle: Option<droit::contexte::IdSpectacle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qui: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub propose: Option<ValeurProposee>,
    /// LES ANNEES DONT LA LIGNE DEMANDE L'EFFECTIF (`manques`) : chacune se dit par la route `effectif`, que
    /// `pour-demarrer` rend en geste avec ce que les paies reprises en donnent (`effectifs`, `routes::rappels`).
    #[serde(skip)]
    pub annees_d_effectif: Vec<socle::mois::Annee>,
    /// L'EFFECTIF QUE LES PAIES REPRISES DONNENT de ces annees (`engagement::effectif_repris`), que `route` calcule une
    /// fois pour toutes les lignes ; `pour-demarrer` le rend avec ses gestes.
    #[serde(skip)]
    pub effectifs: Vec<engagement::effectif_repris::EffectifRepris>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phrase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pour: Option<String>,
    /// La piece qui la porterait (une DSN deposee, que l'import reprend, ou aucune : elle se saisit), jugee par `manques`
    /// pour une case d'une fiche. Elle ne se dit que dans ce qu'une DSN pourrait donner (`route`, `ParUneDsn`) : ailleurs
    /// elle ne s'ecrit pas.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub piece: Option<PieceQuiLaPorte>,
}

/// UNE VALEUR QUE LE MOTEUR PROPOSE pour la case d'une ligne, que la compagnie valide ou corrige : elle s'ecrit comme la
/// case la tient, la page en remplit la case (un texte ; un jour en AAAA-MM-JJ), et se dit dans la phrase de la ligne
/// (`dite` : un jour en JJ/MM/AAAA).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum ValeurProposee {
    Texte(String),
    Jour(Jour),
}

impl ValeurProposee {
    /// Ce que la phrase en dit.
    fn dite(&self) -> String {
        match self {
            ValeurProposee::Texte(t) => t.clone(),
            ValeurProposee::Jour(j) => j.jj_mm_aaaa(),
        }
    }
}

impl Manque {
    /// Une ligne qui bloque `bloque`, sur la case `champ` (aucune : ce qui ne se remplit pas dans une fiche) ; aucune piece
    /// ne la porte, sauf a le dire (`portee_par`).
    fn bloquante(cle: &str, libelle: String, bloque: &str, ou: (&str, &str), champ: Option<CaseDeFiche>, trouver: String) -> Manque {
        Manque {
            cle: cle.to_string(),
            libelle,
            bloque: Some(bloque.to_string()),
            utile: None,
            sinon: None,
            ou: ou.0.to_string(),
            onglet: ou.1.to_string(),
            champ,
            case: champ.map(crate::champs::CheminDeFiche::de),
            trouver,
            cases: None,
            salarie: None,
            spectacle: None,
            qui: None,
            propose: None,
            annees_d_effectif: Vec::new(),
            effectifs: Vec::new(),
            phrase: None,
            pour: None,
            piece: None,
        }
    }

    /// Une ligne utile a `utile`, sans laquelle `sinon` ; aucune piece ne la porte, sauf a le dire (`portee_par`).
    pub fn utile(cle: String, libelle: String, utile: String, sinon: &str, ou: (&str, &str), champ: Option<CaseDeFiche>, trouver: String) -> Manque {
        Manque { bloque: None, utile: Some(utile), sinon: Some(sinon.to_string()), ..Manque::bloquante(&cle, libelle, "", ou, champ, trouver) }
    }

    /// La meme ligne, que `piece` porte.
    fn portee_par(self, piece: PieceQuiLaPorte) -> Manque {
        Manque { piece: Some(piece), ..self }
    }
}

/// CE QUE L'IMPORT A REPRIS, pour ranger ce qui manque (`route`) : rien (la compagnie n'a rien importe), des pieces sans
/// aucune DSN, ou une DSN au moins. C'est sans DSN que ce qu'une DSN porterait se dit a part : l'ajouter au dossier et
/// relancer l'import pourrait le remplir ; avec une DSN reprise, ce qui manque encore se saisit. Sans DSN, l'import dit
/// aussi les annees dont il a repris une paie (`annees` : une DSN de ces annees-la peut exister, et declarer leur taux
/// AT/MP) et les salaries dont un contrat repris attend sa nature (`natures_a_dire`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CeQueLImportARepris {
    Rien,
    SansDsn { annees: std::collections::BTreeSet<socle::mois::Annee>, natures_a_dire: Vec<droit::contexte::IdSalarie> },
    AvecDsn,
}

/// LA PHRASE DE CE QU'UNE DSN POURRAIT DONNER, quand l'import n'en a repris aucune.
const AJOUTEZ_UNE_DSN: &str =
    "Il nous manque encore quelques informations : une DSN déposée pourrait les donner. Ajoutez-en une au dossier de l'import, puis relancez l'import.";

/// CE QU'UNE DSN POURRAIT DONNER, quand l'import n'en a repris aucune : la phrase, et chaque ligne qu'elle remplirait.
#[derive(Debug, Clone, Serialize)]
pub struct ParUneDsn {
    pub phrase: String,
    pub manques: Vec<Manque>,
}

/// CE QUE LE MOTEUR A POSE DE LUI-MEME (`poses`), ou ce qui se dit en information a la place d'une ligne : son libelle,
/// d'ou il vient (`texte`), ou il se change.
#[derive(Debug, Clone, Serialize)]
pub struct Pose {
    pub cle: String,
    pub libelle: String,
    pub texte: String,
    pub ou: String,
    pub onglet: String,
    pub champ: Option<CaseDeFiche>,
}

/// `pour-demarrer` : sa phrase, chaque ligne et chaque pose en phrase entiere (`phrases`), son titre et sa note, les
/// lignes a saisir (`manques`), ce qu'une DSN pourrait donner (`dsn`, `route`), et ce que le moteur a pose (`poses`).
#[derive(Debug, Clone, Serialize)]
pub struct PourDemarrer {
    pub phrase: String,
    pub phrases: Vec<String>,
    pub titre: String,
    pub note: String,
    pub manques: Vec<Manque>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dsn: Option<ParUneDsn>,
    pub poses: Vec<Pose>,
}

/// Le champ `cle` de la fiche entreprise (`champs-base.json`), que cette table doit porter.
fn champ_entreprise(cle: impl Into<CaseDeFiche>) -> R<crate::champs::Champ> {
    let cle = cle.into();
    crate::champs::champ(NomDeFiche::Entreprise, cle)?.ok_or_else(|| socle::echec::livree(&crate::champs::BASE, &format!("entreprise.{}", cle)))
}

/// Le libelle et l'aide du champ `cle` de la fiche entreprise (`champs-base.json`).
fn champ(cle: impl Into<CaseDeFiche>) -> R<(String, String)> {
    let c = champ_entreprise(cle)?;
    Ok((c.libelle, c.aide))
}

/// Une ligne qui bloque, sur une case de la fiche entreprise ; une structure dit ses cases (leur nom, leur forme, leurs
/// valeurs), d'apres la table qui les decrit.
fn manque_de_la_case(cle: impl Into<CaseDeFiche>, libelle: String, bloque: &str, trouver: String) -> R<Manque> {
    let nom: CaseDeFiche = cle.into();
    let piece = PieceQuiLaPorte::de_la_case(NomDeFiche::Entreprise, &crate::champs::CheminDeFiche::de(nom));
    let mut l = Manque::bloquante(nom.nom(), libelle, bloque, ENTREPRISE, Some(nom), trouver).portee_par(piece);
    l.cases = champ_entreprise(nom)?.champs.as_deref().map(crate::champs::cases_dites);
    Ok(l)
}

/// LA LICENCE D'ENTREPRENEUR DE SPECTACLES est-elle exigee de la compagnie `ent` ? « Est entrepreneur de spectacles
/// vivants toute personne qui exerce une activite d'exploitation de lieux de spectacles, de production ou de diffusion de
/// spectacles » (L7122-2) ; il exerce « sous reserve de [...] declarer son activite », declaration qui « donne lieu a la
/// delivrance d'un recepisse [...] valant licence » (L7122-3) ; celui dont ce n'est pas l'activite principale declare ses
/// artistes et techniciens par le guichet unique, pas en DSN (L7122-22, L7122-23 ; `atelier/reference/sources/
/// code-du-travail/articles-20261023.md`, « L'entrepreneur de spectacles vivants et sa declaration »). La norme DSN
/// l'ecrit dans chaque contrat (S21.G00.40.049, `norme-dsn/texte-2027/bloc-S21.G00.40.txt` l. 1418-1426).
///
/// Deux faits la rendent exigee, lus ici pour « Pour demarrer » et pour la DSN :
/// - la convention dont c'est le champ (3090, 1285 ; et la compagnie qui n'a pas dit la sienne) : `exige_si` du champ
///   `licence_spectacle` (`champs-base.json`) ;
/// - dans une convention ou la case se montre (`si` : 1790, 3252), la compagnie engage pour un spectacle
///   (`spectacle` : un spectacle configure pour « Pour demarrer », un contrat de spectacle pour la DSN). Un lieu en 1790
///   sans spectacle n'a rien a dire ; l'audiovisuel ne voit pas la case.
pub fn licence_exigee(ent: &ChampsEntreprise, spectacle: bool) -> R<bool> {
    let c = champ_entreprise(CaseEntreprise::LicenceSpectacle)?;
    Ok(c.exige(ent) || (spectacle && crate::champs::condition_remplie(c.si.as_ref(), ent)))
}

/// La compagnie emploie-t-elle des intermittents ? Un spectacle actif le dit : un CDDU se rattache a un spectacle.
fn a_des_spectacles(ctx: &Contexte) -> bool {
    ctx.spectacles.values().any(|s| !s.archivee())
}

/// Les spectacles (leur cle) que porte un engagement actif, ni importe, dont la nature declare le numero d'objet.
fn spectacles_declares(ctx: &Contexte) -> R<Vec<droit::contexte::IdSpectacle>> {
    let mut out: Vec<droit::contexte::IdSpectacle> = Vec::new();
    for o in engagement::stock::objets(&ctx.racine)? {
        if o.est_non_actif() || o.importe_seul() {
            continue;
        }
        let nature = o.carte().nature;
        let Some(sp) = o.carte().spectacle.as_deref().filter(|s| !s.trim().is_empty()) else { continue };
        let cle = ctx.spectacle(sp)?.cle;
        let declare = nature.map(droit::natures::NatureDeContrat::proprietes).transpose()?.is_some_and(|n| n.ecrit(droit::rubrique!("S21.G00.40.050")));
        if declare && !out.contains(&cle) {
            out.push(cle);
        }
    }
    Ok(out)
}

/// Une suspension saisie (un imprevu du bloc 65) dont le motif n'est pas remunere, d'un salarie dont un engagement est
/// paye au mois : la cle de cet engagement (seul son calcul lit la sante d'une suspension,
/// `calcul::sante_de_la_suspension`).
fn une_suspension_non_remuneree(ctx: &Contexte) -> R<Option<droit::natures::AuMois>> {
    use engagement::imprevus::Suspend;
    for imp in engagement::imprevus::tous(&ctx.racine)? {
        let Some(code) = imp.suspension().map(|s| s.motif) else { continue };
        if !engagement::imprevus::proprietes_du_motif(code, &imp.salarie().to_string())?.remunere {
            if let Some(cle) = engagement::permanents::cle_du_salarie(ctx, imp.salarie())? {
                return Ok(Some(cle));
            }
        }
    }
    Ok(None)
}

/// La ligne d'une case vide qu'un acte exige ou qu'un geste sert (`champs::manquants`), ou elle vit, et la piece qui la
/// porterait (`piece`, jugee sur la case declaree) ; celle d'un salarie dit qui (`salarie` : sa cle et son nom).
fn ligne(m: crate::champs::Manquant, piece: PieceQuiLaPorte, ou: String, onglet: &str, salarie: Option<(&droit::contexte::IdSalarie, &str)>) -> Manque {
    let racine = m.ecran.champ;
    let (bloque, utile, sinon) = match m.sert {
        crate::champs::Sert::Exigee(actes) => (crate::champs::actes_en_clair(&actes), None, None),
        crate::champs::Sert::Utile { gestes, sinon } => (None, crate::champs::actes_en_clair(&gestes), sinon),
    };
    Manque {
        cle: match salarie {
            Some((s, _)) => format!("{}.{}", m.ecran, s),
            None => m.ecran.to_string(),
        },
        libelle: m.libelle,
        bloque,
        utile,
        sinon,
        ou,
        onglet: onglet.to_string(),
        champ: Some(racine),
        case: Some(m.ecran),
        trouver: m.aide,
        cases: m.cases,
        salarie: salarie.map(|(s, _)| s.clone()),
        spectacle: None,
        qui: salarie.map(|(_, n)| n.to_string()),
        propose: None,
        annees_d_effectif: Vec::new(),
        effectifs: Vec::new(),
        phrase: None,
        pour: None,
        piece: Some(piece),
    }
}

/// LA PIECE QUI PORTE LA CASE `cle` DE LA FICHE `f` (`PieceQuiLaPorte::de_la_case`). L'import ne complete une case que
/// vide (`routes::importation::completer`) : une case d'une structure deja entamee (une de ses cases dite, `dite`) ne
/// recoit rien d'une DSN, elle se saisit ; de meme un champ declare dont la forme lisible (`Manquant::ecran`) est deja
/// dite : il se derive d'elle a l'ecriture, l'import ne le complete pas (`completer::champs_a_completer`).
fn piece_de<F: droit::fiche::FicheSaisie>(f: &F, m: &crate::champs::Manquant) -> PieceQuiLaPorte {
    let entamee = m.cle.dans.is_some() && f.dite(m.cle.champ);
    let derivee = m.ecran.champ != m.cle.champ && f.dite(m.ecran.champ);
    if entamee || derivee {
        PieceQuiLaPorte::Aucune
    } else {
        PieceQuiLaPorte::de_la_case(F::NOM, &m.cle)
    }
}

/// Tout ce qui manque a la compagnie pour ses premiers actes. Vide : rien ne manque. Une ligne dit ce qu'elle bloque
/// (`bloque`), ou, quand rien ne se refuse sans elle, ce qu'elle sert (`utile`) et ce qui se passe sinon (`sinon`).
pub fn manques(ctx: &Contexte) -> R<Vec<Manque>> {
    let ent = &ctx.entreprise;
    let mut out: Vec<Manque> = Vec::new();
    let intermittents = a_des_spectacles(ctx);
    // CE QUE LES ACTES EXIGENT DES FICHES (`exige_par` de `champs-base.json`, que chaque acte lit aussi :
    // `champs::refus_des_manques`) : l'entreprise, puis chaque salarie actif ; ce qui ne bloque rien dit ce qui se passe
    // sans lui (`sinon`).
    for m in crate::champs::manquants(ent)? {
        if m.cle.seul() == Some(documents::conges_spectacles::CLE_ADHERENT.into()) && !intermittents {
            continue;
        }
        let piece = piece_de(ent, &m);
        out.push(ligne(m, piece, ENTREPRISE.0.to_string(), ENTREPRISE.1, None));
    }
    for (cle, s) in ctx.salaries.iter().filter(|(_, s)| !s.archivee()) {
        let nom = match s.nomme() {
            Some(n) => n,
            None => cle.to_string(),
        };
        // L'import rattache un salarie a sa DSN par son NIR (`routes::import`, « Les salaries, par NIR ») : une fiche
        // sans NIR ne recoit rien d'une DSN, ce qui lui manque se saisit.
        let rattache = s.nir.dite().is_some();
        for m in crate::champs::manquants(&s.champs)? {
            let piece = if rattache { piece_de(&s.champs, &m) } else { PieceQuiLaPorte::Aucune };
            out.push(ligne(m, piece, format!("Configuration › Salariés, la fiche de {}", nom), "salarie", Some((cle, &nom))));
        }
    }
    // Un spectacle ne se dit que si un engagement dont la nature declare le numero d'objet (S21.G00.40.050, `champs` de
    // sa nature dans `contrats-base.json`) le porte : rien de remplissable s'il n'y a pas a le remplir.
    for cle in spectacles_declares(ctx)? {
        let Some(sp) = ctx.spectacles.get(&cle) else { continue };
        let titre = match sp.titre.remplie() {
            Some(t) => t.to_string(),
            None => cle.to_string(),
        };
        for m in crate::champs::manquants(sp)? {
            let piece = piece_de(sp, &m);
            let mut l = ligne(m, piece, format!("Configuration › Spectacles, la fiche de {}", titre), "spectacle", None);
            l.cle = format!("{}.{}", l.cle, cle);
            l.spectacle = Some(cle.clone());
            l.qui = Some(titre.clone());
            out.push(l);
        }
    }
    // LE TITRE D'UN SPECTACLE QUE L'IMPORT N'A PAS ECRIT (« A COMPLETER » sur une fiche active : le contrat et le bulletin
    // l'imprimeraient tel quel) : la case se demande, avec le titre que les bulletins de l'ancien logiciel portent
    // (la note `titre_propose` de sa fiche), a valider ou a corriger, et le numero d'objet auquel il correspond.
    for (cle, sp) in ctx.spectacles.iter().filter(|(_, s)| !s.archivee()) {
        let lu = |t: Option<&str>| t.map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
        if !lu(sp.titre.remplie()).is_none_or(|t| t.eq_ignore_ascii_case(TITRE_A_COMPLETER)) {
            continue;
        }
        let objet = match lu(sp.numero_objet.remplie()) {
            Some(o) => o,
            None => cle.to_string(),
        };
        let propose = lu(sp.titre_propose.as_deref());
        let trouver = match &propose {
            Some(t) => {
                let ou_lu: String = lu(sp.titre_lu_dans.as_deref()).iter().map(|d| format!(" dans la case « {} »", d)).collect();
                format!("Proposé : « {} », lu{} des bulletins de l'ancien logiciel pour le numéro d'objet {}. Validez-le ou corrigez-le.", t, ou_lu, objet)
            }
            None => format!("Aucune pièce reprise ne porte le titre du numéro d'objet {} : celui que vous avez déclaré à France Travail.", objet),
        };
        let mut l = Manque::utile(
            format!("titre.{}", cle),
            "Titre".into(),
            "les contrats et les bulletins".into(),
            "ils impriment « A COMPLETER »",
            (&format!("Configuration › Spectacles, la fiche du numéro d'objet {}", objet), "spectacle"),
            Some(droit::fiche::CaseSpectacle::Titre.into()),
            trouver,
        );
        l.spectacle = Some(cle.clone());
        l.qui = Some(format!("Numéro d'objet {}", objet));
        l.propose = propose.map(ValeurProposee::Texte);
        out.push(l);
    }
    // La licence d'entrepreneur de spectacles : la DSN l'ecrit dans chaque contrat (S21.G00.40.049) et refuse sans elle
    // QUAND ELLE EST EXIGEE de la compagnie (`exige_si` de `champs-base.json`, lu par `dsn::licence_du_contrat` aussi) :
    // en 1790 et en 3252 la case se montre sans etre exigee, dans l'audiovisuel elle ne se montre pas.
    // Elle peut etre datee (une par periode) : le contexte la resout au jour.
    if ctx.licence(&ctx.aujourdhui).is_none_or(|l| l.trim().is_empty()) && licence_exigee(ent, intermittents)? {
        let (libelle, aide) = champ(CaseEntreprise::LicenceSpectacle)?;
        out.push(manque_de_la_case(CaseEntreprise::LicenceSpectacle, libelle, "la DSN", aide)?);
    }
    // L'EFFECTIF, puis le reste du bareme de l'annee : le contexte refuse lui-meme, avec la phrase qui dit ou trouver ce
    // qui manque. L'effectif a part : le bareme s'arrete au premier refus (le taux AT/MP), et tout doit sortir d'un coup.
    // La date de la premiere embauche n'est demandee que quand l'effectif ne se connait pas autrement : un seuil d'effectif
    // n'est franchi qu'apres « cinq années civiles consécutives » (CSS, art. L. 130-1, II,
    // `atelier/reference/sources/code-de-la-securite-sociale/articles-20261011.md` l. 296), et des taux de chaque paie en
    // dependent (onze et cinquante salaries) : elle est exigee pour l'effectif de la paie.
    // Une compagnie plus ancienne dit l'effectif de chaque annee qui manque (`EffectifsParAnnee::a_dire`), tel que l'Urssaf l'a
    // calcule (L. 130-1, I) : celle qui arrive d'un autre logiciel n'a pas ses comptes rendus de nature 118.
    let an = ctx.aujourdhui.annee();
    let avant = an.precedente()?;
    // Chaque ligne qui demande l'effectif dit ses annees (`annees_d_effectif`) : chacune se dit d'un geste (route
    // `effectif`), et ce que les paies reprises en donnent s'y propose (`route`). Ce que l'Urssaf a renvoye et ce que la
    // compagnie a dit ne se demandent pas : ils ne se proposent pas.
    let effectif = ctx.effectif()?;
    if !effectif.a_dire(an).is_empty() && effectif.premiere_embauche.is_none() {
        let (libelle, _) = champ(CaseEntreprise::PremiereEmbauche)?;
        let m = match effectif.atteint(socle::grandeurs::Effectif::personnes(11), an) {
            Err(m) => m.phrase,
            Ok(_) => format!("L'effectif de {} n'est pas connu.", avant),
        };
        let mut l = manque_de_la_case(CaseEntreprise::PremiereEmbauche, libelle, "l'effectif de toute paie (seuils de 11 et 50 salariés)", m)?;
        l.annees_d_effectif = effectif.a_dire(an);
        out.push(l);
    } else if let Some(manquent) = annees_d_avant_a_dire(&effectif, an) {
        let annees = manquent.iter().map(socle::mois::Annee::to_string).collect::<Vec<_>>().join(", ");
        let mut l = Manque::bloquante(
            "effectif",
            format!("L'effectif de {}", annees),
            "l'effectif de toute paie (seuils de 11 et 50 salariés)",
            ENTREPRISE,
            None,
            "Ces années sont d'avant PuppetPaie : leur effectif se dit tel que l'Urssaf l'a calculé (l'effectif annuel moyen de l'année, sur le compte rendu « Effectifs » de nature 118 reçu au début de l'année suivante), une année à la fois. Le compte rendu 118 de l'Urssaf le remplace à son arrivée.".to_string(),
        );
        l.annees_d_effectif = manquent;
        out.push(l);
    }
    // LE CSE, la ou la convention en fait dependre une contribution (`par_representation` : le FNAS de la CCNEAC,
    // article III.3.1) et seulement quand l'effectif de la paie compte au moins onze salaries : en dessous, la question
    // ne se pose pas (III.3.1.b, `droit::bareme::representation_du_personnel`). Il ne se lit qu'avec la cle de l'effectif
    // (`droit::contexte::EffectifsParAnnee::cse`). Un bareme qui ne se lit pas se dit plus bas, ce qui lui manque.
    let bareme = ctx.bareme(&ctx.aujourdhui);
    let cle = droit::fiche::CasePieceEntreprise::RepresentationPersonnel;
    // C'est l'effectif de la paie (celui de l'annee d'avant, L. 130-1, I) qui decide (III.3.1.a et b) : inconnu, c'est
    // lui que la contribution demande, pas le CSE.
    match bareme {
        Ok(b) if par_representation(&b) => {
            let cse = effectif.cse(an);
            let dit = cse.as_ref().is_some_and(|k| ent.cse.ouvrir(k).representation_personnel.est_dite());
            match effectif.valeur(an) {
                Some(e) if e >= socle::grandeurs::Effectif::personnes(11) && cse.is_some() && !dit => {
                    let (libelle, aide) = champ(cle)?;
                    out.push(manque_de_la_case(cle, libelle, "la contribution au FNAS de chaque paie", aide)?);
                }
                // Sans la cle, l'effectif d'une des cinq annees d'avant manque : `a_dire` l'a demande plus haut.
                Some(_) => {}
                None => {
                    let mut l = Manque::bloquante(
                        "effectif",
                        format!("L'effectif de {}", avant),
                        "la contribution au FNAS de chaque paie",
                        ENTREPRISE,
                        None,
                        format!("La CCNEAC fixe la contribution au FNAS par l'effectif (article III.3.1) : celui de {} tel que l'Urssaf l'a calculé (compte rendu « Effectifs » de nature 118).", avant),
                    );
                    l.annees_d_effectif = vec![avant];
                    out.push(l);
                }
            }
        }
        Ok(_) => {}
        // L'effectif qui manque est demande plus haut.
        Err(e) => match e.classe {
            Some(Classe::TauxAtmpAbsent) => out.push(manque_de_la_case(CaseEntreprise::TauxAtmp, format!("Le taux AT/MP {}", an), "toute paie", e.phrase)?),
            Some(Classe::EffectifAbsent) => {}
            _ => out.push(Manque::bloquante("bareme", format!("Le barème {}", an), "toute paie", ENTREPRISE, None, e.phrase)),
        },
    }
    // La sante d'un permanent suspendu sans indemnisation tout un mois : hors de la 3090, aucun texte ne la dit, et le mois
    // refuse tant que la fiche ne repond pas (`calcul::sante_de_la_suspension`). Elle ne se demande que si une suspension
    // non remuneree est saisie : un champ n'apparait que parce qu'un fait l'exige.
    let dite = |cle: &droit::natures::AuMois| ent.permanent.ouvrir(cle).sante_suspension_non_indemnisee.est_dite();
    if ent.idcc.dite() != Some(&droit::fiche::IDCC_3090) && une_suspension_non_remuneree(ctx)?.is_some_and(|cle| !dite(&cle)) {
        let cle = droit::bareme::SANTE_SUSPENSION;
        let (libelle, aide) = champ(cle)?;
        out.push(manque_de_la_case(cle, libelle, "le mois d'un permanent suspendu sans indemnisation tout le mois", aide)?);
    }
    // LA TVA ET LA TAXE SUR LES SALAIRES : chaque DSN mensuelle declare la base de chaque salarie au taux normal quand la
    // compagnie la doit (S21.G00.79 de type 91), celle de decembre si elle la doit (bloc S21.G00.44) ; sans reponse, la DSN
    // d'un mois qui a des paies ne se fait pas (`taxe_sur_les_salaires::regime_taxe_sur_les_salaires`).
    if let documents::taxe_sur_les_salaires::Decision::ADire(m) = documents::taxe_sur_les_salaires::regime_taxe_sur_les_salaires(ent, None).decision {
        let cle = documents::taxe_sur_les_salaires::CHAMP_TAXE_SUR_LES_SALAIRES;
        let (libelle, _) = champ(cle)?;
        out.push(manque_de_la_case(cle, libelle, "la DSN du mois", m)?);
    }
    // LES CONTRATS DES PERMANENTS : c'est tout ce que la compagnie a a saisir des organismes (le reste, les textes le
    // donnent : `droit::organismes`). Sans eux, le mois d'un permanent se refuse (`dsn::affiliations`).
    // Le mois que « Pour demarrer » prepare : celui du jour.
    let mois = ctx.aujourdhui.mois_civil();
    let permanents = engagement::permanents::Permanents::de(ctx, &socle::date::Periode::du_mois(mois))?;
    if permanents.ouverts() && ctx.organismes()?.par_nature.permanent.references.is_empty() {
        out.push(Manque::bloquante(
            "organismes_permanents",
            "Les contrats de prévoyance et de santé des permanents".into(),
            "le mois d'un salarié au mois",
            ("Configuration › Organismes et contrats", "organismes"),
            None,
            "La compagnie emploie des salariés au mois : leurs contrats de prévoyance et de santé se disent avec ceux des intermittents. La référence du contrat se lit sur le certificat d'adhésion de l'assureur.".into(),
        // Aucune piece : la DSN dit la reference d'une adhesion, pas ce qu'elle couvre (S21.G00.15) ; l'import ne la
        // pose sur un contrat que par inference, quand elle est seule ou que son libelle nomme son role
        // (`routes::importation::parametrage::poser_organismes`).
        ));
    }
    // LES CONGES PAYES ACQUIS AVANT PUPPETPAIE : un permanent dont le contrat commence avant le premier mois ou PuppetPaie
    // le paie a acquis des jours ailleurs (L3141-3 du code du travail, `atelier/reference/sources/code-du-travail/
    // articles-20260919.md`) ; un solde ne se devine pas (regle 1). Sa fiche les dit (`conges_reprise`, que
    // `engagement::conges::reprise_conges` lit). Rien ne se refuse sans eux : la ligne dit ce qui se passe sinon (`sinon`),
    // et non ce qu'elle bloque. C'est ici seulement que la question se pose, contrat repris d'un import compris.
    for (salarie, nom, depuis) in conges_a_reprendre(ctx)? {
        out.push(conges_a_dire(&salarie, &nom, depuis));
    }
    Ok(out)
}

/// Les annees d'avant PuppetPaie dont l'effectif manque (`EffectifsParAnnee::a_dire`) quand la premiere embauche est dite, et
/// qu'elle ne remplace donc pas ; aucune sinon.
fn annees_d_avant_a_dire(effectif: &droit::contexte::EffectifsParAnnee, an: socle::mois::Annee) -> Option<Vec<socle::mois::Annee>> {
    let manquent = effectif.a_dire(an);
    (!manquent.is_empty() && effectif.premiere_embauche.is_some()).then_some(manquent)
}

/// Le bareme fait-il dependre une contribution de la representation du personnel (`par_representation` : le FNAS de la
/// CCNEAC, article III.3.1) ?
fn par_representation(b: &droit::forme_bareme::Pile) -> bool {
    b.contributions_conventionnelles.iter().flatten().any(|c| c.par_representation.as_ref().is_some_and(|p| !p.is_empty()))
}

/// L'EFFECTIF QUE « POUR DEMARRER » DEMANDE (sa ligne `effectif`, `manques`) : celui des annees d'avant PuppetPaie qui
/// manquent (`annees_d_avant_a_dire`), ou celui de l'annee d'avant que la contribution au FNAS lit quand il n'est pas
/// connu.
pub fn effectif_a_dire(ctx: &Contexte) -> R<bool> {
    let an = ctx.aujourdhui.annee();
    let effectif = ctx.effectif()?;
    if annees_d_avant_a_dire(&effectif, an).is_some() {
        return Ok(true);
    }
    Ok(match ctx.bareme(&ctx.aujourdhui) {
        Ok(b) => par_representation(&b) && effectif.valeur(an).is_none(),
        // Un bareme qui ne se lit pas a sa propre ligne (`bareme`, `manques`) : ce n'est pas l'effectif qu'elle demande.
        Err(_) => false,
    })
}

/// LA LIGNE QUI DEMANDE LES CONGES ACQUIS AVANT PUPPETPAIE d'un salarie au mois (`salarie`, `nom` tel que l'ecran le dit),
/// paye ici depuis `depuis` : la case `conges_reprise` de sa fiche, ou elle mene. « Pour démarrer » la dit, et la carte
/// d'un contrat commence avant le logiciel la montre avant sa validation (`routes::mois`).
pub fn conges_a_dire(salarie: &droit::contexte::IdSalarie, nom: &str, depuis: Jour) -> Manque {
    let le = depuis.jj_mm_aaaa();
    let mut l = Manque::utile(
        format!("conges_reprise.{}", salarie),
        format!("Ses congés payés acquis avant le {}", le),
        "le compteur de ses congés payés".into(),
        "il part de zéro",
        (&format!("la fiche de {}, « Congés payés à la reprise »", nom), "salarie"),
        Some(droit::fiche::CasePieceSalarie::CongesReprise.into()),
        format!("Son contrat a commencé avant que PuppetPaie le paie : recopiez de son dernier bulletin de votre ancien logiciel les jours acquis et pris, avec le {} pour date de reprise.", le),
    );
    l.qui = Some(nom.to_string());
    l.salarie = Some(salarie.clone());
    l
}

/// La reprise des conges de ce salarie est-elle deja dite sur sa fiche (`conges_reprise.date`) ?
pub fn conges_repris(ctx: &Contexte, au_mois: &droit::natures::AuMois, salarie: &droit::contexte::IdSalarie) -> bool {
    ctx.salaries.get(salarie).and_then(|f| f.permanent.ouvrir(au_mois).conges_reprise.dite()).is_some_and(|c| c.date.est_dite())
}

/// Les permanents dont un contrat commence avant le premier mois ou PuppetPaie le paie, et dont la fiche ne dit pas les
/// conges a la reprise (`conges_reprise.date`) : `(salarie, nom, premier jour paye ici)`. Le premier mois paye ici est
/// celui du fichier de l'engagement ; un contrat repris d'un ancien logiciel qui continue ici y est des le premier mois
/// neuf (`engagement::importe`), valide ou non. Un intermittent n'en est pas (ses conges sont ceux des Conges
/// Spectacles) ; un contrat ne d'ici n'a rien d'avant.
fn conges_a_reprendre(ctx: &Contexte) -> R<Vec<(droit::contexte::IdSalarie, String, Jour)>> {
    // (cle des cases d'un salarie au mois, salarie, debut du contrat, premier jour paye ici)
    let mut contrats: Vec<(droit::natures::AuMois, droit::contexte::IdSalarie, Jour, Jour)> = Vec::new();
    let mut vus: Vec<droit::contexte::IdSalarie> = Vec::new();
    // Les engagements valides et actifs, chacun dans le mois de son fichier (`engagement::stock::objets`) : un brouillon
    // n'engage rien, un contrat repris qui continue ici si.
    for o in engagement::stock::objets(&ctx.racine)? {
        if !(o.est_valide() || o.est_importe()) || o.est_non_actif() || o.importe_seul() {
            continue;
        }
        let e = o.carte();
        let Some(salarie) = e.salarie.clone() else { continue };
        let (Some(debut), Some(nature)) = (e.debut, e.nature) else { continue };
        let Some(au) = nature.proprietes()?.au_mois().filter(|_| !vus.contains(&salarie)) else {
            continue;
        };
        vus.push(salarie.clone());
        if engagement::stock::commence_avant_le_logiciel(&ctx.racine, debut, o.premier_mois())? {
            contrats.push((au, salarie, debut, o.premier_mois().premier_jour()));
        }
    }
    let mut out = Vec::new();
    for (au, salarie, debut, premier) in contrats {
        if debut >= premier || conges_repris(ctx, &au, &salarie) {
            continue;
        }
        let nom = match ctx.salaries.get(&salarie).map(|f| documents::tableaux_export::qui_souple(f)).filter(|n| !n.is_empty()) {
            Some(n) => n,
            None => salarie.to_string(),
        };
        out.push((salarie, nom, premier));
    }
    Ok(out)
}

/// CE QUE LE MOTEUR A INSCRIT DE LUI-MEME, et qui se dit avec ce qui manque : le taux AT/MP de l'annee, quand il vient de
/// la notification de la CARSAT recue avec les retours (`routes::atmp::recue`, avec sa date) ou du taux collectif
/// de l'arrete pour un risque « TC » (`routes::atmp::inscrire_l_arrete`), d'apres sa `provenance`. Le libelle est celui de
/// l'arrete de l'annee (`droit::bareme::risques_atmp`, `atelier/reference/sources/tarification-atmp/`).
pub fn poses(ctx: &Contexte) -> R<Vec<Pose>> {
    let an = ctx.aujourdhui.annee();
    // Sans taux AT/MP de l'annee, le moteur n'en a pose aucun ; un taux qui ne se lit pas se refuse.
    Ok(ctx.taux_atmp_de(an).transpose()?.map(|t| pose_atmp(an, t)).transpose()?.flatten().into_iter().collect())
}

/// Le taux AT/MP `t` de l'annee `an`, s'il est pose par le moteur (sa `provenance`).
fn pose_atmp(an: socle::mois::Annee, t: droit::fiche::TauxAtmp) -> R<Option<Pose>> {
    let d_ou = match (&t.notification, t.provenance) {
        (Some(n), Some(droit::fiche::ProvenanceAtmp::Notification)) => format!(
            "Ceux de la notification de la CARSAT du {}, en vigueur depuis le {} : le logiciel les a lus dans les retours de la DSN.",
            // Sans jour de calcul, celui de la notification.
            match &n.calcul {
                Some(c) => c,
                None => n.le.exigee("taux_atmp.notification.le")?,
            }
            .jj_mm_aaaa(),
            n.effet.exigee("taux_atmp.notification.effet")?.jj_mm_aaaa()
        ),
        (_, Some(droit::fiche::ProvenanceAtmp::Arrete)) => format!(
            "Le taux collectif de l'arrêté de {} pour ce risque « TC », le même pour toutes les compagnies qui y sont classées (code de la sécurité sociale, art. D. 242-6-14) : le logiciel l'a inscrit. Si votre notification AT/MP (compte AT/MP sur net-entreprises) en dit un autre, inscrivez-le.",
            t.depuis.exigee("taux_atmp.depuis")?.annee()
        ),
        // Un taux que la compagnie a inscrit elle-meme : le moteur n'a rien pose.
        _ => return Ok(None),
    };
    let risque = match &t.code_risque {
        Some(code) => droit::contexte::socle_livre_lu(an)?.and_then(|s| droit::bareme::risques_de(&s).into_iter().find(|r| r.code == *code)),
        None => None,
    };
    let nom = match (&risque, &t.code_risque) {
        (Some(droit::bareme::RisqueAtmp { code_arrete: Some(c), .. }), _) => c.a_l_arrete(),
        (Some(r), _) => r.code.to_string(),
        (None, code) => code.exigee("taux_atmp.code_risque")?.to_string(),
    };
    let taux: String = t.valeur.iter().map(|v| format!(", taux {}", socle::texte::pourcent(*v))).collect();
    let de: String = risque.iter().map(|r| format!("« {} » ", r.libelle)).collect();
    Ok(Some(Pose {
        cle: "taux_atmp".into(),
        libelle: format!("Accidents du travail : code risque {}{} en {}", nom, taux, an),
        texte: format!("{}{}", de, d_ou),
        ou: ENTREPRISE.0.into(),
        onglet: ENTREPRISE.1.into(),
        champ: Some(CaseEntreprise::TauxAtmp.into()),
    }))
}

/// LA NATURE D'UN CONTRAT DE PERMANENT REPRIS, A DIRE (`routes::importation::nature`) : un bulletin ne dit pas si c'est un
/// CDI ou un CDD ; la DSN deposee la declare (S21.G00.40.007), et l'import qui la reprend la dit de lui-meme
/// (`nature::dire_depuis_les_dsn`), pour le salarie qu'elle atteint : celui dont la fiche a un NIR. Ces lignes ne se
/// disent que dans ce qu'une DSN pourrait donner (`route`) ; la nature reste a dire a sa place, et un salarie sans NIR
/// n'y est pas.
fn natures_par_une_dsn(ctx: &Contexte, salaries: &[droit::contexte::IdSalarie]) -> Vec<Manque> {
    let mut out = Vec::new();
    for cle in salaries {
        let Some(s) = ctx.salaries.get(cle).filter(|s| !s.archivee() && s.nir.dite().is_some()) else { continue };
        let nom = match s.nomme() {
            Some(n) => n,
            None => cle.to_string(),
        };
        let mut l = Manque::bloquante(
            &format!("nature.{}", cle),
            "Nature du contrat (CDI ou CDD)".into(),
            "la DSN et les documents de fin de contrat",
            (&format!("Configuration › Reprendre l'historique, le contrat de {}", nom), "importer"),
            None,
            "Un bulletin ne dit pas si le contrat est un CDI ou un CDD ; la DSN déposée le déclare.".into(),
        )
        .portee_par(PieceQuiLaPorte::Dsn);
        l.qui = Some(nom);
        out.push(l);
    }
    out
}

/// `pour-demarrer`. `proposes` : les propositions qui ne se tirent pas des fiches (les acces aux organismes, que le
/// moteur lit dans le coffre), a la suite ; `pose` : ce qui se dit en information a leur place, apres ce que le moteur a
/// pose. Une case est EXIGEE POUR un geste (sans elle, il ne se fait pas : `bloque`) ou UTILE POUR un geste (sans elle,
/// il se fait autrement : `utile`, `sinon`) ; chaque ligne le dit (`pour`), et sa phrase entiere dit aussi ou elle vit
/// et de qui (`phrase`). Le refus reste a l'acte (`champs::refus_des_manques`).
///
/// CE QU'UNE DSN DONNERAIT SE DIT A PART (`dsn`) quand l'import a repris des pieces sans aucune DSN (`repris`) : chaque
/// ligne qu'une DSN deposee porte (`Manque::piece`) y passe, sous la phrase qui dit de l'ajouter au dossier et de
/// relancer l'import ; `manques` ne garde que ce qui se saisit. Sans import, ou avec une DSN reprise, tout se saisit.
/// Le titre compte toutes les cases exigees, ou qu'elles soient rangees.
pub fn route(ctx: &Contexte, proposes: Vec<Manque>, pose: Option<Pose>, repris: CeQueLImportARepris) -> R<PourDemarrer> {
    let mut lignes = manques(ctx)?;
    lignes.extend(proposes);
    if let CeQueLImportARepris::SansDsn { annees, natures_a_dire } = &repris {
        // Le taux AT/MP de l'annee : une DSN ne declare que celui de sa propre annee (`routes::importation::parametrage::
        // poser_atmp`). Il ne se range sous la DSN que si une DSN de cette annee peut exister : l'import en a repris
        // une paie. Une compagnie qui change de logiciel en janvier n'a que des DSN de l'annee d'avant.
        let an = ctx.aujourdhui.annee();
        let atmp = Some(CaseDeFiche::from(CaseEntreprise::TauxAtmp));
        for l in lignes.iter_mut().filter(|l| l.qui.is_none() && l.champ == atmp && !annees.contains(&an)) {
            l.piece = Some(PieceQuiLaPorte::Aucune);
        }
        lignes.extend(natures_par_une_dsn(ctx, natures_a_dire));
    }
    // L'EFFECTIF QUE LES PAIES REPRISES DONNENT (`engagement::effectif_repris`), calcule une fois pour les annees que les
    // lignes demandent, et rendu a chacune pour les siennes ; avec lui, la date de la premiere embauche que les pieces
    // reprises disent, proposee dans sa case (la compagnie la valide ou la corrige : rien ne s'ecrit sans son geste).
    let mut demandees: Vec<socle::mois::Annee> = lignes.iter().flat_map(|l| l.annees_d_effectif.iter().copied()).collect();
    demandees.sort();
    demandees.dedup();
    let premiere_embauche = Some(CaseDeFiche::from(CaseEntreprise::PremiereEmbauche));
    // D'ou vient la date proposee pour la premiere embauche : la ligne de sa case le dit, a la page comme en ligne de commande.
    let mut d_ou_l_embauche: Option<String> = None;
    if !demandees.is_empty() {
        let (repris, embauche) = engagement::effectif_repris::effectifs_repris(ctx, &demandees)?;
        d_ou_l_embauche = embauche.as_ref().map(|e| e.d_ou.clone());
        for l in lignes.iter_mut() {
            l.effectifs = repris.iter().filter(|e| l.annees_d_effectif.contains(&e.annee)).cloned().collect();
            if l.qui.is_none() && l.champ == premiere_embauche {
                l.propose = embauche.as_ref().map(|e| ValeurProposee::Jour(e.jour));
            }
        }
    }
    for l in lignes.iter_mut() {
        let pour = match (&l.bloque, &l.utile, &l.sinon) {
            (Some(b), _, _) => format!("case exigée pour {}", b),
            (None, Some(u), Some(s)) => format!("utile pour {} ; sinon, {}", u, s),
            _ => return Err(socle::refus!("La ligne « {} » de « Pour démarrer » ne dit ni ce qu'elle bloque ni ce qu'elle sert : le signaler au développeur.", l.libelle)),
        };
        // Une valeur que le moteur propose (`propose`) se dit dans la phrase : la ligne de commande la lit comme l'ecran.
        let de_l_embauche = match (&l.propose, &d_ou_l_embauche) {
            (Some(p), Some(d)) if l.qui.is_none() && l.champ == premiere_embauche => Some(format!(" Proposé : « {} », {}. Validez-la ou corrigez-la.", p.dite(), d)),
            _ => None,
        };
        let propose: String = match &de_l_embauche {
            Some(p) => p.clone(),
            None => l.propose.iter().map(|p| format!(" Proposé : « {} ».", p.dite())).collect(),
        };
        // L'effectif que les paies reprises donnent, annee par annee, ou ce qui manque pour le calculer.
        let effectifs: String = l.effectifs.iter().map(|e| format!(" {}", e.phrase)).collect();
        l.phrase = Some(format!("{} › {} : {}.{}{}", l.ou, l.libelle, pour, propose, effectifs));
        let mut c = pour.chars();
        let initiale: String = c.next().into_iter().flat_map(char::to_uppercase).collect();
        // La page montre `pour` sous la case : la provenance de la date proposee s'y lit.
        l.pour = Some(match de_l_embauche {
            Some(d) => format!("{}{}.{}", initiale, c.as_str(), d),
            None => format!("{}{}.", initiale, c.as_str()),
        });
    }
    let (par_une_dsn, mut lignes): (Vec<Manque>, Vec<Manque>) = match repris {
        CeQueLImportARepris::SansDsn { .. } => lignes.into_iter().partition(|l| l.piece == Some(PieceQuiLaPorte::Dsn)),
        CeQueLImportARepris::Rien | CeQueLImportARepris::AvecDsn => (Vec::new(), lignes),
    };
    // La piece ne se dit que dans ce qu'une DSN pourrait donner.
    for l in lignes.iter_mut() {
        l.piece = None;
    }
    let dsn = (!par_une_dsn.is_empty()).then(|| ParUneDsn { phrase: AJOUTEZ_UNE_DSN.to_string(), manques: par_une_dsn });
    let n = lignes.iter().chain(dsn.iter().flat_map(|d| &d.manques)).filter(|l| l.bloque.is_some()).count();
    let (titre, note) = match (n, lignes.is_empty()) {
        (0, true) if dsn.is_some() => ("Pour démarrer : une DSN pourrait donner ce qui manque".to_string(), ""),
        (0, true) => ("Pour démarrer : rien à remplir".to_string(), ""),
        (0, false) => ("Pour démarrer : tout ce qui est exigé est rempli".to_string(), "Ce qui suit est utile : sans ces cases, les gestes se font autrement."),
        (n, _) => (
            format!("Pour démarrer : {} à remplir d'avance", if n == 1 { "une case exigée".to_string() } else { format!("{} cases exigées", n) }),
            "Une case exigée bloque son geste tant qu'elle est vide ; ce qui est rempli d'avance ne se demandera plus au moment de l'acte.",
        ),
    };
    let phrase = if note.is_empty() { format!("{}.", titre) } else { format!("{}. {}", titre, note) };
    let phrase = match &dsn {
        Some(d) => format!("{} {}", phrase, d.phrase),
        None => phrase,
    };
    let titre = if lignes.is_empty() && dsn.is_none() { String::new() } else { titre };
    let mut poses = poses(ctx)?;
    poses.extend(pose);
    // Chaque ligne en phrase entiere, puis ce qu'une DSN donnerait, puis ce que le moteur a pose de lui-meme : ce que la
    // ligne de commande lit.
    let mut phrases: Vec<String> = lignes.iter().flat_map(|l| l.phrase.clone()).collect();
    if let Some(d) = &dsn {
        phrases.push(d.phrase.clone());
        phrases.extend(d.manques.iter().flat_map(|l| l.phrase.clone()));
    }
    phrases.extend(poses.iter().map(|p| format!("{} : {}", p.libelle, p.texte)));
    Ok(PourDemarrer { phrase, phrases, titre, note: note.to_string(), manques: lignes, dsn, poses })
}

/// Ce qui suit le refus d'un acte quand la compagnie n'a pas fini de demarrer : toute la liste, d'un coup, sans ce que
/// le refus `deja_dit` dit deja (un manque que le refus nomme, ou dont il donne l'explication, ne se redit pas).
pub fn a_la_suite(ctx: &Contexte, deja_dit: &str) -> R<Option<String>> {
    let lignes: Vec<String> = manques(ctx)?
        .iter()
        .filter_map(|x| x.bloque.as_ref().map(|b| (x, b)))
        .filter(|(x, _)| {
            let t = x.trouver.trim();
            t.is_empty() || !deja_dit.contains(t)
        })
        .filter(|(x, _)| !(deja_dit.contains(&x.libelle) && x.qui.as_deref().is_none_or(|q| deja_dit.contains(q))))
        .map(|(x, b)| format!("« {} » pour {}, dans {}", x.libelle, b, x.ou))
        .collect();
    if lignes.is_empty() {
        return Ok(None);
    }
    Ok(Some(format!(" Pour démarrer, il manque encore : {}.", lignes.join(" ; "))))
}
