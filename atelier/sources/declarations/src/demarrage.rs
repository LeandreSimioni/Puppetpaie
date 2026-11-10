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
use droit::fiche::ChampsEntreprise;
use socle::date::Jour;
use socle::echec::{Classe, R};

/// Dans la fiche d'un spectacle : le titre que l'import a lu dans les bulletins de l'ancien logiciel, propose, et ou il
/// l'a lu (la ligne « Spectacle », ou la case « Analytique »). « Pour démarrer » le propose dans la case du titre.
pub const TITRE_PROPOSE: &str = "titre_propose";
pub const TITRE_LU_DANS: &str = "titre_lu_dans";
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
    pub champ: Option<String>,
    pub case: Option<String>,
    pub trouver: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cases: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub salarie: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spectacle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qui: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub propose: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phrase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pour: Option<String>,
}

impl Manque {
    /// Une ligne qui bloque `bloque`, sur la case `champ` (aucune : ce qui ne se remplit pas dans une fiche).
    fn bloquante(cle: &str, libelle: String, bloque: &str, ou: (&str, &str), champ: Option<&str>, trouver: String) -> Manque {
        Manque {
            cle: cle.to_string(),
            libelle,
            bloque: Some(bloque.to_string()),
            utile: None,
            sinon: None,
            ou: ou.0.to_string(),
            onglet: ou.1.to_string(),
            champ: champ.map(String::from),
            case: champ.map(String::from),
            trouver,
            cases: None,
            salarie: None,
            spectacle: None,
            qui: None,
            propose: None,
            phrase: None,
            pour: None,
        }
    }

    /// Une ligne utile a `utile`, sans laquelle `sinon`.
    pub fn utile(cle: String, libelle: String, utile: String, sinon: &str, ou: (&str, &str), champ: Option<&str>, trouver: String) -> Manque {
        Manque { bloque: None, utile: Some(utile), sinon: Some(sinon.to_string()), ..Manque::bloquante(&cle, libelle, "", ou, champ, trouver) }
    }
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
    pub champ: Option<String>,
}

/// `pour-demarrer` : sa phrase, chaque ligne et chaque pose en phrase entiere (`phrases`), son titre et sa note, les
/// lignes (`manques`) et ce que le moteur a pose (`poses`).
#[derive(Debug, Clone, Serialize)]
pub struct PourDemarrer {
    pub phrase: String,
    pub phrases: Vec<String>,
    pub titre: String,
    pub note: String,
    pub manques: Vec<Manque>,
    pub poses: Vec<Pose>,
}

/// Le champ `cle` de la fiche entreprise (`champs-base.json`), que cette table doit porter.
fn champ_entreprise(cle: &str) -> R<crate::champs::Champ> {
    crate::champs::champ("entreprise", cle)?.ok_or_else(|| socle::echec::livree("champs-base.json", &format!("entreprise.{}", cle)))
}

/// Le libelle et l'aide du champ `cle` de la fiche entreprise (`champs-base.json`).
fn champ(cle: &str) -> R<(String, String)> {
    let c = champ_entreprise(cle)?;
    Ok((c.libelle, c.aide))
}

/// Une ligne qui bloque, sur une case de la fiche entreprise ou sans case ; une structure dit ses cases (leur nom, leur
/// forme, leurs valeurs), d'apres la table qui les decrit.
fn manque(cle: &str, libelle: String, bloque: &str, ou: (&str, &str), champ: Option<&str>, trouver: String) -> R<Manque> {
    let mut l = Manque::bloquante(cle, libelle, bloque, ou, champ, trouver);
    if let Some(k) = champ {
        l.cases = champ_entreprise(k)?.champs.as_deref().map(crate::champs::cases_dites);
    }
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
    let c = champ_entreprise("licence_spectacle")?;
    let ent = crate::champs::Saisie::de_la_fiche("entreprise", ent)?;
    Ok(c.exige(&ent) || (spectacle && crate::champs::condition_remplie(c.si.as_ref(), &ent)))
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
        let declare = nature.map(droit::natures::NatureDeContrat::proprietes).transpose()?.is_some_and(|n| n.ecrit("S21.G00.40.050"));
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
        if engagement::imprevus::est_une_suspension(&imp) && engagement::imprevus::motif_de_suspension(imp.motif())?.is_some_and(|m| !m.remunere) {
            if let Some(cle) = engagement::permanents::cle_du_salarie(ctx, imp.salarie())? {
                return Ok(Some(cle));
            }
        }
    }
    Ok(None)
}

/// La ligne d'une case vide qu'un acte exige ou qu'un geste sert (`champs::manquants`), ou elle vit ; celle d'un salarie
/// dit qui (`salarie` : sa cle et son nom).
fn ligne(m: crate::champs::Manquant, ou: String, onglet: &str, salarie: Option<(&str, &str)>) -> Manque {
    let racine = match m.ecran.split_once('.') {
        Some((r, _)) => r.to_string(),
        None => m.ecran.clone(),
    };
    let (bloque, utile, sinon) = match m.sert {
        crate::champs::Sert::Exigee(actes) => (crate::champs::actes_en_clair(&actes), None, None),
        crate::champs::Sert::Utile { gestes, sinon } => (None, crate::champs::actes_en_clair(&gestes), sinon),
    };
    Manque {
        cle: match salarie {
            Some((s, _)) => format!("{}.{}", m.ecran, s),
            None => m.ecran.clone(),
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
        salarie: salarie.map(|(s, _)| s.to_string()),
        spectacle: None,
        qui: salarie.map(|(_, n)| n.to_string()),
        propose: None,
        phrase: None,
        pour: None,
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
    for m in crate::champs::manquants("entreprise", &crate::champs::Saisie::de_la_fiche("entreprise", ent)?)? {
        if m.cle == documents::conges_spectacles::CLE_ADHERENT && !intermittents {
            continue;
        }
        out.push(ligne(m, ENTREPRISE.0.to_string(), ENTREPRISE.1, None));
    }
    for (cle, s) in ctx.salaries.iter().filter(|(_, s)| !s.archivee()) {
        let cle = cle.as_str();
        let nom = match s.nomme() {
            Some(n) => n,
            None => cle.to_string(),
        };
        for m in crate::champs::manquants("salarie", &crate::champs::Saisie::de_la_fiche("salarie", &s.champs)?)? {
            out.push(ligne(m, format!("Configuration › Salariés, la fiche de {}", nom), "salarie", Some((cle, &nom))));
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
        for m in crate::champs::manquants("spectacle", &crate::champs::Saisie::de_la_fiche("spectacle", sp)?)? {
            let mut l = ligne(m, format!("Configuration › Spectacles, la fiche de {}", titre), "spectacle", None);
            l.cle = format!("{}.{}", l.cle, cle);
            l.spectacle = Some(cle.to_string());
            l.qui = Some(titre.clone());
            out.push(l);
        }
    }
    // LE TITRE D'UN SPECTACLE QUE L'IMPORT N'A PAS ECRIT (« A COMPLETER » sur une fiche active : le contrat et le bulletin
    // l'imprimeraient tel quel) : la case se demande, avec le titre que les bulletins de l'ancien logiciel portent
    // (`TITRE_PROPOSE`), a valider ou a corriger, et le numero d'objet auquel il correspond.
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
            Some("titre"),
            trouver,
        );
        l.spectacle = Some(cle.to_string());
        l.qui = Some(format!("Numéro d'objet {}", objet));
        l.propose = propose;
        out.push(l);
    }
    // La licence d'entrepreneur de spectacles : la DSN l'ecrit dans chaque contrat (S21.G00.40.049) et refuse sans elle
    // QUAND ELLE EST EXIGEE de la compagnie (`exige_si` de `champs-base.json`, lu par `dsn::licence_du_contrat` aussi) :
    // en 1790 et en 3252 la case se montre sans etre exigee, dans l'audiovisuel elle ne se montre pas.
    // Elle peut etre datee (une par periode) : le contexte la resout au jour.
    if ctx.licence(&ctx.aujourdhui).is_none_or(|l| l.trim().is_empty()) && licence_exigee(ent, intermittents)? {
        let (libelle, aide) = champ("licence_spectacle")?;
        out.push(manque("licence_spectacle", libelle, "la DSN", ENTREPRISE, Some("licence_spectacle"), aide)?);
    }
    // L'EFFECTIF, puis le reste du bareme de l'annee : le contexte refuse lui-meme, avec la phrase qui dit ou trouver ce
    // qui manque. L'effectif a part : le bareme s'arrete au premier refus (le taux AT/MP), et tout doit sortir d'un coup.
    // La date de la premiere embauche n'est demandee que quand l'effectif ne se connait pas autrement : un seuil d'effectif
    // n'est franchi qu'apres « cinq années civiles consécutives » (CSS, art. L. 130-1, II,
    // `atelier/reference/sources/code-de-la-securite-sociale/articles-20261011.md` l. 296), et des taux de chaque paie en
    // dependent (onze et cinquante salaries) : elle est exigee pour l'effectif de la paie.
    // Une compagnie plus ancienne dit l'effectif de chaque annee qui manque (`Effectif::a_dire`), tel que l'Urssaf l'a
    // calcule (L. 130-1, I) : celle qui arrive d'un autre logiciel n'a pas ses comptes rendus de nature 118.
    let an = ctx.aujourdhui.an;
    let effectif = ctx.effectif()?;
    let manquent = effectif.a_dire(an);
    if !manquent.is_empty() && effectif.premiere_embauche.is_none() {
        let (libelle, _) = champ("premiere_embauche")?;
        let m = match effectif.atteint(11, an) {
            Err(m) => m.phrase,
            Ok(_) => format!("L'effectif de {} n'est pas connu.", an - 1),
        };
        out.push(manque("premiere_embauche", libelle, "l'effectif de toute paie (seuils de 11 et 50 salariés)", ENTREPRISE, Some("premiere_embauche"), m)?);
    } else if !manquent.is_empty() {
        let annees = manquent.iter().map(i32::to_string).collect::<Vec<_>>().join(", ");
        out.push(manque(
            "effectif",
            format!("L'effectif de {}", annees),
            "l'effectif de toute paie (seuils de 11 et 50 salariés)",
            ENTREPRISE,
            None,
            "Ces années sont d'avant PuppetPaie : leur effectif se dit tel que l'Urssaf l'a calculé (l'effectif annuel moyen de l'année, sur le compte rendu « Effectifs » de nature 118 reçu au début de l'année suivante), sous « Avancé », une année à la fois. Le compte rendu 118 de l'Urssaf le remplace à son arrivée.".to_string(),
        )?);
    }
    // LE CSE, la ou la convention en fait dependre une contribution (`par_representation` : le FNAS de la CCNEAC,
    // article III.3.1) et seulement quand l'effectif de la paie compte au moins onze salaries : en dessous, la question
    // ne se pose pas (III.3.1.b, `droit::bareme::representation_du_personnel`). Il ne se lit qu'avec la cle de l'effectif
    // (`droit::contexte::Effectif::cse`). Un bareme qui ne se lit pas se dit plus bas, ce qui lui manque.
    let bareme = ctx.bareme(&ctx.aujourdhui);
    let cle = "representation_personnel";
    let par_representation = |b: &droit::forme_bareme::Pile| b.contributions_conventionnelles.iter().flatten().any(|c| c.par_representation.as_ref().is_some_and(|p| !p.is_empty()));
    // C'est l'effectif de la paie (celui de l'annee d'avant, L. 130-1, I) qui decide (III.3.1.a et b) : inconnu, c'est
    // lui que la contribution demande, pas le CSE.
    if let Ok(b) = &bareme {
        if par_representation(b) {
            let cse = effectif.cse(an);
            let dit = cse.as_ref().is_some_and(|k| ent.cse.ouvrir(k).representation_personnel.est_dite());
            match effectif.valeur(an) {
                Some(e) if e >= 11.0 && cse.is_some() && !dit => {
                    let (libelle, aide) = champ(cle)?;
                    out.push(manque(cle, libelle, "la contribution au FNAS de chaque paie", ENTREPRISE, Some(cle), aide)?);
                }
                // Sans la cle, l'effectif d'une des cinq annees d'avant manque : `a_dire` l'a demande plus haut.
                Some(_) => {}
                None => out.push(manque(
                    "effectif",
                    format!("L'effectif de {}", an - 1),
                    "la contribution au FNAS de chaque paie",
                    ENTREPRISE,
                    None,
                    format!("La CCNEAC fixe la contribution au FNAS par l'effectif (article III.3.1) : celui de {} tel que l'Urssaf l'a calculé (compte rendu « Effectifs » de nature 118), sous « Avancé ».", an - 1),
                )?),
            }
        }
    }
    // L'effectif qui manque est demande plus haut.
    if let Err(e) = bareme {
        match e.classe {
            Some(Classe::TauxAtmpAbsent) => out.push(manque("taux_atmp", format!("Le taux AT/MP {}", an), "toute paie", ENTREPRISE, Some("taux_atmp"), e.phrase)?),
            Some(Classe::EffectifAbsent) => {}
            _ => out.push(manque("bareme", format!("Le barème {}", an), "toute paie", ENTREPRISE, None, e.phrase)?),
        }
    }
    // La sante d'un permanent suspendu sans indemnisation tout un mois : hors de la 3090, aucun texte ne la dit, et le mois
    // refuse tant que la fiche ne repond pas (`calcul::sante_de_la_suspension`). Elle ne se demande que si une suspension
    // non remuneree est saisie : un champ n'apparait que parce qu'un fait l'exige.
    let dite = |cle: &droit::natures::AuMois| ent.permanent.ouvrir(cle).sante_suspension_non_indemnisee.est_dite();
    if ent.idcc.portee() != Some("3090") && une_suspension_non_remuneree(ctx)?.is_some_and(|cle| !dite(&cle)) {
        let cle = droit::bareme::SANTE_SUSPENSION;
        let (libelle, aide) = champ(cle)?;
        out.push(manque(cle, libelle, "le mois d'un permanent suspendu sans indemnisation tout le mois", ENTREPRISE, Some(cle), aide)?);
    }
    // LA TVA ET LA TAXE SUR LES SALAIRES : la DSN de decembre declare chaque annee si la compagnie la doit (bloc
    // S21.G00.44) et ne se fait pas sans reponse (`taxe_sur_les_salaires::regime_taxe_sur_les_salaires`). Une compagnie qui
    // demarre en novembre la rencontrait au premier mois de decembre, sans que rien ne l'ait dit d'avance.
    if let documents::taxe_sur_les_salaires::Decision::ADire(m) = documents::taxe_sur_les_salaires::regime_taxe_sur_les_salaires(ent, None).decision {
        let cle = documents::taxe_sur_les_salaires::CHAMP_TAXE_SUR_LES_SALAIRES;
        let (libelle, _) = champ(cle)?;
        out.push(manque(cle, libelle, "la DSN de décembre", ENTREPRISE, Some(cle), m)?);
    }
    // LES CONTRATS DES PERMANENTS : c'est tout ce que la compagnie a a saisir des organismes (le reste, les textes le
    // donnent : `droit::organismes`). Sans eux, le mois d'un permanent se refuse (`dsn::affiliations`).
    // Le mois que « Pour demarrer » prepare : celui du jour.
    let mois = ctx.aujourdhui.mois_civil();
    let permanents = engagement::permanents::Permanents::de(ctx, &mois.premier_jour(), &mois.dernier_jour())?;
    if permanents.ouverts() && ctx.organismes()?.par_nature.permanent.references.is_empty() {
        out.push(manque(
            "organismes_permanents",
            "Les contrats de prévoyance et de santé des permanents".into(),
            "le mois d'un salarié au mois",
            ("Configuration › Organismes et contrats", "organismes"),
            None,
            "La compagnie emploie des salariés au mois : leurs contrats de prévoyance et de santé se disent avec ceux des intermittents. La référence du contrat se lit sur le certificat d'adhésion de l'assureur.".into(),
        )?);
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

/// LA LIGNE QUI DEMANDE LES CONGES ACQUIS AVANT PUPPETPAIE d'un salarie au mois (`salarie`, `nom` tel que l'ecran le dit),
/// paye ici depuis `depuis` : la case `conges_reprise` de sa fiche, ou elle mene. « Pour démarrer » la dit, et la carte
/// d'un contrat commence avant le logiciel la montre avant sa validation (`routes::mois`).
pub fn conges_a_dire(salarie: &str, nom: &str, depuis: Jour) -> Manque {
    let le = depuis.jj_mm_aaaa();
    let mut l = Manque::utile(
        format!("conges_reprise.{}", salarie),
        format!("Ses congés payés acquis avant le {}", le),
        "le compteur de ses congés payés".into(),
        "il part de zéro",
        (&format!("la fiche de {}, « Congés payés à la reprise »", nom), "salarie"),
        Some("conges_reprise"),
        format!("Son contrat a commencé avant que PuppetPaie le paie : recopiez de son dernier bulletin de votre ancien logiciel les jours acquis et pris, avec le {} pour date de reprise.", le),
    );
    l.qui = Some(nom.to_string());
    l.salarie = Some(salarie.to_string());
    l
}

/// La reprise des conges de ce salarie est-elle deja dite sur sa fiche (`conges_reprise.date`) ?
pub fn conges_repris(ctx: &Contexte, au_mois: &droit::natures::AuMois, salarie: &str) -> bool {
    ctx.salaries.get(salarie).and_then(|f| f.permanent.ouvrir(au_mois).conges_reprise.dite()).is_some_and(|c| c.date.est_dite())
}

/// Les permanents dont un contrat commence avant le premier mois ou PuppetPaie le paie, et dont la fiche ne dit pas les
/// conges a la reprise (`conges_reprise.date`) : `(salarie, nom, premier jour paye ici)`. Le premier mois paye ici est
/// celui du fichier de l'engagement ; un contrat repris d'un ancien logiciel qui continue ici y est des le premier mois
/// neuf (`engagement::importe`), valide ou non. Un intermittent n'en est pas (ses conges sont ceux des Conges
/// Spectacles) ; un contrat ne d'ici n'a rien d'avant.
fn conges_a_reprendre(ctx: &Contexte) -> R<Vec<(String, String, Jour)>> {
    // (cle des cases d'un salarie au mois, salarie, debut du contrat, premier jour paye ici)
    let mut contrats: Vec<(droit::natures::AuMois, String, Jour, Jour)> = Vec::new();
    let mut vus: Vec<String> = Vec::new();
    // Les engagements valides et actifs, chacun dans le mois de son fichier (`engagement::stock::objets`) : un brouillon
    // n'engage rien, un contrat repris qui continue ici si.
    for o in engagement::stock::objets(&ctx.racine)? {
        if !(o.est_valide() || o.est_importe()) || o.est_non_actif() || o.importe_seul() {
            continue;
        }
        let e = o.carte();
        let Some(salarie) = e.salarie.as_ref().map(ToString::to_string) else { continue };
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
        let nom = match ctx.salaries.get(salarie.as_str()).map(|f| documents::tableaux_export::qui_souple(f)).filter(|n| !n.is_empty()) {
            Some(n) => n,
            None => salarie.clone(),
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
    let an = ctx.aujourdhui.an;
    let Some(Ok(t)) = ctx.taux_atmp_de(an) else { return Ok(Vec::new()) };
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
            t.depuis.exigee("taux_atmp.depuis")?.an
        ),
        _ => return Ok(Vec::new()),
    };
    let risque = match &t.code_risque {
        Some(code) => droit::contexte::socle_livre_lu(an)?.and_then(|s| droit::bareme::risques_de(&s).into_iter().find(|r| r.code == *code)),
        None => None,
    };
    let nom = match (&risque, &t.code_risque) {
        (Some(droit::bareme::RisqueAtmp { code_arrete: Some(c), .. }), _) => c.clone(),
        (Some(r), _) => r.code.clone(),
        (None, code) => code.exigee("taux_atmp.code_risque")?.clone(),
    };
    let taux: String = t.valeur.iter().map(|v| format!(", taux {}", format!("{:.2} %", v.round_dp(2)).replace('.', ","))).collect();
    let de: String = risque.iter().map(|r| format!("« {} » ", r.libelle)).collect();
    Ok(vec![Pose {
        cle: "taux_atmp".into(),
        libelle: format!("Accidents du travail : code risque {}{} en {}", nom, taux, an),
        texte: format!("{}{}", de, d_ou),
        ou: ENTREPRISE.0.into(),
        onglet: ENTREPRISE.1.into(),
        champ: Some("taux_atmp".into()),
    }])
}

/// `pour-demarrer`. `proposes` : les propositions qui ne se tirent pas des fiches (les acces aux organismes, que le
/// moteur lit dans le coffre), a la suite ; `pose` : ce qui se dit en information a leur place, apres ce que le moteur a
/// pose. Une case est EXIGEE POUR un geste (sans elle, il ne se fait pas : `bloque`) ou UTILE POUR un geste (sans elle,
/// il se fait autrement : `utile`, `sinon`) ; chaque ligne le dit (`pour`), et sa phrase entiere dit aussi ou elle vit
/// et de qui (`phrase`). Le refus reste a l'acte (`champs::refus_des_manques`).
pub fn route(ctx: &Contexte, proposes: Vec<Manque>, pose: Option<Pose>) -> R<PourDemarrer> {
    let mut lignes = manques(ctx)?;
    lignes.extend(proposes);
    for l in lignes.iter_mut() {
        let pour = match (&l.bloque, &l.utile, &l.sinon) {
            (Some(b), _, _) => format!("case exigée pour {}", b),
            (None, Some(u), Some(s)) => format!("utile pour {} ; sinon, {}", u, s),
            _ => return Err(socle::refus!("La ligne « {} » de « Pour démarrer » ne dit ni ce qu'elle bloque ni ce qu'elle sert : le signaler au développeur.", l.libelle)),
        };
        // Une valeur que le moteur propose (`propose`) se dit dans la phrase : la ligne de commande la lit comme l'ecran.
        let propose: String = l.propose.iter().map(|p| format!(" Proposé : « {} ».", p)).collect();
        l.phrase = Some(format!("{} › {} : {}.{}", l.ou, l.libelle, pour, propose));
        let mut c = pour.chars();
        let initiale: String = c.next().into_iter().flat_map(char::to_uppercase).collect();
        l.pour = Some(format!("{}{}.", initiale, c.as_str()));
    }
    let n = lignes.iter().filter(|l| l.bloque.is_some()).count();
    let (titre, note) = match (n, lignes.is_empty()) {
        (0, true) => ("Pour démarrer : rien à remplir".to_string(), ""),
        (0, false) => ("Pour démarrer : tout ce qui est exigé est rempli".to_string(), "Ce qui suit est utile : sans ces cases, les gestes se font autrement."),
        (n, _) => (
            format!("Pour démarrer : {} à remplir d'avance", if n == 1 { "une case exigée".to_string() } else { format!("{} cases exigées", n) }),
            "Une case exigée bloque son geste tant qu'elle est vide ; ce qui est rempli d'avance ne se demandera plus au moment de l'acte.",
        ),
    };
    let phrase = if note.is_empty() { format!("{}.", titre) } else { format!("{}. {}", titre, note) };
    let titre = if lignes.is_empty() { String::new() } else { titre };
    let mut poses = poses(ctx)?;
    poses.extend(pose);
    // Chaque ligne en phrase entiere, puis ce que le moteur a pose de lui-meme : ce que la ligne de commande lit.
    let mut phrases: Vec<String> = lignes.iter().flat_map(|l| l.phrase.clone()).collect();
    phrases.extend(poses.iter().map(|p| format!("{} : {}", p.libelle, p.texte)));
    Ok(PourDemarrer { phrase, phrases, titre, note: note.to_string(), manques: lignes, poses })
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
