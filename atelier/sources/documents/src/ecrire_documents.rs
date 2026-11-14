//! Les compteurs des documents : les numeros de contrat et de bulletin que portent les paies d'un mois
//! (`numeroter`, `paies_aux_numeros_declares`), ceux qu'un envoi reserve avant de les ecrire (`Reserves`), la sequence
//! continue des N CE (`prochain_certificat`) et le verrou sous lequel un numero se reserve puis s'ecrit (`prendre_le_verrou`).

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use droit::contexte::Contexte;
use socle::echec::{erreur, Classe};
use socle::echec::Echec;
use socle::echec::R;
use socle::mois::Annee;
use socle::verrou::Verrou;
use socle::ecriture::json_indente;
use engagement::calcul::Paie;
use engagement::numeros::NumeroDeCertificat;

fn os(chemin: &std::path::Path, e: std::io::Error) -> Echec {
    socle::echec::du_disque(&chemin, &e).reclasser(Classe::Disque)
}

/// La suite des numeros de chaque annee, sous la racine de la compagnie.
const COMPTEURS: &str = "config/compteurs.json";

/// `config/compteurs.json` : par annee (`"2026"`), le dernier numero de chaque suite : les bulletins (`17/2026`, leur
/// rang dans l'annee), les certificats Congés Spectacles (N CE, une suite qui ne repart pas a 1 chaque an).
pub type Compteurs = BTreeMap<Annee, CompteursDeLAnnee>;

/// Les derniers numeros d'une annee, suite par suite.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CompteursDeLAnnee {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bulletin: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub certificat_cs: Option<NumeroDeCertificat>,
}

/// `config/compteurs.json`, lu par sa forme, ou vide s'il n'existe pas. La seule lecture du fichier.
pub fn compteurs(racine: &std::path::Path) -> R<Compteurs> {
    let p = racine.join(COMPTEURS);
    if !p.exists() {
        return Ok(Compteurs::new());
    }
    socle::forme::lire_fichier_en_phrase(&p, |cle, _| {
        format!("Les compteurs de numéros de la compagnie (config/compteurs.json) ne se lisent pas{} : le fichier se corrige à la main.", if cle.is_empty() { String::new() } else { format!(" ({})", cle) })
    })
    .map_err(|e| e.reclasser(Classe::Illisible))
}

/// LE PROCHAIN N CE de la suite continue des certificats, qui ne repart pas a 1 chaque an : la LECTURE balaie toutes les
/// annees de `compteurs.json`, et le suivant du plus grand ; le premier sans aucun.
pub fn prochain_certificat(ctx: &Contexte) -> R<NumeroDeCertificat> {
    let vu = compteurs(&ctx.racine)?.values().filter_map(|c| c.certificat_cs).max();
    Ok(match vu.map(NumeroDeCertificat::suivant) {
        Some(n) if n > NumeroDeCertificat::PREMIER => n,
        Some(_) | None => NumeroDeCertificat::PREMIER,
    })
}

/// `verrou_compteurs(ctx, qui=...)` : refuse si un autre tient les compteurs.
pub fn prendre_le_verrou(ctx: &Contexte, qui: &str) -> R<Verrou> {
    let chemin = socle::verrou::chemin_compteurs(&ctx.racine);
    let maintenant = socle::paris::ecrit::seconde::texte(socle::temps::secondes()).replace('T', " ");
    let marque = socle::verrou::marque(qui, std::process::id(), &maintenant);
    match Verrou::prendre(&chemin, &marque, socle::verrou::DELAI_VERROU).map_err(|m| m.reclasser(Classe::Disque))? {
        Ok(v) => Ok(v),
        Err(v) => Err(erreur(Classe::CompteursVerrouilles, v.message())),
    }
}


// ------------------------------------------------------------------ les numeros des contrats et des bulletins

/// Les numeros de contrat (FA1.26.13) et de bulletin (17/2026) que les paies porteraient (`engagement::numeros::numeroter`), en
/// memoire : l'apercu n'ecrit rien. Un engagement qui PORTE DEJA son numero de contrat le garde (`engagement::numeros`),
/// une paie partie son numero de bulletin (`engagement::etats::numeros_des_documents`).
pub fn numeroter(ctx: &Contexte, paies: &mut [Paie], annee: i32, departs: &engagement::departs::Departs) -> Result<(), Echec> {
    numeroter_en_attente(ctx, paies, annee, &mut HashMap::new(), departs)
}

/// La meme, avec les numeros de bulletin deja RESERVES par un appel precedent du meme processus : `annee -> dernier
/// numero`, sinon a la suite du compteur de l'annee (`config/compteurs.json`) et des bulletins importes de l'annee
/// (`engagement::numeros::dernier_bulletin_importe`).
pub fn numeroter_en_attente(ctx: &Contexte, paies: &mut [Paie], annee: i32, attente: &mut HashMap<i32, i64>, departs: &engagement::departs::Departs) -> Result<(), Echec> {
    let n_bull = match attente.get(&annee) {
        Some(v) => v + 1,
        None => {
            let lu = compteurs(&ctx.racine)?.get(&Annee::nouvelle(annee.into())?).and_then(|c| c.bulletin);
            lu.into_iter().fold(engagement::numeros::dernier_bulletin_importe(&ctx.racine, annee.into())?, i64::max) + 1
        }
    };
    let n_bull = engagement::numeros::numeroter(ctx, paies, annee, n_bull, departs)?;
    attente.insert(annee, n_bull - 1);
    Ok(())
}

// ------------------------------------------------------------- les numeros reserves

/// Les numeros reserves par ce processus, dans leur ordre d'arrivee. Rien n'est ecrit avant `valider`.
#[derive(Default)]
pub struct Reserves {
    /// Par annee, les derniers numeros reserves de chaque suite (ce qui n'est pas reserve est vide).
    valeurs: BTreeMap<i64, CompteursDeLAnnee>,
}

impl Reserves {
    /// Le rang du prochain bulletin de l'annee : le suivant de ce qui est reserve, sinon de ce que `compteurs.json`
    /// porte, apres les bulletins importes de l'annee (`engagement::numeros::dernier_bulletin_importe`).
    pub fn prochain_bulletin(&self, ctx: &Contexte, annee: i64) -> R<i64> {
        if let Some(v) = self.valeurs.get(&annee).and_then(|c| c.bulletin) {
            return Ok(v + 1);
        }
        let lu = compteurs(&ctx.racine)?.get(&Annee::nouvelle(annee)?).and_then(|c| c.bulletin);
        let importe = engagement::numeros::dernier_bulletin_importe(&ctx.racine, annee)?;
        // Une suite se numerote a partir de 1 : le suivant du plus grand numero lu ou importe.
        Ok(lu.into_iter().chain(Some(importe)).fold(0, i64::max) + 1)
    }

    /// Reserve le rang `v` du dernier bulletin de l'annee, sans ecrire.
    pub fn enregistrer_bulletin(&mut self, annee: i64, v: i64) {
        self.valeurs.entry(annee).or_default().bulletin = Some(v);
    }

    /// Reserve le dernier N CE `n`, sans ecrire.
    pub fn enregistrer_certificat(&mut self, annee: i64, n: NumeroDeCertificat) {
        self.valeurs.entry(annee).or_default().certificat_cs = Some(n);
    }

    /// Les numeros de bulletin des paies, reserves ; leurs numeros de contrat, ceux qu'elles porteraient.
    pub fn numeroter(&mut self, ctx: &Contexte, paies: &mut [Paie], annee: i32, departs: &engagement::departs::Departs) -> R<()> {
        let mut attente: HashMap<i32, i64> = self.valeurs.get(&annee.into()).and_then(|c| c.bulletin).map(|v| (annee, v)).into_iter().collect();
        numeroter_en_attente(ctx, paies, annee, &mut attente, departs)?;
        if let Some(n) = attente.get(&annee) {
            self.enregistrer_bulletin(annee.into(), *n);
        }
        Ok(())
    }

    /// Ecrit les numeros reserves dans `config/compteurs.json`.
    pub fn valider(&self, ctx: &Contexte) -> R<()> {
        if self.valeurs.is_empty() {
            return Ok(());
        }
        let p = ctx.racine.join(COMPTEURS);
        let mut c = compteurs(&ctx.racine)?;
        for (an, r) in &self.valeurs {
            let e = c.entry(Annee::nouvelle(*an)?).or_default();
            if let Some(b) = r.bulletin {
                e.bulletin = Some(b);
            }
            if let Some(n) = r.certificat_cs {
                e.certificat_cs = Some(n);
            }
        }
        std::fs::write(&p, json_indente(&c, 2)).map_err(|e| os(&p, e))
    }
}

// ------------------------------------------------------------------ les paies d'un mois aux numeros declares

/// LA SAISIE DU MOIS AUX NUMEROS DECLARES : ce qui peut partir de `mois` (`etats::validees_du_mois` : la saisie reduite
/// aux engagements valides, et leurs paies ecrites), chaque contrat sous le numero que son engagement porte depuis sa
/// validation (`engagement::numeros`), chaque bulletin sous le sien (ecrit dans la paie a son depart, ou celui que le
/// compteur propose). `reserves` : les numeros de bulletin qu'un envoi propose, que la route d'envoi ecrit plus loin. Un
/// mois sans engagement valide n'a rien.
pub fn paies_aux_numeros_declares(ctx: &Contexte, mois: socle::mois::Mois, reserves: Option<&mut Reserves>, departs: &engagement::departs::Departs) -> Result<(engagement::saisie::VueDuMois, Vec<Paie>), Echec> {
    let Some((data, mut paies)) = engagement::etats::validees_du_mois(ctx, mois, departs)? else {
        return Ok((engagement::saisie::VueDuMois::vide(mois), Vec::new()));
    };
    let annee = data.mois.annee();
    match reserves {
        Some(r) => r.numeroter(ctx, &mut paies, annee, departs)?,
        None => numeroter(ctx, &mut paies, annee, departs)?,
    }
    Ok((data, paies))
}

