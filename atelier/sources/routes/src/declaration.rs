//! LA CHARGE D'UNE ROUTE SE DECLARE UNE FOIS, ET SA FICHE EN SORT. Une seule declaration (`charge!`) donne la structure que
//! la route lit (`serde`, `deny_unknown_fields`) et les champs que sa fiche dit (`ChargeDuType`) : le nom de chaque champ
//! est celui de la structure ; sa forme, son type la dit (`ValeurDeLaCharge`) : une liste fermee est un `choix`, un jour
//! une date, un `bool` un oui-non… ; il est obligatoire s'il n'est pas `Option`. Ne s'ecrit a la main que ce que le type
//! ne peut pas savoir : son libelle, la phrase de son sens, s'il est « avance » (`Legende`). Un champ ne peut donc ni
//! manquer a la fiche, ni s'y dire autrement que son type : le compilateur le tient.
//!
//! ```ignore
//! charge! {
//!     /// La charge de `deposer` : …
//!     #[derive(Debug, Serialize)]
//!     pub(crate) struct ChargeDeposer {
//!         quoi: Quoi = QUOI,
//!         #[serde(skip_serializing_if = "Option::is_none")]
//!         mois: Option<Mois> = legendes::MOIS,
//!         forcer: Option<bool> = dit("Annule et remplace", "vrai : l'annule-et-remplace …").avancee(),
//!     }
//! }
//! // la fiche : `charge: &ChargeDeposer::CHARGE`
//! ```

use std::marker::PhantomData;

use crate::catalogue::{Affiche, Champ, Forme, ListeDeChoix};

/// LA FORME QU'UN TYPE DONNE A UNE VALEUR DE LA CHARGE, et s'il l'exige. `Sorte` ne sert qu'a ranger les impls sans
/// qu'elles se recouvrent : une liste fermee l'est par son type (`ParLaListe`, pour toute `socle::liste::ListeFermee`),
/// un autre type par la table de ce module ou a cote de lui (`ParSaForme`) ; `Option` et `Vec` la tirent de leur valeur.
/// Le compilateur choisit la seule qui vaut : un type qui n'en a aucune ne se declare pas, et une liste fermee a qui l'on
/// donnerait aussi une forme a la main (un texte) en aurait deux, et ne se declare pas non plus.
pub trait ValeurDeLaCharge<Sorte> {
    const FORME: Forme;
    /// Faux : le champ peut manquer (`Option`), il est vide.
    const OBLIGATOIRE: bool = true;
}

/// La forme d'une liste fermee : son type la dit (`socle::liste::ListeFermee`).
pub enum ParLaListe {}
/// La forme d'un autre type, dite une fois pour lui.
pub enum ParSaForme {}
/// La forme d'une valeur facultative : celle de sa valeur.
pub struct Facultative<S>(PhantomData<S>);
/// La forme d'une liste de valeurs, tiree de celle de chacune.
pub struct EnListe<S>(PhantomData<S>);

impl<T: socle::liste::ListeFermee> ValeurDeLaCharge<ParLaListe> for T {
    const FORME: Forme = Forme::Choix(ListeDeChoix::de::<T>());
}

impl<S, T: ValeurDeLaCharge<S>> ValeurDeLaCharge<Facultative<S>> for Option<T> {
    const FORME: Forme = T::FORME;
    const OBLIGATOIRE: bool = false;
}

impl<S, T: ValeurDeLaCharge<S>> ValeurDeLaCharge<EnListe<S>> for Vec<T> {
    const FORME: Forme = en_liste(T::FORME);
}

/// La forme d'une liste dont chaque valeur a la forme `f` : des textes, des jours, des objets. Une autre n'a pas encore de
/// case qui la saisisse en liste : elle ne compile pas.
const fn en_liste(f: Forme) -> Forme {
    match f {
        Forme::Texte => Forme::Liste,
        Forme::Date => Forme::Jours,
        Forme::Objet => Forme::Objet,
        _ => panic!("une liste de cette forme n'a pas de case : la dire au développeur"),
    }
}

/// La forme de chaque type que des charges partagent, dite une fois. Un type propre a un module se dit a cote de lui.
macro_rules! formes {
    ($($forme:expr => [$($t:ty),* $(,)?]),* $(,)?) => {
        $($(
            impl ValeurDeLaCharge<ParSaForme> for $t {
                const FORME: Forme = $forme;
            }
        )*)*
    };
}

formes! {
    Forme::Texte => [
        String,
        std::path::PathBuf,
        socle::forme::TexteDit,
        socle::fichiers::NomDeFichier,
        socle::paris::HeureDuJour,
        droit::contexte::SalarieDesigne,
        droit::contexte::IdSalarie,
        droit::contexte::IdSpectacle,
        droit::fiche::Nic,
        droit::fiche::Nir,
        droit::fiche::CodePcsEse,
        droit::fiche::Courriel,
        droit::fiche::CaseDeFiche,
        documents::CategorieDeDocument,
        droit::emplois::CodeEmploi,
        droit::natures::IdentifiantDeDeclaration,
        engagement::numeros::NumeroContrat,
        engagement::carte::IdDeModele,
        envois::retours::IdDeFlux,
        compagnie::installation::IdDeCompagnie,
        compagnie::installation::CouleurDeCompagnie,
    ],
    Forme::Nombre => [
        usize,
        u32,
        u64,
        socle::mois::Annee,
        socle::grandeurs::Octets,
        socle::date::Duree<socle::date::EnJours>,
        droit::natures::NumeroDOrdre,
        engagement::numeros::NumeroDAvenant,
    ],
    Forme::Montant => [socle::grandeurs::Montant, socle::grandeurs::MontantUnitaire],
    Forme::OuiNon => [bool],
    Forme::Date => [socle::date::Jour],
    Forme::Mois => [socle::mois::Mois],
    Forme::MotDePasse => [socle::secret::Secret],
    // Une liste fermee de la norme, sans libelles dans son type : la route qui la fait choisir rend ses valeurs et
    // leurs libelles a cote (les nomenclatures de `imprevus`, la nature d'une carte).
    Forme::ChoixRendu => [
        droit::arrets::MotifDArret,
        droit::arrets::MotifDeReprise,
        droit::permanents::MotifDeSuspension,
        droit::natures::NatureDeContrat,
        droit::fiche::StatutConventionnel,
        declarations::obligations::LieuHorsDsn,
    ],
    Forme::Objet => [engagement::carte::Changements, compagnie::problemes::CaptureDeLaPage, socle::forme::Liste, droit::organismes::Saisies],
}

/// CE QUE LE TYPE D'UN CHAMP NE PEUT PAS SAVOIR : son libelle (ce que l'ecran montre), son sens (ce que le LLM lit), s'il
/// est « avance » (l'ecran le range sous Avancé). Un texte peut se preciser (`rendue`, `fichier`) ; aucune autre forme.
#[derive(Clone, Copy, Debug)]
pub struct Legende {
    libelle: Affiche,
    sens: &'static str,
    avance: bool,
    precise: Option<Forme>,
}

/// La legende d'un champ : son libelle et son sens.
pub const fn dit(libelle: &'static str, sens: &'static str) -> Legende {
    Legende { libelle: Affiche::new(libelle), sens, avance: false, precise: None }
}

impl Legende {
    /// Le champ est « avance ».
    pub const fn avancee(self) -> Legende {
        Legende { avance: true, ..self }
    }

    /// Un texte dont la route rend les valeurs a cote, sous la cle du champ (`Forme::ChoixRendu`).
    pub const fn rendue(self) -> Legende {
        Legende { precise: Some(Forme::ChoixRendu), ..self }
    }

    /// Un texte qui porte un fichier choisi sur le poste, de ces types de media (`Forme::Fichier`).
    pub const fn fichier(self, admis: &'static [&'static str]) -> Legende {
        Legende { precise: Some(Forme::Fichier(admis)), ..self }
    }
}

/// Les legendes que bien des charges partagent.
pub mod legendes {
    use super::{dit, Legende};

    /// Le mois de l'ecran.
    pub const MOIS: Legende = dit("Mois", "le mois de l'écran, AAAA-MM");
    /// Le salarie, tel que la personne le designe.
    pub const SALARIE: Legende = dit("Salarié", "la clé de la fiche du salarié (ou son nom)");
}

/// UN CHAMP D'UNE CHARGE DECLAREE : ce que la fiche en dit, et s'il est obligatoire, tires de son type.
#[derive(Clone, Copy, Debug)]
pub struct ChampDuType {
    pub champ: Champ,
    pub obligatoire: bool,
}

impl ChampDuType {
    /// Le champ `nom`, de type `T`, et sa legende : sa forme est celle du type (precisee si c'est un texte), il est
    /// obligatoire si le type n'est pas `Option`.
    pub const fn du_type<S, T: ValeurDeLaCharge<S>>(nom: &'static str, l: Legende) -> ChampDuType {
        let forme = match l.precise {
            None => T::FORME,
            Some(p) => match T::FORME {
                Forme::Texte => p,
                _ => panic!("seul un texte se précise (`rendue`, `fichier`) : les autres formes, leur type les dit"),
            },
        };
        ChampDuType { champ: Champ { nom, libelle: l.libelle, forme, sens: l.sens, avance: l.avance }, obligatoire: T::OBLIGATOIRE }
    }
}

/// LA CHARGE D'UNE ROUTE, telle que sa declaration la dit (`charge!`) : chaque champ dans l'ordre de la structure.
#[derive(Clone, Copy, Debug)]
pub struct ChargeDuType {
    pub champs: &'static [ChampDuType],
}

/// LA DECLARATION D'UNE CHARGE : la structure que la route lit, et sa charge au catalogue (`CHARGE`). Chaque champ s'ecrit
/// `nom: Type = legende`, ses attributs `serde` au-dessus de lui ; sa cle est son nom (jamais `#[serde(rename)]` ni
/// `flatten` : la fiche ne les verrait pas). La declaration ajoute elle-meme la lecture stricte (`Deserialize`,
/// `deny_unknown_fields`) : la structure ne les ecrit pas.
///
/// Une route qui prend le mot de passe tape le dit en tete (`motdepasse = LEGENDE;`) : l'enveloppe le lit et le retire
/// avant que la route lise sa structure (`crate::enveloppe::MOT_DE_PASSE`), qui ne le porte donc pas ; la charge le dit en
/// dernier, sa forme et son obligation tirees du type que l'enveloppe lit (`crate::enveloppe::MotDePasseTape`).
macro_rules! charge {
    (motdepasse = $m:expr; $($reste:tt)*) => {
        $crate::declaration::charge! { @[$crate::declaration::ChampDuType::du_type::<_, $crate::enveloppe::MotDePasseTape>($crate::enveloppe::MOT_DE_PASSE, $m),] $($reste)* }
    };
    (
        @[$($en_plus:expr,)*]
        $(#[$a:meta])*
        $v:vis struct $nom:ident {
            $( $(#[$fa:meta])* $fv:vis $f:ident : $t:ty = $l:expr ),* $(,)?
        }
    ) => {
        $(#[$a])*
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        $v struct $nom {
            $( $(#[$fa])* $fv $f: $t, )*
        }

        impl $nom {
            /// Sa charge au catalogue : la fiche de la route la nomme (`charge: &Type::CHARGE`).
            $v const CHARGE: $crate::declaration::ChargeDuType = $crate::declaration::ChargeDuType {
                champs: &[ $( $crate::declaration::ChampDuType::du_type::<_, $t>(stringify!($f), $l), )* $($en_plus,)* ],
            };
        }
    };
    ($($reste:tt)*) => {
        $crate::declaration::charge! { @[] $($reste)* }
    };
}

pub(crate) use charge;

/// LE NOM D'UN CHAMP D'UNE CHARGE DECLAREE, que le compilateur tient : `cle!(ChargeChanger, nouveau)` ne compile que si la
/// structure a ce champ. Ce qu'un geste rendu nomme de ses cases.
macro_rules! cle {
    ($t:ty, $f:ident) => {{
        let _ = |c: &$t| {
            let _ = &c.$f;
        };
        stringify!($f)
    }};
}

pub(crate) use cle;
