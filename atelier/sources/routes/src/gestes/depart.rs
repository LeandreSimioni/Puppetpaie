//! CE QUI PART SE FABRIQUE A SON DEPART, depuis l'engagement valide (`engagement::etats::validees_du_mois`) :
//!
//! - la DSN du mois, quand on la depose (`fabriquer_dsn`, que la route `depot-apercu` appelle) : des qu'un engagement du mois est
//!   valide, sans attendre ni DPAE ni contrat. Le moteur dit si c'est un depot initial ou un annule-et-remplace ; apres la
//!   veille de l'echeance, un mois deja declare ne se redepose plus (`AnnuleRemplaceHorsDelai`, cahier technique DSN
//!   §1.4.2) : sa correction part dans la DSN d'un mois suivant (`produire_regularisations::regularisations_pour`) ;
//! - le bulletin et le certificat Conges Spectacles, quand les documents partent au salarie (`fabriquer_documents`, la
//!   route `envoyer-documents`). Le numero du bulletin (`N/AAAA`, suite de l'annee) et le premier N CE du certificat
//!   s'ecrivent alors dans la paie de l'engagement (`DocumentsFabriques::engager`) et se relisent ensuite. Une paie partie
//!   puis refaite part sous un numero nouveau, comme un bulletin rectificatif de celui qui est parti, et l'original reste
//!   a cote ; quand la DSN du mois ne se refait plus, le rectificatif porte l'ecart avec le bulletin parti. Chaque paie a un
//!   seul bulletin et un seul paiement : la DSN ne fabrique aucun bulletin.
//!
//! L'AER ne se fabrique pas : France Travail la renvoie apres la DSN, et elle part avec les documents quand elle est la.
//! Un numero de contrat s'ecrit dans l'engagement a sa validation (`engagement::numeros`) ; tout ce qui part le lit la.

use droit::fiche::Lue as _;
use socle::echec::Classe;
use socle::mois::{Graphie, Mois};
use socle::paris::{HeureDuJournal, InstantEcrit, Moment};
use crate::dossier_salarie::cle_de;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use documents::conges_spectacles::{self, PaieCs};
use declarations::produire_regularisations::{self as regularisations, Reg};
use crate::etapes::Moteur;
use compagnie::pieces_ecrites::{cle_de_piece, journal_des_ecritures, noter_piece_ecrite, refuser_si_partie, EmpreinteNotee};
use droit::contexte::Contexte;
use engagement::calcul::Paie;
use engagement::Avis;
use socle::echec::erreur;
use socle::echec::Echec;
use socle::echec::R;
use documents::ecrire_documents::{Reserves, Suite};

fn info(texte: impl Into<String>) -> Avis {
    Avis { info: true, texte: texte.into() }
}

fn avertissement(texte: impl Into<String>) -> Avis {
    Avis { info: false, texte: texte.into() }
}

fn os(chemin: &Path, e: std::io::Error) -> Echec {
    socle::echec::du_disque(chemin, &e).reclasser(Classe::Disque)
}


// ------------------------------------------------------------- le journal des pieces ecrites

/// LA DSN DU MOIS EST-ELLE ECRITE ? Une DSN mensuelle du mois notee au journal des ecritures, et toujours la ou le moteur
/// l'a ecrite (`sorties/<mois>/`). Un fichier DSN que le moteur n'a pas note (pose a la main) ne compte pas. C'est le
/// critere de l'accueil (`crate::ouverture::bilan_des_mois`), des rappels (`crate::rappels::dsn_deposable`) et de
/// l'etape de depot (`routes::etapes`).
pub(crate) fn mois_ecrit(racine: &Path, mois: Mois) -> R<bool> {
    let prefixe = format!("{}/", mois);
    let sorties = racine.join("sorties");
    Ok(journal_des_ecritures(racine)?
        .keys()
        .any(|k| k.strip_prefix(&prefixe).is_some_and(crate::etapes::est_une_dsn_mensuelle) && sorties.join(k).is_file()))
}

// ------------------------------------------------------------- la DSN, a son depot

/// Ce que doit etre la prochaine DSN REELLE de ce mois : `(type, ordre, remplace)`. Normale d'ordre 1 si rien n'est
/// passe ; sinon annule-et-remplace au rang suivant -- et un refus si son delai est passe.
fn forme_de_la_dsn(ctx: &Contexte, m: &mut Moteur, le_mois: Mois, nic: Option<&str>) -> R<(String, i64, bool)> {
    let faits = m.depots_reels(le_mois, nic)?;
    let dernier = m.dernier_ordre(le_mois, &faits)?;
    // CE QUE LA PLATE-FORME A VU EN REEL ce mois civil, lu en silence dans les bilans archives
    // (`ordre_signalements::dernier_ordre_des_mensuelles`) : le rang se compte « au cours du mois courant » (cahier
    // technique DSN, p. 136), toutes declarations du SIREN confondues, d'ou qu'elles soient parties (un autre poste).
    let vues = envois::ordre_signalements::dernier_ordre_des_mensuelles(&ctx.racine, &envois::ordre_signalements::siren_de(ctx)?, false, ctx.aujourdhui.mois_civil(), le_mois)?;
    let dernier = match dernier {
        Some(d) => d.max(vues.ordre),
        // Une mensuelle du mois acceptee en reel que le journal ignore : la suivante la remplace.
        None if vues.du_mois_acceptee => vues.ordre.max(1),
        None => return Ok(("01".into(), vues.ordre + 1, false)),
    };
    m.refuser_si_hors_delai(le_mois)?;
    Ok(("03".into(), dernier + 1, true))
}

/// La declaration qu'un annule-et-remplace remplace (la derniere DSN mensuelle partie du mois, `dsn_parties`)
/// portait-elle un versement du prelevement a la source (S21.G00.20.002 = DGFIP_PAS) ? `None` : aucune n'est partie.
fn remplacee_versait_pas_de(ctx: &Contexte, m: &mut Moteur, mois: Mois, nic: Option<&str>) -> R<Option<bool>> {
    Ok(dsn_parties(ctx, m, mois, nic)?.last().map(verse_a_la_dgfip))
}

/// La DSN porte-t-elle un versement du prelevement a la source ?
fn verse_a_la_dgfip(f: &declarations::dsn::Fabriquee) -> bool {
    f.ordres.iter().any(|o| o.reference.as_deref() == Some(declarations::dsn::DGFIP_PAS))
}

// ------------------------------------------------------------- ce qu'une DSN porte

/// La forme d'une DSN mensuelle : son en-tete, l'etablissement qu'elle declare, et le jour de sa constitution.
struct Forme {
    type_dsn: String,
    ordre: i64,
    /// Le NIC de l'etablissement ; `None` : la compagnie n'en a qu'un.
    etablissement: Option<String>,
    constitution: socle::date::Jour,
    remplacee_versait_pas: Option<bool>,
}

impl Forme {
    fn options(&self) -> declarations::dsn::Options {
        declarations::dsn::Options {
            date_constitution: Some(self.constitution),
            envoi_test: false,
            type_declaration: self.type_dsn.clone(),
            ordre: self.ordre,
            etablissement: self.etablissement.clone(),
            remplacee_versait_pas: self.remplacee_versait_pas,
            ..Default::default()
        }
    }
}
/// LA DSN DE `mois` TELLE QU'ELLE PARTIRAIT, sous la forme `forme` : depuis les paies ecrites des engagements valides du
/// mois de son etablissement, avec ce que `fabriquer_dsn` lui donne (les corrections a porter, les arrets du mois).
fn fabriquer_du_present(ctx: &Contexte, lu: &Lu, mois: Mois, forme: &Forme) -> R<declarations::dsn::Fabriquee> {
    lu.present(ctx, mois, forme)?.fabriquer(ctx, mois, forme)
}

/// CE QUE LA DSN DE `mois` LIT, tous etablissements confondus : ce qui est parti, les paies ecrites des engagements
/// valides du mois, les corrections d'autres mois a porter, les arrets du mois. Une fabrication le lit une fois (ses
/// avis, la DSN de chaque etablissement, le fichier deja ecrit qu'elle compare) : rien ne s'ecrit entre deux lectures.
struct Lu {
    departs: engagement::departs::Departs,
    paies: Vec<Paie>,
    regs: Vec<Reg>,
    arrets: Vec<declarations::dsn::Arret>,
}

impl Lu {
    fn du_mois(ctx: &Contexte, mois: Mois) -> R<Lu> {
        let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
        let (_data, paies) = documents::ecrire_documents::paies_aux_numeros_declares(ctx, mois, None, &departs)?;
        let regs = regularisations::regularisations_pour(ctx, mois, &departs)?;
        let arrets = crate::gestes::marques_depot::arrets_pour(ctx, mois, &departs)?;
        Ok(Lu { departs, paies, regs, arrets })
    }

    /// Ce que la DSN de l'etablissement de `forme` en declare.
    fn present(&self, ctx: &Contexte, mois: Mois, forme: &Forme) -> R<Present> {
        let fiche = ctx.entreprise_du_mois(mois);
        let nic = forme.etablissement.clone();
        let du_nic = |sal: &droit::fiche::SalarieDuContrat| -> R<bool> {
            match &nic {
                Some(n) => Ok(droit::bareme::etablissement_de(&fiche, sal).map_err(|e| e.reclasser(Classe::Illisible))? == *n),
                None => Ok(true),
            }
        };
        let mut paies: Vec<Paie> = Vec::new();
        for p in &self.paies {
            if du_nic(&p.salarie)? {
                paies.push(p.clone());
            }
        }
        let mut regs_e: Vec<&Reg> = Vec::new();
        for r in &self.regs {
            if du_nic(&r.apres.salarie)? {
                regs_e.push(r);
            }
        }
        let regs_dsn = regularisations::pour_la_dsn(&regs_e);
        let mut arrets: Vec<declarations::dsn::Arret> = Vec::new();
        for a in &self.arrets {
            let sal = engagement::imprevus::fiche_du_salarie(ctx, &a.saisi).map_err(|e| e.reclasser(Classe::Illisible))?;
            if du_nic(&sal)? {
                arrets.push(declarations::dsn::Arret { saisi: a.saisi.clone() });
            }
        }
        Ok(Present { paies, regs: regs_dsn, arrets, departs: self.departs.clone() })
    }
}

/// CE QU'UNE DSN DE `mois` DECLARERAIT AUJOURD'HUI pour l'etablissement de `forme` : les paies ecrites de ses engagements
/// valides, les corrections d'autres mois a porter, les arrets du mois.
struct Present {
    paies: Vec<Paie>,
    regs: Vec<declarations::dsn::Regularisation>,
    arrets: Vec<declarations::dsn::Arret>,
    departs: engagement::departs::Departs,
}

impl Present {
    /// Ni paie ni correction : rien a declarer (un mois tenu avant ce logiciel, un mois sans engagement).
    fn vide(&self) -> bool {
        self.paies.is_empty() && self.regs.is_empty()
    }

    fn fabriquer(&self, ctx: &Contexte, mois: Mois, forme: &Forme) -> R<declarations::dsn::Fabriquee> {
        declarations::dsn::fabriquer(ctx, &self.paies, mois, &self.regs, &self.arrets, &forme.options(), &self.departs).map_err(|e| e.reclasser(Classe::Illisible))
    }
}

/// LA DSN QUE PORTE LE FICHIER `chemin` (mois principal `mois`, etablissement `nic` ; le principal sans lui), fabriquee
/// de nouveau depuis les paies des engagements qu'elle couvre (`fabriquer_du_present`), sous la forme que son en-tete
/// dit, au jour ou elle a ete constituee. `None` : le fichier ne dit plus ce que disent les paies, il se refait.
pub(crate) fn refabriquer(ctx: &Contexte, chemin: &Path, mois: socle::mois::Mois, nic: Option<&str>) -> R<Option<declarations::dsn::Fabriquee>> {
    refabriquer_sur(ctx, None, chemin, mois, nic)
}

/// `refabriquer`, sur ce que la fabrication en cours a deja lu (`lu`), ou sur ce qui se lit maintenant.
fn refabriquer_sur(ctx: &Contexte, lu: Option<&Lu>, chemin: &Path, mois: Mois, nic: Option<&str>) -> R<Option<declarations::dsn::Fabriquee>> {
    // Un fichier qui n'est pas la (encore jamais ecrit) se fait ; un fichier qui ne se lit pas se refuse.
    let octets = match std::fs::read(chemin) {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(os(chemin, e)),
    };
    let entete = declarations::dsn_lue::lire(&octets, "")?;
    // Une DSN dont l'en-tete ne dit pas sa constitution, son type ou son ordre n'est pas celle du moteur : elle se refait.
    let (Some(constitution), Some(type_dsn), Some(ordre)) = (entete.depot, entete.type_declaration.clone(), entete.ordre.as_deref().map(str::trim).map(str::parse::<i64>)) else {
        return Ok(None);
    };
    let ordre = ordre.map_err(|e| erreur(Classe::Illisible, format!("{} : numéro d'ordre illisible ({}).", socle::fichiers::nom_du(chemin), e)))?;
    let plusieurs = !ctx.entreprise_du_mois(mois).secondaires().is_empty();
    let nic = match nic.filter(|n| !n.is_empty()) {
        Some(n) => n.to_string(),
        None => ctx.entreprise.nic.ecrite("nic").map_err(|e| e.reclasser(Classe::Illisible))?.to_string(),
    };
    let remplacee_versait_pas = if type_dsn == "03" || type_dsn == "05" {
        let mut m = Moteur::nouveau(ctx);
        remplacee_versait_pas_de(ctx, &mut m, mois, plusieurs.then_some(nic.as_str()))?
    } else {
        None
    };
    let forme = Forme { type_dsn, ordre, etablissement: plusieurs.then_some(nic), constitution, remplacee_versait_pas };
    let f = match lu {
        Some(lu) => fabriquer_du_present(ctx, lu, mois, &forme)?,
        None => fabriquer_du_present(ctx, &Lu::du_mois(ctx, mois)?, mois, &forme)?,
    };
    Ok((declarations::dsn::en_latin1(&f.texte).map_err(|e| e.reclasser(Classe::Illisible))? == octets).then_some(f))
}

/// L'engagement de `mois` du salarie `cle` au contrat `numero`, actif ou non.
fn engagement_de(ctx: &Contexte, mois: Mois, cle: &str, numero: &str) -> R<engagement::stock::Objet> {
    engagement::stock::objets_du_mois(&ctx.racine, mois)?
        .into_iter()
        .chain(engagement::stock::non_actifs_du_mois(&ctx.racine, mois)?)
        .find(|o| {
            let b = &*o.brut;
            b.saisie.carte.salarie.as_deref() == Some(cle) && b.saisie.carte.numero.as_deref() == Some(numero)
        })
        .ok_or_else(|| erreur(Classe::Illisible, format!("{} : aucun engagement de {} ne porte le contrat {} qu'une DSN partie a declare.", cle, mois.libelle(socle::mois::Graphie::Accentuee), numero)))
}

/// La paie de `mois` que la version `version` de l'engagement `o` (salarie `cle`, contrat `numero`) a ecrite
/// (`engagement::etats::paie_de_la_version_n`).
fn paie_de_la_version(ctx: &Contexte, o: &engagement::stock::Objet, cle: &str, numero: &str, mois: Mois, version: u64) -> R<Paie> {
    engagement::etats::paie_de_la_version_n(ctx, o, version, mois)?
        .ok_or_else(|| erreur(Classe::Illisible, format!("{} : la version {} du contrat {} n'a pas de paie de {}.", cle, version, numero, mois.libelle(Graphie::Accentuee))))
}

/// Le jour ou la version `n` de l'engagement `o` a ete validee (`le` de la presente, `valide_le` d'une passee) ; `None`
/// si l'engagement ne le dit pas. Un jour qui ne se lit pas se refuse.
fn valide_le(o: &engagement::stock::Objet, n: u64) -> R<Option<socle::date::Jour>> {
    use engagement::stock::Version;
    let Some(le) = o.brut.valide.iter().map(Version::Presente).chain(o.brut.versions.iter().map(Version::Passee)).find(|v| v.numero() == Some(n)).and_then(|v| v.valide_le()) else {
        return Ok(None);
    };
    Ok(Some(le.jour()))
}

/// LA DSN MENSUELLE PARTIE que dit la ligne `lignes[rang]` du journal, fabriquee de nouveau par la fonction qui l'a
/// fabriquee (`dsn::fabriquer`) depuis les versions qu'elle a emportees (`versions` de sa ligne) : les paies de son mois,
/// et les corrections des mois passes qu'elle portait -- leur paie d'apres, la version emportee ; ce qu'elles annulaient,
/// le dernier etat declare par ce qui etait parti avant elle (`versement_declare::dernier_declare` sur les lignes qui la
/// precedent) ; les arrets du mois de son etablissement, qu'elle reporte. `remplacee_versait_pas` : sur un
/// annule-et-remplace, si la DSN remplacee versait a la DGFiP.
fn dsn_partie(ctx: &Contexte, m: &mut Moteur, lignes: &[envois::journal::Ligne], rang: usize, remplacee_versait_pas: Option<bool>) -> R<declarations::dsn::Fabriquee> {
    let l = &lignes[rang];
    let e = l.dsn().ok_or_else(|| erreur(Classe::Illisible, "journal : la DSN partie n'est pas une DSN".to_string()))?;
    let le_mois = e.mois.ok_or_else(|| erreur(Classe::Illisible, "journal : une DSN partie sans son mois".to_string()))?;
    let mois = le_mois.to_string();
    let type_dsn = e.detail.type_declaration.clone().filter(|t| !t.is_empty()).ok_or_else(|| erreur(Classe::Illisible, "journal : une DSN partie sans son type".to_string()))?;
    let plusieurs = !ctx.entreprise_du_mois(le_mois).secondaires().is_empty();
    let nic = match l.etablissement() {
        Some(n) => n.to_string(),
        None => ctx.entreprise.nic.ecrite("nic").map_err(|e| e.reclasser(Classe::Illisible))?.to_string(),
    };
    // Constituee le jour ou son fichier a ete ecrit (le journal des ecritures), sinon le jour de son depot.
    let fichier = l.fichier().ok_or_else(|| erreur(Classe::Illisible, "journal : une DSN partie sans son fichier".to_string()))?;
    let ecrite = journal_des_ecritures(&ctx.racine)?.remove(&format!("{}/{}", mois, fichier)).map(|n| n.quand);
    let constitution = match ecrite {
        Some(q) => q.jour(),
        None => l.quand().jour(),
    };
    let forme = Forme { type_dsn, ordre: m.ordre_de_la_ligne(le_mois, l)?, etablissement: plusieurs.then_some(nic), constitution, remplacee_versait_pas };
    let avant = envois::journal::departs_des_lignes(&lignes[..rang]);
    let ctx_m = ctx.du_mois(le_mois).map_err(|e| e.reclasser(Classe::Illisible))?;
    let mut paies: Vec<Paie> = Vec::new();
    let mut regs: Vec<declarations::dsn::Regularisation> = Vec::new();
    for v in l.versions().into_iter().flatten() {
        let (cle, numero, version) = (v.salarie.to_string(), v.numero.to_string(), v.version);
        let Some(c) = v.mois.filter(|&c| c != le_mois) else {
            let o = engagement_de(ctx, le_mois, &cle, &numero)?;
            paies.push(paie_de_la_version(&ctx_m, &o, &cle, &numero, le_mois, version)?);
            continue;
        };
        let ctx_c = ctx.du_mois(c).map_err(|e| e.reclasser(Classe::Illisible))?;
        let o = engagement_de(ctx, c, &cle, &numero)?;
        let dernier = declarations::versement_declare::dernier_declare(&ctx_c, &o, &avant, c, le_mois).map_err(|e| e.reclasser(Classe::Illisible))?;
        // La version d'un non actif : la correction annulait ce qui avait ete declare (`Paie::annulee`), le jour ou il
        // a cesse d'etre actif. Sinon, le jour ou la version qu'elle emporte a ete validee.
        let (apres, decidee_le) = match (engagement::etats::NonActif::de(o.clone()).filter(|n| n.version() == version), &dernier) {
            (Some(n), Some(d)) => (d.avant.annulee(), Some(n.le().jour())),
            _ => (paie_de_la_version(&ctx_c, &o, &cle, &numero, c, version)?, valide_le(&o, version)?),
        };
        regs.push(match dernier {
            Some(d) => declarations::dsn::Regularisation {
                mois: c,
                avant: d.avant,
                taux_pas_origine: d.versement.taux_pas.clone(),
                declare: d.versement,
                apres,
                net_deja_verse: d.net_deja_verse,
                decidee_le,
            },
            // Un contrat ajoute apres la limite : rien n'etait declare ni verse pour lui.
            None => declarations::dsn::Regularisation {
                mois: c,
                avant: apres.clone(),
                declare: declarations::versement_declare::VersementDeclare {
                    fichier: String::new(),
                    net_imposable: socle::num::Decimal::ZERO,
                    net_verse: socle::num::Decimal::ZERO,
                    net_social: socle::num::Decimal::ZERO,
                    rappels_net_social: socle::num::Decimal::ZERO,
                    frais: socle::num::Decimal::ZERO,
                    brut: socle::num::Decimal::ZERO,
                    lignes: Vec::new(),
                    affiliations: Vec::new(),
                    taux_pas: None,
                    debut_contrat: None,
                },
                apres,
                taux_pas_origine: None,
                net_deja_verse: socle::num::Decimal::ZERO,
                decidee_le,
            },
        });
    }
    let mut arrets: Vec<declarations::dsn::Arret> = Vec::new();
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    for a in crate::gestes::marques_depot::arrets_pour(ctx, le_mois, &departs)? {
        if let Some(n) = &forme.etablissement {
            let sal = engagement::imprevus::fiche_du_salarie(ctx, &a.saisi).map_err(|e| e.reclasser(Classe::Illisible))?;
            if droit::bareme::etablissement_de(&ctx.entreprise, &sal).map_err(|e| e.reclasser(Classe::Illisible))? != *n {
                continue;
            }
        }
        arrets.push(a);
    }
    declarations::dsn::fabriquer(ctx, &paies, le_mois, &regs, &arrets, &forme.options(), &departs).map_err(|e| e.reclasser(Classe::Illisible))
}

/// LES DSN MENSUELLES PARTIES de `mois` pour l'etablissement `nic` (`Null` : le principal), reelles et acceptees, dans
/// l'ordre de leur rang, chacune fabriquee de nouveau telle qu'elle est partie (`dsn_partie`) ; un annule-et-remplace
/// sait ainsi si celle qu'il remplacait versait a la DGFiP.
fn dsn_parties(ctx: &Contexte, m: &mut Moteur, mois: Mois, nic: Option<&str>) -> R<Vec<declarations::dsn::Fabriquee>> {
    let lignes = m.lignes()?;
    let mut faits: Vec<(i64, usize)> = Vec::new();
    for l in m.depots_reels(mois, nic)? {
        let Some(rang) = lignes.iter().position(|x| *x == l) else { continue };
        faits.push((m.ordre_de_la_ligne(mois, &l)?, rang));
    }
    faits.sort();
    let mut out: Vec<declarations::dsn::Fabriquee> = Vec::new();
    for (_, rang) in faits {
        let versait = out.last().map(verse_a_la_dgfip);
        out.push(dsn_partie(ctx, m, &lignes, rang, versait)?);
    }
    Ok(out)
}

/// CE QU'UNE DSN DE `mois` FAIT VERSER, pour l'etablissement `nic` (le principal sans lui) : la derniere DSN mensuelle
/// partie du mois, telle qu'elle est partie (ses paies et les corrections d'autres mois qu'elle portait, `dsn_parties`) ;
/// si aucune n'est partie, celle qui partirait aujourd'hui (`fabriquer_du_present`). Rend aussi si elle est partie.
/// `None` : rien n'est parti et rien n'est a declarer pour cet etablissement (ni paie validee, ni correction).
pub(crate) fn dsn_du_mois(ctx: &Contexte, mois: Mois, nic: Option<&str>) -> R<Option<(declarations::dsn::Fabriquee, bool)>> {
    let mut m = Moteur::nouveau(ctx);
    let plusieurs = !ctx.entreprise_du_mois(mois).secondaires().is_empty();
    let nic = match nic.filter(|n| !n.is_empty()) {
        Some(n) => n.to_string(),
        None => ctx.entreprise.nic.ecrite("nic").map_err(|e| e.reclasser(Classe::Illisible))?.to_string(),
    };
    if let Some(f) = dsn_parties(ctx, &mut m, mois, plusieurs.then_some(nic.as_str()))?.pop() {
        return Ok(Some((f, true)));
    }
    let forme = Forme { type_dsn: "01".into(), ordre: 1, etablissement: plusieurs.then_some(nic), constitution: ctx.aujourdhui, remplacee_versait_pas: None };
    let present = Lu::du_mois(ctx, mois)?.present(ctx, mois, &forme)?;
    if present.vide() {
        return Ok(None);
    }
    Ok(Some((present.fabriquer(ctx, mois, &forme)?, false)))
}

/// La DSN du mois, telle que le depot la prend.
pub(crate) struct DsnFabriquee {
    /// Une DSN par etablissement, le principal d'abord.
    pub(crate) fichiers: Vec<PathBuf>,
    pub(crate) avertissements: Vec<Avis>,
}

impl DsnFabriquee {
    /// Ce que la fabrique dit avant le depot : les avertissements, et les informations.
    pub(crate) fn avis(&self) -> (Vec<String>, Vec<String>) {
        let de = |info: bool| self.avertissements.iter().filter(|a| a.info == info).map(|a| a.texte.clone()).collect::<Vec<_>>();
        (de(false), de(true))
    }
}

/// LA DSN DE `mois` NE S'ECRIT NI NE PART AVANT SON PREMIER JOUR DE DEPOT : « le dépôt initial de la DSN "normale" peut
/// être émis jusqu'à 1 mois à l'avance » (cahier technique DSN 2026.1, § 1.4.2,
/// `atelier/reference/norme-dsn/texte/chapitre-1-la-norme.txt` ; `Echeance::premier_depot`). Avant ce jour, le refus dit
/// le jour et l'echeance. Seule `fabriquer_dsn` ecrit une mensuelle : c'est elle qui refuse.
fn pas_avant(ctx: &Contexte, mois: Mois) -> R<()> {
    let echeance = declarations::echeance_dsn::de_la_compagnie(ctx, mois).map_err(|e| e.reclasser(Classe::Illisible))?;
    let premier = echeance.premier_depot().map_err(|e| e.reclasser(Classe::Illisible))?;
    if premier.ordinal() <= ctx.aujourdhui.ordinal() {
        return Ok(());
    }
    Err(erreur(
        Classe::Refuse,
        format!(
            "La DSN de {} se dépose à partir du {} : la norme admet un dépôt au plus un mois avant l'échéance (cahier technique DSN 2026.1, § 1.4.2). Son échéance : {}. Rien n'est parti.",
            mois.libelle(Graphie::Accentuee),
            premier.jj_mm_aaaa(),
            echeance.phrase()
        ),
    ))
}

/// LA DSN DU MOIS, A SON DEPOT : une par etablissement, depuis les paies ecrites des engagements valides du mois, avec
/// les corrections des mois deja declares qu'elle porte, les arrets qu'elle reporte, les suspensions, l'OETH d'avril et
/// la CSA de mars. Un mois deja declare repart en annule-et-remplace, sous un nom a lui (`_AR<ordre>`) : le fichier
/// depose reste tel qu'il est parti. Un fichier deja ecrit qui dit exactement la meme chose est repris tel quel : le test
/// qu'il a passe vaut pour lui.
///
/// Refus : un mois de l'ancien logiciel, sa DSN importee ou non (`import::mois::refuser_la_dsn_de_l_ancien_logiciel`) ;
/// un mois dont aucun engagement n'est valide alors qu'il en a (rien ne part d'un engagement en saisie) ; un mois
/// deja declare dont l'annule-et-remplace n'est plus admis.
pub(crate) fn fabriquer_dsn(ctx: &Contexte, le_mois: Mois, qui: &str) -> R<DsnFabriquee> {
    let (mois, texte) = (le_mois, &le_mois.to_string());
    import::mois::refuser_la_dsn_de_l_ancien_logiciel(ctx, mois)?;
    let _verrou = documents::ecrire_documents::prendre_le_verrou(ctx, qui)?;
    crate::mois::refuser_si_clos(ctx, mois)?;
    pas_avant(ctx, mois)?;
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let mut avertissements: Vec<Avis> = Vec::new();

    // RIEN NE PART D'UN ENGAGEMENT EN SAISIE : la DSN declare les engagements valides du mois.
    let valides = engagement::etats::valides_du_mois(ctx, le_mois)?;
    let en_saisie = valides.iter().filter(|v| !**v).count();
    if en_saisie > 0 && en_saisie == valides.len() {
        return Err(erreur(
            Classe::EngagementEnSaisie,
            format!(
                "{} : aucun engagement n'est valide. Rien ne part d'un engagement en saisie : le valider d'abord (« Valider l'engagement »). Rien n'est ecrit.",
                mois.libelle(Graphie::Accentuee)
            ),
        ));
    }
    if en_saisie > 0 {
        avertissements.push(avertissement(format!(
            "{} engagement(s) de {} encore en saisie : la DSN ne les declare pas. Une fois valides, la DSN se refait.",
            en_saisie,
            mois.libelle(Graphie::Accentuee)
        )));
    }
    let lu = Lu::du_mois(ctx, mois)?;
    let paies = &lu.paies;
    avertissements.extend(sans_taux_recu(ctx, paies.iter())?.into_iter().map(avertissement));
    avertissements.extend(crate::mois::archives_employees(&paies)?.into_iter().map(avertissement));
    for p in paies {
        avertissements.extend(p.avertissements.iter().cloned());
    }
    for p in paies {
        if p.engagement.sans_paie() {
            let contrat = match p.engagement.numero.as_deref() {
                Some(n) => format!("contrat {}", n),
                None => "un contrat sans numéro".to_string(),
            };
            avertissements.push(info(format!(
                "{} : {} en cours, aucun cachet en {} -- la DSN porte le contrat avec un versement a zero.",
                documents::tableaux_export::qui_souple(&p.salarie),
                contrat,
                mois.libelle(Graphie::Accentuee)
            )));
        }
    }

    let fiche = ctx.entreprise_du_mois(mois);
    let principal = fiche.nic_principal();
    let plusieurs = !fiche.secondaires().is_empty();
    let mut m = Moteur::nouveau(ctx);
    // Un mois deja declare dont l'annule-et-remplace n'est plus admis : refuse AVANT d'ecrire quoi que ce soit.
    // Un etablissement par NIC quand la compagnie en a plusieurs ; sinon le principal, sans NIC.
    let nics: Vec<Option<String>> = if plusieurs { fiche.nics().map_err(|e| e.reclasser(Classe::Illisible))?.into_iter().map(Some).collect() } else { vec![None] };
    for nic in &nics {
        forme_de_la_dsn(ctx, &mut m, le_mois, nic.as_deref())?;
    }
    let etiq = le_mois.etiquette();
    let sortie = ctx.racine.join("sorties").join(texte);

    // LES CORRECTIONS DES MOIS DEJA DECLARES : la DSN de ce mois les porte, datees de leur mois d'origine.
    let regs = &lu.regs;
    // La DSN DECLARE, ELLE NE FABRIQUE AUCUN BULLETIN : le bulletin d'une paie corrigee part avec les documents de son
    // mois (`fabriquer_documents` : le rectificatif d'un bulletin parti, ou le bulletin d'une paie qui n'est jamais
    // partie). Une correction a zero (la paie d'apres est `sans_paie`) n'a pas de bulletin : le net deja verse se retient.
    for (r, ligne) in regs.iter().zip(regularisations::resume_regularisations(&regs)?) {
        let suite = if r.apres.engagement.sans_paie() {
            " La paie est retirée : le net déjà versé se retient, et le salarié doit en être prévenu par écrit."
        } else {
            ""
        };
        avertissements.push(info(format!("Régularisation portée par la DSN : {}.{}", ligne, suite)));
    }
    if paies.is_empty() && regs.is_empty() {
        avertissements.push(info(format!(
            "Aucun salarié en {} : la DSN part quand même, sans individu (envoi néant) -- une DSN est due chaque mois.",
            mois.libelle(Graphie::Accentuee)
        )));
    }
    // LA DECLARATION OETH D'AVRIL : elle ajoute au bordereau Urssaf une contribution (CTP 730 ou 740).
    if let Some(doeth) = declarations::oeth::declaration(ctx, mois).map_err(|e| e.reclasser(Classe::Illisible))? {
        avertissements.push(info(format!("{} Le bordereau Urssaf du mois en porte {}.", doeth.resume(), socle::texte::euros(doeth.a_verser()))));
    }
    // LA CONTRIBUTION SUPPLEMENTAIRE A L'APPRENTISSAGE DE MARS : le bloc 82 (079 ou 080) et le CTP 998 (`csa`).
    if let Some(c) = declarations::csa::declaration(ctx, mois, &departs).map_err(|e| e.reclasser(Classe::Illisible))? {
        avertissements.push(info(c.resume()));
    }
    // LES ARRETS DE TRAVAIL : la DSN mensuelle REPORTE ceux que `Arret::a_reporter` y met, signales ou non.
    let arrets = &lu.arrets;
    let nomenclature = crate::metier::imprevus::nomenclatures()?;
    let mut non_signales = 0;
    for a in arrets {
        let a = &a.saisi;
        let resume = crate::imprevus::resume_arret(ctx, &nomenclature, a)?;
        let reprise = a.reprise;
        let fin = match reprise {
            Some(r) => r,
            None => a.fin_previsionnelle,
        };
        avertissements.push(info(format!(
            "Arret de travail reporte dans la DSN : {}, {}, du {} au {}{}.",
            a.salarie,
            resume.motif_libelle,
            engagement::imprevus::dernier_jour_declare(a).map_err(|e| e.reclasser(Classe::Illisible))?.jj_mm_aaaa(),
            fin.jj_mm_aaaa(),
            if reprise.is_some() { " (reprise)" } else { " (fin previsionnelle)" }
        )));
        if !resume.signale {
            non_signales += 1;
        }
    }
    // LES SUSPENSIONS (bloc 65) : chacune se declare entiere dans la DSN de chaque mois qu'elle traverse.
    for s in crate::metier::imprevus::magasin(engagement::imprevus::du_mois(&ctx.racine, le_mois, Some(engagement::imprevus::SUSPENSION)))? {
        let engagement::imprevus::Imprevu::Suspension(s) = s else { continue };
        avertissements.push(info(format!("Suspension declaree dans la DSN : {}, motif {}, du {} au {}.", s.salarie, s.motif, s.debut.jj_mm_aaaa(), s.fin.jj_mm_aaaa())));
    }
    if non_signales > 0 {
        avertissements.push(avertissement(format!(
            "{} arret(s) reporte(s) dans la DSN mais JAMAIS SIGNALE(S) : le signalement d'arret est du dans les cinq jours, la DSN du mois ne le remplace pas (Arrets de travail › Signaler).",
            non_signales
        )));
    }

    // UNE DSN PAR ETABLISSEMENT : chacun avec ses salaries, ses corrections et ses arrets ; celle du principal garde
    // son nom de toujours, les autres portent leur NIC. Un etablissement sans salarie ce mois-ci depose quand meme la
    // sienne, sans individu : une DSN est due chaque mois, par SIRET.
    let mut a_ecrire: Vec<(PathBuf, String, Vec<envois::journal::Version>)> = Vec::new();
    let mut fichiers: Vec<PathBuf> = Vec::new();
    for nic in &nics {
        let n = nic.clone();
        let mut nom = match &n {
            Some(x) if Some(x.as_str()) != principal => format!("DSN_{}_{}.dsn", etiq, x),
            _ => format!("DSN_{}.dsn", etiq),
        };
        let (type_dsn, ordre, remplace) = forme_de_la_dsn(ctx, &mut m, le_mois, nic.as_deref())?;
        let mut remplacee_versait_pas = None;
        if remplace {
            remplacee_versait_pas = remplacee_versait_pas_de(ctx, &mut m, mois, nic.as_deref())?;
            nom = format!("{}_AR{}.dsn", &nom[..nom.len() - ".dsn".len()], ordre);
            avertissements.push(info(format!(
                "La DSN de {} est deja partie : celle-ci est un ANNULE-ET-REMPLACE integral (ordre {}). Elle remplace la precedente, ordres de telepaiement compris, et s'envoie au plus tard le {} a minuit.",
                mois.libelle(Graphie::Accentuee),
                ordre,
                m.limite(le_mois)?.jj_mm_aaaa()
            )));
        }
        let f_dsn = sortie.join(&nom);
        // LE FICHIER DEJA ECRIT QUI DIT LA MEME CHOSE se reprend (`refabriquer`) : meme declaration, au jour ou il a ete
        // constitue.
        if refabriquer_sur(ctx, Some(&lu), &f_dsn, le_mois, n.as_deref())?.is_none() {
            // Ce que la DSN de cet etablissement corrige des mois passes : elle le dit a son depot.
            let portees = declarations::produire_regularisations::versions_portees(ctx, &lu.regs, match n.as_deref() {
                Some(x) => x,
                None => fiche.nic.ecrite("nic").map_err(|e| e.reclasser(Classe::Illisible))?,
            })?;
            // La ligne du journal de son depot les emporte en sa forme (`envois::journal::Version`).
            let portees: Vec<envois::journal::Version> = portees
                .iter()
                .map(|p| -> R<envois::journal::Version> {
                    Ok(envois::journal::Version {
                        salarie: droit::contexte::IdSalarie::lire(&p.salarie)?,
                        numero: engagement::numeros::NumeroContrat::lire(&p.numero)?,
                        version: p.version,
                        mois: Some(p.mois),
                    })
                })
                .collect::<R<_>>()
                .map_err(|e| e.reclasser(Classe::Illisible))?;
            let forme = Forme { type_dsn, ordre, etablissement: n, constitution: ctx.aujourdhui, remplacee_versait_pas };
            a_ecrire.push((f_dsn.clone(), fabriquer_du_present(ctx, &lu, mois, &forme)?.texte, portees));
        }
        fichiers.push(f_dsn);
    }
    for (f, _, _) in &a_ecrire {
        refuser_si_partie(&ctx.racine, f)?;
    }
    if !a_ecrire.is_empty() {
        std::fs::create_dir_all(&sortie).map_err(|e| os(&sortie, e))?;
        for (f, texte, _) in &a_ecrire {
            declarations::dsn::ecrire(texte, f).map_err(|m| m.reclasser(Classe::Disque))?;
        }
        // La paie que chaque DSN declare : l'ecran dira si elle change avant l'envoi.
        let empreinte = m.empreinte_saisie_avec(le_mois, || Ok(Moteur::portees(&lu.regs)))?;
        for (f, _, portees) in a_ecrire {
            noter_piece_ecrite(&ctx.racine, &cle_de_piece(&f), empreinte.clone().map(EmpreinteNotee::Saisie), Some(portees))?;
        }
    }
    Ok(DsnFabriquee { fichiers, avertissements })
}

// ------------------------------------------------------------- les documents, a leur envoi

/// Les documents d'un mois, fabriques pour ceux a qui ils partent : leurs numeros sont reserves, pas encore ecrits.
pub(crate) struct DocumentsFabriques {
    _verrou: socle::verrou::Verrou,
    reserves: Reserves,
    /// `(cle du salarie, numero du contrat, numero de bulletin, premier et dernier N CE)` : ce qui s'ecrit dans les paies au
    /// depart.
    a_poser: Vec<(String, String, Option<String>, Option<(i64, i64)>)>,
    pub(crate) avertissements: Vec<String>,
}

impl DocumentsFabriques {
    /// LES DOCUMENTS SONT PARTIS : leurs numeros s'ecrivent dans les paies des engagements, et les compteurs passent
    /// au-dessus.
    pub(crate) fn engager(self, ctx: &Contexte, mois: Mois) -> R<()> {
        self.reserves.valider(ctx)?;
        for (cle, numero, bulletin, ce) in &self.a_poser {
            engagement::etats::poser_numeros_des_documents(ctx, mois, cle, numero, bulletin.as_deref(), ce.map(|c| c.0))?;
        }
        Ok(())
    }

    /// UN ENVOI COUPE EN ROUTE : seuls les salaries `servis` ont recu leurs documents, eux seuls ont leurs numeros. Les
    /// compteurs passent au-dessus du plus grand numero servi ; ceux des autres se rendent, ils les reprendront a leur
    /// depart.
    pub(crate) fn engager_servis(self, ctx: &Contexte, mois: Mois, servis: &[&str]) -> R<()> {
        let mut bornes = Reserves::default();
        let (mut bulletin_max, mut ce_max): (Option<(i64, i64)>, Option<i64>) = (None, None);
        let an = i64::from(mois.annee());
        for (cle, numero, bulletin, ce) in self.a_poser.iter().filter(|a| servis.contains(&a.0.as_str())) {
            // Le numero d'un bulletin, `N/AAAA` : son rang dans l'annee.
            if let Some(b) = bulletin.as_deref() {
                let rang = b.split('/').next().map(str::trim).map(str::parse::<i64>);
                let n = match rang {
                    Some(Ok(n)) => n,
                    _ => return Err(erreur(Classe::Illisible, format!("numéro de bulletin illisible : {}", socle::fichiers::repr(b)))),
                };
                bulletin_max = Some((an, bulletin_max.into_iter().map(|(_, m)| m).chain([n]).fold(n, i64::max)));
            }
            if let Some((_, dernier)) = ce {
                ce_max = Some(ce_max.into_iter().chain([*dernier]).fold(*dernier, i64::max));
            }
            engagement::etats::poser_numeros_des_documents(ctx, mois, cle, numero, bulletin.as_deref(), ce.map(|c| c.0))?;
        }
        if let Some((an, n)) = bulletin_max {
            bornes.enregistrer(an, Suite::Bulletin, n);
        }
        if let Some(n) = ce_max {
            bornes.enregistrer(an, Suite::CertificatCs, n);
        }
        bornes.valider(ctx)
    }
}

/// Le numero d'un bulletin dans un nom de fichier : `12/2026` -> `12-2026`.
/// L'ECART D'UNE PAIE REFAITE AVEC LE BULLETIN PARTI (`parti`, ce qu'il a paye), verse -- ou retenu -- avec la paie de
/// `mois_versement` : la difference de net, et le prelevement a la source sur la difference de net imposable
/// (`engagement::etats::prelevement_de_la_correction`, au taux du bulletin parti pour un trop-verse : BOFiP,
/// BOI-IR-PAS-30-10-50, § 1 et § 90, `atelier/reference/sources/bofip/BOI-IR-PAS-30-10-50-extrait-regularisation.md`).
fn ecart_avec_le_bulletin_parti(ctx: &Contexte, apres: &Paie, parti: &Paie, mois_versement: Mois) -> R<documents::bulletin::Ecart> {
    let taux_parti = parti.pas.as_ref().and_then(|p| p.taux.as_ref().map(droit::fiche::Montant::ecrit)).filter(|t| !t.is_empty());
    let d_net = apres.net_a_payer - parti.net_a_payer;
    let pas_c = engagement::etats::prelevement_de_la_correction(ctx, apres, parti.net_imposable, taux_parti.as_deref(), mois_versement).map_err(|e| e.reclasser(Classe::Illisible))?;
    let indu = pas_c.indu();
    let prelevement = match pas_c.prelevement {
        Some(montant) => {
            let taux = match (indu, taux_parti) {
                (false, _) => pas_c.versement.taux.clone(),
                (true, t) => t.map(droit::fiche::Montant::Texte),
            };
            let taux = taux.ok_or_else(|| erreur(Classe::Illisible, format!("La correction de {} prélève {} sans taux de prélèvement à la source.", mois_versement.libelle(Graphie::Accentuee), montant)))?;
            Some(documents::bulletin::PrelevementDeLEcart { indu, taux, montant })
        }
        None => None,
    };
    let a_verser = match &prelevement {
        Some(p) => d_net - p.montant,
        None => d_net,
    };
    Ok(documents::bulletin::Ecart {
        net_avant: parti.net_a_payer,
        net_apres: apres.net_a_payer,
        difference_net: d_net,
        difference_net_imposable: pas_c.difference,
        mois_versement,
        prelevement,
        a_verser,
    })
}

fn numero_en_nom(n: &str) -> String {
    n.replace('/', "-")
}

/// LES DOCUMENTS DU MOIS, FABRIQUES A LEUR ENVOI, pour les salaries `cles` : depuis les paies ecrites des engagements
/// valides (chaque contrat sous le numero que sa validation lui a ecrit), un `Bulletin_<cle>_<Mois>.pdf` (tous ses
/// bulletins du mois) et un `CertificatCS_<cle>_<Mois>.pdf` par salarie,
/// dans `sorties/<mois>/envoi/`. Un bulletin parti garde son numero ; un bulletin qui part pour la premiere fois prend le
/// suivant de l'annee ; une paie partie puis refaite part sous un nouveau numero, imprimee comme le rectificatif de celle
/// qui est partie, et le bulletin d'origine se garde a cote (`sorties/<mois>/Bulletin_<cle>_<Mois>_<numero>.pdf`). Les N CE
/// suivent la sequence continue des certificats. L'archive du mois (`Bulletins_<Mois>.pdf`, `CertificatsCS_<Mois>.pdf` et
/// son recapitulatif) porte tous les documents numerotes du mois.
/// CE QUE LES BULLETINS EXIGENT DES FICHES (`exige_par` de `champs-base.json`, la liste que « Pour demarrer » lit) :
/// l'entreprise, et le spectacle de chaque paie dont la nature declare un numero d'objet (S21.G00.40.050, la marque des
/// annexes VIII et X) ; tout ce qui manque, d'un coup, avant que rien ne s'ecrive.
fn exiges_des_bulletins<'a>(ent: &droit::fiche::ChampsEntreprise, paies: impl Iterator<Item = &'a Paie>) -> R<()> {
    use declarations::champs::Saisie;
    let mut saisies: Vec<(&str, String, Saisie)> = vec![("entreprise", "la fiche entreprise".to_string(), Saisie::de_la_fiche("entreprise", ent)?)];
    let mut spectacles: Vec<&str> = Vec::new();
    for p in paies {
        let Some(sp) = p.spectacle.as_ref() else { continue };
        if !p.engagement.nat().map_err(|e| e.reclasser(Classe::Illisible))?.ecrit("S21.G00.40.050") || spectacles.contains(&sp.cle.as_str()) {
            continue;
        }
        spectacles.push(&sp.cle);
        let qui = match sp.titre.portee() {
            Some(titre) => format!("Spectacles, la fiche de {}", titre),
            None => format!("Spectacles, la fiche {}", sp.cle),
        };
        saisies.push(("spectacle", qui, Saisie::de_la_fiche("spectacle", &sp.champs)?));
    }
    let fiches: Vec<(&str, String, &Saisie)> = saisies.iter().map(|(n, q, s)| (*n, q.clone(), s)).collect();
    match declarations::champs::refus_des_manques("bulletin", &fiches, &[])? {
        Some(m) => Err(erreur(Classe::Refuse, m)),
        None => Ok(()),
    }
}

pub(crate) fn fabriquer_documents(ctx: &Contexte, mois: Mois, cles: &[String], qui: &str) -> R<DocumentsFabriques> {
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;
    let validees = engagement::etats::validees_du_mois(ctx, mois, &departs)?;
    let ent = ctx.pour_un_modele(&ctx.entreprise_du_mois(mois));
    if let Some((_, paies)) = &validees {
        exiges_des_bulletins(&ent, paies.iter().filter(|p| cles.contains(&cle_de(p)) && !p.engagement.sans_paie()))?;
    }
    let verrou = documents::ecrire_documents::prendre_le_verrou(ctx, qui)?;
    let mut faits = DocumentsFabriques { _verrou: verrou, reserves: Reserves::default(), a_poser: Vec::new(), avertissements: Vec::new() };
    let Some((_data, paies)) = validees else { return Ok(faits) };
    faits.avertissements.extend(sans_taux_recu(ctx, paies.iter().filter(|p| cles.contains(&cle_de(p))))?);
    let ecrits = engagement::etats::numeros_des_documents(ctx, mois, &departs)?;
    let an = i64::from(mois.annee());
    let etiq = mois.etiquette();
    let sortie = ctx.racine.join("sorties").join(mois.to_string());
    let dossier = sortie.join("envoi");
    std::fs::create_dir_all(&dossier).map_err(|e| os(&dossier, e))?;
    let part = |p: &Paie| cles.contains(&cle_de(p));
    let mut a_poser: HashMap<usize, (Option<String>, Option<(i64, i64)>)> = HashMap::new();

    // LES BULLETINS : un numero par paie qui a des cachets, ecrit dans la paie a son depart.
    // Le numero de chaque bulletin, a cote de sa paie : celui que la paie ecrite porte, sinon celui que ce depart reserve.
    // Les numeros neufs se donnent dans l'ordre ou les documents partent (`cles`), salarie par salarie : ceux d'un salarie
    // se suivent, et un envoi coupe en route laisse aux servis les premiers, sans trou.
    let mut numeros: Vec<Option<String>> = vec![None; paies.len()];
    let mut numerotees: Vec<bool> = vec![false; paies.len()];
    for i in 0..paies.len() {
        if paies[i].engagement.sans_paie() {
            continue;
        }
        if let Some(b) = ecrits.get(i).and_then(|e| e.bulletin.clone()) {
            numeros[i] = Some(b);
            numerotees[i] = true;
        }
    }
    for cle in cles {
        for i in (0..paies.len()).filter(|i| cle_de(&paies[*i]) == *cle) {
            if paies[i].engagement.sans_paie() || numerotees[i] {
                continue;
            }
            let n = faits.reserves.prochain(ctx, Suite::Bulletin, an)?;
            faits.reserves.enregistrer(an, Suite::Bulletin, n);
            let numero = format!("{}/{}", n, an);
            numeros[i] = Some(numero.clone());
            a_poser.entry(i).or_default().0 = Some(numero);
            numerotees[i] = true;
        }
    }
    // LE RECTIFICATIF D'UNE PAIE PARTIE PUIS REFAITE. Tant que la DSN du mois se refait, il remplace le bulletin parti ;
    // quand elle ne se refait plus (`se_corrige_ailleurs`), il porte l'ecart avec ce bulletin, verse avec la paie du mois
    // dont la DSN declare la correction (`ecart_avec_le_bulletin_parti`).
    let clos = regularisations::se_corrige_ailleurs(ctx, &departs, mois)?;
    let valides: Vec<engagement::etats::Valide> = engagement::etats::du_mois(ctx, mois, &departs)?.iter().filter_map(|e| e.valide().cloned()).collect();
    let mut rectificatifs: HashMap<usize, documents::bulletin::Rectificatif> = HashMap::new();
    for i in 0..paies.len() {
        let Some(r) = ecrits.get(i).and_then(|e| e.rectifie.clone()) else { continue };
        let parti = match (clos, valides.get(i)) {
            (true, Some(v)) => engagement::etats::paie_partie(ctx, v.objet(), &departs, mois)?.map(|p| (p, v)),
            _ => None,
        };
        let ecart = match parti {
            Some((parti, v)) => {
                let porteur = crate::regularisation::dsn_qui_porte_la_correction(ctx, v, &departs, mois)?;
                Some(ecart_avec_le_bulletin_parti(ctx, &paies[i], &parti, porteur)?)
            }
            None => None,
        };
        rectificatifs.insert(i, documents::bulletin::Rectificatif { rectifie: r, mois_origine: mois, ecart });
    }

    // LES CERTIFICATS CONGES SPECTACLES : un par contrat qui finit ce mois-ci, numerotes dans la sequence continue, dans
    // l'ordre ou les documents partent, salarie par salarie, comme les bulletins.
    let (mut pour_cs, avert_cs) = crate::dossier_salarie::certificats_dus(ctx, &paies)?;
    faits.avertissements.extend(avert_cs);
    // Dans l'ordre des salaries demandes ; un salarie qu'ils ne nomment pas vient apres eux.
    pour_cs.sort_by_key(|(cle, _, _)| (cles.iter().position(|c| c == cle).is_none(), cles.iter().position(|c| c == cle)));
    let mut prochain_ce: Option<i64> = None;
    let mut certificats: Vec<(usize, &PaieCs, i64)> = Vec::new();
    for (cle, debut, pc) in &pour_cs {
        let Some(pc) = pc else { continue };
        let Some(i) = (0..paies.len()).find(|i| cle_de(&paies[*i]) == *cle && paies[*i].engagement.debut == *debut) else { continue };
    let n = match ecrits.get(i).and_then(|e| e.premier_ce) {
            Some(n) => n,
            None if part(&paies[i]) => {
                let n = match prochain_ce {
                    Some(n) => n,
                    None => documents::ecrire_documents::prochain_continu(ctx, Suite::CertificatCs, 1)?,
                };
                let combien = conges_spectacles::decouper(&pc.engagement)?.len() as i64;
                prochain_ce = Some(n + combien);
                faits.reserves.enregistrer(an, Suite::CertificatCs, n + combien - 1);
                a_poser.entry(i).or_default().1 = Some((n, n + combien - 1));
                if ctx.salarie(cle).map_err(|e| e.reclasser(Classe::Illisible))?.nir.is_none() {
                    faits.avertissements.push(format!(
                        "{} : certificat Conges Spectacles emis SANS numero de securite sociale -- Audiens ne pourra pas le rattacher.",
                        documents::tableaux_export::qui_souple(&paies[i].salarie)
                    ));
                }
                n
            }
            None => continue,
        };
        certificats.push((i, pc, n));
    }

    // LE BULLETIN D'ORIGINE RESTE : une paie refaite part sous un nouveau numero, et ce qui est parti se garde.
    for (i, p) in paies.iter().enumerate().filter(|(i, p)| part(p) && numerotees[*i]) {
        if let (Some(r), true) = (ecrits[i].rectifie.as_deref(), a_poser.get(&i).is_some_and(|x| x.0.is_some())) {
            let cle = cle_de(p);
            let piece = dossier.join(envois::envoi::nom_fichier("Bulletin", &cle, &etiq));
            let garde = sortie.join(format!("Bulletin_{}_{}_{}.pdf", cle, etiq, numero_en_nom(r)));
            if piece.exists() && !garde.exists() {
                std::fs::copy(&piece, &garde).map_err(|e| os(&garde, e))?;
            }
        }
    }
    let impression = Impression { paies: &paies, numeros: &numeros, rectificatifs: &rectificatifs, certificats: &certificats };
    imprimer(ctx, mois, &ent, &impression, cles, false)?;
    let mut poses: Vec<(usize, (Option<String>, Option<(i64, i64)>))> = a_poser.into_iter().collect();
    poses.sort_by_key(|(i, _)| *i);
    faits.a_poser = poses
        .into_iter()
        .map(|(i, (b, ce))| {
            let numero = paies[i].engagement.numero.clone().ok_or_else(|| erreur(Classe::Illisible, format!("{} : une paie qui part sans numéro de contrat.", cle_de(&paies[i]))))?;
            Ok((cle_de(&paies[i]), numero.to_string(), b, ce))
        })
        .collect::<R<_>>()?;
    Ok(faits)
}

/// Ce qu'un envoi de documents imprime : les paies du mois, au meme rang le numero de bulletin de chacune (`None` : elle
/// n'en a pas), le rectificatif de celles qui en sont un, et les certificats Conges Spectacles numerotes (rang de la paie,
/// certificat, premier N CE).
struct Impression<'a> {
    paies: &'a [Paie],
    numeros: &'a [Option<String>],
    rectificatifs: &'a HashMap<usize, documents::bulletin::Rectificatif>,
    certificats: &'a [(usize, &'a PaieCs, i64)],
}

/// LES DOCUMENTS D'UN ENVOI, ECRITS : un `Bulletin_<cle>_<Mois>.pdf` (tous ses bulletins du mois) et un
/// `CertificatCS_<cle>_<Mois>.pdf` par salarie de `cles`, dans `sorties/<mois>/envoi/`, et l'archive du mois
/// (`Bulletins_<Mois>.pdf`, `CertificatsCS_<Mois>.pdf` et son recapitulatif), qui porte tous les documents numerotes.
/// `absents` : seuls les fichiers qui manquent s'ecrivent (une piece partie refaite pour se lire).
fn imprimer(ctx: &Contexte, le_mois: Mois, ent: &droit::contexte::EntrepriseDuDocument, imp: &Impression, cles: &[String], absents: bool) -> R<()> {
    let (etiq, mois) = (le_mois.etiquette(), &le_mois.to_string());
    let sortie = ctx.racine.join("sorties").join(mois);
    let dossier = sortie.join("envoi");
    std::fs::create_dir_all(&dossier).map_err(|e| os(&dossier, e))?;
    let a_ecrire = |f: &Path| !(absents && f.exists());
    let paies = imp.paies;
    let numerotee = |i: usize| imp.numeros.get(i).is_some_and(Option::is_some);
    let departs = envois::journal::lu(&ctx.racine).map(|j| j.departs)?;

    // UN JEU DE DOCUMENTS PAR SALARIE QUI LES RECOIT, et l'archive du mois : tous ses documents numerotes.
    let mut destinataires: Vec<String> = Vec::new();
    for p in paies.iter().filter(|p| cles.contains(&cle_de(p))) {
        if !destinataires.contains(&cle_de(p)) {
            destinataires.push(cle_de(p));
        }
    }
    let siennes = |cle: &str| -> Vec<usize> { (0..paies.len()).filter(|i| cle_de(&paies[*i]) == cle && numerotee(*i)).collect() };
    let piece_de = |cle: &str| dossier.join(envois::envoi::nom_fichier("Bulletin", cle, &etiq));
    let archivees: Vec<usize> = (0..paies.len()).filter(|i| numerotee(*i)).collect();
    let archive = sortie.join(format!("Bulletins_{}.pdf", etiq));
    let archive_a_ecrire = !archivees.is_empty() && a_ecrire(&archive);

    // Ce que les bulletins cumulent se lit une fois, pour tous les salaries qu'ils impriment.
    let mut imprimees: Vec<usize> = destinataires.iter().filter(|c| a_ecrire(&piece_de(c))).flat_map(|c| siennes(c)).collect();
    if archive_a_ecrire {
        imprimees.extend(&archivees);
    }
    imprimees.sort_unstable();
    imprimees.dedup();
    let leurs_paies: Vec<Paie> = imprimees.iter().map(|i| paies[*i].clone()).collect();
    let passe =documents::bulletin::passe_des_bulletins(ctx, &leurs_paies, le_mois, &departs).map_err(|e| e.reclasser(Classe::Illisible))?;
    let bulletins = |rangs: &[usize], toutes: Option<&[Paie]>| -> R<fabrique_pdf::Description> {
        let imprimees: Vec<Paie> = rangs.iter().map(|i| paies[*i].clone()).collect();
        let rectifs: HashMap<usize, documents::bulletin::Rectificatif> = rangs.iter().enumerate().filter_map(|(k, i)| imp.rectificatifs.get(i).map(|r| (k, r.clone()))).collect();
        let leurs_numeros: Vec<Option<String>> = rangs.iter().map(|i| imp.numeros[*i].clone()).collect();
        documents::bulletin::jeux_et_description(ctx, &imprimees, &leurs_numeros, ent, le_mois, toutes, false, &rectifs, &departs, &passe).map_err(|e| e.reclasser(Classe::Illisible))
    };

    for cle in &destinataires {
        let siennes = siennes(cle);
        let piece = piece_de(cle);
        if !siennes.is_empty() && a_ecrire(&piece) {
            documents::fabrique::ecrire_pdf(&bulletins(&siennes, Some(paies))?, &piece)?;
        }
        let ses_certificats: Vec<(&PaieCs, i64)> = imp.certificats.iter().filter(|(i, _, _)| cle_de(&paies[*i]) == *cle).map(|(_, pc, n)| (*pc, *n)).collect();
        let piece = dossier.join(envois::envoi::nom_fichier("CertificatCS", cle, &etiq));
        if !ses_certificats.is_empty() && a_ecrire(&piece) {
            let description = conges_spectacles::description_numerotee(&ses_certificats, ent)?;
            documents::fabrique::ecrire_pdf(&description, &piece)?;
        }
    }

    if archive_a_ecrire {
        documents::fabrique::ecrire_pdf(&bulletins(&archivees, None)?, &archive)?;
    }
    if !imp.certificats.is_empty() {
        let mut paires: Vec<(&PaieCs, i64)> = imp.certificats.iter().map(|(_, pc, n)| (*pc, *n)).collect();
        paires.sort_by_key(|(_, n)| *n);
        let archive = sortie.join(format!("CertificatsCS_{}.pdf", etiq));
        if a_ecrire(&archive) {
            let description = conges_spectacles::description_numerotee(&paires, ent)?;
            documents::fabrique::ecrire_pdf(&description, &archive)?;
        }
        let recap = sortie.join(format!("CertificatsCS_{}_recapitulatif.txt", etiq));
        if a_ecrire(&recap) {
            let texte = conges_spectacles::recapitulatif(&paires, ent)?;
            std::fs::write(&recap, texte).map_err(|e| os(&recap, e))?;
        }
    }
    Ok(())
}

// ------------------------------------------------------------- une piece partie, refaite pour se lire

/// UNE PIECE PARTIE, pour la lire : son fichier dans `sorties/<mois>/` ou son `envoi/` ; s'il manque, refait a
/// l'identique depuis ce que le journal dit d'elle -- la ligne qui l'a fait partir, les versions des engagements qu'elle
/// a emportees, le jour ou elle a ete fabriquee -- et remis a sa place. Regle 4 : refaite pour se lire, jamais pour
/// repartir ; ce qui se redepose se fabrique a son depart (`fabriquer_dsn`, `fabriquer_documents`).
///
/// Se refont : la DSN mensuelle (`dsn_partie`, et l'empreinte que le journal a notee a son depot le verifie), les
/// bulletins, rectificatifs compris, et les certificats Conges Spectacles d'un envoi de documents, avec l'archive du mois.
/// Ce qui vient d'un organisme (l'AER, un bilan, un compte rendu) ne se refait pas : il se lit dans `retours/`.
/// Refus : un nom qui n'est pas celui d'une piece partie de `mois`, une piece qui ne se refait pas, une piece refaite
/// qui ne dit pas ce qui est parti.
pub(crate) fn piece_partie(ctx: &Contexte, le_mois: Mois, nom: &str) -> R<PathBuf> {
    let refus = |m: String| erreur(Classe::Refuse, m);
    if nom.is_empty() || nom.contains(['/', '\\']) || nom.contains("..") {
        return Err(refus(format!("{} n'est pas une piece de {}.", nom, le_mois)));
    }
    let mois = le_mois;
    if let Some(f) = envois::etats::chemin_piece(&ctx.racine, le_mois, nom) {
        return Ok(f);
    }
    let mut m = Moteur::nouveau(ctx);
    let lignes = m.lignes()?;
    use envois::journal::{Acte, Ligne};
    let du_mois = |l: &Ligne| l.mois() == Some(le_mois) && l.accepte();
    let etiq = le_mois.etiquette();
    let sortie = ctx.racine.join("sorties").join(mois.to_string());

    // LA DSN MENSUELLE : la derniere ligne qui l'a deposee, le reel devant son test.
    let deposee = |reel: bool| lignes.iter().rposition(|l| l.acte() == Acte::Dsn && l.fichier() == Some(nom) && du_mois(l) && l.reel() == reel);
    if crate::etapes::est_une_dsn_mensuelle(nom) {
        let Some(rang) = deposee(true).or_else(|| deposee(false)) else {
            return Err(refus(format!("{} : le journal ne dit pas qu'elle est partie.", nom)));
        };
        refaire_la_dsn(ctx, &mut m, &lignes, rang, &sortie.join(nom))?;
        return Ok(sortie.join(nom));
    }

    // LES DOCUMENTS D'UN ENVOI AUX SALARIES, et l'archive du mois.
    let des_documents = nom.starts_with("Bulletin_")
        || nom.starts_with("CertificatCS_")
        || [format!("Bulletins_{}.pdf", etiq), format!("CertificatsCS_{}.pdf", etiq), format!("CertificatsCS_{}_recapitulatif.txt", etiq)].contains(&nom.to_string());
    if des_documents {
        let envois: Vec<usize> = (0..lignes.len()).filter(|r| lignes[*r].acte() == Acte::Documents && du_mois(&lignes[*r])).collect();
        // LE BULLETIN D'ORIGINE GARDE A COTE DE SON RECTIFICATIF (`Bulletin_<cle>_<Mois>_<N-AAAA>.pdf`, ou `N/AAAA` est le
        // numero rectifie) : la piece du salarie telle que l'a ecrite le dernier envoi ou son bulletin portait ce numero.
        let garde = nom.strip_prefix("Bulletin_").and_then(|r| r.strip_suffix(".pdf")).and_then(|r| r.rsplit_once('_')).and_then(|(tete, n)| {
            let cle = tete.strip_suffix(etiq.as_str())?.strip_suffix('_')?;
            n.split_once('-').filter(|(a, b)| a.chars().all(|c| c.is_ascii_digit()) && b.len() == 4).map(|(a, b)| (cle.to_string(), format!("{}/{}", a, b)))
        });
        if let Some((cle, numero)) = garde {
            let piece = sortie.join("envoi").join(envois::envoi::nom_fichier("Bulletin", &cle, &etiq));
            for rang in envois.iter().rev() {
                if !refaire_les_documents(ctx, mois, &lignes, *rang, false)?.iter().any(|(c, n)| *c == cle && n.as_deref() == Some(numero.as_str())) {
                    continue;
                }
                // Cet envoi ne refait que cette piece : ce qu'il ecrirait d'autre n'est pas le dernier etat parti.
                let fichiers = || -> Vec<PathBuf> {
                    [sortie.clone(), sortie.join("envoi")].iter().flat_map(|d| std::fs::read_dir(d).into_iter().flatten().flatten().map(|e| e.path())).filter(|p| p.is_file()).collect()
                };
                let avant = fichiers();
                let de_cote = piece.with_extension("pdf.presente");
                let presente = piece.exists();
                if presente {
                    std::fs::rename(&piece, &de_cote).map_err(|e| os(&piece, e))?;
                }
                let fait = refaire_les_documents(ctx, mois, &lignes, *rang, true).and_then(|_| std::fs::rename(&piece, sortie.join(nom)).map_err(|e| os(&piece, e)));
                for f in fichiers().into_iter().filter(|f| !avant.contains(f) && *f != sortie.join(nom)) {
                    let _ = std::fs::remove_file(f);
                }
                if presente {
                    std::fs::rename(&de_cote, &piece).map_err(|e| os(&de_cote, e))?;
                }
                fait?;
                return Ok(sortie.join(nom));
            }
            return Err(refus(format!("{} : aucun envoi de documents de {} n'a fait partir le bulletin n° {}.", nom, mois.libelle(Graphie::Accentuee), numero)));
        }
        // Du dernier envoi au premier : le fichier est celui que le dernier envoi qui le porte a ecrit.
        for rang in envois.into_iter().rev() {
            refaire_les_documents(ctx, mois, &lignes, rang, true)?;
        }
        return envois::etats::chemin_piece(&ctx.racine, le_mois, nom)
            .ok_or_else(|| refus(format!("{} : aucun envoi de documents de {} ne l'a fait partir.", nom, mois.libelle(Graphie::Accentuee))));
    }
    // LA DPAE : son enregistrement, son `.zip` et son recapitulatif, d'apres la derniere ligne qui l'a deposee.
    if nom.starts_with("DPAE_") {
        // L'enregistrement que designent son recapitulatif (`<base>_recapitulatif.txt`) et son `.zip` (`<base>.txt.zip`).
        let base = match (recapitulatif_de(nom), nom.strip_suffix(".zip")) {
            (Some(b), _) => format!("{}.txt", b),
            (None, Some(b)) => b.to_string(),
            (None, None) => nom.to_string(),
        };
        let deposee = |reel: bool| lignes.iter().rposition(|l| l.acte() == Acte::Dpae && l.fichier() == Some(base.as_str()) && du_mois(l) && l.reel() == reel);
        let Some(rang) = deposee(true).or_else(|| deposee(false)) else {
            return Err(refus(format!("{} : le journal ne dit pas qu'elle est partie.", nom)));
        };
        refaire_la_dpae(ctx, mois, &lignes[rang], &sortie.join(&base))?;
        return Ok(sortie.join(nom));
    }
    // LES SIGNALEMENTS (amorcage, arret, reprise, fin de contrat) : leur ligne porte le mois de l'evenement, et le fichier
    // est range dans ce mois.
    let signale = |reel: bool| lignes.iter().rposition(|l| l.acte() == Acte::Dsn && l.fichier() == Some(nom) && du_mois(l) && l.reel() == reel);
    if nom.ends_with(".dsn") {
        if let Some(rang) = signale(true).or_else(|| signale(false)) {
            refaire_le_signalement(ctx, mois, &lignes[rang], &sortie.join(nom))?;
            return Ok(sortie.join(nom));
        }
    }
    // LE CONTRAT ET L'AVENANT : la derniere ligne qui les a fait partir.
    if nom.starts_with("Contrat") || nom.starts_with("Avenant") {
        // L'EXEMPLAIRE CONTRESIGNE : le contrat parti, plus la mention que sa signature a notee (`contresigner`).
        let signe = lignes.iter().rev().filter(|l| l.mois() == Some(le_mois)).flat_map(|l| envois::etats::revenus(l)).find(|r| r.fichier_signe.as_deref().is_some_and(|f| f.split(", ").any(|f| f == nom)));
        if let Some(r) = signe {
            let Some(envoye) = envois::releve_signatures::souche_contresignee(&nom) else {
                return Err(refus(format!("{} : la signature notée ne dit pas quel contrat parti elle signe.", nom)));
            };
            let Some(mention) = r.mention.clone().filter(|m| !m.is_empty()) else {
                return Err(refus(format!("{} : la signature notée ne dit pas la mention de l'artiste ; il ne se refait pas.", nom)));
            };
            let Some(code) = r.code.as_deref() else {
                return Err(refus(format!("{} : la signature notée ne dit pas la demande à laquelle elle répond ; il ne se refait pas.", nom)));
            };
            let pdf = sortie.join("envoi").join(nom);
            contresigner(ctx, &lignes, code, &sortie.join("envoi").join(format!("{}.pdf", envoye)), &pdf, &mention)?;
            return Ok(pdf);
        }
        let porte = |l: &Ligne| l.signature().is_some_and(|s| s.detail.contrats.iter().any(|c| c.fichier == nom));
        if let Some(rang) = lignes.iter().rposition(|l| du_mois(l) && porte(l)) {
            let pdf = sortie.join("envoi").join(nom);
            // Parti par courriel, il a son empreinte : le refait la redonne, ou il ne s'ecrit pas ; imprime, ses clauses le gardent.
            let empreinte = lignes[rang].signature().and_then(|s| s.detail.contrats.iter().find(|c| c.fichier == nom)).and_then(|c| c.empreinte.as_deref());
            match empreinte {
                Some(e) => std::fs::write(&pdf, contrat_parti(ctx, &lignes, rang, &pdf, e)?).map_err(|e| os(&pdf, e))?,
                None => refaire_le_contrat(ctx, &lignes, rang, &pdf, &pdf)?,
            }
            return Ok(pdf);
        }
    }
    if nom.starts_with("Amorcage") && recapitulatif_de(nom).is_some() {
        return Err(refus(format!(
            "{} est la note qui accompagnait les signalements d'amorçage au moment de les déposer : elle n'est pas partie et ne se refait pas ; les signalements partis s'ouvrent, eux.",
            nom
        )));
    }
    // L'AER vient de France Travail : elle ne se refait pas, elle se relit dans `retours/` (l'archive du retour 46) et se
    // range de nouveau comme a sa releve (`routes::documents::ranger_aer_detail`), signee au jour de cette releve.
    if nom.starts_with("AER_") {
        let mut archives: Vec<PathBuf> = std::fs::read_dir(ctx.racine.join(envois::retours::RETOURS))
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && socle::fichiers::nom_du(p).split('_').any(|x| x == NATURE_AER))
            .collect();
        archives.sort();
        for a in archives {
            let n = socle::fichiers::nom_du(&a);
            // Le nom de l'archive commence par l'instant de sa releve, `AAAAMMJJ_HHMMSS`.
            let quand = match (n.get(..4), n.get(4..6), n.get(6..8), n.get(9..11), n.get(11..13), n.get(13..15)) {
                (Some(an), Some(mo), Some(j), Some(h), Some(mi), Some(s)) => format!("{}-{}-{}T{}:{}:{}", an, mo, j, h, mi, s),
                _ => continue,
            };
            let quand = InstantEcrit::lire(&quand).ok_or_else(|| erreur(Classe::Illisible, format!("instant illisible : {}", socle::fichiers::repr(&quand))))?;
            let (jour, instant) = instant_de(quand.into())?;
            let ctx_j = contexte_du_jour(ctx, jour)?;
            socle::temps::pendant(instant, || crate::documents::ranger_aer_detail(&ctx_j, Some(mois), Some(std::slice::from_ref(&a)), false))?;
            if let Some(f) = envois::etats::chemin_piece(&ctx.racine, le_mois, nom) {
                return Ok(f);
            }
        }
        return Err(refus(format!(
            "{} vient de France Travail : elle ne se refait pas, elle se relit dans retours/, où son retour (nature 46) manque ; le fichier a été déplacé ou effacé.",
            nom
        )));
    }
    Err(refus(format!("{} n'est plus dans sorties/{} : le journal dit qu'il est parti, le fichier a été déplacé ou effacé.", nom, mois)))
}

/// Le contexte de la compagnie au jour `jour`, ce jour-la etant aujourd'hui : celui de la route qui a fabrique une piece.
fn contexte_du_jour(ctx: &Contexte, jour: socle::date::Jour) -> R<Contexte> {
    let mut c = Contexte::nouveau(&ctx.racine, Some(jour)).map_err(|e| e.reclasser(Classe::Illisible))?;
    c.aujourdhui = jour;
    Ok(c)
}

/// Le moment d'une heure du journal (a Paris), et son jour. Un jour sans heure, ou une heure que le changement d'heure
/// saute, se refuse.
fn instant_de(quand: HeureDuJournal) -> R<(socle::date::Jour, Moment)> {
    let illisible = || erreur(Classe::Illisible, format!("instant illisible : {}", socle::fichiers::repr(&quand.iso())));
    let HeureDuJournal::Instant(i) = quand else { return Err(illisible()) };
    let s = i.naif().seconde_du_jour();
    Ok((i.jour(), Moment::a_paris(i.jour(), s / 3_600, s / 60 % 60, s % 60).ok_or_else(illisible)?))
}

/// LA DSN MENSUELLE QUE DIT LA LIGNE `lignes[rang]`, refaite (`dsn_partie`) au jour ou son fichier a ete ecrit (le journal
/// des ecritures), et ecrite a `cible` si ses octets ont l'empreinte que le journal a notee a son depot.
fn refaire_la_dsn(ctx: &Contexte, m: &mut Moteur, lignes: &[envois::journal::Ligne], rang: usize, cible: &Path) -> R<()> {
    let l = &lignes[rang];
    let le_mois = l.mois().ok_or_else(|| erreur(Classe::Illisible, "journal : une DSN partie sans son mois".to_string()))?;
    let plusieurs = !ctx.entreprise_du_mois(le_mois).secondaires().is_empty();
    let nic = match (plusieurs, l.etablissement()) {
        (true, Some(n)) => Some(n.to_string()),
        (true, None) => Some(ctx.entreprise.nic.ecrite("nic").map_err(|e| e.reclasser(Classe::Illisible))?.to_string()),
        (false, _) => None,
    };
    // Un annule-et-remplace : la DSN qu'il remplacait versait-elle a la DGFiP ?
    let mut avant: Vec<(i64, usize)> = Vec::new();
    for x in m.depots_reels(le_mois, nic.as_deref())? {
        if let Some(r) = lignes.iter().position(|y| *y == x).filter(|r| *r < rang) {
            avant.push((m.ordre_de_la_ligne(le_mois, &x)?, r));
        }
    }
    avant.sort();
    let type_dsn = l.dsn().and_then(|e| e.detail.type_declaration.clone());
    let versait = match avant.last() {
        Some((_, r)) if matches!(type_dsn.as_deref(), Some("03" | "05")) => Some(verse_a_la_dgfip(&dsn_partie(ctx, m, lignes, *r, None)?)),
        _ => None,
    };
    let f = dsn_partie(ctx, m, lignes, rang, versait)?;
    let octets = declarations::dsn::en_latin1(&f.texte).map_err(|e| e.reclasser(Classe::Illisible))?;
    if l.empreinte() != Some(envois::journal::empreinte(&octets).as_str()) {
        return Err(erreur(
            Classe::Refuse,
            format!("{} ne se refait pas à l'identique de ce qui est parti : le fichier a été déplacé ou effacé.", socle::fichiers::nom_du(cible)),
        ));
    }
    if let Some(parent) = cible.parent() {
        std::fs::create_dir_all(parent).map_err(|e| os(parent, e))?;
    }
    std::fs::write(cible, &octets).map_err(|e| os(cible, e))
}

/// QUAND LA PIECE `nom` DE `mois` A ETE ECRITE : le journal des ecritures, sinon l'instant de la ligne `l` qui l'a
/// fait partir ; `(jour, moment, instant ecrit)`.
fn ecrite_a(ctx: &Contexte, mois: Mois, nom: &str, l: &envois::journal::Ligne) -> R<(socle::date::Jour, Moment, InstantEcrit)> {
    let quand = match journal_des_ecritures(&ctx.racine)?.remove(&format!("{}/{}", mois, nom)) {
        Some(n) => n.quand.into(),
        None => l.quand(),
    };
    let (jour, instant) = instant_de(quand)?;
    Ok((jour, instant, InstantEcrit::de(instant)))
}

/// Les paies que la ligne `l` a emportees (`versions`), chacune dans la version sur laquelle elle est partie, vues en
/// `mois`.
fn paies_emportees(ctx: &Contexte, mois: Mois, l: &envois::journal::Ligne) -> R<Vec<Paie>> {
    let ctx_m = ctx.du_mois(mois).map_err(|e| e.reclasser(Classe::Illisible))?;
    let mut out = Vec::new();
    for v in l.versions().into_iter().flatten() {
        let (cle, numero, version) = (v.salarie.to_string(), v.numero.to_string(), v.version);
        let o = engagement_de(ctx, mois, &cle, &numero)?;
        out.push(paie_de_la_version(&ctx_m, &o, &cle, &numero, mois, version)?);
    }
    Ok(out)
}

/// Les fichiers `ecrits` se gardent s'ils ont l'empreinte que la ligne `l` a notee au depot du premier ; sinon ils
/// s'effacent et le refus le dit : une piece refaite qui ne dit pas ce qui est parti ne se montre pas.
fn garder_si_identique(l: &envois::journal::Ligne, ecrits: &[PathBuf]) -> R<()> {
    let premier = &ecrits[0];
    let ecrit = std::fs::read(premier).map_err(|e| os(premier, e))?;
    if l.empreinte() == Some(envois::journal::empreinte(&ecrit).as_str()) {
        return Ok(());
    }
    for f in ecrits {
        let _ = std::fs::remove_file(f);
    }
    Err(erreur(
        Classe::Refuse,
        format!("{} ne se refait pas à l'identique de ce qui est parti : le fichier a été déplacé ou effacé.", socle::fichiers::nom_du(premier)),
    ))
}

/// LA DPAE QUE DIT LA LIGNE `l` (`cible` : son enregistrement), refaite par la fonction qui l'a ecrite
/// (`routes::depots::ecrire_la_dpae`) : les embauches qu'elle a emportees, dans leur version, horodatee de l'instant ou
/// elle a ete ecrite, au jour de cet instant ; l'heure d'embauche que la route prend quand on ne lui en donne pas.
/// Avec son `.zip` et son recapitulatif ; verifiee par l'empreinte que le journal a notee a son depot.
fn refaire_la_dpae(ctx: &Contexte, mois: Mois, l: &envois::journal::Ligne, cible: &Path) -> R<()> {
    let nom = socle::fichiers::nom_du(cible);
    let (jour, instant, quand) = ecrite_a(ctx, mois, &nom, l)?;
    let ctx_j = contexte_du_jour(ctx, jour)?;
    let paies = paies_emportees(&ctx_j, mois, l)?;
    let retenues: Vec<&Paie> = paies.iter().collect();
    let base = cible.file_stem().ok_or_else(|| erreur(Classe::Illisible, format!("journal : {} n'est pas un nom de fichier", nom)))?.to_string_lossy().into_owned();
    let recap = cible.with_file_name(format!("{}_recapitulatif.txt", base));
    let zip = cible.with_file_name(format!("{}.zip", nom));
    socle::temps::pendant(instant, || crate::depots::ecrire_la_dpae(&ctx_j, &retenues, cible, &recap, declarations::dpae::HEURE_EMBAUCHE, quand))?;
    garder_si_identique(l, &[cible.to_path_buf(), zip, recap])
}

/// LE SIGNALEMENT QUE DIT LA LIGNE `l` (amorcage 08, arret 04, reprise 05, fin de contrat 07), refait a `cible` par ce qui
/// l'a ecrit, sous le numero d'ordre que sa ligne porte, constitue le jour ou il a ete ecrit, a l'instant de son ecriture ;
/// verifie par l'empreinte que le journal a notee a son depot. Un annule-et-remplace ne se refait pas : sa ligne ne dit pas
/// ce qu'il remplacait.
fn refaire_le_signalement(ctx: &Contexte, mois: Mois, l: &envois::journal::Ligne, cible: &Path) -> R<()> {
    let nom = socle::fichiers::nom_du(cible);
    let manque = |quoi: &str| erreur(Classe::Illisible, format!("journal : {} sans {}", nom, quoi));
    let detail = l.dsn().map(|e| e.detail.clone()).ok_or_else(|| manque("son signalement"))?;
    let nature = detail.nature_declaration.clone().ok_or_else(|| manque("sa nature"))?;
    let ordre: i64 = detail.ordre.as_deref().ok_or_else(|| manque("son numero d'ordre"))?.trim().parse().map_err(|_| manque("numero d'ordre lisible"))?;
    if matches!(detail.type_declaration.as_deref(), Some(t) if !t.is_empty() && t != "01") {
        return Err(erreur(Classe::Refuse, format!("{} annulait et remplaçait un signalement : il ne se refait pas ; le fichier a été déplacé ou effacé.", nom)));
    }
    let (jour, instant, _) = ecrite_a(ctx, mois, &nom, l)?;
    let ctx_j = contexte_du_jour(ctx, jour)?;
    let premier = || l.versions().and_then(|v| v.first()).map(|v| v.salarie.to_string()).ok_or_else(|| manque("le salarie qu'il signale"));
    let refaire = || -> R<()> {
        match nature.as_str() {
            declarations::amorcage::NATURE => {
                let motif = nom.strip_prefix("Amorcage").and_then(|r| r.get(..2)).ok_or_else(|| manque("le motif de son amorcage"))?.to_string();
                let paies = paies_emportees(&ctx_j, mois, l)?;
                let paie = paies.first().ok_or_else(|| erreur(Classe::Illisible, format!("journal : {} n'emporte aucun engagement", nom)))?;
                let options = declarations::amorcage::Options {
                    motif,
                    type_declaration: declarations::amorcage::NORMALE.to_string(),
                    ordre,
                    remplace: None,
                    envoi_test: false,
                    date_constitution: Some(jour),
                };
                let texte = declarations::amorcage::generer(&ctx_j, paie, &options, &envois::journal::lu(&ctx.racine).map(|j| j.departs)?).map_err(|e| e.en_classe(Classe::Illisible))?;
                declarations::dsn::ecrire(&texte, cible).map_err(|e| e.reclasser(Classe::Illisible))
            }
            declarations::fctu::NATURE => {
                let charge = crate::imprevus::ChargeFinContratSignaler::refait(mois, premier()?, ordre, jour);
                crate::imprevus::signaler_la_fin(&ctx_j, &charge, true).map(|_| ())
            }
            n => {
                // L'arret tel qu'il etait a l'ecriture : celui de ses etats d'alors que l'empreinte reconnait.
                let premier = premier()?;
                let evenement = detail.evenement.clone().ok_or_else(|| manque("l'evenement qu'il signale"))?;
                let dernier = crate::imprevus::dernier_jour_saisi(&ctx_j, &premier, &evenement)?;
                let nature = crate::imprevus::NatureSignalement::du_code(n)?;
                let charge = crate::imprevus::ChargeArretSignaler::refait(premier.clone(), dernier, nature, ordre, jour);
                let mut rendu = Ok(());
                for a in crate::imprevus::etats_d_alors(&ctx_j, &premier, &dernier, compagnie::maintenant())? {
                    crate::imprevus::signaler_l_arret(&ctx_j, &charge, Some(&a))?;
                    rendu = garder_si_identique(l, &[cible.to_path_buf()]);
                    if rendu.is_ok() {
                        break;
                    }
                }
                rendu
            }
        }
    };
    socle::temps::pendant(instant, refaire)?;
    garder_si_identique(l, &[cible.to_path_buf()])
}

/// L'EXEMPLAIRE CONTRESIGNE `signe` DU CONTRAT (OU DE L'AVENANT) `envoye` QU'A FAIT PARTIR LA DEMANDE DE SIGNATURE `code` :
/// le PDF parti, octet pour octet (l'empreinte que la demande a notee), plus la mention de l'artiste dans sa case, par une
/// mise a jour incrementale (`fabrique_pdf::marquage::contresigner`). On ne contresigne que ce qui est parti : le PDF se
/// lit ou il est ; s'il manque ou a change, il se refait (`refaire_le_contrat`) et doit redonner l'empreinte notee, sinon
/// refus. Le fichier `envoye` n'est jamais reecrit.
pub(crate) fn contresigner(ctx: &Contexte, lignes: &[envois::journal::Ligne], code: &str, envoye: &Path, signe: &Path, mention: &str) -> R<()> {
    let nom = socle::fichiers::nom_du(envoye);
    let refus = |m: String| erreur(Classe::Refuse, m);
    let entree = |l: &envois::journal::Ligne| l.signature().and_then(|s| s.detail.contrats.iter().find(|c| c.fichier == nom && c.code.as_deref() == Some(code)).cloned());
    let Some(rang) = lignes.iter().rposition(|l| entree(l).is_some()) else {
        return Err(refus(format!("{} : aucune demande de signature ne porte le code {}.", nom, code)));
    };
    let Some(empreinte) = entree(&lignes[rang]).and_then(|c| c.empreinte) else {
        return Err(refus(format!("{} est parti sans son empreinte : rien ne dit que c'est lui qu'on contresigne. Relancer la demande de signature.", nom)));
    };
    let octets = contrat_parti(ctx, lignes, rang, envoye, &empreinte)?;
    let contresigne = fabrique_pdf::marquage::contresigner(&octets, mention).map_err(|r| erreur(Classe::FabriqueRefus, format!("{} : {}", nom, r.0)))?;
    if let Some(parent) = signe.parent() {
        std::fs::create_dir_all(parent).map_err(|e| os(parent, e))?;
    }
    std::fs::write(signe, contresigne).map_err(|e| os(signe, e))
}

/// LES OCTETS DU CONTRAT (OU DE L'AVENANT) `envoye` TELS QU'ILS SONT PARTIS, d'empreinte `empreinte` (notee par la
/// demande de signature `lignes[rang]`) : le fichier, s'il a cette empreinte ; sinon refait a cote, par la ligne qui
/// l'a ecrit (celle-ci, ou une plus ancienne qui a fait partir les memes octets : la relance d'un avenant reprend le
/// PDF parti). Refus quand rien ne redonne l'empreinte. Le fichier `envoye` n'est pas reecrit.
fn contrat_parti(ctx: &Contexte, lignes: &[envois::journal::Ligne], rang: usize, envoye: &Path, empreinte: &str) -> R<Vec<u8>> {
    if let Ok(o) = std::fs::read(envoye) {
        if envois::journal::empreinte(&o) == empreinte {
            return Ok(o);
        }
    }
    let nom = socle::fichiers::nom_du(envoye);
    let refait = envoye.with_extension("pdf.refait");
    let a_empreinte = |l: &envois::journal::Ligne| l.signature().is_some_and(|s| s.detail.contrats.iter().any(|c| c.fichier == nom && c.empreinte.as_deref() == Some(empreinte)));
    for r in (0..=rang).rev().filter(|r| a_empreinte(&lignes[*r])) {
        let fait = refaire_le_contrat(ctx, lignes, r, envoye, &refait).and_then(|_| std::fs::read(&refait).map_err(|e| os(&refait, e)));
        let _ = std::fs::remove_file(&refait);
        let o = fait?;
        if envois::journal::empreinte(&o) == empreinte {
            return Ok(o);
        }
    }
    Err(erreur(Classe::Refuse, format!("{} ne se refait pas à l'identique de ce qui est parti : le fichier parti a été déplacé ou effacé.", nom)))
}

/// LE CONTRAT OU L'AVENANT `pdf` QU'A FAIT PARTIR LA LIGNE `lignes[rang]` (une demande de signature ou une impression),
/// ecrit a `vers`. Il est
/// refait par ce qui l'a ecrit (`documents::contrat::ecrire_contrat`, `ecrire_avenant`) :
/// chaque contrat dans la version de l'engagement que la ligne a emportee, avec le modele fige a son depart
/// (`modeles_contrat::du_contrat_parti`), au jour et a l'heure de son depart ; un contrat
/// qui en remplacait un autre le dit en tete, d'apres le contrat parti avant lui ; un avenant cite ce que sa ligne a
/// note (articles modifies, effet), date du contrat qu'il modifie. Verifie par les clauses que la ligne a notees de chaque
/// contrat : la piece refaite qui ne les redit pas s'efface et se refuse.
pub(crate) fn refaire_le_contrat(ctx: &Contexte, lignes: &[envois::journal::Ligne], rang: usize, pdf: &Path, vers: &Path) -> R<()> {
    let l = &lignes[rang];
    let nom = socle::fichiers::nom_du(pdf);
    let (jour, instant) = instant_de(l.quand())?;
    let ctx_j = contexte_du_jour(ctx, jour)?;
    let contrats: Vec<&envois::journal::ContratParti> = l.signature().into_iter().flat_map(|s| s.detail.contrats.iter().filter(|c| c.fichier == nom)).collect();
    let mut paies: Vec<Paie> = Vec::new();
    for c in &contrats {
        let numero = c.numero_dit().ok_or_else(|| erreur(Classe::Illisible, format!("journal : un contrat de {} est parti sans numero", nom)))?.to_string();
        let (cle, mois_c) = (c.salarie.to_string(), c.mois);
        let version = l
            .versions()
            .into_iter()
            .flatten()
            .find(|v| v.salarie == c.salarie && c.numero.as_ref() == Some(&v.numero))
            .map(|v| v.version)
            .ok_or_else(|| erreur(Classe::Illisible, format!("journal : le contrat {} est parti sans sa version", numero)))?;
        let o = engagement_de(&ctx_j, mois_c, &cle, &numero)?;
        paies.push(paie_de_la_version(&ctx_j.du_mois(mois_c).map_err(|e| e.reclasser(Classe::Illisible))?, &o, &cle, &numero, mois_c, version)?);
    }
    let (Some(premier), Some(p0)) = (contrats.first(), paies.first()) else {
        return Err(erreur(Classe::Illisible, format!("journal : aucun contrat ne part dans {}", nom)));
    };
    let ent = documents::avenant::ent_du_mois(&ctx_j, premier.mois);
    let ecrire = || -> R<()> {
        if premier.est_avenant() {
            let numero_av = premier.avenant.ok_or_else(|| erreur(Classe::Illisible, format!("journal : {} sans numero d'avenant", nom)))?;
            // Le contrat qu'il modifie : parti avant lui, avec son modele fige ; ou signe hors du logiciel (`ailleurs`),
            // date de son premier jour, avec le modele que son engagement suit.
            let (date_contrat, modele) = if premier.ailleurs {
                (premier.debut, crate::modeles_contrat::du_contrat(&ctx_j, p0)?)
            } else {
                let parti = envois::etats::contrat_parti(&lignes[..rang], &premier.salarie, premier.numero_dit())
                    .ok_or_else(|| erreur(Classe::Illisible, format!("journal : {} modifie un contrat qui n'est pas parti avant lui", nom)))?;
                (parti.date(), crate::modeles_contrat::du_contrat_parti(&ctx_j, parti.modele.as_deref(), p0)?)
            };
            let modifs = crate::gestes::signature::pour_la_piece(premier.modifications.as_deref().ok_or_else(|| erreur(Classe::Illisible, format!("journal : {} sans les articles qu'il modifie", nom)))?);
            let effet = premier.effet.ok_or_else(|| erreur(Classe::Illisible, format!("journal : {} sans sa date d'effet", nom)))?;
            return documents::contrat::ecrire_avenant(&ctx_j, p0, &ent, vers, &modifs, numero_av, date_contrat, effet, jour, &modele, premier.ailleurs);
        }
        let mut remplacements: Vec<((String, socle::date::Jour, String), (String, socle::date::Jour))> = Vec::new();
        if nom.strip_suffix("remplace.pdf").is_some_and(|b| b.ends_with('_')) {
            for c in &contrats {
                let parti = envois::etats::contrat_parti(&lignes[..rang], &c.salarie, c.numero_dit())
                    .ok_or_else(|| erreur(Classe::Illisible, format!("journal : {} remplace un contrat qui n'est pas parti avant lui", nom)))?;
                let numero = match &parti.numero {
                    Some(n) => n.to_string(),
                    None => SANS_NUMERO.to_string(),
                };
                let le_sien = c.numero_dit().ok_or_else(|| erreur(Classe::Illisible, format!("journal : un contrat de {} est parti sans numero", nom)))?;
                remplacements.push(((c.salarie.to_string(), c.debut, le_sien.to_string()), (numero, parti.date())));
            }
        }
        // Chaque contrat avec le modele qu'il suivait a son depart.
        let mut modeles = Vec::new();
        for (c, p) in contrats.iter().zip(&paies) {
            modeles.push(crate::modeles_contrat::du_contrat_parti(&ctx_j, c.modele.as_deref(), p)?);
        }
        documents::contrat::ecrire_contrat(&ctx_j, &paies, &modeles, &ent, vers, &remplacements)
    };
    socle::temps::pendant(instant, ecrire)?;
    let mut pareils = Ok(true);
    for (c, p) in contrats.iter().zip(&paies) {
        if let (Some(cl), Ok(true)) = (&c.clauses, &pareils) {
            pareils = crate::gestes::signature::clauses_du_contrat(&ctx_j, p, &ent).map(|refaites| refaites == *cl);
        }
    }
    let pareils = match pareils {
        Ok(p) => p,
        Err(e) => {
            let _ = std::fs::remove_file(vers);
            return Err(e);
        }
    };
    if !pareils {
        let _ = std::fs::remove_file(vers);
        return Err(erreur(Classe::Refuse, format!("{} ne se refait pas à l'identique de ce qui est parti : le fichier a été déplacé ou effacé.", nom)));
    }
    Ok(())
}

/// Le nom d'une archive de retour d'AER : `<instant>_nature46_…`, la nature 46 de France Travail.
const NATURE_AER: &str = "nature46";

/// La base d'un recapitulatif (`<base>_recapitulatif.txt`), si `nom` en est un.
fn recapitulatif_de(nom: &str) -> Option<&str> {
    nom.strip_suffix("recapitulatif.txt")?.strip_suffix('_')
}

/// Ce que la tete d'un contrat qui en remplace un autre dit du contrat parti avant lui quand celui-ci n'avait pas de
/// numero (signe hors du logiciel).
const SANS_NUMERO: &str = "(sans numéro)";

/// La paie ecrite de `mois` que porte la version de l'engagement `o` dont le bulletin a le numero `numero`.
fn paie_ecrite_au_bulletin(o: &engagement::stock::Objet, mois: Mois, numero: &str) -> R<Option<engagement::paie_ecrite::PaieEcrite>> {
    Ok(engagement::paies::ecrites(o, mois)?.into_iter().find(|p| p.bulletin.as_deref() == Some(numero)))
}

/// LES DOCUMENTS QU'A FAIT PARTIR LA LIGNE `lignes[rang]` (un envoi aux salaries de `mois`), refaits tels qu'ils sont
/// partis : chaque engagement du mois dans la version sur laquelle ses derniers documents etaient partis a ce moment-la
/// (`engagement::etats::paie_partie`), sous les numeros que cette version porte ; le rectificatif d'une paie refaite,
/// avec l'ecart au bulletin parti avant lui quand la DSN du mois ne se refaisait plus ; au jour et a l'heure de l'envoi.
/// Seuls les fichiers qui manquent s'ecrivent, et aucun si `ecrire` est faux. Rend, pour chaque paie de l'envoi, le
/// salarie et le numero de son bulletin.
fn refaire_les_documents(ctx: &Contexte, mois: Mois, lignes: &[envois::journal::Ligne], rang: usize, ecrire: bool) -> R<Vec<(String, Option<String>)>> {
    let l = &lignes[rang];
    let (jour, instant) = instant_de(l.quand())?;
    let cles: Vec<String> = l.documents().ok_or_else(|| erreur(Classe::Illisible, "journal : la ligne n'est pas un envoi de documents".to_string()))?.detail.salaries.iter().map(|s| s.cle.to_string()).collect();
    let refaire = || -> R<Vec<(String, Option<String>)>> {
        let ctx = contexte_du_jour(ctx, jour)?.du_mois(mois).map_err(|e| e.reclasser(Classe::Illisible))?;
        let jusqu_a = envois::journal::departs_des_lignes(&lignes[..=rang]);
        let avant = envois::journal::departs_des_lignes(&lignes[..rang]);
        let clos = regularisations::se_corrige_ailleurs(&ctx, &avant, mois)?;
        let mut paies: Vec<Paie> = Vec::new();
        let mut numeros: Vec<Option<String>> = Vec::new();
        let mut premiers_ce: Vec<Option<i64>> = Vec::new();
        let mut rectificatifs: HashMap<usize, documents::bulletin::Rectificatif> = HashMap::new();
        for e in engagement::etats::du_mois(&ctx, mois, &envois::journal::lu(&ctx.racine).map(|j| j.departs)?)? {
            let Some(v) = e.valide() else { continue };
            let Some(p) = engagement::etats::paie_partie(&ctx, v.objet(), &jusqu_a, mois)? else { continue };
            let ecrite = match p.numero_bulletin.as_deref() {
                Some(n) => paie_ecrite_au_bulletin(v.objet(), mois, n)?,
                None => None,
            };
            if let Some(r) = ecrite.as_ref().and_then(|x| x.rectifie.as_deref()) {
                let parti = if clos { engagement::etats::paie_partie(&ctx, v.objet(), &avant, mois)? } else { None };
                let ecart = match parti {
                    Some(parti) => {
                        let porteur = crate::regularisation::dsn_qui_porte_la_correction(&ctx, v, &avant, mois)?;
                        Some(ecart_avec_le_bulletin_parti(&ctx, &p, &parti, porteur)?)
                    }
                    None => None,
                };
                rectificatifs.insert(paies.len(), documents::bulletin::Rectificatif { rectifie: r.to_string(), mois_origine: mois, ecart });
            }
            numeros.push(if p.engagement.sans_paie() { None } else { p.numero_bulletin.clone() });
            premiers_ce.push(ecrite.as_ref().and_then(|x| x.ce));
            paies.push(p);
        }
        let (mut pour_cs, _) = crate::dossier_salarie::certificats_dus(&ctx, &paies)?;
        // Dans l'ordre des salaries de l'envoi ; un salarie que l'envoi ne nomme pas vient apres eux.
        pour_cs.sort_by_key(|(cle, _, _)| (cles.iter().position(|c| c == cle).is_none(), cles.iter().position(|c| c == cle)));
        let mut certificats: Vec<(usize, &PaieCs, i64)> = Vec::new();
        for (cle, debut, pc) in &pour_cs {
            let Some(pc) = pc else { continue };
            let Some(i) = (0..paies.len()).find(|i| cle_de(&paies[*i]) == *cle && paies[*i].engagement.debut == *debut) else { continue };
            if let Some(n) = premiers_ce[i] {
                certificats.push((i, pc, n));
            }
        }
        let ent = ctx.pour_un_modele(&ctx.entreprise_du_mois(mois));
        let impression = Impression { paies: &paies, numeros: &numeros, rectificatifs: &rectificatifs, certificats: &certificats };
        if ecrire {
            imprimer(&ctx, mois, &ent, &impression, &cles, true)?;
        }
        Ok(paies.iter().zip(&numeros).map(|(p, n)| (cle_de(p), n.clone())).collect())
    };
    socle::temps::pendant(instant, refaire)
}

/// LE TAUX DE PAS DES PAIES QUI PARTENT : une phrase par salarie dont une paie n'a pas de taux transmis par la DGFiP qui
/// vaille le jour de son paiement. Elle prend alors le taux non personnalise de la grille : « Lorsque le débiteur ne dispose
/// pas d'un taux calculé par l'administration fiscale [...] il est appliqué un taux proportionnel » (CGI, art. 204 H, III,
/// 1, `atelier/reference/sources/code-general-des-impots/xml/204_H.xml` l. 126). Se dit au depot de la DSN et a la
/// production des bulletins, pour ces paies-la seulement.
fn sans_taux_recu<'a>(ctx: &Contexte, paies: impl Iterator<Item = &'a Paie>) -> R<Vec<String>> {
    let mut lignes: Vec<String> = Vec::new();
    // La paie dit le taux qu'elle a pris (`Paie::pas`, ecrit a la validation) : un autre type que « 01 » (taux transmis
    // par la DGFiP, S21.G00.50.007) est la grille.
    for p in paies.filter(|p| p.pas.as_ref().is_some_and(|v| v.type_taux.as_deref() != Some("01"))) {
        let paye_le = p.engagement.date_du_versement()?;
        let pourquoi = droit::pas::avertissements(ctx, Some(&[cle_de(p)]), Some(paye_le), Some(p.engagement.mois))?;
        let demande: String = compagnie::amorcage_demande::en_cours(ctx, &cle_de(p))?
            .map(|d| format!(" ; taux demandé le {}, réponse attendue avant le {}", d.quand.jj_mm_aaaa(), d.avant.jj_mm_aaaa()))
            .into_iter()
            .collect();
        // UN TAUX ARRIVE APRES QUE LA PAIE EST PARTIE (ses documents), avant son paiement : rien ne dit qu'aucun taux ne
        // valait. La paie garde la grille, sans regularisation : « Aucune régularisation ne doit être effectuée par le
        // débiteur en raison de la réception pour la première fois d'un taux personnalisé alors qu'il a, jusque-là,
        // régulièrement appliqué les grilles de taux par défaut. Le taux personnalisé ne s'applique que pour l'avenir »
        // (BOI-IR-PAS-30-10-50, § 20, `atelier/reference/sources/bofip/BOI-IR-PAS-30-10-50-20190227.html`).
        let l = match pourquoi.first() {
            Some(x) => format!("{}{} : sa paie prend le taux non personnalisé (CGI, art. 204 H).", x, demande),
            None => format!(
                "{} : le taux transmis par la DGFiP est arrivé après le départ de sa paie, qui garde le taux non personnalisé ; il vaut pour les suivantes (BOI-IR-PAS-30-10-50, § 20).",
                ctx.nomme(&cle_de(p))
            ),
        };
        if !lignes.contains(&l) {
            lignes.push(l);
        }
    }
    Ok(lignes)
}
