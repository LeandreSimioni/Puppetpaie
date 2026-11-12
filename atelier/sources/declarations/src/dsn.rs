//! La DSN mensuelle, dans la version de la norme NEODeS que son mois principal declare appelle
//! (`norme::de_la_mensuelle`, `logiciel/donnees/norme-dsn-base.json`).
//!
//! Le fichier est une suite de lignes `RUBRIQUE,'valeur'` separees par CRLF,
//! ecrites en latin-1 par l'appelant. Le moteur refuse plutot que d'ecrire une
//! valeur qu'il ne sait pas.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt::Display;
use std::path::Path;

use rust_decimal::RoundingStrategy;

use droit::assiette::Assiette;
use droit::base_assujettie::BaseAssujettie;
use droit::code_cotisation::{CleDeTotalisation, CodeCotisation81};
use droit::nature_declaration::NatureDeLaDeclaration;
use droit::type_declaration::TypeDeLaDeclaration;
use droit::bordereau::{AssietteDuBordereau, MontantDuBordereau};
use droit::bareme::{self, bareme_applique, EntrepriseLue};
use droit::fiche::{ChampsEntreprise, DeclarantDsn, Lue};
use droit::forme_bareme::{CotisationDsn, CotisationSupplementaire, IdDeCotisation, Pile, Rubrique, PREVOYANCE};
use documents::bordereau::{self, Destinataire, LigneBordereau, TauxDeLaLigne};
use droit::autre_revenu_brut::AutreElementDeRevenuBrut;
use droit::reduction::au_dela_du_seuil;
use droit::contexte::IdSalarie;
use engagement::calcul::Paie;
use engagement::etats::PrelevementDeLaCorrection;
use droit::contexte::{aujourdhui, Contexte};
use droit::natures::{contrat_court, Mesure, MotifDeRupture, Nature, TypeDeRemuneration};
use socle::date::Jour;
use socle::mois::Mois;
use socle::echec::{refus, refus_avec, Lieu, R};
use socle::{num::{add, div, mul, pct, sub, zero2, Decimal}, refus};
use droit::organismes::{CodeCtp, Ctp, IdentifiantDOrganisme, Organismes, Versement};
use droit::pas::{self, Triplet, ZONE_DEFAUT};

pub const LOGICIEL: &str = "PUPPETPAIE";
pub const VERSION: &str = "1.0.0";
pub const SIRET_FRANCE_TRAVAIL: IdentifiantDOrganisme = IdentifiantDOrganisme::du_moteur("13000548100010");

/// Le code de `v` si la liste de la norme `norme` l'a ; sinon le refus `HorsDeLaVersion` (« Une rubrique assortie d'une
/// liste de valeurs ne peut contenir qu'une des valeurs indiquees dans cette liste », cahier 2027.1.1 p. 86,
/// `texte-2027/chapitre-4-controles.txt` l. 164). Le seul chemin par lequel un code d'une liste de la norme devient une
/// valeur de fichier (`Fichier::ecrire_code`, `changements_contrat`).
pub(crate) fn code_admis<T: droit::natures::ListeDeLaNorme>(norme: &crate::norme::Norme, v: T) -> R<&'static str> {
    if norme.a_dans_sa_liste(v)? {
        return Ok(v.code_de_la_norme());
    }
    let bornes = match (v.depuis(), v.jusqu_a()) {
        (Some(d), Some(j)) => format!("il n'y est que de {} à {}", d, j),
        (Some(d), None) => format!("il n'y est qu'à partir de {}", d),
        (None, Some(j)) => format!("il n'y est que jusqu'à {}", j),
        (None, None) => String::new(),
    };
    Err(socle::echec::erreur(
        socle::echec::Classe::HorsDeLaVersion,
        format!(
            "{} « {} » ({}) : la liste de la norme {} ne l'a pas ({}) ; la déclaration serait rejetée. Rien n'est écrit.",
            T::quoi(),
            v.code_de_la_norme(),
            T::RUBRIQUE,
            norme.version,
            bornes
        ),
    ))
}

/// L'accumulateur de lignes DSN, et la version de la norme du fichier qu'il fabrique.
pub struct Fichier {
    pub lignes: Vec<String>,
    norme: Option<crate::norme::Norme>,
    /// L'individu en cours d'ecriture n'a qu'UN bloc « Contrat - S21.G00.40 » dans la declaration : S21.G00.78.006 ne
    /// s'ecrit pas. « Dans le cas de la declaration d'un contrat unique, cette rubrique n'est pas a renseigner. »
    /// (cahier technique 2026.1, p. 298, `atelier/reference/norme-dsn/texte/bloc-S21.G00.79.txt` l. 9-14).
    /// Pose par `generer` individu par individu ; faux ailleurs (la rubrique s'ecrit).
    pub contrat_unique: bool,
}

impl Fichier {
    /// Un morceau de fichier sans version (un bloc qu'on insere, un essai) : il ne decide d'aucune rubrique.
    pub fn nouveau() -> Fichier {
        Fichier { lignes: Vec::new(), norme: None, contrat_unique: false }
    }

    /// Le fichier d'une version de la norme : `S10.G00.00.006` s'ecrit depuis elle (`ecrire_la_version`), et les
    /// rubriques qu'une version a retirees ou ajoutees se decident par elle (`norme()`).
    pub fn en_norme(norme: crate::norme::Norme) -> Fichier {
        Fichier { lignes: Vec::new(), norme: Some(norme), contrat_unique: false }
    }

    /// La version de la norme de ce fichier ; un morceau sans version refuse de decider.
    pub fn norme(&self) -> R<&crate::norme::Norme> {
        self.norme.as_ref().ok_or_else(|| {
            socle::echec::a_la_cle(
                "Une DSN s'écrit sans version de la norme : les rubriques qu'elle admet ne se décident pas. Le signaler au développeur.",
                "S10.G00.00.006",
                "Fichier::en_norme",
            )
        })
    }

    /// `S10.G00.00.006`, le numero de version de la norme utilisee.
    pub fn ecrire_la_version(&mut self) -> R<()> {
        let v = self.norme()?.version.clone();
        self.add(crate::norme::RUBRIQUE, v);
        Ok(())
    }

    /// UN CODE D'UNE LISTE DE LA NORME (`droit::natures::ListeDeLaNorme`), dans sa rubrique : il ne s'ecrit que si la
    /// liste de la version de ce fichier l'a (« Une rubrique assortie d'une liste de valeurs ne peut contenir qu'une des
    /// valeurs indiquees dans cette liste », cahier 2027.1.1 p. 86, `texte-2027/chapitre-4-controles.txt` l. 164).
    pub fn ecrire_code<T: droit::natures::ListeDeLaNorme>(&mut self, v: T) -> R<()> {
        let code = code_admis(self.norme()?, v)?;
        self.add(T::RUBRIQUE, code);
        Ok(())
    }

    pub fn add(&mut self, rubrique: &str, valeur: impl Display) {
        self.lignes.push(format!("{},'{}'", rubrique, valeur));
    }

    /// Une valeur absente ne s'ecrit pas « None » : c'est un refus.
    pub fn add_opt(&mut self, rubrique: &str, valeur: Option<impl Display>) -> R<()> {
        match valeur {
            Some(v) => {
                self.add(rubrique, v);
                Ok(())
            }
            None => Err(absente(rubrique)),
        }
    }

    /// Pose `rubrique` d'une valeur JSON écrite en texte ; refus sur `null`.
    /// Une rubrique dite par un texte, exigee : sans lui, le refus la nomme.
    pub fn add_ecrit(&mut self, rubrique: &str, valeur: Option<String>) -> R<()> {
        self.add(rubrique, valeur.ok_or_else(|| absente(rubrique))?);
        Ok(())
    }

    /// LE COMPLEMENT D'ADRESSE (S10.G00.01.009, S21.G00.06.007, S21.G00.11.006 : « Complement de la localisation de la
    /// construction », facultatif, X [1,50], `atelier/reference/norme-dsn/texte/bloc-S21.G00.06.txt` l. 98-110) : la
    /// seconde ligne de l'adresse, ecrite entiere quand la fiche la dit, absente sinon.
    pub fn add_complement(&mut self, rubrique: &str, case: &Option<droit::fiche::LigneAdresse>) {
        if let Some(t) = case.remplie() {
            self.add(rubrique, t);
        }
    }

    /// La case `cle` d'une fiche, que la rubrique exige : vide, elle refuse en se nommant.
    pub fn add_case<T: AsRef<str>>(&mut self, rubrique: &str, case: &Option<T>, cle: &str) -> R<()> {
        match case {
            None => Err(socle::fichiers::absent(cle)),
            Some(t) => {
                self.add(rubrique, t.as_ref());
                Ok(())
            }
        }
    }

    /// Les rubriques ecrites, `(rubrique, valeur)`, dans l'ordre.
    pub fn rubriques(&self) -> Vec<(String, String)> {
        self.lignes
            .iter()
            .map(|l| match l.split_once(',') {
                Some((k, v)) => (
                    k.to_string(),
                    match v.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
                        Some(entre) => entre.to_string(),
                        None => v.to_string(),
                    },
                ),
                None => (l.clone(), String::new()),
            })
            .collect()
    }

    pub fn texte(&self) -> String {
        let mut t = self.lignes.join("\r\n");
        t.push_str("\r\n");
        t
    }
}

#[track_caller]
fn absente(rubrique: &str) -> socle::echec::Echec {
    refus!(
        "{} : valeur absente. Une rubrique DSN sans valeur ne s'ecrit pas -- et surtout pas avec le mot None.",
        rubrique
    )
}

/// Le bloc `S10.G00.02` (contact émetteur), commun à la mensuelle et aux signalements : UNE personne, celle que les
/// organismes contactent (`declarant_dsn` de la fiche entreprise), civilité, nom, courriel et téléphone. Cahier
/// technique DSN 2026.1, p. 132 (`norme-dsn/texte/bloc-S10.G00.02.txt`, lignes 15-45) : « Acteur à contacter, chez
/// le déclarant » ; `02.001` : « 01 - monsieur / 02 - madame » ; `02.002` : « Nom, prénom de l'agent de l'émetteur
/// pouvant donner des précisions sur cet envoi ». Le nom ne vient plus du signataire (le courriel et le téléphone
/// venaient déjà du déclarant : deux personnes dans le même bloc), et la civilité n'est plus « 01 » en dur : elle
/// se lit dans la fiche, sinon refus.
pub fn contact_emetteur(f: &mut Fichier, ent: &ChampsEntreprise) -> R<()> {
    let decl = ent.declarant_dsn.exigee("declarant_dsn")?;
    let civilite = match decl.civilite.code() {
        Some(c @ ("01" | "02")) => c,
        autre => {
            return Err(socle::echec::cle(format!(
                "La civilité de la personne que les organismes contactent {} : la saisir dans Configuration › Entreprise, « Qui les organismes contactent », case Civilité (Monsieur ou Madame). Elle s'écrit en S10.G00.02.001 (cahier technique DSN, p. 132). Rien n'est écrit.",
                match autre {
                    Some(c) => format!("vaut « {} », ni 01 ni 02", c),
                    None => "manque".to_string(),
                }
            ), "declarant_dsn.civilite"))
        }
    };
    f.add("S10.G00.02.001", civilite);
    if decl.nom_prenom.remplie().is_none() {
        return Err(socle::echec::cle("Le nom de la personne que les organismes contactent manque : le saisir dans Configuration › Entreprise, « Qui les organismes contactent ». Il s'écrit en S10.G00.02.002 et S20.G00.07.001. Rien n'est écrit.", "declarant_dsn.nom_prenom"));
    }
    f.add_case("S10.G00.02.002", &decl.nom_prenom, "nom_prenom")?;
    f.add_case("S10.G00.02.004", &decl.email, "email")?;
    f.add_case("S10.G00.02.005", &decl.tel, "tel")?;
    Ok(())
}

/// Le bloc `S20.G00.07` (contact chez le declare) de la personne que les organismes contactent (`declarant_dsn`), commun
/// a la mensuelle et aux signalements : nom, telephone, courriel, type.
pub fn contact_chez_le_declare(f: &mut Fichier, ent: &ChampsEntreprise) -> R<()> {
    let decl = ent.declarant_dsn.exigee("declarant_dsn")?;
    f.add_case("S20.G00.07.001", &decl.nom_prenom, "nom_prenom")?;
    f.add_case("S20.G00.07.002", &decl.tel, "tel")?;
    f.add_case("S20.G00.07.003", &decl.email, "email")?;
    f.add("S20.G00.07.004", type_du_contact(decl));
    Ok(())
}

/// LE TYPE DU CONTACT CHEZ LE DECLARE (S20.G00.07.004), commun a la mensuelle, aux signalements et a l'amorcage. Le
/// moteur ne declare qu'UN contact (`declarant_dsn`) : celui qui repond aux organismes pour toute la declaration. La
/// norme (cahier technique 2027.1.1, `norme-dsn/texte-2027/bloc-S20.G00.07.txt`, rubrique « Type », les valeurs
/// lignes 67-80) donne la liste ; « 04 - Contact chez le declare recouvrant, entre autres, des cotisations de Securite
/// Sociale (Urssaf CN, MSA) et contributions assurance chomage et cotisations AGS (France Travail) » est celui-la. Les
/// types 14 et 17 (referent handicap des 250 salaries et plus, medecine du travail interne) ne sont pas ce contact-la.
/// Une compagnie neuve n'a donc rien a choisir (la fiche ne montre pas cette case, et « Pour demarrer » ne la
/// demande pas) ; une valeur posee dans la fiche (`declarant_dsn.qualite`) reste lue telle quelle.
pub const TYPE_CONTACT_ORDINAIRE: &str = "04";

/// Le type du contact a ecrire en S20.G00.07.004 : celui de la fiche s'il y en a un, sinon [`TYPE_CONTACT_ORDINAIRE`].
fn type_du_contact(decl: &DeclarantDsn) -> &str {
    match decl.qualite.remplie() {
        Some(q) => q,
        None => TYPE_CONTACT_ORDINAIRE,
    }
}

/// LA MEDECINE DU TRAVAIL INTERNE DE L'ETABLISSEMENT, dans l'identifiant de son service de prevention et de sante au travail
/// (S21.G00.11.025 : « Cette rubrique doit etre renseignee par l'identifiant du service de prevention et de sante au travail
/// (SPST) », `X [1,14]`, conditionnelle en mensuelle ; cahier 2026.1, `norme-dsn/texte/bloc-S21.G00.12.txt` l. 33-37, et
/// 2027.1.1, `texte-2027/bloc-S21.G00.11.txt` l. 232-237). La norme 2027 y nomme les SPST autonomes : « MT01 - MEDECINE
/// INTERNE ETABLISSEMENT » (note differentielle 2026.1.2 -> 2027.1.2, § 13,
/// `norme-dsn/dsn-note-differentielle-ct-2026.1.2-vers-2027.1.2-20251224.txt` l. 3497-3503), et en fait la condition du
/// contact de type 17 (`contact_sante_travail`) ; la norme 2026 n'a ni l'une ni l'autre. La fiche garde le code de la DPAE
/// (`medecine_travail`), ou la medecine interne s'ecrit MT999 (`droit::fiche::CodeMedecine::INTERNE`) : les deux textes nomment la
/// meme medecine interne a l'etablissement, ce qui est une inference.
///
/// Rend `MT01` quand la fiche dit une medecine interne et que la version du fichier est 2027 ou apres ; sinon rien. L'identifiant d'un
/// service EXTERNE ne s'ecrit pas encore : sa definition est « precisee dans la fiche consigne dediee », qui n'est pas
/// deposee, et que le code de la DPAE soit cet identifiant serait une inference (la rubrique est conditionnelle).
fn spst_ecrit(f: &Fichier, ent_e: &ChampsEntreprise) -> R<Option<String>> {
    let interne = ent_e.medecine_travail.dite().is_some_and(droit::fiche::CodeMedecine::interne);
    if !interne || f.norme()?.version.as_str() < "P27V01" || !usage_admet(f, "S21.G00.11.025", NatureDeLaDeclaration::Mensuelle)? {
        return Ok(None);
    }
    Ok(Some("MT01".to_string()))
}

/// LE CONTACT DE LA MEDECINE DU TRAVAIL INTERNE (S20.G00.07 de type 17). Norme 2027 (cahier technique 2027.1.1,
/// `norme-dsn/texte-2027/bloc-S20.G00.07.txt` l. 46-65 et 81, CCH-11 de S20.G00.07.004) : « Si et seulement si la
/// declaration contient un bloc Etablissement - S21.G00.11 avec la rubrique Identifiant du SPST - S21.G00.11.025 renseignee
/// avec la valeur MT01 - MEDECINE INTERNE ETABLISSEMENT [...], alors un bloc Contact chez le declare - S20.G00.07 de type 17
/// [...] doit obligatoirement etre present. Dans le cas contraire, il est interdit. » ; c'est « le medecin du travail ou
/// un interlocuteur place sous sa responsabilite, pour la transmission des arrets de travail ». Il vient de la fiche
/// (`contact_sante_travail`, montre et demande quand le code vaut MT999, `champs-base.json`) ; sans lui, la DSN refuse.
fn contact_sante_travail(f: &mut Fichier, ent_e: &ChampsEntreprise) -> R<()> {
    if spst_ecrit(f, ent_e)?.as_deref() != Some("MT01") {
        return Ok(());
    }
    let c = ent_e.contact_sante_travail.dite();
    let lu = |k: fn(&droit::fiche::ContactSanteTravail) -> &Option<String>| c.and_then(|c| k(c).portee());
    let (nom_prenom, tel, email) = (lu(|c| &c.nom_prenom), lu(|c| &c.tel), c.and_then(|c| c.email.portee()));
    let (Some(nom_prenom), Some(tel), Some(email)) = (nom_prenom, tel, email) else {
        let manque: Vec<&str> = [(nom_prenom, "nom et prénom"), (tel, "téléphone"), (email, "courriel")]
            .into_iter()
            .filter(|(k, _)| k.is_none())
            .map(|(_, l)| l)
            .collect();
        return Err(refus!(
            "La médecine du travail de la compagnie est interne (code MT999) : la DSN déclare qui y répond, le médecin du travail ou une personne sous sa responsabilité (S20.G00.07, type 17). Manque : {}. Le saisir dans Configuration › Entreprise, « Qui répond pour la médecine du travail interne ». Rien n'est écrit.",
            manque.join(", ")
        ));
    };
    f.add("S20.G00.07.001", nom_prenom);
    f.add("S20.G00.07.002", tel);
    f.add("S20.G00.07.003", email);
    f.add("S20.G00.07.004", "17");
    Ok(())
}

/// Un nombre a `n` decimales ecrites, le demi au pair.
fn fmt_f(x: Decimal, n: u32) -> String {
    let mut q = x.round_dp_with_strategy(n, RoundingStrategy::MidpointNearestEven);
    q.rescale(n);
    q.to_string()
}

/// Deux decimales, jamais « -0.00 ».
pub fn mt(x: Decimal) -> String {
    a_deux_decimales(x).to_string()
}

/// Le montant a deux decimales, tel que la DSN l'ecrit (`mt`), jamais -0.00.
pub fn a_deux_decimales(x: Decimal) -> Decimal {
    if x.is_zero() {
        zero2()
    } else {
        let mut q = x.round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven);
        q.rescale(2);
        q
    }
}

/// Un montant signe.
pub fn ms(x: Decimal, signe: i64) -> String {
    mt(mul(x, Decimal::from(signe)))
}

/// A l'euro, la fraction egale a 0,50 comptee pour 1 (CSS L. 133-10) : `bordereau::euro`, comme les lignes 23.
fn rond(x: Decimal) -> Decimal {
    bordereau::euro(x)
}

fn cle_salarie(p: &Paie) -> IdSalarie {
    p.salarie.id.clone()
}

// ------------------------------------------------- les emplois multiples (S21.G00.40.036)

/// L'emploi qu'un contrat declare : ce que le bloc 40 en dit (PCS-ESE `40.004`, code emploi `40.005`, libelle
/// `40.006`), lu sur la fiche telle que l'engagement l'a faite (`emplois::emploi_de`).
fn emploi_declare(p: &Paie) -> (Option<String>, Option<String>, Option<String>) {
    let s = &p.salarie;
    (s.pcs_ese.en_texte(), s.code_emploi_ft.en_texte(), s.emploi.en_texte())
}

/// Les contrats d'une periode de paie : la paie ecrite de chaque engagement valide du mois
/// (`engagement::etats::validees_du_mois`), dans l'ordre de l'ecran ; rien si le mois n'a pas de saisie. Un engagement
/// en saisie n'y est pas : une declaration ne lit que l'engagement valide. Ils completent ceux qu'une declaration
/// porte : la DSN d'un etablissement ne porte pas les contrats d'un autre, un signalement n'en porte qu'un.
pub fn contrats_du_mois(ctx: &Contexte, mois: Mois, departs: &engagement::departs::Departs) -> R<Vec<Paie>> {
    Ok(engagement::etats::validees_du_mois(ctx, mois, departs)?.into_iter().flat_map(|(_, paies)| paies).collect())
}

/// `S21.G00.40.036` (code emplois multiples) du contrat `p` : « 02 - emplois multiples » si le salarie tient plus
/// d'un emploi chez la compagnie dans la periode de paie, « 01 - emploi unique » sinon. Cahier technique DSN 2026.1,
/// p. 199 (`norme-dsn/texte/bloc-S21.G00.40.txt`, lignes 1174-1183) : « Cette rubrique précise si le salarié a
/// plusieurs emplois chez un même employeur au cours d'une même période de paie. » `autres` : les contrats de la
/// meme periode (ceux de la declaration et ceux des engagements valides du mois), de tous les salaries ; seuls ceux de
/// `p` comptent.
/// INFERENCE, ecrite comme telle : un emploi est ce que le bloc 40 decrit (PCS, code, libelle), la norme ne le
/// definit pas autrement ; c'est ce que CoolPaie ecrivait (02 si et seulement si deux codes emploi distincts le meme
/// mois, 148 contrats de son historique, fiche N1 de la relecture).
pub fn emplois_multiples(p: &Paie, autres: &[&Paie]) -> &'static str {
    let cle = cle_salarie(p);
    let mut emplois: BTreeSet<(Option<String>, Option<String>, Option<String>)> = autres.iter().filter(|q| cle_salarie(q) == cle).map(|q| emploi_declare(q)).collect();
    emplois.insert(emploi_declare(p));
    if emplois.len() > 1 {
        "02"
    } else {
        "01"
    }
}

// ------------------------------------------------- l'individu (S21.G00.30)

/// LE BLOC 30 D'UN INDIVIDU, de son NIR a son matricule, tel que sa fiche le dit (`s` : la DSN mensuelle y met les
/// identifiants du mois declare, `changements_individu::identite_du_mois`). La codification UE se lit par
/// `codification_ue` (quatre valeurs, et un ancien 03 se relit) ; le pays de naissance dans la liste livree (`pays.rs`,
/// un code inconnu refuse). L'adresse mel (30.018) est conditionnelle (usage « C », et « conditionne a l'accord
/// explicite » de l'individu : `atelier/reference/norme-dsn/texte/tableaux-des-usages.txt`, `texte/bloc-S21.G00.30.txt`) :
/// elle s'ecrit quand la nature l'admet (`courriel`) et que la fiche la dit.
pub fn bloc_individu(f: &mut Fichier, s: &droit::fiche::ChampsSalarie, courriel: bool) -> R<()> {
    crate::ntt::poser_nir(f, s)?;
    f.add_case("S21.G00.30.002", &s.nom, "nom")?;
    f.add_case("S21.G00.30.004", &s.prenom, "prenom")?;
    f.add("S21.G00.30.006", s.date_naissance.exigee("date_naissance")?.jjmmaaaa());
    f.add_case("S21.G00.30.007", &s.lieu_naissance, "lieu_naissance")?;
    f.add_case("S21.G00.30.008", &s.adresse, "adresse")?;
    f.add_case("S21.G00.30.009", &s.code_postal, "code_postal")?;
    f.add_case("S21.G00.30.010", &s.commune, "commune")?;
    f.add("S21.G00.30.013", socle::codification_ue::codification_ue(&s.nationalite())?);
    f.add_case("S21.G00.30.014", &s.departement_naissance, "departement_naissance")?;
    f.add_case("S21.G00.30.015", &s.pays_de_naissance()?, "pays_naissance")?;
    if let Some(email) = s.email.portee().filter(|_| courriel) {
        f.add("S21.G00.30.018", email);
    }
    f.add_case("S21.G00.30.019", &s.code_salarie, "code_salarie")
}


// ----------------------------------------------------- les entrees

/// Un arret de travail a reporter, tel que le magasin des imprevus le garde (`engagement::imprevus::ArretSaisi`).
#[derive(Clone, Debug)]
pub struct Arret {
    pub saisi: engagement::imprevus::ArretSaisi,
}

impl Arret {
    /// L'arret en sa forme, pour les regles du magasin.
    pub fn forme(&self) -> &engagement::imprevus::ArretSaisi {
        &self.saisi
    }

    /// Une periode commune avec [debut, fin] : la regle du bloc 60 (`imprevus::arret_croise`).
    pub fn couvre(&self, debut: &Jour, fin: &Jour) -> R<bool> {
        engagement::imprevus::arret_croise(&self.saisi, debut, fin)
    }

    /// L'arret se reporte-t-il dans la DSN mensuelle de `mois` ? Celle de chaque mois qu'il croise ; et, s'il a fini avant
    /// `mois` et n'a ete saisi qu'apres le depart de la derniere DSN mensuelle d'un mois anterieur qui declarait son
    /// contrat (aucune ne le portait), la DSN de `mois` quand elle part apres sa saisie. Cahier technique 2026.1, § 2.5.2
    /// (`atelier/reference/norme-dsn/texte/chapitre-2-principes-de-constitution.txt`) : « il est nécessaire de reporter les
    /// informations d'un signalement dans la DSN mensuelle consécutive à l'émission du signalement afin que l'évènement
    /// signalé soit pris en compte dans le traitement d'un évènement futur » ; § 2.5 : l'arret est reporte dans la DSN
    /// mensuelle même si les impacts de l'absence sont portés dans une paie ultérieure. Un arret sans date de saisie, ou
    /// dont le contrat n'a rien declare de reel, ne se reporte que dans les mois qu'il croise.
    pub fn a_reporter(&self, mois: Mois, departs: &engagement::departs::Departs) -> R<bool> {
        let debut_mois = mois.premier_jour();
        if self.couvre(&debut_mois, &mois.dernier_jour())? {
            return Ok(true);
        }
        let origine = Jour::nouveau(1900, 1, 1).ok_or_else(|| refus("jour inexistant"))?;
        if !self.couvre(&origine, &debut_mois.plus_jours(-1)?)? {
            return Ok(false);
        }
        // Un contrat sans numero n'a rien declare.
        Ok(self.saisi.contrat_numero.as_ref().is_some_and(|numero| self.saisi_apres_la_derniere_dsn(numero, mois, departs)))
    }

    /// L'arret a-t-il ete saisi apres le depart de la derniere DSN mensuelle d'un mois anterieur a `mois` qui declarait le
    /// contrat `numero`, et avant celui de la DSN de `mois` ?
    fn saisi_apres_la_derniere_dsn(&self, numero: &engagement::numeros::NumeroContrat, mois: Mois, departs: &engagement::departs::Departs) -> bool {
        // Une date du journal (« 2026-10-15T10:00:00 ») et une date de saisie (« 2026-11-03 11:39 ») a la minute.
        let minute = |t: &str| t.replacen('T', " ", 1).chars().take(16).collect::<String>();
        let saisi = socle::paris::ecrit::saisie::texte(self.saisi.saisi_le);
        let mut avant: Option<String> = None;
        for d in departs.de(&self.saisi.salarie, numero).iter().filter(|d| d.envoi == engagement::departs::Envoi::Dsn) {
            // Le mois que la DSN declarait : celui qui la porte (une correction), sinon le sien.
            let Some(declarante) = d.par.or(d.mois) else { continue };
            let le = minute(&d.le.iso());
            if declarante == mois && le < saisi {
                return false;
            }
            if declarante < mois && avant.as_ref().is_none_or(|a| le > *a) {
                avant = Some(le);
            }
        }
        avant.is_some_and(|a| a < saisi)
    }

    /// L'arret suspend-il le contrat de cette paie ? La regle des imprevus (`imprevus::du_contrat`) : le meme salarie, la
    /// meme date de debut du contrat, le meme numero quand l'un et l'autre en ont un.
    fn du_contrat(&self, p: &Paie) -> R<bool> {
        engagement::imprevus::du_contrat(&self.saisi, &cle_salarie(p), p.engagement.numero.as_deref(), &p.engagement.debut)
    }
}

/// Ceux des arrets qui suspendent le contrat de cette paie (`Arret::du_contrat`).
fn arrets_du_contrat<'a>(arrets: &[&'a Arret], p: &Paie) -> R<Vec<&'a Arret>> {
    let mut out = Vec::new();
    for a in arrets {
        if a.du_contrat(p)? {
            out.push(*a);
        }
    }
    Ok(out)
}

/// Le bloc « Arret de travail - S21.G00.60 » (`signalement.bloc_60`).
///
/// Ce que chaque nature admet du bloc se lit dans le tableau des usages (cahier technique 2026.1, p. 363-382,
/// `atelier/reference/norme-dsn/texte/tableaux-des-usages.txt`, bloc S21.G00.60) : la subrogation (60.004 a 60.008) est
/// C en 01, O ou C en 04, I en 05 et en 07 ; la date de l'accident (60.012), I en 01 et en 07. `nature` est celle du
/// fichier, jamais une autre.
pub fn bloc_60(f: &mut Fichier, a: &Arret, nature: NatureDeLaDeclaration, banque: Option<&droit::fiche::Banque>) -> R<()> {
    let v = &a.saisi;
    f.add("S21.G00.60.001", &v.motif);
    // La veille du debut prescrit, ou ce jour s'il a ete travaille (S21.G00.60.002, `imprevus::dernier_jour_declare`).
    f.add("S21.G00.60.002", engagement::imprevus::dernier_jour_declare(v)?.jjmmaaaa());
    f.add("S21.G00.60.003", v.fin_previsionnelle.jjmmaaaa());
    let subrogation = if usage_admet(f, "S21.G00.60.004", nature)? { v.subrogation } else { None };
    if let Some(subrogation) = subrogation {
        f.add("S21.G00.60.004", subrogation);
        if crate::signalement::avec_subrogation(a)? {
            let Some((iban, bic)) = banque.and_then(|b| b.iban.remplie().zip(b.bic.remplie())) else {
                return Err(refus("Subrogation demandee : l'IBAN et le BIC de la compagnie (S21.G00.60.007 et .008) sont \
obligatoires -- CCH-11. Ils se lisent dans la fiche entreprise, ils ne se saisissent pas dans l'arret."));
            };
            f.add("S21.G00.60.005", v.subrogation_debut.ok_or_else(|| socle::fichiers::absent("subrogation_debut"))?.jjmmaaaa());
            f.add("S21.G00.60.006", v.subrogation_fin.ok_or_else(|| socle::fichiers::absent("subrogation_fin"))?.jjmmaaaa());
            f.add("S21.G00.60.007", iban);
            f.add("S21.G00.60.008", bic);
        }
    }
    if let Some(reprise) = v.reprise {
        f.add("S21.G00.60.010", reprise.jjmmaaaa());
        if let Some(motif) = v.reprise_motif {
            f.add("S21.G00.60.011", motif);
        }
    }
    if usage_admet(f, "S21.G00.60.012", nature)? {
        if let Some(accident) = v.date_accident {
            f.add("S21.G00.60.012", accident.jjmmaaaa());
        }
    }
    Ok(())
}

/// Une correction d'un mois depose : ce qui a ete declare, et la paie corrigee.
///
/// `declare` est le dernier etat DECLARE du contrat, ecrit depuis la paie que la DSN a declaree (`versement_declare::de_la_paie`) : les blocs negatifs
/// (50.002, 50.004, 51, 53, 54, 58, 78, 79, 81) l'annulent tel qu'il est parti, et l'agrege (bordereau Urssaf du mois
/// d'origine, versements 20) en retranche ce que ses lignes 78 et 81 recomposent (`assiette_declaree`,
/// `cumuler_le_declare`) ; le bloc 55 se lit dans les blocs negatifs ecrits (`composants_de_versement`). `avant` est
/// la paie d'avant refaite par le moteur du jour : elle ne donne plus que le contrat (ses bornes, sa nature, son statut,
/// sa pile de bareme), aucun montant.
pub struct Regularisation {
    pub mois: Mois,
    pub avant: Paie,
    pub declare: crate::versement_declare::VersementDeclare,
    pub apres: Paie,
    pub taux_pas_origine: Option<String>,
    /// Le net a payer avant impot deja verse au salarie pour ce contrat, lu dans la derniere paie declaree
    /// (`produire_regularisations::versement_declare_de`) : ce qui dit si la correction verse ou reprend quelque chose,
    /// et a quel montant net social correspondait ce qui a ete paye (`blocs_58_de_la_correction`).
    pub net_deja_verse: Decimal,
    /// Le jour ou la correction a ete ecrite (`decidee_le`) : la date de la modification (S21.G00.41.001) quand elle
    /// change la date de debut du contrat (`changements_contrat::ancienne_date_de_debut`). `None` : inconnu.
    pub decidee_le: Option<Jour>,
}

/// Les parametres de `generer`, avec leurs defauts.
pub struct Options {
    pub date_constitution: Option<Jour>,
    pub fraction: String,
    pub type_declaration: TypeDeLaDeclaration,
    pub envoi_test: bool,
    pub ordre: i64,
    pub remplace: Option<String>,
    pub etablissement: Option<String>,
    /// Sur un annule et remplace (type 03 ou 05) : la declaration remplacee portait-elle un versement DGFiP
    /// (S21.G00.20.002 = DGFIP_PAS) ? `None` : on ne sait pas, et la DSN refuse s'il n'y a rien a reverser ce mois-ci.
    pub remplacee_versait_pas: Option<bool>,
}

impl Options {
    /// Une premiere DSN mensuelle normale (fraction 11), en reel, constituee le jour de son depot.
    pub fn normale() -> Options {
        Options {
            date_constitution: None,
            fraction: "11".into(),
            type_declaration: TypeDeLaDeclaration::Normale,
            envoi_test: false,
            ordre: 1,
            remplace: None,
            etablissement: None,
            remplacee_versait_pas: None,
        }
    }
}

// ------------------------------------------------------- les cotisations

/// Une cotisation d'un versement (`dsn.cotisations`).
#[derive(Clone, Debug)]
pub struct Cotisation {
    pub bloc78: BaseAssujettie,
    pub base78: Decimal,
    /// Le code de cotisation (81.001) ; `None` : une base du bloc 78 sans cotisation (`cotisations_dsn` du bareme), qui
    /// ne porte pas de montant.
    pub code81: Option<CodeCotisation81>,
    pub org: Option<IdentifiantDOrganisme>,
    pub affectation: Option<String>,
    pub cle_total: Option<CleDeTotalisation>,
    pub code_insee: Option<droit::fiche::CodeInsee>,
    pub base81: Option<Decimal>,
    /// Le taux de la ligne (81.007), tel que le bareme ou le bulletin l'ecrit.
    pub taux: Option<Decimal>,
    pub montant: Option<Decimal>,
    /// Le composant d'une base 31 (S21.G00.79.001, et son montant 79.004) quand la source du taux de la ligne le dit
    /// (`composant_79`, `bareme::bareme_sante`) ; `None` : la tranche A, sur le brut.
    pub composant79: Option<(String, Decimal)>,
    /// La part patronale de la CET (`parts_patronales_agirc_arrco`) : son code se lit dans la version de la norme du
    /// fichier (`Norme::cotisation_cet_patronale`), pas dans `code81`.
    pub cet: bool,
}

/// Le composant d'une base 31 quand la source du taux n'en dit pas (`Cotisation::composant79`) : « 11 - Tranche A »
/// (S21.G00.79.001). La prevoyance et la sante des intermittents : « La base de calcul de ces cotisations est le salaire
/// brut dans la limite de la tranche A journalière » (Parametres Audiens 2026, § 3,
/// `atelier/reference/sources/audiens-parametres-2026/Audiens-Parametres-2026.txt` l. 197).
const TRANCHE_A: &str = "11";

/// LA SECTION DES ORGANISMES (`organismes-base.json`, `par_nature`) ET DES CONTRIBUTIONS D'UNE NATURE DE CONTRAT : celle
/// que `contrats-base.json` dit pour elle (`Nature::section_des_organismes`).
fn section_de(nat: &Nature) -> &str {
    nat.section_des_organismes()
}

/// UNE CONTRIBUTION DECLAREE SOUS UNE ADHESION AUDIENS ce mois-la (la CASC-SVP, une contribution de branche :
/// `contributions_dsn`) : sa reference d'adhesion, son libelle, les lignes du bulletin qu'elle somme, et l'affectation
/// (`202 + place`) de sa base 31 pour chaque section des organismes qui l'a.
#[derive(Clone, Debug)]
pub struct ContributionDuMois {
    pub reference: String,
    pub libelle: String,
    pub lignes: Vec<IdDeCotisation>,
    pub natures: std::collections::BTreeMap<String, String>,
}

/// Les assiettes que le calcul a ecrites dans la paie, telles quelles : rien ne se recalcule.
pub fn bases(p: &Paie, bar: &Pile) -> R<HashMap<Assiette, Decimal>> {
    let b = &p.bases;
    let mut out = HashMap::new();
    let lue = |a: Assiette| -> R<Decimal> { b.get(&a).copied().ok_or_else(|| socle::echec::cle(socle::echec::PAIE_ILLISIBLE, a.nom())) };
    out.insert(Assiette::Brut, p.brut);
    out.insert(Assiette::BrutCs, lue(Assiette::BrutCs)?);
    out.insert(Assiette::Csg, p.base_csg);
    out.insert(Assiette::BrutX115, lue(Assiette::BrutX115)?);
    out.insert(Assiette::BrutX110, lue(Assiette::BrutX110)?);
    out.insert(Assiette::Zero, zero2());
    // La tranche 1 journaliere d'un intermittent (`calcul::calculer`, 7.6), quand la pile a un plafond journalier.
    if bar.plafond_journalier.is_some() && !p.engagement.nat()?.mensuelle() {
        out.insert(Assiette::T1Journaliere, lue(Assiette::T1Journaliere)?);
    }
    // Les assiettes plafonnees d'un intermittent (`calcul::calculer`, 7.6) : les bases 02 et 07 portent ce que le
    // bulletin a calcule, sur le meme plafond (point 22).
    for a in [
        Assiette::T1Mensuelle, Assiette::T2Mensuelle, Assiette::ApecMensuelle, Assiette::CetMensuelle, Assiette::PssPeriode,
        Assiette::T2Conventionnelle, Assiette::BrutPlafonne, Assiette::BrutX115Plafonne, Assiette::BrutChomage,
        Assiette::T1MensuelleX115,
    ] {
        if let Some(x) = b.get(&a) {
            out.insert(a, *x);
        }
    }
    // Les assiettes abattues d'une paie sous deduction forfaitaire specifique (`calcul::calculer`, 7.5 bis), que
    // `cle_abattue` fait lire aux bases de securite sociale et d'Agirc-Arrco.
    if p.deduction_forfaitaire.is_some() {
        for a in [Assiette::BrutDfs, Assiette::BrutX115Dfs, Assiette::BrutX110Dfs, Assiette::BrutPlafonneDfs, Assiette::BrutX115PlafonneDfs] {
            if let Some(x) = b.get(&a) {
                out.insert(a, *x);
            }
        }
    }
    Ok(out)
}

fn base(bases: &HashMap<Assiette, Decimal>, a: Assiette) -> R<Decimal> {
    bases.get(&a).copied().ok_or_else(|| socle::fichiers::absent(a.nom()))
}

/// La cle de `bases` que lit une base 78 de code `bloc78`, ou une cotisation qu'elle porte, pour cette paie : sous
/// deduction forfaitaire specifique et sous une base qu'elle abat (`BaseAssujettie::abattue_par_la_dfs`), la forme
/// abattue que le bulletin a calculee (`brut_dfs`...). Une cle sans forme abattue refuse : la DSN ne se fait pas sur une
/// assiette que le bulletin n'a pas.
pub fn cle_abattue(p: &Paie, bloc78: BaseAssujettie, a: Assiette) -> R<Assiette> {
    if p.deduction_forfaitaire.is_none() || !bloc78.abattue_par_la_dfs() || a == Assiette::Zero {
        return Ok(a);
    }
    if let Some(abattue) = a.abattue().filter(|ab| p.bases.contains_key(ab)) {
        return Ok(abattue);
    }
    Err(socle::echec::cle(format!(
        "{} : la deduction forfaitaire specifique abat la base {}, mais le calcul n'a pas sa forme abattue \
(aucun texte déposé ne la dit). Rien n'est écrit.",
        qui_contrat(p),
        bloc78
    ), a.nom()))
}

/// Le brut que la paie declare en 51 de type 001 : sous deduction forfaitaire specifique, le brut abattu. Guide Urssaf
/// « Comment declarer et regulariser les cotisations Urssaf en DSN » du 04/06/2026, p. 36
/// (`atelier/reference/sources/editeur-dsn/guide-urssaf-regularisation.txt`, l. 1440-1459) : les revenus bruts du montant
/// net social se recomposent par « 51 de type 001 + 54 de type 10 - 54 de type 35 » ; le BOSS (montant net social, Q 26,
/// `atelier/reference/sources/montant-net-social/`, l. 387-393) veut le brut entier, « independamment des abattements » :
/// le 51 de type 001 est donc le brut moins la deduction que porte le 54 de type 10 (`autre_revenu`).
pub fn brut_du_51(p: &Paie) -> Decimal {
    match (&p.deduction_forfaitaire, base_de_la_paie(p, Assiette::BrutDfs)) {
        (Some(_), Some(abattu)) => abattu,
        _ => p.brut,
    }
}

/// L'assiette `a` que le calcul a ecrite dans la paie (`Paie::bases`), `None` s'il ne l'a pas ecrite.
fn base_de_la_paie(p: &Paie, a: Assiette) -> Option<Decimal> {
    p.bases.get(&a).copied()
}

fn somme_lignes(p: &Paie, ids: &[IdDeCotisation], patronale_seule: bool) -> Decimal {
    p.lignes
        .iter()
        .filter(|l| ids.iter().any(|id| l.est(id)))
        .fold(zero2(), |s, l| add(s, if patronale_seule { l.montant_pat } else { add(l.montant_pat, l.montant_sal) }))
}

/// LE BAREME QUE LA DSN LIT POUR UN MOIS ET UN ETABLISSEMENT (`bar_du`), et la pile de chaque paie qu'il a deja empilee : elle lit
/// la pile d'une paie a chaque bloc qui la declare (versement, bases, cotisations, bordereau), et elle ne depend que de
/// ce bareme et de la paie (`bareme_de`).
pub struct BaremeDuMois {
    bar: Pile,
    /// Les contributions de branche du mois, posees par la DSN (`contributions_dsn`).
    pub contributions_du_mois: Vec<ContributionDuMois>,
    /// La compagnie telle que ce bareme la lit (`Contexte::entreprise_lue`, au meme jour).
    pub entreprise: EntrepriseLue,
    piles: std::sync::Mutex<HashMap<String, std::sync::Arc<Pile>>>,
}

impl BaremeDuMois {
    pub fn nouveau(bar: Pile, entreprise: EntrepriseLue) -> BaremeDuMois {
        BaremeDuMois { bar, contributions_du_mois: Vec::new(), entreprise, piles: Default::default() }
    }
}

impl std::ops::Deref for BaremeDuMois {
    type Target = Pile;
    fn deref(&self) -> &Pile {
        &self.bar
    }
}

/// Ce que la section `dsn` de la reduction generale de la pile dit de ses CTP, `None` : elle n'en a pas.
fn reduction_dsn(pile: &Pile) -> Option<&droit::forme_bareme::ReductionDsn> {
    pile.reduction_generale.as_ref().and_then(|r| r.dsn.as_ref())
}

/// Ce que les lignes 81 que le moteur compose a l'Urssaf ou a France Travail declarent, tel qu'un refus les nomme.
const PART_URSSAF: &str = "la part Urssaf de la reduction generale";
const PART_CHOMAGE: &str = "la part chomage de la reduction generale";
const HEURES_EN_PLUS: &str = "la reduction et la deduction des heures en plus (114, 021), a l'Urssaf";

/// L'ORGANISME DE LA PREMIERE COTISATION `code` DE LA PILE, chez qui `pour` (une ligne 81 que le moteur compose) se
/// declare. S21.G00.81.002, « Urssaf : SIRET de l'Urssaf », « France Travail : SIRET de France Travail (uniquement pour
/// les employeurs [...] d'artistes du spectacle [...]) » (`atelier/reference/norme-dsn/texte/bloc-S21.G00.81.txt`
/// l. 367-389) : une ligne recouvree par l'un ou l'autre le nomme. Sans lui, refus : la ligne s'ecrirait sans organisme.
fn org_de(pile: &Pile, code: CodeCotisation81, pour: &str) -> R<IdentifiantDOrganisme> {
    pile.cotisations_dsn.iter().find(|c| c.code81 == Some(code)).and_then(|c| c.org.clone()).ok_or_else(|| {
        socle::echec::a_la_cle(
            "Le barème de l'année ne nomme pas l'organisme d'une cotisation que la DSN déclare : le signaler au développeur.",
            "cotisations_dsn.org",
            format!(
                "bareme {} : aucune cotisation {} (cotisations_dsn) ne nomme son organisme (S21.G00.81.002), chez qui se declare {}. Rien n'est ecrit.",
                pile.annee, code, pour
            ),
        )
    })
}

/// La pile de bareme de cette paie, sur le bareme du mois choisi.
fn bareme_de(p: &Paie, bar: &BaremeDuMois) -> R<std::sync::Arc<Pile>> {
    bareme_du_contrat(&p.engagement, &p.salarie, bar)
}

/// La pile de bareme du contrat `e` de `salarie` (une paie calculee ou importee), sur le bareme du mois choisi.
fn bareme_du_contrat(e: &engagement::Engagement, salarie: &droit::fiche::SalarieDuContrat, bar: &BaremeDuMois) -> R<std::sync::Arc<Pile>> {
    let nat = e.nat()?;
    let fin = e.fin_periode();
    let cpf_cdd = droit::cpf_cdd::ouvre_cpf_cdd(e.nature, e.motif_recours, &droit::cpf_cdd::le_contrat(e.numero.as_deref(), &e.salarie))?;
    let caisse = droit::caisse_conges::affilie(&nat, salarie, &e.debut, e.fin.as_ref())?;
    let majoration = droit::majoration_chomage::due(&nat, e.motif_recours, &e.debut, e.fin.as_ref());
    let cle = format!("{}|{}|{:?}|{}|{}|{}|{}", fin.iso(), nat.cle, salarie, nat.mensuelle(), caisse, cpf_cdd, majoration);
    if let Some(pile) = socle::memoire::tenu(&bar.piles).get(&cle) {
        return Ok(pile.clone());
    }
    let pile = std::sync::Arc::new(bareme_applique(bar, &bar.entreprise, salarie, &nat, Some(&fin), caisse, cpf_cdd, majoration)?);
    socle::memoire::tenu(&bar.piles).insert(cle, pile.clone());
    Ok(pile)
}

/// LA BASE `bloc78` DE LA PAIE `p` (S21.G00.78.004 sous ce code), telle que la paie la porte : la base du calcul que le
/// bareme de cette paie declare sous ce code (`cotisations_dsn`, `base78` ; abattue sous deduction forfaitaire
/// specifique : `cle_abattue`), lue dans ses bases (`Paie::bases`), rien de recalcule (une paie importee :
/// `base_declaree_importee`). Zero quand le bareme ne declare rien sous ce code.
pub fn base_declaree(p: &Paie, bar: &BaremeDuMois, bloc78: BaseAssujettie) -> R<Decimal> {
    let bar = bareme_de(p, bar)?;
    let Some(cle) = bar.cotisations_dsn.iter().filter(|c| c.bloc78 == bloc78).map(|c| c.base78).find(|k| *k != Assiette::Zero) else {
        return Ok(zero2());
    };
    Ok(match cle_abattue(p, bloc78, cle)? {
        Assiette::Brut => p.brut,
        Assiette::Csg => p.base_csg,
        k => p.bases.get(&k).copied().ok_or_else(|| socle::echec::cle(socle::echec::PAIE_ILLISIBLE, k.nom()))?,
    })
}

/// LA BASE `bloc78` D'UNE PAIE IMPORTEE, telle que ses pieces la disent, sous la cle que le bareme de son contrat declare
/// sous ce code (comme `base_declaree`) ; `None` : les pieces ne la disent pas. Zero quand le bareme ne declare rien sous
/// ce code. Une paie importee n'a pas de deduction forfaitaire specifique : rien ne s'y abat.
pub fn base_declaree_importee(p: &engagement::importe::PaieImportee, bar: &BaremeDuMois, bloc78: BaseAssujettie) -> R<Option<Decimal>> {
    let bar = bareme_du_contrat(&p.engagement, &p.salarie, bar)?;
    let Some(cle) = bar.cotisations_dsn.iter().filter(|c| c.bloc78 == bloc78).map(|c| c.base78).find(|k| *k != Assiette::Zero) else {
        return Ok(Some(zero2()));
    };
    Ok(match cle {
        Assiette::Brut => p.brut,
        k => p.bases.get(&k).copied(),
    })
}

/// Les cotisations d'un versement, dans l'ordre du bareme.
pub fn cotisations(p: &Paie, bar: &BaremeDuMois) -> R<Vec<Cotisation>> {
    let contribs = bar.contributions_du_mois.clone();
    let exclues = places_non_affiliees(p, &bar.entreprise.fiche)?;
    let bar = bareme_de(p, bar)?;
    let bases = bases(p, &bar)?;
    let mut out: Vec<Cotisation> = Vec::new();
    for c in &bar.cotisations_dsn {
        if c.seuil_smic.map(|n| au_dela_du_seuil(bar.reduction_generale.as_ref(), p.brut, p.heures_travaillees, n.0)).transpose()? == Some(false) {
            continue;
        }
        let bloc78 = c.bloc78;
        let abattue = |a: Assiette| -> R<Decimal> { base(&bases, cle_abattue(p, bloc78, a)?) };
        let mut l = Cotisation {
            bloc78,
            base78: abattue(c.base78)?,
            code81: c.code81,
            org: c.org.clone(),
            affectation: c.affectation.clone(),
            cle_total: c.cle_total,
            code_insee: c.code_insee.clone(),
            base81: None,
            taux: None,
            montant: None,
            composant79: None,
            cet: false,
        };
        if let Some(lignes) = c.lignes.as_ref().filter(|l| !l.is_empty()) {
            l.montant = Some(somme_lignes(p, lignes, false));
            l.composant79 = composant_79(p, &bar, lignes);
        } else if let Some(taux) = c.taux.map(|t| t.0) {
            let b = abattue(c.base.ok_or_else(|| socle::fichiers::absent("base"))?)?;
            l.base81 = Some(b);
            l.taux = Some(taux);
            l.montant = Some(match c.composants.as_ref().filter(|c| !c.is_empty()) {
                Some(composants) => composants.iter().fold(zero2(), |s, t| add(s, pct(b, t.0))),
                None => pct(b, taux),
            });
        }
        // Une cotisation qui porte un montant se declare sous son code (81.001) : le bareme le dit.
        if l.montant.is_some() && l.code81.is_none() {
            return Err(socle::echec::livree("baremes", "cotisations_dsn.code81"));
        }
        let unifie = l.code81 == Some(AGIRC_ARRCO_UNIFIE);
        out.push(l);
        if unifie {
            let l131 = out[out.len() - 1].clone();
            out.extend(parts_patronales_agirc_arrco(p, &bar, c, &l131)?);
        }
    }
    let nat = p.engagement.nat()?;
    let nature = section_de(&nat);
    for contrib in &contribs {
        let Some(affectation) = contrib.natures.get(nature) else { continue };
        // Apres la derniere cotisation affectee a une adhesion ; sans elle, a la fin.
        let apres = match out.iter().rposition(|l| l.affectation.as_deref().is_some_and(|a| !a.is_empty())) {
            Some(i) => i + 1,
            None => out.len(),
        };
        out.insert(
            apres,
            Cotisation {
                bloc78: BaseAssujettie::PrevoyanceSanteRetraite,
                base78: base(&bases, Assiette::Zero)?,
                code81: Some(CodeCotisation81::PrevoyanceAssuranceMutuelle),
                org: None,
                affectation: Some(affectation.clone()),
                cle_total: None,
                code_insee: None,
                base81: None,
                taux: None,
                montant: Some(somme_lignes(p, &contrib.lignes, false)),
                composant79: None,
                cet: false,
            },
        );
    }
    if let Some(rg) = &p.reduction_generale {
        if !rg.montant.is_zero() {
            let dsn = bar.dsn_de_la_reduction()?;
            let urssaf = Some(org_de(&bar, CodeCotisation81::AssuranceMaladie, PART_URSSAF)?);
            let ft = Some(org_de(&bar, CodeCotisation81::AssuranceChomage, PART_CHOMAGE)?);
            // Sous la base 03 : abattue sous deduction forfaitaire specifique (`cle_abattue`), comme la reduction
            // elle-meme se calcule sur le brut abattu (`droit::reduction::reduction_generale_sous_dfs`).
            let brut_03 = bases[&cle_abattue(p, BaseAssujettie::BruteDeplafonnee, Assiette::Brut)?];
            for (code, org, part, cle) in [
                (dsn.code81_urssaf, urssaf, rg.urssaf, dsn.cle_total_urssaf),
                (dsn.code81_chomage, ft, rg.chomage, dsn.cle_total_chomage),
                // La part retraite n'a pas de cle au bareme : elle se totalise sous son code. Sans organisme :
                // S21.G00.81.002, « AGIRC-ARRCO : non concerné » (`bloc-S21.G00.81.txt` l. 372).
                (dsn.code81_retraite, None, rg.agirc_arrco, dsn.code81_retraite.into()),
            ] {
                if !part.is_zero() {
                    out.push(Cotisation {
                        bloc78: BaseAssujettie::BruteDeplafonnee,
                        base78: brut_03,
                        code81: Some(code),
                        org,
                        affectation: None,
                        cle_total: Some(cle),
                        code_insee: None,
                        base81: Some(brut_03),
                        taux: None,
                        montant: Some(-part),
                        composant79: None,
                        cet: false,
                    });
                }
            }
        }
    }
    // LES HEURES EN PLUS (`engagement::heures_en_plus`) : « 114 - Montant de réduction des heures supplémentaires/
    // complémentaires » et « 021 - Déduction patronale au titre des heures supplémentaires », sous la base 03 (fiche
    // net-entreprises 1265, tableau des combinaisons P26V01, feuille URSSAF CN, `net-entreprises-fiche1265-combinaisons-78-
    // 79-81-p26v01-20250716.txt`, l. 48 et 77), a l'Urssaf, signees '-' comme la reduction generale (guide Urssaf du
    // 04/06/2026, `editeur-dsn/guide-urssaf-regularisation.txt`, l. 1353 : « Au niveau individuel, le montant de reduction
    // doit etre signe »). 81.003 : la remuneration des heures.
    if let Some(remuneration) = p.bases.get(&Assiette::HeuresEnPlus).copied().filter(|m| !m.is_zero()) {
        let urssaf = Some(org_de(&bar, CodeCotisation81::AssuranceMaladie, HEURES_EN_PLUS)?);
        let brut_03 = bases[&cle_abattue(p, BaseAssujettie::BruteDeplafonnee, Assiette::Brut)?];
        for (code, id) in [(CODE81_REDUCTION_HEURES, engagement::heures_en_plus::REDUCTION), (CODE81_DEDUCTION_HEURES, engagement::heures_en_plus::DEDUCTION)] {
            let montant = p.lignes.iter().filter(|l| l.est(&id)).fold(zero2(), |a, l| add(a, add(l.montant_pat, l.montant_sal)));
            if montant.is_zero() {
                continue;
            }
            out.push(Cotisation {
                bloc78: BaseAssujettie::BruteDeplafonnee,
                base78: brut_03,
                code81: Some(code),
                org: urssaf.clone(),
                affectation: None,
                cle_total: Some(code.into()),
                code_insee: None,
                base81: Some(remuneration),
                taux: None,
                montant: Some(montant),
                composant79: None,
                cet: false,
            });
        }
    }
    // La base 31 d'une affiliation que le salarie n'a pas ce mois-la (`rattachements`) ne se declare pas : elle citerait
    // (78.005) un bloc 70 absent. Sa cotisation est nulle -- sinon la paie et l'affiliation se contredisent.
    let mut gardees = Vec::with_capacity(out.len());
    for c in out {
        // L'affectation d'une base 31 (78.005) : « 202 » et suivants, la place de l'adhesion.
        let place = match c.affectation.as_deref().map(str::trim).filter(|a| !a.is_empty()) {
            Some(a) => Some(a.parse::<usize>().map_err(|_| refus!("{} : l'affectation {} d'une base {} ne se lit pas.", qui_contrat(p), socle::fichiers::repr(a), c.bloc78))?),
            None => None,
        }
        .and_then(|a| a.checked_sub(202));
        if c.bloc78 == BaseAssujettie::PrevoyanceSanteRetraite && place.is_some_and(|x| exclues.contains(&x)) {
            if let Some(m) = c.montant.filter(|m| !m.is_zero()) {
                return Err(refus!(
                    "{} : une cotisation {} de {} sous une affiliation prevoyance que le salarie n'a pas ce mois-la \
(S21.G00.70). La paie et l'affiliation se contredisent ; rien n'est declare.",
                    qui_contrat(p),
                    code_declare(&c)?,
                    mt(m)
                ));
            }
            continue;
        }
        gardees.push(c);
    }
    Ok(regrouper_par_base(gardees))
}

/// Le composant de la base 31 des `lignes` : le type que la source du taux de la premiere qui en porte un lui donne
/// (`composant_79` de la pile, `bareme::bareme_sante`), et la base de cette ligne au bulletin.
fn composant_79(p: &Paie, pile: &Pile, lignes: &[IdDeCotisation]) -> Option<(String, Decimal)> {
    lignes.iter().find_map(|id| {
        let type_79 = pile.rubrique(id)?.composant_79.clone()?;
        let base = p.lignes.iter().find(|l| l.est(id))?.base;
        Some((type_79, base))
    })
}

/// « 131 - Cotisation regime unifie Agirc-Arrco » : modalite Agirc-Arrco, parts salariale et patronale confondues
/// (cahier technique 2026.1, p. 302 lignes 80-81 et p. 305 ligne 287, `atelier/reference/norme-dsn/texte/bloc-S21.G00.81.txt`).
pub const AGIRC_ARRCO_UNIFIE: CodeCotisation81 = CodeCotisation81::AgircArrco;
/// « 142 - Cotisation regime unifie Agirc-Arrco (part patronale tranche T1) » et « 146 - [...] (part patronale tranche
/// T2) » : deux codes de la modalite URSSAF de 81.001 (p. 302, lignes 106-112 ; libelles p. 305, lignes 292 et 296), pas
/// de la modalite Agirc-Arrco (lignes 80-81). SIG-18 (p. 303, lignes 158-163) : un 131 sous une base 78 datee dans le
/// mois principal veut au moins un 142 sous le meme versement.
pub const AGIRC_ARRCO_PATRONALE_T1: CodeCotisation81 = CodeCotisation81::AgircArrcoPatronaleT1;
pub const AGIRC_ARRCO_PATRONALE_T2: CodeCotisation81 = CodeCotisation81::AgircArrcoPatronaleT2;
/// Les lignes du bulletin dont un 131 fait la somme : la retraite des deux tranches (CEG comprise) et la CET.
const LIGNES_AGIRC_ARRCO: [IdDeCotisation; 3] = [droit::forme_bareme::RETRAITE_T1, droit::forme_bareme::RETRAITE_T2, droit::forme_bareme::CET];

/// La part patronale d'une ligne du bulletin, et ce qui y est de la CET. Un cadre intermittent paie la CET FONDUE dans ses
/// lignes de tranche 1 et de tranche 2 (`cet_si_depasse`, `calcul::calculer` 7.7 : le dernier composant patronal prend la
/// CET en plus) ; la retraite et la CEG seules sont ce que la ligne vaudrait sans elle, au meme arrondi (chaque composant
/// sur la base de la ligne), et la CET le reste. Une ligne que ce calcul ne retrouve pas au centime refuse : sa CET ne
/// se separe pas sans deviner.
fn sans_la_cet(p: &Paie, l: &engagement::calcul::LigneCotisation, r: &Rubrique) -> R<(Decimal, Option<(Decimal, Decimal)>)> {
    let Some(cet) = r.cet_si_depasse.as_ref().filter(|_| base_de_la_paie(p, Assiette::Cet).is_some_and(|b| !b.is_zero())) else {
        return Ok((l.montant_pat, None));
    };
    let cet_pat = cet.pat;
    let comp = match r.composants_pat.as_deref() {
        Some(c) if !c.is_empty() => c.iter().map(|t| t.0).collect(),
        _ => vec![r.taux_pat.ok_or_else(|| socle::fichiers::absent("taux_pat"))?.0],
    };
    let sans = droit::bareme::somme_pct(l.base, &comp);
    let mut avec = comp;
    if let Some(dernier) = avec.last_mut() {
        *dernier += cet_pat;
    }
    if droit::bareme::somme_pct(l.base, &avec) != l.montant_pat {
        return Err(refus!(
            "La ligne « {} » du bulletin ({} de part patronale sur {}) ne se refait pas, CET comprise, à partir de \
ses taux : la part de la CET (146 en 2026, 148 en 2027) ne s'en sépare pas sans deviner. Rien n'est écrit.",
            l.id,
            socle::texte::euros(l.montant_pat),
            socle::texte::euros(l.base)
        ));
    }
    Ok((sans, Some((sub(l.montant_pat, sans), cet_pat))))
}

/// Les parts patronales Agirc-Arrco que l'Urssaf attend a cote d'un 131, sous la meme base 78. Guide Urssaf « Comment
/// declarer et regulariser les cotisations Urssaf en DSN », version du 04/06/2026
/// (`atelier/reference/sources/editeur-dsn/guide-urssaf-regularisation.pdf`, transcription `.txt` a cote), p. 34 et
/// p. 37-38 : « le montant des cotisations parts patronales de retraite complementaire Agirc Arrco (tranche 1) et de CEG
/// (Tranche 1) ainsi que le taux sont a declarer [...] sous le code 142 » ; « le montant des cotisations parts patronales
/// de retraite complementaire Agirc-Arrco (Tranche 2), de CEG (Tranche 2) et de CET (tranches 1 et 2) ainsi que le taux
/// [...] sous le code 146 ». Le 131 garde les deux parts : le meme guide recalcule la retraite complementaire SALARIALE
/// de tranche 1 comme « 81.004 pour le type 131 - 81.004 pour le type 142 » (p. 36, tableau du montant net social).
///
/// Chacune porte l'organisme Urssaf (81.002 « SIRET de l'Urssaf », p. 307), son assiette (81.003, « a renseigner pour une
/// cotisation », p. 308) et son taux (81.007) : ceux de la ligne du bulletin. La CET n'a pas la base de la tranche 2 : elle
/// part dans un 146 a elle, ce que le guide admet (« il est tolere de multiplier les blocs [...] 146 [...] pour declarer
/// separement chacun des elements sous ce meme code », p. 38). Le bloc 78 est celui du 131 : « Chaque cotisation
/// individuelle est rattachee a la base assujettie juridique correspondante » (p. 5), et la modalite Urssaf de 78.001
/// admet la base 23 (norme p. 293, ligne 133). Une part nulle ne s'ecrit pas. En 2027 la CET a son code, 148, « entierement
/// sous l'assiette deplafonnee » (03, 22 ou 23 ; fiche net-entreprises 2556, l. 83-87) : le code s'ecrit selon la version
/// du fichier (`Cotisation::cet`, `Norme::cotisation_cet_patronale`), la base reste celle du 131 (type 23).
fn parts_patronales_agirc_arrco(p: &Paie, pile: &Pile, c: &CotisationDsn, l131: &Cotisation) -> R<Vec<Cotisation>> {
    let rubrique = |id: &IdDeCotisation| pile.rubrique(id);
    // Le 131 nomme ses lignes, ou porte un taux sur l'assiette de la ligne de retraite qu'il declare (`bareme::poser_t1`).
    let ids: Vec<IdDeCotisation> = if let Some(lignes) = c.lignes.as_ref().filter(|l| !l.is_empty()) {
        lignes.clone()
    } else {
        LIGNES_AGIRC_ARRCO.iter().filter(|id| rubrique(id).is_some_and(|r| r.base == c.base)).cloned().collect()
    };
    let dites = |l: &[IdDeCotisation]| l.iter().map(IdDeCotisation::as_str).collect::<Vec<_>>().join(", ");
    if ids.is_empty() || ids.iter().any(|id| !LIGNES_AGIRC_ARRCO.contains(id)) {
        return Err(refus!(
            "Le bareme declare un 131 (Agirc-Arrco) fait {} : sa part patronale (142, 146), que l'Urssaf attend sous le \
meme versement (cahier technique 2026.1, SIG-18 p. 303), ne se lit que sur les lignes de retraite du bulletin \
({}). Rien n'est ecrit.",
            if ids.is_empty() { "d'aucune ligne du bulletin".to_string() } else { format!("des lignes {}", dites(&ids)) },
            dites(&LIGNES_AGIRC_ARRCO)
        ));
    }
    let org = org_de(pile, CodeCotisation81::AssuranceMaladie, "la part patronale Agirc-Arrco (142, 146), a l'Urssaf")?;
    let part = |code: CodeCotisation81, base: Decimal, taux: Decimal, montant: Decimal| Cotisation {
        bloc78: l131.bloc78,
        base78: l131.base78,
        code81: Some(code),
        org: Some(org.clone()),
        affectation: l131.affectation.clone(),
        cle_total: None,
        code_insee: None,
        base81: Some(base),
        taux: Some(taux),
        montant: Some(montant),
        composant79: None,
        cet: false,
    };
    let mut out = Vec::new();
    let (mut cet_montant, mut cet_base, mut cet_taux) = (zero2(), zero2(), None::<Decimal>);
    for id in &ids {
        // La CET d'un non cadre n'existe pas ; une tranche 2 nulle n'est pas au bulletin (`calcul::calculer`).
        let (Some(r), Some(l)) = (rubrique(id), p.lignes.iter().find(|l| l.est(id))) else { continue };
        let (montant, cet) = sans_la_cet(p, l, r)?;
        let taux = match &cet {
            Some(_) => r.taux_pat.ok_or_else(|| socle::fichiers::absent("taux_pat"))?.0,
            None => l.taux_pat.ok_or_else(|| refus!("La ligne « {} » du bulletin n'a pas de taux patronal : sa part Agirc-Arrco ne se declare pas.", id))?,
        };
        if let Some((m, t)) = cet {
            if cet_taux.is_some_and(|x| x != t) {
                return Err(refus!("La CET des lignes {} n'a pas un seul taux patronal : sa part patronale (146 en 2026, 148 en 2027) ne s'ecrit pas.", dites(&ids)));
            }
            cet_taux = Some(t);
            cet_montant = add(cet_montant, m);
            cet_base = add(cet_base, l.base);
        }
        if !montant.is_zero() {
            let code = if *id == droit::forme_bareme::RETRAITE_T1 { AGIRC_ARRCO_PATRONALE_T1 } else { AGIRC_ARRCO_PATRONALE_T2 };
            // La ligne « cet » d'un permanent est toute de la CET ; celle d'un cadre intermittent est fondue dans ses
            // tranches et se separe plus bas (`sans_la_cet`).
            out.push(Cotisation { cet: *id == droit::forme_bareme::CET, ..part(code, l.base, taux, montant) });
        }
    }
    if let (Some(t), false) = (cet_taux, cet_montant.is_zero()) {
        out.push(Cotisation { cet: true, ..part(AGIRC_ARRCO_PATRONALE_T2, cet_base, t, cet_montant) });
    }
    Ok(out)
}

/// Les cotisations d'une meme base se suivent, pour s'ecrire sous un seul bloc 78. Une base est la « Somme des
/// montants assujettis de maniere homogene a une ou plusieurs cotisations sociales » (norme DSN 2026.1, bloc 78,
/// p. 291 ; `norme-dsn/texte/bloc-S21.G00.78.txt:60`) : la reduction generale (018, 106) et le versement mobilite
/// (081), ajoutes apres les bases 31 et 43, ouvraient sinon un second bloc 03 au meme montant sous le meme
/// versement, et la base 03 nominative valait le double. Ordre stable : une ligne rejoint la derniere de sa base,
/// les autres ne bougent pas.
fn regrouper_par_base(lignes: Vec<Cotisation>) -> Vec<Cotisation> {
    let mut out: Vec<Cotisation> = Vec::with_capacity(lignes.len());
    for l in lignes {
        let meme = |c: &Cotisation| c.bloc78 == l.bloc78 && c.affectation == l.affectation && c.base78 == l.base78;
        match out.iter().rposition(meme) {
            Some(i) => out.insert(i + 1, l),
            None => out.push(l),
        }
    }
    out
}

// ------------------------------------------------ adhesions et affiliations

/// Le premier et le dernier jour du mois.
fn bornes_du_mois(mois: Mois) -> (Jour, Jour) {
    (mois.premier_jour(), mois.dernier_jour())
}

/// Les references d'adhesion auxquelles cette nature rattache, parmi celles qui valent pour la
/// periode `debut`..=`fin`.
pub fn affiliations(orgs: &Organismes, nat: &Nature, contribs: &[ContributionDuMois], (debut, fin): (Jour, Jour)) -> R<Vec<String>> {
    let cle = section_de(nat).to_string();
    let Some(sec) = orgs.par_nature.de(&cle) else {
        return Err(refus_avec(format!(
            "Contrat {} : la DSN rattache chaque salarié à ses contrats de prévoyance et de santé (S21.G00.70.013), et \
les organismes composés pour la compagnie ne disent rien d'un salarié « {} » (seulement des intermittents et des \
permanents). Aucune saisie de la compagnie ne le dit : c'est la composition livrée qui \
ne connaît pas ce cas. Rien n'est écrit.",
            nat.libelle, cle
        ), Lieu { fichier: Some("organismes-base.json".into()), ..Default::default() }));
    };
    if sec.references.is_empty() {
        let pourquoi = match sec.a_confirmer {
            Some(p) => p,
            None => "Leurs références se saisissent dans Configuration › Organismes et contrats ; elles se lisent sur le certificat \
d'adhésion de l'assureur.",
        };
        return Err(refus!(
            "Contrat {} : la DSN rattache chaque salarié à ses contrats de prévoyance et de santé (S21.G00.70.013), et ceux \
d'un salarié « {} » ne sont pas connus.\n  {}\n  Déclarer quand même le rattacherait aux contrats d'une autre \
population, ce que la plate-forme accepte sans rien dire. Rien n'est écrit.",
            nat.libelle, cle, pourquoi
        ));
    }
    let mut out: Vec<String> = sec.references.iter().filter(|r| orgs.adhesion_en_vigueur(r, &debut, &fin)).cloned().collect();
    for c in contribs {
        if c.natures.contains_key(&cle) {
            out.push(c.reference.clone());
        }
    }
    Ok(out)
}

/// Une affiliation que le contrat declare (bloc S21.G00.70), dans l'ordre de `affiliations`.
pub struct Affiliation {
    /// Sa place parmi les references de la nature : elle fait l'identifiant (70.012) et l'affectation de sa base 31
    /// (`202 + place`, les `cotisations_dsn` du bareme). Pour un permanent, 0 est la prevoyance et 1 la sante -- l'ordre
    /// que `organismes::composer` ecrit et que le socle attend.
    pub place: usize,
    /// La reference de l'adhesion (70.013 = 15.005).
    pub reference: String,
    /// L'option retenue par le salarie (70.004), quand l'organisme la demande pour ce contrat.
    pub option: Option<String>,
    /// Les ayants droit conjoint (70.008) et enfants (70.011), quand ils entrent dans le calcul de la cotisation.
    pub ayants_droit: Option<(i64, i64)>,
    /// La date de debut de l'affiliation (70.014), quand ce n'est pas celle du contrat.
    pub debut: Option<Jour>,
}

/// Ce qui affilie un permanent, ce mois-la, aux deux adhesions de sa nature : la prevoyance de sa convention, selon la
/// regle de sa cotisation (`bareme::affiliation_prevoyance`), et la sante, sauf dispense.
struct AffiliationsDuPermanent {
    prevoyance: bareme::AffiliationPrevoyance,
    sante: bool,
    /// La cle des cases d'un salarie au mois, que la fin d'une dispense lit (`rattachements`).
    au_mois: droit::natures::AuMois,
}

/// `None` pour un contrat qui n'est pas paye au mois : un intermittent est affilie a toutes les adhesions de sa nature.
fn affiliations_du_permanent(p: &Paie, ent: &ChampsEntreprise) -> R<Option<AffiliationsDuPermanent>> {
    let Some(au_mois) = p.engagement.nat()?.au_mois() else {
        return Ok(None);
    };
    let fin = p.engagement.fin_periode();
    let prevoyance = bareme::affiliation_prevoyance(ent, &p.salarie, &au_mois, &fin)?;
    // La dispense (CCNEAC XII.2.1.8 ; 3090, accord du 19/07/2024, art. 9.2 ; `sante_dispense`) : le bulletin n'a plus
    // de mutuelle (`bareme_sante`), et le
    // salarie n'est pas rattache au contrat de sante -- 70.014/.015 decrivent la dispense comme ce qui ouvre ou ferme
    // l'affiliation (cahier technique 2026.1, p. 287).
    let dispense = p.salarie.permanent.ouvrir(&au_mois).sante_dispense.portee();
    // Un mois entierement suspendu sans indemnisation, garanties suspendues (`calcul::sante_de_la_suspension` : 3090,
    // accord sante du 19/07/2024, art. 4.2 et 15) : pas de cotisation, et pas de rattachement a la sante ce mois-la --
    // comme une dispense. INFERENCE NOMMEE : aucune fiche DSN deposee ne dit comment declarer une affiliation sante
    // suspendue (pas de bloc 70 ? un bloc 70 avec une fin de rattachement ? une base 31 a zero ?) ; le moteur n'ecrit ni
    // bloc 70 ni base 31 de sante pour ce mois (`atelier/reference/sources/suspension-non-remuneree/RAPPORT-20261015.md`,
    // point 2 et « Ce qui reste ouvert », 3).
    let suspendue = p.salarie.sante_suspendue.as_deref().filter(|s| !s.is_empty());
    if let Some(motif) = suspendue.filter(|_| p.lignes.iter().any(|l| l.est(&droit::forme_bareme::MUTUELLE))) {
        return Err(refus!(
            "{} : sa sante est suspendue ce mois-ci ({}) et le bulletin cotise a la mutuelle. Rien n'est declare.",
            qui_contrat(p),
            motif
        ));
    }
    if let Some(motif) = dispense.filter(|_| p.lignes.iter().any(|l| l.est(&droit::forme_bareme::MUTUELLE))) {
        return Err(socle::echec::cle(
            format!(
                "{} : la fiche porte une dispense de complementaire sante (« {} ») et le bulletin cotise a \
la mutuelle{} : il a ete calcule sans elle. La DSN ne choisit pas entre l'affiliation et la dispense : \
recalculer le bulletin si le salarie est dispense, retirer la dispense de la fiche s'il est affilie. Rien n'est \
declare.",
                qui_contrat(p),
                motif,
                ent.idcc.portee().into_iter().map(|i| format!(" (convention {})", i)).collect::<String>()
            ),
            "sante_dispense",
        ));
    }
    Ok(Some(AffiliationsDuPermanent { prevoyance, sante: dispense.is_none() && suspendue.is_none(), au_mois }))
}

/// Les places (`Affiliation::place`) des adhesions de sa nature auxquelles le salarie n'est pas affilie ce mois-la.
fn places_non_affiliees(p: &Paie, ent: &ChampsEntreprise) -> R<Vec<usize>> {
    let mut out = Vec::new();
    if let Some(a) = affiliations_du_permanent(p, ent)? {
        if a.prevoyance == bareme::AffiliationPrevoyance::NonAffilie {
            out.push(0);
        }
        if !a.sante {
            out.push(1);
        }
    }
    Ok(out)
}

/// Les affiliations que le contrat declare (bloc S21.G00.70), parmi les references `refs` de sa nature
/// (`affiliations`) : celles du salarie ce mois-la, avec ce que l'organisme demande de chacune.
///
/// Cahier technique DSN 2026.1 (`atelier/reference/norme-dsn/texte/bloc-S21.G00.70.txt`, p. 285-286 ;
/// `bloc-S21.G00.71.txt`, p. 287) : 70.004 « fournies par l'organisme de prévoyance et [...] à renseigner seulement sur
/// sa demande » ; 70.008 et 70.011 « si elle intervient dans le calcul de la cotisation » ; 70.014 « lorsque la date de
/// début d'affiliation au contrat OC ne correspond pas à la date de début du contrat de travail : [...] fin d'une
/// dispense d'affiliation, etc. ». Le moteur date ce qu'il sait : une adhesion ouverte apres le debut du contrat
/// (`depuis`, Thalie Sante au 01/01/2026) et une prevoyance que l'anciennete dans la branche ouvre
/// (`prevoyance_branche_depuis`). La date s'ecrit chaque mois ou le contrat se declare (inference : la condition de la
/// norme reste vraie ; la liste des cas d'usage de net-entreprises n'est pas dans le depot).
/// `ent` : la fiche de l'entreprise que la paie a lue (`BaremeDuMois::entreprise`).
pub fn rattachements(p: &Paie, orgs: &Organismes, ent: &ChampsEntreprise, refs: Vec<String>) -> R<Vec<Affiliation>> {
    let e = &p.engagement;
    let permanent = affiliations_du_permanent(p, ent)?;
    let fin = e.fin_periode();
    let mut out = Vec::new();
    for (place, reference) in refs.into_iter().enumerate() {
        let mut debut: Option<Jour> = None;
        if let Some(a) = &permanent {
            if place == 0 {
                match &a.prevoyance {
                    bareme::AffiliationPrevoyance::NonAffilie => continue,
                    bareme::AffiliationPrevoyance::DesLEmbauche => {}
                    bareme::AffiliationPrevoyance::Le(j) => debut = Some(*j),
                    bareme::AffiliationPrevoyance::AuPlusTardLe(j) => {
                        if *j > e.debut {
                            return Err(socle::echec::cle(format!(
                                "{} : la prevoyance de sa convention s'ouvre apres une anciennete DANS LA BRANCHE, et sa \
fiche ne date que son anciennete dans l'entreprise (« Dans la compagnie depuis ») : assez pour la cotisation, pas pour dire si \
l'affiliation a commence avec le contrat (le {}) ou plus tard, au plus tard le {}. La DSN date le debut de \
l'affiliation quand ce n'est pas celui du contrat (S21.G00.70.014, cahier technique p. 287) ; elle ne le devine pas.\n  \
A dire sur sa fiche : Configuration › Salariés, bloc « Salarié au mois », « Dans la branche depuis » : la date de son entree dans la branche (s'il n'y \
travaillait pas avant, sa date d'embauche chez nous). Rien n'est declare.",
                                qui_contrat(p),
                                e.debut.jj_mm_aaaa(),
                                j.jj_mm_aaaa()
                            ), "prevoyance_branche_depuis"));
                        }
                    }
                }
            }
            if place == 1 && !a.sante {
                continue;
            }
            // Affilie a la sante a la fin d'une dispense (Q13) : 70.014 « fin d'une dispense d'affiliation »
            // (`bloc-S21.G00.71.txt`, p. 287), le lendemain de son dernier jour. Ce mois-ci n'est plus dispense
            // (`calcul::dispense_de_la_periode`) : une fin de dispense posee est passee.
            if place == 1 {
                if let Some(j) = p.salarie.permanent.ouvrir(&a.au_mois).sante_dispense_jusqu_au.dite() {
                    let lendemain = j.plus_jours(1)?;
                    if lendemain <= fin && debut.is_none_or(|x| lendemain > x) {
                        debut = Some(lendemain);
                    }
                }
            }
        }
        let adhesion = orgs.adhesion(&reference);
        if let Some(d) = adhesion.and_then(|a| a.depuis) {
            if debut.is_none_or(|x| d > x) {
                debut = Some(d);
            }
        }
        let (option, ayants_droit) = match adhesion {
            Some(a) => option_retenue(p, ent, a, &reference)?,
            None => (None, None),
        };
        out.push(Affiliation { place, reference, option, ayants_droit, debut: debut.filter(|d| *d > e.debut) });
    }
    Ok(out)
}

/// L'option retenue par le salarie (70.004) et ses ayants droit (70.008, 70.011) sous l'adhesion `adhesion`, quand
/// `organismes-base.json` dit que l'organisme les demande pour ce contrat (`contrats`, `options` : la sante des
/// permanents 3090, fiche de parametrage Audiens version 139, groupes 00003 a 00008). Ils se lisent de `sante_svp`, la
/// meme fiche que la cotisation (`bareme::bareme_sante`) ; sans elle, ou d'un niveau que la table n'a pas, refus.
fn option_retenue(p: &Paie, ent: &ChampsEntreprise, adhesion: &droit::organismes::Adhesion, reference: &str) -> R<(Option<String>, Option<(i64, i64)>)> {
    let base = droit::organismes::base()?;
    let libelle = adhesion.libelle.trim();
    let Some(options) = base.options(ent.idcc.portee(), libelle) else {
        return Ok((None, None));
    };
    let lieu = format!(
        "{} : affilie a {} (S21.G00.70.013 = {}), dont Audiens demande l'option retenue par le salarie (S21.G00.70.004 ; \
fiche de parametrage Audiens, groupes << PERMANENT SVP SANTE >>, `organismes-base.json`)",
        qui_contrat(p),
        libelle,
        reference
    );
    if bareme::alsace_moselle(ent, &p.salarie) && !options.regime_local {
        return Err(refus!(
            "{} : ses options sont du regime general (GE), et ce salarie releve du regime local d'Alsace-Moselle -- le code \
de son option n'est dans aucune fiche de parametrage du depot. Il se lit sur la fiche de parametrage Audiens de la \
compagnie ; rien n'est declare.",
            lieu
        ));
    }
    // Le niveau d'un salarie au mois : un contrat qui ne l'est pas n'en dit aucun.
    let au_mois = p.engagement.nat()?.au_mois();
    let choix = au_mois.as_ref().and_then(|au| p.salarie.permanent.ouvrir(au).sante_svp.dite());
    let niveau = choix.and_then(|c| c.niveau.portee());
    let enfants = match choix.and_then(|c| c.enfants.dite()) {
        Some(m) => m.entier()?,
        None => None,
    }
    .filter(|n| (0..=99).contains(n));
    let conjoint = choix.is_some_and(|c| c.conjoint.oui());
    let famille = conjoint || enfants.is_some_and(|n| n > 0);
    let code = niveau.and_then(|niveau| options.niveaux.get(niveau)).map(|n| if famille { n.famille.as_str() } else { n.isole.as_str() });
    match (code, enfants) {
        (Some(c), Some(n)) => Ok((Some(c.to_string()), famille.then_some((i64::from(conjoint), n)))),
        _ => Err(socle::echec::a_la_cle(
            &format!(
                "{} : elle se lit sur la fiche du salarie (son niveau, son conjoint, ses enfants), d'apres son bulletin d'adhesion -- la meme fiche que sa cotisation. Rien n'est declare.",
                lieu
            ),
            "sante_svp",
            match niveau {
                Some(n) => format!("niveau « {} »", n),
                None => "rien".to_string(),
            },
        )),
    }
}

/// `places_par_contrat` : combien d'identifiants d'affiliation un contrat se reserve.
pub fn places_par_contrat(bar: &Pile) -> i64 {
    3.max(2 + bar.contributions_dsn.iter().flatten().count() as i64)
}

/// Les references de chaque nature (hors `ref`), en vigueur pendant le mois.
fn refs_du_mois(orgs: &Organismes, sec: &droit::organismes::Rattachement, mois: Mois) -> Vec<String> {
    let (debut, fin) = bornes_du_mois(mois);
    sec.references.iter().filter(|r| orgs.adhesion_en_vigueur(r, &debut, &fin)).cloned().collect()
}

/// `casc_svp` : la CASC-SVP de ce mois, ou `None`.
fn casc_svp(ctx: &Contexte, ent: &ChampsEntreprise, orgs: &Organismes, mois: Mois) -> R<Option<ContributionDuMois>> {
    let base = droit::adhesions::casc_svp_base()?;
    if ent.idcc.portee() != Some(base.idcc.as_str()) || mois < base.depuis {
        return Ok(None);
    }
    let (an, eff) = (mois.annee(), ctx.effectif()?);
    if bareme::casc_exemptee(ent, eff.cinquante(an).as_ref(), eff.cse(an).as_ref()) {
        return Ok(None);
    }
    if crate::casc::mois_regularises(ctx)?.contains(&mois) {
        return Ok(None);
    }
    let contrat = &base.dsn.contrat;
    let adh = orgs.adhesions_prevoyance.iter().find(|a| a.libelle.trim() == contrat);
    let adh = match adh {
        Some(a) => a,
        None => {
            return Err(refus!(
                "Convention 3090 : la CASC-SVP se declare en DSN depuis janvier 2025, et les organismes lus n'ont pas \
l'adhesion {} que le logiciel compose pour toute compagnie de la 3090. Rien n'est ecrit.",
                contrat
            ))
        }
    };
    let reference = adh.reference.clone();
    let mut natures = std::collections::BTreeMap::new();
    for (n, s) in orgs.par_nature.natures() {
        if s.references.is_empty() || s.references.contains(&reference) {
            continue;
        }
        let refs = refs_du_mois(orgs, s, mois);
        if refs.len() >= 3 {
            return Err(refus!(
                "Contrat '{}' : {} adhesions deja, plus de place pour la CASC-SVP (trois identifiants \
d'affiliation par contrat). Rien n'est ecrit.",
                n,
                refs.len()
            ));
        }
        natures.insert(n.to_string(), (202 + refs.len()).to_string());
    }
    Ok(Some(ContributionDuMois { reference, libelle: adh.libelle.clone(), lignes: base.dsn.lignes_bulletin.clone(), natures }))
}


/// `contributions_dsn` : les contributions declarees sous une adhesion Audiens.
fn contributions_dsn(ctx: &Contexte, ent: &ChampsEntreprise, orgs: &Organismes, mois: Mois, bar: &Pile) -> R<Vec<ContributionDuMois>> {
    let mut out = Vec::new();
    if let Some(c) = casc_svp(ctx, ent, orgs, mois)? {
        out.push(c);
    }
    let specs: Vec<&droit::forme_bareme::ContributionDsn> = bar.contributions_dsn
        .iter()
        .flatten()
        .filter(|c| mois >= c.en_dsn_depuis)
        .collect();
    if specs.is_empty() {
        return Ok(out);
    }
    let places = places_par_contrat(bar);
    let mut deja: HashMap<String, i64> = HashMap::new();
    for spec in specs {
        let nom = spec.contribution.clone();
        let adh = orgs.adhesions_prevoyance.iter().find(|a| {
            a.contribution.as_deref().is_some_and(|c| c.trim().to_uppercase() == nom.to_uppercase()) || a.libelle.trim().to_uppercase() == nom.to_uppercase()
        });
        let adh = match adh {
            Some(a) => a,
            None => {
                return Err(refus!(
                    "Convention {} : le {} se declare en DSN depuis {}, sous une adhesion Audiens, et les organismes \
du mois n'ont pas l'adhesion de cette contribution, que le logiciel compose depuis le bareme. Rien n'est ecrit.",
                    ent.idcc.ecrite("idcc")?,
                    nom,
                    spec.en_dsn_depuis
                ))
            }
        };
        let reference = adh.reference.clone();
        let mut natures = std::collections::BTreeMap::new();
        for (n, sec) in orgs.par_nature.natures() {
            if sec.references.is_empty() || sec.references.contains(&reference) {
                continue;
            }
            let refs = refs_du_mois(orgs, sec, mois);
            let place = refs.len() as i64 + *deja.entry(n.to_string()).or_insert(0);
            if place >= places {
                return Err(refus!(
                    "Contrat '{}' : {} adhesions deja, plus de place pour le {} ({} identifiants d'affiliation par \
contrat). Rien n'est ecrit.",
                    n,
                    refs.len(),
                    nom,
                    places
                ));
            }
            natures.insert(n.to_string(), (202 + place).to_string());
            *deja.entry(n.to_string()).or_insert(0) += 1;
        }
        out.push(ContributionDuMois { reference, libelle: adh.libelle.clone(), lignes: spec.lignes.clone(), natures });
    }
    Ok(out)
}

// ------------------------------------------------------------ les blocs

/// La rubrique est-elle admise pour cette nature dans le tableau des usages de la version du fichier ?
pub fn usage_admet(f: &Fichier, rubrique: &str, nature_dsn: NatureDeLaDeclaration) -> R<bool> {
    f.norme()?.admet(rubrique, nature_dsn)
}

/// LA LICENCE D'ENTREPRENEUR DE SPECTACLES d'un contrat (S21.G00.40.049). « La licence est obligatoire pour toute
/// structure [...] dont l'activite principale est la production ou la diffusion de spectacle ou l'exploitation de lieu de
/// spectacle, a partir de 7 representations annuelles si l'activite principale de l'entreprise n'est pas d'organiser des
/// spectacles » ; « en l'absence de licence spectacle, cette rubrique doit etre renseignee par la reference portee sur le
/// recepisse » (cahier technique 2027.1.1, `norme-dsn/texte-2027/bloc-S21.G00.40.txt` l. 1418-1426 ; la meme phrase en
/// 2026.1, `texte/bloc-S21.G00.40.txt` l. 1367-1375). Ecrite quand la compagnie en a une ; sans elle, la DSN refuse
/// QUAND ELLE EST EXIGEE (`demarrage::licence_exigee`, que « Pour demarrer » lit aussi : la convention, ou un contrat de
/// spectacle, `spectacle`, dans une convention ou la case se montre) et ne l'ecrit pas sinon : une production
/// audiovisuelle n'en a pas, ni un lieu en 1790 qui n'engage pour aucun spectacle. Une valeur vide ne s'ecrit
/// jamais : `X [1,100]`.
fn licence_du_contrat(f: &mut Fichier, nature_dsn: NatureDeLaDeclaration, ent: &ChampsEntreprise, spectacle: bool) -> R<()> {
    let licence = ent.licence_spectacle.ecrite("licence_spectacle")?.to_string();
    if !licence.trim().is_empty() {
        return si_admise(f, nature_dsn, "S21.G00.40.049", Some(licence));
    }
    if crate::demarrage::licence_exigee(ent, spectacle)? && usage_admet(f, "S21.G00.40.049", nature_dsn)? {
        return Err(refus("La licence d'entrepreneur de spectacles manque : la saisir dans Configuration › Entreprise, « Licence d'entrepreneur de spectacles » (le numéro attribué par la DRAC, ou la référence du récépissé de déclaration). L'entrepreneur de spectacles vivants la déclare dans chaque contrat (S21.G00.40.049). Rien n'est écrit."));
    }
    Ok(())
}

fn si_admise(f: &mut Fichier, nature_dsn: NatureDeLaDeclaration, rubrique: &str, valeur: Option<String>) -> R<()> {
    if usage_admet(f, rubrique, nature_dsn)? {
        f.add_opt(rubrique, valeur)?;
    }
    Ok(())
}

fn motif_recours(p: &Paie, nat: &Nature) -> R<droit::natures::MotifDeRecours> {
    let e = &p.engagement;
    match e.motif_recours {
        Some(m) => Ok(m),
        None => Err(refus!(
            "Dites pourquoi ce CDD est conclu (« Motif du CDD », sur la carte de l'engagement) : {} ({}) n'en dit pas le motif.\n\
C'est le motif qui rend le contrat licite (code du travail, L. 1242-2 : remplacement d'un salarié, accroissement \
temporaire d'activité, emploi saisonnier…) ; la DSN le déclare, il y est obligatoire (S21.G00.40.021, SIG-11).",
            droit::cpf_cdd::le_contrat(e.numero.as_deref(), &e.salarie),
            nat.libelle
        )),
    }
}

fn motif_rupture(p: &Paie, nat: &Nature) -> R<MotifDeRupture> {
    let e = &p.engagement;
    if let Some(m) = e.motif_rupture {
        // CCH-11 de S21.G00.62.002 : un motif que la nature n'admet pas ne s'ecrit pas.
        if let Some(phrase) = nat.motif_rupture_refuse(m, e.numero_declare()?) {
            return Err(refus(phrase));
        }
        return Ok(m);
    }
    if let Some(d) = nat.motif_rupture_par_defaut() {
        return Ok(d);
    }
    let connus: Vec<String> = nat
        .motifs_rupture()
        .iter()
        .map(|m| format!("{} ({})", m.code, m.libelle))
        .collect();
    Err(socle::echec::cle(format!(
        "Contrat {} ({}) : il se termine le {}, et le motif de la rupture (S21.G00.62.002) n'est pas dit. Un {} ne \
finit pas tout seul, et le motif commande les droits du salarie a l'assurance chomage : il se saisit. Motifs : {}.",
        e.numero_declare()?,
        nat.libelle,
        e.fin.iter().map(|f| f.jjmmaaaa()).collect::<String>(),
        nat.libelle,
        connus.join(", ")
    ), "motif_rupture"))
}


fn qt(x: Decimal) -> String {
    fmt_f(x, 2)
}

/// « Prenom NOM » d'une fiche, a defaut sa cle : qui une phrase de refus designe.
fn qui_fiche(s: &droit::fiche::ChampsSalarie, cle: &IdSalarie) -> String {
    match (s.prenom.portee(), s.nom.portee()) {
        (Some(prenom), Some(nom)) => format!("{} {}", prenom, nom),
        (Some(un), None) | (None, Some(un)) => un.to_string(),
        (None, None) => cle.to_string(),
    }
}

/// « Prenom NOM, contrat <numero> » : qui une phrase de refus du bloc 40 designe.
fn qui_contrat(p: &Paie) -> String {
    let nom = qui_fiche(&p.salarie, &cle_salarie(p));
    match &p.engagement.numero {
        Some(n) => format!("{}, contrat {}", nom, n),
        None => format!("{}, contrat sans numero", nom),
    }
}

/// `S21.G00.86.003` d'une DSN mensuelle : l'anciennete dans l'entreprise (type 07, en jours), « connue a la fin de la
/// periode mensuelle declaree », jamais zero (cahier technique 2026.1, p. 322, `norme-dsn/texte/bloc-S21.G00.86.txt`,
/// l. 104-115). Elle part de `date_anciennete` de la fiche, l'embauche : le premier jour du premier contrat du salarie (la
/// paie la lit ainsi, `calcul::fiche_de_l_engagement` ; l'import la pose au premier contrat lu), et que les gens peuvent
/// changer. Une fiche qui ne la porte pas ne dit pas ce jour : elle refuse. Une date apres la fin du mois donnerait zero
/// ou moins, que la CCH-12 interdit : elle refuse aussi.
pub fn anciennete_du_mois(s: &droit::fiche::ChampsSalarie, cle: &IdSalarie, fin_m: &Jour) -> R<i64> {
    let Some(depuis) = s.date_anciennete.dite().copied() else {
        return Err(socle::echec::cle(format!(
            "{} : la fiche ne porte pas sa date d'anciennete (« Dans la compagnie depuis »), d'ou se \
compte l'anciennete declaree en S21.G00.86.003. C'est le jour de son embauche, que la validation de son premier \
engagement pose et l'import au premier contrat lu ; cette fiche ne la dit pas, et elle ne se devine pas. La saisir sur \
la fiche : le jour ou la personne est entree dans la compagnie.",
            qui_fiche(s, cle)
        ), "date_anciennete"));
    };
    let jours = fin_m.moins(&depuis) + 1;
    if jours < 1 {
        return Err(socle::echec::cle(format!(
            "{} : la fiche date son anciennete du {}, apres la fin du mois declare ({}). \
L'anciennete declaree en S21.G00.86.003 serait de {} jour(s), et la norme DSN interdit zero (CCH-12, cahier technique \
p. 322). Corriger la date sur la fiche : le jour ou la personne est entree dans la compagnie.",
            qui_fiche(s, cle),
            depuis.jj_mm_aaaa(),
            fin_m.jj_mm_aaaa(),
            jours
        ), "date_anciennete"));
    }
    Ok(jours)
}

/// `S21.G00.40.002` (statut conventionnel) et `S21.G00.40.003` (statut categoriel retraite complementaire) d'un
/// contrat. Cahier technique DSN 2026.1, p. 182-183 (`atelier/reference/norme-dsn/texte/bloc-S21.G00.40.txt`, l. 133-198).
///
/// Ils se LISENT, jamais ne se devinent (regle 1) : d'abord la fiche telle que l'engagement l'a faite (`emploi_de` :
/// le metier choisi sur la ligne les a resolus, sinon la fiche les porte) ; a defaut, le metier de la fiche s'il est
/// au CATALOGUE (`emplois-base.json`), par la meme regle que `emplois::resoudre`. Un metier hors catalogue (la
/// nomenclature France Travail seule) ne dit rien du classement : 05 et 04 ne se posent pas par
/// defaut, ce serait faux pour un ouvrier (07), un employe (06), un cadre (04 / 01).
///
/// Un metier cadre au catalogue, sur une fiche qui ne dit pas cadre ou non cadre, refuse aussi : la paie s'est
/// calculee non cadre (`bareme::est_cadre` ne lit que la fiche), le catalogue dit cadre, et ni 01 ni 04 ne se lit.
///
/// La CCH-11 (p. 182, l. 156-160 : 40.003 = 01 => 40.002 dans 03, 04 ou 08) est un controle bloquant : verifiee ici,
/// avant de produire.
pub fn statuts_du_contrat(p: &Paie, idcc: Option<&str>) -> R<(String, String)> {
    let s = &p.salarie;
    let lu = |v: Option<&str>| v.map(str::trim).filter(|v| !v.is_empty()).map(str::to_string);
    let code = s.code_emploi_ft.dite();
    let m = match code {
        Some(c) => droit::emplois::metier(c)?.filter(|m| m.au_catalogue().is_some()),
        None => None,
    };
    // Ce qui manque quand le catalogue ne dit rien du metier.
    let metier_dit = || match code {
        Some(c) => format!("son metier ({}) n'est pas au catalogue des metiers, qui les dirait", c),
        None => "la fiche n'a pas de metier (code emploi)".to_string(),
    };
    let rc = match (lu(s.statut_categoriel_rc.code()), &m) {
        (Some(rc), _) => rc,
        (None, Some(m)) if m.cadre_par_defaut(idcc) == Some(false) => "04".to_string(),
        (None, Some(m)) => {
            return Err(refus!(
                "{} : le metier {} ({}) est cadre au catalogue, et la fiche ne dit pas cadre ou non cadre \
(S21.G00.40.003, « Cadre ou non cadre »). La paie s'est calculee non cadre ; ni l'un ni l'autre ne se devine. Saisir \
« Cadre ou non cadre » sur la fiche : 01 (cadre, et refaire la paie) ou 04 (non cadre).",
                qui_contrat(p),
                m.code,
                m.libelle()
            ))
        }
        (None, None) => {
            let aussi = if lu(s.statut_conventionnel.code()).is_none() {
                ", ni le classement conventionnel (S21.G00.40.002, « Classement conventionnel (DSN) » : 04 cadre, 05 profession \
intermediaire, 06 employe, 07 ouvrier)"
            } else {
                ""
            };
            return Err(refus!(
                "{} : la fiche ne dit pas le statut categoriel de retraite complementaire (S21.G00.40.003, \
« Cadre ou non cadre » : 01 cadre, 04 non cadre){}, et {}. Ils ne se devinent pas : choisir le metier dans la liste des \
metiers, ou saisir « Cadre ou non cadre » et « Classement conventionnel (DSN) » sur la fiche.",
                qui_contrat(p),
                aussi,
                metier_dit()
            ));
        }
    };
    let sc = match lu(s.statut_conventionnel.code()) {
        Some(sc) => Some(sc),
        None => m.as_ref().and_then(|m| match rc.as_str() {
            "01" => Some("04".to_string()),
            "04" => m.statut_non_cadre().map(|s| s.as_str().to_string()),
            _ => None,
        }),
    };
    let Some(sc) = sc else {
        let raison = match &m {
            Some(m) => format!("le metier {} ne le dit pas pour le statut {}", m.code, rc),
            None => metier_dit(),
        };
        return Err(refus!(
            "{} : la fiche ne dit pas le classement conventionnel (S21.G00.40.002, « Classement conventionnel (DSN) » : 04 cadre, 05 \
profession intermediaire, 06 employe, 07 ouvrier), et {}. Il ne se devine pas : le saisir sur la fiche.",
            qui_contrat(p),
            raison
        ));
    };
    if rc == "01" && !["03", "04", "08"].contains(&sc.as_str()) {
        return Err(refus!(
            "{} : cadre (S21.G00.40.003 = 01) et classe {} en S21.G00.40.002. La norme DSN n'admet pour un cadre que 03 \
(cadre dirigeant), 04 (autres cadres) ou 08 (CCH-11, cahier technique p. 182) : la declaration serait rejetee. Corriger \
« Classement conventionnel (DSN) » (04) ou « Cadre ou non cadre » sur la fiche.",
            qui_contrat(p),
            sc
        ));
    }
    Ok((sc, rc))
}

/// Les blocs 40 a 71 d'un contrat. `emplois` : le code emplois multiples (`40.036`) du contrat,
/// que `emplois_multiples` tire des autres contrats de la periode.
pub fn bloc_contrat(
    f: &mut Fichier, p: &Paie, ent: &ChampsEntreprise, orgs: &Organismes, rang: i64, bar: &BaremeDuMois, fin_de_contrat: bool,
    arrets: &[&Arret], nature_dsn: NatureDeLaDeclaration, emplois: &str,
) -> R<()> {
    bloc_contrat_fctu(f, p, ent, orgs, rang, bar, fin_de_contrat, arrets, nature_dsn, None, emplois)
}

/// Les blocs d'un contrat avec `fin_fctu` : la fin du contrat d'un signalement de
/// fin de contrat (blocs 62 et 63, `fctu::bloc_62_63`) a la place de celle
/// d'une mensuelle.
pub fn bloc_contrat_fctu(
    f: &mut Fichier, p: &Paie, ent: &ChampsEntreprise, orgs: &Organismes, rang: i64, bar: &BaremeDuMois, fin_de_contrat: bool,
    arrets: &[&Arret], nature_dsn: NatureDeLaDeclaration, fin_fctu: Option<&crate::fctu::FinDeContrat>, emplois: &str,
) -> R<()> {
    let e = &p.engagement;
    let s = &p.salarie;
    let nat = e.nat()?;
    let atmp = bar.taux_atmp.as_ref();
    let ecrit_atmp = usage_admet(f, "S21.G00.40.043", nature_dsn)?;
    // SIG-11 : sous le code risque 999ZZ (pas encore de notification), le taux (40.043) est interdit ; sous tout autre
    // code, il est obligatoire (`norme-dsn/texte/bloc-S21.G00.40.txt`, l. 1309-1313).
    let code_risque = atmp.and_then(|a| a.code_risque.clone()).filter(|c| !c.is_empty());
    let valeur_atmp = atmp.and_then(|a| a.valeur).filter(|v| !v.is_zero());
    let sans_notification = code_risque.as_deref() == Some(bareme::SANS_NOTIFICATION);
    // Le code risque (40.040) et le taux (40.043, sauf sans notification) que le contrat declare.
    let atmp_ecrit: Option<(String, Option<Decimal>)> = match (ecrit_atmp, code_risque, valeur_atmp) {
        (false, _, _) => None,
        (true, Some(c), _) if sans_notification => Some((c, None)),
        (true, Some(c), Some(v)) => Some((c, Some(v))),
        (true, _, _) => {
            return Err(socle::echec::cle(format!(
                "Bareme {} : le taux AT/MP manque (sa valeur et son code risque). C'est le taux AT/MP notifie a l'etablissement, \
declare en S21.G00.40.043 ; il ne se devine pas.",
                bar.annee
            ), "taux_atmp"))
        }
    };
    // Les statuts se lisent, ou refusent (`statuts_du_contrat`), avant qu'une ligne du contrat soit ecrite.
    let (statut_conventionnel, statut_categoriel_rc) = statuts_du_contrat(p, ent.idcc.portee())?;
    f.add("S21.G00.40.001", e.debut.jjmmaaaa());
    f.add("S21.G00.40.002", statut_conventionnel);
    f.add("S21.G00.40.003", statut_categoriel_rc);
    f.add_case("S21.G00.40.004", &s.pcs_ese, "pcs_ese")?;
    // Le complement PCS-ESE : « 999SPT pour les emplois permanents du spectacle » (CCH-18, p. 185) quand la nature le
    // fixe (`contrats-base.json` : cdi, cdd) ; le contrat d'un intermittent (`droit::caisse_conges::intermittent` : un
    // CDDU, le CDD d'un artiste ou d'un technicien de la liste de l'annexe 8) declare le code de son metier dans la
    // nomenclature des emplois du spectacle, que France Travail Spectacle lit.
    let intermittent = droit::caisse_conges::intermittent(&nat, &p.salarie)?;
    match nat.code("S21.G00.40.005").filter(|c| !c.is_empty() && !intermittent) {
        Some(c) => f.add("S21.G00.40.005", c),
        None => f.add_case("S21.G00.40.005", &s.code_emploi_ft, "code_emploi_ft")?,
    }
    f.add_case("S21.G00.40.006", &s.emploi, "emploi")?;
    f.add_opt("S21.G00.40.007", nat.code("S21.G00.40.007"))?;
    f.add_opt("S21.G00.40.008", nat.code("S21.G00.40.008"))?;
    f.add_opt("S21.G00.40.009", e.numero.clone())?;
    if !nat.duree_indeterminee || e.fin.is_some() {
        f.add_opt("S21.G00.40.010", e.fin.map(|j| j.jjmmaaaa()))?;
    }
    f.add("S21.G00.40.011", nat.unite_quotite());
    let quotite = e.quotite()?;
    // Le plafond de la nature (`contrats-base.json`, `unite_quotite.plafond`) vaut pour 40.012 comme pour 40.013 :
    // CCH-12 du bloc 40 (journee, 31.00), CCH-13 (heure, 250.00) ; cahier technique 2026.1, p. 189.
    let depasse = |rubrique: &str, quoi: &str, valeur: String| -> R<()> {
        let plafond = nat.plafond_quotite();
        let q = socle::num::dec_texte(&valeur)?;
        if q <= plafond {
            return Ok(());
        }
        let cch = match nat.unite_quotite() {
            "12" | "20" => " CCH-12",
            "10" | "21" => " CCH-13",
            autre => {
                return Err(refus!(
                    "La norme DSN ne plafonne la quotité qu'en unités 10, 12, 20 et 21 (CCH-12 et CCH-13 du bloc 40) ; la nature {} la compte en unité « {} » : le signaler au développeur.",
                    nat.libelle, autre
                ))
            }
        };
        Err(refus!(
            "Contrat {} {} : {} de {}, et la norme DSN la plafonne a {} en unite << {} >> \
({}, bloc 40{}). A trancher avant de le declarer.",
            e.numero_declare()?,
            match e.fin {
                Some(j) => format!("du {} au {}", e.debut.jjmmaaaa(), j.jjmmaaaa()),
                None => format!("commence le {}, sans terme", e.debut.jjmmaaaa()),
            },
            quoi,
            valeur,
            qt(plafond),
            nat.unite_quotite(),
            rubrique,
            cch
        ))
    };
    depasse("S21.G00.40.013", "quotite", qt(quotite))?;
    match e.quotite_reference()? {
        Some(r) => {
            depasse("S21.G00.40.012", "quotite de reference", qt(r))?;
            quotite_de_reference_sans_heures_supplementaires(f, p, &nat, r)?;
            f.add("S21.G00.40.012", qt(r))
        }
        None => return Err(refus("S21.G00.40.012 : la quotite de reference manque.")),
    }
    f.add("S21.G00.40.013", qt(quotite));
    // La modalite que la nature fixe, sinon celle de l'engagement (`Engagement::modalite_temps_travail`).
    f.ecrire_code(e.modalite_temps_travail()?)?;
    if bareme::alsace_moselle(&bar.entreprise.fiche, &p.salarie) {
        f.add("S21.G00.40.016", "01");
    } else {
        f.add_opt("S21.G00.40.016", nat.code("S21.G00.40.016"))?;
    }
    f.add("S21.G00.40.017", engagement::minima::idcc_du_contrat(ent.idcc.exigee("idcc")?, p.spectacle.as_ref(), &p.salarie)?.to_string());
    f.add_opt("S21.G00.40.018", nat.code("S21.G00.40.018"))?;
    f.add_case("S21.G00.40.019", &ent.siret, "siret")?;
    f.add_opt("S21.G00.40.020", nat.code("S21.G00.40.020"))?;
    if nat.ecrit("S21.G00.40.021") {
        let m = match nat.motif_recours_fixe() {
            Some(m) => m,
            None => motif_recours(p, &nat)?,
        };
        f.ecrire_code(m)?;
    }
    // « dans le cas ou les indemnites ne sont pas versees par l'employeur » (p. 195) : un contrat declare a la caisse
    // des Conges Spectacles (`caisse_conges::affilie`), et aucun autre -- ni un CDI, ni un contrat de douze mois ou plus.
    let caisse_conges = nat.ecrit("S21.G00.40.022") && p.declaree_a_la_caisse_des_conges()?;
    if caisse_conges {
        f.add("S21.G00.40.022", documents::conges_spectacles::code_dsn(ent)?);
    }
    // La deduction forfaitaire specifique que la fiche exerce (`calcul::deduction_forfaitaire`) : « Taux de deduction
    // forfaitaire specifique pour frais professionnels - S21.G00.40.023 », « Abattement pour frais professionnels applique
    // a la part de remuneration afferente a l'activite ouvrant droit a la deduction » (cahier technique 2026.1, p. 195,
    // `atelier/reference/norme-dsn/texte/bloc-S21.G00.40.txt` lignes 936-943 ; N [4,6], deux decimales).
    if let Some(d) = &p.deduction_forfaitaire {
        si_admise(f, nature_dsn, "S21.G00.40.023", Some(fmt_f(d.taux, 2)))?;
    }
    for &r in &["S21.G00.40.024", "S21.G00.40.026"] {
        f.add_opt(r, nat.code(r))?;
    }
    // LES EMPLOIS MULTIPLES ne sont plus une constante de la nature : 02 quand le salarie tient plusieurs emplois
    // dans la periode de paie (p. 199, `emplois_multiples`).
    f.add("S21.G00.40.036", emplois);
    f.add_opt("S21.G00.40.037", nat.code("S21.G00.40.037"))?;
    si_admise(f, nature_dsn, "S21.G00.40.039", nat.code("S21.G00.40.039"))?;
    if let Some((code, valeur)) = atmp_ecrit {
        si_admise(f, nature_dsn, "S21.G00.40.040", Some(code))?;
        if let Some(v) = valeur {
            si_admise(f, nature_dsn, "S21.G00.40.043", Some(fmt_f(v, 2)))?;
        }
    }
    licence_du_contrat(f, nature_dsn, ent, nat.ecrit("S21.G00.40.050"))?;
    // Le numero d'objet du spectacle : rubrique conditionnelle (usage « C », `norme-dsn/texte/tableaux-des-usages.txt`
    // l. 270), seulement AUTORISEE pour les natures 02 et 10 (CCH-11, `texte/bloc-S21.G00.40.txt` l. 1379-1388) ; une
    // rubrique conditionnelle qu'on n'a pas s'omet, elle ne s'ecrit pas a blanc (`texte/chapitre-3-structuration.txt`
    // l. 257-259). Utile, jamais exige : « Pour demarrer » le propose (`sinon` de `champs-base.json`).
    let objet = p.spectacle.as_ref().and_then(|s| s.numero_objet.dite()).map(|o| o.trim()).filter(|o| !o.is_empty());
    if let Some(objet) = objet.filter(|_| nat.ecrit("S21.G00.40.050")) {
        if usage_admet(f, "S21.G00.40.050", nature_dsn)? {
            f.add("S21.G00.40.050", objet);
        }
    }
    si_admise(f, nature_dsn, "S21.G00.40.051", nat.code("S21.G00.40.051"))?;
    if let Some(b) = s.statut_boeth.dite() {
        si_admise(f, nature_dsn, "S21.G00.40.072", Some(b.as_str().to_string()))?;
    }
    proratisation_du_plafond(f, p, nature_dsn)?;
    // S21.G00.40.086 (« Adhesion a une caisse de conges payes a titre obligatoire ») ne s'ecrit pas : « A ne pas
    // renseigner pour la version de norme P27V01 » (journal de maintenance 2027.1.2, evolution 17,
    // `atelier/reference/norme-dsn/dsn-journal-maintenance-2027.1.2-20260615.txt` l. 368-381), absente de P26V01. Le lot
    // V4b l'ecrivait sur le cahier 2027.1.1 ; `norme::verifier_les_valeurs` la refuse (`rubriques_a_ne_pas_renseigner`).
    for a in arrets {
        bloc_60(f, a, nature_dsn, ent.banque.dite())?;
    }
    if let Some(fin) = fin_fctu {
        crate::fctu::bloc_62_63(f, fin, e)?;
    } else if e.dernier_mois() && fin_de_contrat {
        let fin = e.fin.ok_or_else(|| refus("S21.G00.62.001 : le contrat n'a pas de fin."))?;
        f.add("S21.G00.62.001", fin.jjmmaaaa());
        let motif = motif_rupture(p, &nat)?;
        f.ecrire_code(motif)?;
        // Le meme jour que le FCTU du contrat (`fctu::dernier_jour_paye`, cahier technique DSN p. 275 et § 2.5.2 p. 68).
        f.add("S21.G00.62.006", crate::fctu::dernier_jour_paye(e)?.jjmmaaaa());
        // 62.017 d'un CDDU suit le motif : 01 (circuit derogatoire) pour le 031, 02 sinon (CCH-12, `Nature::codes_fin`).
        for (rub, val) in nat.codes_fin(Some(motif)) {
            f.add(&rub, val);
        }
    }
    for s in &e.suspensions {
        suspension_sans_maintien(f, p, s)?;
        f.add("S21.G00.65.001", s.motif.clone());
        f.add("S21.G00.65.002", s.debut.jjmmaaaa());
        f.add("S21.G00.65.003", s.fin.jjmmaaaa());
    }
    let places = places_par_contrat(bar);
    let refs = affiliations(orgs, &nat, &bar.contributions_du_mois, (e.debut_periode(), e.fin_periode()))?;
    for a in rattachements(p, orgs, &bar.entreprise.fiche, refs)? {
        if let Some(o) = a.option {
            si_admise(f, nature_dsn, "S21.G00.70.004", Some(o))?;
        }
        if let Some((conjoint, enfants)) = a.ayants_droit {
            si_admise(f, nature_dsn, "S21.G00.70.008", Some(conjoint.to_string()))?;
            si_admise(f, nature_dsn, "S21.G00.70.011", Some(enfants.to_string()))?;
        }
        si_admise(f, nature_dsn, "S21.G00.70.012", Some((202 + places * (rang - 1) + a.place as i64).to_string()))?;
        f.add("S21.G00.70.013", a.reference);
        if let Some(d) = a.debut {
            si_admise(f, nature_dsn, "S21.G00.70.014", Some(d.jjmmaaaa()))?;
        }
    }
    si_admise(f, nature_dsn, "S21.G00.71.002", nat.code("S21.G00.71.002"))?;
    Ok(())
}

/// Bloc S21.G00.65 en norme 2027 : « absence a temps plein du salarie sans maintien de sa remuneration normale par son
/// employeur » (la regle `bloc_65_sans_maintien_de_remuneration` de la version, texte dans `norme-dsn-base.json`). Le
/// moteur tire le bloc d'une suspension saisie comme imprevu (`imprevus::suspensions_du_mois`) ; un motif que
/// `permanents-base.json` dit remunere (637, conge pour evenement familial : C. trav. L. 3142-2) ne retire rien du salaire,
/// et la norme 2027 en fait une suspension qui ne se declare pas : refus, plutot que de taire un imprevu saisi. Ni le bloc 75
/// (liste p. 297) ni un autre ne recoit un conge paye : il est dans la remuneration du mois.
fn suspension_sans_maintien(f: &Fichier, p: &Paie, s: &engagement::imprevus::Suspension) -> R<()> {
    let Some(texte) = f.norme()?.regle("bloc_65_sans_maintien_de_remuneration")? else { return Ok(()) };
    let motif = s.motif;
    let Some(m) = droit::permanents::motif_de_suspension(motif)?.filter(|m| m.remunere) else { return Ok(()) };
    Err(refus!(
        "{} : suspension au motif {} ({}), du {} au {}, saisie comme imprevu. Ce motif est un conge remunere \
({}). En norme {}, le bloc S21.G00.65 ne declare qu'une << absence a temps plein du \
salarie sans maintien de sa remuneration normale par son employeur >> : {} Un conge pour evenement familial se paie (le moteur \
ne retire rien du salaire) et ne se declare pas en suspension : retirer l'imprevu ; s'il n'est vraiment pas paye, le motif est a revoir. \
Rien n'est ecrit.",
        qui_contrat(p),
        motif,
        m.libelle,
        s.debut.iso(),
        s.fin.iso(),
        m.texte(),
        f.norme()?.version,
        texte
    ))
}

/// S21.G00.40.012 en norme 2027 : la quotite de reference « ne doit pas inclure les heures supplementaires
/// structurelles » (la regle de la version, `Norme::regle`, texte dans `norme-dsn-base.json`). En heures, au-dela de la
/// duree legale (35 heures par semaine, C. trav. L. 3121-27 : 151,67 h par mois, `quotite_reference_defaut` de la
/// nature), une heure est une heure supplementaire (L. 3121-28) : une quotite de reference plus haute en inclut, et
/// refuse. Une duree collective plus courte (32 h) passe. Un contrat d'usage (en jours) n'est pas concerne.
fn quotite_de_reference_sans_heures_supplementaires(f: &Fichier, p: &Paie, nat: &droit::natures::Nature, reference: Decimal) -> R<()> {
    let Some(texte) = f.norme()?.regle("quotite_reference_sans_heures_supplementaires")? else { return Ok(()) };
    let Some(legale) = nat.quotite_reference_defaut().filter(|_| nat.mensuelle()) else { return Ok(()) };
    if reference <= legale {
        return Ok(());
    }
    Err(refus!(
        "{} : quotite de reference de {} h (S21.G00.40.012, la durée de référence de la fiche entreprise ou de l'engagement), \
au-dessus de la duree legale ({} h par mois, 35 heures par semaine). En norme {}, elle << ne doit pas inclure les heures \
supplementaires structurelles >> : {} Declarer la duree collective sans les heures supplementaires (une compagnie a 39 h \
declare {} h ; ses quatre heures sont des heures supplementaires, payees comme telles). Rien n'est ecrit.",
        qui_contrat(p),
        qt(reference),
        qt(legale),
        f.norme()?.version,
        texte,
        qt(legale)
    ))
}

/// S21.G00.40.084, « Application de la proratisation du Plafond de Securite Sociale a hauteur de la quotite de
/// travail ». Norme 2027.1.2 (note differentielle CT 2026.1.2 -> 2027.1.2, § 4.5,
/// `atelier/reference/norme-dsn/dsn-note-differentielle-ct-2026.1.2-vers-2027.1.2-20251224.txt` l. 1737-1790) : « Pour un
/// salarie a temps partiel, cette rubrique doit etre renseignee avec le choix de l'employeur de proratiser le plafond
/// [...]. Pour un salarie en forfait jour reduit, [...] avec le choix du salarie » ; le journal de maintenance 2027.1.2
/// (`dsn-journal-maintenance-2027.1.2-20260615.txt`) la rend CONDITIONNELLE en DSN mensuelle (evolution 21, l. 433-452 :
/// O -> C) et retire sa CCH-12 (evolution 16, l. 347-366) : elle ne s'ecrit plus pour un temps plein ni pour un contrat
/// d'usage. Interdite en FCTU (I), conditionnelle en amorcage (C),
/// absente de la table P26V01 : la table de la version du fichier decide. Elle dit ce que le calcul a fait, tel que la
/// paie le porte (`Contenu::plafond_temps_partiel`) :
/// - temps partiel, plafond proratise (la regle, fiche net-entreprises 3369) : 01 ;
/// - temps partiel, l'etablissement renonce (fiche entreprise, `plafond_temps_partiel` = « entier ») : 02 (fiche 3369) ;
/// - temps partiel d'un artiste, exclu de la proratisation (L. 242-10, 1° ; BOSS § 1090) : 02. INFERENCE : la norme ne
///   dit rien d'un salarie que la loi exclut ; 02 (« Non ») est ce que le calcul fait, le plafond entier ;
/// - temps plein, contrat d'usage : rien.
/// Le forfait jour reduit (40.011 = 20) n'existe pas dans le moteur (aucune nature ne l'a) : un cas a porter.
fn proratisation_du_plafond(f: &mut Fichier, p: &Paie, nature_dsn: NatureDeLaDeclaration) -> R<()> {
    use engagement::calcul::PlafondTempsPartiel as P;
    let r = "S21.G00.40.084";
    if !usage_admet(f, r, nature_dsn)? {
        return Ok(());
    }
    let valeur = match p.plafond_temps_partiel {
        P::Proratise => Some("01"),
        P::Entier | P::Exclu => Some("02"),
        P::NonConcerne => None,
    };
    if let Some(v) = valeur {
        f.add(r, v);
    }
    Ok(())
}

/// Le bloc S21.G00.86 : l'anciennete dans l'entreprise, en jours.
pub fn bloc_anciennete(f: &mut Fichier, p: &Paie, jours: i64) -> R<()> {
    let nat = p.engagement.nat()?;
    f.add_opt("S21.G00.86.001", nat.code("S21.G00.86.001"))?;
    f.add_opt("S21.G00.86.002", nat.code("S21.G00.86.002"))?;
    f.add("S21.G00.86.003", jours);
    f.add_opt("S21.G00.86.005", p.engagement.numero.clone())
}

/// (taux, type, identifiant, montant soumis) d'UN versement. L'abattement contrats courts n'a pas de rubrique : il se
/// lit dans l'ecart entre 50.002 et 50.013 (norme 2026.1, p. 245). S21.G00.50.012 est reservee aux abattements des
/// assistants maternels et familiaux, que seuls PAJEmploi et les departements declarent : le moteur ne l'ecrit pas.
fn pas_du_versement(
    taux_pas: Option<&Triplet>, bar: &Pile, montant: Decimal, le: Jour, court: bool,
) -> R<(droit::fiche::Montant, Option<droit::type_taux_pas::TypeDeTauxPas>, Option<String>, Decimal)> {
    let r = pas::du_versement(taux_pas, bar, montant, Some(le), court, ZONE_DEFAUT)?;
    Ok((r.taux, r.type_taux, r.identifiant, r.montant_soumis))
}

pub fn court(p: &Paie, le: Jour) -> R<bool> {
    let e = &p.engagement;
    contrat_court(&*e.nat()?, e.debut, e.fin, le)
}

/// Les quatre blocs 51 d'une paie, et les activites 53 sous le 002.
pub fn remunerations(f: &mut Fichier, p: &Paie, debut: &Jour, fin: &Jour, signe: i64) -> R<()> {
    let e = &p.engagement;
    let nat = e.nat()?;
    for &t in nat.types_remuneration() {
        let montant = match t {
            TypeDeRemuneration::Brute => brut_du_51(p),
            TypeDeRemuneration::AssietteChomage => p.assiette_chomage,
            TypeDeRemuneration::SalaireRetabli => p.salaire_retabli,
            TypeDeRemuneration::SalaireDeBase => p.salaire_de_base,
        };
        f.add("S21.G00.51.001", debut.jjmmaaaa());
        f.add("S21.G00.51.002", fin.jjmmaaaa());
        f.add_opt("S21.G00.51.010", e.numero.clone())?;
        f.add("S21.G00.51.011", t.code());
        f.add("S21.G00.51.013", ms(montant, signe));
        if t == TypeDeRemuneration::AssietteChomage {
            activites(f, p, signe)?;
        }
    }
    // LES HEURES EN PLUS d'un salarie paye au mois (`engagement::heures_en_plus`) : « 017 - Heures supplémentaires ou
    // complémentaires aléatoires », leurs heures en 51.012 (SIG-11 : obligatoire pour ce type) et leur remuneration en
    // 51.013, datees de la periode d'emploi (fiche net-entreprises 2066, `atelier/reference/sources/
    // net-entreprises-base-connaissances/net-entreprises-fiche2066-heures-supplementaires-exonerees-20251021.txt`).
    // UN MOIS D'ABSENCE : les deux termes du rapport qui corrige le SMIC de la reduction generale (CSS D. 241-7, IV ;
    // `engagement::calcul::smic_d_une_absence`), « 028 - Rémunération perçue hors éléments non affectés par l'absence »
    // et « 029 - Rémunération habituelle mois complet hors éléments non affectés par l'absence », « à déclarer en cas
    // d'absence » (fiche net-entreprises 2681, `atelier/reference/sources/net-entreprises-base-connaissances/
    // net-entreprises-fiche2681-reduction-generale-absence-proratisation-smic-types-028-029-20260609.txt`).
    if let (Some(percue), Some(habituelle)) =
        (p.bases.get(&Assiette::RemunerationPercueHorsEna), p.bases.get(&Assiette::RemunerationHabituelleHorsEna))
    {
        for (code, montant) in [("028", *percue), ("029", *habituelle)] {
            f.add("S21.G00.51.001", debut.jjmmaaaa());
            f.add("S21.G00.51.002", fin.jjmmaaaa());
            f.add_opt("S21.G00.51.010", e.numero.clone())?;
            f.add("S21.G00.51.011", code);
            f.add("S21.G00.51.013", ms(montant, signe));
        }
    }
    let lire = |a: Assiette| p.bases.get(&a).copied().filter(|m| !m.is_zero());
    if let (Some(montant), Some(heures)) = (lire(Assiette::HeuresEnPlus), lire(Assiette::HeuresEnPlusHeures)) {
        f.add("S21.G00.51.001", debut.jjmmaaaa());
        f.add("S21.G00.51.002", fin.jjmmaaaa());
        f.add_opt("S21.G00.51.010", e.numero.clone())?;
        f.add("S21.G00.51.011", "017");
        f.add("S21.G00.51.012", ms(heures, signe));
        f.add("S21.G00.51.013", ms(montant, signe));
    }
    Ok(())
}

/// Les mesures d'activite (bloc 53) d'une paie, dans l'ordre ou elles s'ecrivent : (53.001, quantite, 53.003).
pub fn mesures_d_activite(p: &Paie) -> R<Vec<(String, Decimal, String)>> {
    let e = &p.engagement;
    let nat = e.nat()?;
    let mut out = Vec::new();
    for u in nat.unites_activite() {
        let valeur = match u.mesure {
            Mesure::Jours => Decimal::from(e.jours),
            Mesure::Cachets => Decimal::from(e.cachets),
            // Les heures sont la somme de celles des lignes (ARCHITECTURE, « Heures, cachets et jours ») : sans ligne en
            // heures, aucune.
            Mesure::Heures => e.heures.iter().filter(|h| !h.is_zero()).fold(Decimal::ZERO, |a, h| a + *h),
        };
        if u.si_non_nul == Some(true) && valeur.is_zero() {
            continue;
        }
        // Une unite que seule la caisse des Conges Spectacles demande (`contrats-base.json`, `si_caisse_conges`).
        if u.si_caisse_conges == Some(true) && !p.declaree_a_la_caisse_des_conges()? {
            continue;
        }
        out.push((u.type_activite.clone(), valeur, u.code.clone()));
    }
    if let Some(a) = nat.unite_absence() {
        if !p.heures_absence.is_zero() {
            out.push((a.type_activite.clone(), p.heures_absence, a.code.clone()));
        }
    }
    Ok(out)
}

fn activites(f: &mut Fichier, p: &Paie, signe: i64) -> R<()> {
    for (t, valeur, code) in mesures_d_activite(p)? {
        f.add("S21.G00.53.001", t);
        f.add("S21.G00.53.002", ms(valeur, signe));
        f.add("S21.G00.53.003", code);
    }
    Ok(())
}

/// Le bloc 52 : les primes et indemnites qui ont leur code.
pub fn primes_et_indemnites(f: &mut Fichier, p: &Paie, debut: &Jour, fin: &Jour, signe: i64) -> R<()> {
    for l in &p.remuneration {
        let code = match l.dsn_bloc52 {
            Some(c) => c,
            None => continue,
        };
        if let Some(m) = p.engagement.motif_rupture {
            if let Some(pourquoi) = droit::permanents::code_52_permis(Some(code), Some(m))? {
                return Err(refus!(
                    "Contrat {}, ligne << {} >> : {}",
                    p.engagement.numero_declare()?,
                    l.libelle,
                    pourquoi
                ));
            }
        }
        f.add("S21.G00.52.001", code);
        f.add("S21.G00.52.002", ms(l.montant, signe));
        f.add("S21.G00.52.003", debut.jjmmaaaa());
        f.add("S21.G00.52.004", fin.jjmmaaaa());
        f.add_opt("S21.G00.52.006", p.engagement.numero.clone())?;
    }
    Ok(())
}

/// Les blocs 54 : les lignes qui portent `dsn_autres_bruts`, puis 92 et 93.
pub fn autre_revenu(f: &mut Fichier, p: &Paie, bar: &BaremeDuMois, debut: &Jour, fin: &Jour, signe: i64) -> R<()> {
    let e = &p.engagement;
    for l in &p.remuneration {
        let Some(code) = l.dsn_autres_bruts else { continue };
        if l.montant.is_zero() {
            continue;
        }
        f.ecrire_code(code)?;
        f.add("S21.G00.54.002", ms(l.montant, signe));
        f.add("S21.G00.54.003", debut.jjmmaaaa());
        f.add("S21.G00.54.004", fin.jjmmaaaa());
        f.add_opt("S21.G00.54.005", e.numero.clone())?;
    }
    // « 10 - Deduction forfaitaire specifique » (bloc 54, p. 257, `bloc-S21.G00.54.txt` ligne 56) : le bloc 54 « vehicule
    // [...] frais professionnels (y compris le montant de la Deduction Forfaitaire Specifique si applicable) » (chapitre
    // 2, p. 27, lignes 461-463), par contrat (CCH-16 de 50.001, p. 243, `bloc-S21.G00.50.txt` lignes 82-86). Le montant
    // est ce que la deduction retire du brut soumis : le brut moins le brut abattu du calcul (`brut_dfs`).
    if p.deduction_forfaitaire.is_some() {
        let deduction = sub(p.brut, brut_du_51(p));
        if !deduction.is_zero() {
            f.ecrire_code(AutreElementDeRevenuBrut::DeductionForfaitaireSpecifique)?;
            f.add("S21.G00.54.002", ms(deduction, signe));
            f.add("S21.G00.54.003", debut.jjmmaaaa());
            f.add("S21.G00.54.004", fin.jjmmaaaa());
            f.add_opt("S21.G00.54.005", e.numero.clone())?;
        }
    }
    let bp = bareme_de(p, bar)?;
    for (type_54, supp) in [
        (AutreElementDeRevenuBrut::CotisationPatronaleSante, &bp.cotisation_sante_patronale),
        (AutreElementDeRevenuBrut::CotisationPrevoyanceRetraiteSupplementaire, &bp.cotisation_prevoyance_retraite_supp),
    ] {
        let Some(supp) = supp else { continue };
        let montant = match supp {
            CotisationSupplementaire::DesLignes(l) => somme_lignes(p, &l.lignes_patronales, true),
            // Le taux couvre la prevoyance du statut ; une garantie qu'une couche range avec elle (la tranche B d'un
            // cadre technicien de la CCNEAC, XII.2.2.2) est aussi « incluse dans la base CSG » : sa part patronale s'ajoute.
            CotisationSupplementaire::SurUneBase(t) => {
                let sur_la_base = pct(base(&bases(p, &bp)?, t.base)?, t.taux_sur_la_base.0);
                if type_54 == AutreElementDeRevenuBrut::CotisationPrevoyanceRetraiteSupplementaire {
                    let ajoutees: Vec<IdDeCotisation> = bp.garanties_de_prevoyance().into_iter().filter(|g| *g != PREVOYANCE).collect();
                    add(sur_la_base, somme_lignes(p, &ajoutees, true))
                } else {
                    sur_la_base
                }
            }
        };
        let montant = rond(montant);
        if montant.is_zero() {
            continue;
        }
        f.ecrire_code(type_54)?;
        f.add("S21.G00.54.002", ms(montant, signe));
        f.add("S21.G00.54.003", debut.jjmmaaaa());
        f.add("S21.G00.54.004", fin.jjmmaaaa());
        f.add_opt("S21.G00.54.005", e.numero.clone())?;
    }
    Ok(())
}

/// Les dates (58.001, 58.002) d'un element de revenu calcule en net rattache a la periode [debut, fin] et verse le
/// `versement`, quand elles s'ecrivent : celles d'une correction (le 58 d'un mois anterieur, ou le rappel d'une
/// correction, `blocs_58_de_la_correction`). Norme 2026.1 (p. 265) : « en periode de versement », la periode. Norme
/// 2027 (cahier 2027.1.1, p. 267-268, la regle `net_social_en_date_de_versement` de la version) : « en date de
/// versement », la date du versement deux fois (CCH-14 : l'une ne va pas sans l'autre ; la fiche net-entreprises 2983
/// admet aussi le mois du versement). Le cas nominal de 2027 ne les ecrit pas : `dates_du_58_du_mois`.
fn dates_du_58(f: &Fichier, debut: &Jour, fin: &Jour, versement: &Jour) -> R<(Jour, Jour)> {
    Ok(if f.norme()?.regle("net_social_en_date_de_versement")?.is_some() { (*versement, *versement) } else { (*debut, *fin) })
}

/// Les dates (58.001, 58.002) du 58 d'un versement du mois (la paie, ou le mois sans paie), ou `None` quand elles ne
/// s'ecrivent pas. Norme 2026.1 (p. 265) : la periode de versement, ecrite. Norme 2027 : la fiche net-entreprises 2983
/// (modifiee le 09/10/2026, `atelier/reference/sources/net-entreprises-base-connaissances/
/// net-entreprises-fiche2983-montant-net-social-20261009.txt`, l. 23-25 et l. 69-77) : « Le bloc S21.G00.58 heritant de la
/// datation du bloc Versement individu - S21.G00.50 (parent par defaut), les rubriques de datation [58.001 et 58.002]
/// ne sont pas obligatoires » ; elles servent « en cas d'erreur ou d'ecart sur un mois precedent » ; l'exemple du cas
/// nominal les laisse « Non renseignee ». Le cahier 2027.1.1 (p. 267) : « a renseigner des lors que sa datation est
/// differente de celle du Versement individu ». Le type 13 suit (« La meme regle de datation s'applique aux revenus
/// declares pour les types 10, 13, 14 et 15 », p. 268).
fn dates_du_58_du_mois(f: &Fichier, debut: &Jour, fin: &Jour) -> R<Option<(Jour, Jour)>> {
    Ok(if f.norme()?.regle("net_social_en_date_de_versement")?.is_some() { None } else { Some((*debut, *fin)) })
}

fn dater_le_58(f: &mut Fichier, dates: Option<(Jour, Jour)>) {
    if let Some((d, fi)) = dates {
        f.add("S21.G00.58.001", d.jjmmaaaa());
        f.add("S21.G00.58.002", fi.jjmmaaaa());
    }
}

/// Norme 2027.1.1, S21.G00.58.003 type « 13 - Part exoneree de cotisations sociales des indemnites de rupture »
/// (p. 268 ; la regle `part_exoneree_des_indemnites_de_rupture` de la version) : la somme des indemnites que la paie
/// declare en bloc 52 (indemnites de rupture : 52.001 = 001, 003, 004, 007, 021...) et que le catalogue met hors de
/// l'assiette des cotisations (`soumis` faux, `logiciel/donnees/rubriques-base.json`). Le moteur exonere la ligne
/// entiere ou refuse (au-dela des seuils, l'indemnite n'est pas portee) : la part exoneree est le montant de la ligne.
/// `None` quand la version n'a pas le type ou que la paie n'a pas d'indemnite exoneree.
fn part_exoneree_des_indemnites_de_rupture(f: &Fichier, p: &Paie) -> R<Option<Decimal>> {
    if f.norme()?.regle("part_exoneree_des_indemnites_de_rupture")?.is_none() {
        return Ok(None);
    }
    let part = p
        .remuneration
        .iter()
        .filter(|l| !l.soumis && l.dsn_bloc52.is_some())
        .fold(Decimal::ZERO, |a, l| a + l.montant);
    Ok(if part.is_zero() { None } else { Some(part) })
}

/// Les blocs 58 de la paie : le type 03, le montant net social (`Paie::net_social`, calcule par `calcul::calculer`),
/// et en norme 2027 le type 13 (`part_exoneree_des_indemnites_de_rupture`), dates par `dates_du_58_du_mois` (en 2027 :
/// pas de dates, le 58 herite du versement).
pub fn net_social(f: &mut Fichier, p: &Paie, debut: &Jour, fin: &Jour, signe: i64) -> R<()> {
    let dates = dates_du_58_du_mois(f, debut, fin)?;
    // Le montant net des heures en plus exonere d'impot (CGI 81 quater), hors de la remuneration nette fiscale : « 01 -
    // Heure(s) complémentaire(s) ou supplémentaire(s) exonérée(s) fiscalement », date comme le versement (fiche
    // net-entreprises 2066 ; `Assiette::HeuresEnPlusExonere`).
    if let Some(exonere) = p.bases.get(&Assiette::HeuresEnPlusExonere).filter(|m| !m.is_zero()) {
        dater_le_58(f, dates);
        f.add("S21.G00.58.003", "01");
        f.add("S21.G00.58.004", ms(*exonere, signe));
    }
    dater_le_58(f, dates);
    f.add("S21.G00.58.003", "03");
    f.add("S21.G00.58.004", ms(p.net_social, signe));
    // Les indemnites journalieres que la compagnie subrogee reverse, comptees dans le montant net social : « 10 - IJSS
    // subrogees prises en compte dans le calcul du MNS » (S21.G00.58.003, `norme-dsn/texte/bloc-S21.G00.60.txt`, p. 265).
    if let Some(ij) = p.bases.get(&Assiette::IjSubrogeesNettes).filter(|m| !m.is_zero()) {
        dater_le_58(f, dates);
        f.add("S21.G00.58.003", "10");
        f.add("S21.G00.58.004", ms(*ij, signe));
    }
    if let Some(part) = part_exoneree_des_indemnites_de_rupture(f, p)? {
        dater_le_58(f, dates);
        f.add("S21.G00.58.003", "13");
        f.add("S21.G00.58.004", ms(part, signe));
    }
    Ok(())
}

/// Les blocs 78, 79 et 81 d'une paie.
pub fn bases_et_cotisations(
    f: &mut Fichier, p: &Paie, bar: &BaremeDuMois, rang: i64, debut: &Jour, fin: &Jour, signe: i64, nature_dsn: NatureDeLaDeclaration,
    bases_exclues: &[BaseAssujettie],
) -> R<()> {
    let e = &p.engagement;
    let mut courant: Option<(BaseAssujettie, Option<String>, Decimal)> = None;
    let places = places_par_contrat(bar);
    let composants = usage_admet(f, "S21.G00.79.001", nature_dsn)?;
    for mut c in cotisations(p, bar)? {
        if bases_exclues.contains(&c.bloc78) {
            continue;
        }
        if let Some(a) = c.affectation.as_deref().filter(|a| !a.is_empty()) {
            let n: i64 = a.trim().parse().map_err(|_| refus!("affectation illisible : {}", a))?;
            c.affectation = Some((n + places * (rang - 1)).to_string());
        }
        let cle = (c.bloc78, c.affectation.clone(), c.base78);
        if courant.as_ref() != Some(&cle) {
            courant = Some(cle);
            f.ecrire_code(c.bloc78)?;
            f.add("S21.G00.78.002", debut.jjmmaaaa());
            f.add("S21.G00.78.003", fin.jjmmaaaa());
            f.add("S21.G00.78.004", ms(c.base78, signe));
            if let Some(a) = c.affectation.as_deref().filter(|a| !a.is_empty()) {
                si_admise(f, nature_dsn, "S21.G00.78.005", Some(a.to_string()))?;
            }
            if !f.contrat_unique {
                si_admise(f, nature_dsn, "S21.G00.78.006", e.numero.clone().map(String::from))?;
            }
            if !composants {
            } else if c.bloc78 == BaseAssujettie::BruteDeplafonnee {
                f.add("S21.G00.79.001", "01");
                // Le SMIC de reference de la reduction generale, sinon le plafond de la periode.
                let v = match p.reduction_generale.as_ref() {
                    Some(rg) => rg.smic_reference,
                    None => p.plafond,
                };
                f.add("S21.G00.79.004", ms(v, signe));
            } else if c.bloc78 == BaseAssujettie::PrevoyanceSanteRetraite {
                // Le composant que la source du taux donne (`composant79` : la sante des permanents 3090, base forfaitaire
                // 18 sur le plafond du mois, fiche Audiens 139), sinon la tranche A sur le brut.
                let (type_79, montant) = match c.composant79.clone() {
                    Some(composant) => composant,
                    None => (TRANCHE_A.to_string(), p.brut),
                };
                f.add("S21.G00.79.001", type_79);
                f.add("S21.G00.79.004", ms(montant, signe));
            }
        }
        let montant = match c.montant {
            Some(m) => m,
            None => continue,
        };
        let org = c.org.clone();
        // La CET patronale : « 146 » en P26V01 (guide Urssaf du 04/06/2026, p. 38) ; « 148 » en P27V01, le 146 y etant
        // « hors CET » (fiche net-entreprises 2556, l. 21 et 83-87). La version en decide (`cotisation_cet_patronale`).
        let code81 = if c.cet { f.norme()?.cotisation_cet_patronale()? } else { code_declare(&c)? };
        f.ecrire_code(code81)?;
        if let Some(o) = &org {
            f.add("S21.G00.81.002", o);
        }
        if org.is_some() || c.cle_total.is_some() {
            f.add("S21.G00.81.003", ms(c.base81.ok_or_else(|| socle::fichiers::absent("base81"))?, signe));
        }
        f.add("S21.G00.81.004", ms(montant, signe));
        if let Some(insee) = c.code_insee.as_deref() {
            f.add("S21.G00.81.005", insee);
        }
        if let (Some(_), Some(t)) = (&org, c.taux) {
            f.add("S21.G00.81.007", fmt_f(t, 3));
        }
    }
    Ok(())
}

/// S21.G00.50.003, le numero de versement : « Numéro valorisé par le débiteur permettant de distinguer deux versements au
/// même salarié » ; CCH-11 : « Pour un même bloc "Individu - S21.G00.30", il est interdit de déclarer plusieurs blocs
/// « Versement individu - S21.G00.50 » avec un même "Numéro de versement" » ; N [2,2], CSL 00 `[0]*[1-9][0-9]*` (norme
/// DSN 2026.1, bloc 50, p. 243-244 : `atelier/reference/norme-dsn/texte/bloc-S21.G00.50.txt`, lignes 111-126). La norme ne
/// dit rien de l'ordre : `n` est le rang du versement sous l'individu dans ce fichier, les versements du mois d'abord, puis
/// ses regularisations (`generer`). Ce n'est pas le rang du contrat, qui numerote les affiliations (70.012, 78.005) : une
/// regularisation d'un contrat aussi paye ce mois a le rang de ce contrat, et un numero de versement a elle.
fn numero_de_versement(n: i64) -> R<String> {
    if !(1..=99).contains(&n) {
        return Err(refus!(
            "Un individu porte un {}e versement dans cette DSN : le numero de versement (S21.G00.50.003) s'ecrit sur deux \
chiffres, de 01 a 99 (norme DSN 2026.1, p. 244), et deux versements du meme individu n'ont jamais le meme numero (CCH-11, \
p. 243). Porter une partie des corrections sur une DSN suivante. Rien n'est ecrit.",
            n
        ));
    }
    Ok(format!("{:02}", n))
}

fn bloc_versement(f: &mut Fichier, p: &Paie, bar: &BaremeDuMois, rang: i64, taux_pas: Option<&Triplet>) -> R<()> {
    let e = &p.engagement;
    let debut = e.debut_periode();
    let fin = e.fin_periode();
    // « Date à laquelle le débiteur effectue le versement » (`bloc-S21.G00.50.txt`, l. 50-55 ; CCH-13, l. 61-62 : au plus
    // tard le dernier jour du mois suivant) : le jour ou se fait la retenue, celui du taux de PAS de la paie.
    f.add("S21.G00.50.001", e.date_du_versement()?.jjmmaaaa());
    f.add("S21.G00.50.002", mt(p.net_imposable));
    // Le versement du mois a le rang de son contrat : un contrat paye ce mois, un versement (`generer`).
    f.add("S21.G00.50.003", numero_de_versement(rang)?);
    f.add("S21.G00.50.004", mt(p.net_verse()));
    // Le prelevement d'une paie validee est celui qu'elle a ecrit (`Paie::pas`) : la DSN ne le recalcule pas. Un mois
    // sans paie n'en a pas : son versement nul porte le taux qui vaut pour le salarie.
    let (taux, type_taux, identifiant, soumis) = match &p.pas {
        Some(v) => (v.taux.clone(), v.type_taux.clone(), v.identifiant.clone(), v.montant_soumis),
        None if e.sans_paie() => pas_du_versement(taux_pas, bar, p.net_imposable, fin, court(p, fin)?)?,
        None => {
            return Err(socle::echec::cle(format!(
                "La paie de {} pour {} ne porte pas de prelevement a la source. Revalider l'engagement.",
                p.salarie.id.to_string(),
                e.mois
            ), "pas"))
        }
    };
    f.add("S21.G00.50.006", taux.ecrit());
    let type_taux = type_taux.ok_or_else(|| absente("S21.G00.50.007"))?;
    f.ecrire_code(type_taux)?;
    if type_taux.transmis() {
        f.add_ecrit("S21.G00.50.008", identifiant.clone())?;
    }
    f.add("S21.G00.50.009", mt(pct(soumis, taux.dec()?)));
    f.add("S21.G00.50.013", mt(soumis));

    // Un mois decoupe : les remunerations de chaque periode elementaire, datees de ses bornes (« La periode a
    // declarer dans le bloc Remuneration est cette periode elementaire de paie, lorsque le logiciel de paie produit
    // effectivement un decoupage », norme 2026.1, § 2.1.3.2, p. 28 ; les mesures d'activite aussi, § 2.1.3.5, p. 29).
    for q in p.elementaires() {
        remunerations(f, q, &q.engagement.debut_periode(), &q.engagement.fin_periode(), 1)?;
    }
    primes_et_indemnites(f, p, &debut, &fin, 1)?;
    if e.sans_paie() {
        let dates = dates_du_58_du_mois(f, &debut, &fin)?;
        dater_le_58(f, dates);
        f.add("S21.G00.58.003", "03");
        f.add("S21.G00.58.004", "0.00");
        let mut vues: Vec<Option<String>> = Vec::new();
        let places = places_par_contrat(bar);
        for c in cotisations(p, bar)? {
            if c.bloc78 != BaseAssujettie::PrevoyanceSanteRetraite || vues.contains(&c.affectation) {
                continue;
            }
            vues.push(c.affectation.clone());
            f.ecrire_code(c.bloc78)?;
            f.add("S21.G00.78.002", debut.jjmmaaaa());
            f.add("S21.G00.78.003", fin.jjmmaaaa());
            f.add("S21.G00.78.004", "0.00");
            if let Some(a) = c.affectation.as_deref().filter(|a| !a.is_empty()) {
                let n: i64 = a.trim().parse().map_err(|_| refus!("affectation illisible : {}", a))?;
                f.add("S21.G00.78.005", n + places * (rang - 1));
            }
            if !f.contrat_unique {
                f.add_opt("S21.G00.78.006", e.numero.clone())?;
            }
            f.add("S21.G00.79.001", match c.composant79.as_ref() {
                Some((t, _)) => t.as_str(),
                None => TRANCHE_A,
            });
            f.add("S21.G00.79.004", "0.00");
            f.ecrire_code(code_declare(&c)?)?;
            f.add("S21.G00.81.004", "0.00");
        }
        return Ok(());
    }
    autre_revenu(f, p, bar, &debut, &fin, 1)?;
    // Le net social de la periode (le versement est date par 50.001 ci-dessus).
    net_social(f, p, &debut, &fin, 1)?;
    // Les bases d'un mois decoupe, periode elementaire par periode elementaire, datees de ses bornes (78.002, 78.003) :
    // « Pour tout changement intervenant en cours de mois, il convient de ventiler les bases assujetties par periodes »
    // (fiche net-entreprises 1265, l. 52) ; FAQ Agirc-Arrco « Changement de situation en cours de mois ».
    for q in p.elementaires() {
        bases_et_cotisations(f, q, bar, rang, &q.engagement.debut_periode(), &q.engagement.fin_periode(), 1, NatureDeLaDeclaration::Mensuelle, &[])?;
    }
    Ok(())
}

/// Un versement tel que la mensuelle l'ecrit pour un contrat : les affiliations du contrat (S21.G00.70.012, 70.013) et
/// les rubriques de son versement (blocs 50 a 81), dans l'ordre ou elles s'ecrivent.
pub struct VersementEcrit {
    pub affiliations: Vec<(String, String)>,
    pub rubriques: Vec<(String, String)>,
}

/// CE QU'UNE DSN A DECLARE POUR UN CONTRAT, ecrit depuis la paie qu'elle a declaree (`engagement::etats::paies_declarees`)
/// par les fonctions qui ecrivent la mensuelle de `mois` : ses affiliations (`rattachements`) et son versement
/// (`bloc_versement`), qu'une regularisation annule (`versement_declare::de_la_paie`).
pub fn versement_ecrit(ctx: &Contexte, p: &Paie, mois: Mois) -> R<VersementEcrit> {
    let ctx = &ctx.du_mois(mois)?;
    let ent = &ctx.entreprise;
    let nic = bareme::etablissement_de(ent, &p.salarie)?;
    let orgs = ctx.organismes()?;
    let bar = bar_du(ctx, ent, &orgs, &nic, mois)?;
    let e = &p.engagement;
    let refs = affiliations(&orgs, &*e.nat()?, &bar.contributions_du_mois, (e.debut_periode(), e.fin_periode()))?;
    let affiliations = rattachements(p, &orgs, &bar.entreprise.fiche, refs)?
        .into_iter()
        .map(|a| ((202 + a.place as i64).to_string(), a.reference))
        .collect();
    let mut f = Fichier::en_norme(crate::norme::de_la_mensuelle(mois)?);
    bloc_versement(&mut f, p, &bar, 1, None)?;
    Ok(VersementEcrit { affiliations, rubriques: f.rubriques() })
}

/// Les affiliations que CE fichier a deja ecrites pour le contrat `numero` de l'individu en cours :
/// (S21.G00.70.013, S21.G00.70.012).
fn affiliations_ecrites(f: &Fichier, numero: &str) -> Vec<(String, String)> {
    // L'individu en cours commence a son NIR ; sans lui, le fichier entier.
    let lignes = match f.lignes.iter().rposition(|l| l.starts_with("S21.G00.30.001,")) {
        Some(depart) => &f.lignes[depart..],
        None => &f.lignes[..],
    };
    let mut out = Vec::new();
    let (mut du_contrat, mut id) = (false, None::<String>);
    for l in lignes {
        let Some((k, v)) = l.split_once(',') else { continue };
        let v = v.trim_matches('\'');
        match k {
            "S21.G00.40.001" | "S21.G00.50.001" => du_contrat = false,
            "S21.G00.40.009" => du_contrat = v == numero,
            "S21.G00.70.012" if du_contrat => id = Some(v.to_string()),
            "S21.G00.70.013" if du_contrat => {
                if let Some(i) = id.take() {
                    out.push((v.to_string(), i));
                }
            }
            _ => {}
        }
    }
    out
}

/// L'annulation de ce qui a ete DECLARE : les lignes des `blocs` du versement declare, montants
/// changes de signe, le reste tel quel. 78.005 cite une affiliation (70.012) du versement declare : elle se retrouve dans
/// ce fichier par sa reference (70.013), son numero n'y est pas forcement le meme (le rang du contrat change).
fn annuler_le_declare(f: &mut Fichier, r: &Regularisation, blocs: &[&str]) -> R<()> {
    let d = &r.declare;
    let numero = r.apres.engagement.numero.clone().ok_or_else(|| absente("S21.G00.40.009"))?;
    let ecrites = affiliations_ecrites(f, &numero);
    let lignes: Vec<(String, String)> = d.blocs(blocs).cloned().collect();
    for (k, v) in lignes {
        if k == "S21.G00.78.005" {
            let reference = d.affiliations.iter().find(|(id, _)| *id == v).map(|(_, rf)| rf.clone()).ok_or_else(|| {
                refus!(
                    "Correction de {} (contrat {}) : {} cite l'affiliation {} en S21.G00.78.005 sans la \
declarer dans les blocs 70 du contrat. L'annulation ne sait pas a quoi la rattacher. Rien n'est ecrit.",
                    r.mois, numero, d.fichier, v
                )
            })?;
            let id = ecrites.iter().find(|(rf, _)| *rf == reference).map(|(_, i)| i.clone()).ok_or_else(|| {
                refus!(
                    "Correction de {} (contrat {}) : {} rattachait une base a l'adhesion {} \
(S21.G00.70.013), que le contrat ne declare plus dans cette DSN. L'annulation ne sait pas a quelle affiliation la \
rattacher. Rien n'est ecrit.",
                    r.mois, numero, d.fichier, reference
                )
            })?;
            f.add(&k, id);
        } else if crate::versement_declare::est_un_montant(&k) == Some(true) {
            let m: Decimal = v.parse().map_err(|_| crate::versement_declare::illisible(&k, &v, &d.fichier))?;
            f.add(&k, ms(m, -1));
        } else {
            f.add(&k, v);
        }
    }
    Ok(())
}

/// Les blocs 58 de type 03 (montant net social) du versement d'une correction. Norme DSN 2026.1, S21.G00.58.003
/// (`atelier/reference/norme-dsn/texte/bloc-S21.G00.60.txt`, l. 14-18, p. 265) : pour le montant net social, « la date de
/// debut (S21.G00.58.001) et la date de fin (S21.G00.58.002) de periode de rattachement sont a renseigner en periode
/// de versement ». BOSS, montant net social, Q 35 et Q 41 (`atelier/reference/sources/montant-net-social/
/// boss-montant-net-social-20261001.txt`, l. 476-492 et 552-571) : un rappel ou une reprise se prend dans le montant net
/// social « du mois lors duquel ce rappel ou ce versement est effectue » ; si seul le montant net social declare etait
/// faux, la correction se rattache a la periode d'emploi concernee, « en precisant quel etait le montant net social
/// effectivement verse au titre de cette periode ». D'ou deux parts :
///
/// - **ce qui a ete verse ne change pas** (`apres.net_a_payer` = le net deja verse, lu sur le bulletin) : seul le
///   declare peut etre faux ; l'annulation du 58 depose et le 58 recalcule, aux dates de la periode d'origine (la
///   periode de son versement d'origine) ;
/// - **la correction verse ou reprend** (net a payer different) : le montant net social de ce qui a ete paye est le
///   net deja verse moins ce que la paie d'origine payait hors du montant net social (remboursements de frais,
///   acompte : `avant.net_a_payer - avant.net_social`, qui ne dependent que des lignes saisies). S'il differe du
///   declare (un depot qui portait le net a payer), le 58 depose s'annule et ce montant se declare,
///   aux dates d'origine ; la difference avec le montant net social corrige se declare aux dates du mois du versement,
///   du premier au dernier jour du mois de la correction (inference : la « periode de versement » est le mois dont le
///   versement porte la date, `S21.G00.50.001` = `fin_mois`).
///
/// Une correction deja portee a pu declarer un rappel aux dates de son propre mois (`declare.rappels_net_social`) :
/// le montant net social d'origine est ce qui a ete paye moins ces rappels.
fn blocs_58_de_la_correction(f: &mut Fichier, r: &Regularisation, debut: &Jour, fin: &Jour, fin_mois: Jour) -> R<()> {
    let (avant, apres) = (&r.avant, &r.apres);
    // Ce que des corrections deja portees ont declare aux dates du mois de leur versement : le montant net social de la
    // periode d'origine ne le reprend pas.
    let rappels = r.declare.rappels_net_social;
    // Norme 2027 : le type 13 (indemnites de rupture exonerees) se declare avec le montant net social ; une correction
    // qui le porterait (avant ou apres) aurait a l'annuler et le redeclarer avec lui, ce qui n'est pas porte : refus.
    if part_exoneree_des_indemnites_de_rupture(f, avant)?.is_some() || part_exoneree_des_indemnites_de_rupture(f, apres)?.is_some() {
        return Err(refus!(
            "Correction de {} (contrat {}) : la paie porte une indemnite de rupture hors de l'assiette des cotisations, que \
la norme {} declare en S21.G00.58 de type 13 (<< Part exoneree de cotisations sociales des indemnites de rupture >>, \
cahier 2027.1.1, p. 268). Le logiciel ne sait pas encore l'annuler et la redeclarer dans une correction. Rien n'est ecrit.",
            r.mois,
            apres.engagement.numero_declare()?,
            f.norme()?.version
        ));
    }
    // Les heures en plus exonerees d'impot se declarent en S21.G00.58 de type 01, dates du versement (fiche net-entreprises
    // 2066) : une correction qui en porte (avant ou apres) aurait a les annuler et les redeclarer, ce qui n'est pas porte.
    let exonere = |p: &Paie| p.bases.get(&Assiette::HeuresEnPlusExonere).is_some_and(|m| !m.is_zero());
    if exonere(avant) || exonere(apres) {
        return Err(refus!(
            "Correction de {} (contrat {}) : la paie porte des heures supplémentaires ou complémentaires exonérées d'impôt, que \
la DSN déclare en S21.G00.58 de type 01 (fiche net-entreprises 2066). Le logiciel ne sait pas les annuler et les redéclarer \
dans une correction. Rien n'est écrit.",
            r.mois,
            apres.engagement.numero_declare()?
        ));
    }
    // Le 58 de la paie d'origine se date de son versement (sa fin de periode, 50.001 de la DSN d'origine) en norme 2027.
    let (d_o, f_o) = dates_du_58(f, debut, fin, fin)?;
    let origine = |f: &mut Fichier, montant: Decimal| -> R<()> {
        annuler_les_58(f, r, (d_o, f_o))?;
        f.add("S21.G00.58.001", d_o.jjmmaaaa());
        f.add("S21.G00.58.002", f_o.jjmmaaaa());
        f.add("S21.G00.58.003", "03");
        f.add("S21.G00.58.004", ms(montant, 1));
        Ok(())
    };
    // UN CONTRAT AJOUTE (rien de declare, rien de verse : `produire_regularisations::ajout`) : pas de 58 d'origine a
    // annuler ni a redeclarer ; tout son montant net social est verse par la correction, aux dates du mois du versement
    // (BOSS, montant net social, Q 35 : « du mois lors duquel ce rappel ou ce versement est effectue »).
    let ajoute = r.declare.lignes.is_empty() && r.declare.fichier.is_empty() && r.net_deja_verse.is_zero();
    if !ajoute && apres.net_a_payer == r.net_deja_verse {
        return origine(f, sub(apres.net_social, rappels));
    }
    let paye = if ajoute { Decimal::ZERO } else { sub(r.net_deja_verse, sub(avant.net_a_payer, avant.net_social)) };
    if !ajoute && sub(paye, rappels) != r.declare.net_social {
        origine(f, sub(paye, rappels))?;
    }
    let debut_mois = Jour::nouveau(fin_mois.an, fin_mois.mois, 1).ok_or_else(|| refus("mois de versement illisible"))?;
    let (d_v, f_v) = dates_du_58(f, &debut_mois, &fin_mois, &fin_mois)?;
    f.add("S21.G00.58.001", d_v.jjmmaaaa());
    f.add("S21.G00.58.002", f_v.jjmmaaaa());
    f.add("S21.G00.58.003", "03");
    f.add("S21.G00.58.004", ms(sub(apres.net_social, paye), 1));
    Ok(())
}

/// L'annulation des blocs 58 du versement d'origine (`annuler_le_declare`), dans le versement d'une correction (date
/// du mois de la correction). Un 58 d'origine sans dates (le cas nominal de 2027, `dates_du_58_du_mois`) heritait de la
/// date de SON versement : annule tel quel, il heriterait de celle de la correction. Il recoit donc les dates du
/// versement d'origine (`origine`), comme le complement qui le suit (fiche net-entreprises 2983 : la correction d'un MNS
/// se rattache « a la date du mois du versement auquel ce MNS etait associe »).
fn annuler_les_58(f: &mut Fichier, r: &Regularisation, origine: (Jour, Jour)) -> R<()> {
    let lignes: Vec<(String, String)> = r.declare.blocs(&["58"]).cloned().collect();
    let mut date = false;
    for (k, v) in lignes {
        match k.as_str() {
            "S21.G00.58.001" => {
                date = true;
                f.add(&k, v);
            }
            "S21.G00.58.003" => {
                if !date {
                    dater_le_58(f, Some(origine));
                }
                date = false;
                f.add(&k, v);
            }
            _ if crate::versement_declare::est_un_montant(&k) == Some(true) => {
                let m: Decimal = v.parse().map_err(|_| crate::versement_declare::illisible(&k, &v, &r.declare.fichier))?;
                f.add(&k, ms(m, -1));
            }
            _ => f.add(&k, v),
        }
    }
    Ok(())
}

/// `rang` : le rang du contrat sous l'individu (ses affiliations, 78.005) ; `numero` : celui du versement (50.003), qui
/// n'est jamais celui d'un autre versement de l'individu (`numero_de_versement`).
fn bloc_regularisation(
    f: &mut Fichier, r: &Regularisation, bar_o: &BaremeDuMois, rang: i64, numero: i64, pas_c: &PrelevementDeLaCorrection,
    fin_mois: Jour,
) -> R<()> {
    // `avant` ne donne ici que l'engagement declare (ses bornes, s'il est sans paie) : les montants annules sont ceux
    // du versement declare (`r.declare`), pas ceux d'un recalcul.
    let (avant, apres) = (&r.avant, &r.apres);
    let debut = avant.engagement.debut_periode();
    let fin = avant.engagement.fin_periode();
    // La paie corrigee se rattache a la periode du contrat corrige : ses dates ont pu changer (fiche
    // net-entreprises 1952 ; cahier technique DSN 2026.1, §2.1.3.6). L'annulation recopie le declare, a ses dates.
    let (debut_c, fin_c) = (apres.engagement.debut_periode(), apres.engagement.fin_periode());
    // Le prelevement du versement de la correction est celui que l'engagement rend (`prelevement_de_la_correction`) :
    // la DSN ne le calcule pas.
    let d_ni = pas_c.difference;
    let d_net = sub(apres.net_verse(), r.declare.net_verse);
    f.add("S21.G00.50.001", fin_mois.jjmmaaaa());
    f.add("S21.G00.50.002", mt(d_ni));
    f.add("S21.G00.50.003", numero_de_versement(numero)?);
    f.add("S21.G00.50.004", mt(d_net));
    let v = &pas_c.versement;
    let (taux, type_taux, identifiant, soumis) = (&v.taux, &v.type_taux, &v.identifiant, v.montant_soumis);
    f.add("S21.G00.50.006", taux.ecrit());
    let type_taux = type_taux.ok_or_else(|| absente("S21.G00.50.007"))?;
    f.ecrire_code(type_taux)?;
    if type_taux.transmis() {
        f.add_ecrit("S21.G00.50.008", identifiant.clone())?;
    }
    let indu = d_ni < Decimal::ZERO;
    f.add("S21.G00.50.009", if indu { "0.00".into() } else { mt(pct(soumis, taux.dec()?)) });
    f.add("S21.G00.50.013", if indu { "0.00".into() } else { mt(soumis) });

    annuler_le_declare(f, r, &["51", "53"])?;
    // Une paie d'apres `sans_paie` ecrit ses blocs 51 et 53 a 0.00 : la norme prevoit un montant 51.013 egal a « 0.00 »
    // (CCH-11 et CCH-13 de S21.G00.51.001-002 ne s'appliquent qu'a un montant « différent de 0.00 ») et le format de
    // 53.002 l'admet (`-?[0]*(0|[1-9][0-9]*)\.[0-9]{2}`) ; seul 51.012 refuse le zero (CCH-12), et il ne s'ecrit pas
    // (cahier technique 2027.1, `atelier/reference/norme-dsn/texte-2027/bloc-S21.G00.51.txt` et `bloc-S21.G00.53.txt`).
    remunerations(f, apres, &debut_c, &fin_c, 1)?;
    if !avant.engagement.sans_paie() {
        annuler_le_declare(f, r, &["54"])?;
    }
    if !apres.engagement.sans_paie() {
        autre_revenu(f, apres, bar_o, &debut_c, &fin_c, 1)?;
    }
    if indu {
        let taux_o = match r.taux_pas_origine.as_deref().filter(|t| !t.is_empty()) {
            Some(t) => t,
            None => {
                return Err(refus_avec(
                    format!(
                        "Correction de {} pour {} : le taux de prelevement a la source applique ce mois-la est \
introuvable dans le versement declare (rubrique S21.G00.50.006). Le bloc 56 de regularisation du PAS \
doit le citer, et on ne le devine pas. Rien n'est ecrit.",
                        r.mois,
                        documents::tableaux_export::qui_souple(&apres.salarie)
                    ),
                    Lieu { chemin: Some(format!("sorties/{}/{}", r.mois, r.declare.fichier)), ..Lieu::default() },
                ))
            }
        };
        f.add("S21.G00.56.001", format!("{:02}{:04}", r.mois.numero(), r.mois.annee()));
        f.add("S21.G00.56.002", "03");
        f.add("S21.G00.56.003", mt(d_ni));
        f.add("S21.G00.56.006", taux_o);
        f.add("S21.G00.56.007", mt(pct(d_ni, socle::num::dec_texte(taux_o)?)));
        f.add("S21.G00.56.010", mt(d_ni));
    }
    blocs_58_de_la_correction(f, r, &debut, &fin, fin_mois)?;
    if !avant.engagement.sans_paie() {
        annuler_le_declare(f, r, &["78", "79", "81"])?;
    }
    if !apres.engagement.sans_paie() {
        bases_et_cotisations(f, apres, bar_o, rang, &debut_c, &fin_c, 1, NatureDeLaDeclaration::Mensuelle, &[])?;
    }
    Ok(())
}

// ------------------------------------------- l'agrege de ce qui a ete declare
//
// Le bordereau Urssaf du mois d'origine (`lignes_regularisees`), les versements 20 (`cumuler_le_declare`) et le bloc 55
// d'une regularisation portent la MEME difference que le nominatif : « corrige moins declare »
// Le declare se recompose depuis les lignes 78 et 81 deposees du contrat (`r.declare`), lues
// par ce que la pile de bareme du contrat dit de chacune : quelle base porte un bloc 78 (78.004) ou une cotisation
// (81.003), a quelle cle de totalisation va une cotisation. Ce qui ne s'y retrouve pas sans ambiguite REFUSE (regle 1),
// en nommant le CTP ou la cotisation : on ne retombe pas sur la paie d'avant refaite.

/// Ce que le bareme dit d'une ligne 81, reconnue a son code (81.001) et a son organisme (81.002, vide sans lui).
#[derive(Clone, Debug, PartialEq)]
enum Role81 {
    /// Une cotisation : sa cle de totalisation, et la base que porte son 81.003 quand elle l'ecrit.
    Cotisation { cle: CleDeTotalisation, base: Option<Assiette> },
    /// Une part de la reduction generale (« urssaf », « chomage », « retraite ») et sa cle de totalisation.
    Reduction { part: &'static str, cle: CleDeTotalisation },
}

impl Role81 {
    fn cle(&self) -> CleDeTotalisation {
        match self {
            Role81::Cotisation { cle, .. } | Role81::Reduction { cle, .. } => *cle,
        }
    }
}

/// La lecture que la pile de bareme d'un contrat donne de ses lignes deposees : (code, organisme, role) des lignes 81,
/// (bloc 78, base78) des blocs 78. Le meme bareme que `cotisations` : `bareme_de`, plus les contributions du mois et
/// les trois parts de la reduction generale.
struct Lecture {
    /// Le code 81.001, l'organisme 81.002 (`None` : la ligne n'en porte pas), et ce que la ligne porte.
    roles: Vec<(CodeCotisation81, Option<IdentifiantDOrganisme>, Role81)>,
    blocs: Vec<(BaseAssujettie, Assiette)>,
}

fn lecture(p: &Paie, bar: &BaremeDuMois) -> R<Lecture> {
    let contribs = bar.contributions_du_mois.clone();
    let pile = bareme_de(p, bar)?;
    let mut l = Lecture { roles: Vec::new(), blocs: Vec::new() };
    for c in &pile.cotisations_dsn {
        l.blocs.push((c.bloc78, c.base78));
        let Some(code) = c.code81 else { continue };
        let org = c.org.clone();
        let cle_total = c.cle_total;
        // 81.003 s'ecrit avec un organisme ou une cle de totalisation (`bases_et_cotisations`) ; il porte la base du
        // taux, pas celle d'une cotisation faite de lignes du bulletin.
        let base = if (org.is_some() || cle_total.is_some()) && c.lignes.as_ref().is_none_or(Vec::is_empty) && c.taux.is_some() {
            Some(c.base.ok_or_else(|| socle::fichiers::absent("base"))?)
        } else {
            None
        };
        // La cle de totalisation, sinon le code (`totaux_bordereau`).
        let cle = match cle_total {
            Some(cle) => cle,
            None => code.into(),
        };
        l.roles.push((code, org, Role81::Cotisation { cle, base }));
    }
    let nature = section_de(&*p.engagement.nat()?).to_string();
    for contrib in &contribs {
        if contrib.natures.contains_key(&nature) {
            l.blocs.push((BaseAssujettie::PrevoyanceSanteRetraite, Assiette::Zero));
            let code = CodeCotisation81::PrevoyanceAssuranceMutuelle;
            l.roles.push((code, None, Role81::Cotisation { cle: code.into(), base: None }));
        }
    }
    let org = |code: CodeCotisation81, pour: &str| -> R<Option<IdentifiantDOrganisme>> { Ok(Some(org_de(&pile, code, pour)?)) };
    l.blocs.push((BaseAssujettie::BruteDeplafonnee, Assiette::Brut));
    // Les trois parts de la reduction generale, pour une paie qui en a une (`cotisations` ne les ecrit qu'alors).
    if p.reduction_generale.as_ref().is_some_and(|r| !r.montant.is_zero()) {
        let dsn = pile.dsn_de_la_reduction()?;
        for (code, org, part, cle) in [
            (dsn.code81_urssaf, org(CodeCotisation81::AssuranceMaladie, PART_URSSAF)?, "urssaf", dsn.cle_total_urssaf),
            (dsn.code81_chomage, org(CodeCotisation81::AssuranceChomage, PART_CHOMAGE)?, "chomage", dsn.cle_total_chomage),
            (dsn.code81_retraite, None, "retraite", dsn.code81_retraite.into()),
        ] {
            l.roles.push((code, org, Role81::Reduction { part, cle }));
        }
    }
    // Les heures en plus : la reduction salariale et la deduction patronale, a l'Urssaf (`cotisations`).
    for code in [CODE81_REDUCTION_HEURES, CODE81_DEDUCTION_HEURES] {
        l.roles.push((code, org(CodeCotisation81::AssuranceMaladie, HEURES_EN_PLUS)?, Role81::Cotisation { cle: code.into(), base: None }));
    }
    Ok(l)
}

/// Une ligne 81 deposee, avec le bloc 78 qui la porte.
struct Ligne81 {
    code: CodeCotisation81,
    /// 81.002, `None` : la ligne n'en porte pas.
    org: Option<IdentifiantDOrganisme>,
    base: Option<Decimal>,
    montant: Option<Decimal>,
    /// 78.005 du bloc 78 en cours : l'affiliation (70.012) dans la DSN deposee.
    affiliation: Option<String>,
}

/// Les blocs 78 (78.001, 78.004) et les lignes 81 du versement declare.
fn lignes_declarees(d: &crate::versement_declare::VersementDeclare) -> R<(Vec<(BaseAssujettie, Decimal)>, Vec<Ligne81>)> {
    let lu = |k: &str, v: &str| -> R<Decimal> { v.parse().map_err(|_| crate::versement_declare::illisible(k, v, &d.fichier)) };
    let (mut b78, mut l81): (Vec<(BaseAssujettie, Decimal)>, Vec<Ligne81>) = (Vec::new(), Vec::new());
    let (mut bloc, mut affiliation) = (None::<BaseAssujettie>, None::<String>);
    for (k, v) in d.blocs(&["78", "81"]) {
        match k.as_str() {
            "S21.G00.78.001" => {
                bloc = Some(BaseAssujettie::du_code(v).ok_or_else(|| {
                    refus!("La DSN déposée {} porte, en {}, « {} », qui n'est pas un code de sa liste dans la norme.", d.fichier, k, v)
                })?);
                affiliation = None;
            }
            "S21.G00.78.004" => b78.push((bloc.ok_or_else(|| refus!("{} sans S21.G00.78.001 dans {}", k, d.fichier))?, lu(k, v)?)),
            "S21.G00.78.005" => affiliation = Some(v.clone()),
            "S21.G00.81.001" => l81.push(Ligne81 {
                code: CodeCotisation81::du_code(v).ok_or_else(|| {
                    refus!("La DSN déposée {} porte, en {}, « {} », qui n'est pas un code de sa liste dans la norme.", d.fichier, k, v)
                })?,
                org: None, base: None, montant: None, affiliation: affiliation.clone() }),
            "S21.G00.81.002" | "S21.G00.81.003" | "S21.G00.81.004" => {
                let l = l81.last_mut().ok_or_else(|| refus!("{} sans S21.G00.81.001 dans {}", k, d.fichier))?;
                match k.as_str() {
                    "S21.G00.81.002" => {
                        l.org = Some(IdentifiantDOrganisme::lire(v).map_err(|_| {
                            refus!("La DSN déposée {} porte, en {}, « {} », qui n'a pas la forme d'un identifiant d'organisme.", d.fichier, k, v)
                        })?)
                    }
                    "S21.G00.81.003" => l.base = Some(lu(k, v)?),
                    _ => l.montant = Some(lu(k, v)?),
                }
            }
            _ => {}
        }
    }
    Ok((b78, l81))
}

impl Lecture {
    fn roles_de(&self, code: CodeCotisation81, org: &Option<IdentifiantDOrganisme>) -> Vec<&Role81> {
        self.roles.iter().filter(|(c, o, _)| *c == code && o == org).map(|(_, _, r)| r).collect()
    }

    /// La base `nom` telle que les lignes deposees la portent : les 78.004 des blocs 78 dont le bareme dit qu'ils la
    /// portent (et rien d'autre), les 81.003 des cotisations dont il dit de meme. Elles doivent toutes dire la meme
    /// chose, et il en faut une.
    fn base(&self, nom: Assiette, b78: &[(BaseAssujettie, Decimal)], l81: &[Ligne81]) -> R<Decimal> {
        let bloc_sur = |b: BaseAssujettie| {
            let siens: Vec<Assiette> = self.blocs.iter().filter(|(x, _)| *x == b).map(|(_, base)| *base).collect();
            !siens.is_empty() && siens.iter().all(|x| *x == nom)
        };
        let cotisation_sur = |code: CodeCotisation81, org: &Option<IdentifiantDOrganisme>| {
            let r = self.roles_de(code, org);
            !r.is_empty() && r.iter().all(|x| matches!(x, Role81::Cotisation { base: Some(b), .. } if *b == nom))
        };
        let mut vus: Vec<(String, Decimal)> = Vec::new();
        for (b, v) in b78 {
            if bloc_sur(*b) {
                vus.push((format!("S21.G00.78.004 du bloc {}", b), *v));
            }
        }
        for l in l81 {
            if let (Some(v), true) = (l.base, cotisation_sur(l.code, &l.org)) {
                vus.push((format!("S21.G00.81.003 de la cotisation {}", l.code), v));
            }
        }
        let Some((_, premiere)) = vus.first() else {
            return Err(refus!("aucune ligne deposee ne porte la base « {} » (ni bloc 78, ni cotisation que le bareme y rattache sans ambiguite)", nom));
        };
        if vus.iter().any(|(_, v)| v != premiere) {
            let detail: Vec<String> = vus.iter().map(|(q, v)| format!("{} = {}", q, mt(*v))).collect();
            return Err(refus!("les lignes deposees portent la base « {} » a des montants differents ({})", nom, detail.join(", ")));
        }
        Ok(*premiere)
    }

    /// La reduction generale que l'Urssaf recouvre, en positif, telle que des lignes 81 la portent : l'oppose des 81.004
    /// des lignes declarees sous le code et l'organisme de la part Urssaf. Le bareme d'un permanent y declare aussi la
    /// part chomage (018 chez l'Urssaf, qui recouvre son assurance chomage : `cle_total_chomage`) ; celui d'un
    /// intermittent la declare chez France Travail, hors de ces lignes. C'est ce que le bordereau porte : « La somme des
    /// donnees individuelles (S21.G00.81.004 : code 018) doit etre egale aux donnees agregees (S21.G00.23.005) », « le
    /// montant agrege contient uniquement les montants Urssaf et assurance chomage » (guide declaratif Urssaf, § 8.2 ;
    /// atelier/reference/sources/editeur-dsn/guide-urssaf-regularisation.txt, lignes 1289-1290, 1300-1311 et 1355-1356).
    /// Les lignes n'ont donc pas a dire quelle part est laquelle. Sans ligne, rien n'a ete declare (`cotisations`
    /// n'ecrit pas une part nulle) ; une cotisation que le bareme declare sous le meme code et le meme organisme ne
    /// s'en separe pas : refus.
    fn reduction_a_l_urssaf(&self, l81: &[Ligne81]) -> R<Decimal> {
        let urssaf = self.roles.iter().find(|(_, _, r)| matches!(r, Role81::Reduction { part: "urssaf", .. }));
        let Some((code, org, _)) = urssaf else {
            return Ok(zero2());
        };
        if self.roles_de(*code, org).iter().any(|r| matches!(r, Role81::Cotisation { .. })) {
            return Err(refus!(
                "la reduction generale se declare en S21.G00.81.001 = {}{}, comme une cotisation du bareme : les lignes ne disent pas laquelle est laquelle",
                code,
                org.iter().map(|o| format!(" chez {}", o)).collect::<String>()
            ));
        }
        let mut total = zero2();
        for l in l81.iter().filter(|l| l.code == *code && l.org == *org) {
            total = sub(total, l.montant.ok_or_else(|| refus!("la reduction generale {} n'a pas de S21.G00.81.004", code))?);
        }
        Ok(total)
    }
}

/// Les lignes 81 que le nominatif d'une paie declare (`cotisations`, comme `bases_et_cotisations` les ecrit), dans la
/// forme ou `lignes_declarees` relit celles d'une DSN deposee.
fn lignes_81(p: &Paie, bar: &BaremeDuMois) -> R<Vec<Ligne81>> {
    cotisations(p, bar)?
        .into_iter()
        .filter(|c| c.montant.is_some())
        .map(|c| {
            Ok(Ligne81 {
                code: code_declare(&c)?,
                base: if c.org.is_some() || c.cle_total.is_some() { c.base81 } else { None },
                org: c.org,
                montant: c.montant,
                affiliation: c.affectation,
            })
        })
        .collect()
}

/// La reduction generale que l'Urssaf recouvre pour une paie (CTP 671 d'un technicien intermittent, 668 d'un
/// permanent), lue dans son nominatif comme celle d'une DSN deposee (`Lecture::reduction_a_l_urssaf`) : une seule
/// lecture pour le bordereau courant et le bordereau regularise.
fn reduction_a_l_urssaf(p: &Paie, bar: &BaremeDuMois) -> R<Decimal> {
    if p.reduction_generale.as_ref().is_none_or(|r| r.montant.is_zero()) {
        return Ok(zero2());
    }
    let numero = p.engagement.numero_declare()?;
    lecture(p, bar)?.reduction_a_l_urssaf(&lignes_81(p, bar)?).map_err(|e| e.reformuler(|e| format!("Contrat {} : {}. Rien n'est ecrit.", numero, e)))
}

/// La valeur que le contrat d'une regularisation apportait, tel qu'il a ete DECLARE, a l'assiette `cle` de la table des
/// CTP (celles de `assiettes_ctp`). La nature (permanent, intermittent) et le statut (artiste, technicien) sont ceux du
/// contrat d'avant (`r.avant`), comme ses bornes.
fn assiette_declaree(cle: CleCtp, r: &Regularisation, bar: &BaremeDuMois) -> R<R<Decimal>> {
    // Un contrat ajoute apres la limite (`produire_regularisations::ajout`) : rien de declare, toutes ses assiettes sont
    // nulles -- le bordereau du mois d'origine porte tout le corrige.
    if r.declare.lignes.is_empty() && r.declare.fichier.is_empty() {
        return Ok(Ok(zero2()));
    }
    let p = &r.avant;
    let mensuelle = p.engagement.nat()?.mensuelle();
    // Un permanent declare a la caisse des Conges Spectacles a declare sa taxe d'apprentissage et son FNAL
    // sur des assiettes majorees ; les autres, sur le brut et la tranche 1.
    let caisse = mensuelle && p.declaree_a_la_caisse_des_conges()?;
    let tech = !mensuelle && technicien(p);
    let art = !mensuelle && !tech;
    let l = lecture(p, bar)?;
    let (b78, l81) = lignes_declarees(&r.declare)?;
    let brut = r.declare.brut;
    let si = |oui: bool, v: &dyn Fn() -> R<Decimal>| if oui { v() } else { Ok(zero2()) };
    let base = |a: Assiette| l.base(a, &b78, &l81);
    let t1_ou_majoree = if caisse { Assiette::T1MensuelleX115 } else { Assiette::T1Mensuelle };
    use AssietteDuBordereau as A;
    use MontantDuBordereau as M;
    Ok(match cle {
        CleCtp::Assiette(A::Brut) => Ok(brut),
        CleCtp::Assiette(A::Csg) => base(Assiette::Csg),
        CleCtp::Assiette(A::BrutX115) => base(Assiette::BrutX115),
        CleCtp::Assiette(A::BrutX110) => base(Assiette::BrutX110),
        CleCtp::Assiette(A::BrutPermanent) => si(mensuelle, &|| Ok(brut)),
        CleCtp::Assiette(A::CsgPermanent) => si(mensuelle, &|| base(Assiette::Csg)),
        CleCtp::Assiette(A::T1Permanent) => si(mensuelle, &|| base(Assiette::T1Mensuelle)),
        // La base 07 deposee (78.004 sous le bloc 07), comme `base_07` pour le corrige.
        CleCtp::Assiette(A::ChomagePermanent) => si(mensuelle, &|| {
            let v: Vec<Decimal> = b78.iter().filter(|(b, _)| *b == BaseAssujettie::AssuranceChomage).map(|(_, v)| *v).collect();
            match v.first() {
                None => Ok(zero2()),
                Some(x) if v.iter().all(|y| y == x) => Ok(*x),
                Some(_) => Err(refus("la DSN deposee porte deux bases 07 differentes")),
            }
        }),
        CleCtp::Assiette(A::T1X115Permanent) => si(mensuelle, &|| base(t1_ou_majoree)),
        CleCtp::Montant(M::ReductionGeneralePermanent) => si(mensuelle, &|| l.reduction_a_l_urssaf(&l81)),
        // Les 81.003 de la cotisation 129 deposee (aucune, pour un mois depose sans elles : la correction declare
        // alors toute la contribution, qui etait due).
        CleCtp::Assiette(A::CpfCdd) => Ok(l81.iter().filter(|x| x.code == CODE81_CPF_CDD).filter_map(|x| x.base).fold(zero2(), add)),
        // Les 81.003 de la cotisation 128 deposee d'un permanent, comme `assiettes_ctp`.
        CleCtp::Assiette(A::CfpPermanent) => si(mensuelle, &|| Ok(l81.iter().filter(|x| x.code == CODE81_CFP).filter_map(|x| x.base).fold(zero2(), add))),
        CleCtp::Assiette(A::BrutIntermittent) => si(!mensuelle, &|| Ok(brut)),
        CleCtp::Assiette(A::CsgIntermittent) => si(!mensuelle, &|| base(Assiette::Csg)),
        CleCtp::Assiette(A::TaPrincipale) => if mensuelle && !caisse { Ok(brut) } else { base(Assiette::BrutX110) },
        CleCtp::Assiette(A::BrutTechnicien) => si(tech, &|| Ok(brut)),
        CleCtp::Assiette(A::BrutX115Technicien) => si(tech, &|| base(Assiette::BrutX115)),
        CleCtp::Assiette(A::BrutArtiste) => si(art, &|| Ok(brut)),
        CleCtp::Assiette(A::BrutX115Artiste) => si(art, &|| base(Assiette::BrutX115)),
        CleCtp::Montant(M::ReductionGeneraleUrssaf) => si(tech, &|| l.reduction_a_l_urssaf(&l81)),
        // Les 114 et 021 deposes, signes '-' au nominatif : le montant de la ligne 23 est leur oppose.
        CleCtp::Montant(M::ReductionHeuresEnPlus) => Ok(l81.iter().filter(|x| x.code == CODE81_REDUCTION_HEURES).filter_map(|x| x.montant).fold(zero2(), sub)),
        CleCtp::Montant(M::DeductionHeuresSupplementaires) => Ok(l81.iter().filter(|x| x.code == CODE81_DEDUCTION_HEURES).filter_map(|x| x.montant).fold(zero2(), sub)),
        // Sous 921, l'assiette plafonnee (`assiettes_ctp`) : ce que la DSN deposee a declare en base 02 et sous le FNAL.
        CleCtp::Plafonnee(A::T1Permanent) => si(mensuelle, &|| base(Assiette::T1Mensuelle)),
        CleCtp::Plafonnee(A::T1X115Permanent) => si(mensuelle, &|| base(t1_ou_majoree)),
        CleCtp::Plafonnee(A::BrutArtiste) => si(art, &|| base(Assiette::BrutPlafonne)),
        CleCtp::Plafonnee(A::BrutX115Artiste) => si(art, &|| base(Assiette::BrutX115Plafonne)),
        CleCtp::Plafonnee(A::BrutTechnicien) => si(tech, &|| base(Assiette::BrutPlafonne)),
        CleCtp::Plafonnee(A::BrutX115Technicien) => si(tech, &|| base(Assiette::BrutX115Plafonne)),
        autre @ CleCtp::Plafonnee(_) => return Err(refus!("assiette inconnue : {}", socle::fichiers::repr(&autre.to_string()))),
    })
}

/// La cotisation d'un code a-t-elle un versement 20 hors Urssaf (le bordereau Urssaf se fait par les CTP) ? Les cles de
/// totalisation d'un versement sont ses `codes81` (`organismes-base.json`). Agirc-Arrco y porte 131 et 132
/// (« Cotisation regime unifie Agirc-Arrco », « Cotisation Apec »), et 105 (« y compris Apec »), que la norme 2026.1 ne
/// garde que « pour regularisation des periodes anterieures a 2025 » (cahier technique 2026.1, p. 302 et 305 ;
/// `atelier/reference/norme-dsn/texte/bloc-S21.G00.81.txt`, lignes 79-81).
fn sert_a_un_versement(orgs: &Organismes, code: CodeCotisation81) -> bool {
    orgs.versements().iter().filter(|v| !v.est_urssaf()).any(|v| v.codes81.iter().any(|c| c.code == code))
}

/// `cumuler_totaux` de ce qui a ete DECLARE pour le contrat d'une regularisation : ses lignes 81 deposees, chacune a sa
/// cle de totalisation (le bareme du contrat la dit, par le code et l'organisme). Les composants du bloc 55 ne se
/// cumulent plus ici : ils se lisent dans le nominatif ecrit (`composants_de_versement`), ou `annuler_le_declare` a deja
/// rattache chaque base 31 deposee a son adhesion (70.013).
fn cumuler_le_declare(
    totaux: &mut Vec<(CleDeTotalisation, Decimal)>, r: &Regularisation, bar: &BaremeDuMois, orgs: &Organismes, signe: i64,
) -> R<()> {
    let d = &r.declare;
    let numero = r.apres.engagement.numero_declare()?;
    let refus = |quoi: String| {
        refus!(
            "Correction de {} (contrat {}) : {}. Les versements de la regularisation portent la difference « corrige moins declare », comme le nominatif ; elle ne se devine pas. Rien n'est ecrit.",
            r.mois, numero, quoi
        )
    };
    let l = lecture(&r.avant, bar)?;
    let (_, l81) = lignes_declarees(d)?;
    let s = Decimal::from(signe);
    for x in &l81 {
        let montant = x.montant.ok_or_else(|| refus(format!("la cotisation {} de {} n'a pas de S21.G00.81.004", x.code, d.fichier)))?;
        let mut cles: Vec<CleDeTotalisation> = l.roles_de(x.code, &x.org).iter().map(|r| r.cle()).collect();
        cles.sort();
        cles.dedup();
        let cle = match cles.as_slice() {
            [une] => *une,
            _ if !sert_a_un_versement(orgs, x.code) => continue,
            _ => {
                return Err(refus(format!(
                    "{} declare la cotisation {}{}, que le bareme de {} {} ; a quel versement elle revient ne se lit pas",
                    d.fichier,
                    x.code,
                    x.org.iter().map(|o| format!(" chez {}", o)).collect::<String>(),
                    r.mois,
                    if cles.is_empty() { "ne connait pas pour ce contrat".to_string() } else { format!("rattache a plusieurs totaux ({})", cles.iter().map(CleDeTotalisation::to_string).collect::<Vec<_>>().join(", ")) }
                )))
            }
        };
        ajouter(totaux, cle, mul(s, montant));
        if x.code == CodeCotisation81::PrevoyanceAssuranceMutuelle && x.affiliation.is_none() {
            return Err(refus(format!(
                "une cotisation 059 de {} n'est rattachee a aucune affiliation (S21.G00.78.005) : son composant de versement (S21.G00.55) ne se lit pas",
                d.fichier
            )));
        }
    }
    Ok(())
}

// --------------------------------------------- totaux, bordereau, versements

fn cumuler_totaux(totaux: &mut Vec<(CleDeTotalisation, Decimal)>, paies: &[&Paie], bar: &BaremeDuMois, signe: i64) -> R<()> {
    let s = Decimal::from(signe);
    for p in paies {
        for c in cotisations(p, bar)? {
            let m = match c.montant {
                Some(m) => m,
                None => continue,
            };
            let cle = match c.cle_total {
                Some(cle) => cle,
                None => code_declare(&c)?.into(),
            };
            ajouter(totaux, cle, mul(s, m));
        }
    }
    Ok(())
}

fn ajouter(v: &mut Vec<(CleDeTotalisation, Decimal)>, cle: CleDeTotalisation, m: Decimal) {
    match v.iter_mut().find(|(k, _)| *k == cle) {
        Some((_, x)) => *x = add(*x, m),
        None => v.push((cle, add(zero2(), m))),
    }
}

fn lire(v: &[(CleDeTotalisation, Decimal)], cle: CleDeTotalisation) -> Option<Decimal> {
    v.iter().find(|(k, _)| *k == cle).map(|(_, x)| *x)
}

fn taux_at_du_ctp(ctp: &Ctp, bar: &Pile) -> R<Option<TauxDeLaLigne>> {
    if ctp.taux_at.is_none() {
        return Ok(None);
    }
    // LE TAUX DU BAREME DU MOIS DECLARE : il peut changer en cours d'annee (1,69 puis 1,65 au 01/06/2025) ; `taux_at` du CTP
    // dit seulement « cette ligne porte le taux AT » ("bareme"), le taux ne se recopie pas.
    let Some(valeur) = bar.taux_atmp.as_ref().and_then(|t| t.valeur).filter(|v| !v.is_zero()) else {
        return Err(socle::echec::cle(
            "Le bareme du mois n'a pas de taux AT/MP, et un CTP de la DSN le porte. C'est le taux notifie par l'Assurance Maladie (Configuration › Entreprise). Rien n'est ecrit.",
            "taux_atmp.valeur",
        ));
    };
    let mut valeur = valeur;
    if ctp.statut == Some(droit::fiche::Statut::Artiste) {
        valeur = mul(valeur, Decimal::new(70, 2));
    }
    Ok(Some(TauxDeLaLigne::AtMp(a_deux_chiffres(valeur))))
}

/// Un taux arrondi a deux decimales (au pair), son echelle posee.
fn a_deux_chiffres(t: Decimal) -> Decimal {
    let mut q = t.round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven);
    q.rescale(2);
    q
}

/// Le taux d'une ligne 23 tel que S21.G00.23.003 l'ecrit : le taux AT/MP sur cinq caracteres, celui du versement
/// mobilite tel quel, deux decimales chacun.
fn taux_23(t: TauxDeLaLigne) -> String {
    match t {
        TauxDeLaLigne::AtMp(t) => format!("{:0>5}", fmt_f(t, 2)),
        TauxDeLaLigne::Mobilite(t) => fmt_f(t, 2),
    }
}

fn ligne(code: CodeCtp, qualifiant: String, taux_at: Option<TauxDeLaLigne>, assiette: Option<Decimal>, montant: Option<Decimal>, commune: Option<String>) -> LigneBordereau {
    LigneBordereau { code, qualifiant, taux_at, assiette, montant, commune }
}

fn ecrire_lignes_23(f: &mut Fichier, lignes: &[LigneBordereau]) -> R<()> {
    for l in lignes {
        f.add("S21.G00.23.001", &l.code);
        f.add("S21.G00.23.002", &l.qualifiant);
        if let Some(t) = l.taux_at {
            f.add("S21.G00.23.003", taux_23(t));
        }
        // Un montant (23.005), ou une assiette (23.004) : une ligne qui n'a ni l'un ni l'autre ne s'ecrit pas a zero.
        match l.montant {
            Some(m) => f.add("S21.G00.23.005", mt(m)),
            None => f.add_opt("S21.G00.23.004", l.assiette.map(mt))?,
        }
        if let Some(c) = l.commune.as_deref().filter(|c| !c.is_empty()) {
            f.add("S21.G00.23.006", c);
        }
    }
    Ok(())
}

/// Le salarie est-il technicien (`statut`) ? Le bordereau range a part les techniciens ; tout autre salarie va avec
/// les artistes.
fn technicien(p: &Paie) -> bool {
    p.salarie.statut.dite() == Some(&droit::fiche::Statut::Technicien)
}

/// L'assiette d'une cotisation : la base de son taux (81.003), sinon celle de son bloc 78.
/// Le code (81.001) d'une cotisation qui se declare : le bareme le dit pour toute cotisation qui porte un montant.
fn code_declare(c: &Cotisation) -> R<CodeCotisation81> {
    match c.code81 {
        Some(code) => Ok(code),
        None => Err(socle::echec::livree("baremes", "cotisations_dsn.code81")),
    }
}

fn base_de_la_cotisation(c: &Cotisation) -> Decimal {
    match c.base81 {
        Some(b) => b,
        None => c.base78,
    }
}

/// L'assiette `a` d'une table d'assiettes, qui la porte.
fn assiette(t: &HashMap<Assiette, Decimal>, a: Assiette) -> Decimal {
    t[&a]
}

/// Les assiettes agregees du bordereau Urssaf.
fn assiettes_ctp(paies: &[&Paie], bar: &BaremeDuMois) -> R<HashMap<CleCtp, Decimal>> {
    use AssietteDuBordereau as A;
    // Sous deduction forfaitaire specifique, les assiettes de la securite sociale sont celles des bases abattues
    // (`cle_abattue`, la base 03 ; le 51 de type 001, `brut_du_51`) ; la CSG garde la sienne.
    let mut b = Vec::new();
    for p in paies {
        let mut x = bases(p, &*bareme_de(p, bar)?)?;
        for a in [Assiette::Brut, Assiette::BrutX115, Assiette::BrutX110, Assiette::BrutPlafonne, Assiette::BrutX115Plafonne] {
            if x.contains_key(&a) {
                let k = cle_abattue(p, BaseAssujettie::BruteDeplafonnee, a)?;
                let v = x[&k];
                x.insert(a, v);
            }
        }
        b.push(x);
    }
    let somme = |idx: &dyn Fn(usize) -> bool, f: &dyn Fn(usize) -> Decimal| -> Decimal {
        (0..paies.len()).filter(|i| idx(*i)).fold(zero2(), |s, i| add(s, f(i)))
    };
    let tous = |_: usize| true;
    let mut out = HashMap::new();
    for x in A::TOUTES {
        if let Some(a) = x.de_toutes_les_paies() {
            out.insert(CleCtp::Assiette(*x), somme(&tous, &|i| assiette(&b[i], a)));
        }
    }
    let mensuelle: Vec<bool> = paies.iter().map(|p| p.engagement.nat().map(|n| n.mensuelle())).collect::<socle::echec::R<_>>()?;
    let perm = |i: usize| mensuelle[i];
    let inter = |i: usize| !mensuelle[i];
    out.insert(CleCtp::Assiette(A::BrutPermanent), somme(&perm, &|i| assiette(&b[i], Assiette::Brut)));
    out.insert(CleCtp::Assiette(A::CsgPermanent), somme(&perm, &|i| assiette(&b[i], Assiette::Csg)));
    // La tranche 1 de chaque permanent, et la meme majoree pour un contrat declare a la caisse (`t1_mensuelle_x115`,
    // sinon la tranche 1 elle-meme).
    let (mut t1, mut t1_x115) = (zero2(), zero2());
    for i in (0..paies.len()).filter(|i| perm(*i)) {
        let t = base_de_la_paie(paies[i], Assiette::T1Mensuelle).ok_or_else(|| socle::echec::cle(socle::echec::PAIE_ILLISIBLE, Assiette::T1Mensuelle.nom()))?;
        t1 = add(t1, t);
        t1_x115 = add(t1_x115, match base_de_la_paie(paies[i], Assiette::T1MensuelleX115) {
            Some(majoree) => majoree,
            None => t,
        });
    }
    out.insert(CleCtp::Assiette(A::T1Permanent), t1);
    // Le chomage et l'AGS des permanents (CTP 772 et 937, qualifiant 920) : la somme des bases 07 que leur nominatif
    // declare (`base_07`). Le tableau d'equivalence de l'Urssaf rattache ces deux CTP a la base 07 (040 et 048) sous le
    // qualifiant 920 (atelier/reference/sources/chomage-permanents/urssaf-equivalence-dida-20260611.csv, lignes 1640 et
    // 280) ; c'est le brut dans la limite de quatre plafonds
    // de la periode.
    let mut chomage = zero2();
    for i in (0..paies.len()).filter(|i| perm(*i)) {
        chomage = add(chomage, base_07(paies[i], bar)?);
    }
    out.insert(CleCtp::Assiette(A::ChomagePermanent), chomage);
    // La contribution CPF-CDD (CTP 987, qualifiant 920) : la somme des 81.003 de la cotisation 129 que le nominatif de
    // chaque contrat declare (DIDA : 987 -> 129, base 03).
    let mut cpf_cdd = zero2();
    for p in paies {
        for c in cotisations(p, bar)?.into_iter().filter(|c| c.code81 == Some(CODE81_CPF_CDD)) {
            cpf_cdd = add(cpf_cdd, base_de_la_cotisation(&c));
        }
    }
    out.insert(CleCtp::Assiette(A::CpfCdd), cpf_cdd);
    // La contribution a la formation professionnelle des permanents (CTP 959 ou 971, qualifiant 920) : la somme des
    // 81.003 de leur cotisation 128, l'assiette majoree de 11,5 % d'un contrat declare a la caisse des Conges Spectacles
    // (guide Urssaf des contributions de formation, 12/03/2026 ; fiche net-entreprises 2503 :
    // atelier/reference/sources/majoration-caisses-conges/LISEZ-MOI.md).
    let mut cfp = zero2();
    for i in (0..paies.len()).filter(|i| perm(*i)) {
        for c in cotisations(paies[i], bar)?.into_iter().filter(|c| c.code81 == Some(CODE81_CFP)) {
            cfp = add(cfp, base_de_la_cotisation(&c));
        }
    }
    out.insert(CleCtp::Assiette(A::CfpPermanent), cfp);
    // La reduction generale que l'Urssaf recouvre (`reduction_a_l_urssaf`) : pour un permanent, CTP 668 « reduction
    // generale etendue », sa part securite sociale ET sa part chomage ; pour un technicien intermittent, CTP 671, la
    // seule part securite sociale (France Travail recouvre son chomage).
    let (mut rg_perm, mut rg_tech) = (zero2(), zero2());
    for i in 0..paies.len() {
        if perm(i) {
            rg_perm = add(rg_perm, reduction_a_l_urssaf(paies[i], bar)?);
        } else if technicien(paies[i]) {
            rg_tech = add(rg_tech, reduction_a_l_urssaf(paies[i], bar)?);
        }
    }
    out.insert(CleCtp::Montant(MontantDuBordereau::ReductionGeneralePermanent), rg_perm);
    out.insert(CleCtp::Assiette(A::BrutIntermittent), somme(&inter, &|i| assiette(&b[i], Assiette::Brut)));
    out.insert(CleCtp::Assiette(A::CsgIntermittent), somme(&inter, &|i| assiette(&b[i], Assiette::Csg)));
    // La taxe d'apprentissage sur l'assiette majoree de chaque contrat declare a la caisse (D. 243-0-2), le brut des
    // autres (une majoration de 1,0) : intermittents et permanents ensemble.
    out.insert(CleCtp::Assiette(A::TaPrincipale), somme(&tous, &|i| assiette(&b[i], Assiette::BrutX110)));
    // Le FNAL plafonne des permanents (CTP 332 sous 921) : la tranche 1, majoree pour un contrat declare a la caisse
    // (`calcul::calculer`, `t1_mensuelle_x115`). Le 100 sous 921 garde `t1_permanent`.
    out.insert(CleCtp::Assiette(A::T1X115Permanent), t1_x115);
    let tech = |i: usize| inter(i) && technicien(paies[i]);
    let art = |i: usize| inter(i) && !tech(i);
    out.insert(CleCtp::Assiette(A::BrutTechnicien), somme(&tech, &|i| assiette(&b[i], Assiette::Brut)));
    out.insert(CleCtp::Assiette(A::BrutX115Technicien), somme(&tech, &|i| assiette(&b[i], Assiette::BrutX115)));
    out.insert(CleCtp::Assiette(A::BrutArtiste), somme(&art, &|i| assiette(&b[i], Assiette::Brut)));
    out.insert(CleCtp::Assiette(A::BrutX115Artiste), somme(&art, &|i| assiette(&b[i], Assiette::BrutX115)));
    out.insert(CleCtp::Montant(MontantDuBordereau::ReductionGeneraleUrssaf), rg_tech);
    // Sous le qualifiant 921 (« Assiette plafonnee », cahier technique 2026.1, p. 168), une population porte son assiette
    // PLAFONNEE : celle du bulletin et de la base 02 de chaque contrat (point 22). La cle de la table des CTP reste
    // celle de la population (`brut_artiste` sous 312/921) ; c'est le qualifiant qui dit plafonnee.
    let base = |i: usize, a: Assiette| b[i].get(&a).copied().ok_or_else(|| socle::fichiers::absent(a.nom()));
    let plaf = |f: &dyn Fn(usize) -> bool, a: Assiette| -> R<Decimal> {
        (0..paies.len()).filter(|i| f(*i)).try_fold(zero2(), |s, i| Ok(add(s, base(i, a)?)))
    };
    out.insert(CleCtp::Plafonnee(A::T1Permanent), t1);
    out.insert(CleCtp::Plafonnee(A::T1X115Permanent), t1_x115);
    out.insert(CleCtp::Plafonnee(A::BrutArtiste), plaf(&art, Assiette::BrutPlafonne)?);
    out.insert(CleCtp::Plafonnee(A::BrutX115Artiste), plaf(&art, Assiette::BrutX115Plafonne)?);
    out.insert(CleCtp::Plafonnee(A::BrutTechnicien), plaf(&tech, Assiette::BrutPlafonne)?);
    out.insert(CleCtp::Plafonnee(A::BrutX115Technicien), plaf(&tech, Assiette::BrutX115Plafonne)?);
    // Les heures en plus : la reduction salariale (CTP 003) et la deduction patronale (CTP 004, 005), en montant, comme
    // le nominatif les declare (114 et 021, `cotisations`).
    for (cle, id) in [
        (MontantDuBordereau::ReductionHeuresEnPlus, engagement::heures_en_plus::REDUCTION),
        (MontantDuBordereau::DeductionHeuresSupplementaires, engagement::heures_en_plus::DEDUCTION),
    ] {
        let m = paies.iter().flat_map(|p| p.lignes.iter()).filter(|l| l.est(&id)).fold(zero2(), |a, l| sub(a, add(l.montant_pat, l.montant_sal)));
        out.insert(CleCtp::Montant(cle), m);
    }
    Ok(out)
}

/// La base 07 (« Assiette des contributions d'Assurance Chomage », norme 2026.1 p. 294) que le nominatif du contrat
/// declare : le 78.004 des lignes de `cotisations` sous le bloc 07, le meme montant que le bulletin (la base de la ligne
/// chomage, `brut_chomage` dans les baremes). Zero sans bloc 07 ; deux montants differents refusent.
fn base_07(p: &Paie, bar: &BaremeDuMois) -> R<Decimal> {
    let mut vu: Option<Decimal> = None;
    for c in cotisations(p, bar)?.into_iter().filter(|c| c.bloc78 == BaseAssujettie::AssuranceChomage) {
        match vu {
            Some(v) if v != c.base78 => {
                return Err(refus!(
                    "Contrat {} : le bareme declare deux bases 07 differentes ({} et {}). Rien n'est ecrit.",
                    p.engagement.numero_declare()?,
                    mt(v),
                    mt(c.base78)
                ))
            }
            _ => vu = Some(c.base78),
        }
    }
    Ok(match vu {
        Some(v) => v,
        None => zero2(),
    })
}

/// Le qualifiant d'assiette plafonnee de la table des CTP (S21.G00.23.002, p. 168).
const QUALIFIANT_PLAFONNEE: &str = "921";

/// UNE CLE DE LA TABLE DES CTP (`assiettes_ctp`) : une assiette du bordereau, sa forme plafonnee sous le qualifiant 921,
/// ou un montant. Elle s'ecrit comme la ligne de `ctp_urssaf` la nomme, precedee de « 921: » pour la forme plafonnee.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum CleCtp {
    Assiette(AssietteDuBordereau),
    Plafonnee(AssietteDuBordereau),
    Montant(MontantDuBordereau),
}

impl Display for CleCtp {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            CleCtp::Assiette(a) => write!(f, "{}", a),
            CleCtp::Plafonnee(a) => write!(f, "{}:{}", QUALIFIANT_PLAFONNEE, a),
            CleCtp::Montant(m) => write!(f, "{}", m),
        }
    }
}

/// La cle d'une ligne de la table des CTP dans `assiettes_ctp` : son montant, ou son assiette, plafonnee sous 921.
fn cle_ctp(ctp: &Ctp) -> R<(CleCtp, bool)> {
    if let Some(m) = ctp.montant {
        return Ok((CleCtp::Montant(m), true));
    }
    let k = ctp.assiette.ok_or_else(|| socle::echec::livree("organismes-base.json", &format!("ctp_urssaf.{}.assiette", ctp.code)))?;
    Ok((if ctp.qualifiant.trim() == QUALIFIANT_PLAFONNEE { CleCtp::Plafonnee(k) } else { CleCtp::Assiette(k) }, false))
}

fn valeur_ctp(somme: &HashMap<CleCtp, Decimal>, ctp: &Ctp) -> R<(Decimal, bool)> {
    let (k, par_montant) = cle_ctp(ctp)?;
    match (somme.get(&k), k) {
        (Some(v), _) => Ok((*v, par_montant)),
        (None, CleCtp::Plafonnee(a)) => Err(refus_avec(
            format!(
                "La table des CTP livrée avec le logiciel : le CTP {} sous le qualifiant {} (assiette plafonnée, cahier \
technique 2026.1, p. 168) lit une assiette qui n'a pas de forme plafonnée. Aucune saisie de la compagnie ne le corrige : le \
signaler au développeur. Rien n'est écrit.",
                ctp.code,
                QUALIFIANT_PLAFONNEE
            ),
            Lieu { fichier: Some("organismes-base.json".into()), cle: Some(a.nom().to_string()), ..Default::default() },
        )),
        (None, _) => Err(socle::echec::livree("organismes-base.json", &k.to_string())),
    }
}

/// La contribution CPF-CDD : sa cotisation au nominatif (norme DSN 2026.1, S21.G00.81.001 : « 129 - Contribution dediee
/// au financement du Compte Personnel de Formation pour les titulaires de CDD (CPF-CDD) »).
const CODE81_CPF_CDD: CodeCotisation81 = CodeCotisation81::CpfCdd;
/// La contribution a la formation professionnelle au nominatif (« 128 - Contribution a la formation professionnelle
/// (CFP) », fiche net-entreprises 2503) ; l'assiette de ses CTP de droit commun (959 sous onze salaries, 971 au-dela)
/// est la somme de ses 81.003 chez les permanents (`AssietteDuBordereau::CfpPermanent`).
const CODE81_CFP: CodeCotisation81 = CodeCotisation81::FormationProfessionnelle;
/// « 114 - Montant de réduction des heures supplémentaires/complémentaires » et « 021 - Déduction patronale au titre des
/// heures supplémentaires » (cahier technique 2026.1, `atelier/reference/norme-dsn/texte/bloc-S21.G00.81.txt`, l. 191 et 281).
const CODE81_REDUCTION_HEURES: CodeCotisation81 = CodeCotisation81::ReductionHeuresSupplementaires;
const CODE81_DEDUCTION_HEURES: CodeCotisation81 = CodeCotisation81::DeductionHeuresSupplementaires;

/// Les seuils d'effectif d'un mois, pour les CTP qui en dependent (`effectif_au_moins`, `effectif_sous` d'une ligne de
/// `ctp_urssaf`) : l'effectif A01 que l'Urssaf renvoie en CRM 118 (`config/effectifs.json`, regle 8), la regle des cinq
/// annees civiles consecutives (L. 130-1, II, CSS ; `Effectif::atteint`), pour payer l'annee du mois. Comme
/// l'echeance de la DSN (`Contexte::echeance_dsn`), un effectif qu'un seuil demande et que le moteur ne connait pas se
/// refuse en nommant les annees qui manquent.
pub struct Seuils {
    effectif: droit::contexte::Effectif,
    an: i32,
}

impl Seuils {
    fn du_mois(ctx: &Contexte, mois: Mois) -> R<Seuils> {
        Ok(Seuils { effectif: ctx.effectif()?, an: mois.annee() })
    }

    /// La ligne `ctp` du fichier des organismes s'ecrit-elle a l'effectif de ce mois ? Un effectif qu'un seuil demande et
    /// que le moteur ne connait pas se refuse en nommant les annees qui manquent (`Effectif::atteint`).
    fn retient(&self, ctp: &Ctp) -> R<bool> {
        let atteint = |seuil: Option<i64>| seuil.map(|s| self.effectif.atteint(s, self.an)).transpose();
        Ok(atteint(ctp.effectif_au_moins)?.is_none_or(|a| a) && atteint(ctp.effectif_sous)?.is_none_or(|a| !a))
    }
}

/// `assujettissement::taxe_apprentissage_non_due_au_mois` pour le mois `m`.
fn taxe_apprentissage_non_due_du_mois(ent: &ChampsEntreprise, m: Mois) -> R<bool> {
    droit::apprentissage::taxe_apprentissage_non_due_au_mois(ent, m.annee(), m.numero())
}

fn lignes_bordereau(
    paies: &[&Paie], orgs: &Organismes, ent: &ChampsEntreprise, bar: &BaremeDuMois, solde_ta: Option<&SoldeTa>, doeth: Option<&crate::oeth::Declaration>,
    csa: Option<&crate::csa::Csa>, mois: Mois, effectif: &Seuils,
) -> R<Vec<LigneBordereau>> {
    let mut lignes = Vec::new();
    let somme = assiettes_ctp(paies, bar)?;
    let ctps = crate::assujettissement::ctp_sans_la_taxe_non_due(orgs.ctp_urssaf().to_vec(), taxe_apprentissage_non_due_du_mois(ent, mois)?);
    let ctps = ctps.as_slice();
    let brut_permanent = somme[&CleCtp::Assiette(AssietteDuBordereau::BrutPermanent)];
    if !brut_permanent.is_zero() &&!ctps.iter().any(|c| c.population.as_deref() == Some("permanent")) {
        return Err(refus!(
            "La DSN de {} porte {} de brut de salariés permanents, et la table des CTP livrée avec le logiciel \
n'a aucun CTP des permanents. Le bordereau Urssaf serait faux : un \
permanent relève du régime général (CTP 100 et suivants) et son assurance chômage est recouvrée par l'Urssaf, \
contrairement à celle d'un intermittent. Aucune saisie de la compagnie ne le corrige. Rien n'est écrit.",
            mois,
            socle::texte::euros(brut_permanent)
        ));
    }
    for ctp in ctps {
        if !effectif.retient(ctp)? {
            continue;
        }
        let (valeur, par_montant) = valeur_ctp(&somme, ctp)?;
        if (ctp.statut.is_some() || ctp.population.as_deref().is_some_and(|s| !s.is_empty())) && valeur.is_zero() {
            continue;
        }
        let v = rond(valeur);
        lignes.push(ligne(
            ctp.code.clone(),
            ctp.qualifiant.clone(),
            taux_at_du_ctp(ctp, bar)?,
            if par_montant { None } else { Some(v) },
            if par_montant { Some(v) } else { None },
            None,
        ));
    }
    if let Some(vm) = bareme::versement_mobilite(bar)? {
        // L'assiette majoree de l'intermittent (CSS L. 243-1-3 ; `bareme::bareme_vm`), le brut du permanent : celle du
        // bloc 81 de chacun. « Pour le versement de transport, le CTP 900 » (Tissot, 2017 ;
        // atelier/reference/sources/majoration-caisses-conges/).
        let (mut artistes, mut autres) = (zero2(), zero2());
        for p in paies {
            let a = assiette(&bases(p, &*bareme_de(p, bar)?)?, Assiette::BrutX115);
            if !technicien(p) {
                artistes = add(artistes, a);
            } else {
                autres = add(autres, a);
            }
        }
        for (code, assiette) in [(CodeCtp::du_moteur("900"), autres), (CodeCtp::du_moteur("921"), artistes)] {
            if assiette.is_zero() {
                continue;
            }
            lignes.push(ligne(code, "920".into(), Some(TauxDeLaLigne::Mobilite(a_deux_chiffres(vm.taux))), Some(rond(assiette)), None, Some(vm.code_insee.to_string())));
        }
    }
    let mut local = zero2();
    for p in paies {
        if bareme::alsace_moselle(&bar.entreprise.fiche, &p.salarie) {
            local = add(local, assiette(&bases(p, &*bareme_de(p, bar)?)?, Assiette::Brut));
        }
    }
    if !local.is_zero() {
        // Le CTP que le bareme dit pour le regime local (« 381 », avec sa source).
        let code = bar
            .regime_local_alsace_moselle
            .as_ref()
            .map(|r| r.ctp.clone())
            .ok_or_else(|| socle::echec::livree("baremes", "regime_local_alsace_moselle.ctp"))?;
        lignes.push(ligne(code, "920".into(), None, Some(rond(local)), None, None));
    }
    if let Some(s) = solde_ta {
        lignes.push(ligne(CodeCtp::du_moteur("995"), "920".into(), None, Some(s.assiette), None, None));
    }
    if let Some(d) = doeth {
        for (ctp, montant) in d.bordereau() {
            lignes.push(ligne(ctp, "920".into(), None, Some(montant), None, None));
        }
    }
    // La contribution supplementaire a l'apprentissage de mars : le CTP 998, son montant en 23.004 (taux de 100 %),
    // comme le CTP 730 de l'OETH (guide Urssaf du 12/03/2026, l. 2453-2458 ; `csa`).
    if let Some((ctp, montant)) = csa.and_then(crate::csa::Csa::bordereau) {
        lignes.push(ligne(ctp, "920".into(), None, Some(montant), None, None));
    }
    Ok(lignes)
}

/// Le bordereau Urssaf d'un mois d'origine que la DSN courante regularise : par CTP, la difference « corrige moins
/// declare » (`assiette_declaree`), la meme que celle que porte le nominatif.
///
/// `bar_o` : le bareme du mois d'origine POUR L'ETABLISSEMENT de la DSN (`bar_du`), le meme que celui du nominatif de la
/// correction et du bordereau courant. Point 25 : il etait pris sans `surcharger`, et un etablissement
/// secondaire valorisait son bordereau regularise au taux AT/MP du principal (23.003, `taux_at_du_ctp`) ; regle 8,
/// chaque etablissement a le sien.
fn lignes_regularisees(
    orgs: &Organismes, ent: &ChampsEntreprise, regs: &[Regularisation], mois_o: Mois, bar_o: &BaremeDuMois, effectif: &Seuils,
) -> R<Vec<LigneBordereau>> {
    let siens: Vec<&Regularisation> = regs.iter().filter(|r| r.mois == mois_o).collect();
    let apres = assiettes_ctp(&siens.iter().map(|r| &r.apres).collect::<Vec<_>>(), bar_o)?;
    let mut lignes = Vec::new();
    let ctps = crate::assujettissement::ctp_sans_la_taxe_non_due(orgs.ctp_urssaf().to_vec(), taxe_apprentissage_non_due_du_mois(ent, mois_o)?);
    for ctp in &ctps {
        if !effectif.retient(ctp)? {
            continue;
        }
        let (corrige, par_montant) = valeur_ctp(&apres, ctp)?;
        let (cle, _) = cle_ctp(ctp)?;
        let mut declare = zero2();
        for r in &siens {
            let numero = r.apres.engagement.numero_declare()?;
            let v = assiette_declaree(cle, r, bar_o)?.map_err(|e| {
                e.reformuler(|e| format!(
                    "Correction de {} (contrat {}) : la ligne {} ({}) du bordereau Urssaf de ce mois, assiette « {} », ne se recompose pas depuis ce que {} a declare pour le contrat : {}. Le bordereau d'une regularisation porte la difference « corrige moins declare », comme le nominatif ; elle ne se devine pas. Rien n'est ecrit.",
                    r.mois,
                    numero,
                    ctp.code,
                    ctp.qualifiant,
                    cle,
                    r.declare.fichier,
                    e
                ))
            })?;
            declare = add(declare, v);
        }
        let v = rond(sub(corrige, declare));
        if v.is_zero() {
            continue;
        }
        let taux_at = taux_at_du_ctp(ctp, bar_o)?;
        let code = ctp.code.clone();
        if par_montant && v.is_sign_negative() {
            // Une reduction qui BAISSE (un salaire corrige a la hausse) : son CTP de regularisation, en assiette (23.004)
            // et sans signe, et non le CTP de la reduction en montant negatif. Guide declaratif Urssaf 5.1, § 8.2,
            // « Schema de l'attendu declaratif » : « Ctp 668/671 : […] Montant de cotisation (reduction) XX ; Ctp
            // 669/801 : Montant d'assiette XX », « les montants ne doivent pas comporter de signe ; les CTP comprennent
            // les signes necessaires » (atelier/reference/sources/editeur-dsn/guide-urssaf-regularisation.txt, lignes
            // 1341-1362). La table des CTP valorise 669 et 801 (format E) a 100 % : le montant du bordereau est celui
            // qu'aurait deduit le montant negatif.
            let qualifiant = ctp.qualifiant.clone();
            let regularisation = ctp_de_regularisation(&code, &siens, bar_o)?.ok_or_else(|| {
                socle::echec::cle(
                    format!(
                        "Correction de {} : la ligne {} ({}) du bordereau Urssaf baisse de {}, et le barème du mois ne dit pas sous quel CTP se régularise cette réduction : un montant de réduction ne s'écrit pas négatif (guide déclaratif Urssaf, § 8.2). Rien n'est écrit.",
                        mois_o,
                        code,
                        qualifiant,
                        socle::texte::euros(-v)
                    ),
                    "reduction_generale.dsn.ctp_regularisation",
                )
            })?;
            lignes.push(ligne(regularisation, qualifiant, taux_at, Some(-v), None, None));
            continue;
        }
        lignes.push(ligne(
            code,
            ctp.qualifiant.clone(),
            taux_at,
            if par_montant { None } else { Some(v) },
            if par_montant { Some(v) } else { None },
            None,
        ));
    }
    Ok(lignes)
}

/// Le CTP qui regularise le CTP de reduction `code` (668 -> 669, 671 -> 801) : `reduction_generale.dsn.ctp_regularisation`
/// de la pile de bareme d'un des contrats corriges, celle dont `reduction_generale.dsn.ctp` est ce code (le bareme livre :
/// `par_nature.permanent` et `par_statut.technicien`). Aucun : `None`.
fn ctp_de_regularisation(code: &CodeCtp, siens: &[&Regularisation], bar: &BaremeDuMois) -> R<Option<CodeCtp>> {
    for r in siens {
        for p in [&r.apres, &r.avant] {
            let pile = bareme_de(p, bar)?;
            let Some(dsn) = reduction_dsn(&pile) else { continue };
            if dsn.ctp.as_ref() == Some(code) {
                if let Some(x) = dsn.ctp_regularisation.clone() {
                    return Ok(Some(x));
                }
            }
        }
    }
    Ok(None)
}

/// S21.G00.20.002 d'un versement : la reference de l'organisme, le compte France Travail de la fiche entreprise.
pub fn reference_versement(v: &Versement, ent: &ChampsEntreprise) -> Option<String> {
    if v.siret != SIRET_FRANCE_TRAVAIL {
        return None;
    }
    ent.france_travail_dossier_recouvrement.portee().map(|s| s.trim().to_string())
}

/// Le montant d'un versement : la somme de ses cles de totalisation (`codes81`).
fn montant_versement(v: &Versement, totaux: &[(CleDeTotalisation, Decimal)]) -> Decimal {
    // Une cle qu'aucune cotisation du mois ne porte n'ajoute rien.
    v.codes81.iter().filter_map(|c| lire(totaux, *c)).fold(zero2(), add)
}

// ------------------------------------ le versement a un organisme complementaire
//
// Les composants d'un versement de prevoyance (bloc S21.G00.55, norme 2026.1, `atelier/reference/norme-dsn/texte/
// bloc-S21.G00.55.txt`) se LISENT dans ce que le fichier declare, une fois les individus ecrits : chaque cotisation
// d'une base rattachee a une affiliation (78.005) va au contrat que cette affiliation reference (70.012 -> 70.013 ->
// 15.005), et le code d'affectation est la reference de ce contrat (« Le contrat mentionne doit faire l'objet d'un bloc
// Adhesion Prevoyance S21.G00.15 dans la declaration », p. 260, lignes 72-74 ; CCH-11, lignes 75-79 : le couple
// 20.001 / 55.003 se retrouve en 15.002 / 15.001). Le montant d'un composant est « la part du versement affectee au
// contrat [...] pour la periode d'affectation » (p. 259, lignes 44-46) ; la periode est « une periode civile complete
// (mois, trimestre, semestre ou annee) contenant la ou les periodes cotisees » (p. 260, lignes 85-87). Le versement
// (20.005) est la somme de ses composants, ou zero si elle est negative (bloc-S21.G00.20.txt, p. 157, lignes 191-193
// et CCH-12 lignes 211-213).

/// Un composant de versement (bloc S21.G00.55), lu dans le fichier.
#[derive(Clone, Debug, PartialEq)]
pub struct Composant {
    /// L'organisme de l'adhesion (S21.G00.15.002), celui du versement (S21.G00.20.001).
    pub organisme: IdentifiantDOrganisme,
    /// La reference du contrat chez l'organisme (S21.G00.15.001), le code d'affectation (S21.G00.55.003).
    pub code: String,
    /// AAAAPNN (S21.G00.55.004).
    pub periode: String,
    pub montant: Decimal,
}

/// La plus petite periode civile (trimestre, semestre, annee) qui contient les deux dates JJMMAAAA d'une base 78. Le
/// trimestre etait deja la periode ecrite pour un mois courant ; la norme l'admet (p. 260, ligne 89).
fn periode_d_affectation(debut: Jour, fin: Jour) -> R<String> {
    let ((a1, m1), (a2, m2)) = ((debut.an, debut.mois), (fin.an, fin.mois));
    if a1 != a2 {
        return Err(refus!(
            "S21.G00.55.004 : une base de prevoyance couvre du {} au {}, a cheval sur deux annees civiles ; la periode \
d'affectation est « une periode civile complete [...] contenant la ou les periodes cotisees » (norme p. 260) et aucune \
ne les contient toutes deux. Rien n'est ecrit.",
            debut.jjmmaaaa(),
            fin.jjmmaaaa()
        ));
    }
    let (t1, t2) = ((m1 - 1) / 3 + 1, (m2 - 1) / 3 + 1);
    let (s1, s2) = ((m1 - 1) / 6 + 1, (m2 - 1) / 6 + 1);
    Ok(if t1 == t2 {
        format!("{}T{:02}", a1, t1)
    } else if s1 == s2 {
        format!("{}S{:02}", a1, s1)
    } else {
        format!("{}A00", a1)
    })
}

/// Les composants de versement que declarent les lignes `lignes` (le fichier en cours, individus ecrits), dans l'ordre
/// ou ils apparaissent, cumules par organisme, code d'affectation et periode.
pub fn composants_de_versement(lignes: &[String]) -> R<Vec<Composant>> {
    let lire = |l: &str| -> (String, String) {
        match l.split_once(',') {
            Some((k, v)) => (k.to_string(), v.trim().trim_matches('\'').to_string()),
            None => (l.to_string(), String::new()),
        }
    };
    // 15.005 -> (15.001, 15.002)
    let mut adhesions: HashMap<String, (String, IdentifiantDOrganisme)> = HashMap::new();
    let (mut lib15, mut org15) = (String::new(), None::<IdentifiantDOrganisme>);
    // 70.012 -> 70.013, pour l'individu en cours (les identifiants y sont uniques, `bloc_contrat`).
    let mut ids: HashMap<String, String> = HashMap::new();
    let mut id70: Option<String> = None;
    let mut nom = String::new();
    // La base 78 en cours : (78.002, 78.003, 78.005) ; et le code de la cotisation 81 en cours.
    let mut base: Option<(String, String, Option<String>)> = None;
    let mut code81 = String::new();
    let mut out: Vec<Composant> = Vec::new();
    for l in lignes {
        let (k, v) = lire(l);
        if !matches!(k.get(..10), Some("S21.G00.78" | "S21.G00.79" | "S21.G00.81")) {
            base = None;
        }
        match k.as_str() {
            "S21.G00.15.001" => lib15 = v,
            "S21.G00.15.002" => org15 = Some(IdentifiantDOrganisme::lire(&v)?),
            "S21.G00.15.005" => {
                let org = org15.clone().ok_or_else(|| refus!("S21.G00.15.005 {} sans S21.G00.15.002 : l'adhésion n'a pas d'organisme.", v))?;
                adhesions.insert(v, (lib15.clone(), org));
            }
            "S21.G00.30.001" => {
                ids.clear();
                nom.clear();
            }
            "S21.G00.30.002" => nom = v,
            "S21.G00.70.012" => id70 = Some(v),
            "S21.G00.70.013" => {
                if let Some(i) = id70.take() {
                    ids.insert(i, v);
                }
            }
            "S21.G00.78.001" => base = Some((String::new(), String::new(), None)),
            "S21.G00.78.002" => {
                if let Some(b) = base.as_mut() {
                    b.0 = v;
                }
            }
            "S21.G00.78.003" => {
                if let Some(b) = base.as_mut() {
                    b.1 = v;
                }
            }
            "S21.G00.78.005" => {
                if let Some(b) = base.as_mut() {
                    b.2 = Some(v);
                }
            }
            "S21.G00.81.001" => code81 = v,
            "S21.G00.81.004" => {
                let (debut, fin, aff) = match &base {
                    Some((d, f, Some(a))) => (d, f, a),
                    _ => continue,
                };
                let reference = ids.get(aff).ok_or_else(|| {
                    refus!(
                        "Salarie {} : la cotisation {} est rattachee a l'affiliation {} (S21.G00.78.005), qu'aucun bloc 70 \
du contrat ne declare. Le code d'affectation de son composant de versement (S21.G00.55.003) ne se lit nulle part. Rien \
n'est ecrit.",
                        nom, code81, aff
                    )
                })?;
                let (code, organisme) = adhesions.get(reference).cloned().ok_or_else(|| {
                    refus!(
                        "Salarie {} : le contrat se rattache a l'adhesion {} (S21.G00.70.013), que la declaration ne declare \
pas en bloc S21.G00.15. Le code d'affectation (S21.G00.55.003) doit etre un contrat declare en bloc 15 (norme p. 260, \
CCH-11) : il ne se devine pas. Ce contrat doit figurer, avec sa référence, dans Configuration › Organismes et \
contrats ouvert sur le mois déclaré. Rien n'est écrit.",
                        nom, reference
                    )
                })?;
                let montant: Decimal =
                    v.parse().map_err(|_| socle::echec::a_la_cle(&format!("Le montant de la cotisation {} ne se lit pas (S21.G00.81.004).", code81), "S21.G00.81.004", v))?;
                let periode = match (crate::dsn_lue::jour_dsn(debut), crate::dsn_lue::jour_dsn(fin)) {
                    (Some(d), Some(f)) => periode_d_affectation(d, f)?,
                    _ => return Err(refus!("S21.G00.55.004 : periode cotisee illisible ({} - {}).", debut, fin)),
                };
                match out.iter_mut().find(|c| c.organisme == organisme && c.code == code && c.periode == periode) {
                    Some(c) => c.montant = add(c.montant, montant),
                    None => out.push(Composant { organisme, code, periode, montant: add(zero2(), montant) }),
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

/// Le bloc 20 d'un organisme complementaire et ses composants 55, ou rien si aucun composant ne porte de montant.
fn versement_complementaire(
    v: &Versement, ent: &ChampsEntreprise, banque: &droit::fiche::Banque, composants: &[&Composant], debut: Jour, fin: Jour, echeance: Option<Jour>,
) -> R<Vec<String>> {
    if composants.iter().all(|c| c.montant.is_zero()) {
        return Ok(Vec::new());
    }
    let somme = composants.iter().fold(zero2(), |s, c| add(s, c.montant));
    // « sauf si cette somme fait ressortir un montant negatif. Auquel cas un montant nul doit etre renseigne » (p. 157).
    let montant = bordereau::a_verser(somme).montant;
    let mut f = Fichier::nouveau();
    f.add("S21.G00.20.001", v.siret.clone());
    if let Some(r) = reference_versement(v, ent) {
        f.add("S21.G00.20.002", r);
    }
    f.add_case("S21.G00.20.003", &banque.bic, "bic")?;
    f.add_case("S21.G00.20.004", &banque.iban, "iban")?;
    f.add("S21.G00.20.005", mt(montant));
    f.add("S21.G00.20.006", debut.jjmmaaaa());
    f.add("S21.G00.20.007", fin.jjmmaaaa());
    f.add("S21.G00.20.010", "05");
    if let Some(j) = echeance {
        f.add("S21.G00.20.011", j.jjmmaaaa());
    }
    for c in composants {
        f.add("S21.G00.55.001", mt(c.montant));
        f.add("S21.G00.55.003", &c.code);
        f.add("S21.G00.55.004", &c.periode);
    }
    Ok(f.lignes)
}

// ------------------------------------------------- le versement a la DGFiP
//
// Le prelevement a la source retenu se reverse par la DSN, dans un bloc 20 a part (norme 2026.1, bloc S21.G00.20,
// `atelier/reference/norme-dsn/texte/bloc-S21.G00.20.txt`) : 20.001 = « DGFIP » (p. 154, ligne 47), 20.002 =
// « DGFIP_PAS » (p. 155, ligne 104 ; CCH-11 p. 156, lignes 125-127), 20.005 « arrondi a l'entier le plus proche »
// (p. 157, lignes 194-199), 20.006 et 20.007 le premier et le dernier jour du mois principal declare (p. 158 ligne
// 248, p. 159 ligne 301), 20.010 « 05 » (CCH-14 p. 161, lignes 428-430), 20.011 « non concerne » (p. 161, ligne 466),
// 20.012 interdite en prelevement SEPA (CCH-13 p. 162, lignes 527-529). Aucun bloc 55 : il est « a destination des
// organismes complementaires » (bloc-S21.G00.55.txt, ligne 34).

/// S21.G00.20.001 et S21.G00.20.002 du versement du prelevement a la source.
pub const DGFIP: IdentifiantDOrganisme = IdentifiantDOrganisme::du_moteur("DGFIP");
pub const DGFIP_PAS: &str = "DGFIP_PAS";

/// Les coordonnees bancaires du compte preleve (S21.G00.20.003 et .004) : la fiche entreprise, `banque`. Absentes, la
/// DSN ne s'ecrit pas : un versement en prelevement SEPA (20.010 = 05) les exige (CCH-12 et SIG-13, p. 161).
fn banque_du_prelevement(ent: &ChampsEntreprise) -> R<&droit::fiche::Banque> {
    let banque = ent.banque.dite();
    let manque: Vec<&str> = [(banque.and_then(|b| b.bic.portee()), "le BIC (S21.G00.20.003)"), (banque.and_then(|b| b.iban.portee()), "l'IBAN (S21.G00.20.004)")]
        .iter()
        .filter(|(c, _)| c.is_none())
        .map(|(_, l)| *l)
        .collect();
    if !manque.is_empty() {
        return Err(socle::echec::cle(format!(
            "Coordonnees bancaires absentes de la fiche entreprise : {}. Chaque versement de la DSN (Urssaf, DGFiP, \
caisses) se paie par prelevement SEPA sur ce compte, et la norme exige les deux (S21.G00.20.010 = 05 : CCH-12 et \
SIG-13). Les saisir dans Configuration › Paiement des cotisations (le compte et son BIC, \
sur le RIB de la compagnie). Rien n'est ecrit.",
            manque.join(" et ")
        ), "banque"));
    }
    ent.banque.exigee("banque")
}

/// Le prelevement a la source que CE fichier retient : la somme des S21.G00.50.009 (versements du mois et
/// regularisations positives) et des S21.G00.56.007 (regularisations d'un indu, negatives). INFERENCE : la norme ne
/// nomme pas cette somme ; le versement DGFiP est le « montant des cotisations reglees » (20.005, p. 156) et le PAS
/// precompte est celui des 50.009 (p. 245) corrige des 56.007 (p. 263).
pub fn pas_retenu(lignes: &[String]) -> R<Decimal> {
    let mut s = zero2();
    for l in lignes {
        let Some((k, v)) = l.split_once(',') else { continue };
        if k == "S21.G00.50.009" || k == "S21.G00.56.007" {
            let v = v.trim_matches('\'');
            let m: Decimal = v.parse().map_err(|_| socle::echec::a_la_cle(&format!("Un montant ne se lit pas ({}).", k), k, v))?;
            s = add(s, m);
        }
    }
    Ok(s)
}

/// Le bloc 20 de la DGFiP, ou rien. `du` : le PAS retenu par le fichier ; `remplace` : la DSN est un annule et
/// remplace (type 03 ou 05), et `remplacee_versait_pas` dit si la declaration remplacee portait un versement DGFiP.
fn versement_dgfip(
    du: Decimal, mois: Mois, debut: Jour, fin: Jour, banque: &droit::fiche::Banque, remplace: bool, remplacee_versait_pas: Option<bool>,
) -> R<Vec<String>> {
    // « arrondi a l'entier le plus proche (les deux decimales etant valorisees a zero) » (p. 157, ligne 194) ; le demi
    // va a l'euro superieur (`bordereau::euro`) : CGI art. 1724, « La liquidation de toutes sommes a recevoir [...] est
    // arrondie a l'euro le plus proche. La fraction d'euro egale a 0,50 est comptee pour 1 »
    // (`atelier/reference/sources/code-general-des-impots/articles-20261013.md`). INFERENCE : la norme ne dit pas le demi,
    // et L. 133-10 CSS ne vise pas la DGFiP ; le versement du prelevement a la source est une somme a recevoir du fisc.
    let mut montant = bordereau::euro(du);
    montant.rescale(2);
    // UN PAS NEGATIF (les regularisations rendent plus que le mois ne retient) : « ces rubriques doivent etre renseignees
    // a 0.00 », et le bloc, que la fiche admet « par commodite » sur une DSN initiale, s'ecrit a zero pour que l'ecran
    // dise le remboursement a demander (fiche net-entreprises 1931, l. 4 et 13 ; `bordereau::a_verser`).
    let credit = du < Decimal::ZERO;
    let montant = bordereau::a_verser(montant).montant;
    let ecrire = if !montant.is_zero() || credit {
        true
    } else if remplace {
        // « Pour une declaration de type annule et remplace sans paiement de Prelevement a la source et deposee en
        // substitution d'une declaration pourvue d'un bloc Versement pour le Prelevement a la source, il est
        // obligatoire de declarer un bloc de Versement [...] a zero » (p. 157, lignes 195-199).
        match remplacee_versait_pas {
            Some(v) => v,
            None => {
                return Err(refus_avec(format!(
                    "Annule et remplace de {} sans prelevement a la source a reverser : si la declaration remplacee \
portait un versement DGFiP, celle-ci doit en porter un a 0.00 (S21.G00.20.005, norme p. 157). La DSN remplacee n'a \
pas pu etre relue (le fichier du dernier depot accepte) : on ne devine pas. Rien n'est ecrit.",
                    mois
                ), Lieu { chemin: Some(format!("sorties/{}/", mois)), ..Lieu::default() }))
            }
        }
    } else {
        // Rien a reverser : aucune rubrique ne l'exige, et le bloc Versement « doit etre renseigne seulement a
        // l'echeance de paiement » (chapitre 2, §2.2.2.6, p. 56, ligne 1900 du texte).
        false
    };
    if !ecrire {
        return Ok(Vec::new());
    }
    let mut f = Fichier::nouveau();
    f.add("S21.G00.20.001", DGFIP);
    f.add("S21.G00.20.002", DGFIP_PAS);
    f.add_case("S21.G00.20.003", &banque.bic, "bic")?;
    f.add_case("S21.G00.20.004", &banque.iban, "iban")?;
    f.add("S21.G00.20.005", mt(montant));
    f.add("S21.G00.20.006", debut.jjmmaaaa());
    f.add("S21.G00.20.007", fin.jjmmaaaa());
    f.add("S21.G00.20.010", "05");
    Ok(f.lignes)
}

/// Le solde de la taxe d'apprentissage d'avril.
pub struct SoldeTa {
    pub annee: i32,
    pub assiette: Decimal,
    pub montant: Decimal,
    pub org: IdentifiantDOrganisme,
}

/// CE QUI A ETE DECLARE D'UN MOIS : pour chaque engagement valide de `mois`, dans l'ordre de l'ecran, la paie que la
/// derniere DSN qui a declare ce mois a portee (`engagement::etats::paie_declaree`, regle 4 : ce qui est declare se lit
/// dans la version sur laquelle la DSN est partie), ou sa paie ecrite si aucune DSN ne l'a encore declaree. Un
/// engagement en saisie n'y est pas : rien ne part de lui. `nic` : les seuls salaries de cet etablissement.
pub fn paies_declarees_du_mois(ctx: &Contexte, mois: Mois, nic: Option<&str>, departs: &engagement::departs::Departs) -> R<Vec<Paie>> {
    // Un mois sans saisie n'a rien declare.
    let declarees = |data: engagement::saisie::VueDuMois| -> R<Vec<Paie>> {
        let ctx_m = ctx.du_mois(mois)?;
        let mut out = Vec::new();
        for (i, e) in engagement::etats::du_mois(ctx, mois, departs)?.iter().enumerate() {
            let Some(v) = e.valide() else { continue };
            let p = match engagement::etats::paie_declaree(&ctx_m, v.objet(), departs, mois)? {
                Some(p) => p,
                None => v.paie(ctx, &data, i)?,
            };
            if let Some(n) = nic {
                if bareme::etablissement_de(&ctx_m.entreprise, &p.salarie)? != n {
                    continue;
                }
            }
            out.push(p);
        }
        Ok(out)
    };
    Ok(engagement::stock::vue_du_mois(&ctx.racine, mois)?.map(declarees).transpose()?.into_iter().flatten().collect())
}

/// LES ASSIETTES DES CTP `codes` que le bordereau Urssaf de `mois` porte pour l'etablissement `nic` sur les paies
/// `paies`, une par code, tirees par la fonction qui ecrit le bordereau de la DSN (`lignes_bordereau`) : la taxe
/// d'apprentissage (992, 993) que lisent le solde d'avril et la contribution supplementaire de mars (`csa`).
pub fn assiettes_des_ctp(ctx: &Contexte, paies: &[Paie], mois: Mois, nic: &str, codes: &[CodeCtp]) -> R<Vec<Decimal>> {
    if paies.is_empty() {
        return Ok(vec![zero2(); codes.len()]);
    }
    let ctx = &ctx.du_mois(mois)?;
    let ent = &ctx.entreprise;
    let orgs = ctx.organismes()?;
    let bar = bar_du(ctx, ent, &orgs, nic, mois)?;
    let refs: Vec<&Paie> = paies.iter().flat_map(|p| p.elementaires()).collect();
    let lignes = lignes_bordereau(&refs, &orgs, ent, &bar, None, None, None, mois, &Seuils::du_mois(ctx, mois)?)?;
    Ok(codes.iter().map(|c| lignes.iter().filter(|l| l.code == *c).filter_map(|l| l.assiette).fold(zero2(), add)).collect())
}

/// Les assiettes des CTP `codes` que les DSN de l'annee `annee` declarent pour l'etablissement `nic`, une par code.
pub fn assiettes_de_l_annee(ctx: &Contexte, annee: i32, nic: &str, codes: &[CodeCtp], departs: &engagement::departs::Departs) -> R<Vec<Decimal>> {
    let mut total = vec![zero2(); codes.len()];
    for m in 1..=12 {
        let mois = Mois::nouveau(annee, m)?;
        let paies = paies_declarees_du_mois(ctx, mois, Some(nic), departs)?;
        for (t, a) in total.iter_mut().zip(assiettes_des_ctp(ctx, &paies, mois, nic, codes)?) {
            *t = add(*t, a);
        }
    }
    Ok(total)
}

/// La part principale de la taxe d'apprentissage hors Alsace-Moselle au bordereau (CTP 992 de la table de l'Urssaf),
/// l'assiette de son solde.
const CTP_TAXE_PRINCIPALE: CodeCtp = CodeCtp::du_moteur("992");

fn solde_taxe_apprentissage(ctx: &Contexte, mois: Mois, orgs: &Organismes, bar: &Pile, nic: &str, departs: &engagement::departs::Departs) -> R<Option<SoldeTa>> {
    if mois.numero() != 4 {
        return Ok(None);
    }
    let Some(taux) = bar.solde_taxe_apprentissage.as_ref().map(|s| s.taux) else { return Ok(None) };
    if !orgs.ctp_urssaf().iter().any(|c| c.code == CTP_TAXE_PRINCIPALE) {
        return Ok(None);
    }
    let annee = mois.annee() - 1;
    // Pas de solde d'une taxe que la fiche dit non due pour l'annee (R12, `assujettissement::taxe_apprentissage_non_due`).
    if droit::apprentissage::taxe_apprentissage_non_due(&ctx.entreprise, annee)? {
        return Ok(None);
    }
    let assiette = assiettes_de_l_annee(ctx, annee, nic, &[CTP_TAXE_PRINCIPALE], departs)?[0];
    if assiette.is_zero() {
        return Ok(None);
    }
    let org = bar
        .cotisations_dsn
        .iter()
        .find_map(|c| c.org.clone().filter(|_| c.code81 == Some(CodeCotisation81::AssuranceMaladie)))
        .ok_or_else(|| refus("Le solde de la taxe d'apprentissage se declare a l'Urssaf (81.002), et le bareme ne nomme l'Urssaf sur aucune cotisation 075. Rien n'est ecrit."))?;
    let montant = div(mul(assiette, taux), Decimal::ONE_HUNDRED)
        .round_dp_with_strategy(2, RoundingStrategy::MidpointNearestEven);
    Ok(Some(SoldeTa { annee, assiette, montant, org }))
}

// ------------------------------------------------------- les controles

/// CCH-13 et SIG-23, verifies ici plutot qu'au point de depot.
fn verifier_affiliations(texte: &str) -> R<()> {
    let mut affiliations: BTreeSet<(String, String)> = BTreeSet::new();
    let mut bases: BTreeSet<(String, String)> = BTreeSet::new();
    let mut vus: HashSet<(String, String, String, String)> = HashSet::new();
    let mut doublons: BTreeSet<String> = BTreeSet::new();
    let mut contrat = String::new();
    let mut courant: Option<[String; 4]> = None;
    for ligne in texte.lines() {
        let (rubrique, valeur) = match ligne.split_once(',') {
            Some((r, v)) => (r, v.trim().trim_matches('\'')),
            None => (ligne, ""),
        };
        match rubrique {
            "S21.G00.30.002" => vus.clear(),
            "S21.G00.40.009" => contrat = valeur.to_string(),
            "S21.G00.70.012" => {
                affiliations.insert((contrat.clone(), valeur.to_string()));
            }
            "S21.G00.78.001" => {
                // Sans 78.006 (contrat unique, p. 298), la base se rattache au contrat de l'individu : le dernier 40.009.
                courant = if BaseAssujettie::du_code(valeur) == Some(BaseAssujettie::PrevoyanceSanteRetraite) { Some([String::new(), String::new(), String::new(), contrat.clone()]) } else { None };
            }
            _ => {
                if let Some(c) = courant.as_mut() {
                    match rubrique {
                        "S21.G00.78.002" => c[0] = valeur.to_string(),
                        "S21.G00.78.003" => c[1] = valeur.to_string(),
                        "S21.G00.78.005" => c[2] = valeur.to_string(),
                        "S21.G00.78.006" => c[3] = valeur.to_string(),
                        "S21.G00.81.004" => {
                            bases.insert((c[3].clone(), c[2].clone()));
                            let signe = if valeur.starts_with('-') { "-" } else { "+" };
                            let cle = (c[2].clone(), c[0].clone(), c[1].clone(), signe.to_string());
                            if vus.contains(&cle) {
                                doublons.insert(c[2].clone());
                            }
                            vus.insert(cle);
                            courant = None;
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    if !doublons.is_empty() {
        return Err(refus_avec("Le barème de l'année déclare deux fois une même affiliation de prévoyance : le signaler au développeur.", Lieu { detail: Some(format!(
            "Affiliation(s) prevoyance declarees DEUX FOIS en base 31 pour un meme individu, meme periode et meme \
signe : {} (S21.G00.78.001/SIG-23).\n  Ce n'est admis que sur un contrat de mission (S21.G00.40.007 = 03). Les \
`cotisations_dsn` du bareme en declarent deux pour la meme affiliation.",
            doublons.into_iter().collect::<Vec<_>>().join(", ")
        )), ..Lieu::default() }));
    }
    let mut manquantes: Vec<String> = affiliations
        .iter()
        .filter(|(c, a)| !bases.contains(&(c.clone(), a.clone())))
        .map(|(c, a)| format!("{} (contrat {})", a, c))
        .collect();
    manquantes.sort();
    if !manquantes.is_empty() {
        let mut dites: Vec<String> = bases.iter().map(|(_, a)| a.clone()).collect();
        dites.sort();
        return Err(refus_avec("Le barème de l'année ne déclare pas la base de chaque affiliation de prévoyance : le signaler au développeur.", Lieu { detail: Some(format!(
            "Affiliation(s) prevoyance sans base assujettie de code 31 : {} (S21.G00.78.001/CCH-13).\n  Le bloc 70 \
declare {} affiliation(s) et le bareme ne donne de cotisation 31 que pour {}. La plate-forme refuse la declaration \
entiere.\n  Une base 31 par affiliation, meme a 0.00, et sous chacune une et une seule cotisation 81 de code 059 \
(CCH-15) : c'est aux `cotisations_dsn` du bareme de les dire.",
            manquantes.join(", "),
            if affiliations.is_empty() { "aucune".to_string() } else { affiliations.len().to_string() },
            if dites.is_empty() { "aucune".to_string() } else { dites.join(", ") }
        )), ..Lieu::default() }));
    }
    Ok(())
}

// ----------------------------------------------------------- la generation

/// UN ORDRE DE PAIEMENT (bloc S21.G00.20) tel que la DSN l'ecrit : a qui (20.001), combien (20.005), pour quelle periode
/// (20.006, 20.007), pour quand (20.011), et le credit que la somme due laisse (`bordereau::credit`) : ce que la compagnie
/// a verse en trop et que l'ordre ne reprend pas (`reste`), ce qu'il reprend deja (`deduit`, l'Urssaf seulement).
#[derive(Clone, Debug)]
pub struct Ordre {
    pub organisme: IdentifiantDOrganisme,
    pub reference: Option<String>,
    pub montant: Decimal,
    pub debut: Jour,
    pub fin: Jour,
    pub date_paiement: Option<Jour>,
    pub destinataire: Destinataire,
    pub reste: Decimal,
    pub deduit: Decimal,
}

/// UN BORDEREAU URSSAF (blocs S21.G00.22 et 23) tel que la DSN l'ecrit : la periode qu'il couvre (22.003, 22.004), son
/// total (22.005) et ses lignes 23. Le premier est celui du mois declare ; les suivants, ceux des mois que ses
/// corrections regularisent.
#[derive(Clone, Debug)]
pub struct BordereauEcrit {
    pub debut: Jour,
    pub fin: Jour,
    pub total: Decimal,
    pub lignes: Vec<LigneBordereau>,
}

/// Une DSN fabriquee : son texte, ses ordres de paiement dans l'ordre du fichier, et ses bordereaux Urssaf.
pub struct Fabriquee {
    pub texte: String,
    pub ordres: Vec<Ordre>,
    pub bordereaux: Vec<BordereauEcrit>,
}

/// Le texte de la DSN (`fabriquer`).
pub fn generer(ctx: &Contexte, paies: &[Paie], mois: Mois, regs: &[Regularisation], arrets: &[Arret], o: &Options, departs: &engagement::departs::Departs) -> R<String> {
    Ok(fabriquer(ctx, paies, mois, regs, arrets, o, departs)?.texte)
}

fn echeance_mois_suivant(mois: Mois, jour: u32) -> Jour {
    let suivant = mois.suivant();
    // Un jour que le mois n'a pas (le 31) : son dernier jour.
    match suivant.jour(jour) {
        Some(j) => j,
        None => suivant.dernier_jour(),
    }
}

/// LE BAREME D'UN MOIS POUR UN ETABLISSEMENT : le bareme empile (`Contexte::bareme`) surcharge par ce que la fiche
/// entreprise dit de l'etablissement (`bareme::surcharger` : son taux AT/MP, son versement mobilite). Dans la DSN, la
/// seule facon de l'obtenir (regle 8 : chaque etablissement a son taux AT/MP) : le bordereau courant, le bordereau
/// regularise et le nominatif (`bar_du`) passent par elle. (La pile
/// d'un salarie, `bareme::bareme_applique`, surcharge de meme par l'etablissement du salarie.)
fn bareme_du_nic(ctx: &Contexte, ent: &ChampsEntreprise, nic: &str, mois: Mois) -> R<Pile> {
    bareme::surcharger((*ctx.bareme(&mois.premier_jour())?).clone(), ent, nic)
}

/// LE BLOC 40 DECLARE du contrat de `p` (regle 4 : il se lit, il ne se recalcule pas), pour le bloc Changements contrat
/// de la mensuelle de `mois` : la DSN du mois le plus recent avant `mois` qui est partie pour son engagement, et la paie
/// de ce mois telle que la version sur laquelle elle est partie l'a ecrite (`engagement::etats::paie_declaree`) ; son
/// bloc 40 s'ecrit comme il s'est ecrit ce mois-la, de la fiche du salarie telle que ce calcul l'a lue. `etats` : les
/// engagements de `mois` ; `None` : rien de declare.
fn bloc_40_declare(ctx: &Contexte, p: &Paie, mois: Mois, etats: &[engagement::etats::Etat], departs: &engagement::departs::Departs, orgs: &Organismes) -> R<Option<crate::changements_contrat::Declare>> {
    // Un contrat sans numero n'a rien declare : le numero est la cle de ce qui est parti.
    let Some(numero) = p.engagement.numero.clone() else { return Ok(None) };
    let cle = cle_salarie(p);
    let sienne = |e: &engagement::etats::Etat| {
        e.objet().carte().salarie.as_ref() == Some(&cle) && e.objet().carte().numero.as_deref() == Some(numero.as_str())
    };
    let Some(v) = etats.iter().filter(|e| sienne(e)).find_map(engagement::etats::Etat::valide) else { return Ok(None) };
    let premier_mois = p.engagement.debut.mois_civil();
    let mut m = mois.precedent();
    while m >= premier_mois {
        // La compagnie telle qu'a la fin du mois declare (l'entreprise, ses etablissements) ; la fiche, celle que la paie declaree a lue.
        let ctx_m = Contexte::nouveau(&ctx.racine, Some(m.dernier_jour()))?;
        if let Some(declaree) = engagement::etats::paie_declaree(&ctx_m, v.objet(), departs, m)? {
            // Les autres contrats du salarie declares ce mois-la : le code emplois multiples (40.036) s'ecrit avec eux.
            let mut autres = Vec::new();
            for e in engagement::etats::tels_qu_ecrits(ctx, m, departs)? {
                let Some(w) = e.valide() else { continue };
                if w.objet().fichier == v.objet().fichier && w.objet().rang == v.objet().rang {
                    continue;
                }
                if let Some(q) = engagement::etats::paie_declaree(&ctx_m, w.objet(), departs, m)? {
                    autres.push(q);
                }
            }
            let nic = bareme::etablissement_de(&ctx_m.entreprise, &declaree.salarie)?;
            let bar = bar_du(&ctx_m, &ctx_m.entreprise, orgs, &nic, m)?;
            let mut ent = ctx_m.entreprise.clone();
            ent.licence_spectacle = ctx.licence(&m.premier_jour());
            let ent_e = bareme::fiche_de_l_etablissement(&ent, &nic)?;
            let mut f = Fichier::en_norme(crate::norme::de_la_mensuelle(m)?);
            let refs: Vec<&Paie> = autres.iter().collect();
            bloc_contrat(&mut f, &declaree, &ent_e, orgs, 1, &bar, true, &[], NatureDeLaDeclaration::Mensuelle, emplois_multiples(&declaree, &refs))?;
            let bloc: Vec<(String, String)> = f
                .lignes
                .iter()
                .filter_map(|l| l.split_once(','))
                .filter(|(k, _)| k.starts_with("S21.G00.40."))
                .map(|(k, x)| (k.to_string(), x.trim_matches('\'').to_string()))
                .collect();
            return Ok(Some(crate::changements_contrat::Declare { mois: m, siret: ctx_m.entreprise.siren.ecrite("siren")?.trim().to_string() + &nic, nic, bloc }));
        }
        m = m.precedent();
    }
    Ok(None)
}

/// Le bareme d'un mois pour l'etablissement, avec ses contributions.
pub fn bar_du(ctx: &Contexte, ent: &ChampsEntreprise, orgs: &Organismes, nic: &str, m: Mois) -> R<BaremeDuMois> {
    let lue = ctx.entreprise_lue(&m.premier_jour())?;
    let b = bareme_du_nic(ctx, ent, nic, m)?;
    let c = contributions_dsn(ctx, ent, orgs, m, &b)?;
    let mut bar = BaremeDuMois::nouveau(b, lue);
    bar.contributions_du_mois = c;
    Ok(bar)
}

fn ordre_salaries(paies: &[Paie]) -> Vec<IdSalarie> {
    let mut vus: Vec<IdSalarie> = Vec::new();
    for p in paies {
        let c = cle_salarie(p);
        if !vus.contains(&c) {
            vus.push(c);
        }
    }
    vus
}

/// CE QUE LA DSN EXIGE DES FICHES (`exige_par` de `champs-base.json`, la liste que « Pour demarrer » lit) : tout ce qui
/// manque, d'un coup, avant d'ecrire ; le numero d'adherent Conges Spectacles, seulement quand un contrat est declare a la
/// caisse (`caisse_conges`). La DSN mensuelle et l'amorcage (`amorcage::generer`) declarent le meme individu.
pub fn exiges<'a>(ent: &ChampsEntreprise, salaries: impl Iterator<Item = &'a droit::fiche::SalarieDuContrat>, caisse_conges: bool) -> R<()> {
    let mut vues: Vec<&droit::fiche::SalarieDuContrat> = Vec::new();
    for s in salaries {
        if !vues.iter().any(|v| v.id == s.id) {
            vues.push(s);
        }
    }
    let nomme = |s: &droit::fiche::SalarieDuContrat| match s.nomme() {
        Some(n) => n,
        None => s.id.to_string(),
    };
    let mut saisies = vec![(droit::fiche::NomDeFiche::Entreprise, "la fiche entreprise".to_string(), crate::champs::Saisie::de_la_fiche(droit::fiche::NomDeFiche::Entreprise, ent)?)];
    for s in &vues {
        saisies.push((droit::fiche::NomDeFiche::Salarie, format!("la fiche de {}", nomme(s)), crate::champs::Saisie::de_la_fiche(droit::fiche::NomDeFiche::Salarie, &s.champs)?));
    }
    let fiches: Vec<(droit::fiche::NomDeFiche, String, &crate::champs::Saisie)> = saisies.iter().map(|(n, q, s)| (*n, q.clone(), s)).collect();
    let sauf: Vec<droit::fiche::CaseDeFiche> = if caisse_conges { Vec::new() } else { vec![documents::conges_spectacles::CLE_ADHERENT.into()] };
    match crate::champs::refus_des_manques("dsn", &fiches, &sauf)? {
        Some(m) => Err(refus(m)),
        None => Ok(()),
    }
}

/// La DSN mensuelle de `mois` : toutes les paies du mois de cet etablissement.
///
/// Ce que la norme et les depots de test nous ont appris. Les parametres sont `o.etablissement`, `arrets`,
/// `regs`, `o.envoi_test`, `o.remplace`, `o.ordre`.
///
/// `paies` : toutes les paies du mois.
///
/// `etablissement` : le NIC de l'etablissement que CETTE declaration porte. Une compagnie qui en a
/// plusieurs depose une DSN par etablissement : le bloc 11, le lieu de
/// travail (40.019), le taux AT/MP et le versement mobilite sont les siens,
/// et `paies` ne contient que ses salaries -- un salarie d'un autre
/// etablissement est refuse. Le bloc 06 reste celui de l'entreprise (le
/// siege), et l'OETH ne part qu'avec le principal (D5212-8). Sans
/// `etablissement`, c'est le principal.
///
/// `arrets` : les arrets de travail (`signalement.Arret`) qui ont une periode
/// commune avec ce mois. La DSN mensuelle doit les REPORTER, meme quand un
/// signalement les a deja annonces : cahier technique 2026.1 §2.5.2, << il
/// est necessaire de reporter les informations d'un signalement dans la DSN
/// mensuelle consecutive [...] afin que l'evenement signale soit pris en
/// compte dans le traitement d'un evenement futur >>. Et bloc 60 : << en cas
/// de suspension supra-mensuelle, la suspension doit etre declaree dans
/// TOUTES les DSN mensuelles dont le mois principal declare presente une
/// periode commune avec la suspension >>. Un arret non reporte, c'est une
/// mensuelle qui contredit le signalement -- et c'est elle qui fait
/// l'historique de vie professionnelle du salarie.
///
/// `regularisations` : les corrections de mois DEPOSES dont l'echeance est
/// passee, a porter par CETTE declaration (chacune : mois d'origine, paie AVANT telle que deposee, paie
/// APRES corrigee). Cahier technique 2026.1 §2.2.2.5 : « une fois passee
/// la date d'exigibilite, les corrections doivent etre portees par une DSN
/// mensuelle ulterieure », par les memes blocs DATES DE LA PERIODE
/// D'ORIGINE, et « une unique modalite de regularisation » pour toute la
/// declaration. La modalite retenue est l'ANNULATION-REMPLACEMENT : pour
/// chaque correction, les blocs de la paie d'avant en negatif, puis ceux
/// de la paie d'apres en positif -- deux ecritures que n'importe qui peut
/// relire, la ou une difference ne se verifie qu'en refaisant le calcul.
/// Voir `bloc_regularisation` et `lignes_regularisees` ci-dessous.
///
/// Le prelevement d'une paie est celui qu'elle a ecrit a la validation (`Paie::pas`), celui du versement d'une
/// correction celui que l'engagement rend (`engagement::etats::prelevement_de_la_correction`) : la DSN n'en calcule
/// aucun. Seul le versement nul d'un mois sans paie porte le taux qui vaut pour le salarie, lu ici :
/// les taux recus de sa fiche (`pas::taux_dgfip`), tant que son taux est VALIDE ; faute d'un taux valide, la
/// grille de taux par defaut de l'article 204 H du CGI (`pas_du_versement`, ci-dessous).
///
/// `envoi_test` met S10.G00.00.005 a '01' : controles executes et bilan rendu,
/// donnees transmises aux organismes pour analyse mais NON CONSERVEES. Un
/// fichier code '02' est traite comme reel, quoi qu'on en dise par ailleurs.
///
/// ANNULE ET REMPLACE. `type_declaration='03'` et un `ordre` superieur a
/// celui de la declaration remplacee. C'est TOUT ce qu'il faut : la
/// plate-forme retrouve la declaration a remplacer par son mois, sa fraction
/// et son SIREN.
///
/// LA RUBRIQUE S20.G00.05.006 EST INTERDITE ICI. C'est ecrit noir sur blanc
/// dans le « tableau des usages » du cahier technique 2026.1, p. 362-363 :
/// par colonne, pour cette rubrique, on lit « I C C C C I C », et la premiere
/// colonne est « 01 - DSN mensuelle ». I = interdite. Elle n'est
/// conditionnelle que pour les signalements (arret de travail, reprise, fin
/// de contrat, amorcage) et la DPAE.
/// Le cahier technique la decrit comme « l'identifiant de la derniere
/// declaration, relative au meme EVENEMENT, validee par le point de depot ».
/// « Evenement » est le vocabulaire des SIGNALEMENTS (fin de contrat, arret
/// de travail), pas celui d'une DSN mensuelle. Nous avons lu la definition
/// trop vite, rempli la rubrique, et depose en test. Reponse de la
/// plate-forme, deux anomalies bloquantes :
///
/// ```text
/// CST-04      Presence de la rubrique interdite S20.G00.05.006.
/// B1-208-08   'annule et remplace' ou 'annule' sans normale prealable.
/// ```
///
/// D'ou `remplace` : on peut le passer, mais uniquement pour un signalement,
/// et le moteur n'en emet pas encore. Il est donc REFUSE pour une mensuelle.
/// Le jour ou un signalement sera ecrit, cette porte est deja la -- et le
/// commentaire dit pourquoi elle etait fermee.
///
/// LE NUMERO D'ORDRE EST LE SEUL RATTACHEMENT -- etabli en
/// deposant en test, et lisible dans une troisieme anomalie :
///
/// ```text
/// B1-208-11   'annule et remplace' d'ordre 2026092 inferieur (ou egal)
///             au dernier ordre accepte 2026092.
/// ```
///
/// La plate-forme construit donc un rang « AAAAMM + ordre » a partir du MOIS
/// DE CONSTITUTION, et elle exige qu'un annule-et-remplace soit STRICTEMENT
/// superieur au dernier rang accepte pour cet etablissement. C'est ce rang,
/// et lui seul, qui designe la declaration remplacee : il n'y a rien d'autre
/// a fournir, et c'est pour ca que S20.G00.05.006 est interdite.
///
/// Consequence pratique : `ordre` ne se met pas a 2 par habitude. Il se
/// choisit au-dessus du dernier ordre ACCEPTE du mois civil en cours, qui se
/// lit sur le certificat de conformite du depot precedent (la releve des retours).
///
/// IL REMPLACE, IL N'AJOUTE PAS -- Y COMPRIS LES ORDRES DE TELEPAIEMENT.
/// Le cahier technique 2026.1, p. 15 : « lorsque la derniere DSN mensuelle transmise pour une date
/// d'exigibilite donnee est de type "Annule et remplace integral", elle doit
/// porter les elements de cotisations destines aux organismes de recouvrement,
/// MEME SI ces elements ont deja ete transmis dans une DSN mensuelle relative
/// a la meme date d'exigibilite. » C'est la DERNIERE qui vaut. Exiger qu'elle
/// re-porte ce qui a deja ete transmis n'aurait aucun sens si l'on
/// additionnait. Le mecanisme le confirme : le telepaiement s'execute a la
/// DATE D'EXIGIBILITE, pas au depot -- l'ordre precedent n'a jamais ete
/// execute, il est remplace en attente.
///
/// ET SON ECHEANCE N'EST PAS CELLE D'UNE DSN NORMALE -- cahier technique,
/// p. 13. Une DSN « normale » se depose jusqu'au jour de l'echeance (5 ou 15)
/// a midi ; un annule-et-remplace, lui, « au plus tard la veille a minuit ».
/// Au-dela, la rectification se porte dans les paies des mois suivants
/// (`moteur corriger`). Le moteur connait l'echeance de l'etablissement
/// (`Contexte::echeance_dsn` : le 5 a cinquante salaries, le 15 sinon --
/// critere infere, la norme ne le donne pas, fiche N7 § 1.4.2) et sa veille
/// (`routes::etapes::Moteur::limite`) : passe cette veille, il refuse de produire
/// l'annule-et-remplace (`depart::forme_a_produire`) comme de le
/// deposer (`depot_acte`), « AnnuleRemplaceHorsDelai ». Ce n'est pas ici, dans
/// `generer`, qui ecrit ce qu'on lui donne, mais avant lui.
///
/// ET EN TEST, UN ANNULE-ET-REMPLACE SE PREPARE PAR UNE REPETITION. Le meme
/// jour, la premiere tentative avait rendu :
///
/// ```text
/// B1-208-08   'annule et remplace' ou 'annule' sans normale prealable.
/// ```
///
/// alors que la declaration d'aout existait bel et bien, deposee le 03/09 et
/// certifiee conforme. L'anomalie a disparu des qu'une declaration NORMALE a
/// ete deposee EN TEST juste avant. Le mode test a sa propre base : il ne
/// voit pas les declarations reelles. On teste donc un annule-et-remplace
/// comme on le deposera : la normale en test d'abord, puis l'annule-et-
/// remplace en test (la repetition). Il passe alors les memes controles
/// qu'en reel, rattachement compris : le test vaut le reel. Ce que le test ne controle pas, c'est
/// le montant.
pub fn fabriquer(ctx: &Contexte, paies: &[Paie], mois: Mois, regs: &[Regularisation], arrets: &[Arret], o: &Options, departs: &engagement::departs::Departs) -> R<Fabriquee> {
    let ctx = &ctx.du_mois(mois)?;
    let ent0 = &ctx.entreprise;
    let principal_nic = ent0.nic.ecrite("nic")?;
    let nic = match o.etablissement.as_deref().filter(|n| !n.is_empty()) {
        Some(n) => n.to_string(),
        None => principal_nic.to_string(),
    };
    let principal = nic == principal_nic;
    if !principal && ent0.etablissement(&nic).is_none() {
        return Err(refus!("Etablissement {} inconnu de la fiche entreprise (connus : {}).", nic, ent0.nics()?.join(", ")));
    }
    let mut egares = BTreeSet::new();
    for s in paies.iter().map(|p| &p.salarie).chain(regs.iter().map(|r| &r.apres.salarie)) {
        if bareme::etablissement_de(ent0, s)? != nic {
            egares.insert(s.id.to_string());
        }
    }
    if !egares.is_empty() {
        let n = egares.len();
        return Err(socle::echec::cle(format!(
            "DSN de l'etablissement {} : {} n'y appartien{} pas (l'établissement de sa fiche). Une DSN par \
etablissement, chacun avec ses salaries. Rien n'est ecrit.",
            nic,
            egares.into_iter().collect::<Vec<_>>().join(", "),
            if n > 1 { "nent" } else { "t" }
        ), "etablissement"));
    }
    let caisse = paies.iter().map(Paie::declaree_a_la_caisse_des_conges).collect::<socle::echec::R<Vec<bool>>>()?.contains(&true);
    exiges(ent0, paies.iter().map(|p| &p.salarie).chain(regs.iter().map(|r| &r.apres.salarie)), caisse)?;
    // La correction d'un mois decoupe en periodes elementaires : le declare relu (`versement_declare`) et le
    // corrige se comparent base par base, et un mois decoupe en a une par periode. Un chantier a part.
    if let Some(r) = regs.iter().find(|r| !r.apres.periodes.is_empty() || !r.avant.periodes.is_empty()) {
        return Err(refus!(
            "Correction de {} (contrat {}) : la paie de ce mois se decoupe en periodes elementaires (un changement de \
statut, de categorie ou de duree du travail en cours de mois). La regularisation d'un tel mois, base par base et periode \
par periode, n'est pas faite par le logiciel. Rien n'est ecrit.",
            r.mois,
            r.apres.engagement.numero_declare()?
        ));
    }
    // Les etablissements, celui de cette DSN d'abord : un contrat declare ailleurs a change d'etablissement (bloc 41).
    let nics_courant_d_abord: Vec<String> = std::iter::once(nic.clone()).chain(ent0.nics()?.into_iter().filter(|n| *n != nic)).collect();
    // Ce qui est parti, et les engagements du mois : le bloc 40 declare de chaque contrat se lit dans sa version (`bloc_40_declare`).
    let etats_du_mois = engagement::etats::tels_qu_ecrits(ctx, mois, departs)?;
    let orgs = ctx.organismes()?;
    let debut_mois = mois.premier_jour();
    // La version de la norme suit le mois principal declare (`norme-dsn-base.json`, une inference : sa `note`) ; un
    // mois que la donnee ne couvre pas refuse ici, avant tout calcul. Un annule-et-remplace prend celle de son mois.
    let norme = crate::norme::de_la_mensuelle(mois)?;
    // En P27V01, la fiche entreprise est en NAF 2025 (`naf::verifier_pour`), avant tout calcul.
    crate::naf::verifier_pour(&norme, ent0)?;
    let fin_m = mois.dernier_jour();
    let mut bars: HashMap<Mois, std::sync::Arc<BaremeDuMois>> = HashMap::new();
    let mut bar_de = |m: Mois| -> R<std::sync::Arc<BaremeDuMois>> {
        if let Some(b) = bars.get(&m) {
            return Ok(b.clone());
        }
        let b = std::sync::Arc::new(bar_du(ctx, ent0, &orgs, &nic, m)?);
        bars.insert(m, b.clone());
        Ok(b)
    };
    let bar = bar_de(mois)?;
    let contribs = bar.contributions_du_mois.clone();
    let mut ent = ent0.clone();
    ent.licence_spectacle = ctx.licence(&debut_mois);
    let ent_e = bareme::fiche_de_l_etablissement(&ent, &nic)?;

    let mut arrets_du_mois = Vec::new();
    for a in arrets {
        if a.a_reporter(mois, departs)? {
            arrets_du_mois.push(a);
        }
    }
    // Un arret d'un mois anterieur se reporte sous son contrat, que la DSN declare ce mois-ci ou corrige (sa
    // regularisation) ; un arret du mois sans son contrat n'aurait pas de place.
    let mut orphelins: Vec<&&Arret> = Vec::new();
    for a in &arrets_du_mois {
        let mut porte = false;
        for p in paies.iter().chain(regs.iter().map(|r| &r.apres)) {
            porte |= a.du_contrat(p)?;
        }
        if !porte {
            orphelins.push(a);
        }
    }
    if !orphelins.is_empty() {
        let liste_o: Vec<String> = orphelins
            .iter()
            .map(|a| {
                let s = &a.saisi;
                let fin = match s.reprise {
                    Some(r) => r,
                    None => s.fin_previsionnelle,
                };
                let numero = match s.contrat_numero.as_deref() {
                    Some(n) => n,
                    None => "sans numero",
                };
                format!("{} contrat {} ({} -> {})", s.salarie, numero, s.dernier_jour_travaille.iso(), fin.iso())
            })
            .collect();
        return Err(refus!(
            "Arret de travail a reporter dans la DSN de {}, mais son contrat n'est pas declare ce mois-ci : {}. Le \
bloc « Arret de travail - S21.G00.60 » se declare SOUS le contrat suspendu. Reprendre le contrat dans la saisie du \
mois (il y figure sans cachet), ou corriger les dates de l'arret. Rien n'est ecrit.",
            mois,
            liste_o.join(", ")
        ));
    }
    let mut manquants: Vec<String> = Vec::new();
    for p in paies {
        if p.remuneration.iter().any(|l| l.arret_de_travail) && arrets_du_contrat(&arrets_du_mois, p)?.is_empty() {
            manquants.push(format!("{} contrat {}", cle_salarie(p), p.engagement.numero_declare()?));
        }
    }
    if !manquants.is_empty() {
        return Err(refus!(
            "Absence pour arret de travail dans la paie de {}, mais aucun arret saisi en face : {}. La ligne retire \
le salaire des heures non travaillees ; c'est le bloc « Arret de travail - S21.G00.60 » qui ouvre les indemnites \
journalieres, et il se construit depuis l'arret, pas depuis la ligne. Saisir l'arret dans les imprévus du salarié, ou \
retirer la ligne. Rien n'est ecrit.",
            mois,
            manquants.join(", ")
        ));
    }
    // LA SUSPENSION D'UN CONTRAT QUE CE MOIS NE DECLARE PAS : le bloc 65 se declare SOUS le contrat suspendu,
    // dans chaque DSN dont le mois croise la suspension (bloc S21.G00.65, cahier technique DSN 2026.1, p. 281,
    // `atelier/reference/norme-dsn/texte/bloc-S21.G00.65.txt`) ; il se tire du magasin des imprevus pour le contrat de
    // chaque paie (`imprevus::suspensions_du_mois`). Une suspension de cet etablissement dont le contrat manque ne serait
    // declaree nulle part : refus, comme pour l'arret.
    let mut suspendus_absents = Vec::new();
    for imp in engagement::imprevus::du_mois(&ctx.racine, mois, Some(engagement::imprevus::SUSPENSION))? {
        let fiche = engagement::imprevus::fiche_du_salarie(ctx, &imp)?;
        if bareme::etablissement_de(ent0, &fiche)? != nic {
            continue;
        }
        let mut declare = false;
        for p in paies {
            if engagement::imprevus::du_contrat(&imp, &cle_salarie(p), p.engagement.numero.as_deref(), &p.engagement.debut)? {
                declare = true;
            }
        }
        if let (false, engagement::imprevus::Imprevu::Suspension(s)) = (declare, &imp) {
            suspendus_absents.push(format!(
                "{} contrat{} (suspension {} du {} au {})",
                s.salarie,
                s.contrat_numero.iter().map(|n| format!(" {}", n)).collect::<String>(),
                s.motif,
                s.debut.iso(),
                s.fin.iso()
            ));
        }
    }
    if !suspendus_absents.is_empty() {
        return Err(refus!(
            "Suspension a declarer dans la DSN de {}, mais son contrat n'est pas declare ce mois-ci : {}. Le bloc « Autre \
suspension de l'execution du contrat - S21.G00.65 » se declare SOUS le contrat suspendu, dans chaque mois qu'elle \
traverse. Reprendre le contrat dans la saisie du mois, ou corriger les dates de la suspension. Rien n'est ecrit.",
            mois,
            suspendus_absents.join(", ")
        ));
    }
    let sans_individu = paies.is_empty() && regs.is_empty();
    let constitution = match o.date_constitution {
        Some(d) => d,
        // La date de constitution (S20.G00.05.007) est le JOUR OU LE FICHIER SE FABRIQUE, comme celle des
        // signalements et de la FCTU : une DSN produite le 07/10 est « constituee le 07/10 ».
        None => aujourdhui()?,
    };
    let type_declaration = match (sans_individu, o.type_declaration) {
        (true, TypeDeLaDeclaration::Normale) => TypeDeLaDeclaration::NormaleSansIndividu,
        (true, TypeDeLaDeclaration::AnnuleEtRemplace) => TypeDeLaDeclaration::AnnuleEtRemplaceSansIndividu,
        (_, t) => t,
    };
    if o.remplace.as_deref().is_some_and(|r| !r.is_empty()) {
        return Err(refus("S20.G00.05.006 est interdite sur une DSN mensuelle : la plate-forme a repondu CST-04 le \
10/09/2026. Un annule et remplace se code par le type de declaration 03 et un numero d'ordre superieur, sans \
identifiant de declaration remplacee."));
    }
    if type_declaration.remplace() && o.ordre < 2 {
        return Err(refus!(
            "Un annule et remplace doit porter un numero d'ordre STRICTEMENT superieur au dernier ordre accepte du \
mois civil en cours (recu ordre={}). La plate-forme rend B1-208-11 sinon ; le dernier ordre accepte se lit sur le \
certificat de conformite du depot precedent.",
            o.ordre
        ));
    }

    let mut pas_par_salarie: HashMap<IdSalarie, Option<Triplet>> = HashMap::new();
    for cle in paies.iter().filter(|p| p.engagement.sans_paie()).map(cle_salarie) {
        if !pas_par_salarie.contains_key(&cle) {
            let t = pas::taux_dgfip(ctx, &cle, None, Some(mois))?;
            pas_par_salarie.insert(cle, t);
        }
    }
    let mut f = Fichier::en_norme(norme);

    f.add("S10.G00.00.001", LOGICIEL);
    f.add("S10.G00.00.002", LOGICIEL);
    f.add("S10.G00.00.003", VERSION);
    f.add("S10.G00.00.005", if o.envoi_test { "01" } else { "02" });
    f.ecrire_la_version()?;
    f.add("S10.G00.00.007", "01");
    f.add("S10.G00.00.008", if sans_individu { "02" } else { "01" });
    f.add_case("S10.G00.01.001", &ent.siren, "siren")?;
    f.add_case("S10.G00.01.002", &ent.nic, "nic")?;
    f.add_case("S10.G00.01.003", &ent.raison_sociale, "raison_sociale")?;
    f.add_case("S10.G00.01.004", &ent.adresse, "adresse")?;
    f.add_case("S10.G00.01.005", &ent.code_postal, "code_postal")?;
    f.add_case("S10.G00.01.006", &ent.commune, "commune")?;
    f.add_complement("S10.G00.01.009", &ent.complement_adresse);
    contact_emetteur(&mut f, &ent)?;

    f.ecrire_code(NatureDeLaDeclaration::Mensuelle)?;
    f.ecrire_code(type_declaration)?;
    f.add("S20.G00.05.003", &o.fraction);
    f.add("S20.G00.05.004", o.ordre);
    f.add("S20.G00.05.005", debut_mois.jjmmaaaa());
    f.add("S20.G00.05.007", constitution.jjmmaaaa());
    f.add("S20.G00.05.008", "01");
    f.add("S20.G00.05.010", "01");
    crate::dsn::contact_chez_le_declare(&mut f, &ent)?;
    contact_sante_travail(&mut f, &ent_e)?;
    if sans_individu {
        for code in orgs.absence_de_rattachement() {
            f.add("S20.G00.08.001", code.as_str());
        }
    }

    f.add_case("S21.G00.06.001", &ent.siren, "siren")?;
    f.add_case("S21.G00.06.002", &ent.nic, "nic")?;
    // Le code APEN (06.003) et le code APET (11.002) : obligatoires en P26V01, retires de la norme P27V01 (cahier
    // 2027.1.1, p. 142 et 145 ; usages p. 371 ). Le tableau des usages de la version du fichier en decide.
    if usage_admet(&f, "S21.G00.06.003", NatureDeLaDeclaration::Mensuelle)? {
        f.add_case("S21.G00.06.003", &ent.naf, "naf")?;
    }
    f.add_case("S21.G00.06.004", &ent.adresse, "adresse")?;
    f.add_case("S21.G00.06.005", &ent.code_postal, "code_postal")?;
    f.add_case("S21.G00.06.006", &ent.commune, "commune")?;
    f.add_complement("S21.G00.06.007", &ent.complement_adresse);
    let doeth = if principal { crate::oeth::declaration(ctx, mois)? } else { None };
    // La contribution supplementaire a l'apprentissage, sur l'etablissement principal seul (fiches 2588 et 2963 :
    // « sur un seul établissement »).
    let csa = if principal { crate::csa::declaration(ctx, mois, departs)? } else { None };
    if let Some(d) = &doeth {
        if let Some(a) = d.accord.as_deref().filter(|a| !a.is_empty()) {
            f.add("S21.G00.13.001", a);
            f.add("S21.G00.13.004", d.millesime);
        }
        for (type_boeth, nombre) in &d.externes {
            f.add("S21.G00.13.002", type_boeth);
            f.add("S21.G00.13.003", mt(*nombre));
            f.add("S21.G00.13.004", d.millesime);
        }
    }
    for (rubrique, case, cle) in [
        ("S21.G00.11.001", &ent_e.nic.en_texte(), "nic"),
        ("S21.G00.11.002", &ent_e.naf.en_texte(), "naf"),
        ("S21.G00.11.003", &ent_e.adresse.en_texte(), "adresse"),
        ("S21.G00.11.004", &ent_e.code_postal.en_texte(), "code_postal"),
        ("S21.G00.11.005", &ent_e.commune.en_texte(), "commune"),
        ("S21.G00.11.022", &ent_e.idcc.en_texte(), "idcc"),
    ] {
        if rubrique == "S21.G00.11.002" && !usage_admet(&f, rubrique, NatureDeLaDeclaration::Mensuelle)? {
            continue;
        }
        f.add_case(rubrique, case, cle)?;
        if rubrique == "S21.G00.11.005" {
            f.add_complement("S21.G00.11.006", &ent_e.complement_adresse);
        }
    }
    if let Some(spst) = spst_ecrit(&f, &ent_e)? {
        f.add("S21.G00.11.025", spst);
    }
    let (premier_du_mois, dernier_du_mois) = bornes_du_mois(mois);
    let mut couvertes: HashSet<String> = HashSet::new();
    // Les adhesions qu'au moins une affiliation du fichier reference : celles des salaries affilies ce mois-la
    // (`rattachements`), pas toutes celles de leur nature (CCH-13 et CCH-14 de 15.004, p. 151).
    for p in paies {
        let refs = affiliations(&orgs, &*p.engagement.nat()?, &contribs, (p.engagement.debut_periode(), p.engagement.fin_periode()))?;
        couvertes.extend(rattachements(p, &orgs, &bar.entreprise.fiche, refs)?.into_iter().map(|a| a.reference));
    }
    for r in regs {
        let b = bar_de(r.mois)?;
        let refs = affiliations(&orgs, &*r.apres.engagement.nat()?, &b.contributions_du_mois, (r.apres.engagement.debut_periode(), r.apres.engagement.fin_periode()))?;
        couvertes.extend(rattachements(&r.apres, &orgs, &b.entreprise.fiche, refs)?.into_iter().map(|a| a.reference));
    }
    for a in &orgs.adhesions_prevoyance {
        let reference = a.reference.clone();
        if !couvertes.contains(&reference) && !orgs.adhesion_en_vigueur(&reference, &premier_du_mois, &dernier_du_mois) {
            continue;
        }
        f.add("S21.G00.15.001", &a.libelle);
        f.add("S21.G00.15.002", &a.delegataire);
        f.add("S21.G00.15.004", if sans_individu || !couvertes.contains(&reference) { "02" } else { "01" });
        f.add("S21.G00.15.005", &a.reference);
    }

    let solde_ta = solde_taxe_apprentissage(ctx, mois, &orgs, &bar, &nic, departs)?;
    if let Some(s) = &solde_ta {
        f.add("S21.G00.82.001", mt(s.montant));
        f.ecrire_code(droit::cotisation_etablissement::CotisationDEtablissement::SoldeTaxeDApprentissage)?;
        f.add("S21.G00.82.003", format!("0101{}", s.annee));
        f.add("S21.G00.82.004", format!("3112{}", s.annee));
        f.add("S21.G00.82.005", &s.org);
    }
    let versements = orgs.versements();
    if let Some(d) = &doeth {
        let org_urssaf = orgs.urssaf()?.siret.clone();
        for (code, montant) in d.blocs_82() {
            f.add("S21.G00.82.001", mt(montant));
            f.ecrire_code(code)?;
            f.add("S21.G00.82.003", format!("0101{}", d.millesime));
            f.add("S21.G00.82.004", format!("3112{}", d.millesime));
            f.add("S21.G00.82.005", &org_urssaf);
        }
    }
    if let Some(c) = &csa {
        let org_urssaf = orgs.urssaf()?.siret.clone();
        let (code, montant) = c.bloc_82();
        f.add("S21.G00.82.001", mt(montant));
        f.ecrire_code(code)?;
        f.add("S21.G00.82.003", format!("0101{}", c.millesime));
        f.add("S21.G00.82.004", format!("3112{}", c.millesime));
        f.add("S21.G00.82.005", &org_urssaf);
    }

    let mut totaux = Vec::new();
    // Les totaux et le bordereau cumulent ce que le nominatif declare : les periodes elementaires d'un mois decoupe.
    let refs: Vec<&Paie> = paies.iter().flat_map(|p| p.elementaires()).collect();
    cumuler_totaux(&mut totaux, &refs, &bar, 1)?;
    let mut origines: Vec<Mois> = Vec::new();
    let mut par_origine: HashMap<Mois, Vec<(CleDeTotalisation, Decimal)>> = HashMap::new();
    for r in regs {
        let bar_o = bar_de(r.mois)?;
        if !origines.contains(&r.mois) {
            origines.push(r.mois);
        }
        let t_o = par_origine.entry(r.mois).or_default();
        cumuler_totaux(t_o, &[&r.apres], &bar_o, 1)?;
        cumuler_le_declare(t_o, r, &bar_o, &orgs, -1)?;
    }
    let mut totaux_avec_regs = totaux.clone();
    for m in &origines {
        for (k, v) in &par_origine[m] {
            ajouter(&mut totaux_avec_regs, *k, *v);
        }
    }

    let echeance_dsn = ctx.echeance_dsn()?;
    let echeances: HashMap<IdentifiantDOrganisme, Jour> = [
        (SIRET_FRANCE_TRAVAIL, echeance_mois_suivant(mois, echeance_dsn)),
        // AUDIENS SANTE PREVOYANCE : le 25 du mois qui suit le mois declare, date qui « doit etre
        // renseignee dans le bloc paiement a destination d'Audiens Sante Prevoyance (P0983) » (Audiens, dossier DSN,
        // `atelier/reference/sources/audiens-dsn/` ; `jour_de_paiement` de `donnees/organismes-base.json`). Elle etait
        // la fin du mois suivant le trimestre, pour un versement pourtant mensuel.
        (
            droit::organismes::AUDIENS,
            echeance_mois_suivant(
                mois,
                crate::ordres_paiement::jour_de_paiement(&droit::organismes::AUDIENS)?.ok_or_else(|| socle::echec::livree("organismes-base.json", "P0983.jour_de_paiement"))?,
            ),
        ),
    ]
    .into_iter()
    .collect();
    let urssaf = orgs.urssaf()?;
    let lignes_23 = lignes_bordereau(&refs, &orgs, ent0, &bar, solde_ta.as_ref(), doeth.as_ref(), csa.as_ref(), mois, &Seuils::du_mois(ctx, mois)?)?;
    let mut lignes_o = Vec::new();
    let mut montants_o = Vec::new();
    for &m in &origines {
        // Le bareme du mois d'origine pour CET etablissement, comme celui du bordereau courant (`bar`) : point 25.
        let bar_o = bar_de(m)?;
        let l = lignes_regularisees(&orgs, ent0, regs, m, &bar_o, &Seuils::du_mois(ctx, m)?)?;
        montants_o.push((m, bordereau::montant(&l, &m.premier_jour())?));
        lignes_o.push((m, l));
    }
    // LE TROP-VERSE A L'URSSAF (les bordereaux negatifs des mois corriges) se reprend sur le versement du mois, dans la
    // limite de son bordereau ; le reste demeure a l'Urssaf (`bordereau::reprise_urssaf`, guide Urssaf § 6.4).
    let credit = -montants_o.iter().filter(|(_, m)| *m < Decimal::ZERO).fold(zero2(), |s, (_, m)| add(s, *m));
    let mt_urssaf = bordereau::montant(&lignes_23, &debut_mois)?;
    let (verse_urssaf, deduit_urssaf) = bordereau::reprise_urssaf(mt_urssaf, credit);
    let banque = banque_du_prelevement(&ent)?;
    // Le versement DGFiP s'ecrit ici, en tete des blocs 20, quand les individus ont dit ce qu'ils retiennent.
    let place_dgfip = f.lignes.len();
    // Les ordres de paiement, dans l'ordre du fichier ; ceux des organismes complementaires a leur place, ecrits apres
    // les individus.
    let mut ordres: Vec<Option<Ordre>> = Vec::new();
    let mut places_ordres_oc: Vec<(IdentifiantDOrganisme, usize)> = Vec::new();
    // Un organisme complementaire : celui d'une adhesion prevoyance (S21.G00.15.002 = S21.G00.20.001).
    let organismes_complementaires: HashSet<IdentifiantDOrganisme> =
        orgs.adhesions_prevoyance.iter().map(|a| a.delegataire.clone()).collect();
    let mut places_oc: Vec<(usize, &Versement)> = Vec::new();
    for v in versements {
        if sans_individu && !v.est_urssaf() {
            continue;
        }
        let siret = v.siret.clone();
        if organismes_complementaires.contains(&siret) {
            // Le versement d'un organisme complementaire s'ecrit ici, ses composants lus dans les individus ecrits.
            places_oc.push((f.lignes.len(), v));
            places_ordres_oc.push((siret.clone(), ordres.len()));
            ordres.push(None);
            continue;
        }
        let (montant, destinataire, reste, deduit) = if v.est_urssaf() {
            (verse_urssaf.montant, Destinataire::Urssaf, sub(credit.max(zero2()), deduit_urssaf), deduit_urssaf)
        } else {
            let m = montant_versement(v, &totaux_avec_regs);
            if m.is_zero() {
                continue;
            }
            // Une correction a la baisse peut faire la somme negative : le versement est alors a zero, le credit reste
            // declare dans les cotisations individuelles (`bordereau::a_verser`).
            let a = bordereau::a_verser(m);
            (a.montant, Destinataire::Autre, a.credit, zero2())
        };
        ordres.push(Some(Ordre {
            organisme: siret.clone(),
            reference: reference_versement(v, &ent),
            montant,
            debut: debut_mois,
            fin: fin_m,
            date_paiement: echeances.get(&siret).copied(),
            destinataire,
            reste,
            deduit,
        }));
        f.add("S21.G00.20.001", &siret);
        if let Some(r) = reference_versement(v, &ent) {
            f.add("S21.G00.20.002", r);
        }
        f.add_case("S21.G00.20.003", &banque.bic, "bic")?;
        f.add_case("S21.G00.20.004", &banque.iban, "iban")?;
        f.add("S21.G00.20.005", mt(montant));
        f.add("S21.G00.20.006", debut_mois.jjmmaaaa());
        f.add("S21.G00.20.007", fin_m.jjmmaaaa());
        f.add("S21.G00.20.010", "05");
        if let Some(j) = echeances.get(&siret) {
            f.add("S21.G00.20.011", j.jjmmaaaa());
        }
    }
    let siret_urssaf = urssaf.siret.clone();
    for (mois_o, mt_o) in &montants_o {
        if *mt_o < Decimal::ZERO {
            continue;
        }
        let d_o = mois_o.premier_jour();
        ordres.push(Some(Ordre {
            organisme: siret_urssaf.clone(),
            reference: None,
            montant: *mt_o,
            debut: d_o,
            fin: d_o.mois_civil().dernier_jour(),
            date_paiement: echeances.get(&siret_urssaf).copied(),
            destinataire: Destinataire::Urssaf,
            reste: zero2(),
            deduit: zero2(),
        }));
        f.add("S21.G00.20.001", &siret_urssaf);
        f.add_case("S21.G00.20.003", &banque.bic, "bic")?;
        f.add_case("S21.G00.20.004", &banque.iban, "iban")?;
        f.add("S21.G00.20.005", mt(*mt_o));
        f.add("S21.G00.20.006", d_o.jjmmaaaa());
        f.add("S21.G00.20.007", d_o.mois_civil().dernier_jour().jjmmaaaa());
        f.add("S21.G00.20.010", "05");
        if let Some(j) = echeances.get(&siret_urssaf) {
            f.add("S21.G00.20.011", j.jjmmaaaa());
        }
    }

    f.add("S21.G00.22.001", &siret_urssaf);
    f.add("S21.G00.22.003", debut_mois.jjmmaaaa());
    f.add("S21.G00.22.004", fin_m.jjmmaaaa());
    f.add("S21.G00.22.005", mt(mt_urssaf));
    ecrire_lignes_23(&mut f, &lignes_23)?;
    let mut bordereaux = vec![BordereauEcrit { debut: debut_mois, fin: fin_m, total: mt_urssaf, lignes: lignes_23.clone() }];
    for ((mois_o, lignes), (_, montant)) in lignes_o.iter().zip(&montants_o) {
        let d_o = mois_o.premier_jour();
        f.add("S21.G00.22.001", &siret_urssaf);
        f.add("S21.G00.22.003", d_o.jjmmaaaa());
        f.add("S21.G00.22.004", d_o.mois_civil().dernier_jour().jjmmaaaa());
        f.add("S21.G00.22.005", mt(*montant));
        ecrire_lignes_23(&mut f, lignes)?;
        bordereaux.push(BordereauEcrit { debut: d_o, fin: d_o.mois_civil().dernier_jour(), total: *montant, lignes: lignes.clone() });
    }
    // L'assujettissement fiscal : une fois l'an, dans la DSN de decembre, apres les bordereaux et avant les
    // individus (arborescence de la norme, p. 118, `chapitre-8-arborescences.txt` : 20, 55, 22, 23, 44, 30) ;
    // `assujettissement.rs` dit d'ou vient chaque code.
    for b in crate::assujettissement::blocs(ctx, ent0, &ent_e, &orgs, &bar, &nic, mois, paies, departs)? {
        f.add("S21.G00.44.001", b.code);
        if let Some(m) = b.montant {
            f.add("S21.G00.44.002", mt(m));
        }
        f.add("S21.G00.44.003", mois.annee());
        if let Some(m) = &b.motif {
            f.add("S21.G00.44.004", m);
        }
    }

    let mut ordre = ordre_salaries(paies);
    for r in regs {
        let c = cle_salarie(&r.apres);
        if !ordre.contains(&c) {
            ordre.push(c);
        }
    }
    // LES CONTRATS DE CHAQUE PERIODE DE PAIE, pour les emplois multiples (`40.036`) : ceux de la declaration et ceux de
    // ses engagements valides (`contrats_du_mois`) ; pour un contrat regularise, ceux de son mois d'origine.
    let du_mois = contrats_du_mois(ctx, mois, departs)?;
    let periode: Vec<&Paie> = paies.iter().chain(du_mois.iter()).collect();
    let mut des_mois_regularises: HashMap<Mois, Vec<Paie>> = HashMap::new();
    for r in regs {
        if !des_mois_regularises.contains_key(&r.mois) {
            des_mois_regularises.insert(r.mois, contrats_du_mois(ctx, r.mois, departs)?);
        }
    }
    let mut declarations = crate::changements_individu::Declarations::avant(ctx, mois, departs)?;
    for cle in &ordre {
        let lot: Vec<&Paie> = paies.iter().filter(|p| &cle_salarie(p) == cle).collect();
        let corrections: Vec<&Regularisation> = regs.iter().filter(|r| &cle_salarie(&r.apres) == cle).collect();
        let fiche = match lot.first() {
            Some(p) => &p.salarie,
            None => &corrections[0].apres.salarie,
        };
        // LES IDENTIFIANTS DU MOIS DECLARE : NIR, nom, prenoms et date de naissance tels qu'au dernier jour du
        // mois, pour qu'un changement date apres lui n'entre pas dans sa DSN (`changements_individu::identite_du_mois`).
        let brute = ctx.salarie_brut(&fiche.id);
        let ident = crate::changements_individu::identite_du_mois(fiche, brute, &fin_m)?;
        bloc_individu(&mut f, &ident, true)?;
        // L'identite DECLAREE, relue dans la derniere paie declaree qui le porte : son NTT, s'il etait declare sans NIR
        // (`ntt::poser_ntt`).
        let declaree = declarations.derniere(&ident, brute)?;
        crate::ntt::poser_ntt(&mut f, &ent_e, &ident, declaree.as_ref())?;
        // LE BLOC 31 : ce qui differe de l'identite declaree, a l'ancienne valeur, date par l'historique de la fiche ; un
        // changement que la fiche ne date pas refuse.
        crate::changements_individu::ecrire(&mut f, &ident, brute, declaree.as_ref(), mois, &fin_m)?;
        let taux_pas = pas_par_salarie.get(cle).cloned().flatten();

        for (i, p) in lot.iter().enumerate() {
            let siens: Vec<&Arret> = arrets_du_contrat(&arrets_du_mois, p)?;
            let debut_bloc = f.lignes.len();
            bloc_contrat(&mut f, p, &ent_e, &orgs, i as i64 + 1, &bar, true, &siens, NatureDeLaDeclaration::Mensuelle, emplois_multiples(p, &periode))?;
            // Ce qui a change depuis la derniere DSN deposee du contrat : le bloc S21.G00.41, sous son bloc 40.
            let declare = bloc_40_declare(ctx, p, mois, &etats_du_mois, departs, &orgs)?;
            crate::changements_contrat::poser(&mut f, debut_bloc, ctx, p, mois, &nics_courant_d_abord, declare.as_ref())?;
        }
        let mut rangs_regs = Vec::new();
        let mut ajoutes: Vec<&Regularisation> = Vec::new();
        for r in &corrections {
            let deja = lot.iter().position(|p| p.engagement.numero == r.apres.engagement.numero).map(|i| i as i64 + 1);
            // UN CONTRAT SUR PLUSIEURS MOIS corrige pour plusieurs mois : un seul bloc 40 sous l'individu,
            // pose par la premiere de ses corrections ; chaque mois garde son versement, rattache a ce mois (cahier
            // technique DSN 2026.1, §2.2.2.5 : « valorisees mois civil par mois civil »).
            let meme_contrat = ajoutes.iter().position(|a| a.apres.engagement.numero == r.apres.engagement.numero);
            let rang = match (deja, meme_contrat) {
                (Some(rg), _) => rg,
                (None, Some(k)) => (lot.len() + k + 1) as i64,
                (None, None) => {
                    let rg = (lot.len() + ajoutes.len() + 1) as i64;
                    ajoutes.push(r);
                    // Le bloc 40 est celui du contrat tel qu'il est corrige, lu dans la correction de son dernier mois :
                    // c'est elle qui sait s'il finit (bloc 62).
                    let groupe: Vec<&Regularisation> =
                        corrections.iter().copied().filter(|x| x.apres.engagement.numero == r.apres.engagement.numero).collect();
                    let dernier = groupe.iter().copied().fold(*r, |a, x| if x.mois > a.mois { x } else { a });
                    let b = bar_de(dernier.mois)?;
                    let autres: Vec<&Paie> =
                        regs.iter().filter(|x| x.mois == dernier.mois).map(|x| &x.apres).chain(des_mois_regularises[&dernier.mois].iter()).collect();
                    // UNE FIN CORRIGEE : « une correction de fin de contrat s'opere par l'emission d'un bloc
                    // " Fin du contrat - S21.G00.62 " dans la DSN mensuelle suivante portant les informations correctes
                    // qui ecraseront celles precedemment declarees » (fiche net-entreprises 1722,
                    // `atelier/reference/sources/net-entreprises-base-connaissances/`). UN CONTRAT AJOUTE apres la limite
                    // se declare en entier : sa fin aussi, comme celle de tout contrat termine (bloc 62, « Evenement de fin du
                    // contrat de travail », `norme-dsn/texte/bloc-S21.G00.62.txt`). `bloc_contrat` ne l'ecrit que si le contrat finit dans le mois
                    // de la correction qui le pose.
                    let ajoute = |x: &Regularisation| x.declare.lignes.is_empty() && x.declare.fichier.is_empty();
                    let fin = groupe.iter().any(|x| ajoute(x) || x.apres.engagement.fin != x.avant.engagement.fin);
                    let debut_bloc = f.lignes.len();
                    let siens: Vec<&Arret> = arrets_du_contrat(&arrets_du_mois, &dernier.apres)?;
                    bloc_contrat(&mut f, &dernier.apres, &ent_e, &orgs, rg, &b, fin, &siens, NatureDeLaDeclaration::Mensuelle, emplois_multiples(&dernier.apres, &autres))?;
                    // UNE DATE DE DEBUT CORRIGEE : l'ancienne, lue dans ce qui a ete declare, sous le bloc 40 (fiche 1952) --
                    // une fois, depuis la correction du dernier mois qui a declare le contrat.
                    let declarant = groupe.iter().copied().filter(|x| !ajoute(x)).fold(None, |a: Option<&Regularisation>, x| match a {
                        Some(a) if a.mois >= x.mois => Some(a),
                        _ => Some(x),
                    });
                    if let Some(d) = declarant {
                        crate::changements_contrat::ancienne_date_de_debut(&mut f.lignes, debut_bloc, d, &dernier.apres.engagement.debut, fin_m)?;
                    }
                    rg
                }
            };
            rangs_regs.push(rang);
        }
        // Un seul bloc 40 sous l'individu (les contrats du mois, plus ceux qu'une regularisation ajoute) : 78.006 ne
        // s'ecrit pas sur ses bases (p. 298). Les lignes d'annulation recopient le depose tel quel.
        f.contrat_unique = lot.len() + ajoutes.len() == 1;
        for (i, p) in lot.iter().enumerate() {
            bloc_versement(&mut f, p, &bar, i as i64 + 1, taux_pas.as_ref())?;
        }
        // POINT 18 : une regularisation d'un contrat aussi paye ce mois garde le rang de ce contrat (ses
        // affiliations), mais pas son numero de versement -- deux blocs 50 au meme numero sous un individu, la CCH-11 du
        // bloc 50 l'interdit (p. 243). Les versements du mois portent 01 a n, ses regularisations n+1 et suivants.
        for (k, (rang, r)) in rangs_regs.iter().zip(&corrections).enumerate() {
            let b = bar_de(r.mois)?;
            let numero = (lot.len() + k + 1) as i64;
            let pas_c = engagement::etats::prelevement_de_la_correction(ctx, &r.apres, r.declare.net_imposable, r.taux_pas_origine.as_deref(), mois)?;
            bloc_regularisation(&mut f, r, &b, *rang, numero, &pas_c, fin_m)?;
        }
        f.contrat_unique = false;
        let anciennete = anciennete_du_mois(fiche, cle, &fin_m)?;
        for p in &lot {
            bloc_anciennete(&mut f, p, anciennete)?;
        }
        for r in &ajoutes {
            bloc_anciennete(&mut f, &r.apres, anciennete)?;
        }
    }

    // Les versements aux organismes complementaires, leurs composants lus dans le nominatif ecrit.
    let composants = composants_de_versement(&f.lignes)?;
    let somme_composants = composants.iter().fold(zero2(), |s, c| add(s, c.montant));
    let somme_059 = match lire(&totaux_avec_regs, CodeCotisation81::PrevoyanceAssuranceMutuelle.into()) {
        Some(s) => s,
        None => zero2(),
    };
    if somme_composants != somme_059 {
        return Err(refus!(
            "Les cotisations 059 du mois ({}, régularisations comprises) et les composants de versement que le \
nominatif rattache à une adhésion ({}) ne font pas la même somme : une base de prévoyance n'est pas rattachée à son \
affiliation (S21.G00.78.005). Rien n'est écrit.",
            socle::texte::euros(somme_059),
            socle::texte::euros(somme_composants)
        ));
    }
    if let Some(c) = composants.iter().find(|c| !places_oc.iter().any(|(_, v)| v.siret == c.organisme)) {
        if !sans_individu {
            return Err(refus!(
                "Des cotisations sont déclarées sous l'adhésion {} de l'organisme {} (S21.G00.15.002), et le logiciel ne \
sait pas payer cet organisme : la table livrée des versements n'en a aucun à {}, et le \
bloc S21.G00.20 qui les paie ne se devine pas. Si ce code d'organisme a été saisi dans Configuration › Organismes et \
contrats, le vérifier sur le certificat d'adhésion. Rien n'est écrit.",
                c.code, c.organisme, c.organisme
            ));
        }
    }
    for (place, v) in places_oc.iter().rev() {
        let siret = v.siret.clone();
        let siens: Vec<&Composant> = composants.iter().filter(|c| c.organisme == siret).collect();
        let bloc = versement_complementaire(v, &ent, banque, &siens, debut_mois, fin_m, echeances.get(&siret).copied())?;
        if !bloc.is_empty() {
            let a = bordereau::a_verser(siens.iter().fold(zero2(), |s, c| add(s, c.montant)));
            if let Some((_, i)) = places_ordres_oc.iter().find(|(s, _)| *s == siret) {
                ordres[*i] = Some(Ordre {
                    organisme: siret.clone(),
                    reference: reference_versement(v, &ent),
                    montant: a.montant,
                    debut: debut_mois,
                    fin: fin_m,
                    date_paiement: echeances.get(&siret).copied(),
                    destinataire: Destinataire::Complementaire,
                    reste: a.credit,
                    deduit: zero2(),
                });
            }
        }
        f.lignes.splice(*place..*place, bloc);
    }

    let du = pas_retenu(&f.lignes)?;
    let remplace = type_declaration.remplace();
    let dgfip = versement_dgfip(du, mois, debut_mois, fin_m, banque, remplace, o.remplacee_versait_pas)?;
    if !dgfip.is_empty() {
        let mut arrondi = bordereau::euro(du);
        arrondi.rescale(2);
        let montant = bordereau::a_verser(arrondi).montant;
        let reste = if montant.is_zero() { (-du).max(zero2()) } else { zero2() };
        ordres.insert(0, Some(Ordre { organisme: DGFIP, reference: Some(DGFIP_PAS.to_string()), montant, debut: debut_mois, fin: fin_m, date_paiement: None, destinataire: Destinataire::Dgfip, reste, deduit: zero2() }));
    }
    f.lignes.splice(place_dgfip..place_dgfip, dgfip);

    let n = f.lignes.len() + 2;
    f.add("S90.G00.90.001", n);
    f.add("S90.G00.90.002", "1");
    let texte = f.texte();
    verifier_affiliations(&texte)?;
    crate::ntt::verifier_fichier(&texte)?;
    crate::usages::verifier_fichier(&texte)?;
    crate::formats::verifier_fichier(&texte)?;
    Ok(Fabriquee { texte, ordres: ordres.into_iter().flatten().collect(), bordereaux })
}

/// Le texte en latin-1, comme `dsn.ecrire` l'ecrit.
pub fn en_latin1(texte: &str) -> R<Vec<u8>> {
    texte
        .chars()
        .map(|c| if (c as u32) < 256 { Ok(c as u8) } else { Err(socle::echec::a_la_cle("Un texte de la DSN porte un caractère que la norme n'admet pas : le retirer de la fiche qui le porte.", "caractere", c)) })
        .collect()
}

pub fn ecrire(texte: &str, chemin: &Path) -> R<()> {
    if let Some(d) = chemin.parent() {
        std::fs::create_dir_all(d).map_err(|e| socle::echec::du_disque(&d, &e))?;
    }
    std::fs::write(chemin, en_latin1(texte)?).map_err(|e| socle::echec::du_disque(&chemin, &e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Un dossier de compagnie temporaire : fiches vides, `config/casc-svp.json` des mois regularises.
    fn ctx_dsn(nom: &str, regularises: &[Mois]) -> Contexte {
        let racine: PathBuf = std::env::temp_dir().join(format!("puppetpaie-dsn-{}-{}", std::process::id(), nom));
        let _ = std::fs::remove_dir_all(&racine);
        let cfg = racine.join(socle::fichiers::CONFIG);
        std::fs::create_dir_all(&cfg).unwrap();
        for f in ["entreprise.json", "salaries.json", "spectacles.json"] {
            std::fs::write(cfg.join(f), "{}").unwrap();
        }
        let notee = crate::casc::CascSvp {
            quoi: "Les annees de CASC-SVP regularisees hors DSN.".into(),
            regularisations: vec![crate::casc::RegularisationNotee {
                annee: socle::mois::Annee::nouvelle(2024).unwrap(),
                mois: regularises.to_vec(),
                montant: rust_decimal::Decimal::new(0, 2),
                calcule: rust_decimal::Decimal::new(0, 2),
                vire_le: Jour::nouveau(2025, 1, 15).unwrap(),
                note_le: Jour::nouveau(2025, 1, 16).unwrap(),
                qui: "test".into(),
            }],
        };
        socle::ecriture::ecrire_json(&cfg.join("casc-svp.json"), &notee, 0).unwrap();
        Contexte::nouveau(&racine, Some(Jour::nouveau(2026, 9, 27).unwrap())).unwrap()
    }

    fn orgs_casc() -> Organismes {
        use droit::organismes::{Adhesion, ParNature, Rattachement};
        let adh = |libelle: &str, reference: &str| Adhesion {
            cle: libelle.to_lowercase(),
            libelle: libelle.into(),
            delegataire: droit::organismes::AUDIENS,
            r#type: "01".into(),
            reference: reference.into(),
            contribution: None,
            depuis: None,
            jusqu_a: None,
        };
        let refs = |r: &[&str]| Rattachement { references: r.iter().map(|x| x.to_string()).collect(), a_confirmer: None };
        Organismes {
            base: droit::organismes::base().unwrap(),
            adhesions_prevoyance: vec![adh("CASCSVP", "100"), adh("INTERBRANCHE", "102")],
            par_nature: ParNature { intermittent: refs(&["103", "102"]), permanent: refs(&["100", "101"]) },
        }
    }

    // La CASC-SVP de la 3090 (`atelier/reference/sources/casc-svp-fiche-codification-dsn-202606.txt`).
    #[test]
    fn la_dsn_de_la_3090_porte_la_casc_en_troisieme_affiliation() {
        let c = casc_svp(&ctx_dsn("a", &[]), &ChampsEntreprise { idcc: Some(droit::fiche::Idcc::lire("3090").unwrap()), ..Default::default() }, &orgs_casc(), Mois::nouveau(2026, 9).unwrap()).unwrap().unwrap();
        assert_eq!(c.reference, "100");
        assert_eq!(c.lignes, [droit::forme_bareme::CCN]);
        // l'intermittent : apres 103 et 102, donc 204 ; le permanent l'a deja.
        assert_eq!(c.natures, std::collections::BTreeMap::from([("intermittent".to_string(), "204".to_string())]));
    }

    // `atelier/reference/sources/casc-svp-fiche-codification-dsn-202606.txt`.
    #[test]
    fn pas_de_casc_hors_3090_avant_2025_ni_sur_un_mois_regularise() {
        let o = orgs_casc();
        let m = |an, numero| Mois::nouveau(an, numero).unwrap();
        assert!(casc_svp(&ctx_dsn("b1", &[]), &ChampsEntreprise { idcc: Some(droit::fiche::Idcc::lire("1285").unwrap()), ..Default::default() }, &o, m(2026, 9)).unwrap().is_none());
        assert!(casc_svp(&ctx_dsn("b2", &[]), &ChampsEntreprise { idcc: Some(droit::fiche::Idcc::lire("3090").unwrap()), ..Default::default() }, &o, m(2024, 12)).unwrap().is_none());
        assert!(casc_svp(&ctx_dsn("b3", &[m(2026, 8)]), &ChampsEntreprise { idcc: Some(droit::fiche::Idcc::lire("3090").unwrap()), ..Default::default() }, &o, m(2026, 8)).unwrap().is_none());
    }

    // S21.G00.20.002 de France Travail (`atelier/reference/sources/francetravail-100-dsn/net-entreprises-fiche-3344-intermittents-20251113.html`).
    #[test]
    fn le_dossier_france_travail_de_la_fiche_part_dans_la_dsn() {
        let base = droit::organismes::base().unwrap();
        let fiche = ChampsEntreprise { france_travail_dossier_recouvrement: Some("CS00000001".into()), ..Default::default() };
        let ft = base.versement(&SIRET_FRANCE_TRAVAIL).unwrap();
        assert_eq!(reference_versement(ft, &fiche).as_deref(), Some("CS00000001"));
        assert_eq!(reference_versement(ft, &ChampsEntreprise::default()), None);
        let urssaf = IdentifiantDOrganisme::lire("78861779300013").unwrap();
        assert_eq!(reference_versement(base.versement(&urssaf).unwrap(), &fiche), None);
        assert_eq!(SIRET_FRANCE_TRAVAIL.as_str(), "13000548100010");
    }

    // Le tableau des usages (`atelier/reference/norme-dsn/dsn-cahier-technique-2026.1-usages.txt`).
    #[test]
    fn statut_boeth_admis_en_mensuelle() {
        assert!(usage_admet(&Fichier::en_norme(crate::norme::Norme::connue("P26V01").unwrap()), "S21.G00.40.072", NatureDeLaDeclaration::Mensuelle).unwrap());
    }

    // `atelier/reference/norme-dsn/texte/bloc-S21.G00.50.txt`, `bloc-S21.G00.56.txt` (la somme est une inference, `pas_retenu`).
    #[test]
    fn le_pas_retenu_compte_les_50_009_et_les_regularisations_d_indu() {
        let l: Vec<String> = ["S21.G00.50.009,'187.18'", "S21.G00.50.013,'2000.00'", "S21.G00.50.009,'12.40'", "S21.G00.56.007,'-30.08'"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(pas_retenu(&l).unwrap(), Decimal::from_str_exact("169.50").unwrap());
    }

    // `atelier/reference/sources/net-entreprises-base-connaissances/net-entreprises-fiche1931-situations-creditrices-versement-20250323.txt`.
    #[test]
    fn le_versement_dgfip_s_arrondit_a_l_euro_et_verse_zero_pour_un_pas_negatif() {
        let banque: droit::fiche::Banque = socle::forme::lire_texte(r#"{"bic": "BANQFRPPXXX", "iban": "FR7630001007941234567890185"}"#, "banque").unwrap();
        let (debut, fin) = (Jour::nouveau(2026, 7, 1).unwrap(), Jour::nouveau(2026, 7, 31).unwrap());
        let montant = |du: &str| -> Option<String> {
            let l = versement_dgfip(Decimal::from_str_exact(du).unwrap(), debut.mois_civil(), debut, fin, &banque, false, None).unwrap();
            l.iter().find_map(|x| x.strip_prefix("S21.G00.20.005,").map(str::to_string))
        };
        assert_eq!(montant("187.18"), Some("'187.00'".into()));
        assert_eq!(montant("187.50"), Some("'188.00'".into()));
        assert_eq!(montant("169.49"), Some("'169.00'".into()));
        // Moins de 50 centimes : rien a reverser, rien a ecrire sur une DSN normale.
        assert_eq!(montant("0.40"), None);
        // Un PAS negatif : un versement a 0.00, que la fiche net-entreprises 1931 admet « par commodite » (l. 13).
        assert_eq!(montant("-3.10"), Some("'0.00'".into()));
    }

    // ------------------------------------------------------------ les composants du versement (bloc 55)

    fn l(k: &str, v: &str) -> String {
        format!("{},'{}'", k, v)
    }

    /// Un individu, une adhesion, une base 31 rattachee : le minimum que `composants_de_versement` lit.
    fn fichier_55(debut: &str, fin: &str, aff: &str, avec_15: bool) -> Vec<String> {
        let mut f = Vec::new();
        if avec_15 {
            f.extend([l("S21.G00.15.001", "CONTRAT A"), l("S21.G00.15.002", "P0983"), l("S21.G00.15.004", "01"), l("S21.G00.15.005", "100")]);
        }
        f.extend([
            l("S21.G00.30.001", "1"),
            l("S21.G00.30.002", "MARTIN"),
            l("S21.G00.40.009", "MA1.26.01"),
            l("S21.G00.70.012", "202"),
            l("S21.G00.70.013", "100"),
            l("S21.G00.78.001", "31"),
            l("S21.G00.78.002", debut),
            l("S21.G00.78.003", fin),
            l("S21.G00.78.004", "0.00"),
            l("S21.G00.78.005", aff),
            l("S21.G00.81.001", "059"),
            l("S21.G00.81.004", "12.50"),
            l("S21.G00.78.001", "31"),
            l("S21.G00.78.002", debut),
            l("S21.G00.78.003", fin),
            l("S21.G00.78.005", aff),
            l("S21.G00.81.001", "059"),
            l("S21.G00.81.004", "-2.50"),
        ]);
        f
    }

    /// 55.004 : « une periode civile complete [...] contenant la ou les periodes cotisees » (cahier technique 2026.1,
    /// `bloc-S21.G00.55.txt`, p. 260, l. 85-87).
    #[test]
    fn la_periode_d_affectation_contient_les_periodes_cotisees() {
        let c = composants_de_versement(&fichier_55("01042026", "30042026", "202", true)).unwrap();
        assert_eq!(c.len(), 1, "deux bases du meme contrat et de la meme periode : un composant");
        assert_eq!((c[0].code.as_str(), c[0].organisme.as_str(), c[0].periode.as_str(), c[0].montant), ("CONTRAT A", "P0983", "2026T02", "10.00".parse::<Decimal>().unwrap()));
        assert_eq!(composants_de_versement(&fichier_55("15032026", "10042026", "202", true)).unwrap()[0].periode, "2026S01");
        assert_eq!(composants_de_versement(&fichier_55("15062026", "10072026", "202", true)).unwrap()[0].periode, "2026A00");
        let e = composants_de_versement(&fichier_55("15122025", "10012026", "202", true)).unwrap_err().phrase;
        assert!(e.contains("deux annees civiles"), "{}", e);
    }

    /// 55.003 : « Le contrat mentionne doit faire l'objet d'un bloc Adhesion Prevoyance S21.G00.15 », CCH-11 (p. 260,
    /// l. 72-79).
    #[test]
    fn un_code_d_affectation_qui_ne_se_lit_nulle_part_refuse() {
        let e = composants_de_versement(&fichier_55("01042026", "30042026", "203", true)).unwrap_err().phrase;
        assert!(e.contains("MARTIN") && e.contains("203") && e.contains("ne se lit nulle part"), "{}", e);
        let e = composants_de_versement(&fichier_55("01042026", "30042026", "202", false)).unwrap_err().phrase;
        assert!(e.contains("100") && e.contains("S21.G00.15"), "{}", e);
    }

    // ------------------------------------------------------------ l'arret de travail

    fn j(an: i32, mois: u32, jour: u32) -> Jour {
        Jour::nouveau(an, mois, jour).unwrap()
    }

    fn arret(reprise: Option<Jour>, fin_previsionnelle: Jour) -> Arret {
        Arret {
            saisi: engagement::imprevus::ArretSaisi {
                genre: engagement::imprevus::GenreArret::Arret,
                salarie: IdSalarie::lire("Audinot").unwrap(),
                contrat_debut: j(2025, 9, 1),
                contrat_numero: Some(engagement::numeros::NumeroContrat::lire("AU.25.01").unwrap()),
                motif: droit::arrets::MotifDArret::Maladie,
                dernier_jour_travaille: j(2026, 6, 20),
                debut: j(2026, 6, 21),
                fin_previsionnelle,
                subrogation: Some(droit::arrets::SubrogationDeLArret::Non),
                subrogation_debut: None,
                subrogation_fin: None,
                reprise,
                reprise_motif: reprise.map(|_| droit::arrets::MotifDeReprise::Normale),
                date_accident: None,
                jours_deja_indemnises: None,
                ij_brute: None,
                ij_nette: None,
                ij_brute_majoree: None,
                ij_nette_majoree: None,
                prolongations: Vec::new(),
                reprises: Vec::new(),
                saisi_le: socle::paris::Moment::a_paris(j(2026, 6, 21), 10, 0, 0).unwrap(),
                qui: "interface".into(),
                note: None,
            },
        }
    }

    /// La suspension se declare dans les DSN dont le mois principal a une periode commune avec elle (bloc S21.G00.60,
    /// cahier technique DSN 2026.1, p. 265) ; la reprise est le « premier jour travaille et paye suite a l'arret de
    /// travail » (S21.G00.60.010, p. 269) : le dernier jour d'arret est la veille.
    #[test]
    fn un_arret_repris_le_premier_du_mois_ne_couvre_pas_ce_mois() {
        let (juin, juillet) = ((j(2026, 6, 1), j(2026, 6, 30)), (j(2026, 7, 1), j(2026, 7, 31)));
        let repris_le_1er = arret(Some(j(2026, 7, 1)), j(2026, 7, 3));
        assert!(repris_le_1er.couvre(&juin.0, &juin.1).unwrap());
        assert!(!repris_le_1er.couvre(&juillet.0, &juillet.1).unwrap(), "aucun jour d'arret en juillet");
        assert!(arret(Some(j(2026, 7, 2)), j(2026, 7, 3)).couvre(&juillet.0, &juillet.1).unwrap(), "le 1er juillet est arrete");
        assert!(arret(None, j(2026, 7, 1)).couvre(&juillet.0, &juillet.1).unwrap(), "la fin previsionnelle est un jour d'arret");
        assert!(!arret(None, j(2026, 6, 30)).couvre(&juillet.0, &juillet.1).unwrap());
    }
}
