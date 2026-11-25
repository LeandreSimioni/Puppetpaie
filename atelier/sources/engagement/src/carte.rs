//! LA CARTE VALIDE : ce qu'un engagement dit, une variante par nature de contrat (`logiciel/donnees/contrats-base.json`,
//! `natures` : `cddu_spectacle`, `cdi`, `cdd`). Chaque variante ne porte que ses cases : une case obligatoire y est une
//! valeur, une case facultative un `Option` (absent ou `null`, il est vide) ; ce
//! qui n'appartient pas a une nature n'existe pas dans sa variante. Le motif de recours d'un CDD est une enum, et seul
//! un motif de remplacement porte la personne remplacee (L. 1242-12, 1°).
//!
//! LE BROUILLON (`Brouillon`) est ce que la page enregistre tant que l'engagement n'est pas valide : une variante par
//! nature aussi, chacune de ses cases un `Option` ecrit `null` tant qu'elle n'est pas remplie. Il change par une liste de
//! changements de case, tout ou rien (`Changements`, routes `enregistrer-saisie` et `modifier-engagement`, et l'apercu
//! d'une modification pas encore ecrite), et s'ecrit entier. La carte qu'il ecrit (`saisie::Carte`) se lit en
//! carte valide (`CarteValide::depuis`) : il lui manque une case obligatoire, ou elle porte une case que sa nature n'a
//! pas, le refus nomme chacune et rien ne s'ecrit. Seule une carte valide entre dans les calculs (`crate::engagements`).
//!
//! Ce qui fonde l'obligation de chaque case : le salarie (R. 1221-34, 1°) ; le spectacle d'un CDDU (son numero d'objet,
//! S21.G00.40.050) ; ses lignes (les dates jouees, R. 1221-34, 12°, et la remuneration, L. 1242-12, 7°) ; le debut d'un
//! contrat paye au mois (R. 1221-34, 4°, S21.G00.40.001), son salaire (L. 1242-12, 7°) et ses heures par mois
//! (S21.G00.40.013) ; la fin d'un CDD (L. 1242-12, 2°, S21.G00.40.010 CCH-12) et son motif (S21.G00.40.021, SIG-11,
//! `cdd.champs."S21.G00.40.021".obligatoire`). Le reste est facultatif : aucun fait ne l'exige.

use serde::{Deserialize, Serialize};

use droit::contexte::{IdSalarie, IdSpectacle};
use droit::natures::{ModaliteDuTempsDeTravail, MotifDeRecours, MotifDeRupture, NatureDeContrat};
use socle::date::{Duree, EnJours, Jour};
use socle::grandeurs::{Coefficient, Heures, Jours, Montant, MontantUnitaire};
use socle::echec::{refus, R};

use crate::ligne::{LigneDonnee, LignePaie};
use crate::saisie::{Carte, Champ, CongePris, Groupe, Preavis, Prime, Valeur};

/// UN MODELE DE CONTRAT PROPRE A LA COMPAGNIE, qu'une carte choisit (`modele_contrat`) : `m` et un entier, l'id que la
/// compagnie lui donne a sa creation (`routes::modeles_contrat`, son seul ecrivain, par `de`), garde tel qu'ecrit ; aucun
/// choisi : le defaut de sa nature. Un autre texte se refuse a la lecture.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct IdDeModele {
    texte: String,
    numero: u32,
}

impl IdDeModele {
    /// Le numero de l'id `texte`, s'il a la forme d'un id (`m` et un entier).
    fn numero_de(texte: &str) -> Option<u32> {
        match texte.strip_prefix('m').map(str::parse::<u32>) {
            Some(Ok(numero)) => Some(numero),
            Some(Err(_)) | None => None,
        }
    }

    /// L'id `texte` qu'un fichier ecrit (les modeles, une carte), ou ce qui le refuse.
    pub fn lire(texte: &str) -> R<IdDeModele> {
        match IdDeModele::numero_de(texte) {
            Some(numero) => Ok(IdDeModele { texte: texte.to_string(), numero }),
            None => Err(refus(format!("Le modèle de contrat « {} » a un identifiant mal formé : le corriger dans le fichier des modèles.", texte))),
        }
    }

    /// L'id `texte` que nomme la charge d'une route (`id_de_la_charge`), ou ce qui le refuse : la phrase d'un modele
    /// qu'on ne connait pas, qui dit ou le choisir.
    pub fn de_la_charge(texte: &str) -> R<IdDeModele> {
        match IdDeModele::numero_de(texte) {
            Some(numero) => Ok(IdDeModele { texte: texte.to_string(), numero }),
            None => Err(IdDeModele::aucun(texte)),
        }
    }

    /// Le refus d'un modele `id` qu'on ne connait pas, qui dit ou le choisir : la seule phrase, que la charge et la
    /// recherche dans les modeles disent.
    pub fn aucun(id: impl std::fmt::Display) -> socle::echec::Echec {
        refus(format!("Aucun modèle de contrat {} : en choisir un de la liste (modeles-contrat).", id))
    }

    /// L'id du modele de numero `numero`.
    pub fn de(numero: u32) -> IdDeModele {
        IdDeModele { texte: format!("m{}", numero), numero }
    }

    /// Son numero.
    pub fn numero(&self) -> u32 {
        self.numero
    }

    /// L'id tel qu'ecrit.
    pub fn texte(&self) -> &str {
        &self.texte
    }
}

impl std::fmt::Display for IdDeModele {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.texte)
    }
}

impl TryFrom<String> for IdDeModele {
    type Error = socle::echec::Echec;
    fn try_from(s: String) -> R<IdDeModele> {
        IdDeModele::lire(&s)
    }
}

/// LA LECTURE D'UN ID DANS LA CHARGE D'UNE ROUTE (`#[serde(deserialize_with = "engagement::carte::id_de_la_charge::deserialize")]`) :
/// un id mal forme s'y refuse par la phrase d'une charge (`IdDeModele::de_la_charge`), pas par celle du fichier.
pub mod id_de_la_charge {
    use super::IdDeModele;
    use serde::Deserialize;

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<IdDeModele, D::Error> {
        IdDeModele::de_la_charge(&String::deserialize(d)?).map_err(|e| serde::de::Error::custom(e.phrase))
    }

    /// Un id facultatif (`#[serde(default, deserialize_with = "…::option::deserialize")]`) : absent ou nul, aucun.
    pub mod option {
        use super::IdDeModele;
        use serde::Deserialize;

        pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<IdDeModele>, D::Error> {
            match Option::<String>::deserialize(d)? {
                Some(t) => IdDeModele::de_la_charge(&t).map(Some).map_err(|e| serde::de::Error::custom(e.phrase)),
                None => Ok(None),
            }
        }
    }
}

impl From<IdDeModele> for String {
    fn from(i: IdDeModele) -> String {
        i.texte
    }
}

/// Ce qu'une case de la carte est pour sa nature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Exige {
    Obligatoire,
    Facultatif,
}

/// LE FAIT QUI OUVRE UNE CASE DE LA CARTE : la case n'existe pour un engagement que si ce fait est vrai pour lui. Le
/// moteur le sait (la route qui rend la carte le lit : `routes::mois`, les cases ouvertes de chaque paie) ; aucun ne
/// se devine de l'ecran.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FaitQuiOuvre {
    /// La compagnie a plus d'un etablissement (sinon le contrat est au siege).
    PlusieursEtablissements,
    /// Le contrat se fait signer ici, depuis son modele (`contrat.ecrits` de l'apercu appelle sa periode d'essai).
    ContratEcrit,
    /// La compagnie a des modeles de contrat au choix pour cette nature (`routes::modeles_contrat::au_choix`).
    ModelesAuChoix,
    /// Le contrat a commence avant le logiciel, et aucune DSN importee ne dit deja son numero (regle 4).
    NumeroADire,
    /// Le contrat finit ce mois-ci : sa fin, avant terme ou non, se dit ce mois (la porte « Imprévu… »).
    FinCeMois,
    /// Le contrat finit ce mois sur un motif autre que celui d'une fin a son terme (le motif par defaut de la nature).
    MotifAvantTerme,
    /// Le motif de la fin appelle ce fait (`droit::permanents::faits_de_rupture`).
    DuMotif(droit::permanents::FaitDeRupture),
    /// Le preavis de la fin n'est pas effectue (`declarations::fctu::preavis_non_effectue`) : le dernier jour paye
    /// precede la fin.
    PreavisNonEffectue,
    /// La carte est celle d'une paie importee d'un ancien logiciel (`Carte::importe`) : ce que son bulletin imprime s'y lit.
    Importee,
    /// Un contrat paye au mois a temps plein : ses heures supplementaires du mois (L. 3121-28).
    TempsPlein,
    /// Un contrat paye au mois a temps plein, sous une convention qui ne fixe pas les paliers des heures supplementaires :
    /// la loi les decompte par semaine (L. 3121-29 et L. 3121-36), celles qui passent la huitieme se disent a part
    /// (`engagement::heures_en_plus::decomptees_par_semaine`).
    HeuresDecompteesParSemaine,
    /// Un contrat paye au mois a temps partiel (modalite 20 de S21.G00.40.014) : ses heures complementaires (L. 3123-20) et
    /// la repartition de ses horaires (L. 3123-6, 1°).
    TempsPartiel,
}

/// QUAND UNE CASE EXISTE POUR UN ENGAGEMENT : toujours, ou si un fait est vrai pour lui.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Ouverture {
    Toujours,
    Si(FaitQuiOuvre),
}

/// Une case de toute carte de sa nature.
const TOUJOURS: Ouverture = Ouverture::Toujours;

use droit::permanents::FaitDeRupture as D;
use FaitQuiOuvre as F;

/// LA PLACE D'UNE CASE OUVERTE SUR LA CARTE : de base (sur la carte), sous « Avancé », derriere la porte « Imprévu… »
/// (la fin du contrat), ou rendue (le moteur la tire d'autre chose et la montre, elle ne se saisit pas).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaceDeLaCase {
    Base,
    Avance,
    Imprevu,
    Rendue,
}

/// UNE NATURE DE CONTRAT ET SES CASES, declarees une fois : la structure (chaque case obligatoire une valeur, chaque case
/// facultative un `Option` present), la liste de ses cases (`CASES`), et sa lecture depuis la carte du mois, case par
/// case de meme nom. Chaque case dit le fait qui l'ouvre et sa place (`si Ouverture => Place`, `OUVERTES`) : une case
/// qui ne le dit pas ne compile pas. Ce que porte un motif de recours (`du_motif`, `DU_MOTIF`) est facultatif, et le
/// motif dit s'il le porte (`Proprietes::motif_de_remplacement`). Une case faite de plusieurs champs de la carte (l'emploi)
/// les nomme chacun (`emploi: … = Emploi + PcsEse + …`), et tous ont son ouverture et sa place.
///
/// Elle declare aussi SON BROUILLON (`$brouillon`) : les memes cases, chacune un `Option` lu present (une case pas encore
/// remplie s'ecrit `null`).
macro_rules! nature {
    (
        $(#[$a:meta])*
        $nom:ident / $brouillon:ident {
            obligatoire { $( $o:ident : $to:ty = $co:ident $( ecrite($eo:literal, $eoo:literal) )? si $oo:expr => $po:ident ),* $(,)? }
            facultatif { $( $f:ident : $tf:ty = $cf:ident $( + $cfx:ident )* $( ecrite($ef:literal) )? $( lue($lf:literal) )? si $of:expr => $pf:ident ),* $(,)? }
            du_motif { $( $m:ident : $tm:ty = $cm:ident ),* $(,)? }
        }
    ) => {
        /// LE BROUILLON de cette nature : chacune de ses cases, remplie ou non. Une case qui dit son ecriture
        /// (`ecrite(..)` : une grandeur, une duree) s'ecrit par elle, et se lit par sa lecture quand elle la dit (`lue(..)`).
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $brouillon {
            $( $( #[serde(serialize_with = $eoo)] )? pub $o: Option<$to>, )*
            $( $( #[serde(serialize_with = $ef)] )? $( #[serde(default, deserialize_with = $lf)] )? pub $f: Option<$tf>, )*
            $( pub $m: Option<$tm>, )*
        }

        impl $brouillon {
            /// Ses cases, dans l'ordre de la structure.
            pub const CASES: &'static [Champ] = &[ $( Champ::$co, )* $( Champ::$cf, $( Champ::$cfx, )* )* $( Champ::$cm, )* ];

            /// Aucune case remplie.
            fn vide() -> $brouillon {
                $brouillon { $( $o: None, )* $( $f: None, )* $( $m: None, )* }
            }

            /// Ses cases, telles que la carte `c` les dit.
            fn de_la_carte(c: &Carte) -> $brouillon {
                $brouillon { $( $o: c.$o.clone(), )* $( $f: c.$f.clone(), )* $( $m: c.$m.clone(), )* }
            }

            /// Ses cases posees sur la carte `c`.
            fn sur_la_carte(&self, c: &mut Carte) {
                $( c.$o = self.$o.clone(); )*
                $( c.$f = self.$f.clone(); )*
                $( c.$m = self.$m.clone(); )*
            }
        }

        $(#[$a])*
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $nom {
            $( $( #[serde(serialize_with = $eo)] )? pub $o: $to, )*
            $( $( #[serde(serialize_with = $ef)] )? $( #[serde(default, deserialize_with = $lf)] )? pub $f: Option<$tf>, )*
            $( pub $m: Option<$tm>, )*
        }

        impl $nom {
            /// Ses cases, dans l'ordre de la structure.
            pub const CASES: &'static [(Champ, Exige)] = &[ $( (Champ::$co, Exige::Obligatoire), )* $( (Champ::$cf, Exige::Facultatif), $( (Champ::$cfx, Exige::Facultatif), )* )* ];

            /// Le fait qui ouvre chacune de ses cases, et sa place, dans l'ordre de `CASES`.
            pub const OUVERTES: &'static [(Champ, Ouverture, PlaceDeLaCase)] = &[
                $( (Champ::$co, $oo, PlaceDeLaCase::$po), )*
                $( (Champ::$cf, $of, PlaceDeLaCase::$pf), $( (Champ::$cfx, $of, PlaceDeLaCase::$pf), )* )*
            ];

            /// Les cases qu'un motif de recours porte en plus des siennes.
            pub const DU_MOTIF: &'static [Champ] = &[ $( Champ::$cm, )* ];

            /// Ce que la carte `c` dit de cette nature ; `None` s'il lui manque une case obligatoire (`manques` les nomme).
            fn tiree(c: &Carte) -> Option<$nom> {
                Some($nom {
                    $( $o: c.$o.clone()?, )*
                    $( $f: c.$f.clone(), )*
                    $( $m: c.$m.clone(), )*
                })
            }
        }
    };
}

nature! {
    /// UN CDDU DU SPECTACLE (L. 1242-2, 3°) : l'artiste ou le technicien engage pour un spectacle, paye a ses lignes
    /// (cachets, heures). Son debut, sa fin et ses jours travailles se tirent de ses jours coches (les jours coches sont
    /// les jours travailles) : la carte les rend, elle ne les fait pas saisir ; un mois du contrat sans cachet
    /// (`lignes: []`) et un contrat repris d'un ancien logiciel les ecrivent. Le metier est toujours sur la carte (il est
    /// lie a l'engagement, pas a la personne : une carte qui nomme son salarie l'ecrit, celui de son dernier contrat ou de
    /// sa fiche, et il se change) ; la PCS-ESE, le code France Travail, les statuts et le cadre se lisent du metier
    /// (`droit::emplois::resoudre`) : la carte n'a pas de case « Cadre », un metier cadre se cree dans Configuration ›
    /// Métiers (`droit::emplois::MetierCree`) ; l'emploi ecrit ne vient que du bulletin d'une paie importee
    /// (`ecarts::refaire`). La fin d'un contrat qui finit ce mois passe par la porte « Imprévu… » : son motif, puis ce que
    /// le motif appelle. Ce que rien ne lit pour un CDDU n'est pas sur sa carte : ni lieu (celui des lignes le dit), ni
    /// taux du cachet (celui de sa ligne), ni primes (une ligne de rubrique `prime`), ni ce que seul le solde de tout
    /// compte d'un contrat paye au mois lit (indemnite de licenciement, preavis non effectue, precarite : L. 1243-10, 1°,
    /// la faute grave qui prive du delai de prevenance d'une fin d'essai : L. 1221-25), ni ce que seule la rupture
    /// conventionnelle appelle.
    Cddu / BrouillonCddu {
        obligatoire {
            salarie: IdSalarie = Salarie si TOUJOURS => Base,
            spectacle: IdSpectacle = Spectacle si TOUJOURS => Base,
            lignes: Vec<LignePaie> = Lignes si TOUJOURS => Base,
        }
        facultatif {
            debut: Jour = Debut si TOUJOURS => Rendue,
            fin: Jour = Fin si TOUJOURS => Rendue,
            numero: crate::numeros::NumeroContrat = Numero si Ouverture::Si(F::NumeroADire) => Base,
            etablissement: droit::fiche::Nic = Etablissement si Ouverture::Si(F::PlusieursEtablissements) => Base,
            metier: droit::emplois::CodeEmploi = Metier si TOUJOURS => Base,
            emploi: droit::emplois::EmploiEcrit = Emploi + PcsEse + CodeEmploiFt + Statut + StatutConventionnel + StatutCategorielRc lue("droit::emplois::EmploiEcrit::dit") si Ouverture::Si(F::Importee) => Rendue,
            periode_essai_jours: Duree<EnJours> = PeriodeEssaiJours ecrite("socle::date::Duree::ecrite_en_jours_opt") lue("socle::date::Duree::en_jours_opt") si Ouverture::Si(F::ContratEcrit) => Avance,
            clauses_particulieres: String = ClausesParticulieres si Ouverture::Si(F::ContratEcrit) => Avance,
            modele_contrat: IdDeModele = ModeleContrat si Ouverture::Si(F::ModelesAuChoix) => Base,
            date_paiement: Jour = DatePaiement si TOUJOURS => Avance,
            jours: Jours = Jours ecrite("socle::grandeurs::ecrit::nombre_json::option::serialize") si TOUJOURS => Rendue,
            motif_rupture: MotifDeRupture = MotifRupture si Ouverture::Si(F::FinCeMois) => Imprevu,
            notification_rupture: Jour = NotificationRupture si Ouverture::Si(F::DuMotif(D::NotificationRupture)) => Imprevu,
            engagement_licenciement: Jour = EngagementLicenciement si Ouverture::Si(F::DuMotif(D::EngagementLicenciement)) => Imprevu,
            preavis: Preavis = Preavis si Ouverture::Si(F::MotifAvantTerme) => Imprevu,
            transaction: droit::preavis::TransactionEnCours = Transaction si Ouverture::Si(F::DuMotif(D::Transaction)) => Imprevu,
            dernier_jour_paye: Jour = DernierJourPaye si Ouverture::Si(F::PreavisNonEffectue) => Imprevu,
        }
        du_motif {}
    }
}

nature! {
    /// UN CDI, paye au mois : son debut, son salaire et ses heures par mois. Sa fin est sa rupture.
    Cdi / BrouillonCdi {
        obligatoire {
            salarie: IdSalarie = Salarie si TOUJOURS => Base,
            debut: Jour = Debut si TOUJOURS => Base,
            salaire_mensuel: MontantUnitaire = SalaireMensuel ecrite("socle::grandeurs::ecrit::nombre_json::serialize", "socle::grandeurs::ecrit::nombre_json::option::serialize") si TOUJOURS => Base,
            quotite_contrat: Heures = QuotiteContrat ecrite("socle::grandeurs::ecrit::nombre_json::serialize", "socle::grandeurs::ecrit::nombre_json::option::serialize") si TOUJOURS => Base,
        }
        facultatif {
            fin: Jour = Fin si TOUJOURS => Avance,
            numero: crate::numeros::NumeroContrat = Numero si TOUJOURS => Avance,
            etablissement: droit::fiche::Nic = Etablissement si TOUJOURS => Avance,
            metier: droit::emplois::CodeEmploi = Metier si TOUJOURS => Avance,
            lieu: String = Lieu si TOUJOURS => Avance,
            quotite_reference: Heures = QuotiteReference ecrite("socle::grandeurs::ecrit::nombre_json::option::serialize") si TOUJOURS => Avance,
            modalite_temps_travail: ModaliteDuTempsDeTravail = ModaliteTempsTravail si TOUJOURS => Avance,
            emploi: droit::emplois::EmploiEcrit = Emploi + PcsEse + CodeEmploiFt + Statut + StatutConventionnel + StatutCategorielRc lue("droit::emplois::EmploiEcrit::dit") si TOUJOURS => Avance,
            periode_essai_jours: Duree<EnJours> = PeriodeEssaiJours ecrite("socle::date::Duree::ecrite_en_jours_opt") lue("socle::date::Duree::en_jours_opt") si TOUJOURS => Avance,
            repartition_horaire: String = RepartitionHoraire si Ouverture::Si(F::TempsPartiel) => Avance,
            clauses_particulieres: String = ClausesParticulieres si TOUJOURS => Avance,
            modele_contrat: IdDeModele = ModeleContrat si TOUJOURS => Avance,
            lignes: Vec<LignePaie> = Lignes si TOUJOURS => Avance,
            conges_pris: Vec<CongePris> = CongesPris si TOUJOURS => Avance,
            primes: Vec<Prime> = Primes si TOUJOURS => Avance,
            date_paiement: Jour = DatePaiement si TOUJOURS => Avance,
            jours: Jours = Jours ecrite("socle::grandeurs::ecrit::nombre_json::option::serialize") si TOUJOURS => Avance,
            heures_supplementaires: Heures = HeuresSupplementaires ecrite("socle::grandeurs::ecrit::nombre_json::option::serialize") si Ouverture::Si(F::TempsPlein) => Avance,
            heures_supplementaires_a_50: Heures = HeuresSupplementairesA50 ecrite("socle::grandeurs::ecrit::nombre_json::option::serialize") si Ouverture::Si(F::HeuresDecompteesParSemaine) => Avance,
            heures_complementaires: Heures = HeuresComplementaires ecrite("socle::grandeurs::ecrit::nombre_json::option::serialize") si Ouverture::Si(F::TempsPartiel) => Avance,
            indemnite_licenciement: Montant = IndemniteLicenciement ecrite("socle::grandeurs::ecrit::nombre_json::option::serialize") si TOUJOURS => Avance,
            faute_grave: bool = FauteGrave si TOUJOURS => Avance,
            motif_rupture: MotifDeRupture = MotifRupture si Ouverture::Si(F::FinCeMois) => Imprevu,
            notification_rupture: Jour = NotificationRupture si Ouverture::Si(F::DuMotif(D::NotificationRupture)) => Imprevu,
            engagement_licenciement: Jour = EngagementLicenciement si Ouverture::Si(F::DuMotif(D::EngagementLicenciement)) => Imprevu,
            signature_convention: Jour = SignatureConvention si Ouverture::Si(F::DuMotif(D::SignatureConvention)) => Imprevu,
            preavis: Preavis = Preavis si Ouverture::Si(F::MotifAvantTerme) => Imprevu,
            transaction: droit::preavis::TransactionEnCours = Transaction si Ouverture::Si(F::DuMotif(D::Transaction)) => Imprevu,
            dernier_jour_paye: Jour = DernierJourPaye si Ouverture::Si(F::PreavisNonEffectue) => Imprevu,
            en_droit_a_la_retraite: bool = EnDroitALaRetraite si Ouverture::Si(F::DuMotif(D::EnDroitALaRetraite)) => Imprevu,
            preavis_non_effectue: bool = PreavisNonEffectue si Ouverture::Si(F::DuMotif(D::PreavisNonEffectue)) => Imprevu,
            preavis_mois: Coefficient = PreavisMois ecrite("socle::grandeurs::ecrit::nombre_json::option::serialize") si Ouverture::Si(F::DuMotif(D::PreavisNonEffectue)) => Imprevu,
            precarite_due: bool = PrecariteDue si Ouverture::Si(F::DuMotif(D::PrecariteDue)) => Imprevu,
            precarite_pourquoi: String = PrecaritePourquoi si Ouverture::Si(F::DuMotif(D::PrecariteDue)) => Imprevu,
        }
        du_motif {}
    }
}

nature! {
    /// UN CDD ORDINAIRE (L. 1242-2, hors 3°), paye au mois : son debut et sa fin, son motif, son salaire et ses heures
    /// par mois.
    Cdd / BrouillonCdd {
        obligatoire {
            salarie: IdSalarie = Salarie si TOUJOURS => Base,
            debut: Jour = Debut si TOUJOURS => Base,
            fin: Jour = Fin si TOUJOURS => Base,
            motif_recours: MotifDeRecours = MotifRecours si TOUJOURS => Base,
            salaire_mensuel: MontantUnitaire = SalaireMensuel ecrite("socle::grandeurs::ecrit::nombre_json::serialize", "socle::grandeurs::ecrit::nombre_json::option::serialize") si TOUJOURS => Base,
            quotite_contrat: Heures = QuotiteContrat ecrite("socle::grandeurs::ecrit::nombre_json::serialize", "socle::grandeurs::ecrit::nombre_json::option::serialize") si TOUJOURS => Base,
        }
        facultatif {
            numero: crate::numeros::NumeroContrat = Numero si TOUJOURS => Avance,
            etablissement: droit::fiche::Nic = Etablissement si TOUJOURS => Avance,
            metier: droit::emplois::CodeEmploi = Metier si TOUJOURS => Avance,
            lieu: String = Lieu si TOUJOURS => Avance,
            quotite_reference: Heures = QuotiteReference ecrite("socle::grandeurs::ecrit::nombre_json::option::serialize") si TOUJOURS => Avance,
            modalite_temps_travail: ModaliteDuTempsDeTravail = ModaliteTempsTravail si TOUJOURS => Avance,
            emploi: droit::emplois::EmploiEcrit = Emploi + PcsEse + CodeEmploiFt + Statut + StatutConventionnel + StatutCategorielRc lue("droit::emplois::EmploiEcrit::dit") si TOUJOURS => Avance,
            periode_essai_jours: Duree<EnJours> = PeriodeEssaiJours ecrite("socle::date::Duree::ecrite_en_jours_opt") lue("socle::date::Duree::en_jours_opt") si TOUJOURS => Avance,
            repartition_horaire: String = RepartitionHoraire si Ouverture::Si(F::TempsPartiel) => Avance,
            clauses_particulieres: String = ClausesParticulieres si TOUJOURS => Avance,
            modele_contrat: IdDeModele = ModeleContrat si TOUJOURS => Avance,
            lignes: Vec<LignePaie> = Lignes si TOUJOURS => Avance,
            conges_pris: Vec<CongePris> = CongesPris si TOUJOURS => Avance,
            primes: Vec<Prime> = Primes si TOUJOURS => Avance,
            date_paiement: Jour = DatePaiement si TOUJOURS => Avance,
            jours: Jours = Jours ecrite("socle::grandeurs::ecrit::nombre_json::option::serialize") si TOUJOURS => Avance,
            heures_supplementaires: Heures = HeuresSupplementaires ecrite("socle::grandeurs::ecrit::nombre_json::option::serialize") si Ouverture::Si(F::TempsPlein) => Avance,
            heures_supplementaires_a_50: Heures = HeuresSupplementairesA50 ecrite("socle::grandeurs::ecrit::nombre_json::option::serialize") si Ouverture::Si(F::HeuresDecompteesParSemaine) => Avance,
            heures_complementaires: Heures = HeuresComplementaires ecrite("socle::grandeurs::ecrit::nombre_json::option::serialize") si Ouverture::Si(F::TempsPartiel) => Avance,
            indemnite_licenciement: Montant = IndemniteLicenciement ecrite("socle::grandeurs::ecrit::nombre_json::option::serialize") si TOUJOURS => Avance,
            faute_grave: bool = FauteGrave si TOUJOURS => Avance,
            motif_rupture: MotifDeRupture = MotifRupture si Ouverture::Si(F::FinCeMois) => Imprevu,
            notification_rupture: Jour = NotificationRupture si Ouverture::Si(F::DuMotif(D::NotificationRupture)) => Imprevu,
            engagement_licenciement: Jour = EngagementLicenciement si Ouverture::Si(F::DuMotif(D::EngagementLicenciement)) => Imprevu,
            signature_convention: Jour = SignatureConvention si Ouverture::Si(F::DuMotif(D::SignatureConvention)) => Imprevu,
            preavis: Preavis = Preavis si Ouverture::Si(F::MotifAvantTerme) => Imprevu,
            transaction: droit::preavis::TransactionEnCours = Transaction si Ouverture::Si(F::DuMotif(D::Transaction)) => Imprevu,
            dernier_jour_paye: Jour = DernierJourPaye si Ouverture::Si(F::PreavisNonEffectue) => Imprevu,
            en_droit_a_la_retraite: bool = EnDroitALaRetraite si Ouverture::Si(F::DuMotif(D::EnDroitALaRetraite)) => Imprevu,
            preavis_non_effectue: bool = PreavisNonEffectue si Ouverture::Si(F::DuMotif(D::PreavisNonEffectue)) => Imprevu,
            preavis_mois: Coefficient = PreavisMois ecrite("socle::grandeurs::ecrit::nombre_json::option::serialize") si Ouverture::Si(F::DuMotif(D::PreavisNonEffectue)) => Imprevu,
            precarite_due: bool = PrecariteDue si Ouverture::Si(F::DuMotif(D::PrecariteDue)) => Imprevu,
            precarite_pourquoi: String = PrecaritePourquoi si Ouverture::Si(F::DuMotif(D::PrecariteDue)) => Imprevu,
        }
        du_motif {
            personne_remplacee: String = PersonneRemplacee,
        }
    }
}

/// LA CARTE VALIDE : une variante par nature de contrat, du nom de la sienne (`NatureDeContrat`), la nature dite
/// (`nature`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "nature", rename_all = "snake_case", deny_unknown_fields)]
pub enum CarteValide {
    CdduSpectacle(Cddu),
    Cdi(Cdi),
    Cdd(Cdd),
}

/// UNE NATURE DECRITE pour l'appelant (`natures` de la fiche des routes qui ecrivent une carte, et de la route etat) :
/// sa cle, chacune de ses cases (obligatoire ou facultative), et, pour un CDD, ses motifs et ce que chacun porte en plus.
/// `F` est la forme d'une case (`routes::catalogue::Forme`, au-dessus : celui qui decrit la nomme).
#[derive(Debug, Clone, Serialize)]
pub struct NatureDecrite<F> {
    pub nature: NatureDeContrat,
    pub cases: Vec<CaseDecrite<F>>,
    pub motifs_recours: Vec<MotifDecrit<F>>,
}

/// Une case d'une nature : sa cle sur la carte, son libelle, ce qu'elle est, sa forme (ce que l'ecran en saisit : une
/// date, un nombre… ; `routes::catalogue::Forme::de_la_case`), le fait qui l'ouvre et sa place une fois ouverte.
#[derive(Debug, Clone, Serialize)]
pub struct CaseDecrite<F> {
    pub cle: &'static str,
    pub libelle: &'static str,
    pub exige: Exige,
    pub forme: F,
    pub ouverte: Ouverture,
    pub place: PlaceDeLaCase,
}

/// UNE CASE OUVERTE POUR UN ENGAGEMENT (`CarteValide::cases_ouvertes`) : sa cle et sa place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CaseOuverte {
    pub cle: Champ,
    pub place: PlaceDeLaCase,
}

/// Un motif de recours : son code, et les cases qu'il porte en plus de celles de sa nature.
#[derive(Debug, Clone, Serialize)]
pub struct MotifDecrit<F> {
    pub code: MotifDeRecours,
    pub cases: Vec<CaseDecrite<F>>,
}

impl CarteValide {
    /// Les cases de la nature `n`.
    pub fn cases_de(n: NatureDeContrat) -> &'static [(Champ, Exige)] {
        match n {
            NatureDeContrat::CdduSpectacle => Cddu::CASES,
            NatureDeContrat::Cdi => Cdi::CASES,
            NatureDeContrat::Cdd => Cdd::CASES,
        }
    }

    /// Le fait qui ouvre chaque case de la nature `n`, et sa place.
    pub fn ouvertures_de(n: NatureDeContrat) -> &'static [(Champ, Ouverture, PlaceDeLaCase)] {
        match n {
            NatureDeContrat::CdduSpectacle => Cddu::OUVERTES,
            NatureDeContrat::Cdi => Cdi::OUVERTES,
            NatureDeContrat::Cdd => Cdd::OUVERTES,
        }
    }

    /// LES CASES OUVERTES D'UNE CARTE DE NATURE `n`, chacune sa place, dans l'ordre de la nature : celles de toute carte,
    /// et celles dont le fait est vrai pour elle (`vrai`, que la route qui rend la carte lit du moteur). Une case qu'aucun
    /// fait n'ouvre n'est pas rendue.
    pub fn cases_ouvertes(n: NatureDeContrat, vrai: &dyn Fn(FaitQuiOuvre) -> R<bool>) -> R<Vec<CaseOuverte>> {
        let mut out = Vec::new();
        for (c, o, place) in CarteValide::ouvertures_de(n) {
            let ouverte = match o {
                Ouverture::Toujours => true,
                Ouverture::Si(f) => vrai(*f)?,
            };
            if ouverte {
                out.push(CaseOuverte { cle: *c, place: *place });
            }
        }
        Ok(out)
    }

    /// Les cases que toute carte de nature `n` ouvre, sans aucun fait (`Ouverture::Toujours`).
    pub fn cases_toujours_ouvertes(n: NatureDeContrat) -> Vec<CaseOuverte> {
        CarteValide::ouvertures_de(n).iter().filter(|(_, o, _)| *o == Ouverture::Toujours).map(|(c, _, place)| CaseOuverte { cle: *c, place: *place }).collect()
    }

    /// Les cases qu'un motif de recours de la nature `n` porte en plus des siennes.
    fn du_motif_de(n: NatureDeContrat) -> &'static [Champ] {
        match n {
            NatureDeContrat::CdduSpectacle => Cddu::DU_MOTIF,
            NatureDeContrat::Cdi => Cdi::DU_MOTIF,
            NatureDeContrat::Cdd => Cdd::DU_MOTIF,
        }
    }

    /// Chaque nature et ses cases, tirees du type, chacune sa forme (`forme`) ; ses motifs de recours, et ce que chacun
    /// porte, de `contrats-base.json`.
    pub fn natures<F>(forme: fn(Champ) -> F) -> R<Vec<NatureDecrite<F>>> {
        // `OUVERTES` suit l'ordre de `CASES`, case par case (`nature!`).
        let decrire = |n: NatureDeContrat| -> Vec<CaseDecrite<F>> {
            CarteValide::cases_de(n)
                .iter()
                .zip(CarteValide::ouvertures_de(n))
                .map(|((c, e), (_, o, p))| CaseDecrite { cle: c.nom(), libelle: libelle(*c), exige: *e, forme: forme(*c), ouverte: *o, place: *p })
                .collect()
        };
        let mut out = Vec::new();
        for n in NatureDeContrat::TOUTES {
            let nat = n.proprietes()?;
            let motifs_recours = nat
                .motifs_recours()
                .into_iter()
                .flatten()
                .map(|m| MotifDecrit {
                    code: m.code,
                    cases: if nat.motif_de_remplacement(m.code) {
                        // Ce que porte un motif de recours s'ouvre avec lui : il est de base sur la carte qui le choisit.
                        CarteValide::du_motif_de(n)
                            .iter()
                            .map(|c| CaseDecrite { cle: c.nom(), libelle: libelle(*c), exige: Exige::Facultatif, forme: forme(*c), ouverte: TOUJOURS, place: PlaceDeLaCase::Base })
                            .collect()
                    } else {
                        Vec::new()
                    },
                })
                .collect();
            out.push(NatureDecrite { nature: n, cases: decrire(n), motifs_recours });
        }
        Ok(out)
    }

    /// LA CARTE DU MOIS `c`, VALIDE : sa nature dite, chaque case obligatoire remplie, aucune case que sa nature n'a pas.
    /// Le refus nomme chaque case a remplir (`A_REMPLIR`), ou chaque case a retirer ; `qui` : le salarie qu'il nomme. Ce
    /// que la vue deduit (`Groupe::Deduit`) et ce qu'elle dit de l'engagement ecrit (`Groupe::Vue`) ne sont pas des cases.
    pub fn depuis(c: &Carte, qui: &str) -> R<CarteValide> {
        let Some(nature) = c.nature else {
            return Err(crate::a_finir(format!("{}{}", qui, crate::PAS_DE_NATURE)));
        };
        let cases = CarteValide::cases_de(nature);
        let nat = nature.proprietes()?;
        let libelle_nature = nat.libelle.clone();
        // Un motif que la nature n'offre pas se refuse en disant ceux qu'elle offre (`contrats-base.json`).
        if let Some(phrase) = c.motif_recours.and_then(|m| nat.motif_recours_refuse(m, qui)) {
            return Err(refus(phrase));
        }
        // Ce que porte un motif de recours est une case de sa nature (son motif dit ensuite s'il la porte).
        let du_motif = CarteValide::du_motif_de(nature);
        let a_elle = |x: Champ| x == Champ::Nature || du_motif.contains(&x) || cases.iter().any(|(k, _)| *k == x);
        let de_trop: Vec<&str> = Champ::TOUS
            .iter()
            .copied()
            .filter(|x| !matches!(x.groupe(), Groupe::Deduit | Groupe::Vue) && c.dit(*x) && !a_elle(*x))
            .map(libelle)
            .collect();
        if !de_trop.is_empty() {
            return Err(socle::refus!(
                "{} : {} {} pas une case d'un {} ; retirez-{} de sa carte. Rien n'est écrit.",
                qui,
                guillemets(&de_trop),
                if de_trop.len() > 1 { "ne sont" } else { "n'est" },
                libelle_nature,
                if de_trop.len() > 1 { "les" } else { "la" }
            ));
        }
        let manques: Vec<&str> = cases.iter().filter(|(x, e)| *e == Exige::Obligatoire && !remplie(c, *x)).map(|(x, _)| libelle(*x)).collect();
        if !manques.is_empty() {
            return Err(crate::a_finir(format!("{}{}{} ({}).", qui, A_REMPLIR, guillemets(&manques), libelle_nature)));
        }
        let manquante = || crate::a_finir(format!("{}{}({})", qui, A_REMPLIR, libelle_nature));
        Ok(match nature {
            NatureDeContrat::CdduSpectacle => CarteValide::CdduSpectacle(Cddu::tiree(c).ok_or_else(manquante)?),
            NatureDeContrat::Cdi => CarteValide::Cdi(Cdi::tiree(c).ok_or_else(manquante)?),
            NatureDeContrat::Cdd => {
                let v = Cdd::tiree(c).ok_or_else(manquante)?;
                if v.personne_remplacee.is_some() && !nat.motif_de_remplacement(v.motif_recours) {
                    return Err(socle::refus!(
                        "{} : {} n'est pas une case d'un CDD conclu pour ce motif ({}) ; retirez-la de sa carte. Rien n'est écrit.",
                        qui,
                        guillemets(&[libelle(Champ::PersonneRemplacee)]),
                        v.motif_recours
                    ));
                }
                CarteValide::Cdd(v)
            }
        })
    }

    /// Sa nature.
    pub fn nature(&self) -> NatureDeContrat {
        match self {
            CarteValide::CdduSpectacle(_) => NatureDeContrat::CdduSpectacle,
            CarteValide::Cdi(_) => NatureDeContrat::Cdi,
            CarteValide::Cdd(_) => NatureDeContrat::Cdd,
        }
    }
}

/// LE BROUILLON D'UNE CARTE : ce que la page enregistre tant que l'engagement n'est pas valide, peut-etre incomplet. Il
/// nait avec sa nature (une carte neuve s'ouvre dans celle que dit la route saisie du mois, `nouvelle_carte`) et n'existe jamais
/// sans elle ; chacune des cases de sa nature y est, remplie ou `null`. Il change d'une case a la fois (`Changement`) ;
/// changer de nature le fait changer de variante. Valider, c'est le lire en carte valide (`CarteValide::depuis`). Comme
/// la carte valide, une variante par nature, du nom de la sienne.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "nature", rename_all = "snake_case", deny_unknown_fields)]
pub enum Brouillon {
    CdduSpectacle(BrouillonCddu),
    Cdi(BrouillonCdi),
    Cdd(BrouillonCdd),
}

impl Brouillon {
    /// Un brouillon neuf de la nature `n` : aucune case remplie.
    pub fn neuf(n: NatureDeContrat) -> Brouillon {
        match n {
            NatureDeContrat::CdduSpectacle => Brouillon::CdduSpectacle(BrouillonCddu::vide()),
            NatureDeContrat::Cdi => Brouillon::Cdi(BrouillonCdi::vide()),
            NatureDeContrat::Cdd => Brouillon::Cdd(BrouillonCdd::vide()),
        }
    }

    /// Sa nature.
    pub fn nature(&self) -> NatureDeContrat {
        match self {
            Brouillon::CdduSpectacle(_) => NatureDeContrat::CdduSpectacle,
            Brouillon::Cdi(_) => NatureDeContrat::Cdi,
            Brouillon::Cdd(_) => NatureDeContrat::Cdd,
        }
    }

    /// Les cases de sa nature (et, pour un CDD, ce que porte un motif).
    pub fn cases(n: NatureDeContrat) -> &'static [Champ] {
        match n {
            NatureDeContrat::CdduSpectacle => BrouillonCddu::CASES,
            NatureDeContrat::Cdi => BrouillonCdi::CASES,
            NatureDeContrat::Cdd => BrouillonCdd::CASES,
        }
    }

    /// LE BROUILLON QUE LA CARTE `c` ECRIT (la vue d'un mois d'un engagement en saisie, `stock::Objet::vue`) : sa nature
    /// dite, et aucune case que sa nature n'a pas (le refus les nomme). Ce que la vue deduit (`Groupe::Deduit`) et ce
    /// qu'elle dit de l'engagement ecrit (`Groupe::Vue`) ne sont pas des cases. `qui` : le salarie que le refus nomme.
    pub fn de_la_carte(c: &Carte, qui: &str) -> R<Brouillon> {
        let Some(n) = c.nature else {
            return Err(crate::a_finir(format!("{}{}", qui, crate::PAS_DE_NATURE)));
        };
        let cases = Brouillon::cases(n);
        let de_trop: Vec<&str> = Champ::TOUS
            .iter()
            .copied()
            .filter(|x| !matches!(x.groupe(), Groupe::Deduit | Groupe::Vue) && *x != Champ::Nature && c.dit(*x) && !cases.contains(x))
            .map(libelle)
            .collect();
        if !de_trop.is_empty() {
            return Err(socle::refus!(
                "{} : {} {} pas une case d'un {}. Rien n'est écrit.",
                qui,
                guillemets(&de_trop),
                if de_trop.len() > 1 { "ne sont" } else { "n'est" },
                n.proprietes()?.libelle
            ));
        }
        Ok(match n {
            NatureDeContrat::CdduSpectacle => Brouillon::CdduSpectacle(BrouillonCddu::de_la_carte(c)),
            NatureDeContrat::Cdi => Brouillon::Cdi(BrouillonCdi::de_la_carte(c)),
            NatureDeContrat::Cdd => Brouillon::Cdd(BrouillonCdd::de_la_carte(c)),
        })
    }

    /// La carte qu'il ecrit : sa nature et ses cases remplies.
    pub fn en_carte(&self) -> Carte {
        let mut c = Carte { nature: Some(self.nature()), ..Carte::default() };
        match self {
            Brouillon::CdduSpectacle(b) => b.sur_la_carte(&mut c),
            Brouillon::Cdi(b) => b.sur_la_carte(&mut c),
            Brouillon::Cdd(b) => b.sur_la_carte(&mut c),
        }
        c
    }

    /// LE CHANGEMENT D'UNE CASE `ch`, applique. Changer de nature change le brouillon de variante : les cases propres a
    /// l'ancienne disparaissent, celles propres a la nouvelle naissent vides, et une case des deux (le salarie, le metier,
    /// les bornes, les lignes...) garde ce qu'elle dit : c'est la meme case, et l'ecran la montre telle quelle. Refus :
    /// une case que sa nature n'a pas, un motif de recours que sa nature n'offre pas ; rien ne change.
    pub fn changer(&mut self, ch: Changement, qui: &str) -> R<()> {
        let mut c = self.en_carte();
        if let Changement::Nature(n) = ch {
            let mut neuve = Carte { nature: Some(n), ..Carte::default() };
            for x in Brouillon::cases(n) {
                neuve.prendre(&c, *x);
            }
            *self = Brouillon::de_la_carte(&neuve, qui)?;
            return Ok(());
        }
        let x = ch.champ();
        if !Brouillon::cases(self.nature()).contains(&x) {
            let nat = self.nature().proprietes()?;
            return Err(socle::refus!("{} : {} n'est pas une case d'un {}. Rien n'est écrit.", qui, guillemets(&[libelle(x)]), nat.libelle));
        }
        if let Changement::MotifRecours(Some(m)) = &ch {
            let nat = self.nature().proprietes()?;
            if let Some(phrase) = nat.motif_recours_refuse(*m, qui) {
                return Err(refus(phrase));
            }
        }
        ch.poser(&mut c);
        *self = Brouillon::de_la_carte(&c, qui)?;
        Ok(())
    }
}

impl Brouillon {
    /// LA CARTE PROPOSEE A UN AUTRE SALARIE, copiee de ce brouillon (Leandre, 24/11/2026 : « copier l'engagement pour
    /// proposer à un autre salarié, c'est hyper courant ») : les changements qui la font d'une carte neuve de sa nature
    /// (`Changement::qui_font`), chaque case qui se copie (`se_copie`) telle qu'elle est ; les autres restent vides, le
    /// salarie le premier. Ce sont ceux qu'`enregistrer-saisie` ecrit, par le chemin d'une carte neuve. Les lignes se
    /// copient entieres, frais compris (Leandre, 24/11/2026 : « copie c'est copie, c'est plus facile de supprimer ») :
    /// la carte neuve les montre, et la personne retire ce qui n'est pas vrai du nouveau salarie.
    pub fn proposee_a_un_autre_salarie(&self) -> Changements {
        let n = self.nature();
        let de = self.en_carte();
        let mut c = Carte { nature: Some(n), ..Carte::default() };
        for x in Brouillon::cases(n).iter().copied().filter(|x| se_copie(n, *x)) {
            c.prendre(&de, x);
        }
        Changements(Changement::qui_font(&c))
    }
}

/// UNE CASE D'UNE CARTE DE NATURE `n` QUI SE COPIE POUR UN AUTRE SALARIE (`Brouillon::proposee_a_un_autre_salarie`) :
/// ce que l'engagement fait faire (le spectacle, les lignes et leurs dates, le metier, les termes du contrat, la date du
/// paiement). Ne se copie pas ce qui appartient au salarie ou a un acte : le salarie ; le numero (celui d'un contrat
/// deja declare) ; ce que la carte rend sans le faire saisir (`PlaceDeLaCase::Rendue` : les bornes, les jours et
/// l'emploi d'un CDDU, tires de ses lignes ou d'une piece) ; ce que le salarie a fait ou pris ce mois (ses conges, ses
/// primes, ses jours, ses heures en plus) ; la fin de son contrat et ce qu'elle appelle (le groupe `Fin`, l'indemnite de
/// licenciement, la faute grave) ; ce que la vue deduit ou dit de l'engagement ecrit.
fn se_copie(n: NatureDeContrat, x: Champ) -> bool {
    let rendue = CarteValide::ouvertures_de(n).iter().any(|(c, _, p)| *c == x && *p == PlaceDeLaCase::Rendue);
    !rendue
        && match x {
            Champ::Spectacle
            | Champ::Nature
            | Champ::Debut
            | Champ::Fin
            | Champ::Etablissement
            | Champ::Metier
            | Champ::Lieu
            | Champ::MotifRecours
            | Champ::QuotiteContrat
            | Champ::QuotiteReference
            | Champ::ModaliteTempsTravail
            | Champ::SalaireMensuel
            | Champ::Emploi
            | Champ::PcsEse
            | Champ::CodeEmploiFt
            | Champ::Statut
            | Champ::StatutConventionnel
            | Champ::StatutCategorielRc
            | Champ::PeriodeEssaiJours
            | Champ::PersonneRemplacee
            | Champ::RepartitionHoraire
            | Champ::ClausesParticulieres
            | Champ::ModeleContrat
            | Champ::Lignes
            | Champ::DatePaiement => true,
            Champ::Salarie
            | Champ::Numero
            | Champ::CongesPris
            | Champ::Primes
            | Champ::Jours
            | Champ::HeuresSupplementaires
            | Champ::HeuresSupplementairesA50
            | Champ::HeuresComplementaires
            | Champ::IndemniteLicenciement
            | Champ::FauteGrave
            | Champ::MotifRupture
            | Champ::NotificationRupture
            | Champ::EngagementLicenciement
            | Champ::SignatureConvention
            | Champ::Preavis
            | Champ::Transaction
            | Champ::DernierJourPaye
            | Champ::EnDroitALaRetraite
            | Champ::PreavisNonEffectue
            | Champ::PreavisMois
            | Champ::PrecariteDue
            | Champ::PrecaritePourquoi
            | Champ::JoursContrat
            | Champ::DateModification
            | Champ::Avant
            | Champ::Valide
            | Champ::Importe => false,
        }
}

impl Brouillon {
    /// Le salarie d'une carte de CDDU qui ne dit pas son metier : celle dont le metier se pre-remplit
    /// (`preremplir_le_metier`).
    pub fn salarie_sans_metier(&self) -> Option<&IdSalarie> {
        match self {
            Brouillon::CdduSpectacle(b) if b.metier.is_none() => b.salarie.as_ref(),
            _ => None,
        }
    }

    /// LE METIER D'UNE CARTE DE CDDU QUI VIENT DE NOMMER SON SALARIE, PRE-REMPLI ET ECRIT AVEC ELLE : `metier`, celui que
    /// la route a trouve pour ce salarie (celui de son dernier contrat, `metier_du_dernier_contrat`, sinon celui de sa
    /// fiche), quand la carte n'en dit pas. Le metier est lie a l'engagement, pas a la personne : il se change sur la
    /// carte. Un pre-remplissage ne refuse rien : sans metier trouve, la carte reste sans metier.
    pub fn preremplir_le_metier(&mut self, metier: Option<droit::emplois::CodeEmploi>) {
        if let Brouillon::CdduSpectacle(b) = self {
            if b.salarie.is_some() && b.metier.is_none() {
                b.metier = metier;
            }
        }
    }
}

/// LE METIER DU DERNIER CONTRAT DU SALARIE `s`, d'ici ou repris d'un ancien logiciel (l'historique du salarie est d'un
/// seul tenant) : le contrat actif de debut le plus tard (`sauf` mis a part) ; a debut egal, celui du fichier du mois le
/// plus tard, puis du rang le plus haut dans ce fichier (le dernier ecrit). Son metier est celui de son dernier mois ;
/// celui d'un contrat repris, qui n'ecrit pas de metier, est celui que nomme le code emploi que son bulletin imprime
/// (`CodeEmploi::du_code_declare`), quand il en nomme un. `None` sans aucun contrat, ou s'il ne dit pas de metier. Le
/// dernier contrat dont le metier ne se lit pas arrete la recherche : un contrat plus ancien n'est pas le dernier, et
/// l'erreur dit pourquoi rien ne se pre-remplit (l'appelant ne refuse rien pour elle).
pub fn metier_du_dernier_contrat(racine: &std::path::Path, s: &IdSalarie, sauf: Option<&crate::stock::Objet>) -> R<Option<droit::emplois::CodeEmploi>> {
    let mut dernier: Option<((Jour, socle::mois::Mois, usize), crate::stock::Objet)> = None;
    for o in crate::stock::objets(racine)? {
        let c = o.carte();
        if c.salarie.as_ref() != Some(s) || o.est_non_actif() {
            continue;
        }
        if sauf.is_some_and(|x| x.premier_mois() == o.premier_mois() && x.rang == o.rang) {
            continue;
        }
        let Some(debut) = c.debut_lu() else { continue };
        let cle = (debut, o.premier_mois(), o.rang);
        if dernier.as_ref().is_some_and(|(d, _)| cle <= *d) {
            continue;
        }
        dernier = Some((cle, o));
    }
    match dernier {
        Some((_, o)) => metier_de(&o),
        None => Ok(None),
    }
}

/// Le metier du contrat `o` a son dernier avenant (`metier_du_dernier_contrat`) : le metier et l'emploi sont des termes
/// du contrat (`saisie::Groupe::Terme`), qui valent jusqu'a ce qu'un avenant les change (`stock::Objet::derniers_termes`).
fn metier_de(o: &crate::stock::Objet) -> R<Option<droit::emplois::CodeEmploi>> {
    let v = o.derniers_termes();
    Ok(match (v.metier, v.emploi.and_then(|e| e.code_emploi_ft)) {
        (Some(m), _) => Some(m),
        (None, Some(ft)) => droit::emplois::CodeEmploi::du_code_declare(&ft)?,
        (None, None) => None,
    })
}

/// LES CHANGEMENTS D'UNE CASE, declares une fois : chaque case qui se saisit (ni ce que la vue deduit, ni ce qu'elle dit
/// de l'engagement ecrit), sa cle, son type (`null` : la case se vide) et son champ de la carte. Les lignes de paie
/// arrivent telles que l'appelant les donne (`L` : `LigneDonnee`), et se rangent a la porte (`Changement::ranger`) avant
/// d'etre posees (`LignePaie`).
macro_rules! changements {
    ( $( $(#[$va:meta])* $v:ident ( $t:ty ) = $cle:tt => $champ:ident ),* $(,)? ) => {
        /// UN CHANGEMENT D'UNE CASE DU BROUILLON : `{"case": "salarie", "valeur": "FULLER"}` ; `{"case": "nature",
        /// "valeur": "cdi"}` change sa nature, qui ne se vide jamais. Il s'ecrit comme il se lit (une route le rend
        /// quand elle rend la charge d'`enregistrer-saisie`, `Changement::qui_font`) : chaque valeur dans l'ecriture
        /// de sa case (`socle::ecriture::CaseEcrite` : une grandeur en nombre JSON).
        #[derive(Debug, Clone, PartialEq, Deserialize)]
        #[serde(tag = "case", content = "valeur", deny_unknown_fields)]
        pub enum Changement<L = LignePaie, N = IdSalarie, P = IdSpectacle, M = IdDeModele> {
            #[serde(rename = "nature")]
            Nature(NatureDeContrat),
            #[serde(rename = "salarie")]
            Salarie(Option<N>),
            #[serde(rename = "spectacle")]
            Spectacle(Option<P>),
            #[serde(rename = "lignes")]
            Lignes(Option<Vec<L>>),
            /// Le modele que la carte choisit.
            #[serde(rename = "modele_contrat")]
            ModeleContrat(Option<M>),
            $( $(#[$va])* #[serde(rename = $cle)] $v(Option<$t>), )*
        }

        impl<L: Serialize, N: Serialize, P: Serialize, M: Serialize> Serialize for Changement<L, N, P, M> {
            fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                use serde::ser::SerializeStruct;
                use socle::ecriture::CaseEcrite;
                let mut m = s.serialize_struct("Changement", 2)?;
                match self {
                    Changement::Nature(x) => {
                        m.serialize_field("case", "nature")?;
                        m.serialize_field("valeur", x)?;
                    }
                    Changement::Salarie(x) => {
                        m.serialize_field("case", "salarie")?;
                        m.serialize_field("valeur", x)?;
                    }
                    Changement::Spectacle(x) => {
                        m.serialize_field("case", "spectacle")?;
                        m.serialize_field("valeur", x)?;
                    }
                    Changement::Lignes(x) => {
                        m.serialize_field("case", "lignes")?;
                        m.serialize_field("valeur", x)?;
                    }
                    Changement::ModeleContrat(x) => {
                        m.serialize_field("case", "modele_contrat")?;
                        m.serialize_field("valeur", x)?;
                    }
                    $( Changement::$v(x) => {
                        m.serialize_field("case", $cle)?;
                        m.serialize_field("valeur", &CaseEcrite(x))?;
                    } )*
                }
                m.end()
            }
        }

        impl<L, N, P, M> Changement<L, N, P, M> {
            /// La case qu'il change.
            pub fn champ(&self) -> Champ {
                match self {
                    Changement::Nature(_) => Champ::Nature,
                    Changement::Salarie(_) => Champ::Salarie,
                    Changement::Spectacle(_) => Champ::Spectacle,
                    Changement::Lignes(_) => Champ::Lignes,
                    Changement::ModeleContrat(_) => Champ::ModeleContrat,
                    $( Changement::$v(_) => Champ::$v, )*
                }
            }
        }

        impl Changement<LigneDonnee, String, String, String> {
            /// Ses lignes rangees (`ranger`, `ligne::ranger`), le salarie et le spectacle qu'il nomme retrouves
            /// (`salarie`, `spectacle`), l'id du modele qu'il choisit lu (un id mal forme s'y refuse par la phrase d'une
            /// charge, `IdDeModele::de_la_charge`) ; une autre case telle quelle.
            pub fn ranger(
                self,
                ranger: &dyn Fn(Vec<LigneDonnee>) -> R<Vec<LignePaie>>,
                salarie: &dyn Fn(&str) -> R<IdSalarie>,
                spectacle: &dyn Fn(&str) -> R<IdSpectacle>,
            ) -> R<Changement> {
                Ok(match self {
                    Changement::Nature(n) => Changement::Nature(n),
                    Changement::Salarie(x) => Changement::Salarie(x.map(|nom| salarie(&nom)).transpose()?),
                    Changement::Spectacle(x) => Changement::Spectacle(x.map(|nom| spectacle(&nom)).transpose()?),
                    Changement::Lignes(l) => Changement::Lignes(l.map(ranger).transpose()?),
                    Changement::ModeleContrat(x) => Changement::ModeleContrat(x.map(|id| IdDeModele::de_la_charge(&id)).transpose()?),
                    $( Changement::$v(x) => Changement::$v(x), )*
                })
            }

            /// LES CHANGEMENTS QUI FONT LA CARTE `c` D'UNE CARTE NEUVE DE SA NATURE, tels qu'un appelant les donne : un
            /// par case remplie, sa nature exceptee (la carte neuve la porte) ; le salarie, le spectacle et le modele par
            /// leur cle, chaque ligne telle qu'elle se donne (`LignePaie::donnee` : une ligne que le moteur pose ne se
            /// donne pas, il la pose de nouveau).
            fn qui_font(c: &Carte) -> Vec<Self> {
                let mut out = Vec::new();
                if let Some(x) = &c.salarie {
                    out.push(Changement::Salarie(Some(x.clone().into())));
                }
                if let Some(x) = &c.spectacle {
                    out.push(Changement::Spectacle(Some(x.clone().into())));
                }
                if let Some(x) = &c.lignes {
                    out.push(Changement::Lignes(Some(x.iter().filter_map(LignePaie::donnee).collect())));
                }
                if let Some(x) = &c.modele_contrat {
                    out.push(Changement::ModeleContrat(Some(x.clone().into())));
                }
                $( if let Some(x) = &c.$champ {
                    out.push(Changement::$v(Some(x.clone())));
                } )*
                out
            }
        }

        impl Changement {
            /// Pose sur la carte `c` la valeur qu'il dit.
            fn poser(self, c: &mut Carte) {
                match self {
                    Changement::Nature(n) => c.nature = Some(n),
                    Changement::Salarie(x) => c.salarie = x,
                    Changement::Spectacle(x) => c.spectacle = x,
                    Changement::Lignes(x) => c.lignes = x,
                    Changement::ModeleContrat(x) => c.modele_contrat = x,
                    $( Changement::$v(x) => c.$champ = x, )*
                }
            }
        }
    };
}

changements! {
    Debut(Jour) = "debut" => debut,
    Fin(Jour) = "fin" => fin,
    Numero(crate::numeros::NumeroContrat) = "numero" => numero,
    Etablissement(droit::fiche::Nic) = "etablissement" => etablissement,
    Metier(droit::emplois::CodeEmploi) = "metier" => metier,
    Lieu(String) = "lieu" => lieu,
    MotifRecours(MotifDeRecours) = "motif_recours" => motif_recours,
    QuotiteContrat(Heures) = "quotite_contrat" => quotite_contrat,
    QuotiteReference(Heures) = "quotite_reference" => quotite_reference,
    ModaliteTempsTravail(ModaliteDuTempsDeTravail) = "modalite_temps_travail" => modalite_temps_travail,
    SalaireMensuel(MontantUnitaire) ="salaire_mensuel" => salaire_mensuel,
    /// L'emploi entier, chacun de ses sous-champs dit ou non (`droit::emplois::EmploiEcrit`) ; un objet qui n'en dit
    /// aucun vide la case.
    #[serde(deserialize_with = "droit::emplois::EmploiEcrit::dit")]
    Emploi(droit::emplois::EmploiEcrit) = "emploi" => emploi,
    #[serde(deserialize_with = "socle::date::Duree::en_jours_opt")]
    PeriodeEssaiJours(Duree<EnJours>) ="periode_essai_jours" => periode_essai_jours,
    PersonneRemplacee(String) = "personne_remplacee" => personne_remplacee,
    RepartitionHoraire(String) = "repartition_horaire" => repartition_horaire,
    ClausesParticulieres(String) = "clauses_particulieres" => clauses_particulieres,
    CongesPris(Vec<CongePris>) = "conges_pris" => conges_pris,
    Primes(Vec<Prime>) = "primes" => primes,
    DatePaiement(Jour) = "date_paiement" => date_paiement,
    Jours(Jours) = "jours" => jours,
    HeuresSupplementaires(Heures) = "heures_supplementaires" => heures_supplementaires,
    HeuresSupplementairesA50(Heures) = "heures_supplementaires_a_50" => heures_supplementaires_a_50,
    HeuresComplementaires(Heures) ="heures_complementaires" => heures_complementaires,
    IndemniteLicenciement(Montant) = "indemnite_licenciement" => indemnite_licenciement,
    FauteGrave(bool) = "faute_grave" => faute_grave,
    MotifRupture(MotifDeRupture) = "motif_rupture" => motif_rupture,
    NotificationRupture(Jour) = "notification_rupture" => notification_rupture,
    EngagementLicenciement(Jour) = "engagement_licenciement" => engagement_licenciement,
    SignatureConvention(Jour) = "signature_convention" => signature_convention,
    Preavis(Preavis) = "preavis" => preavis,
    Transaction(droit::preavis::TransactionEnCours) = "transaction" => transaction,
    DernierJourPaye(Jour) = "dernier_jour_paye" => dernier_jour_paye,
    EnDroitALaRetraite(bool) = "en_droit_a_la_retraite" => en_droit_a_la_retraite,
    PreavisNonEffectue(bool) = "preavis_non_effectue" => preavis_non_effectue,
    PreavisMois(Coefficient) = "preavis_mois" => preavis_mois,
    PrecariteDue(bool) = "precarite_due" => precarite_due,
    PrecaritePourquoi(String) = "precarite_pourquoi" => precarite_pourquoi,
}

/// LES CHANGEMENTS D'UN APPEL (`changements` des routes `enregistrer-saisie` et `modifier-engagement`), appliques dans
/// l'ordre, tout ou rien. La page en envoie ce qui a change depuis sa derniere ecriture (une case, le plus souvent) ; un
/// appelant en ligne de commande, toutes les cases d'une carte d'un coup. Aucun : la carte s'enregistre telle qu'elle est
/// (Leandre, 16/11 : « on peut appuyer sur enregistrer comme on veut »).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Changements(Vec<Changement<LigneDonnee, String, String, String>>);

impl Changements {
    /// Un de ses changements nomme-t-il le salarie de la carte ?
    pub fn nomment_le_salarie(&self) -> bool {
        self.0.iter().any(|c| matches!(c, Changement::Salarie(Some(_))))
    }

    /// CHAQUE CHANGEMENT APPLIQUE AU BROUILLON `b` d'une carte de `mois`, dans l'ordre (`Brouillon::changer`), tout ou
    /// rien : ses lignes se rangent d'abord, chacune lue au catalogue de la compagnie (`racine`, `ligne::ranger`) ; le
    /// premier refus (il nomme sa case, ou sa ligne) laisse `b` tel qu'il etait. `qui` : le salarie que le refus nomme,
    /// tel que le brouillon le dit au moment du changement.
    ///
    /// UNE CASE QUE LE MOTIF N'A PAS N'EXISTE PAS, et c'est ici, le seul endroit ou une carte change, qu'elle se vide :
    /// chaque fait de la fin que le motif de rupture n'appelle pas (`droit::permanents::faits_de_rupture` ; sans motif dit,
    /// ceux du motif par defaut de la nature) s'en va. Il ne reste pas ecrit sans se voir, et rien ne le controle (une date
    /// de notification posee sous un 034, puis le motif devient 031 : elle s'en va).
    ///
    /// UN METIER SE CHOISIT DANS CEUX DE LA COMPAGNIE (`metiers` : ceux de Configuration › Métiers, la liste que la route
    /// etat rend) : un autre se refuse en disant d'aller l'y ajouter.
    pub fn appliquer(self, ctx: &droit::contexte::Contexte, mois: socle::mois::Mois, b: &mut Brouillon, metiers: &[droit::emplois::CodeEmploi], qui: impl Fn(&Brouillon) -> String) -> R<()> {
        let mut essai = b.clone();
        for ch in self.0 {
            let q = qui(&essai);
            if let Changement::Metier(Some(m)) = &ch {
                if !metiers.contains(m) {
                    return Err(socle::refus!(
                        "{} : le métier {} n'est pas dans les métiers de la compagnie : ajoutez-le d'abord dans Configuration › Métiers. Rien n'est écrit.",
                        q,
                        m
                    ));
                }
            }
            let ch = ch.ranger(&|l| crate::ligne::ranger(&ctx.racine, mois, l, &q), &|nom| salarie_nomme(ctx, nom), &|nom| Ok(ctx.spectacle(nom)?.cle))?;
            essai.changer(ch, &q)?;
        }
        sans_les_faits_que_le_motif_n_a_pas(&mut essai, &qui)?;
        *b = essai;
        Ok(())
    }
}

/// Le brouillon `b`, sans les faits de la fin que son motif n'appelle pas (`Changements::appliquer`). `qui` : le salarie
/// que le refus nomme.
fn sans_les_faits_que_le_motif_n_a_pas(b: &mut Brouillon, qui: &impl Fn(&Brouillon) -> String) -> R<()> {
    let nat = b.nature().proprietes()?;
    let mut c = b.en_carte();
    let faits = droit::permanents::faits_de_rupture(&nat, c.motif_rupture)?;
    let garde = |f: D| faits.contains(&f);
    let mut vide = false;
    if !garde(D::NotificationRupture) {
        vide |= c.notification_rupture.take().is_some();
    }
    if !garde(D::EngagementLicenciement) {
        vide |= c.engagement_licenciement.take().is_some();
    }
    if !garde(D::SignatureConvention) {
        vide |= c.signature_convention.take().is_some();
    }
    if !garde(D::Transaction) {
        vide |= c.transaction.take().is_some();
    }
    if !garde(D::EnDroitALaRetraite) {
        vide |= c.en_droit_a_la_retraite.take().is_some();
    }
    // La duree du preavis non effectue, et le pourquoi de l'indemnite ecartee, suivent leur case (la carte les ouvre
    // avec elle).
    if !garde(D::PreavisNonEffectue) {
        vide |= c.preavis_non_effectue.take().is_some();
        vide |= c.preavis_mois.take().is_some();
    }
    if !garde(D::PrecariteDue) {
        vide |= c.precarite_due.take().is_some();
        vide |= c.precarite_pourquoi.take().is_some();
    }
    if vide {
        *b = Brouillon::de_la_carte(&c, &qui(b))?;
    }
    Ok(())
}

/// LE SALARIE QU'UN NOM TAPE DESIGNE, retrouve une fois a la porte (`Contexte::cle_du_salarie`) : la carte ne porte
/// que sa cle. Un nom vide ne designe personne ; aucun salarie, ou plusieurs, se refuse en disant quoi corriger.
pub fn salarie_nomme(ctx: &droit::contexte::Contexte, nom: &str) -> R<IdSalarie> {
    if nom.trim().is_empty() {
        return Err(refus("Le nom du salarié est vide : écrire son nom, ou vider la case. Rien n'est écrit."));
    }
    ctx.cle_du_salarie(&droit::contexte::SalarieDesigne::lire(nom)?)
}

/// Le debut du refus d'une carte a qui il manque des cases (`crate::SAISIE_INCOMPLETE`).
pub const A_REMPLIR: &str = " : à remplir sur sa carte avant de valider : ";

/// Une case obligatoire est remplie : dite, et un texte n'y est pas vide.
fn remplie(c: &Carte, x: Champ) -> bool {
    match c.valeur_du(x) {
        Some(Valeur::Texte(t)) => !t.trim().is_empty(),
        _ => c.dit(x),
    }
}

/// « a », « b » et « c ».
fn guillemets(noms: &[&str]) -> String {
    let g: Vec<String> = noms.iter().map(|n| format!("« {} »", n)).collect();
    match g.as_slice() {
        [] => String::new(),
        [seul] => seul.clone(),
        [reste @ .., dernier] => format!("{} et {}", reste.join(", "), dernier),
    }
}

/// Le libelle de la case de la carte dont la cle est `cle` (`Champ::nom`) ; aucun pour une cle qui n'en est pas une.
pub fn libelle_de_la_case(cle: &str) -> Option<&'static str> {
    Champ::TOUS.iter().find(|c| c.nom() == cle).map(|c| libelle(*c))
}

/// LE LIBELLE D'UNE CASE DE LA CARTE, tel que l'ecran l'ecrit.
pub fn libelle(c: Champ) -> &'static str {
    match c {
        Champ::Salarie => "Salarié",
        Champ::Spectacle => "Spectacle",
        Champ::Nature => "Nature",
        Champ::Debut => "Début",
        Champ::Fin => "Fin",
        Champ::Numero => "Numéro du contrat",
        Champ::Etablissement => "Établissement",
        Champ::Metier => "Métier",
        Champ::Lieu => "Lieu",
        Champ::MotifRecours => "Motif du CDD",
        Champ::QuotiteContrat => "Heures par mois",
        Champ::QuotiteReference => "Heures de référence",
        Champ::ModaliteTempsTravail => "Temps de travail",
        Champ::SalaireMensuel => "Salaire mensuel brut",
        Champ::Emploi => "Emploi",
        Champ::PcsEse => "PCS-ESE",
        Champ::CodeEmploiFt => "Code emploi France Travail",
        Champ::Statut => "Statut",
        Champ::StatutConventionnel => "Statut conventionnel",
        Champ::StatutCategorielRc => "Statut catégoriel",
        Champ::PeriodeEssaiJours => "Période d'essai",
        Champ::PersonneRemplacee => "Personne remplacée",
        Champ::RepartitionHoraire => "Répartition des horaires",
        Champ::ClausesParticulieres => "Clauses particulières",
        Champ::ModeleContrat => "Modèle de contrat",
        Champ::Lignes => "Lignes de paie",
        Champ::CongesPris => "Congés pris",
        Champ::Primes => "Primes",
        Champ::DatePaiement => "Payé le",
        Champ::Jours => "Jours travaillés",
        Champ::HeuresSupplementaires => "Heures supplémentaires",
        Champ::HeuresSupplementairesA50 => "Dont à 50 %",
        Champ::HeuresComplementaires => "Heures complémentaires",
        Champ::IndemniteLicenciement => "Indemnité de licenciement",
        Champ::FauteGrave => "Faute grave",
        Champ::MotifRupture => "Motif de la fin",
        Champ::NotificationRupture => "Notifiée le",
        Champ::EngagementLicenciement => "Procédure engagée le",
        Champ::SignatureConvention => "Convention signée le",
        Champ::Preavis => "Préavis",
        Champ::Transaction => "Transaction",
        Champ::DernierJourPaye => "Dernier jour payé",
        Champ::EnDroitALaRetraite => "En droit à la retraite",
        Champ::PreavisNonEffectue => "Préavis non effectué",
        Champ::PreavisMois => "Mois de préavis",
        Champ::PrecariteDue => "Indemnité de fin de contrat",
        Champ::PrecaritePourquoi => "Pourquoi elle n'est pas due",
        Champ::JoursContrat => "Jours du contrat",
        Champ::DateModification => "Date de la modification",
        Champ::Avant => "Termes d'avant",
        Champ::Valide => "Validé",
        Champ::Importe => "Importé",
    }
}
