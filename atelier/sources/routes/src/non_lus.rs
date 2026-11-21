//! CE QUE LES FICHES D'UNE COMPAGNIE PORTENT ET QUE LE MOTEUR NE LIT PAS (`droit::fiche::NonLu` : une valeur datee
//! illisible, une cle que la forme ne connait pas) : la compagnie s'ouvre (`droit::fiche::lire_fiche_posee`) ; une valeur
//! illisible, la compagnie la saisit de nouveau, et rien ne se calcule d'ici la (`crate::redire`) ; une cle inconnue se
//! garde et ne dit rien. L'editeur l'apprend, EN SILENCE, par le journal des incidents (`compagnie::incidents`), pour que
//! le lecteur de la case apprenne l'ecriture qu'il ne connait pas, comme un document que l'import n'a pas lu
//! (`importation::incident`).
//!
//! UN INCIDENT PAR SORTE DE FICHE ET PAR CASE (ou cle inconnue) : son genre ne dit que cela, et un genre deja au journal
//! ne se note plus. CE QUI PART NE PORTE RIEN DE REEL (regle de fond 9) : ni la valeur, ni la cle de la fiche (un nom,
//! un NIR peuvent s'y trouver), ni aucune empreinte calculee sur l'une ou l'autre, qui se retrouverait par essais ; la
//! sorte de fiche, la case ou la cle inconnue, le rang de la valeur datee.

use std::collections::BTreeSet;

use compagnie::incidents;
use droit::contexte::Contexte;
use droit::fiche::NonLu;

/// L'endroit, dans le journal des incidents.
const OU: &str = "fiches";

/// Ce que l'incident dit d'une chose non lue : la sorte de fiche, la case (ou la cle inconnue) et le rang de la valeur
/// datee ; ni la valeur, ni la cle de la fiche.
#[derive(serde::Serialize)]
struct CeQuiPart<'a> {
    fiche: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    case: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rang: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cle_inconnue: Option<&'a str>,
}

/// A CHAQUE GESTE SUR UNE COMPAGNIE : chaque (sorte de fiche, case) que ses fiches portent et ne lisent pas
/// (`Contexte::non_lus`), une fois au journal des incidents, puis l'envoi ordinaire. Rien ne refuse le geste : un journal
/// qui ne se lit pas, ou un incident qui ne s'ecrit pas, se dit et le geste continue (comme `importation::incident`).
pub(crate) fn noter(ctx: &Contexte) {
    let non_lus = ctx.non_lus();
    if non_lus.is_empty() {
        return;
    }
    let deja: BTreeSet<incidents::CleDIncident> = match incidents::lire(&ctx.racine, None, false) {
        Ok(lignes) => lignes.iter().map(|l| l.empreinte.clone()).collect(),
        Err(e) => {
            eprintln!("Le journal des incidents ne se lit pas : ce que les fiches ne lisent pas ne s'y note pas : {}", e.en_lignes());
            return;
        }
    };
    let mut vus: BTreeSet<String> = BTreeSet::new();
    let mut notes = 0;
    for n in &non_lus {
        let fiche = n.fiche().nom().nom();
        let (genre, resume, part) = match n {
            NonLu::Valeur { case, rang, .. } => (
                format!("valeur illisible : fiche {}, case {}", fiche, case),
                format!("Une valeur de la case {} d'une fiche {} ne se lit pas : la compagnie la saisit de nouveau, rien ne se calcule d'ici là.", case, fiche),
                CeQuiPart { fiche, case: Some(case.to_string()), rang: Some(*rang), cle_inconnue: None },
            ),
            NonLu::Cle { cle, .. } => (
                format!("clé inconnue : fiche {}, {}", fiche, cle),
                format!("Une fiche {} porte la clé {}, que sa forme ne connaît pas : gardée, elle ne dit rien.", fiche, cle),
                CeQuiPart { fiche, case: None, rang: None, cle_inconnue: Some(cle) },
            ),
        };
        if !vus.insert(genre.clone()) || deja.contains(&incidents::empreinte(incidents::LOGICIEL, OU, &genre)) {
            continue;
        }
        let note = incidents::Note { nature: incidents::LOGICIEL, resume: &resume, ou: OU, quoi: &genre, detail: &part, trace: None, sans_contexte: true };
        match incidents::noter(&ctx.racine, &note) {
            Ok(Some(_)) => notes += 1,
            Ok(None) => {}
            Err(e) => eprintln!("Ce qu'une fiche ne lit pas ne se note pas au journal des incidents : {}", e.en_lignes()),
        }
    }
    if notes > 0 && incidents::envoi_actif(&ctx.racine) {
        incidents::envoyer(&ctx.racine);
    }
}
