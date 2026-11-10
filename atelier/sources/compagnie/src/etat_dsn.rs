//! L'ETAT DE LA DSN D'UN MOIS, composé ici et nulle part ailleurs (l'accueil, le mois tenu avant, les rappels, la
//! clôture, l'échéancier, le panneau, le suivi d'un dépôt et le dépôt réel le lisent) : le journal des dépôts d'abord
//! (un dépôt que les retours attestent y entre de lui-même : `reprise_journal::reprendre_les_depots`) ; à défaut, la DSN
//! que l'import a lue pour ce mois (le fichier de l'ancien logiciel) -- le mois est déclaré, rien ne propose de la
//! refaire (règle 4 : une déclaration déposée ne se régénère pas, et PuppetPaie n'a pas calculé ce mois). Un mois tenu
//! avant sans aucune trace le dit, sans le dire « en retard » ni proposer de la produire.
//!
//! Ce que l'état veut dire (partie, déposée, à faire) se lit ici, par les méthodes d'[`EtatDsn`] ; sa forme est
//! déclarée une fois (`EtatDsn`, `Etat`), et c'est elle que les routes rendent.

use droit::fiche::Lue as _;
use serde::Serialize;

use droit::contexte::Contexte;
use envois::etats;
use socle::echec::R;
use socle::mois::Mois;

/// Où en est la DSN d'un mois : « passee », « attente », « refusee » (le journal) ; « importee » (la DSN de l'ancien
/// logiciel) ; « sans_trace » (un mois tenu avant dont rien ne garde la DSN, qui ne se réclame pas) ; « a_deposer » (un
/// mois de ce logiciel dont la DSN n'est pas partie).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Etat {
    Passee,
    Attente,
    Refusee,
    Importee,
    SansTrace,
    ADeposer,
}

/// L'état de la DSN d'un mois : `etat`, ce que l'écran en dit (`courte`, `phrase` ; aucun mot pour une DSN à déposer),
/// le jour du dépôt (`quand`, au journal), et pour une DSN du journal les `anomalies` du bilan et si France Travail a
/// renvoyé l'attestation employeur qu'il en édite (`aer_recue`).
/// Il se relit avec la reponse qui le porte (le refus d'une seconde DSN du mois, `routes::enveloppe::APart`).
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EtatDsn {
    pub etat: Etat,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub courte: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phrase: Option<String>,
    pub quand: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anomalies: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    aer_recue: Option<bool>,
}

impl EtatDsn {
    fn est(&self, etats: &[Etat]) -> bool {
        etats.contains(&self.etat)
    }

    /// Une DSN est partie pour ce mois et n'est pas refusée : passée, retour attendu, ou déposée par l'ancien logiciel.
    pub fn partie(&self) -> bool {
        self.est(&[Etat::Passee, Etat::Attente, Etat::Importee])
    }

    /// Une DSN de ce logiciel est sur la plate-forme, refusée comprise.
    pub fn deposee(&self) -> bool {
        self.est(&[Etat::Passee, Etat::Attente, Etat::Refusee])
    }

    /// Passée, sans que France Travail ait encore renvoyé l'attestation employeur (AER) qu'elle en édite.
    pub fn aer_attendue(&self) -> bool {
        self.etat == Etat::Passee && self.aer_recue != Some(true)
    }

    /// Le bilan l'a refusée.
    pub fn refusee(&self) -> bool {
        self.etat == Etat::Refusee
    }

    /// La DSN reste à faire : refusée, ou jamais partie d'un mois de ce logiciel.
    pub fn a_faire(&self) -> bool {
        self.est(&[Etat::Refusee, Etat::ADeposer])
    }

    /// Ce que l'état dit d'un mot à l'écran et en entier, sans jour de dépôt.
    fn dit(etat: Etat, courte: &str, phrase: &str) -> EtatDsn {
        EtatDsn { etat, courte: Some(courte.to_string()), phrase: Some(phrase.to_string()), quand: None, anomalies: None, aer_recue: None }
    }
}

/// L'état de la DSN de `mois` ; `nic` : l'établissement (`None` : le principal ; les états du journal sont les siens).
pub fn du_mois(ctx: &Contexte, le_mois: Mois, nic: Option<&str>) -> R<EtatDsn> {
    let mois = le_mois;
    let lignes = envois::journal::lu(&ctx.racine).map(|j| j.lignes)?;
    if let Some(e) = etats::etat_dsn_du_mois(&lignes, le_mois, nic, ctx.entreprise.nic_principal()) {
        let quand = etats::quand_lisible(&e.quand);
        let (etat, courte, phrase) = if e.refusee {
            (Etat::Refusee, "refusée".to_string(), format!("DSN déposée le {} — REFUSÉE par la plate-forme", quand))
        } else if e.passee {
            (Etat::Passee, "déposée — passée".to_string(), format!("DSN déposée le {} — passée", quand))
        } else {
            (Etat::Attente, "déposée — retour attendu".to_string(), format!("DSN déposée le {} — retour attendu", quand))
        };
        return Ok(EtatDsn { etat, courte: Some(courte), phrase: Some(phrase), quand: Some(e.quand), anomalies: Some(e.anomalies), aer_recue: Some(e.aer_recue) });
    }
    if let Some(etablie) = dsn_importee_le(&ctx.racine, mois)? {
        const SANS_BILAN: &str = "aucun bilan de la plate-forme n'est lu pour elle. Rien n'est à refaire ici.";
        let phrase = match etablie.as_deref().and_then(socle::date::Jour::du_texte) {
            Some(j) => format!("DSN déclarée avant PuppetPaie : celle de l'ancien logiciel, importée (établie le {}) — {}", j.jj_mm_aaaa(), SANS_BILAN),
            None => format!("DSN déclarée avant PuppetPaie : celle de l'ancien logiciel, importée — {}", SANS_BILAN),
        };
        return Ok(EtatDsn::dit(Etat::Importee, "déclarée avant PuppetPaie", &phrase));
    }
    if !importe(&ctx.racine, mois)? {
        return Ok(EtatDsn { etat: Etat::ADeposer, courte: None, phrase: None, quand: None, anomalies: None, aer_recue: None });
    }
    Ok(EtatDsn::dit(
        Etat::SansTrace,
        "aucune trace de sa DSN",
        "Mois tenu avant PuppetPaie : aucune trace de sa DSN — ni au journal, ni dans les retours relevés, ni parmi les DSN importées. Si elle est partie, le relevé des retours ou l'import de la DSN de l'ancien logiciel la retrouvera ; si elle n'est jamais partie, elle se fait dans le logiciel qui a calculé ces paies.",
    ))
}

/// Le mois est-il tenu avant ce logiciel : aucun engagement de la saisie ne le touche, et un engagement importe le touche.
pub fn importe(racine: &std::path::Path, mois: Mois) -> R<bool> {
    if engagement::stock::a_une_saisie(racine, mois)? {
        return Ok(false);
    }
    Ok(!engagement::importe::du_mois(racine, mois)?.is_empty())
}

/// LA DSN QUE L'IMPORT A LUE POUR `mois` (`engagement::importe::dsn_deposee`), que la saisie touche aussi le mois ou
/// non : `Some` du jour que le fichier de l'ancien logiciel porte (S20.G00.05.007, sa constitution ; aucun s'il n'en
/// porte pas), pas celui d'un depot -- aucun bilan de la plate-forme n'est lu pour lui. `None` : aucune DSN.
pub(crate) fn dsn_importee_le(racine: &std::path::Path, mois: Mois) -> R<Option<Option<String>>> {
    Ok(engagement::importe::dsn_deposee(racine, mois)?.map(|x| x.paie.depose_le.map(|t| t.trim().to_string()).filter(|t| !t.is_empty())))
}

/// LA DSN QUI PORTE LES CORRECTIONS DE `mois` : la premiere DSN d'un mois suivant qui n'est pas partie (ni ici, ni par
/// l'ancien logiciel, `engagement::importe::dsn_deposee`). Une correction se declare dans la DSN suivante (cahier
/// technique DSN 2026.1, § 2.1.2.1, `atelier/reference/norme-dsn/texte/chapitre-2-principes-de-constitution.txt`).
pub fn dsn_qui_porte(ctx: &Contexte, mois: Mois) -> R<Mois> {
    let mut porteur = mois.suivant();
    while du_mois(ctx, porteur, None)?.partie() {
        porteur = porteur.suivant();
    }
    Ok(porteur)
}

/// LE PREMIER MOIS DONT LA COMPAGNIE DOIT UNE DSN, quand on le sait. Une compagnie neuve n'est en retard de rien : le
/// premier mois du est celui de sa premiere embauche (`premiere_embauche` de la fiche Entreprise) ; sans cette date, une
/// compagnie qui n'a ni paie saisie ni DSN au journal n'a encore rien a declarer avant le mois courant (des fiches ne
/// sont pas un passe). `None` : rien ne borne -- une compagnie qui a deja des paies ou des declarations.
pub fn premier_mois_du(ctx: &Contexte) -> R<Option<Mois>> {
    if let Some(j) = ctx.entreprise.premiere_embauche.dite() {
        return Ok(Some(Mois::du_jour(j)));
    }
    if !engagement::stock::mois_saisis(&ctx.racine)?.is_empty() {
        return Ok(None);
    }
    let journal = envois::journal::lu(&ctx.racine)?;
    if journal.lignes.iter().any(|l| l.acte() == envois::journal::Acte::Dsn) {
        return Ok(None);
    }
    Ok(Some(ctx.aujourdhui.mois_civil()))
}

/// LA DSN DE `mois` EST-ELLE DUE pour l'etablissement `nic` (`Null` : le principal) ? « La DSN est mensuelle et
/// produite par établissement employeur. Par principe, elle est générée par l'acte de paie et liée à la présence de
/// salariés [...]. A défaut, une DSN Sans individu devra être produite » (cahier technique DSN 2026.1, § 2.4,
/// `atelier/reference/norme-dsn/texte/chapitre-2-principes-de-constitution.txt` l. 2291-2309) : elle est due chaque mois,
/// avec ou sans salarie, a partir du premier mois du (`premier_mois_du`), tant qu'elle reste a faire
/// (`EtatDsn::a_faire`). LE SEUL ENDROIT QUI LE DIT : l'echeancier, les rappels et le panneau des etapes le lisent.
pub fn due(ctx: &Contexte, mois: Mois, nic: Option<&str>) -> R<bool> {
    if premier_mois_du(ctx)?.is_some_and(|p| mois < p) {
        return Ok(false);
    }
    Ok(du_mois(ctx, mois, nic)?.a_faire())
}
