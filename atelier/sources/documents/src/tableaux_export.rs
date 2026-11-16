//! La machinerie commune des etats exportes : un tableau, six facons de l'ecrire --
//! CSV, texte tabule, XLSX, PDF, HTML, JSON --, et les ecrivains du livre de paie, qui a les siens.
//!
//! UN ETAT DECLARE, LA MACHINERIE ECRIT. Un etat rend un `Tableau` : ses colonnes (et lesquelles portent un
//! montant), ses sections (un groupe de lignes et son sous-total), un total general, ses avertissements et les
//! sources qui le fondent. Chaque ligne est un dictionnaire ordonne cle -> valeur : une ligne qui oublie une
//! colonne laisse une case vide. Une cle PRESENTE a `None` (la source ne dit pas ce montant) se lit « — » au PDF
//! et a la page HTML ; une cle ABSENTE reste vide.
//!
//! LES TOTAUX se calculent ici, avec la regle du livre de paie : un montant `None` ne compte pas pour zero, et
//! une colonne qu'aucune ligne ne declare a un total VIDE, pas 0,00.
//!
//! Le `.xlsx` s'ecrit par `classeur` : montants en nombres exacts, formats `# ##0.00 €`, largeurs 14 / 26,
//! volet fige sous l'en-tete. Il se prouve par son contenu, pas a l'octet. Le PDF est celui de la
//! fabrique (`fabrique_pdf`), decrit comme `tableaux.description_pdf`.

use droit::assiette::Assiette;
use droit::fiche::Lue as _;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use fabrique_pdf::forme as pdf;
use fabrique_pdf::mesure::mesure;
use crate::classeur::{Feuille, Style, Vertical, CORPS_DU_CLASSEUR};
use serde::{Serialize, Serializer};

use droit::contexte::Contexte;
use socle::echec::{erreur, Classe};
use socle::grandeurs::{Cachets, Coefficient, Heures, Jours, Montant, Taux};
use engagement::ligne::{QuantiteDeLigne, TauxDeLigne};
use socle::echec::Echec;
use socle::mois::{Mois, MoisDeLAnnee};
pub use socle::echec::R;
use crate::plan_comptable::{EtatDExport, FormatDExport};

/// Un refus `Refuse`.
pub fn refus(m: impl Into<String>) -> Echec {
    erreur(Classe::Refuse, m)
}

/// `FORMATS_TABLEAU` : les six formats de la machinerie, dans l'ordre du format.
pub const FORMATS_TABLEAU: [FormatDExport; 6] = [FormatDExport::Csv, FormatDExport::Tsv, FormatDExport::Xlsx, FormatDExport::Pdf, FormatDExport::Html, FormatDExport::Json];

// ------------------------------------------------------------------ valeurs

/// LE RANG D'UNE LIGNE DANS SON ETAT, dit a partir de un : le numero d'ordre d'une embauche au registre du personnel. Il
/// s'ecrit en nombre entier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RangDansLEtat(usize);

impl RangDansLEtat {
    /// Celui de l'element d'indice `i` d'une suite, compte a partir de zero.
    pub fn de_l_indice(i: usize) -> RangDansLEtat {
        RangDansLEtat(i + 1)
    }
}

impl std::fmt::Display for RangDansLEtat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl Serialize for RangDansLEtat {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}

/// Une valeur de cellule : une grandeur (elle s'ecrit en texte, son echelle gardee : `ecrit::texte`), un compte, un
/// rang, un mois (`AAAA-MM`), un texte.
#[derive(Clone, Debug)]
pub enum Val {
    Montant(Montant),
    Taux(Taux),
    Coefficient(Coefficient),
    Heures(Heures),
    Cachets(Cachets),
    Jours(Jours),
    /// La quantite d'une ligne de paie, dans l'unite de sa rubrique.
    Quantite(QuantiteDeLigne),
    /// Le taux d'une ligne de paie : un montant unitaire, ou la part de sa base.
    TauxDeLigne(TauxDeLigne),
    /// Un compte (« combien de… » : des bulletins, des salaries, des mois).
    Compte(usize),
    Rang(RangDansLEtat),
    Mois(Mois),
    Txt(String),
}

/// Chaque grandeur que la case `$v` (une `&Val`) porte, nommee `$g`, passee a `$e` ; puis les bras des cases qui n'en
/// portent pas (un compte, un rang, un mois, un texte).
macro_rules! chaque_grandeur {
    ($v:expr, $g:ident => $e:expr; $($p:pat => $f:expr),+ $(,)?) => {
        match *$v {
            Val::Montant($g) => $e,
            Val::Taux($g) => $e,
            Val::Coefficient($g) => $e,
            Val::Heures($g) => $e,
            Val::Cachets($g) => $e,
            Val::Jours($g) => $e,
            Val::Quantite(q) => match q {
                QuantiteDeLigne::Heures($g) => $e,
                QuantiteDeLigne::Cachets($g) => $e,
                QuantiteDeLigne::Jours($g) => $e,
                QuantiteDeLigne::Services($g) => $e,
                QuantiteDeLigne::Semaines($g) => $e,
                QuantiteDeLigne::PartDuMois($g) => $e,
                QuantiteDeLigne::Unites($g) => $e,
            },
            Val::TauxDeLigne(t) => match t {
                TauxDeLigne::ParUnite($g) => $e,
                TauxDeLigne::PartDeLaBase($g) => $e,
            },
            $($p => $f,)+
        }
    };
}

impl Val {
    /// La valeur en texte : une grandeur son echelle gardee (`12.50`, `ecrit::texte`), un compte ou un rang en chiffres.
    pub fn en_texte_ecrit(&self) -> String {
        chaque_grandeur!(self, g => socle::grandeurs::ecrit::texte::dit(g);
            Val::Compte(n) => n.to_string(),
            Val::Rang(r) => r.to_string(),
            Val::Mois(m) => m.to_string(),
            Val::Txt(ref s) => s.clone(),
        )
    }

    /// Porte-t-elle une grandeur ?
    fn est_une_grandeur(&self) -> bool {
        chaque_grandeur!(self, _g => true;
            Val::Compte(_) | Val::Rang(_) | Val::Mois(_) | Val::Txt(_) => false,
        )
    }

    /// Cette valeur et `autre`, d'une meme colonne, additionnees : des grandeurs de meme nom qui s'additionnent (des
    /// montants, des heures, des cachets, des jours), ou des comptes. Le reste ne s'additionne pas.
    fn plus(&self, autre: &Val) -> R<Val> {
        Ok(match (self, autre) {
            (Val::Montant(a), Val::Montant(b)) => Val::Montant(*a + *b),
            (Val::Heures(a), Val::Heures(b)) => Val::Heures(*a + *b),
            (Val::Cachets(a), Val::Cachets(b)) => Val::Cachets(*a + *b),
            (Val::Jours(a), Val::Jours(b)) => Val::Jours(*a + *b),
            (Val::Compte(a), Val::Compte(b)) => Val::Compte(a + b),
            (a, b) => return Err(Val::sans_total(&format!("« {} » et « {} »", a.en_texte_ecrit(), b.en_texte_ecrit()))),
        })
    }

    /// La premiere valeur du total d'une colonne : elle-meme, quand elle s'additionne (`plus`).
    fn premiere_du_total(&self) -> R<Val> {
        match self {
            Val::Montant(_) | Val::Heures(_) | Val::Cachets(_) | Val::Jours(_) | Val::Compte(_) => Ok(self.clone()),
            autre => Err(Val::sans_total(&format!("« {} »", autre.en_texte_ecrit()))),
        }
    }

    /// Le refus d'un total d'une colonne dont les valeurs `quoi` ne s'additionnent pas.
    fn sans_total(quoi: &str) -> Echec {
        erreur(Classe::CalculImpossible, format!("{} ne s'additionnent pas dans le total d'une colonne.", quoi))
    }
}

impl Serialize for Val {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        chaque_grandeur!(self, g => socle::grandeurs::ecrit::texte::serialize(&g, s);
            Val::Compte(n) => n.serialize(s),
            Val::Rang(r) => r.serialize(s),
            Val::Mois(m) => m.serialize(s),
            Val::Txt(ref t) => s.serialize_str(t),
        )
    }
}

impl From<Montant> for Val {
    fn from(g: Montant) -> Val {
        Val::Montant(g)
    }
}

impl From<Taux> for Val {
    fn from(g: Taux) -> Val {
        Val::Taux(g)
    }
}

impl From<Coefficient> for Val {
    fn from(g: Coefficient) -> Val {
        Val::Coefficient(g)
    }
}

impl From<Heures> for Val {
    fn from(g: Heures) -> Val {
        Val::Heures(g)
    }
}

impl From<Cachets> for Val {
    fn from(g: Cachets) -> Val {
        Val::Cachets(g)
    }
}

impl From<Jours> for Val {
    fn from(g: Jours) -> Val {
        Val::Jours(g)
    }
}

impl From<QuantiteDeLigne> for Val {
    fn from(q: QuantiteDeLigne) -> Val {
        Val::Quantite(q)
    }
}

impl From<TauxDeLigne> for Val {
    fn from(t: TauxDeLigne) -> Val {
        Val::TauxDeLigne(t)
    }
}

// ------------------------------------------------------------------ les colonnes

/// LES COLONNES DES ETATS, declarees une fois : une ligne par colonne, sa cle ecrite (sous laquelle une ligne la porte
/// et le tableau la nomme), ce qu'elle porte (`Genre`) et son en-tete ; un etat qui la titre autrement le dit
/// (`ColonneDEtat::titree`).
macro_rules! colonnes_des_etats {
    ($( $(#[$m:meta])* $var:ident = $cle:literal, $genre:ident, $entete:literal; )*) => {
        /// LA COLONNE D'UN ETAT (voir la declaration). Deux variantes qui s'ecrivent sous la meme cle ne se posent
        /// jamais dans un meme etat : une ligne n'a qu'une case par cle.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum ColonneDEtat {
            $( $(#[$m])* $var, )*
            /// Le brut d'un mois de l'annee, au recapitulatif annuel : `m01` a `m12`, sous le nom abrege du mois avec son
            /// point (« Janv. »).
            DuMois(MoisDeLAnnee),
            /// Une assiette du calcul, a l'etat des assiettes : `b_` et son nom, sous son nom.
            DeLAssiette(Assiette),
        }

        impl ColonneDEtat {
            /// Sa cle ecrite.
            pub fn cle(self) -> Cow<'static, str> {
                match self {
                    $( ColonneDEtat::$var => Cow::Borrowed($cle), )*
                    ColonneDEtat::DuMois(m) => Cow::Owned(format!("m{:02}", m.numero())),
                    ColonneDEtat::DeLAssiette(a) => Cow::Owned(format!("b_{}", a)),
                }
            }

            /// Ce qu'elle porte.
            pub fn genre(self) -> Genre {
                match self {
                    $( ColonneDEtat::$var => Genre::$genre, )*
                    ColonneDEtat::DuMois(_) | ColonneDEtat::DeLAssiette(_) => Genre::Montant,
                }
            }

            /// Son en-tete.
            pub fn entete(self) -> Cow<'static, str> {
                match self {
                    $( ColonneDEtat::$var => Cow::Borrowed($entete), )*
                    ColonneDEtat::DuMois(m) => Cow::Owned(format!("{}.", m.nom(socle::mois::Graphie::Titre).chars().take(4).collect::<String>())),
                    ColonneDEtat::DeLAssiette(a) => Cow::Borrowed(a.nom()),
                }
            }
        }
    };
}

colonnes_des_etats! {
    APayer = "a_payer", Texte, "À payer";
    Abattement = "abattement", Montant, "Abattement";
    Acquis = "acquis", Nombre, "Acquis N (j)";
    Acte = "acte", Texte, "Acte";
    Actif = "actif", Texte, "Fiche";
    Adresse = "adresse", Texte, "Adresse";
    Analytique = "analytique", Texte, "Analytique";
    Annee = "annee", Texte, "Année";
    /// L'assiette du prelevement a la source d'une paie.
    AssiettePas = "assiette", Montant, "Assiette PAS";
    /// L'assiette qu'une ligne du bordereau Urssaf declare : la meme cle qu'`AssiettePas`, a droite sans total.
    AssietteDeclaree = "assiette", Taux, "Assiette déclarée";
    Au = "au", Texte, "au";
    /// La base ou la quantite d'une ligne de bulletin, d'une cotisation : a droite, sans total.
    Base = "base", Taux, "Base";
    /// La base de la taxe sur les salaires d'un salarie : la meme cle que `Base`, un montant qui se totalise.
    BaseDeLaTaxe = "base", Montant, "Base (assiette CSG sans réduction)";
    Brut = "brut", Montant, "Brut";
    Bulletins = "bulletins", Nombre, "Bulletins";
    Cachets = "cachets", Nombre, "Cachets";
    Calcule = "calcule", Montant, "0,1 % de la tranche A";
    Charges = "charges", Montant, "Charges patronales";
    Chomage = "chomage", Montant, "dont chômage";
    /// La cle de la fiche d'un salarie, sur une ligne de la liste des contrats (la meme notion que `CleSalarie`).
    Cle = "cle", Texte, "Clé du salarié";
    /// La cle de la fiche d'un salarie, qui range deux homonymes a part ; aucun tableau ne la montre.
    CleSalarie = "cle_salarie", Texte, "Clé du salarié";
    Code = "code", Texte, "Code";
    Coefficient = "coefficient", Taux, "Coefficient";
    Commune = "commune", Texte, "Commune";
    Compte = "compte", Texte, "Compte";
    Contrat = "contrat", Texte, "Contrat";
    ContratType = "contrat_type", Texte, "Type de contrat";
    CotisationsPatronales = "cotisations_patronales", Montant, "Cot. patronales";
    CotisationsSalariales = "cotisations_salariales", Montant, "Cot. salariales";
    CoutEmployeur = "cout_employeur", Montant, "Coût employeur";
    Cp = "cp", Texte, "Code postal";
    Cpnef = "cpnef", Montant, "CPNEF (dialogue social)";
    Credit = "credit", Montant, "Crédit";
    Cs = "cs", Texte, "N° Congés Spectacles";
    Ctp = "ctp", Texte, "CTP";
    Date = "date", Texte, "Date";
    Debit = "debit", Montant, "Débit";
    Debut = "debut", Texte, "Début";
    Destinataire = "destinataire", Texte, "Destinataire";
    Dixieme = "dixieme", Montant, "Au dixième";
    /// Le premier jour d'une periode de versement.
    Du = "du", Texte, "Période du";
    /// La contribution due d'une annee : la meme cle que `Du`, un montant.
    ContributionDue = "du", Montant, "Contribution due";
    Email = "email", Texte, "Courriel";
    Emploi = "emploi", Texte, "Emploi";
    Entree = "entree", Texte, "Entrée";
    Fichier = "fichier", Texte, "Fichier";
    Fin = "fin", Texte, "Fin";
    Gain = "gain", Montant, "Gain";
    Genre = "genre", Texte, "Ligne";
    Heures = "heures", Nombre, "Heures";
    Identifiant = "identifiant", Texte, "SIRET ou code";
    Indemnite = "indemnite", Montant, "Indemnité retenue";
    Issue = "issue", Texte, "Issue";
    Journal = "journal", Texte, "Journal";
    Jours = "jours", Nombre, "Jours";
    Libelle = "libelle", Texte, "Libellé";
    /// Le libelle d'un compte du plan, sur une ecriture ; aucun tableau ne le montre.
    LibelleCompte = "libelle_compte", Texte, "Libellé du compte";
    Lieu = "lieu", Texte, "à";
    Maintien = "maintien", Montant, "Au maintien";
    MajorationUn = "majoration_1", Montant, "Première majoration";
    MajorationDeux = "majoration_2", Montant, "Seconde majoration";
    Masse = "masse", Montant, "Masse salariale";
    MassePermanents = "masse_permanents", Montant, "Masse hors intermittents";
    Mode = "mode", Texte, "Mode";
    /// Le mois d'une ligne.
    Mois = "mois", Texte, "Mois";
    /// Un nombre de mois : la meme cle que `Mois`, une quantite qui se totalise.
    NombreDeMois = "mois", Nombre, "Mois";
    Montant = "montant", Montant, "Montant";
    Naissance = "naissance", Texte, "Date de naissance";
    Nationalite = "nationalite", Texte, "Nationalité";
    Nature = "nature", Texte, "Nature";
    Net = "net", Montant, "Net";
    NetAPayer = "net_a_payer", Montant, "Net à payer";
    NetImposable = "net_imposable", Montant, "Net imposable";
    Nir = "nir", Texte, "NIR";
    Nom = "nom", Texte, "Nom";
    Objet = "objet", Texte, "N° d'objet";
    Ordre = "ordre", Texte, "N°";
    Organisme = "organisme", Texte, "Organisme";
    Origine = "origine", Texte, "Reconstitué depuis";
    PartPat = "part_pat", Montant, "Part patronale";
    PartSal = "part_sal", Montant, "Part salariale";
    Pas = "pas", Montant, "PAS";
    Periode = "periode", Texte, "Période";
    Permanents = "permanents", Montant, "Contribution conventionnelle";
    Piece = "piece", Texte, "Pièce";
    Plafond = "plafond", Taux, "Plafond appliqué";
    Prelevement = "prelevement", Montant, "Prélevé";
    Prenom = "prenom", Texte, "Prénom";
    Pris = "pris", Nombre, "Pris (j)";
    Qualifiant = "qualifiant", Texte, "Qualifiant";
    Qualification = "qualification", Texte, "Qualification";
    Quand = "quand", Texte, "Date";
    Reference = "reference", Texte, "Référence";
    Regle = "regle", Texte, "Règle";
    Reponse = "reponse", Texte, "Réponse";
    Report = "report", Nombre, "Report N-1 (j)";
    Retraite = "retraite", Montant, "dont retraite";
    Salarie = "salarie", Texte, "Salarié";
    Salaries = "salaries", Nombre, "Salariés";
    Sexe = "sexe", Texte, "Sexe";
    Smic = "smic", Montant, "SMIC de référence";
    Solde = "solde", Nombre, "Solde (j)";
    SoldeCrediteur = "solde_crediteur", Montant, "Solde créditeur";
    SoldeDebiteur = "solde_debiteur", Montant, "Solde débiteur";
    Sortie = "sortie", Texte, "Sortie";
    Spectacle = "spectacle", Texte, "Spectacle";
    Statut = "statut", Texte, "Statut";
    Taux = "taux", Taux, "Taux %";
    TauxAt = "taux_at", Taux, "Taux AT";
    TauxNormal = "taux_normal", Montant, "4,25 %";
    TauxPat = "taux_pat", Taux, "Taux patronal";
    TauxPatronal = "taux_patronal", Taux, "Taux patronal réel";
    TauxPermanents = "taux_permanents", Taux, "Taux %";
    TauxSal = "taux_sal", Taux, "Taux sal. %";
    Taxe = "taxe", Montant, "Taxe brute";
    Total = "total", Montant, "Total dû";
    TotalHt = "total_ht", Montant, "Total HT";
    TrancheA = "tranche_a", Montant, "Tranche A déclarée";
    Type = "type", Texte, "Type de taux";
    Urssaf = "urssaf", Montant, "dont Urssaf";
    VerseLe = "verse_le", Texte, "Versé le";
}

impl Serialize for ColonneDEtat {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.cle())
    }
}

impl ColonneDEtat {
    /// La colonne d'un tableau, sous son en-tete.
    pub fn colonne(self) -> Colonne {
        Colonne { cle: self, libelle: self.entete().into_owned(), genre: self.genre() }
    }

    /// La colonne d'un tableau, sous l'en-tete que l'etat lui donne.
    pub fn titree(self, libelle: impl Into<String>) -> Colonne {
        Colonne { cle: self, libelle: libelle.into(), genre: self.genre() }
    }
}

/// Une ligne : un dictionnaire ordonne (l'ordre d'insertion compte pour le JSON). Une cle presente a `None` s'ecrit
/// `null` ; une cle absente ne s'ecrit pas.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(transparent)]
pub struct Ligne {
    #[serde(serialize_with = "socle::ecriture::en_ordre")]
    pub champs: Vec<(ColonneDEtat, Option<Val>)>,
}

impl Ligne {
    pub fn nouvelle() -> Ligne {
        Ligne::default()
    }

    /// `d[k] = v` : remplace en gardant la place, ou ajoute a la fin.
    pub fn poser(&mut self, k: ColonneDEtat, v: Option<Val>) -> &mut Ligne {
        match self.champs.iter_mut().find(|(c, _)| *c == k) {
            Some(x) => x.1 = v,
            None => self.champs.push((k, v)),
        }
        self
    }

    pub fn t(&mut self, k: ColonneDEtat, s: impl Into<String>) -> &mut Ligne {
        self.poser(k, Some(Val::Txt(s.into())))
    }

    /// Un texte, ou rien quand il est vide.
    pub fn ot(&mut self, k: ColonneDEtat, s: Option<impl Into<String>>) -> &mut Ligne {
        self.poser(k, s.map(|s| Val::Txt(s.into())))
    }

    pub fn m(&mut self, k: ColonneDEtat, m: Mois) -> &mut Ligne {
        self.poser(k, Some(Val::Mois(m)))
    }

    /// Une grandeur dans la case `k` (un montant, un taux, des heures, la quantite ou le taux d'une ligne de paie…).
    pub fn grandeur(&mut self, k: ColonneDEtat, v: impl Into<Val>) -> &mut Ligne {
        self.poser(k, Some(v.into()))
    }

    /// Une grandeur, ou rien (la source ne la dit pas).
    pub fn grandeur_ou_rien<G: Into<Val>>(&mut self, k: ColonneDEtat, v: Option<G>) -> &mut Ligne {
        self.poser(k, v.map(Into::into))
    }

    /// Un montant.
    pub fn montant(&mut self, k: ColonneDEtat, v: Montant) -> &mut Ligne {
        self.grandeur(k, v)
    }

    /// Un montant, ou rien.
    pub fn om(&mut self, k: ColonneDEtat, v: Option<Montant>) -> &mut Ligne {
        self.grandeur_ou_rien(k, v)
    }

    /// Un taux, ou rien (le taux d'une ligne de cotisation).
    pub fn taux(&mut self, k: ColonneDEtat, v: Option<Taux>) -> &mut Ligne {
        self.grandeur_ou_rien(k, v)
    }

    /// La base d'une ligne de cotisation, ou rien : ses euros ou ses heures.
    pub fn base(&mut self, k: ColonneDEtat, v: Option<engagement::calcul::BaseDeCotisation>) -> &mut Ligne {
        match v {
            Some(engagement::calcul::BaseDeCotisation::Montant(m)) => self.montant(k, m),
            Some(engagement::calcul::BaseDeCotisation::Heures(h)) => self.grandeur(k, h),
            None => self.rien(k),
        }
    }

    /// Un taux de PAS, ou rien (deux decimales, comme la norme l'ecrit).
    pub fn taux_de_pas(&mut self, k: ColonneDEtat, v: Option<droit::pas::TauxDePas>) -> &mut Ligne {
        self.taux(k, v.map(|t| t.taux()))
    }

    /// Un compte (« combien de… »).
    pub fn compte(&mut self, k: ColonneDEtat, n: usize) -> &mut Ligne {
        self.poser(k, Some(Val::Compte(n)))
    }

    /// Le rang de la ligne dans son etat.
    pub fn rang(&mut self, k: ColonneDEtat, r: RangDansLEtat) -> &mut Ligne {
        self.poser(k, Some(Val::Rang(r)))
    }

    /// Le montant `v` ajoute a celui de la case `k`, ou pose quand elle est vide : le cumul d'une ligne d'un etat de
    /// synthese (le recapitulatif annuel, la masse salariale, le cout par spectacle).
    pub fn ajouter(&mut self, k: ColonneDEtat, v: Montant) -> R<&mut Ligne> {
        let total = match self.val(k) {
            None => v,
            Some(Val::Montant(avant)) => *avant + v,
            Some(autre) => return Err(erreur(Classe::CalculImpossible, format!("La case « {} » porte « {} », pas un montant.", k.cle(), autre.en_texte_ecrit()))),
        };
        Ok(self.montant(k, total))
    }

    pub fn rien(&mut self, k: ColonneDEtat) -> &mut Ligne {
        self.poser(k, None)
    }

    /// `d.get(k)` distinguant l'absente (`None`) de la presente a `None` (`Some(None)`).
    pub fn get(&self, k: ColonneDEtat) -> Option<&Option<Val>> {
        self.champs.iter().find(|(c, _)| *c == k).map(|(_, v)| v)
    }

    /// La valeur, ou rien.
    pub fn val(&self, k: ColonneDEtat) -> Option<&Val> {
        self.get(k).and_then(|v| v.as_ref())
    }

    /// Le texte de la case `k`, tel qu'il s'ecrit : rien pour une case vide (absente ou `None`). C'est ce qui range et
    /// groupe les lignes.
    pub fn texte(&self, k: ColonneDEtat) -> String {
        self.val(k).map(Val::en_texte_ecrit).into_iter().collect()
    }

    /// Le mois de la case `k`, quand elle en porte un (`m`).
    pub fn mois(&self, k: ColonneDEtat) -> Option<Mois> {
        match self.val(k) {
            Some(Val::Mois(m)) => Some(*m),
            _ => None,
        }
    }
}

// ------------------------------------------------------------------ le tableau

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Genre {
    Montant,
    Nombre,
    Taux,
    Texte,
}

impl Genre {
    pub fn nom(self) -> &'static str {
        match self {
            Genre::Montant => "montant",
            Genre::Nombre => "nombre",
            Genre::Taux => "taux",
            Genre::Texte => "texte",
        }
    }
}

impl Serialize for Genre {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.nom())
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Colonne {
    pub cle: ColonneDEtat,
    pub libelle: String,
    #[serde(rename = "type")]
    pub genre: Genre,
}

impl Colonne {
    pub fn totalisee(&self) -> bool {
        matches!(self.genre, Genre::Montant | Genre::Nombre)
    }

    pub fn a_droite(&self) -> bool {
        matches!(self.genre, Genre::Montant | Genre::Nombre | Genre::Taux)
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Section {
    pub libelle: String,
    pub lignes: Vec<Ligne>,
    pub total: Option<Ligne>,
}

impl Section {
    pub fn nouvelle(libelle: impl Into<String>, lignes: Vec<Ligne>) -> Section {
        Section { libelle: libelle.into(), lignes, total: None }
    }
}

/// UN TABLEAU, tel que la route le rend : son fichier ne s'ecrit que par son nom.
#[derive(Clone, Debug, Serialize)]
pub struct Tableau {
    pub etat: EtatDExport,
    pub titre: String,
    pub periode: String,
    pub colonnes: Vec<Colonne>,
    pub sections: Vec<Section>,
    pub total: Option<Ligne>,
    pub avertissements: Vec<String>,
    pub sources: Vec<String>,
    pub mois: Vec<Mois>,
    pub format: FormatDExport,
    #[serde(serialize_with = "nom_du_fichier")]
    pub fichier: Option<PathBuf>,
    pub sous_totaux: bool,
    pub avec_total: bool,
    pub donnees_personnelles: bool,
}

/// Le nom seul d'un fichier ecrit (`null` s'il n'est pas ecrit) ; un chemin sans nom ne s'ecrit pas.
pub fn nom_du_fichier<S: Serializer>(f: &Option<PathBuf>, s: S) -> Result<S::Ok, S::Error> {
    match f {
        Some(p) => s.serialize_str(socle::fichiers::NomDeFichier::du(p).map_err(|e| serde::ser::Error::custom(e.phrase))?.texte()),
        None => s.serialize_none(),
    }
}

impl Tableau {
    pub fn nouveau(etat: EtatDExport, titre: impl Into<String>, periode: impl Into<String>) -> Tableau {
        Tableau {
            etat,
            titre: titre.into(),
            periode: periode.into(),
            colonnes: Vec::new(),
            sections: Vec::new(),
            total: None,
            avertissements: Vec::new(),
            sources: Vec::new(),
            mois: Vec::new(),
            format: FormatDExport::Csv,
            fichier: None,
            sous_totaux: true,
            avec_total: true,
            donnees_personnelles: false,
        }
    }

}

/// Les lignes en sections, une par clé, les clés triées,
/// les lignes de chaque section triées par `tri` s'il y en a un (tri stable).
pub fn sections_de<K: Ord + Clone>(
    lignes: Vec<Ligne>, cle: impl Fn(&Ligne) -> K, libelle: impl Fn(&K) -> R<String>, tri: Option<&dyn Fn(&Ligne) -> Vec<String>>,
) -> R<Vec<Section>> {
    let mut paquets: BTreeMap<K, Vec<Ligne>> = BTreeMap::new();
    for l in lignes {
        paquets.entry(cle(&l)).or_default().push(l);
    }
    paquets
        .into_iter()
        .map(|(k, mut v)| {
            if let Some(t) = tri {
                v.sort_by_cached_key(|l| t(l));
            }
            Ok(Section { libelle: libelle(&k)?, lignes: v, total: None })
        })
        .collect()
}

/// Le libelle d'une section par mois (la case `mois` de ses lignes, `Ligne::mois`) : « Septembre 2026 ».
pub fn section_du_mois(m: &Option<Mois>) -> R<String> {
    m.map(libelle_mois).ok_or_else(sans_mois)
}

/// Le refus d'une ligne rangee par son mois qui n'en porte pas.
pub fn sans_mois() -> Echec {
    erreur(Classe::Illisible, "Une ligne de l'état n'a pas de mois.")
}

// ------------------------------------------------------------------ totaux

/// La somme de ce qui est DECLARE, ou `None` quand aucune ligne ne l'est.
fn somme(lignes: &[&Ligne], cle: ColonneDEtat) -> R<Option<Val>> {
    let mut total: Option<Val> = None;
    for l in lignes {
        let v = match l.get(cle) {
            Some(Some(Val::Txt(t))) if t.is_empty() => continue,
            Some(Some(v)) => v,
            _ => continue,
        };
        total = Some(match total {
            Some(t) => t.plus(v)?,
            None => v.premiere_du_total()?,
        });
    }
    Ok(total)
}

/// La ligne d'un total des `lignes` : la premiere colonne de texte porte le libelle, les colonnes totalisees leur somme,
/// les autres restent vides.
fn ligne_du_total(colonnes: &[Colonne], lignes: &[&Ligne], libelle: &str) -> R<Ligne> {
    let porte = colonnes.iter().find(|c| c.genre == Genre::Texte).map(|c| c.cle);
    let mut d = Ligne::nouvelle();
    for c in colonnes.iter().filter(|c| c.totalisee()) {
        d.poser(c.cle, somme(lignes, c.cle)?);
    }
    if let Some(p) = porte {
        d.t(p, libelle);
    }
    Ok(d)
}

/// Pose le sous-total de chaque section (« Total ... ») et le total general (`totaliser_general`).
pub fn totaliser(t: &mut Tableau, libelle_total: &str) -> R<()> {
    for s in t.sections.iter_mut() {
        s.total = if s.libelle.is_empty() || !t.avec_total {
            None
        } else {
            let l: Vec<&Ligne> = s.lignes.iter().collect();
            Some(ligne_du_total(&t.colonnes, &l, &format!("Total {}", s.libelle))?)
        };
    }
    totaliser_general(t, libelle_total)
}

/// Pose le total general de toutes les lignes, quand le tableau en a un ; le sous-total d'une section reste celui que
/// l'etat lui a donne (le grand livre : le total de son compte).
pub fn totaliser_general(t: &mut Tableau, libelle_total: &str) -> R<()> {
    t.total = if t.avec_total {
        let toutes: Vec<&Ligne> = t.sections.iter().flat_map(|s| s.lignes.iter()).collect();
        Some(ligne_du_total(&t.colonnes, &toutes, libelle_total)?)
    } else {
        None
    };
    Ok(())
}

/// Le libelle du total general : « TOTAL GÉNÉRAL ».
pub const TOTAL_GENERAL: &str = "TOTAL GÉNÉRAL";

// ------------------------------------------------------------ mise en forme

/// Une cellule en texte francais, virgule decimale ; une case vide ne s'ecrit pas (rien).
pub fn texte(v: Option<&Val>, c: &Colonne) -> String {
    v.map(|v| match (c.a_droite(), v.est_une_grandeur()) {
        // Une grandeur a la francaise, sa virgule decimale.
        (true, true) => v.en_texte_ecrit().replace('.', ","),
        _ => v.en_texte_ecrit(),
    })
    .into_iter()
    .collect()
}

/// Ce qu'une rangee d'un tableau est : une ligne, le sous-total d'une section, ou le total general.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rangee {
    Ligne,
    Total,
    General,
}

impl Rangee {
    /// La classe HTML d'un total ; une ligne n'en a pas.
    fn classe(self) -> Option<&'static str> {
        match self {
            Rangee::Ligne => None,
            Rangee::Total => Some("total"),
            Rangee::General => Some("general"),
        }
    }
}

/// (cellules, genre) dans l'ordre d'ecriture : lignes, sous-totaux, total.
fn rangees(t: &Tableau) -> Vec<(&Ligne, Rangee)> {
    let plusieurs = t.sections.len() > 1 && t.sous_totaux;
    let mut out: Vec<(&Ligne, Rangee)> = Vec::new();
    for s in &t.sections {
        for l in &s.lignes {
            out.push((l, Rangee::Ligne));
        }
        if plusieurs {
            if let Some(tot) = &s.total {
                out.push((tot, Rangee::Total));
            }
        }
    }
    if let Some(tot) = &t.total {
        out.push((tot, Rangee::General));
    }
    out
}

/// `(texte, genre)` de chaque cellule d'une ligne, `—` pour un montant de ligne que la source ne dit pas.
fn cellule_lisible(l: &Ligne, c: &Colonne, genre: Rangee) -> String {
    let v = l.get(c.cle);
    if matches!(v, Some(None)) && c.genre == Genre::Montant && genre == Rangee::Ligne {
        "—".to_string()
    } else {
        texte(v.and_then(|x| x.as_ref()), c)
    }
}

/// Le nom de la compagnie, s'il est dit.
pub fn nom_compagnie(ctx: &Contexte) -> Option<&str> {
    ctx.entreprise.nommee()
}

fn entete_texte(t: &Tableau, nom: Option<&str>) -> Vec<String> {
    let mut out = vec![format!("{} — {}", t.titre, t.periode)];
    if let Some(nom) = nom {
        out.insert(0, nom.to_string());
    }
    out
}

fn lire_nom(ctx: Option<&Contexte>) -> Option<&str> {
    ctx.and_then(nom_compagnie)
}

// ---------------------------------------------------------------------- csv

/// `csv_tableau` : point-virgule, virgule decimale, UTF-8 avec BOM.
pub fn csv_tableau(t: &Tableau, ctx: Option<&Contexte>) -> R<Vec<u8>> {
    let mut s = String::from("\u{feff}");
    let ligne = |s: &mut String, c: &[String]| s.push_str(&socle::csv::ligne(c, ';'));
    for x in entete_texte(t, lire_nom(ctx)) {
        ligne(&mut s, &[x]);
    }
    ligne(&mut s, &[]);
    ligne(&mut s, &t.colonnes.iter().map(|c| c.libelle.clone()).collect::<Vec<_>>());
    let mut precedent = Rangee::Ligne;
    for (d, genre) in rangees(t) {
        if genre == Rangee::General && precedent == Rangee::Total {
            ligne(&mut s, &[]);
        }
        ligne(&mut s, &t.colonnes.iter().map(|c| texte(d.val(c.cle), c)).collect::<Vec<_>>());
        if genre == Rangee::Total {
            ligne(&mut s, &[]);
        }
        precedent = genre;
    }
    for a in &t.avertissements {
        ligne(&mut s, &[]);
        ligne(&mut s, &[format!("! {}", a)]);
    }
    if !t.sources.is_empty() {
        ligne(&mut s, &[]);
        for src in &t.sources {
            ligne(&mut s, &[format!("Source : {}", src)]);
        }
    }
    Ok(s.into_bytes())
}

// ---------------------------------------------------------------------- tsv

/// `tsv_tableau` : les donnees seules. Une colonne « Groupe » en tete quand il y a plusieurs sections.
pub fn tsv_tableau(t: &Tableau, _ctx: Option<&Contexte>) -> R<Vec<u8>> {
    let groupe = t.sections.len() > 1;
    let mut s = String::new();
    let mut entete: Vec<String> = if groupe { vec!["Groupe".to_string()] } else { Vec::new() };
    entete.extend(t.colonnes.iter().map(|c| c.libelle.clone()));
    s.push_str(&socle::csv::ligne(&entete, '\t'));
    for sec in &t.sections {
        for l in &sec.lignes {
            let mut cel: Vec<String> = if groupe { vec![sec.libelle.clone()] } else { Vec::new() };
            cel.extend(t.colonnes.iter().map(|c| texte(l.val(c.cle), c)));
            s.push_str(&socle::csv::ligne(&cel, '\t'));
        }
    }
    Ok(s.into_bytes())
}

// --------------------------------------------------------------------- json

/// CE QUE LE FICHIER JSON D'UN TABLEAU PORTE : le fichier n'y est pas encore nomme quand il s'ecrit (`null`).
#[derive(Serialize)]
struct FichierTableau<'a> {
    etat: EtatDExport,
    titre: &'a str,
    periode: &'a str,
    mois: &'a [Mois],
    colonnes: &'a [Colonne],
    sections: &'a [Section],
    total: Option<&'a Ligne>,
    avertissements: &'a [String],
    sources: &'a [String],
    donnees_personnelles: bool,
    format: FormatDExport,
    fichier: Option<&'a socle::fichiers::NomDeFichier>,
}

/// Le tableau en JSON indente d'une espace.
pub fn json_tableau(t: &Tableau, _ctx: Option<&Contexte>) -> R<Vec<u8>> {
    let f = FichierTableau {
        etat: t.etat,
        titre: &t.titre,
        periode: &t.periode,
        mois: &t.mois,
        colonnes: &t.colonnes,
        sections: &t.sections,
        total: t.total.as_ref(),
        avertissements: &t.avertissements,
        sources: &t.sources,
        donnees_personnelles: t.donnees_personnelles,
        format: t.format,
        fichier: None,
    };
    Ok(socle::ecriture::json_indente(&f, 1).into_bytes())
}

// --------------------------------------------------------------------- xlsx

/// La case `v` dans la cellule (`ligne`, `col`) : un nombre (une grandeur, un compte, un rang) dans le style `nombre`,
/// un mois ou un texte dans le style `texte` ; une case vide, ou un texte vide, n'ecrit rien.
fn ecrire_cellule(ws: &mut Feuille, ligne: u32, col: u16, v: Option<&Val>, nombre: &Style, texte: &Style) {
    let Some(v) = v else { return };
    chaque_grandeur!(v, g => ws.nombre(ligne, col, g, nombre);
        Val::Compte(n) => ws.entier(ligne, col, n, nombre),
        Val::Rang(r) => ws.entier(ligne, col, r.0, nombre),
        Val::Mois(m) => ws.texte(ligne, col, m.to_string(), texte),
        Val::Txt(ref s) => {
            if !s.is_empty() {
                ws.texte(ligne, col, s.clone(), texte)
            }
        },
    )
}

/// Le nom de la feuille : celui de l'etat (`livre-de-paie`), qui tient dans les 31 caracteres qu'Excel admet et n'en
/// porte aucun qu'il refuse ; le titre entier est la premiere ligne de la feuille.
fn nom_de_feuille(t: &Tableau) -> R<&'static str> {
    let nom = t.etat.as_str();
    if nom.is_empty() || nom.chars().count() > 31 || nom.chars().any(|c| "[]:*?/\\'".contains(c)) {
        return Err(refus(format!(
            "L'état « {} » ne peut pas nommer une feuille Excel : 31 caractères au plus, sans crochet, deux-points, astérisque, point d'interrogation, barre ni apostrophe. Le renommer dans le moteur.",
            nom
        )));
    }
    Ok(nom)
}

/// `xlsx_tableau`.
pub fn xlsx_tableau(t: &Tableau, ctx: Option<&Contexte>) -> R<Vec<u8>> {
    let mut ws = Feuille::nouvelle(nom_de_feuille(t)?);
    let mut ligne: u32 = 0;
    for x in entete_texte(t, lire_nom(ctx)) {
        ws.texte(ligne, 0, x, &Style::gras(13));
        ligne += 1;
    }
    ligne += 1;
    let entete = Style { gras: true, retour_a_la_ligne: true, vertical: Some(Vertical::Centre), ..Style::CLASSEUR };
    for (i, c) in t.colonnes.iter().enumerate() {
        ws.texte(ligne, i as u16, c.libelle.clone(), &entete);
    }
    let ligne_entete = ligne;
    ligne += 1;
    let nombre_de = |c: &Colonne| -> Option<&'static str> {
        match c.genre {
            Genre::Montant => Some("# ##0.00 \u{20ac}"),
            Genre::Nombre => Some("# ##0.##"),
            Genre::Taux => Some("0.000##"),
            Genre::Texte => None,
        }
    };
    for (d, genre) in rangees(t) {
        let (gras, corps) = match genre {
            Rangee::General => (true, 12),
            Rangee::Total => (true, CORPS_DU_CLASSEUR),
            Rangee::Ligne => (false, CORPS_DU_CLASSEUR),
        };
        for (i, c) in t.colonnes.iter().enumerate() {
            let texte = Style { gras, corps, ..Style::CLASSEUR };
            let nombre = Style { format: nombre_de(c), ..texte.clone() };
            ecrire_cellule(&mut ws, ligne, i as u16, d.val(c.cle), &nombre, &texte);
        }
        ligne += 1;
    }
    let simple = Style::CLASSEUR;
    for a in &t.avertissements {
        ligne += 1;
        ws.texte(ligne, 0, format!("! {}", a), &simple);
        ligne += 1;
    }
    for s in &t.sources {
        ws.texte(ligne, 0, format!("Source : {}", s), &simple);
        ligne += 1;
    }
    for (i, c) in t.colonnes.iter().enumerate() {
        ws.largeur(i as u16, i as u16, if c.a_droite() { 14 } else { 26 });
    }
    ws.figer(ligne_entete + 1);
    ws.octets()
}

// --------------------------------------------------------------- html / pdf

const CSS: &str = "\n* { font-family: Arial, sans-serif; }\nbody { font-size: 7pt; color: #2C3E50; }\nh1 { font-size: 11pt; color: #B22222; margin: 0 0 1pt 0;\n     text-transform: uppercase; letter-spacing: 0.5pt; }\n.sub { font-size: 6.8pt; color: #555; margin: 0 0 6pt 0; }\n.rs { font-size: 8.5pt; font-weight: bold; color: #2C3E50; margin: 0 0 1pt 0; }\ntable { width: 100%; border-collapse: collapse; }\nth { font-size: 6pt; color: #B22222; text-transform: uppercase;\n     letter-spacing: 0.3pt; text-align: left; padding: 0 3pt 2pt 0;\n     border-bottom: 0.7pt solid #B22222; }\ntd { font-size: 7pt; padding: 1.6pt 3pt 1.6pt 0; border-bottom: 0.3pt solid #eee; }\nth.n, td.n { text-align: right; }\ntr.total td { font-weight: bold; border-top: 0.5pt solid #999;\n              border-bottom: 0.5pt solid #999; }\ntr.general td { font-weight: bold; font-size: 8pt; color: #B22222; }\n.note { font-size: 6.3pt; color: #777; margin: 6pt 0 0 0; }\n.source { font-size: 6pt; color: #999; margin: 2pt 0 0 0; }\n";

/// `tableaux._e`.
fn echapper(v: &str) -> String {
    v.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// `html_tableau`.
pub fn html_tableau(t: &Tableau, ctx: Option<&Contexte>) -> R<Vec<u8>> {
    let mut corps = String::new();
    for (d, genre) in rangees(t) {
        let mut cel = String::new();
        for c in &t.colonnes {
            let v = d.get(c.cle);
            let s = if matches!(v, Some(None)) && c.genre == Genre::Montant && genre == Rangee::Ligne {
                "&mdash;".to_string()
            } else {
                echapper(&texte(v.and_then(|x| x.as_ref()), c))
            };
            cel.push_str(&format!("<td{}>{}</td>", if c.a_droite() { " class='n'" } else { "" }, s));
        }
        corps.push_str(&format!("<tr{}>{}</tr>", genre.classe().iter().map(|classe| format!(" class='{}'", classe)).collect::<String>(), cel));
    }
    let th: String = t.colonnes.iter().map(|c| format!("<th{}>{}</th>", if c.a_droite() { " class='n'" } else { "" }, echapper(&c.libelle))).collect();
    let nom = lire_nom(ctx);
    let tete = format!(
        "{}<h1>{}</h1><p class='sub'>{}{}</p>",
        // Le nom de la compagnie, quand il est dit ; sinon la page n'en a pas la ligne.
        nom.map(|nom| format!("<p class='rs'>{}</p>", echapper(nom))).into_iter().collect::<String>(),
        echapper(&t.titre),
        echapper(&t.periode),
        if t.donnees_personnelles { " &mdash; porte des données personnelles" } else { "" }
    );
    let pied: String = t.avertissements.iter().map(|a| format!("<p class='note'>! {}</p>", echapper(a))).collect::<String>()
        + &t.sources.iter().map(|s| format!("<p class='source'>Source : {}</p>", echapper(s))).collect::<String>();
    let page = format!(
        "<!DOCTYPE html><html lang='fr'><head><meta charset='utf-8'><title>{} — {}</title><style>{} body{{font-size:10pt;margin:24px}}td,th{{font-size:9pt}}</style></head><body>{}<table><tr>{}</tr>{}</table>{}</body></html>",
        echapper(&t.titre),
        echapper(&t.periode),
        CSS,
        tete,
        th,
        corps,
        pied
    );
    Ok(page.into_bytes())
}

const PAGE_LARGEUR: pdf::Mesure = pdf::Mesure::milliemes(841_890);
const PAGE_HAUTEUR: pdf::Mesure = pdf::Mesure::milliemes(595_280);
const ROUGE: &str = "#B22222";
const ENCRE: &str = "#2C3E50";

const CHASSE: pdf::Mesure = mesure("3.9");
const CHASSE_ENTETE: pdf::Mesure = mesure("4.2");
const MARGE_CELLULE: pdf::Mesure = pdf::Mesure::entier(3);
/// La ligne de texte la plus longue, en caracteres, avant qu'elle raccourcisse.
const LIGNE_DE_TEXTE: i64 = 40;

/// La largeur de chaque colonne, en fraction de la largeur utile. Une longueur s'y compte en caracteres (gonfles d'un
/// dixieme pour un total, de trois pour le total general), une largeur en points.
pub(crate) fn fractions(entetes: &[(String, bool)], rangees: &[(Vec<String>, Rangee)], utile: pdf::Mesure) -> Vec<pdf::Mesure> {
    use pdf::Mesure;
    let n = entetes.len();
    let mut plus_long = vec![Mesure::ZERO; n];
    let mut plus_long_mot = vec![Mesure::ZERO; n];
    let mut mot_entete = vec![Mesure::ZERO; n];
    for (i, (libelle, _)) in entetes.iter().enumerate() {
        mot_entete[i] = Mesure::entier(libelle.split(socle::texte::blanc).map(|m| m.chars().count()).fold(0, usize::max) as i64);
    }
    let gonfle = |caracteres: usize, genre: Rangee| {
        let (n, d) = match genre {
            Rangee::Total => (11, 10),
            Rangee::General => (13, 10),
            Rangee::Ligne => (1, 1),
        };
        match Mesure::entier(caracteres as i64).fois_rapport(n, d) {
            Some(m) => m,
            None => unreachable!("un rapport de dix"),
        }
    };
    for (cellules, genre) in rangees {
        for (i, c) in cellules.iter().take(n).enumerate() {
            plus_long[i] = plus_long[i].max(gonfle(c.chars().count(), *genre));
            for m in c.split(socle::texte::blanc).filter(|m| !m.is_empty()) {
                plus_long_mot[i] = plus_long_mot[i].max(gonfle(m.chars().count(), *genre));
            }
        }
    }
    let texte: Vec<bool> = entetes.iter().map(|(_, a_droite)| !a_droite).collect();
    let besoins = |ligne: i64| -> Vec<Mesure> {
        (0..n)
            .map(|i| {
                let lg = if texte[i] { plus_long[i].min(Mesure::entier(ligne)).max(plus_long_mot[i]) } else { plus_long[i] };
                (lg * CHASSE).max(mot_entete[i] * CHASSE_ENTETE) + MARGE_CELLULE * 2
            })
            .collect()
    };
    let somme_de = |b: &[Mesure]| b.iter().sum::<Mesure>();
    // La ligne de texte la plus longue, en caracteres : elle raccourcit jusqu'a ce que les colonnes tiennent.
    let mut ligne = LIGNE_DE_TEXTE;
    let mut b = besoins(ligne);
    while somme_de(&b) > utile && ligne > 8 {
        ligne -= 2;
        b = besoins(ligne);
    }
    let total = somme_de(&b);
    b.iter()
        .map(|x| match x.sur(total) {
            Some(f) => f,
            None => unreachable!("chaque colonne a sa marge : le total n'est pas nul"),
        })
        .collect()
}

fn filets(couleur: &str, epaisseur: pdf::Mesure) -> Option<pdf::Filet> {
    Some(pdf::Filet { couleur: couleur.to_string(), epaisseur })
}

/// Un paragraphe d'un seul morceau droit, à gauche ; sans couleur, celle de la police.
fn paragraphe(texte: String, gras: bool, taille: pdf::Mesure, couleur: Option<&str>, majuscules: bool, avant: pdf::Mesure, apres: pdf::Mesure) -> pdf::Bloc {
    pdf::Bloc::Paragraphe(pdf::Paragraphe {
        morceaux: vec![pdf::Morceau { texte, gras, italique: false, couleur: None }],
        taille: Some(taille),
        couleur: couleur.map(str::to_string),
        alignement: pdf::Alignement::Gauche,
        majuscules,
        espace_avant: avant,
        espace_apres: apres,
        retrait: pdf::Mesure::ZERO,
    })
}

/// Une ligne maigre de cellules en texte, sans rien d'autre.
fn ligne_de(cellules: Vec<String>) -> pdf::Ligne {
    pdf::Ligne {
        cellules: cellules.into_iter().map(pdf::Cellule::Texte).collect(),
        gras: false,
        fond: None,
        couleur: None,
        taille: None,
        filet_haut: None,
        filet_bas: None,
    }
}

/// La description (FORMAT.md) d'un etat -- A4 paysage, son titre, son tableau, ses
/// notes. `entetes` : (libelle, a droite) ; `rangees` : (cellules en texte, ce qu'est la rangee).
pub(crate) fn description_pdf(
    titre: &str, periode: &str, nom: Option<&str>, sous_titre: &str, pied: &str, entetes: &[(String, bool)], rangees: &[(Vec<String>, Rangee)], notes: &[String],
    sources: &[String],
) -> pdf::Description {
    let fr = fractions(entetes, rangees, PAGE_LARGEUR - pdf::Mesure::entier(68));
    let colonnes = fr
        .iter()
        .zip(entetes)
        .map(|(f, e)| pdf::ColonneTableau { largeur: *f, alignement: if e.1 { pdf::Alignement::Droite } else { pdf::Alignement::Gauche } })
        .collect();
    let lignes = rangees
        .iter()
        .map(|(cellules, genre)| {
            let mut l = ligne_de(cellules.clone());
            if *genre == Rangee::Total {
                l.gras = true;
                l.filet_haut = filets("#999999", mesure("0.5"));
                l.filet_bas = filets("#999999", mesure("0.5"));
            } else if *genre == Rangee::General {
                l.gras = true;
                l.taille = Some(pdf::Mesure::entier(8));
                l.couleur = Some(ROUGE.to_string());
                l.filet_haut = filets("#999999", mesure("0.5"));
                l.filet_bas = filets("#999999", mesure("0.5"));
            }
            l
        })
        .collect();
    let mut entete = ligne_de(entetes.iter().map(|e| e.0.clone()).collect());
    entete.gras = true;
    entete.taille = Some(mesure("6.3"));
    entete.couleur = Some(ROUGE.to_string());
    entete.filet_bas = filets(ROUGE, mesure("0.7"));
    let mut blocs: Vec<pdf::Bloc> = Vec::new();
    let pt = pdf::Mesure::entier;
    let zero = pdf::Mesure::ZERO;
    if let Some(nom) = nom {
        blocs.push(paragraphe(nom.to_string(), true, mesure("8.5"), None, false, zero, pt(1)));
    }
    blocs.push(paragraphe(titre.to_string(), true, pt(11), Some(ROUGE), true, zero, pt(1)));
    blocs.push(paragraphe(sous_titre.to_string(), false, mesure("6.8"), Some("#555555"), false, zero, pt(6)));
    blocs.push(pdf::Bloc::Tableau(pdf::Tableau {
        colonnes,
        entete: Some(vec![entete]),
        lignes: Some(lignes),
        taille: Some(pt(7)),
        couleur: Some(ENCRE.to_string()),
        interligne: None,
        marge_cellule: pdf::MargeCellule { haut: mesure("1.6"), droite: pt(3), bas: mesure("1.6"), gauche: pt(3) },
        filet_lignes: filets("#EEEEEE", mesure("0.3")),
        filets_cellules: None,
        espace_avant: zero,
        espace_apres: zero,
    }));
    for a in notes {
        blocs.push(paragraphe(format!("! {}", a), false, mesure("6.3"), Some("#777777"), false, pt(6), zero));
    }
    for src in sources {
        blocs.push(paragraphe(format!("Source : {}", src), false, pt(6), Some("#999999"), false, pt(2), zero));
    }
    pdf::Description {
        format: crate::bulletin::FORMAT,
        page: Some(pdf::Page {
            largeur: PAGE_LARGEUR,
            hauteur: PAGE_HAUTEUR,
            marges: pdf::Marges { haut: pt(40), droite: pt(34), bas: pt(40), gauche: pt(34) },
        }),
        police: Some(pdf::Police { taille: pt(7), interligne: mesure("1.25"), couleur: ENCRE.to_string() }),
        style: None,
        logo: None,
        titre: Some(format!("{} — {}", titre, periode)),
        documents: vec![pdf::Document::Blocs(blocs)],
        entete_page: Some(pdf::Repere {
            gauche: Some(format!("{} — {} (suite)", titre, periode)),
            centre: None,
            droite: None,
            taille: Some(mesure("8.5")),
            couleur: Some(ENCRE.to_string()),
            gras: true,
            distance: pt(24),
            premiere_page: false,
            image: None,
        }),
        pied_page: Some(pdf::Repere {
            gauche: Some(pied.to_string()),
            centre: None,
            droite: Some("page {page} sur {pages}".to_string()),
            taille: Some(mesure("6.5")),
            couleur: Some("#777777".to_string()),
            gras: false,
            distance: pt(20),
            premiere_page: true,
            image: None,
        }),
    }
}

/// `pdf_fabrique` : les octets du PDF mis en page par la fabrique. Un refus de la fabrique est `Refuse`.
pub fn pdf_fabrique(description: &pdf::Description) -> R<Vec<u8>> {
    fabrique_pdf::fabriquer(description).map_err(|r| crate::fabrique::echec(r).reformuler(|p| format!("PDF impossible : la fabrique de PDF refuse cet état ({}). Aucun PDF n'est fait autrement.", p)))
}

/// `pdf_tableau` : A4 paysage, l'en-tete des colonnes a chaque page, un pied numerote.
pub fn pdf_tableau(t: &Tableau, ctx: Option<&Contexte>) -> R<Vec<u8>> {
    let sous = format!("{}{}", t.periode, if t.donnees_personnelles { " — porte des données personnelles" } else { "" });
    let rangs: Vec<(Vec<String>, Rangee)> = rangees(t).into_iter().map(|(d, genre)| (t.colonnes.iter().map(|c| cellule_lisible(d, c, genre)).collect(), genre)).collect();
    let entetes: Vec<(String, bool)> = t.colonnes.iter().map(|c| (c.libelle.clone(), c.a_droite())).collect();
    let d = description_pdf(&t.titre, &t.periode, lire_nom(ctx), &sous, &format!("{} — {}", t.titre, t.periode), &entetes, &rangs, &t.avertissements, &t.sources);
    pdf_fabrique(&d)
}

// ------------------------------------------------------------------ l'ecriture

/// Les octets d'un tableau dans son format.
pub fn octets(t: &Tableau, ctx: Option<&Contexte>) -> R<Vec<u8>> {
    match t.format {
        FormatDExport::Csv => csv_tableau(t, ctx),
        FormatDExport::Tsv => tsv_tableau(t, ctx),
        FormatDExport::Xlsx => xlsx_tableau(t, ctx),
        FormatDExport::Pdf => pdf_tableau(t, ctx),
        FormatDExport::Html => html_tableau(t, ctx),
        FormatDExport::Json => json_tableau(t, ctx),
        autre @ (FormatDExport::Fec
        | FormatDExport::Ximport
        | FormatDExport::Oxygene
        | FormatDExport::Sage
        | FormatDExport::Quadra
        | FormatDExport::Ebp
        | FormatDExport::Pennylane) => Err(refus(format!("Format inconnu pour cet état : « {} ».", autre))),
    }
}

/// Le format demande, s'il est l'un de `formats`. (Le XLSX et le PDF sont toujours
/// disponibles : ni openpyxl ni la fabrique externe ne sont plus en jeu.) Une demande sans format n'en est aucun.
pub fn format_possible(fmt: Option<FormatDExport>, formats: &[FormatDExport]) -> R<FormatDExport> {
    match fmt {
        Some(f) if formats.contains(&f) => Ok(f),
        autre => {
            let ecrit = match autre {
                Some(f) => f.as_str(),
                None => "",
            };
            Err(refus(format!("Format inconnu pour cet état : « {} ». Les formats possibles sont {}.", ecrit, formats.iter().map(|f| f.as_str()).collect::<Vec<_>>().join(", "))))
        }
    }
}

/// Ecrit le tableau dans `dossier/nom.<ext>` (`tsv` s'ecrit `.txt`) et rend le chemin.
pub fn ecrire(t: &mut Tableau, dossier: &Path, nom: &str, ctx: &Contexte) -> R<PathBuf> {
    std::fs::create_dir_all(dossier).map_err(|e| socle::echec::du_disque(&dossier, &e).reclasser(Classe::Disque))?;
    let ext = if t.format == FormatDExport::Tsv { "txt" } else { t.format.as_str() };
    let chemin = dossier.join(format!("{}.{}", nom, ext));
    let contenu = octets(t, Some(ctx))?;
    std::fs::write(&chemin, contenu).map_err(|e| socle::echec::du_disque(&chemin, &e).reclasser(Classe::Disque))?;
    t.fichier = Some(chemin.clone());
    Ok(chemin)
}

// ------------------------------------------------------------------ petits outils

/// « Septembre 2026 » (le nom en titre).
pub fn libelle_mois(m: Mois) -> String {
    m.libelle(socle::mois::Graphie::Titre)
}

/// AAAA-MM-JJ -> JJ/MM/AAAA ; le reste tel quel.
pub fn jj(iso: &str) -> String {
    let c: Vec<char> = iso.chars().collect();
    if c.len() >= 10 && c[4] == '-' {
        format!("{}/{}/{}", c[8..10].iter().collect::<String>(), c[5..7].iter().collect::<String>(), c[..4].iter().collect::<String>())
    } else {
        iso.to_string()
    }
}

/// « NOM Prenom ».
pub fn qui(fiche: &droit::fiche::ChampsSalarie) -> R<String> {
    if fiche.nom.is_none() {
        return Err(socle::fichiers::absent("nom").reclasser(Classe::Manque));
    }
    Ok(qui_souple(fiche))
}

/// Le « qui » d'une fiche qui n'a peut-etre pas de nom : le nom et le prenom (`prenom_affichage`, sinon `prenom`) qu'elle
/// dit.
pub fn qui_souple(fiche: &droit::fiche::ChampsSalarie) -> String {
    [fiche.nom.portee(), fiche.prenom_affichage.portee().or(fiche.prenom.portee())].into_iter().flatten().collect::<Vec<_>>().join(" ")
}

