//! CE QUE LES FICHES D'UNE COMPAGNIE PORTENT ET QUE LE MOTEUR NE LIT PAS (`droit::fiche::NonLu` : une valeur datee
//! illisible, une cle que la forme ne connait pas) : rien ne ferme la compagnie (`droit::fiche::lire_fiche_posee`), et
//! l'editeur l'apprend, EN SILENCE, par le journal des incidents (`compagnie::incidents`), pour que le lecteur de la case
//! apprenne l'ecriture qu'il ne connait pas, comme un document que l'import n'a pas lu (`importation::incident`).
//!
//! UNE CHOSE NON LUE, UN INCIDENT : son genre porte une empreinte de ce qu'elle est (la fiche et sa cle, la case, le rang,
//! la valeur ecrite ; la cle inconnue), et une chose deja au journal ne se note plus : le geste suivant, qui relit les
//! memes fiches, n'envoie rien de neuf. CE QUI PART NE DIT NI LA VALEUR NI LA CLE DE LA FICHE (un nom, un NIR peuvent
//! s'y trouver) : la sorte de fiche, la case ou la cle inconnue, le rang ; l'empreinte ne se relit pas.

use std::collections::BTreeSet;

use sha2::{Digest, Sha256};

use compagnie::incidents;
use droit::contexte::Contexte;
use droit::fiche::{FicheNommee, NonLu};

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

/// L'empreinte de ce qu'une chose non lue est, sans rien en dire.
fn empreinte(n: &NonLu) -> String {
    let mut h = Sha256::new();
    match n.fiche() {
        FicheNommee::Entreprise => {}
        FicheNommee::Salarie(k) => h.update(k.to_string().as_bytes()),
        FicheNommee::Spectacle(k) => h.update(k.to_string().as_bytes()),
    }
    h.update(b"\n");
    match n {
        NonLu::Valeur { case, rang, ecrite, .. } => h.update(format!("{}\n{}\n{}", case, rang, ecrite).as_bytes()),
        NonLu::Cle { cle, .. } => h.update(cle.as_bytes()),
    }
    h.finalize().iter().take(6).map(|b| format!("{:02x}", b)).collect()
}

/// A CHAQUE GESTE SUR UNE COMPAGNIE : chaque chose que ses fiches portent et ne lisent pas (`Contexte::non_lus`), une
/// fois au journal des incidents, puis l'envoi ordinaire. Rien ne refuse le geste : un journal qui ne se lit pas, ou un
/// incident qui ne s'ecrit pas, se dit et le geste continue (comme `importation::incident`).
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
    let mut notes = 0;
    for n in &non_lus {
        let fiche = n.fiche().nom().nom();
        let (genre, resume, part) = match n {
            NonLu::Valeur { case, rang, .. } => (
                format!("valeur illisible : fiche {}, case {} ({})", fiche, case, empreinte(n)),
                format!("Une valeur de la case {} d'une fiche {} ne se lit pas : tenue pour non dite.", case, fiche),
                CeQuiPart { fiche, case: Some(case.to_string()), rang: Some(*rang), cle_inconnue: None },
            ),
            NonLu::Cle { cle, .. } => (
                format!("clé inconnue : fiche {}, {} ({})", fiche, cle, empreinte(n)),
                format!("Une fiche {} porte la clé {}, que sa forme ne connaît pas : gardée, elle ne dit rien.", fiche, cle),
                CeQuiPart { fiche, case: None, rang: None, cle_inconnue: Some(cle) },
            ),
        };
        if deja.contains(&incidents::empreinte(incidents::LOGICIEL, OU, &genre)) {
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
