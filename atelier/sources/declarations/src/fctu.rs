//! Le signalement de fin de contrat de travail unique, nature 07.
//!
//! Il porte l'evenement ET la paie du solde de tout compte (cahier technique
//! 2026.1 p. 107) : un individu, un contrat, un bloc 62 et un bloc 63
//! obligatoires, ni bordereau ni ordre de telepaiement. Les controles qui lui
//! sont propres (CCH-14, CCH-15, CCH-11) sont tenus ici ; `Echec::Impossible` est la regle de la norme qui s'oppose.

use droit::fiche::Lue as _;

use droit::base_assujettie::BaseAssujettie;
use droit::contexte::{aujourdhui, Contexte};
use socle::date::Jour;
use crate::dsn::{
    autre_revenu, bases_et_cotisations, bloc_anciennete, bloc_contrat_fctu, primes_et_indemnites,
    remunerations, usage_admet, Arret, Fichier, LOGICIEL, VERSION,
};
use engagement::Engagement;
use socle::echec::{Classe, R};
use socle::fichiers::repr;
use droit::type_declaration::TypeDeLaDeclaration;
use droit::preavis::{TransactionEnCours, TypeDePreavis};
use droit::type_de_prime::TypeDePrime;

pub const NATURE: droit::nature_declaration::NatureDeLaDeclaration = droit::nature_declaration::NatureDeLaDeclaration::SignalementFinDeContrat;
/// CCH-14 (cahier technique p. 293) : la base assujettie de prevoyance n'a
/// pas sa place dans un signalement de fin de contrat.
const BASES_INTERDITES: [BaseAssujettie; 1] = [BaseAssujettie::PrevoyanceSanteRetraite];
/// Les types qu'un FCTU admet : pas de « 02 » ni de « 05 », qui ne sont « autorises que pour une nature de declaration
/// "01 - DSN Mensuelle" » (CCH-14, `atelier/reference/norme-dsn/texte/bloc-S20.G00.05.txt` l. 70-71).
const ADMIS: [TypeDeLaDeclaration; 3] = [TypeDeLaDeclaration::Normale, TypeDeLaDeclaration::AnnuleEtRemplace, TypeDeLaDeclaration::Annule];

/// Un refus d'un signalement : `Impossible` est la regle de la norme qui s'oppose (un refus ne la ou elle s'oppose, de
/// classe `Refuse`) ; `Autre` tout le reste.
#[derive(Debug, Clone)]
pub enum Echec {
    Impossible(socle::echec::Echec),
    Autre(socle::echec::Echec),
}

impl From<socle::echec::Echec> for Echec {
    fn from(e: socle::echec::Echec) -> Echec {
        Echec::Autre(e)
    }
}

impl Echec {
    /// La regle de la norme qui s'oppose, en une phrase, son endroit note.
    #[track_caller]
    pub fn impossible(phrase: impl Into<String>) -> Echec {
        Echec::Impossible(socle::echec::erreur(Classe::Refuse, phrase))
    }

    /// Le refus, de la classe `classe` que son lecteur lui donne ; il garde ce qu'il porte a part.
    pub fn en_classe(self, classe: Classe) -> socle::echec::Echec {
        match self {
            Echec::Impossible(e) | Echec::Autre(e) => e.reclasser(classe),
        }
    }
}

/// `fctu-base.json`, lu par sa forme (`droit::preavis::base`).
fn nomenclatures() -> R<droit::preavis::BaseFctu> {
    droit::preavis::base()
}

/// `FinDeContrat.libelle_preavis` : le libelle du type de preavis ; un type que la nomenclature ne connait pas refuse en
/// le nommant.
pub fn libelle_preavis(type_preavis: TypeDePreavis) -> R<String> {
    match nomenclatures()?.types_preavis.du_code(type_preavis) {
        Some(c) => Ok(c.libelle.clone()),
        None => Err(socle::echec::cle(format!("Le type de préavis « {} » n'est pas dans la nomenclature de la DSN.", type_preavis), "type_preavis")),
    }
}

/// Le code par defaut d'une nomenclature.
fn defaut<T: Copy>(n: &droit::preavis::Nomenclature<T>, quoi: &str) -> R<T> {
    n.defaut().ok_or_else(|| socle::echec::livree("fctu-base.json", quoi))
}

/// La fin d'un contrat, telle qu'elle se declare. Le dernier jour travaille et paye
/// (S21.G00.62.006) n'y est pas : il se lit sur le contrat, par `dernier_jour_paye`, comme dans la mensuelle.
#[derive(Debug, Clone)]
pub struct FinDeContrat {
    pub fin: Jour,
    pub motif: droit::natures::MotifDeRupture,
    pub mois_rattachement: socle::mois::Mois,
    pub type_preavis: TypeDePreavis,
    pub preavis_debut: Option<Jour>,
    pub preavis_fin: Option<Jour>,
    pub notification: Option<Jour>,
    pub signature_convention: Option<Jour>,
    pub engagement_licenciement: Option<Jour>,
    pub transaction: Option<TransactionEnCours>,
}


// ------------------------------------------------ le dernier jour travaille et paye

/// Le type de preavis (S21.G00.63.001) est-il un preavis NON effectue ? Le drapeau `non_effectue` de
/// `fctu-base.json` : 02, 03, 10, 50, 51, 61, que nomme la CCH-12 de S21.G00.62.006 (cahier technique DSN 2026.1,
/// p. 275). Un type inconnu refuse.
pub fn preavis_non_effectue(type_preavis: TypeDePreavis) -> R<bool> {
    match nomenclatures()?.types_preavis.du_code(type_preavis) {
        Some(c) => Ok(c.non_effectue == Some(true)),
        None => Err(socle::refus!(
            "Type de préavis {} inconnu de la norme (S21.G00.63.001) : le choisir dans la liste.",
            repr(type_preavis.code())
        )),
    }
}

// ------------------------------------------------ la date de notification de la rupture

/// La date de notification de la rupture (S21.G00.62.003) est-elle obligatoire pour ce motif de rupture ? La liste
/// `notification_rupture.motifs` de `fctu-base.json`, celle de la CCH-12 de S21.G00.62.003 (cahier technique DSN 2026.1,
/// p. 275, `atelier/reference/norme-dsn/texte/bloc-S21.G00.62.txt`). Un signalement de ce motif qui ne la porte pas est
/// refuse par le point de depot. Un seul endroit : la liste des fins a signaler (`routes::imprevus::ce_qui_manque`) et
/// `verifier` la lisent ici.
pub fn notification_exigee(motif: droit::natures::MotifDeRupture) -> R<bool> {
    Ok(nomenclatures()?.notification_rupture.motifs.contains(&motif))
}

// ------------------------------------------------ le type de preavis et ce que la paie verse

/// Les lignes d'une paie declarees en indemnite compensatrice de preavis (S21.G00.52.001), en (code, libelle).
pub fn indemnites_de_preavis_versees(paie: &engagement::calcul::Paie) -> R<Vec<(TypeDePrime, String)>> {
    let codes = droit::preavis::codes_52_de_l_indemnite_de_preavis()?;
    Ok(paie
        .remuneration
        .iter()
        .filter(|l| !l.montant.is_zero())
        .filter_map(|l| l.dsn_bloc52.filter(|c| codes.contains(c)).map(|c| (c, l.libelle.clone())))
        .collect())
}

/// Les controles de S21.G00.62.006 (cahier technique DSN 2026.1, p. 275-276) : CCH-15, le jour tombe dans le contrat
/// (`debut` = 40.001, `fin` = 62.001) ; CCH-12, sur un preavis non effectue, il precede le debut du preavis (63.002).
fn controler_62_006(
    jour_paye: &Jour, debut: &Jour, fin: &Jour, type_preavis: Option<TypeDePreavis>, debut_preavis: Option<&Jour>,
) -> R<()> {
    if jour_paye > fin {
        return Err(socle::refus!(
            "Le dernier jour travaille et paye au salaire habituel ({}) est posterieur a la fin du contrat ({}).",
            jour_paye.iso(),
            fin.iso()
        ));
    }
    if jour_paye < debut {
        return Err(socle::refus!(
            "Le dernier jour travaille et paye au salaire habituel ({}) est anterieur au debut du contrat ({}) -- CCH-15 \
de S21.G00.62.006.",
            jour_paye.iso(),
            debut.iso()
        ));
    }
    if let (Some(t), Some(dp)) = (type_preavis, debut_preavis) {
        if preavis_non_effectue(t)? && jour_paye >= dp {
            return Err(socle::refus!(
                "Preavis « {} » : le dernier jour travaille et paye au salaire habituel ({}) doit preceder le debut du \
preavis ({}) -- CCH-12 de S21.G00.62.006.",
                libelle_preavis(t)?,
                jour_paye.iso(),
                dp.iso()
            ));
        }
    }
    Ok(())
}

/// S21.G00.62.006, « Dernier jour travaille paye selon le salaire defini par le contrat de travail ou la convention
/// collective » (cahier technique DSN 2026.1, p. 275, `atelier/reference/norme-dsn/texte/bloc-S21.G00.62.txt`) : LA
/// SEULE SOURCE, que lisent la mensuelle (`dsn::bloc_contrat_fctu`, le mois ou le contrat finit) et le FCTU du meme
/// contrat. La mensuelle doit reporter « a l'identique » ce que porte le signalement (§ 2.5.2, p. 68).
///
/// Le jour que la saisie dit (`dernier_jour_paye`) ; sinon la fin du contrat, qui n'est ce jour-la que si le preavis
/// a ete effectue ou qu'il n'y en a pas. Sur un preavis NON effectue (02, 03, 10, 50, 51, 61), le dernier jour paye au
/// salaire habituel precede le debut du preavis (CCH-12) : sans saisie, le moteur refuse plutot que de le deviner.
pub fn dernier_jour_paye(e: &Engagement) -> R<Jour> {
    let numero = e.numero_declare()?;
    let fin = e.fin.ok_or_else(|| socle::refus!("S21.G00.62.006 : le contrat {} n'a pas de fin.", numero))?;
    let preavis = e.preavis.as_ref();
    let type_preavis = preavis.map(|p| p.genre);
    let debut_preavis = preavis.and_then(|p| p.debut);
    let jour_paye = match e.dernier_jour_paye {
        Some(j) => j,
        None => {
            if let Some(t) = type_preavis {
                if preavis_non_effectue(t)? {
                    return Err(socle::refus!(
                        "Contrat {} : preavis « {} », et la saisie ne dit pas le dernier jour travaille et paye au salaire \
habituel (« Dernier jour payé », S21.G00.62.006). Sur un preavis non effectue, ce n'est pas la fin du contrat ({}) : il \
precede le debut du preavis (CCH-12, cahier technique DSN p. 275). Il se saisit ; le moteur ne le devine pas.",
                        numero,
                        libelle_preavis(t)?,
                        fin.iso()
                    ));
                }
            }
            fin
        }
    };
    controler_62_006(&jour_paye, &e.debut, &fin, type_preavis, debut_preavis.as_ref())
        .map_err(|m| m.reformuler(|m| format!("Contrat {} : {}", numero, m)))?;
    Ok(jour_paye)
}

// ------------------------------------------------------------- les controles

/// `verifier` : refuse ce que la norme refuse ; rend les avertissements.
pub fn verifier(fin: &FinDeContrat, e: &Engagement, type_declaration: TypeDeLaDeclaration) -> Result<Vec<String>, Echec> {
    let mut avertissements = Vec::new();
    if !ADMIS.contains(&type_declaration) {
        return Err(Echec::impossible(format!(
            "Type de declaration {}. Un FCTU est {}.",
            repr(type_declaration.code()),
            ADMIS.iter().map(|t| format!("{} - {}", t.code(), t.libelle())).collect::<Vec<_>>().join(", ")
        )));
    }
    let types = nomenclatures()?.types_preavis;
    let preavis = match types.du_code(fin.type_preavis) {
        Some(p) => p,
        None => {
            return Err(Echec::impossible(format!(
                "Type de preavis {} inconnu de la norme (S21.G00.63.001). Les types sont dans fctu-base.json.",
                repr(fin.type_preavis.code())
            )))
        }
    };
    let libelle = preavis.libelle.clone();
    let dates = preavis.dates;
    let (debut_p, fin_p) = (fin.preavis_debut, fin.preavis_fin);

    // Le bloc 63 est (1,*) : un FCTU en porte toujours un.
    if dates && !(debut_p.is_some() && fin_p.is_some()) {
        return Err(Echec::impossible(format!(
            "Preavis « {} » : ses deux dates sont OBLIGATOIRES (CCH-11 de S21.G00.63.002 et CCH-14 de .003 -- elles \
sont presentes si et seulement si le type n'est pas 90). Si le contrat n'a pas de preavis, le type est « 90 - pas de \
clause de preavis applicable », et les dates sont alors interdites.",
            libelle
        )));
    }
    if !dates && (debut_p.is_some() || fin_p.is_some()) {
        return Err(Echec::impossible(format!(
            "Preavis « {} » : les dates de preavis y sont INTERDITES (CCH-11 de S21.G00.63.002).",
            libelle
        )));
    }
    if let (true, Some(pd), Some(pf)) = (dates, debut_p, fin_p) {
        if pf < pd {
            return Err(Echec::impossible(format!(
                "Preavis : la date de fin ({}) est anterieure a la date de debut ({}) -- CCH-13 de S21.G00.63.003.",
                pf.iso(),
                pd.iso()
            )));
        }
        // CCH-16 : le preavis commence apres le debut du contrat.
        if pd < e.debut {
            return Err(Echec::impossible(format!(
                "Preavis : il commence le {}, avant le debut du contrat ({}) -- CCH-16 de S21.G00.63.002.",
                pd.iso(),
                e.debut.iso()
            )));
        }
        // CCH-13 : le preavis commence apres la notification de la rupture.
        if let Some(n) = fin.notification {
            if pd < n {
                return Err(Echec::impossible(format!(
                    "Preavis : il commence le {}, avant la notification de la rupture ({}) -- CCH-13 de \
S21.G00.63.002.",
                    pd.iso(),
                    n.iso()
                )));
            }
        }
    }

    // CCH-11 et CCH-12 du bloc 63 : deux motifs limitent le type de preavis.
    if let Some(limite) = types.permis_au_motif(fin.motif) {
        let permis = &limite.types;
        if !permis.contains(&fin.type_preavis) {
            let mut noms = Vec::new();
            for c in permis {
                let lib = types.du_code(*c).map(|x| x.libelle.clone());
                noms.push(format!("« {} - {} »", c, lib.ok_or_else(|| socle::fichiers::absent(c.code()))?));
            }
            return Err(Echec::impossible(format!(
                "Preavis « {} » avec le motif de rupture {} : {} n'y permet que {}. {}",
                libelle,
                fin.motif,
                limite.controle,
                noms.join(", "),
                if fin.motif == droit::natures::MotifDeRupture::RuptureConventionnelle {
                    "Une rupture conventionnelle n'a pas de preavis : sa convention FIXE la date de rupture (L1237-13)."
                } else {
                    "Une rupture en periode d'essai releve du delai de prevenance (L1221-25 et L1221-26), pas du \
preavis de L1234-1."
                }
            )));
        }
    }

    // CCH-11 de S21.G00.62.003 : la notification tombe dans le contrat.
    if let Some(nt_j) = fin.notification {
        if !(e.debut <= nt_j && nt_j <= fin.fin) {
            return Err(Echec::impossible(format!(
                "Notifiée le {} : la rupture se notifie pendant le contrat, entre son premier jour, le {}, et sa fin, le \
{} -- CCH-11 de S21.G00.62.003.",
                nt_j.jj_mm_aaaa(),
                e.debut.jj_mm_aaaa(),
                fin.fin.jj_mm_aaaa()
            )));
        }
    }

    // CCH-12 de S21.G00.62.003 : pour ces motifs, la date de notification est obligatoire.
    if fin.notification.is_none() && notification_exigee(fin.motif)? {
        return Err(Echec::impossible(format!(
            "Motif de rupture {} : la date de notification de la rupture (S21.G00.62.003, `notification_rupture` dans la \
saisie) est OBLIGATOIRE -- CCH-12 de S21.G00.62.003, cahier technique DSN p. 275. Le point de depot refuserait le \
signalement sans elle. Rien n'est ecrit.",
            fin.motif
        )));
    }

    // S21.G00.62.006 se lit sur le contrat, comme dans la mensuelle ; ses controles valent aussi contre la fin et le
    // preavis que ce signalement declare (62.001, 63.001, 63.002).
    let jour_paye = dernier_jour_paye(e).map_err(Echec::Impossible)?;
    controler_62_006(&jour_paye, &e.debut, &fin.fin, Some(fin.type_preavis), debut_p.as_ref())
        .map_err(Echec::Impossible)?;

    // CCH-11 de S21.G00.62.002 : le motif doit etre autorise pour la nature du contrat (091/092 pas pour un CDD).
    let nat = e.nat()?;
    if let Some(refus) = nat.motif_rupture_refuse(fin.motif, e.numero_declare()?) {
        return Err(Echec::impossible(refus));
    }

    // La derogation du CDDU (circuit derogatoire, motif 031 seul : fiche 2062) : le moteur le DIT plutot que de refuser,
    // car le salarie peut demander le signalement (fiche 1902). Rompu avant son terme, le CDDU se signale.
    if !nat.signalement_fctu(Some(fin.motif)) {
        avertissements.push(format!(
            "Contrat {} : un {} arrivé à son terme n'a pas besoin de signalement de fin de contrat ; sa fin part dans la \
DSN du mois, par le circuit dérogatoire. Ce signalement ne se fait que si le salarié le demande.",
            e.numero_declare()?,
            nat.libelle
        ));
    }
    avertissements.push(format!(
        "Fin du contrat le {} : le signalement est dû dans les cinq jours ouvrés qui suivent (cahier technique p. 107).",
        fin.fin.jj_mm_aaaa()
    ));
    Ok(avertissements)
}

// ------------------------------------------------------------ les blocs 62/63

/// La fin du contrat et son preavis, dans l'ordre de l'arborescence. `e` : le contrat, d'ou se lit S21.G00.62.006
/// (`dernier_jour_paye`).
pub fn bloc_62_63(f: &mut Fichier, fin: &FinDeContrat, e: &Engagement) -> R<()> {
    f.add("S21.G00.62.001", fin.fin.jjmmaaaa());
    f.ecrire_code(fin.motif)?;
    if let Some(n) = fin.notification {
        f.add("S21.G00.62.003", n.jjmmaaaa());
    }
    if let Some(s) = fin.signature_convention {
        f.add("S21.G00.62.004", s.jjmmaaaa());
    }
    if let Some(s) = fin.engagement_licenciement {
        f.add("S21.G00.62.005", s.jjmmaaaa());
    }
    f.add("S21.G00.62.006", dernier_jour_paye(e)?.jjmmaaaa());
    // << Transaction en cours >> est OBLIGATOIRE : elle se declare meme pour
    // dire qu'il n'y en a pas.
    match &fin.transaction {
        Some(t) => f.add("S21.G00.62.008", t),
        None => f.add("S21.G00.62.008", defaut(&nomenclatures()?.transaction, "transaction")?),
    }
    // S21.G00.62.020 : « cette date doit etre renseignee au premier jour du
    // mois (sous la forme 01mmaaaa) » -- cahier technique p. 278.
    f.add("S21.G00.62.020", fin.mois_rattachement.premier_jour().jjmmaaaa());
    f.add("S21.G00.63.001", &fin.type_preavis);
    if let Some(p) = fin.preavis_debut {
        f.add("S21.G00.63.002", p.jjmmaaaa());
    }
    if let Some(p) = fin.preavis_fin {
        f.add("S21.G00.63.003", p.jjmmaaaa());
    }
    Ok(())
}

// ---------------------------------------------------------------- generation

/// Les parametres de `generer`, avec leurs defauts.
pub struct Options {
    pub type_declaration: TypeDeLaDeclaration,
    pub ordre: i64,
    pub remplace: Option<String>,
    pub envoi_test: bool,
    pub date_constitution: Option<Jour>,
}

impl Options {
    /// Un premier signalement normal, en reel, constitue le jour de son depot.
    pub fn normale() -> Options {
        Options { type_declaration: TypeDeLaDeclaration::Normale, ordre: 1, remplace: None, envoi_test: false, date_constitution: None }
    }
}

/// Le fichier d'un FCTU, pret a deposer : les suspensions de l'engagement de la paie (bloc 65) ;
/// `arrets` : les arrets a reporter (bloc 60) ; `departs` : ce qui est parti, pour lire les engagements valides du mois.
pub fn generer(
    ctx: &Contexte, paie: &engagement::calcul::Paie, fin: &FinDeContrat, o: &Options, arrets: &[Arret], departs: &engagement::departs::Departs,
) -> Result<String, Echec> {
    let type_declaration = o.type_declaration;
    let remplace = o.remplace.as_deref().filter(|r| !r.is_empty());
    if matches!(type_declaration, TypeDeLaDeclaration::AnnuleEtRemplace | TypeDeLaDeclaration::Annule) && remplace.is_none() {
        return Err(Echec::impossible(format!(
            "Un signalement « {} » doit viser la declaration qu'il reprend (S20.G00.05.006) : sans elle, la \
plate-forme rend B1-208-08.",
            type_declaration.libelle()
        )));
    }
    if type_declaration == TypeDeLaDeclaration::Normale && remplace.is_some() {
        return Err(Echec::impossible(
            "S20.G00.05.006 est interdite sur un signalement normal : elle ne s'ecrit que sur un annule-et-remplace \
ou un annule.",
        ));
    }
    verifier(fin, &paie.engagement, type_declaration)?;
    // Le type de preavis (63.001) et l'indemnite que la paie du solde verse (52.001) disent la meme chose.
    if let Some(pourquoi) = droit::preavis::desaccord_du_preavis(fin.type_preavis,&indemnites_de_preavis_versees(paie)?)? {
        return Err(Echec::impossible(format!(
            "Contrat {} : {}",
            paie.engagement.numero_declare()?,
            pourquoi
        )));
    }

    let e = &paie.engagement;

    let fin_jour = fin.fin;
    let ctx = &ctx.au(&fin_jour)?;
    let ent0 = &ctx.entreprise;
    let s = &paie.salarie;
    let bar = crate::dsn::BaremeDuMois::nouveau((*ctx.bareme(&fin_jour)?).clone(), ctx.entreprise_lue(&fin_jour)?);
    let constitution = match o.date_constitution {
        Some(j) => j,
        None => aujourdhui()?,
    };
    let (debut_p, fin_p) = (e.debut_periode(), e.fin_periode());

    // La version de la norme d'un signalement suit le jour ou il est constitue (`norme-dsn-base.json`, une inference :
    // `note`) ; un jour que la donnee ne couvre pas refuse.
    let norme = crate::norme::du_signalement(&constitution)?;
    crate::naf::verifier_pour(&norme, ent0)?;
    let mut f = Fichier::en_norme(norme);
    // ------------------------------------------------------------ S10 envoi
    f.add("S10.G00.00.001", LOGICIEL);
    f.add("S10.G00.00.002", LOGICIEL);
    f.add("S10.G00.00.003", VERSION);
    f.add("S10.G00.00.005", if o.envoi_test { "01" } else { "02" });
    f.ecrire_la_version()?;
    f.add("S10.G00.00.007", "01");
    f.add("S10.G00.00.008", "01");
    f.add_case("S10.G00.01.001", &ent0.siren, "siren")?;
    f.add_case("S10.G00.01.002", &ent0.nic, "nic")?;
    f.add_case("S10.G00.01.003", &ent0.raison_sociale, "raison_sociale")?;
    f.add_case("S10.G00.01.004", &ent0.adresse, "adresse")?;
    f.add_case("S10.G00.01.005", &ent0.code_postal, "code_postal")?;
    f.add_case("S10.G00.01.006", &ent0.commune, "commune")?;
    f.add_complement("S10.G00.01.009", &ent0.complement_adresse);
    crate::dsn::contact_emetteur(&mut f, ent0)?;

    // ---------------------------------------------------------- S20 declaration
    f.ecrire_code(NATURE)?;
    f.ecrire_code(type_declaration)?;
    f.add("S20.G00.05.003", "11");
    f.add("S20.G00.05.004", o.ordre);
    // S20.G00.05.005 (mois principal declare) est INTERDITE : un signalement
    // ne declare pas un mois.
    if let Some(r) = remplace {
        f.add("S20.G00.05.006", r);
    }
    f.add("S20.G00.05.007", constitution.jjmmaaaa());
    f.add("S20.G00.05.010", "01");
    crate::dsn::contact_chez_le_declare(&mut f, ent0)?;

    // ------------------------------------------------------------ S21 entreprise
    f.add_case("S21.G00.06.001", &ent0.siren, "siren")?;
    f.add_case("S21.G00.06.002", &ent0.nic, "nic")?;
    // Code APEN et code APET : retires de la norme P27V01 (cahier 2027.1.1, p. 142 et 145) ; la version en decide.
    if usage_admet(&f, droit::rubrique!("S21.G00.06.003"), NATURE)? {
        f.add_case("S21.G00.06.003", &ent0.naf, "naf")?;
    }
    f.add_case("S21.G00.06.004", &ent0.adresse, "adresse")?;
    f.add_case("S21.G00.06.005", &ent0.code_postal, "code_postal")?;
    f.add_case("S21.G00.06.006", &ent0.commune, "commune")?;
    f.add_complement("S21.G00.06.007", &ent0.complement_adresse);
    // L'etablissement est celui du salarie, comme dans ses mensuelles (`droit::bareme::entreprise_du_salarie`) : le NIC est
    // une donnee identifiante (cahier technique DSN 2026.1, § 2.5.1, p. 67).
    let etab = droit::bareme::entreprise_du_salarie(ent0, &paie.salarie)?;
    for (rub, case, cle) in [
        ("S21.G00.11.001", &etab.nic.en_texte(), "nic"),
        ("S21.G00.11.002", &etab.naf.en_texte(), "naf"),
        ("S21.G00.11.003", &etab.adresse.en_texte(), "adresse"),
        ("S21.G00.11.004", &etab.code_postal.en_texte(), "code_postal"),
        ("S21.G00.11.005", &etab.commune.en_texte(), "commune"),
        ("S21.G00.11.022", &etab.idcc.en_texte(), "idcc"),
    ] {
        if rub == "S21.G00.11.002" && !usage_admet(&f, droit::rubrique!("S21.G00.11.002"), NATURE)? {
            continue;
        }
        f.add_case(rub, case, cle)?;
        if rub == "S21.G00.11.005" {
            f.add_complement("S21.G00.11.006", &etab.complement_adresse);
        }
    }
    // Les adhesions prevoyance, sans S21.G00.15.004 : la rubrique « personnel
    // couvert » est interdite dans un signalement.
    let orgs = ctx.organismes()?;
    // Le numero de licence EN VIGUEUR a la fin du contrat, pas la liste.
    let mut ent_l = etab.clone();
    ent_l.licence_spectacle = ctx.licence(&fin_jour);
    for a in &orgs.adhesions_prevoyance {
        f.add("S21.G00.15.001", &a.libelle);
        f.add("S21.G00.15.002", &a.delegataire);
        f.add("S21.G00.15.005", &a.reference);
    }

    // ------------------------------------------------------------ S21 individu
    crate::dsn::bloc_individu(&mut f, s, false)?;
    crate::ntt::poser_ntt(&mut f, &ent_l, s, crate::changements_individu::derniere_declaree(ctx, &paie.salarie, &fin_jour, departs)?.as_ref())?;

    // ------------------------------------------------------------ S21 contrat
    let siens: Vec<&Arret> = arrets.iter().collect();
    // Les emplois multiples (`40.036`) : les autres contrats du salarie dans la periode de paie du signalement.
    let du_mois = crate::dsn::contrats_du_mois(ctx, debut_p.mois_civil(), departs)?;
    let emplois = crate::dsn::emplois_multiples(paie, &du_mois.iter().collect::<Vec<_>>());
    bloc_contrat_fctu(&mut f, paie, &ent_l, &orgs, 1, &bar, true, &siens, NATURE, Some(fin), emplois)?;

    // ------------------------------------------------------------ S21 versement
    // La date de versement : la fin de la periode de paie.
    f.add("S21.G00.50.001", fin_p.jjmmaaaa());
    f.add("S21.G00.50.003", "01");
    f.add("S21.G00.50.020", defaut(&nomenclatures()?.rattachement_versement, "rattachement_versement")?);
    // Un mois decoupe en periodes elementaires : les remunerations et les bases de chacune, datees de ses bornes.
    let bornes = |q: &engagement::calcul::Paie| -> R<(Jour, Jour)> {
        if paie.periodes.is_empty() { Ok((debut_p, fin_p)) } else { Ok((q.engagement.debut_periode(), q.engagement.fin_periode())) }
    };
    for q in paie.elementaires() {
        let (d, f_) = bornes(q)?;
        remunerations(&mut f, q, &d, &f_, 1)?;
    }
    primes_et_indemnites(&mut f, paie, &debut_p, &fin_p, 1)?;
    autre_revenu(&mut f, paie, &bar, &debut_p, &fin_p, 1)?;
    // Les bases de prevoyance ne partent pas dans un FCTU (CCH-14, p. 293).
    for q in paie.elementaires() {
        let (d, f_) = bornes(q)?;
        bases_et_cotisations(&mut f, q, &bar, 1, &d, &f_, 1, NATURE, &BASES_INTERDITES)?;
    }

    // ------------------------------------------------------------ S21 anciennete
    let depuis = *s.date_anciennete.exigee("date_anciennete")?;
    bloc_anciennete(&mut f, paie, fin_jour.moins(&depuis) + 1)?;

    let n = f.lignes.len() + 2;
    f.add("S90.G00.90.001", n);
    f.add("S90.G00.90.002", "1");
    let texte = f.texte();
    crate::usages::verifier_fichier(&texte)?;
    crate::formats::verifier_fichier(&texte)?;
    Ok(texte)
}

/// Les deux controles que la nature 07 ajoute : CCH-14 (la base 31 est
/// interdite en S21.G00.78.001) et CCH-15 (au moins trois blocs 51, de types
/// 001, 002 et 010).
pub fn verifier_controles_07(texte: &str) -> Result<(), Echec> {
    let lignes: Vec<&str> = texte.lines().map(str::trim).collect();
    let valeur = |l: &str| -> String { crate::usages::rubrique_valeur(l).1.to_string() };
    let mut bases = lignes.iter().filter(|l| l.starts_with("S21.G00.78.001,")).map(|l| BaseAssujettie::du_code(&valeur(l)));
    if bases.any(|b| b == Some(BaseAssujettie::PrevoyanceSanteRetraite)) {
        return Err(Echec::impossible(
            "CCH-14 : la valeur « 31 » est interdite en S21.G00.78.001 dans un signalement de fin de contrat. Le \
bareme declare une base assujettie de prevoyance, qui n'a pas sa place ici -- l'identifiant d'affiliation qui \
l'accompagne (78.005) y est interdit lui aussi.",
        ));
    }
    let types: Vec<String> = lignes.iter().filter(|l| l.starts_with("S21.G00.51.011,")).map(|l| valeur(l)).collect();
    let manquants: Vec<&str> = ["001", "002", "010"].into_iter().filter(|t| !types.iter().any(|x| x == t)).collect();
    if !manquants.is_empty() {
        return Err(Echec::impossible(format!(
            "CCH-15 : un signalement de fin de contrat porte au moins trois blocs 51 par versement -- « 001 » \
(remuneration brute non plafonnee), « 002 » (salaire brut soumis a contributions chomage) et « 010 » (salaire de \
base). Manque{} : {}.",
            if manquants.len() > 1 { "nt" } else { "" },
            manquants.join(", ")
        )));
    }
    Ok(())
}
