//! DEPOSER UNE DSN OU UNE DPAE : tout ce que `moteur` verifie avant que le depot parte (`envois::depot_acte`), la preuve
//! et le secret de l'organisme pris aux portes de l'acte reel (`compagnie::preuve`), et les suites qu'il fait ensuite
//! (`suites::faire`).
//!
//! DEPOSER EST L'UN DES RARES GESTES DU LOGICIEL QUI ENGAGENT L'EMPLOYEUR -- jusqu'a des ordres de
//! telepaiement reellement preleves. L'ordre est donc toujours le meme, et c'est le moteur qui le tient (jamais
//! la page) : ouvrir la porte du branchement ; lire le fichier ; verifier que le mode demande colle a ce qu'il code (un
//! fichier `02` depose << pour voir >> EST une declaration) ; exiger la preuve (le mot de passe du logiciel, retape au
//! moment de l'acte) ; exiger la REPETITION (ce contenu exact est deja passe en test, ou un motif ECRIT) ; seulement
//! alors chercher le secret de l'organisme ; deposer ; NOTER, quoi qu'il arrive -- succes comme echec.
//!
//! Le secret de l'organisme ne sort pas d'ici que pour le client (`releve_retours::Client`) : il n'est ni
//! journalise, ni affiche, ni renvoye. Un depot de test ne part jamais avec la nature `02` ni l'indicateur
//! `120` : la bascule se fait en memoire (`depot::basculer_*`), jamais sur le disque.
//!
//! Le SIMULATEUR (`bridage::branchement`) joue le depot -- un flux `FICTIF-...` -- et rien ne quitte le poste ;
//! le reste (repetition, journal) se deroule comme en vrai.

use droit::fiche::Lue as _;
use socle::echec::Classe;
use std::path::{Path, PathBuf};

use droit::contexte::Contexte;
use declarations::dsn_lue::DsnLue;
use envois::depot;
use envois::etats;
use socle::echec::R;
use socle::echec::erreur;
use envois::releve_retours::Client;
use envois::suites::Suites;
use envois::journal::{Acte, EmbaucheDeclaree, Ligne, Ouverture, Version};
use droit::contexte::IdSalarie;
use engagement::numeros::NumeroContrat;
use socle::mois::Mois;

pub use envois::depot_acte::DepotFait;
use socle::echec::Echec;
use compagnie::preuve::{motdepasse_organisme, verifier_preuve, Preuve};

/// UN REFUS DE DEPOT QUI PORTE OU EN EST CE QUI LE REFUSE (la DSN du mois, le test du fichier) : sa phrase, et l'etat a
/// part (`routes::enveloppe::APart`, case `etat`).
fn avec_etat<T: serde::Serialize>(classe: Classe, message: impl Into<String>, etat: &T) -> Echec {
    /// Ce que le refus porte a part : l'etat.
    #[derive(serde::Serialize)]
    struct Etat<'a, T> {
        etat: &'a T,
    }
    Echec { classe: Some(classe), message: socle::echec::avec(message, Etat { etat }) }
}

// -------------------------------------------------------- la repetition avant l'irreversible

/// Le refus, ecrit pour qu'on sache quoi faire dans la minute, a l'ecran comme par `moteur route` : un test qui a RATE se
/// corrige, un test PERIME se refait, une absence de test se fait, par la route `deposer` sans `reel`.
fn phrase_repetition(quoi: &str, etat: &etats::Repetition) -> String {
    let acte = if quoi == "dsn" { "la DSN" } else { "la DPAE" };
    let quand = |l: &envois::journal::Ligne| etats::quand_lisible(l.quand());
    let situation = if let Some(c) = &etat.conforme {
        format!(
            "Ce contenu est parti en test le {}, et son compte rendu n'est pas encore arrivé : l'accusé d'enregistrement dit que l'envoi est reçu, pas qu'il est conforme. Relevez les retours, puis envoyez le réel.",
            quand(c)
        )
    } else if let Some(l) = etat.ratees.last() {
        format!(
            "Ce contenu a été essayé en test le {}, et l'essai a échoué : {}. C'est cela qu'il faut corriger, puis le déposer de nouveau en test.",
            quand(l),
            etats::pourquoi_rate(l)
        )
    } else if let Some(l) = etat.perimees.last() {
        format!("Un fichier du même nom a été essayé en test le {}, mais son contenu a changé depuis : déposez d'abord ce fichier en test.", quand(l))
    } else {
        "Ce contenu n'a jamais été déposé en test : déposez d'abord ce fichier en test.".to_string()
    };
    format!(
        "Dépôt réel de {} refusé : rien n'est parti. {} Si le service de test est indisponible et que l'échéance ne peut pas attendre, le réel part avec un motif écrit (contournement), noté au journal des dépôts.",
        acte, situation
    )
}

/// Leve tant que ce contenu n'a pas ete repete ; rend ce qu'il faut noter. LA SORTIE
/// DE SECOURS EST VOULUE, et elle est tracee : un motif ECRIT (dix caracteres au moins) passe outre ; ce
/// qu'on refuse, c'est le contournement SILENCIEUX.
fn exiger_repetition(lignes: &[Ligne], quoi: Acte, chemin: &Path, contournement: &str) -> R<Ouverture> {
    let octets = std::fs::read(chemin).map_err(|e| erreur(Classe::Disque, socle::echec::du_disque(&chemin, &e)))?;
    let nom = socle::fichiers::nom_du(chemin);
    let etat = etats::repetition(lignes, quoi, &envois::journal::empreinte(&octets), &nom);
    // UN REEL NE PART QUE SUR LE BILAN LU DE SON TEST : l'accuse d'enregistrement dit
    // « envoi recu », pas « conforme » (regle 5). Pour une DSN, c'est le bilan de nature 11 du test, qui arrive « en
    // general dans les dix minutes » (la reponse du point de depot). Pour une DPAE, le bilan 11 ou le certificat 41 :
    // celui d'un test revient bel et bien (constate sur des DPAE de test : le certificat 41 les dit enregistrees,
    // code 99).
    if let (etats::EtatDuTest::Testee, Some(l)) = (etat.etat, &etat.conforme) {
        return Ok(Ouverture { test: Some(l.quand().to_string()), flux: etat.flux_conforme(), empreinte: etat.empreinte.clone(), compte_rendu: etat.compte_rendu.clone(), contournement: None });
    }
    let raison = contournement.trim();
    if !raison.is_empty() {
        if raison.chars().count() < 10 {
            return Err(erreur(
                Classe::Refuse,
                "Déposer sans répétition demande un motif écrit : ce qui empêchait le test, en une phrase, qu'on relira le jour où ce dépôt posera question. Rien n'est parti.",
            ));
        }
        return Ok(Ouverture { test: None, flux: None, empreinte: etat.empreinte.clone(), compte_rendu: None, contournement: Some(raison.to_string()) });
    }
    Err(avec_etat(Classe::RepetitionManquante, phrase_repetition(quoi.nom(), &etat), &etat))
}

/// LES ENGAGEMENTS QU'UN SIGNALEMENT COUVRE : un signalement porte un individu, un contrat, un evenement
/// (`signalement`) ; l'engagement du mois de son evenement dont il cite le numero (S21.G00.40.009), a sa version
/// (`journal::versions_du_mois`).
fn versions_du_signalement(ctx: &Contexte, mois: Mois, octets: &[u8]) -> R<Vec<Version>> {
    let texte = String::from_utf8_lossy(octets);
    let numeros: Vec<String> =
        texte.lines().filter_map(|l| l.strip_prefix("S21.G00.40.009,")).map(|v| v.trim().trim_matches('\'').to_string()).collect();
    envois::journal::versions_du_mois(ctx, &mois.to_string(), &envois::journal::lu(&ctx.racine).map(|j| j.departs)?, |_, numero, _| numeros.iter().any(|n| n == numero)).map_err(Echec::from)
}

/// La derniere porte -- une copie de developpement ne declare rien en reel.
fn refuser_si_copie(ctx: &Contexte, quoi: &str) -> R<()> {
    if ctx.racine.join("COPIE-DE-DEVELOPPEMENT.json").is_file() {
        return Err(erreur(
            Classe::Refuse,
            format!("Copie de developpement : {} ne part jamais d'ici. Les essais se font en test ; pour le faire en vrai, ouvrir la vraie compagnie.", quoi),
        ));
    }
    Ok(())
}

// ------------------------------------------------------------------------ la DSN

/// `ordre_d_essai_du_fichier` : l'ordre auquel un essai part : le calcule, jamais plus bas que celui que le fichier porte
/// deja. `None` reste `None`.
fn ordre_d_essai_du_fichier(calcule: Option<i64>, chemin: &Path) -> R<Option<i64>> {
    let Some(c) = calcule else { return Ok(None) };
    let octets = std::fs::read(chemin).map_err(|e| erreur(Classe::Disque, socle::echec::du_disque(&chemin, &e)))?;
    Ok(Some(match rang_dans_le_fichier(&octets)? {
        Some(m) => c.max(m),
        None => c,
    }))
}

/// Le mois de constitution (`AAAA-MM`) d'une DSN (S20.G00.05.007). La rubrique est obligatoire pour toute nature de
/// declaration (norme DSN, `atelier/reference/norme-dsn/dsn-note-differentielle-ct-2026.1.2-vers-2027.1.2-20251224.txt`,
/// l. 4838 : « S20.G00.05.007 Date de constitution du fichier O O O O O O O ») : une DSN qui ne la dit pas se refuse.
fn mois_de_constitution(a: &DsnLue) -> R<String> {
    match a.depot {
        Some(j) => Ok(j.mois_civil().to_string()),
        None => Err(erreur(Classe::Illisible, "Cette DSN ne dit pas sa date de constitution (S20.G00.05.007, obligatoire) : elle ne part pas.")),
    }
}

/// Le rang que la DSN porte (S20.G00.05.004), s'il y en a un ; un rang qui ne se lit pas se refuse.
fn rang_dans_le_fichier(octets: &[u8]) -> R<Option<i64>> {
    declarations::dsn_lue::lire(octets, "")?.ordre.map(|o| ordre_lu(&o)).transpose()
}

/// L'ordre qu'une DSN a deposer porte (S20.G00.05.004) : absent ou illisible, elle se refait.
fn ordre_dit(o: Option<&str>) -> R<i64> {
    let o = o.ok_or_else(|| erreur(Classe::Illisible, socle::echec::cle("Cette DSN ne dit pas son ordre : la refaire. Rien n'est parti.", "S20.G00.05.004")))?;
    ordre_lu(o)
}

/// Un ordre de declaration (S20.G00.05.004) lu en nombre ; illisible, il se refuse.
fn ordre_lu(o: &str) -> R<i64> {
    o.trim().parse::<i64>().map_err(|e| erreur(Classe::Illisible, socle::echec::avec("L'ordre d'une DSN ne se lit pas.", socle::echec::APart { cle: Some("S20.G00.05.004".into()), detail: Some(format!("« {} » : {}", o, e)), ..socle::echec::APart::default() })))
}

/// `prochain_ordre_de_test` : l'ordre a donner au prochain essai de ce mois, ou `None` pour le premier. Les
/// suivants partent en annule-et-remplace, a un ordre plus grand que tout ce que la base de test a pu retenir :
/// le plus grand des ordres deja poses par nous, de ceux des fichiers essayes encore sur le disque, du nombre
/// d'essais, et de ce que les bilans archives disent de la base d'essai pour le mois de constitution du fichier
/// (`ordre_signalements::dernier_ordre_des_mensuelles` : les essais faits AILLEURS sous le meme SIREN, l'atelier ou un
/// autre poste, comptent pour l'ordre sans s'afficher), plus un. Une mensuelle du mois deja
/// acceptee dans la base d'essai, meme venue d'ailleurs, fait partir l'essai en annule-et-remplace. Trop grand ne coute
/// rien ; trop petit, c'est B1-208-11. `constitution` : le mois de constitution du fichier (`AAAA-MM`, S20.G00.05.007).
fn prochain_ordre_de_test(ctx: &Contexte, le_mois: Mois, nic: Option<&str>, constitution: &str) -> R<Option<i64>> {
    let mois = &le_mois.to_string();
    let du_mois = envois::journal::lire(&ctx.racine, Some(Acte::Dsn), Some(le_mois), false).map_err(|m| erreur(Classe::Disque, m))?;
    let essais: Vec<&Ligne> = etats::de_l_etablissement(&du_mois, nic, ctx.entreprise.nic.ecrite("nic")?).into_iter().filter(|l| !l.reel() && !l.signalement()).collect();
    let vues = envois::ordre_signalements::dernier_ordre_des_mensuelles(&ctx.racine, &envois::ordre_signalements::siren_de(ctx)?, true, constitution, mois)?;
    if essais.is_empty() && !vues.du_mois_acceptee {
        return Ok(None);
    }
    let mut connus: Vec<i64> = vec![essais.len() as i64, vues.ordre];
    for l in &essais {
        if let Some(n) = l.dsn().and_then(|e| e.detail.ordre_test).filter(|n| *n > 0) {
            connus.push(n);
        }
        if let Some(nom) = l.fichier().filter(|f| !f.is_empty()) {
            let p = ctx.racine.join("sorties").join(mois).join(nom);
            if p.exists() {
                let octets = std::fs::read(&p).map_err(|e| erreur(Classe::du_disque(&e), socle::echec::du_disque(&p, &e)))?;
                if let Some(n) = rang_dans_le_fichier(&octets)? {
                    connus.push(n);
                }
            }
        }
    }
    Ok(connus.into_iter().max().map(|m| m + 1))
}

/// Le garde-fou du depot REEL d'une mensuelle, ecrit depuis la
/// norme. `Ok(true)` : un annule-et-remplace en regle (rien a forcer) ; `Ok(false)` : une premiere DSN ou une
/// normale (`DsnDejaDeposee` tranche) ; un refus sinon.
fn verifier_depot_remplacement(ctx: &Contexte, le_mois: Mois, a: &DsnLue, nic: Option<&str>) -> R<bool> {
    let mois = &le_mois.to_string();
    let mut m = crate::etapes::Moteur::nouveau(ctx);
    let faits = m.depots_reels(le_mois, nic)?;
    let dernier = m.dernier_ordre(le_mois, &faits)?;
    if !a.type_declaration.as_deref().is_some_and(|t| ["03", "05"].contains(&t)) {
        return Ok(false);
    }
    // CE QUE LA PLATE-FORME A VU EN REEL au mois de constitution du fichier, lu en silence dans les bilans archives
    // (`ordre_signalements::dernier_ordre_des_mensuelles`) : une mensuelle reelle envoyee d'un autre poste compte, comme
    // a la production (`depart::forme_a_produire`).
    let constitution = mois_de_constitution(a)?;
    let vues = envois::ordre_signalements::dernier_ordre_des_mensuelles(&ctx.racine, &envois::ordre_signalements::siren_de(ctx)?, false, &constitution, mois)?;
    let dernier = match dernier {
        Some(d) => d.max(vues.ordre),
        None if vues.du_mois_acceptee || vues.ordre > 0 => vues.ordre,
        None => return Ok(false),
    };
    m.refuser_si_hors_delai(le_mois)?;
    let ordre = ordre_dit(a.ordre.as_deref())?;
    if ordre <= dernier {
        return Err(erreur(
            Classe::OrdreTropPetit,
            format!(
                "Cet annule-et-remplace porte l'ordre {}, et la derniere DSN reelle connue pour {} (journal et bilans de la plate-forme) l'ordre {} : la plate-forme n'integre pas un rang inferieur ou egal (cahier technique DSN, p. 15 ; B1-208-11). Refaire la DSN du mois : elle partira a l'ordre {}. Rien n'est parti.",
                ordre,
                mois,
                dernier,
                dernier + 1
            ),
        ));
    }
    Ok(true)
}

/// Depose une DSN. La seule sortie du moteur qui declare quelque chose.
///
/// L'ORDRE COMPTE : 1. lire le fichier ; 2. verifier que le mode demande colle a la nature codee dedans ;
/// 3. exiger la preuve (reel) ; 4. exiger la REPETITION (reel, `contournement` pour la sortie de secours) ;
/// 5. seulement alors chercher le secret de l'organisme ; 6. deposer ; 7. NOTER, quoi qu'il arrive.
pub(crate) fn deposer_dsn(
    ctx: &Contexte,
    fichier: &str,
    reel: bool,
    preuve: Option<&Preuve>,
    qui: &str,
    forcer: bool,
    contournement: &str,
) -> R<DepotFait> {
    deposer_dsn_dans(ctx, fichier, reel, preuve, qui, "production", forcer, contournement, None)
}

/// `deposer_dsn` avec l'ENVIRONNEMENT du point de depot (`production` ou `editeur`) et l'ordre d'un essai, que seul
/// l'outil de l'atelier donne (`atelier::commandes::outils::depot_dsn`). La plate-forme de l'editeur est une plate-forme d'ESSAI :
/// un depot REEL n'y a pas de sens et reste refuse ici, car le journal le noterait comme une declaration partie.
///
/// `ordre_d_essai` : l'ordre au-dessous duquel un ESSAI de mensuelle ne part pas, donne par qui sait ce que la base
/// d'essai a deja retenu pour ce SIRET et ce mois (un autre poste, une compagnie d'essai, le « dernier ordre accepte »
/// d'un B1-208-11). L'essai part alors en annule-et-remplace a cet ordre, ou plus haut si le journal en sait davantage.
/// Refuse pour un depot reel et pour un signalement (leur ordre est celui du fichier, lu sur la plate-forme a son
/// ecriture : `ordre_signalements`).
#[allow(clippy::too_many_arguments)]
pub fn deposer_dsn_dans(
    ctx: &Contexte,
    fichier: &str,
    reel: bool,
    preuve: Option<&Preuve>,
    qui: &str,
    environnement: &str,
    forcer: bool,
    contournement: &str,
    ordre_d_essai: Option<i64>,
) -> R<DepotFait> {
    let branchement = compagnie::bridage::branchement(&ctx.racine)?;
    branchement.ouvrir()?;
    if reel && environnement != "production" {
        return Err(erreur(
            Classe::Refuse,
            format!("L'environnement '{}' est la plate-forme d'essai de l'editeur : rien n'y est declare. Un depot REEL ne part que sur 'production' ; pour essayer, un depot de test. Rien n'est parti.", environnement),
        ));
    }
    let chemin = PathBuf::from(crate::depots::resoudre(&ctx.racine, fichier)?);
    let octets = crate::depots::lire_octets(&chemin.to_string_lossy())?;
    let apercu = declarations::dsn_lue::lire(&octets, "")?;
    // 2. le mode contre le fichier ; `presence_prouvee` : c'est ICI qu'on la prouve, jamais le transport.
    depot::verifier(&apercu, reel, None, true)?;
    // LE MOIS QUE LE FICHIER DECLARE ; un signalement se date par son evenement, sa ligne par le mois de ce jour.
    let le_mois = apercu.mois_declare().map_err(|m| erreur(Classe::Refuse, format!("Ce fichier ne dit pas le mois qu'il déclare ({}). Rien n'est parti.", m)))?;
    let evenement = if apercu.est_signalement() { apercu.evenement().map(|j| j.iso()) } else { None };
    let mois = le_mois.to_string();
    if let Some(n) = ordre_d_essai {
        if reel || apercu.est_signalement() || n < 1 {
            return Err(erreur(
                Classe::Refuse,
                "Un ordre d'essai ne vaut que pour le depot de TEST d'une DSN mensuelle, et c'est un entier positif : un depot reel part a l'ordre du fichier, un signalement a celui que la plate-forme a donne a son ecriture. Rien n'est parti.",
            ));
        }
    }
    // UNE COMPAGNIE D'ESSAI ne depose que ce qu'elle a ecrit sous SON identite : les DSN copiees de la fictive portent le
    // SIREN de la fictive, que le compte net-entreprises ne declare pas. Le refus ne cite aucun des deux numeros.
    if compagnie::essai::est_essai(&ctx.racine)? && apercu.emetteur.as_deref() != ctx.entreprise.siren.portee() {
        return Err(erreur(
            Classe::Refuse,
            "Compagnie d'essai : ce fichier ne porte pas le SIREN de la compagnie (il a ete ecrit sous une autre identite : celle de la compagnie fictive, avant le montage, ou une identite d'avant). Déposer ici la DSN du mois, qui s'y fabrique. Rien n'est parti.",
        ));
    }
    let nic: Option<&str> = apercu.etablissement().filter(|n| !n.is_empty());
    let lignes = envois::journal::lu(&ctx.racine).map(|j| j.lignes)?;

    let (mut forme, mut repetition) = (String::new(), None);
    // UN SIGNALEMENT REEL PART A UN ORDRE QUE PERSONNE N'A ENCORE PRIS : la sequence ne se remet jamais a zero
    // (cahier technique DSN, p. 136), et la plate-forme n'accepte pas un second signalement normal au meme ordre
    // (p. 14). Deux signalements ecrits avant que le premier parte portent le meme : le second se refait. Refuse
    // avant la preuve : aucun mot de passe n'y change rien.
    if reel && apercu.est_signalement() {
        let dernier = envois::journal::dernier_ordre_des_signalements(&ctx.racine).map_err(|m| erreur(Classe::Refuse, m))?;
        let ordre = ordre_dit(apercu.ordre.as_deref())?;
        if ordre <= dernier {
            return Err(erreur(
                Classe::Refuse,
                format!(
                    "Ce signalement porte l'ordre {}, et un signalement est déjà parti à l'ordre {} : le numéro d'ordre d'un signalement ne se remet jamais à zéro et ne se prend pas deux fois (cahier technique DSN, p. 136 et 14). Refaire le signalement : il partira à l'ordre {}. Rien n'est parti.",
                    ordre,
                    dernier,
                    dernier + 1
                ),
            ));
        }
    }
    // UNE DSN MENSUELLE REELLE DIT CE QU'ELLE CORRIGE : sa fabrication l'a note (`pieces_ecrites::versions_portees_de`). Un fichier
    // que le moteur n'a pas ecrit ne le dit pas, et ce qu'il corrige ne se devine pas (regle 1).
    if reel && !apercu.est_signalement() && compagnie::pieces_ecrites::versions_portees_de(&ctx.racine, &chemin)?.is_none() {
        return Err(erreur(
            Classe::Refuse,
            format!(
                "{} n'a pas été écrite par le logiciel : ce qu'elle corrige des mois passés ne se lit pas. Rien n'est parti. Déposer la DSN du mois (route deposer, avec le mois), qui s'écrit ici.",
                socle::fichiers::nom_du(&chemin)
            ),
        ));
    }
    if reel {
        forme = verifier_preuve(ctx, preuve, &apercu.confirmation_attendue().map_err(|m| erreur(Classe::Refuse, m))?, "depot REEL d'une DSN", "net-entreprises (la declaration part a l'Urssaf et aux organismes)")?;
        // PAS DEUX FOIS SANS LE VOULOIR. Une DSN reelle deja partie pour ce mois, et pas refusee, ne repart pas.
        // UN SIGNALEMENT N'EST PAS CONCERNE : plusieurs peuvent partir le meme mois.
        if !apercu.est_signalement() {
            let etat_mois = compagnie::etat_dsn::du_mois(ctx, le_mois, nic)?;
            // L'ANNULE-ET-REMPLACE EN REGLE PART SANS QU'ON FORCE RIEN (ses gardes passent toujours).
            let remplace = verifier_depot_remplacement(ctx, le_mois, &apercu, nic)?;
            if etat_mois.partie() && !remplace && !forcer {
                return Err(avec_etat(
                    Classe::DsnDejaDeposee,
                    format!(
                        "{}{}. Rien n'est parti : la plate-forme n'accepte pas une seconde DSN normale pour le même mois (cahier technique DSN §1.4.3.2). Pour corriger, refaites la DSN du mois : elle sortira en annule-et-remplace.",
                        { let l = socle::mois::Mois::en_clair(&mois); l[..1].to_uppercase() + &l[1..] },
                        etat_mois.phrase.iter().map(|p| format!(" : {}", p)).collect::<String>()
                    ),
                    &etat_mois,
                ));
            }
        }
        // LA PAIE A CHANGE DEPUIS L'ECRITURE : le fichier est peut-etre conforme a son test, il ne
        // declare plus la paie.
        let change = if apercu.est_signalement() {
            None
        } else {
            crate::etapes::Moteur::nouveau(ctx).paie_changee_depuis_l_ecriture(le_mois, &chemin)?
        };
        if let Some(c) = change {
            return Err(erreur(
                Classe::Refuse,
                format!(
                    "{} : {}. Rien n'est parti. Reecrire le mois, deposer la nouvelle DSN en test, puis l'envoyer.",
                    socle::fichiers::nom_du(&chemin),
                    c
                ),
            ));
        }
        // Et la repetition, en DERNIER des garde-fous.
        repetition = Some(exiger_repetition(&lignes, Acte::Dsn, &chemin, contournement)?);
    }

    let joue = branchement.joue();
    let mut mdp: Option<String> = None;
    if !joue {
        mdp = motdepasse_organisme(ctx, "net-entreprises", preuve.and_then(|p| p.motdepasse.as_deref()))?;
    }
    if reel && !joue {
        refuser_si_copie(ctx, "une DSN reelle")?;
    }

    // UN ESSAI DE PLUS SUR UN MOIS DEJA ESSAYE part en annule-et-remplace, a l'ordre suivant : la base de test
    // garde les essais. JAMAIS PLUS BAS QUE L'ORDRE DU FICHIER.
    let ordre_test = if reel || apercu.est_signalement() { None } else { prochain_ordre_de_test(ctx, le_mois, nic, &mois_de_constitution(&apercu)?)? };
    let ordre_test = ordre_test.max(ordre_d_essai);
    let ordre_test = ordre_d_essai_du_fichier(ordre_test, &chemin)?;

    // LA PAIE DECLAREE : la version de chaque engagement que la DSN emporte -- pour la mensuelle, ceux du mois qui relevent
    // de son etablissement (S21.G00.11.001 ; une paie qui change ensuite appelle un annule-et-remplace,
    // `engagement::etats::a_refaire`) ; pour un signalement, celui dont il cite le contrat (S21.G00.40.009) dans le mois
    // de son evenement.
    let versions = if apercu.est_signalement() {
        versions_du_signalement(ctx, le_mois, &octets)?
    } else {
        let nic = match apercu.etablissement().filter(|n| !n.is_empty()) {
            Some(n) => n.to_string(),
            None => ctx.entreprise.nic.ecrite("nic")?.to_string(),
        };
        // Un etablissement qui ne se lit pas refuse le depot : la premiere faute, rendue apres la lecture.
        let faute: std::cell::RefCell<Option<Echec>> = std::cell::RefCell::new(None);
        let garder = |_: &str, _: &str, e: &engagement::stock::Ecrit| match crate::etapes::nic_de_l_engagement_ecrit(ctx, e) {
            Ok(n) => n.as_deref() == Some(nic.as_str()),
            Err(x) => {
                faute.borrow_mut().get_or_insert(x);
                false
            }
        };
        let mut v = envois::journal::versions_du_mois(ctx, &mois, &envois::journal::lu(&ctx.racine).map(|j| j.departs)?, garder).map_err(Echec::from)?;
        if let Some(x) = faute.into_inner() {
            return Err(x);
        }
        // Et ce qu'elle corrige des mois passes, tel que sa fabrication l'a note : un fichier qui part tel qu'il a ete
        // ecrit emporte ce qu'il porte, pas ce qu'une DSN fabriquee aujourd'hui porterait (regle 4). Un essai d'un
        // fichier que le moteur n'a pas ecrit n'emporte rien ; en reel, il est refuse plus haut.
        if let Some(portees) = compagnie::pieces_ecrites::versions_portees_de(&ctx.racine, &chemin)? {
            v.extend(portees);
        }
        v
    };
    let d = envois::depot_acte::Depot { chemin: &chemin, octets: &octets, mois: le_mois, evenement, reel, qui, preuve: &forme, repetition: repetition.as_ref(), motdepasse: mdp, versions: &versions };
    let mut suites = Suites::default();
    let fait = envois::depot_acte::deposer_dsn(ctx, branchement, &d, &apercu, environnement, ordre_test, &mut suites);
    let faites = crate::gestes::suites::faire(ctx, suites)?;
    let mut fait = fait?;
    fait.suites.extend(faites.incidents.into_iter().chain(faites.a_faire));
    Ok(fait)
}

// ----------------------------------------------------------------------- la DPAE

/// Depose une DPAE. Meme ordre, memes regles que la DSN. La DPAE
/// declare des EMBAUCHES, et le journal retient lesquelles (salarie, premier jour).
pub(crate) fn deposer_dpae(
    ctx: &Contexte,
    fichier: &str,
    reel: bool,
    preuve: Option<&Preuve>,
    qui: &str,
    majuscules: bool,
    contournement: &str,
) -> R<DepotFait> {
    deposer_dpae_avec(ctx, fichier, reel, preuve, qui, majuscules, contournement, None)
}

/// `deposer_dpae` avec, au besoin, le client Urssaf que l'appelant donne a la place du vrai (la preuve : un client pointe sur
/// un serveur local). Un client donne n'est jamais confronte au coffre, et ses identifiants sont verifies comme ceux
/// d'un client construit ici. Sans client, rien ne change.
#[allow(clippy::too_many_arguments)]
pub(crate) fn deposer_dpae_avec(
    ctx: &Contexte,
    fichier: &str,
    reel: bool,
    preuve: Option<&Preuve>,
    qui: &str,
    majuscules: bool,
    contournement: &str,
    client_donne: Option<&mut Client>,
) -> R<DepotFait> {
    let branchement = compagnie::bridage::branchement(&ctx.racine)?;
    branchement.ouvrir()?;
    let chemin = PathBuf::from(crate::depots::resoudre(&ctx.racine, fichier)?);
    let apercu = crate::depots::apercu_depot_dpae(ctx, &chemin.to_string_lossy())?;
    let octets = crate::depots::lire_octets(&chemin.to_string_lossy())?;
    let le_mois = apercu.mois.ok_or_else(|| erreur(Classe::Refuse, "Ce fichier ne déclare aucune embauche datée : il ne dit pas son mois. Rien n'est parti."))?;
    let mois = le_mois.to_string();

    if reel && !apercu.reel {
        return Err(erreur(
            Classe::Refuse,
            format!(
                "Le fichier porte l'indicateur {} (envoi de test) : déposé en réel, il ne déclarerait rien. Rien n'est parti : déposez le fichier du mois, que la route deposer écrit elle-même.",
                apercu.indicateur
            ),
        ));
    }

    let (mut forme, mut repetition) = (String::new(), None);
    if reel {
        forme = verifier_preuve(ctx, preuve, &mois, "depot REEL d'une DPAE", "l'Urssaf (declaration d'embauche)")?;
        // LA REPETITION, apres la preuve : le fichier sur le disque porte toujours l'indicateur reel, et c'est
        // NOUS qui le basculons en test juste en dessous.
        let lignes = envois::journal::lu(&ctx.racine).map(|j| j.lignes)?;
        repetition = Some(exiger_repetition(&lignes, Acte::Dpae, &chemin, contournement)?);
    }

    let mut contenu = octets.clone();
    if !reel {
        contenu = depot::basculer_dpae(&contenu, depot::LONGUEUR_DPAE)?;
    }

    let joue = branchement.joue();
    let mut client: Option<Client> = None;
    let mut donne = client_donne;
    if !joue {
        let soucis = match donne.as_deref() {
            Some(c) => c.verifier_identifiants()?,
            None => {
                let mdp = motdepasse_organisme(ctx, "urssaf", preuve.and_then(|p| p.motdepasse.as_deref()))?;
                let c = Client::dpae(ctx, majuscules, mdp);
                let soucis = c.verifier_identifiants()?;
                client = Some(c);
                soucis
            }
        };
        if !soucis.is_empty() {
            return Err(erreur(Classe::Refuse, format!("Ces points feraient echouer la connexion MtoM. Rien n'est envoye :\n  {}", soucis.join("\n  "))));
        }
        if depot::est_reel(&contenu) {
            refuser_si_copie(ctx, "une DPAE reelle")?;
        }
    }

    let embauches: Vec<EmbaucheDeclaree> = apercu
        .embauches
        .iter()
        .map(|l| {
            Ok(EmbaucheDeclaree {
                salarie: IdSalarie::lire(l.salarie.as_deref().ok_or_else(|| format!("{} : aucune fiche ne porte l'identité de cette embauche du fichier.", l.nom))?)?,
                numero: l.numero.as_deref().map(NumeroContrat::lire).transpose()?,
                debut: l.debut.clone(),
                fin: l.fin.clone(),
            })
        })
        .collect::<Result<_, String>>()
        .map_err(Echec::from)?;
    // La version de chaque engagement dont l'embauche part : une date d'embauche qui change ensuite le dit.
    let declare = |cle: &str, n: &str| embauches.iter().any(|x| x.salarie.as_str() == cle && x.numero.as_deref() == Some(n));
    let versions = envois::journal::versions_du_mois(ctx, &mois, &envois::journal::lu(&ctx.racine).map(|j| j.departs)?, |cle, n, _| declare(cle, n)).map_err(Echec::from)?;
    let d = envois::depot_acte::Depot { chemin: &chemin, octets: &octets, mois: le_mois, evenement: None, reel, qui, preuve: &forme, repetition: repetition.as_ref(), motdepasse: None, versions: &versions };
    let c = match donne.as_deref_mut() {
        Some(c) => Some(c),
        None => client.as_mut(),
    };
    let mut suites = Suites::default();
    let fait = envois::depot_acte::deposer_dpae(ctx, branchement, &d, &contenu, embauches.clone(), c, apercu.deja_depose.clone(), &mut suites);
    let faites = crate::gestes::suites::faire(ctx, suites)?;
    let mut fait = fait?;
    fait.suites.extend(faites.incidents.into_iter().chain(faites.a_faire));
    // UN ACTE, UNE PREUVE : la demande de taux part sous celle de la DPAE.
    let meme = reel.then(|| Preuve { donnee: Some(forme.clone()), motdepasse: preuve.and_then(|p| p.motdepasse.clone()), mois_recopie: preuve.and_then(|p| p.mois_recopie.clone()) });
    fait.suites.extend(deposer_les_amorcages(ctx, &mois, &embauches, reel, meme.as_ref().or(preuve), qui, contournement));
    Ok(fait)
}

/// LA DEMANDE DE TAUX DE PAS PART AVEC LA DPAE : pour chaque embauche de la DPAE de `mois` dont le salarie n'a pas de
/// taux valide au jour de son versement, et dont aucune demande n'attend sa reponse (`amorcage_demande::a_combler`), le
/// signalement d'amorcage (nature 08, motif 01) s'ecrit dans le geste, a la suite des signalements deja partis, et part
/// dans le meme mode, sous la meme preuve, par le chemin de tout signalement (`deposer_dsn`). En reel, il part sur le
/// bilan de son test : le test de ces octets se fait d'abord s'il n'est pas fait (un depot de test ne conserve rien,
/// regle 10), et s'il n'est pas encore releve, les retours de la journee se relevent une fois. La DGFiP repond sous cinq
/// jours ouvres (compte rendu de nature 94). Il ne fait jamais echouer la DPAE, deja partie : une demande qui ne part pas
/// reste a faire, et se dit. Rend ce qu'est devenue chaque demande, en une phrase.
fn deposer_les_amorcages(ctx: &Contexte, mois: &str, embauches: &[EmbaucheDeclaree], reel: bool, preuve: Option<&Preuve>, qui: &str, contournement: &str) -> Vec<String> {
    use compagnie::amorcage_demande as ad;
    let mode = if reel { "" } else { " en test" };
    let paies = match paies_des_embauches(ctx, mois, embauches) {
        Ok(p) => p,
        Err(e) => return vec![format!("La demande de taux de prélèvement à la source n'est pas partie{} : {}", mode, e.phrase())],
    };
    let lignes = match ad::a_combler(ctx, &paies, declarations::amorcage::EMBAUCHE, false) {
        Ok(l) => l,
        Err(e) => return vec![format!("La demande de taux de prélèvement à la source n'est pas partie{} : {}", mode, e.phrase())],
    };
    let (retenues, ecartees): (Vec<ad::Ligne>, Vec<ad::Ligne>) = lignes.into_iter().partition(|l| l.pourquoi_pas.is_empty());
    // UNE PHRASE PAR RAISON, pas par salarie : la phrase du depot tient dans sa place quel que soit le nombre d'embauches.
    let noms = |v: &[String]| v.join(", ");
    let mut par_raison: Vec<(String, Vec<String>)> = Vec::new();
    for l in &ecartees {
        match par_raison.iter_mut().find(|(r, _)| *r == l.pourquoi_pas) {
            Some((_, v)) => v.push(ctx.nomme(&l.salarie)),
            None => par_raison.push((l.pourquoi_pas.clone(), vec![ctx.nomme(&l.salarie)])),
        }
    }
    let mut dit: Vec<String> = par_raison.iter().map(|(r, v)| format!("Taux de PAS non demandé pour {} : {}.", noms(v), r)).collect();
    if retenues.is_empty() {
        return dit;
    }
    let fichiers = match ad::ecrire(ctx, &paies, &retenues, declarations::amorcage::EMBAUCHE, mois, ctx.aujourdhui, false) {
        Ok(f) => f,
        Err(e) => {
            dit.push(format!("La demande de taux de prélèvement à la source n'est pas partie{} : {}", mode, e.phrase()));
            return dit;
        }
    };
    let teste = |f: &Path| -> bool {
        let (Ok(octets), Ok(lignes)) = (std::fs::read(f), envois::journal::lu(&ctx.racine).map(|j| j.lignes)) else { return false };
        let nom = socle::fichiers::nom_du(f);
        etats::repetition(&lignes, Acte::Dsn, &envois::journal::empreinte(&octets), &nom).conforme.is_some()
    };
    let mut releve = false;
    let mut parties: Vec<String> = Vec::new();
    for (l, f) in retenues.iter().zip(&fichiers) {
        let fichier = f.to_string_lossy().into_owned();
        let mut fait = if reel && !teste(f) { deposer_dsn(ctx, &fichier, false, preuve, qui, false, contournement).map(|_| ()) } else { Ok(()) };
        if fait.is_ok() {
            fait = deposer_dsn(ctx, &fichier, reel, preuve, qui, false, contournement).map(|_| ());
        }
        if reel && !releve && matches!(&fait, Err(e) if e.classe == Some(Classe::RepetitionManquante)) {
            releve = true;
            let (debut, fin) = envois::releve_retours::derniere_journee();
            let _ = crate::gestes::releve::relever_retours(ctx, debut, fin, false, "production", None, None);
            fait = deposer_dsn(ctx, &fichier, reel, preuve, qui, false, contournement).map(|_| ());
        }
        let qui_demande = ctx.nomme(&l.salarie);
        match fait {
            Ok(_) => parties.push(qui_demande),
            Err(e) => dit.push(format!("La demande de taux de prélèvement à la source de {} n'est pas partie{} : {} Elle reste à demander.", qui_demande, mode, e.phrase())),
        }
    }
    if !parties.is_empty() {
        dit.insert(0, format!("Taux de PAS demandé{} avec la DPAE pour {}.", mode, noms(&parties)));
    }
    dit
}

/// Les engagements valides de `mois` que nomment les embauches d'une DPAE (`salarie`, `numero`), vus comme des embauches.
fn paies_des_embauches(ctx: &Contexte, mois: &str, embauches: &[EmbaucheDeclaree]) -> R<Vec<engagement::calcul::Paie>> {
    let o = |m: String| erreur(Classe::Refuse, m);
    let departs = envois::journal::lu(&ctx.racine).map_err(o)?.departs;
    let Some((data, _)) = engagement::etats::validees_du_mois(ctx, mois, &departs).map_err(o)? else { return Ok(Vec::new()) };
    let declaree = |p: &engagement::calcul::Paie| {
        let cle = p.salarie.id.to_string();
        embauches.iter().any(|e| e.salarie.as_str() == cle && e.numero.as_deref() == p.engagement.numero.as_deref())
    };
    let mut paies = Vec::new();
    for e in engagement::engagements_de_la_saisie(ctx, &data).map_err(o)? {
        let p = engagement::embauche(ctx, &e).map_err(o)?;
        if declaree(&p) {
            paies.push(p);
        }
    }
    Ok(paies)
}
