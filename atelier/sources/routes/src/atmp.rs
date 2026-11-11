//! LE CODE RISQUE ET LE TAUX AT/MP DE LA COMPAGNIE : une case de la configuration, comme les autres, datee. Le siege l'a
//! dans la fiche entreprise (`taux_atmp`), chaque autre etablissement sur sa ligne (`routes::etablissements`) : la meme
//! forme (`droit::fiche::TauxAtmp` : code risque, taux, `depuis`, `provenance`, notification), la meme regle, le meme
//! affichage. La case dit le present : le taux en vigueur aujourd'hui ; sous elle, la ligne d'avant et celle a venir.
//!
//! Chaque source ecrit une ligne datee de la meme case, avec sa provenance (`fiches::inscrire_taux_atmp`, le seul
//! endroit qui l'ecrit pour le siege ; `routes::etablissements::ecrire` pour les autres) :
//! - l'import de la compagnie : le code et le taux que declarent ses DSN reprises (S21.G00.40.040, .043), au 1er janvier de
//!   chaque annee reprise (`import::parametrage`) ;
//! - la notification recue avec les retours de la DSN (`recue`) : a sa date d'effet ; le meme taux n'y change rien, mais
//!   sa provenance s'inscrit. Si la personne a inscrit pour cette annee un autre taux, rien ne s'ecrit : le conflit se dit
//!   a l'ouverture, et c'est a elle de choisir ;
//! - le taux collectif de l'arrete de l'annee, pour un risque « TC » (annexe 1, D. 242-6-14, I ; et pour un autre
//!   etablissement nouvellement cree, D. 242-6-17) : a l'ouverture, au 1er janvier, des que le bareme de l'annee est livre
//!   (`inscrire_l_arrete`), si aucun taux ne vaut deja a partir de ce jour ; et aussitot qu'un code TC s'inscrit sans
//!   taux ;
//! - la personne : « Inscrire le taux d'une notification », le geste ordinaire de la fiche (`enregistrer-fiche`) ; la
//!   date qu'elle tape fait foi. Si elle change un taux recu par la DSN, l'avertissement le dit.
//!
//! La paie ne lit que la case (`droit::bareme::taux_atmp_de_l_annee`) : sans ligne, ou avec une ligne perimee (un risque
//! sans TC ne vaut pas au-dela du 31 decembre de son annee), le taux se demande, comme toute case vide.
//! Ce que les textes disent (`atelier/reference/sources/tarification-atmp/LISEZ-MOI.md`) : la CARSAT notifie le code
//! risque et le taux ; l'employeur declare 999ZZ tant qu'il n'a pas de notification, sans taux (norme DSN, S21.G00.40.040
//! et .043, `norme-dsn/texte/bloc-S21.G00.40.txt` l. 1224-1315).

use droit::fiche::Lue as _;

use serde::Serialize;

use crate::enveloppe::Dite;
use droit::bareme::{self, RisqueAtmp};
use droit::contexte::Contexte;
use droit::fiche::{Etablissement, FicheEntrepriseDatee, Montant, NotificationAtmp, ProvenanceAtmp, TauxAtmp};
use socle::date::Jour;
use socle::echec::{erreur, Classe, R};
use socle::num::Decimal;

/// Le socle livre de l'annee, s'il existe, lu par sa forme (`droit::contexte::socle_livre_lu`) ; un socle livre qui ne se
/// lit pas se refuse.
fn socle_de(an: i32) -> R<Option<droit::forme_bareme::Pile>> {
    droit::contexte::socle_livre_lu(an).map_err(|m| m.reclasser(Classe::Illisible))
}

fn en_pourcent(t: Decimal) -> String {
    let mut t = t.round_dp(2);
    t.rescale(2);
    format!("{} %", t.to_string().replace('.', ","))
}

fn premier_janvier(an: i32) -> R<Jour> {
    Jour::nouveau(an, 1, 1).ok_or_else(|| erreur(Classe::Refuse, format!("Année {} hors du calendrier.", an)))
}

fn decimal(t: &TauxAtmp) -> R<Option<Decimal>> {
    t.valeur.as_ref().map(Montant::decimal).transpose().map_err(|m| m.reclasser(Classe::Illisible))
}

// ------------------------------------------------------------------ les sources

/// LE CODE ET LE TAUX QUE LA CARSAT NOTIFIE (retour DSN de nature 34, `envois::retours::taux_atmp_d_octets`) s'inscrivent
/// dans la case du siege a leur date d'effet, avec la notification (`le` : le jour ou elle a paru) ; le meme taux n'y
/// change que la provenance. Une ligne que la personne a inscrite pour la meme annee avec un autre code ou un autre taux
/// ne se remplace pas : `Ok(Some(phrase))` dit le conflit, et c'est a elle de choisir. `Ok(Some(phrase))` aussi quand
/// rien ne s'inscrit, en disant pourquoi.
pub(crate) fn recue(ctx: &Contexte, n: &envois::retours::TauxAtmpNotifie, le: Option<Jour>, source: &str) -> R<Option<String>> {
    let t = TauxAtmp {
        code_risque: Some(n.code_risque.clone()),
        valeur: Some(Montant::Flottant(socle::num::court(n.taux))),
        depuis: Some(n.date_effet),
        provenance: Some(ProvenanceAtmp::Notification),
        notification: Some(NotificationAtmp { le, calcul: n.date_calcul, effet: Some(n.date_effet) }),
    };
    if n.code_risque == bareme::SANS_NOTIFICATION {
        return Ok(Some(format!("{} : code risque {} ; rien n'est inscrit. Le code et le taux se lisent sur la notification (compte AT/MP sur net-entreprises) et s'inscrivent dans Configuration › Entreprise, « Accidents du travail (AT/MP) ».", source, n.code_risque)));
    }
    if let Some(m) = bareme::taux_atmp_refuse(&t).map_err(|m| m.reclasser(Classe::Illisible))? {
        return Ok(Some(format!("{} : {} Rien n'est inscrit ; le code et le taux de la notification (compte AT/MP sur net-entreprises) s'inscrivent dans Configuration › Entreprise, « Accidents du travail (AT/MP) ».", source, m)));
    }
    let fiche = crate::fiches::lire_fichier::<FicheEntrepriseDatee>(ctx)?;
    let deja = fiche.taux_atmp.as_deref().and_then(|l| droit::fiche::valeur_au(l, &n.date_effet));
    if let Some(d) = deja.filter(|d| d.provenance == Some(ProvenanceAtmp::Saisie) && d.depuis.is_some_and(|j| j.an == n.date_effet.an)) {
        if d.code_risque != t.code_risque || decimal(&d)? != decimal(&t)? {
            return Ok(Some(format!(
                "Conflit sur le taux AT/MP : vous avez inscrit {} ; la notification de la CARSAT ({}) dit {}. Rien n'est changé : c'est à vous de choisir. Pour prendre celui de la notification, inscrivez-le dans Configuration › Entreprise, « Accidents du travail (AT/MP) ».",
                ligne_dite(&d)?.dit,
                source,
                ligne_dite(&t)?.dit
            )));
        }
    }
    crate::fiches::inscrire_taux_atmp(ctx, t, source)?;
    Ok(None)
}

/// LE TAUX COLLECTIF DE L'ARRETE DE L'ANNEE `an` s'inscrit au 1er janvier, des que le socle de l'annee est livre (en
/// decembre, la ligne est a venir), pour le siege et pour chaque autre etablissement dont le risque est « TC » (annexe 1,
/// D. 242-6-14, I), ou qui est nouvellement cree (D. 242-6-17, `bareme::etablissement_neuf`) ; hors de l'Alsace-Moselle
/// (art. 4 de l'arrete : l'annexe 3 y vaut, `bareme::taux_de_l_arrete`). Seulement si aucun taux ne vaut deja a partir
/// de ce 1er janvier ou apres (la notification de l'annee, deja recue, ou la saisie) ; une ligne de l'annee qui ne dit que
/// le code se complete a sa date. Le code est celui de la derniere ligne. Appele a l'ouverture, a l'import, et apres
/// l'enregistrement de la case (`SorteDeFiche::apres_ecriture`). Rend ce qui s'est inscrit, en phrases.
pub(crate) fn inscrire_l_arrete(ctx: &Contexte, an: i32) -> R<Vec<String>> {
    // Une annee sans socle livre n'a pas d'arrete a inscrire.
    Ok(socle_de(an)?.map(|socle| inscrire_selon(ctx, an, socle)).transpose()?.into_iter().flatten().collect())
}

fn inscrire_selon(ctx: &Contexte, an: i32, socle: droit::forme_bareme::Pile) -> R<Vec<String>> {
    let janvier = premier_janvier(an)?;
    let risques = bareme::risques_de(&socle);
    let arrete = match bareme::arrete_atmp(&socle) {
        Some(a) => a,
        None => format!("l'arrêté de tarification de {}", an),
    };
    let note = format!("Taux collectif de {} (annexe 1 ; code de la sécurité sociale, art. D. 242-6-14), atelier/reference/sources/tarification-atmp/.", arrete);
    // La ligne de l'arrete pour la derniere ligne `l` d'une case, si elle lui revient.
    // Une ligne de cette annee qui ne dit que le code (la personne l'a inscrit sans taux) se complete a sa propre date.
    let de_l_arrete = |l: &TauxAtmp, alsace: bool, neuf: bool, depuis: Jour| -> Option<TauxAtmp> {
        let de_l_annee = l.depuis.filter(|d| *d >= janvier);
        if de_l_annee.is_some_and(|d| d.an > an || l.valeur.is_some()) {
            return None;
        }
        let code = l.code_risque.clone().filter(|c| !c.is_empty())?;
        let r = risques.iter().find(|r| r.code == code)?;
        let t = bareme::taux_de_l_arrete(r, neuf, alsace)?;
        Some(TauxAtmp {
            code_risque: Some(code),
            valeur: Some(Montant::Flottant(socle::num::court(t))),
            depuis: Some(match de_l_annee {
                Some(d) => d.max(depuis),
                None => depuis,
            }),
            provenance: Some(ProvenanceAtmp::Arrete),
            notification: None,
        })
    };
    let mut dit = Vec::new();
    let fiche = crate::fiches::lire_fichier::<FicheEntrepriseDatee>(ctx)?;
    // Le siege.
    let siege = fiche.taux_atmp.as_deref().and_then(derniere);
    let alsace = bareme::en_alsace_moselle(ctx.entreprise_au(&janvier).code_postal.portee());
    if let Some(t) = siege.and_then(|l| de_l_arrete(&l, alsace, false, janvier)) {
        let phrase = format!("Taux AT/MP {}, siège : {}.", an, ligne_dite(&t)?.dit);
        if crate::fiches::inscrire_taux_atmp(ctx, t, &note)? {
            dit.push(phrase);
        }
    }
    // Les autres etablissements : la derniere liste, si elle ne change pas apres ce 1er janvier ; un etablissement ouvert
    // dans l'annee, a son ouverture.
    let Some(liste) = fiche.etablissements.as_deref().and_then(|l| droit::fiche::valeurs_datees(l).last().copied()).filter(|v| v.depuis.is_none_or(|d| d <= janvier)).and_then(|v| v.valeur.clone()) else {
        return Ok(dit);
    };
    let mut changee = false;
    let mut nouvelle = Vec::new();
    for mut e in liste {
        if let Some(ouverture) = e.ouverture {
            let depuis = ouverture.max(janvier);
            let neuf = bareme::etablissement_neuf(&e, an);
            let alsace = bareme::en_alsace_moselle(e.code_postal.portee());
            if let Some(t) = e.taux_atmp.clone().and_then(|l| de_l_arrete(&l, alsace, neuf, depuis)) {
                dit.push(format!("Taux AT/MP {}, établissement {} : {}.", an, nom_de(&e), ligne_dite(&t)?.dit));
                e.taux_atmp = Some(t);
                changee = true;
            }
        }
        nouvelle.push(e);
    }
    let liste: Vec<Etablissement> = nouvelle;
    if changee {
        crate::fiches::inscrire_etablissements(ctx, &liste, janvier, &note)?;
    }
    Ok(dit)
}

/// La derniere ligne d'une case du taux AT/MP (la plus tardive par sa date), d'ou qu'elle vienne.
fn derniere(liste: &[droit::fiche::Datee<TauxAtmp>]) -> Option<TauxAtmp> {
    // A meme date, la derniere ecrite.
    liste.iter().filter_map(|d| d.valeur.clone()).filter(|t| !t.est_vide()).max_by_key(|t| t.depuis)
}

/// LA CASE DU TAUX AT/MP D'UN AUTRE ETABLISSEMENT `nic`, en liste datee comme celle du siege : chaque ligne que porte une
/// version de la liste des etablissements, a sa propre date, une fois, avec la note de la version qui l'a portee la
/// premiere.
fn case_d_un_etablissement(versions: &[&droit::fiche::Datee<Vec<Etablissement>>], nic: Option<&str>) -> Vec<droit::fiche::Datee<TauxAtmp>> {
    let mut out: Vec<droit::fiche::Datee<TauxAtmp>> = Vec::new();
    for v in versions {
        for t in v.valeur.iter().flatten().filter(|x| x.nic.portee() == nic).filter_map(|x| x.taux_atmp.clone()).filter(|t| !t.est_vide()) {
            if !out.iter().any(|d| d.valeur.as_ref() == Some(&t)) {
                out.push(droit::fiche::Datee { depuis: t.depuis, valeur: Some(t), note: v.note.clone() });
            }
        }
    }
    out
}

/// Le nom d'un etablissement : son libelle, sinon son NIC.
fn nom_de(e: &Etablissement) -> String {
    match (e.libelle.remplie(), e.nic.portee()) {
        (Some(l), _) => l.to_string(),
        (None, Some(n)) => n.to_string(),
        (None, None) => "sans nom".to_string(),
    }
}

// ------------------------------------------------------------------ ce que l'ecran et le LLM lisent

/// Un risque du socle, tel que l'ecran le montre.
#[derive(Debug, Serialize)]
struct RisqueDit {
    code: String,
    code_arrete: Option<String>,
    libelle: String,
    precisions: Option<String>,
    #[serde(serialize_with = "socle::forme::ecrire_a_virgule_opt")]
    taux: Option<socle::num::Decimal>,
    tc: bool,
}

fn risque_dit(r: &RisqueAtmp) -> RisqueDit {
    RisqueDit { code: r.code.clone(), code_arrete: r.code_arrete.clone(), libelle: r.libelle.clone(), precisions: r.precisions.clone(), taux: r.taux, tc: r.tc }
}

/// UNE LIGNE DE LA CASE, telle que l'ecran et le LLM la lisent : son code, son taux, sa date, d'ou elle vient, et la
/// phrase qui la dit.
#[derive(Debug, Serialize)]
struct LigneDite {
    code_risque: Option<String>,
    #[serde(serialize_with = "socle::forme::ecrire_a_virgule_opt")]
    taux: Option<socle::num::Decimal>,
    depuis: Option<Jour>,
    /// Pour la ligne d'avant : son dernier jour.
    #[serde(skip_serializing_if = "Option::is_none")]
    jusqu_au: Option<Jour>,
    provenance: Option<ProvenanceAtmp>,
    dit: String,
    /// La note de la ligne datee (d'ou elle vient) ; sans note, rien n'est dit.
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
}

fn ligne_dite(t: &TauxAtmp) -> R<LigneDite> {
    let valeur = decimal(t)?;
    let d_ou = match (t.provenance, &t.notification) {
        (Some(ProvenanceAtmp::Notification), n) => {
            let le: String = n.as_ref().and_then(|n| n.calcul.or(n.le)).iter().map(|j| format!(" du {}", j.jj_mm_aaaa())).collect();
            format!("reçu par la DSN (notification{})", le)
        }
        (Some(ProvenanceAtmp::Arrete), _) => format!("taux collectif de l'arrêté{}", t.depuis.iter().map(|j| format!(" {}", j.an)).collect::<String>()),
        (Some(ProvenanceAtmp::Import), _) => "repris à l'import".to_string(),
        (Some(ProvenanceAtmp::Saisie), _) => "inscrit dans PuppetPaie".to_string(),
        (None, _) => "inscrit".to_string(),
    };
    let taux = match valeur {
        Some(v) => en_pourcent(v),
        None => "sans taux".to_string(),
    };
    let code: String = t.code_risque.iter().map(|c| format!(", code risque {}", c)).collect();
    let depuis: String = t.depuis.iter().map(|j| format!(" à partir du {}", j.jj_mm_aaaa())).collect();
    Ok(LigneDite { code_risque: t.code_risque.clone(), taux: valeur, depuis: t.depuis, jusqu_au: None, provenance: t.provenance, dit: format!("{}{}{} : {}", taux, code, depuis, d_ou), note: None })
}

/// LA LIGNE D'AVANT, CELLE EN VIGUEUR `aujourdhui`, ET CELLE A VENIR d'une case du taux AT/MP, telles que
/// `droit::fiche::temps_au` les lit, chacune avec sa note ; une ligne qui ne dit rien est vide, et une ligne du meme
/// code et du meme taux que celle en vigueur, d'ou qu'elle vienne, n'est pas un changement.
fn trois(liste: &[droit::fiche::Datee<TauxAtmp>], aujourdhui: &Jour) -> R<(Option<LigneDite>, Option<LigneDite>, Option<LigneDite>)> {
    let liste: Vec<droit::fiche::Datee<TauxAtmp>> = liste.iter().map(|d| droit::fiche::Datee { valeur: d.valeur.clone().filter(|t| !t.est_vide()), ..d.clone() }).collect();
    let taux = |t: &TauxAtmp| t.valeur.as_ref().map(|v| v.decimal().map(|d| d.normalize()).map_err(|e| e.phrase));
    let temps = droit::fiche::temps_au(&liste, aujourdhui, |a, b| a.code_risque == b.code_risque && taux(a) == taux(b)).map_err(|m| m.reclasser(Classe::Illisible))?;
    let dite = |devant: &str, t: Option<&TauxAtmp>, depuis: Option<Jour>, jusqu_au: Option<Jour>, note: Option<String>| -> R<LigneDite> {
        let mut l = match t.filter(|t| !t.est_vide()) {
            Some(t) => ligne_dite(t)?,
            None => LigneDite {
                code_risque: None,
                taux: None,
                depuis,
                jusqu_au: None,
                provenance: None,
                dit: format!("aucun taux{}", depuis.iter().map(|j| format!(" à partir du {}", j.jj_mm_aaaa())).collect::<String>()),
                note: None,
            },
        };
        let jusqu: String = jusqu_au.iter().map(|j| format!(", jusqu'au {}", j.jj_mm_aaaa())).collect();
        l.dit = format!("{}{}{}", devant, l.dit, jusqu);
        l.jusqu_au = jusqu_au;
        l.note = note;
        Ok(l)
    };
    let avant = temps.avant.map(|a| dite("Avant : ", Some(&a.valeur), None, Some(a.jusqu_au), a.note)).transpose()?;
    let maintenant = temps.en_vigueur.filter(|e| e.valeur.as_ref().is_some_and(|t| !t.est_vide())).map(|e| dite("En vigueur : ", e.valeur.as_ref(), e.depuis, None, e.note)).transpose()?;
    let a_venir = temps.a_venir.map(|a| dite("À venir : ", a.valeur.as_ref(), Some(a.depuis), None, a.note)).transpose()?;
    Ok((avant, maintenant, a_venir))
}

/// La phrase et l'explication du taux de l'annee lu par la paie (`lu` : `bareme::taux_atmp_de_l_annee`).
fn dire(lu: Option<socle::echec::R<droit::forme_bareme::TauxAtmp>>, an: i32, nom: &Option<String>) -> (String, String) {
    match lu {
        Some(Ok(t)) => match t.valeur {
            Some(v) => (format!("Taux AT/MP {} : {}{}.", an, en_pourcent(v), nom.iter().map(|n| format!(", code risque {}", n)).collect::<String>()), String::new()),
            None => (
                format!("Taux AT/MP {} : pas encore de notification (999ZZ).", an),
                format!("999ZZ : la compagnie n'a pas encore reçu de notification de la CARSAT. La DSN le déclare ainsi, sans taux. Aucun texte ne dit quel taux le bulletin cotise en attendant : le logiciel ne calcule pas la paie de {} tant que le code et le taux notifiés ne sont pas inscrits.", an),
            ),
        },
        Some(Err(m)) => (format!("Taux AT/MP {} : à inscrire.", an), m.phrase),
        None => (
            format!("Taux AT/MP {} : pas encore dit.", an),
            format!("Le code risque et le taux AT/MP de {} ne sont pas dits : ils sont sur la notification de la CARSAT, et s'inscrivent ici avec la date à partir de laquelle ils valent.", an),
        ),
    }
}

/// Le code et le taux d'un autre etablissement, tels que l'ecran les dit, sous sa ligne.
#[derive(Debug, Serialize)]
struct EtablissementDit {
    nic: Option<String>,
    libelle: Option<String>,
    phrase: String,
    explication: String,
    risque: Option<RisqueDit>,
    avant: Option<LigneDite>,
    maintenant: Option<LigneDite>,
    a_venir: Option<LigneDite>,
}

/// CE QUE REND `taux-atmp` : pour le siege, la ligne d'avant, celle en vigueur et celle a venir ; la phrase du taux de
/// l'annee tel que la paie le lit, et ce qui manque ; le risque tel que l'arrete l'ecrit ; ou le lire ; la date que
/// l'ecran propose pour une ligne neuve ; les codes que le socle connait ; les memes choses pour chaque autre
/// etablissement.
#[derive(Debug, Serialize)]
pub(crate) struct TauxAtmpDit {
    phrase: String,
    explication: String,
    avant: Option<LigneDite>,
    maintenant: Option<LigneDite>,
    a_venir: Option<LigneDite>,
    risque: Option<RisqueDit>,
    ou_le_lire: &'static str,
    /// Le nom du geste qui inscrit une ligne (le geste ordinaire de la fiche, a la date tapee).
    inscrire: &'static str,
    propose_depuis: Jour,
    codes: Vec<RisqueDit>,
    etablissements: Vec<EtablissementDit>,
}

impl Dite for TauxAtmpDit {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

/// `taux-atmp` : la case du siege et celles des autres etablissements, aujourd'hui (voir `TauxAtmpDit`).
pub(crate) fn route(ctx: &Contexte) -> R<TauxAtmpDit> {
    let aujourdhui = ctx.aujourdhui;
    let an = aujourdhui.an;
    let socle = socle_de(an)?;
    let risques: Vec<RisqueAtmp> = socle.iter().flat_map(bareme::risques_de).collect();
    let nom_de_code = |code: &Option<String>| -> (Option<String>, Option<&RisqueAtmp>) {
        let r = code.as_ref().and_then(|c| risques.iter().find(|r| r.code == *c));
        (r.and_then(|r| r.code_arrete.clone()).or_else(|| code.clone()), r)
    };
    let brute = ctx.entreprise_brute();
    let siege: Vec<droit::fiche::Datee<TauxAtmp>> = brute.taux_atmp.iter().flatten().cloned().collect();
    let (avant, maintenant, a_venir) = trois(&siege, &aujourdhui)?;
    let code = ctx.entreprise.taux_atmp.as_ref().and_then(|t| t.code_risque.clone());
    let (nom, risque) = nom_de_code(&code);
    let (phrase, explication) = dire(ctx.taux_atmp_de(an), an, &nom);
    // Les autres etablissements : chaque ligne de chaque version de la liste, par NIC.
    let versions: Vec<&droit::fiche::Datee<Vec<Etablissement>>> = brute.etablissements.iter().flat_map(|l| droit::fiche::valeurs_datees(l)).collect();
    let mut etablissements = Vec::new();
    for e in ctx.etablissements_dits() {
        let n = e.nic.portee().map(String::from);
        let (avant, maintenant, a_venir) = trois(&case_d_un_etablissement(&versions, n.as_deref()), &aujourdhui)?;
        let code = e.taux_atmp.as_ref().and_then(|t| t.code_risque.clone());
        let (nom, r) = nom_de_code(&code);
        let lu = e.taux_atmp.as_ref().and_then(|t| bareme::taux_atmp_de_l_annee(t, an, socle.as_ref(), bareme::en_alsace_moselle(e.code_postal.portee())));
        let (phrase, explication) = dire(lu, an, &nom);
        etablissements.push(EtablissementDit { nic: n, libelle: e.libelle.portee().map(String::from), phrase, explication, risque: r.map(risque_dit), avant, maintenant, a_venir });
    }
    Ok(TauxAtmpDit {
        phrase,
        explication,
        avant,
        maintenant,
        a_venir,
        risque: risque.map(risque_dit),
        ou_le_lire: "La CARSAT notifie à la compagnie son code risque et son taux : la notification se lit dans le compte AT/MP de net-entreprises (compte entreprise › Accidents du travail et maladies professionnelles).",
        inscrire: "Inscrire le taux d'une notification",
        propose_depuis: premier_janvier(an + 1)?,
        codes: risques.iter().map(risque_dit).collect(),
        etablissements,
    })
}
