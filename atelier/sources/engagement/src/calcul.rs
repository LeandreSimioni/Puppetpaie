//! Les pieces de `calculer` : les
//! lignes de remuneration et leur designation, les structs de sortie
//! (`LigneCotisation`, `Paie`), la reduction generale, le seuil de SMIC et la
//! deduction forfaitaire, puis `calculer` lui-meme (§7).

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, OnceLock};


use droit::bareme::{bareme_applique, SANTE_SUSPENSION, SANTE_SUSPENSION_MAINTENUE, SANTE_SUSPENSION_SUSPENDUE};
use droit::natures::{AuMois, Nature};
use droit::fiche::{AccordDfs, ChampsEntreprise, Lue, ChampsSalarie, FicheSalarie, OptionDfs, SalarieDuContrat, Statut};
use droit::catalogue::{code_54, rubrique, HorsAssiette, Unite};
use droit::contexte::{Contexte, IdSalarie};
use socle::date::{Jour, Periode};
use socle::texte::euros;
use droit::emplois::{emploi_de, EmploiEcrit, EmploiEngagement};
use crate::{Avis, Engagement, LignePaie};
use crate::paie_ecrite::{Cle, FormeDePaie};
use droit::assiette::Assiette;
use socle::echec::{refus, refus_avec, Lieu, Echec, R};
use socle::grandeurs::{courte, Coefficient, Heures, Montant, MontantUnitaire, Taux};
use crate::ligne::{PaiementDeLigne, QuantiteDeLigne};
pub use droit::reduction::ReductionGenerale;
use droit::bareme::somme_pct;
use droit::forme_bareme::{IdDeCotisation, Pile, Rubrique, MUTUELLE, PREVOYANCE};
use droit::reduction::{au_dela_du_seuil, reduction_generale, reduction_generale_sous_dfs};
use droit::plafond::{plafond_permanent, pour_paie, EngagementPlafond};


/// Un taux de cotisation qu'une addition compose (le taux reduit et son complement, une part et la CET) : a trois
/// decimales, la moitie au pair, dans sa forme la plus courte. Aucun texte ne fixe cet arrondi ; les taux additionnes
/// n'ont pas plus de trois decimales.
fn taux_compose(x: Taux) -> Taux {
    x.au_pair(3)
}

// ------------------------------------------------------ les lignes de brut

/// UNE LIGNE DE REMUNERATION : une rubrique, ce qu'elle paie (`P`), son montant. Celle que le calcul rend d'une ligne de
/// paie dit toujours ce qu'elle paie, une quantite a un taux (`LigneBrut`, `P` = `PaiementDeLigne`) ; celle d'un
/// bulletin importe que l'import a rattachee a sa rubrique garde son montant lu, et ce qu'elle paie quand le bulletin
/// l'imprime (`LigneRattachee` : un forfait imprime a son seul montant n'a pas de quantite). Celle du calcul s'ecrit
/// dans la paie ecrite (`paie_ecrite::FormeDePaie::remuneration`), ses nombres en texte normalise.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, bound(serialize = "P: EcritSansZero", deserialize = "P: serde::Deserialize<'de>"))]
pub struct LigneBrut<P = PaiementDeLigne> {
    pub code: droit::catalogue::CodeDeRubrique,
    pub libelle: String,
    pub unite: Unite,
    /// Ce qu'elle paie, sa quantite et son taux dans l'unite de sa rubrique (`unite`), ecrit par sa forme sans zero
    /// inutile (`{"cachets": ["2", "150"]}`).
    #[serde(serialize_with = "EcritSansZero::ecrit")]
    pub paiement: P,
    #[serde(with = "socle::grandeurs::ecrit::nombre")]
    pub montant: Montant,
    #[serde(with = "socle::grandeurs::ecrit::nombre")]
    pub heures_annexes: Heures,
    pub soumis: bool,
    pub imposable: bool,
    pub conges_spectacles: bool,
    pub salaire_de_base: bool,
    pub retabli: bool,
    pub conges_pris: bool,
    pub dsn_bloc52: Option<droit::type_de_prime::TypeDePrime>,
    pub arret_de_travail: bool,
    pub dsn_autres_bruts: Option<droit::autre_revenu_brut::AutreElementDeRevenuBrut>,
    /// Ce que la ligne dit d'elle-meme sur le bulletin quand sa quantite ne suffit pas : la part payee d'un mois incomplet
    /// (`droit::periodes::DecompteDuMois::sur_le_bulletin`).
    pub precision: Option<String>,
}

/// L'ECRITURE DE CE QU'UNE LIGNE DE REMUNERATION PAIE : par sa forme, sans zero inutile (`PaiementDeLigne::sans_zero`).
/// Seule la ligne du calcul s'ecrit.
pub trait EcritSansZero {
    fn ecrit<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error>;
}

impl EcritSansZero for PaiementDeLigne {
    fn ecrit<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        serde::Serialize::serialize(&self.sans_zero(), s)
    }
}

/// UNE LIGNE D'UN BULLETIN IMPORTE RATTACHEE A SA RUBRIQUE (`importe::LigneImprimee::rattachee`) : son montant tel que
/// le bulletin l'imprime, et ce qu'elle paie quand il l'imprime (`None` : un forfait imprime a son seul montant).
pub type LigneRattachee = LigneBrut<Option<PaiementDeLigne>>;

impl LigneBrut {
    /// La ligne en francais, telle qu'une personne la lit : « 2 Cachets », « 1 Journée de travail », « 6 h de
    /// masterclass / action culturelle », « 154 h d'absence non rémunérée », « Indemnité compensatrice de congés payés
    /// (4,92 jours) », « Indemnité de fin de contrat (précarité) ». Un nombre se dit au centieme. La quantite se compte
    /// devant le libelle quand son premier mot nomme l'unite (un cachet, une journee, un service, une semaine) ; sinon
    /// elle se dit apres lui, dans son unite. Un forfait et un pourcentage se disent par leur seul libelle : leur
    /// quantite n'est pas un nombre de choses.
    pub fn designation(&self) -> String {
        if matches!(self.unite, Unite::Forfait | Unite::Pourcentage) {
            return self.libelle.clone();
        }
        // UN SALAIRE MENSUEL se paie au mois : sa quantite est la fraction du mois payee (`rubriques-base.json`, `mois`).
        // Un mois entier se dit seul ; une fraction se dit en part du mois, pas en decimales.
        if let PaiementDeLigne::PartDuMois(part, _) = self.paiement {
            if part == Coefficient::UN {
                return self.libelle.clone();
            }
            if let Some(p) = &self.precision {
                return format!("{} ({})", self.libelle, p);
            }
            let part = socle::texte::virgule(socle::grandeurs::au_centieme_au_pair(part.en_taux()));
            return format!("{} ({} % du mois)", self.libelle, part);
        }
        // Une base (une ligne en pourcentage) n'est pas un nombre de choses, ni un montant seul : son seul libelle.
        let Some(q) = self.paiement.quantite().map(QuantiteDeLigne::au_centieme) else { return self.libelle.clone() };
        let nb = q.dit();
        let (tete, reste) = match self.libelle.split_once(' ') {
            Some((t, r)) => (t, Some(r)),
            None => (self.libelle.as_str(), None),
        };
        let compte = match self.unite {
            Unite::Heure => ["heure"].as_slice(),
            Unite::Jour => ["jour", "journée"].as_slice(),
            Unite::Cachet => ["cachet"].as_slice(),
            Unite::Service => ["service"].as_slice(),
            Unite::Semaine => ["semaine"].as_slice(),
            Unite::Mois | Unite::Forfait | Unite::Pourcentage => [].as_slice(),
        };
        // Des heures qui se nomment au pluriel (« Heures supplémentaires ») : leur nombre, et ce qui les distingue
        // (leur majoration), entre parentheses.
        if self.unite == Unite::Heure && tete.to_lowercase() == "heures" {
            return match &self.precision {
                Some(p) => format!("{} ({} h, {})", self.libelle, nb, p),
                None => format!("{} ({} h)", self.libelle, nb),
            };
        }
        // Au singulier jusqu'a deux exclus (« 1,5 journée »).
        let pluriel = q.au_pluriel();
        if !compte.contains(&tete.to_lowercase().as_str()) {
            if self.unite == Unite::Heure {
                let mut l = self.libelle.chars();
                let minuscule: String = l.next().into_iter().flat_map(|c| c.to_lowercase()).chain(l).collect();
                return format!("{} h {}", nb, socle::texte::de(&minuscule));
            }
            return format!("{} ({} {}{})", self.libelle, nb, self.unite, if pluriel { "s" } else { "" });
        }
        // Le pluriel porte sur le premier mot : il nomme ce qui se compte.
        let tete = if pluriel && !tete.ends_with('s') { format!("{}s", tete) } else { tete.to_string() };
        match reste {
            Some(r) => format!("{} {} {}", nb, tete, r),
            None => format!("{} {}", nb, tete),
        }
    }

}

/// Les lignes de remuneration d'un engagement : une par ligne de paie, le
/// montant arrondi ligne par ligne.
pub(crate) fn lignes_brut(eng: &Engagement, ctx: &Contexte) -> R<Vec<LigneBrut>> {
    eng.lignes.iter().map(|l| ligne_brut(l, ctx)).collect()
}

/// LA LIGNE DE REMUNERATION D'UNE LIGNE DE PAIE, son montant arrondi : ce qu'elle paie, toujours dit (une case vide se
/// refuse en se nommant, `LignePaie::paiement`), et sa quantite a son taux.
pub(crate) fn ligne_brut(l: &LignePaie, ctx: &Contexte) -> R<LigneBrut> {
    let r = rubrique(&ctx.racine, l.rubrique(), &ctx.lue_au)?;
    let paiement = l.paiement(r.unite)?;
    let heures_annexes = heures_annexes(&r, &paiement, l.heures())?;
    let montant = r.sens.de(paiement.montant().au_centime());
    ligne_de(r, paiement, heures_annexes, montant, l.genre(), l.precision())
}

/// LA LIGNE D'UN BULLETIN IMPORTE RATTACHEE A LA RUBRIQUE `code` (`importe::LigneImprimee::rattachee`) : son montant tel
/// qu'il le lit, qui ne se recompose jamais, et ce qu'il dit qu'elle paie et ses heures par unite quand il les imprime.
/// Un forfait imprime a son seul montant ne dit pas ce qu'il paie : il ne compte aucune heure.
pub(crate) fn ligne_rattachee(code: &droit::catalogue::CodeDeRubrique, paiement: Option<PaiementDeLigne>, heures: Option<socle::grandeurs::HeuresParUnite>, montant: Montant, ctx: &Contexte) -> R<LigneRattachee> {
    let r = rubrique(&ctx.racine, code, &ctx.lue_au)?;
    let heures_annexes = match &paiement {
        Some(p) => heures_annexes(&r, p, heures)?,
        None => r.sens.de(courte(Heures::ZERO)),
    };
    ligne_de(r, paiement, heures_annexes, montant, None, None)
}

/// La ligne de remuneration rattachee a sa rubrique `r`, le genre d'un avantage en nature et sa precision quand elle les
/// dit.
fn ligne_de<P>(r: droit::catalogue::Rubrique, paiement: P, heures_annexes: Heures, montant: Montant, genre: Option<droit::autre_revenu_brut::AutreElementDeRevenuBrut>, precision: Option<&str>) -> R<LigneBrut<P>> {
    let dsn_autres_bruts = code_54(&r, genre)?;
    Ok(LigneBrut {
        code: r.code.into(),
        libelle: r.libelle,
        unite: r.unite,
        paiement,
        montant,
        heures_annexes,
        soumis: r.soumis,
        imposable: r.imposable,
        conges_spectacles: r.conges_spectacles,
        salaire_de_base: r.salaire_de_base,
        retabli: r.retabli,
        conges_pris: r.conges_pris,
        dsn_bloc52: r.dsn_bloc52,
        arret_de_travail: r.arret_de_travail,
        dsn_autres_bruts,
        precision: precision.map(String::from),
    })
}

/// LES HEURES DES ANNEXES 8 ET 10 de ce que paie une ligne rattachee a sa rubrique `r`, a ses heures par unite quand elle
/// en dit, sous le signe de la rubrique (une absence en retire ; des heures nulles gardent leur echelle).
fn heures_annexes(r: &droit::catalogue::Rubrique, paiement: &PaiementDeLigne, heures: Option<socle::grandeurs::HeuresParUnite>) -> R<Heures> {
    Ok(r.sens.de(paiement.heures(r.heures_par_unite(heures))?))
}

// ----------------------------------------------------------------- sortie

/// LES LIGNES DE COTISATION DONT LA BASE COMPTE DES HEURES : celles que le calcul compose hors du bareme sur des heures,
/// la deduction patronale des heures supplementaires (`heures_en_plus::DEDUCTION`, D241-24 : par heure).
const BASES_EN_HEURES: [IdDeCotisation; 1] = [crate::heures_en_plus::DEDUCTION];

/// LA BASE D'UNE LIGNE DE COTISATION, et la valeur d'une assiette d'une paie (`Contenu::bases`) : des euros, ou des
/// heures, celles des heures en plus (`Assiette::HeuresEnPlusHeures`) et la base de la deduction patronale des heures
/// supplementaires (`heures_en_plus::DEDUCTION`), son nombre d'heures, masquee au bulletin. Elle s'ecrit par sa forme,
/// sa grandeur puis son nombre en texte (`{"montant": "1512.40"}`, `{"heures": "3"}`).
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum BaseDeCotisation {
    Montant(
        #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize", deserialize_with = "socle::grandeurs::lu::nombre_ou_texte::deserialize")] Montant,
    ),
    Heures(
        #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize", deserialize_with = "socle::grandeurs::lu::nombre_ou_texte::deserialize")] Heures,
    ),
}

impl BaseDeCotisation {
    /// LA BASE `n` QU'UN BULLETIN IMPORTE IMPRIME (un nombre lu, `import::bulletin_lu`) sur la ligne que l'import qualifie
    /// de la cotisation `id`, dans la grandeur que cette cotisation compte : la porte de sa lecture. Une cotisation se
    /// relit comme une paie validee la relit (`relue` : des heures pour la deduction des heures supplementaires, que le
    /// calcul compose hors du bareme, des euros pour toute autre, le calcul refusant une cotisation du bareme dont
    /// l'assiette compte des heures) ; une ligne sans cotisation (un regroupement, une ligne que l'import ne qualifie
    /// pas) somme des cotisations du bareme : des euros.
    pub fn imprimee(id: Option<&IdDeCotisation>, n: socle::forme::Nombre) -> BaseDeCotisation {
        match id {
            Some(id) => BaseDeCotisation::relue(id, n),
            None => BaseDeCotisation::Montant(n.grandeur()),
        }
    }

    /// La valeur `n` qu'une paie ecrite donne a l'assiette `a`, lue dans la grandeur que l'assiette compte : des heures
    /// quand elle en compte (`Assiette::en_heures`), des euros sinon. La porte de sa lecture.
    pub fn de_l_assiette(a: Assiette, n: socle::forme::Nombre) -> BaseDeCotisation {
        if a.en_heures() {
            BaseDeCotisation::Heures(n.grandeur())
        } else {
            BaseDeCotisation::Montant(n.grandeur())
        }
    }

    /// La base nulle de l'assiette `a`, dans la grandeur qu'elle compte (`de_l_assiette`), ses deux decimales ecrites :
    /// celle d'une paie annulee (`Paie::annulee`).
    pub fn nulle(a: Assiette) -> BaseDeCotisation {
        if a.en_heures() {
            BaseDeCotisation::Heures(Heures::ZERO.au_centieme())
        } else {
            BaseDeCotisation::Montant(Montant::ZERO)
        }
    }

    /// LA BASE `v` QUE LE CALCUL DONNE A L'ASSIETTE `a`, dans la grandeur que l'assiette compte : la regle de
    /// `de_l_assiette` (`Assiette::en_heures`) ; une base d'une autre grandeur se refuse.
    pub(crate) fn calculee(a: Assiette, v: BaseDeCotisation) -> R<BaseDeCotisation> {
        match (a.en_heures(), v) {
            (true, BaseDeCotisation::Heures(_)) | (false, BaseDeCotisation::Montant(_)) => Ok(v),
            (true, BaseDeCotisation::Montant(_)) | (false, BaseDeCotisation::Heures(_)) => {
                Err(refus(format!("L'assiette « {} » ne compte pas la grandeur que le calcul lui donne : le signaler au développeur.", a.nom())))
            }
        }
    }

    /// LA BASE `n` ECRITE DE LA LIGNE DE LA COTISATION `id`, relue sans bareme (une paie validee se relit telle qu'elle
    /// est partie) : des heures pour les lignes que le calcul compose hors du bareme sur des heures
    /// (`BASES_EN_HEURES`), des euros pour toute autre : le calcul refuse une cotisation du bareme dont l'assiette compte
    /// des heures (la boucle des cotisations de `calculer`). La porte de sa lecture.
    pub(crate) fn relue(id: &IdDeCotisation, n: socle::forme::Nombre) -> BaseDeCotisation {
        if BASES_EN_HEURES.contains(id) {
            BaseDeCotisation::Heures(n.grandeur())
        } else {
            BaseDeCotisation::Montant(n.grandeur())
        }
    }

    /// Ecrite dans une paie ecrite, sans son unite (sa ligne ou son assiette la dit a la relecture, `relue`,
    /// `de_l_assiette`) : son nombre dans sa forme la plus courte (`socle::grandeurs::ecrit::nombre`).
    pub fn en_nombre(self) -> String {
        match self {
            BaseDeCotisation::Montant(m) => socle::grandeurs::ecrit::nombre::texte(m),
            BaseDeCotisation::Heures(h) => socle::grandeurs::ecrit::nombre::texte(h),
        }
    }

    /// Deux bases de meme grandeur ajoutees (deux periodes d'un mois decoupe) ; des euros et des heures ne s'ajoutent pas.
    pub fn plus(self, o: BaseDeCotisation) -> R<BaseDeCotisation> {
        match (self, o) {
            (BaseDeCotisation::Montant(a), BaseDeCotisation::Montant(b)) => Ok(BaseDeCotisation::Montant(a + b)),
            (BaseDeCotisation::Heures(a), BaseDeCotisation::Heures(b)) => Ok(BaseDeCotisation::Heures(a + b)),
            _ => Err(refus("une base en euros et une base en heures ne s'ajoutent pas : le signaler au développeur")),
        }
    }

    /// Au centime, ou au centieme d'heure : deux decimales, la moitie vers le haut (`Montant::au_centime`,
    /// `Heures::au_centieme`).
    pub fn au_centime(self) -> BaseDeCotisation {
        match self {
            BaseDeCotisation::Montant(m) => BaseDeCotisation::Montant(m.au_centime()),
            BaseDeCotisation::Heures(h) => BaseDeCotisation::Heures(h.au_centieme()),
        }
    }

    /// Ses euros, quand elle en compte ; `None` pour des heures, qui ne sont pas un montant.
    pub fn montant(self) -> Option<Montant> {
        match self {
            BaseDeCotisation::Montant(m) => Some(m),
            BaseDeCotisation::Heures(_) => None,
        }
    }

    /// LA VALEUR DE L'ASSIETTE `assiette`, qui compte des heures (`Assiette::en_heures`) : refus si elle dit des euros.
    pub fn en_heures(self, assiette: Assiette) -> R<Heures> {
        match self {
            BaseDeCotisation::Heures(h) => Ok(h),
            BaseDeCotisation::Montant(m) => Err(refus(format!(
                "L'assiette « {} » compte des heures, et la paie en dit des euros ({}) : elle ne se lit pas. Le signaler au développeur.",
                assiette.nom(),
                socle::texte::euros(m)
            ))),
        }
    }

    /// LA VALEUR DE L'ASSIETTE `assiette`, qui compte des euros : refus si elle dit des heures (une base qui n'est pas
    /// dans la grandeur que son assiette attend ne se lit pas).
    pub fn en_euros(self, assiette: Assiette) -> R<Montant> {
        match self {
            BaseDeCotisation::Montant(m) => Ok(m),
            BaseDeCotisation::Heures(h) => Err(refus(format!(
                "L'assiette « {} » compte des euros, et la paie en dit des heures ({}) : elle ne se lit pas. Le signaler au développeur.",
                assiette.nom(),
                socle::grandeurs::ecrit::texte::dit(h)
            ))),
        }
    }

    /// Est-elle nulle ?
    pub fn est_nulle(self) -> bool {
        match self {
            BaseDeCotisation::Montant(m) => m.est_nul(),
            BaseDeCotisation::Heures(h) => h.est_nul(),
        }
    }
}

/// UNE LIGNE DE COTISATION. Elle s'ecrit dans la paie ecrite (`paie_ecrite::FormeDePaie::lignes`), ses nombres en texte
/// dans leur forme la plus courte ; sa base `B` s'ecrit en nombre, des euros ou des heures, sans son unite
/// (`BaseDeCotisation::en_nombre`) : elle se lit sa base en nombre (`LigneCotisation<socle::forme::Nombre>`, la seule
/// qui se lit), que `relue` lit aussitot dans la grandeur que sa cotisation compte (`BaseDeCotisation::relue`) ; elle ne
/// circule que sa base typee.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, bound(serialize = "B: Copy + Into<BaseDeCotisation>", deserialize = "B: serde::Deserialize<'de> + Into<socle::forme::Nombre>"))]
pub struct LigneCotisation<B = BaseDeCotisation> {
    /// La cotisation de la ligne.
    pub id: droit::forme_bareme::IdDeCotisation,
    /// Sa famille.
    pub famille: droit::forme_bareme::FamilleDeCotisation,
    pub libelle: String,
    #[serde(serialize_with = "crate::paie_ecrite::base_en_nombre")]
    pub base: B,
    #[serde(with = "socle::grandeurs::ecrit::nombre::option")]
    pub taux_pat: Option<Taux>,
    #[serde(with = "socle::grandeurs::ecrit::nombre")]
    pub montant_pat: Montant,
    #[serde(with = "socle::grandeurs::ecrit::nombre::option")]
    pub taux_sal: Option<Taux>,
    #[serde(with = "socle::grandeurs::ecrit::nombre")]
    pub montant_sal: Montant,
    pub masquer_base_et_taux: bool,
}

impl LigneCotisation {
    /// Est-elle la ligne de la cotisation `id` ?
    pub fn est(&self, id: &droit::forme_bareme::IdDeCotisation) -> bool {
        self.id == *id
    }
}

impl LigneCotisation<socle::forme::Nombre> {
    /// La ligne lue, sa base dans la grandeur que sa cotisation compte (`BaseDeCotisation::relue`) : la porte de sa
    /// lecture.
    pub(crate) fn relue(self) -> LigneCotisation {
        LigneCotisation {
            base: BaseDeCotisation::relue(&self.id, self.base),
            id: self.id,
            famille: self.famille,
            libelle: self.libelle,
            taux_pat: self.taux_pat.map(socle::grandeurs::courte),
            montant_pat: self.montant_pat,
            taux_sal: self.taux_sal.map(socle::grandeurs::courte),
            montant_sal: self.montant_sal,
            masquer_base_et_taux: self.masquer_base_et_taux,
        }
    }
}

/// La deduction forfaitaire specifique (DFS) appliquee a un contrat : arrete du 4 septembre 2025
/// relatif aux frais professionnels, art. 9 (`atelier/reference/sources/dfs/LISEZ-MOI.md`). La paie ecrite la garde
/// telle quelle (`paie_ecrite::FormeDePaie`), ses grandeurs en texte normalise (`socle::grandeurs::ecrit::nombre`).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeductionForfaitaire {
    /// « 20 » ou « 25 » : le taux de l'article 5 de l'annexe IV du CGI (redaction du 31/12/2000) que la profession porte.
    pub categorie: droit::forme_bareme::CategorieDfs,
    /// La profession de la fiche (`deduction_forfaitaire.professions` du bareme).
    pub profession: droit::fiche::ProfessionDfs,
    /// Le taux de l'annee (bareme), en pourcent.
    #[serde(with = "socle::grandeurs::ecrit::nombre")]
    pub taux: Taux,
    /// Les lignes du bulletin dont l'assiette est abattue.
    pub rubriques: Vec<IdDeCotisation>,
    /// La deduction retenue sur ce contrat : le brut moins l'assiette abattue, apres le plafond annuel et l'assiette
    /// minimum. C'est ce que la DSN declare en bloc 54 de type 10.
    #[serde(with = "socle::grandeurs::ecrit::nombre")]
    pub montant: Montant,
    /// Ce que l'annee civile a deja deduit pour ce salarie avant ce contrat (les mois anterieurs, les contrats
    /// precedents du mois).
    #[serde(with = "socle::grandeurs::ecrit::nombre")]
    pub deja: Montant,
}

/// Ce que la remuneration nette fiscale reintegre au net des cotisations : la CSG et la CRDS non deductibles (part
/// salariale de `csg_crds_nd`) et la part patronale des garanties frais de sante (`mutuelle`) ; norme DSN 2026.1,
/// p. 60-61 (« AJOUTER ») et p. 244 (50.004). Un seul endroit : `calculer` en tire 50.002, `Paie::net_verse` 50.004.
fn reintegrations(lignes: &[LigneCotisation]) -> Montant {
    let csg_nd = lignes.iter().filter(|l| l.est(&droit::forme_bareme::CSG_CRDS_ND)).take(1).fold(Montant::ZERO, |a, l| a + l.montant_sal);
    csg_nd + lignes.iter().filter(|l| l.est(&droit::forme_bareme::MUTUELLE)).fold(Montant::ZERO, |a, l| a + l.montant_pat)
}

/// Une paie d'un engagement : ce que son calcul a produit, ou sa forme ecrite relue (`relire`). Elle se lit partout, ce
/// qu'elle porte a travers `Contenu` ; elle ne se construit et ne s'ecrit que dans cette crate, qui seule calcule.
#[derive(Debug, Clone)]
pub struct Paie(Contenu);

impl std::ops::Deref for Paie {
    type Target = Contenu;
    fn deref(&self) -> &Contenu {
        &self.0
    }
}

/// Ce qu'une paie porte. Se lit a travers `Paie` ; un `Contenu` construit hors de cette crate ne devient jamais une paie.
#[derive(Debug, Clone)]
pub struct Contenu {
    pub engagement: Engagement,
    pub salarie: SalarieDuContrat,
    /// La fiche du spectacle telle que le calcul l'a lue ; `None` pour un contrat paye au mois.
    pub spectacle: Option<droit::fiche::Spectacle>,
    pub brut: Montant,
    pub base_csg: Montant,
    pub plafond: Montant,
    /// Le « Plafond Urssaf » du bas de bulletin, tel que le calcul l'a pose (`plafond_urssaf`) et que la paie ecrite le
    /// garde : un document le lit, il ne le recalcule pas.
    pub plafond_urssaf: Montant,
    /// LA PART DU BRUT QUE L'ABATTEMENT DE LA CSG A ABATTUE dans cette paie (`plafond::Regularisees::abattue_csg`, telle
    /// qu'elle s'est appliquee) : la paie suivante de l'annee la lit pour regulariser la limite de l'abattement
    /// (`ajouter_a_l_annee`), la taxe sur les salaires pour rendre l'abattement a l'assiette de la CSG (CGI 231, 1). Ce
    /// n'est pas une assiette : aucune cotisation ne s'y assoit, la DSN ne la declare pas, et deux paies qui ne different
    /// que par elle disent les memes chiffres (`etats::chiffres_egaux`).
    pub part_abattue_csg: Montant,
    /// Le SMIC horaire de la periode, dans sa forme la plus courte (`Contexte::smic`).
    pub smic: MontantUnitaire,
    pub heures: Heures,
    pub lignes: Vec<LigneCotisation>,
    pub total_pat: Montant,
    pub total_sal: Montant,
    /// Ce qui se paie au salarie avant impot : le brut, moins les cotisations salariales, plus ou moins TOUTES les
    /// lignes hors assiette (defraiements, acompte retenu, indemnites), moins les avantages en nature, recus en nature
    /// (C. trav. R. 3243-1, 9° : « la somme qui aurait été versée au salarié en l'absence de retenue à la source »).
    pub net_a_payer: Montant,
    /// Le montant net social (S21.G00.58.004 de type 03, « Montant net social » du bulletin) : voir `calculer`
    /// (7.10). Le brut, moins les cotisations salariales, plus les lignes hors assiette qui sont un revenu ; ni
    /// remboursement de frais, ni mouvement de tresorerie (acompte).
    pub net_social: Montant,
    /// La remuneration nette fiscale (S21.G00.50.002, « Net imposable » du bulletin, base du PAS) : voir
    /// `calculer` (7.10) ; les lignes hors assiette n'y entrent que si le catalogue les dit imposables.
    pub net_imposable: Montant,
    pub cout_employeur: Montant,
    pub numero_bulletin: Option<crate::numeros::NumeroDeBulletin>,
    /// Les assiettes du calcul, que la DSN declare telles quelles, chacune dans ce qu'elle compte.
    pub bases: BTreeMap<Assiette, BaseDeCotisation>,
    /// Les lignes de remuneration (le haut du bulletin), pas les cotisations.
    pub remuneration: Vec<LigneBrut>,
    pub reduction_generale: Option<ReductionGenerale>,
    pub deduction_forfaitaire: Option<DeductionForfaitaire>,
    pub avertissements: Vec<Avis>,
    pub salaire_de_base: Montant,
    pub salaire_retabli: Montant,
    pub assiette_chomage: Montant,
    pub heures_travaillees: Heures,
    pub heures_absence: Heures,
    /// Les periodes elementaires de paie d'un mois decoupe (`periodes`), chacune une paie calculee sur ses
    /// bornes, sa fiche et sa pile ; vide pour un mois d'un seul tenant. La paie du mois en est la somme ; la DSN declare
    /// leurs remunerations (51) et leurs bases (78, 79, 81) une par une.
    pub periodes: Vec<Paie>,
    /// Le prelevement a la source de la paie, tel que la validation l'a calcule au taux qu'elle avait (`etats::valider`) ;
    /// `None` pour une paie que rien n'a encore validee.
    pub pas: Option<droit::pas::Versement>,
    /// Le compteur de conges payes que le bulletin montre (`conges::compteur_au_bulletin`), tel que la validation l'a
    /// ecrit ; `None` pour une paie que rien n'a encore validee, et pour un contrat sans compteur de l'employeur.
    pub conges: Option<crate::conges::CompteurConges>,
    /// Le compteur de conges avant le mois (`conges::Avant`), que la validation ecrit avec le compteur du bulletin et que
    /// le mois suivant reprend ; `None` sans compteur.
    pub conges_avant: Option<crate::conges::Avant>,
    /// Le plafond d'un salarie a temps partiel (`plafond_temps_partiel`), tel que le calcul l'a applique : ce que la DSN
    /// declare en S21.G00.40.084.
    pub plafond_temps_partiel: PlafondTempsPartiel,
}

impl Paie {
    /// Les champs sans valeur par defaut ; les autres partent a zero ou vides.
    pub(crate) fn nouveau(
        engagement: Engagement, salarie: SalarieDuContrat, spectacle: Option<droit::fiche::Spectacle>, brut: Montant,
        base_csg: Montant, plafond: Montant, plafond_urssaf: Montant, part_abattue_csg: Montant, smic: MontantUnitaire, heures: Heures,
        plafond_temps_partiel: PlafondTempsPartiel,
    ) -> Paie {
        Paie(Contenu {
            engagement, salarie, spectacle, brut, base_csg, plafond, plafond_urssaf, part_abattue_csg, smic, heures,
            lignes: Vec::new(), total_pat: Montant::ZERO, total_sal: Montant::ZERO, net_a_payer: Montant::ZERO, net_social: Montant::ZERO,
            net_imposable: Montant::ZERO, cout_employeur: Montant::ZERO, numero_bulletin: None,
            bases: BTreeMap::new(), remuneration: Vec::new(), reduction_generale: None,
            deduction_forfaitaire: None, avertissements: Vec::new(), salaire_de_base: Montant::ZERO,
            salaire_retabli: Montant::ZERO, assiette_chomage: Montant::ZERO, heures_travaillees: Heures::ZERO,
            heures_absence: Heures::ZERO, periodes: Vec::new(), pas: None, conges: None, conges_avant: None, plafond_temps_partiel,
        })
    }

    /// LA MEME PAIE, ANNULEE : son contrat, sa fiche et son spectacle, sans aucune ligne, aucune activite ni aucun
    /// montant. Ce qu'est la paie d'un engagement qui n'est plus actif (`etats::NonActif`) : une correction qui la porte
    /// annule tout ce qui a ete declare, et ne declare rien a la place.
    pub fn annulee(&self) -> Paie {
        let mut engagement = self.engagement.clone();
        engagement.lignes.clear();
        (engagement.jours, engagement.cachets, engagement.heures) = (socle::grandeurs::Jours::ZERO, socle::grandeurs::Cachets::ZERO, None);
        let mut p = Paie::nouveau(engagement, self.salarie.clone(), self.spectacle.clone(), Montant::ZERO, Montant::ZERO, Montant::ZERO, self.plafond_urssaf, Montant::ZERO, self.smic, Heures::ZERO, self.plafond_temps_partiel);
        p.0.bases = self.bases.keys().map(|k| (*k, BaseDeCotisation::nulle(*k))).collect();
        p
    }

    /// Ce que la paie porte, a ecrire : le calcul, la relecture, la validation.
    pub(crate) fn ecrire(&mut self) -> &mut Contenu {
        &mut self.0
    }

    /// S21.G00.50.004, le montant net verse : « la remuneration nette fiscale (S21.G00.50.002), de laquelle sont
    /// deduits le montant de la CSG non deductible, le montant de la CRDS et le montant des contributions des employeurs
    /// destinees a financer des garanties « frais de sante » reintegrees dans la base fiscale » (norme DSN 2026.1,
    /// p. 244). Ce n'est PAS le net a payer : ni defraiement, ni acompte, ni indemnite non imposable n'y entrent.
    pub fn net_verse(&self) -> Montant {
        self.net_imposable - reintegrations(&self.lignes)
    }

    /// Le contrat de cette paie est-il declare a la caisse des Conges Spectacles (`caisse_conges::affilie`, D. 7121-41).
    pub fn declaree_a_la_caisse_des_conges(&self) -> R<bool> {
        declaree_a_la_caisse_des_conges(self)
    }

    /// Les lignes saisies de cette paie, une par ligne de remuneration (`minima::saisies_de`).
    #[cfg(feature = "ecran")]
    pub fn saisies(&self) -> Vec<Option<&crate::LignePaie>> {
        crate::minima::saisies_de(self)
    }

    /// Le minimum conventionnel de la ligne `i` de la remuneration (`minima::minimum_de_la_ligne`).
    #[cfg(feature = "ecran")]
    pub fn minimum_de_la_ligne(&self, ctx: &Contexte, i: usize) -> R<Option<droit::minima::Minimum>> {
        crate::minima::minimum_de_la_ligne(ctx, self, i)
    }

    /// Le cachet de repetition de la convention pour ce metier, s'il en a un (`minima::cachet_de_repetition`).
    #[cfg(feature = "ecran")]
    pub fn cachet_de_repetition(&self, ctx: &Contexte) -> R<Option<droit::minima::Minimum>> {
        crate::minima::cachet_de_repetition(ctx, self)
    }

    /// Le titre du spectacle, ou pour un permanent le libelle de sa nature de contrat.
    pub fn titre(&self) -> R<String> {
        match &self.spectacle {
            Some(s) => s.titre.portee().map(str::to_string).ok_or_else(|| socle::fichiers::absent("titre")),
            None => Ok(self.engagement.nat()?.libelle.clone()),
        }
    }

    /// Les paies que la DSN declare une par une : les periodes elementaires d'un mois decoupe, sinon la paie elle-meme.
    pub fn elementaires(&self) -> Vec<&Paie> {
        if self.periodes.is_empty() {
            vec![self]
        } else {
            self.periodes.iter().collect()
        }
    }

    /// La paie est-elle decoupee au jour `j` (une periode elementaire commence ce jour-la, apres le debut du mois) ?
    pub fn decoupee_le(&self, j: &Jour) -> R<bool> {
        for p in self.periodes.iter().skip(1) {
            if p.engagement.periode_payee()?.debut() == *j {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn ligne(&self, id: &IdDeCotisation) -> Option<&LigneCotisation> {
        self.lignes.iter().find(|l| l.est(id))
    }

    /// Ce que la paie paye, en une ligne, la plus courte qu'une personne reconnaisse (`en_bref`).
    pub fn paye_en_bref(&self) -> String {
        en_bref(&self.remuneration)
    }
}

/// Des lignes de remuneration en une ligne, la plus courte qu'une personne reconnaisse : « 4 cachets, 8 repas,
/// 4 nuitées », « 28 h de travail », « salaire mensuel de base, 119 h d'absence maladie, maintien de salaire,
/// indemnités journalières (22 jours) ». Le detail ligne a ligne est celui du bulletin (`LigneBrut::designation`).
/// - Les lignes d'une meme rubrique s'additionnent.
/// - La part d'un repas ou d'une nuitee au-dela de sa limite (`REPAS_AU_DELA`, `NUITEE_AU_DELA`) est celle d'une ligne deja comptee.
/// - Deux rubriques qui ne different que par leur precision entre parentheses, sur la meme mesure, se disent une fois :
///   les indemnites journalieres deduites puis versees.
/// - Ce qui rend, juste apres elle, ce qu'une deduction a retire (la meme mesure) se dit sans la repeter : le maintien
///   de salaire d'une absence, l'indemnite des conges pris.
pub fn en_bref(lignes: &[LigneBrut]) -> String {
    let mut groupes: Vec<LigneBrut> = Vec::new();
    for l in lignes.iter().filter(|l| l.code != crate::NUITEE_AU_DELA && l.code != crate::REPAS_AU_DELA) {
        // Deux quantites d'une meme rubrique sont de la meme grandeur ; deux grandeurs ne s'ajouteraient pas, et se diraient
        // a part.
        match groupes.iter_mut().find(|g| g.code == l.code && g.precision == l.precision).and_then(|g| g.paiement.plus(l.paiement).map(|p| (g, p))) {
            Some((g, p)) => {
                g.paiement = p;
                g.montant += l.montant;
            }
            None => groupes.push(l.clone()),
        }
    }
    let base = |l: &LigneBrut| l.libelle.split_once(" (").map(|(b, _)| b.to_string());
    let meme_mesure = |a: &LigneBrut, b: &LigneBrut| a.unite == b.unite && a.paiement.quantite_ou_base() == b.paiement.quantite_ou_base();
    let mut dits = Vec::new();
    let mut i = 0;
    while i < groupes.len() {
        let g = &groupes[i];
        let jumelle = groupes.get(i + 1).filter(|s| meme_mesure(g, s) && base(g).is_some() && base(g) == base(s));
        if let (Some(_), Some(commune)) = (jumelle, base(g)) {
            let mut une = g.clone();
            une.libelle = commune;
            une.precision = None;
            dits.push(bref(&une));
            i += 2;
            continue;
        }
        let rend = i > 0 && groupes[i - 1].montant < Montant::ZERO && g.montant >= Montant::ZERO && meme_mesure(&groupes[i - 1], g);
        dits.push(if rend { g.libelle.to_lowercase() } else { bref(g) });
        i += 1;
    }
    dits.join(", ")
}

/// Une ligne (deja additionnee) dite en bref : un forfait qui se compte dit son nombre (« 8 repas »), des heures de
/// travail se disent en heures (« 28 h de travail ») ; le reste comme au bulletin.
fn bref(l: &LigneBrut) -> String {
    let Some(q) = l.paiement.quantite().map(QuantiteDeLigne::au_centieme) else { return l.designation().to_lowercase() };
    let (tete, reste) = match l.libelle.split_once(' ') {
        Some((t, r)) => (t, r),
        None => (l.libelle.as_str(), ""),
    };
    let reste = if reste.is_empty() { String::new() } else { format!(" {}", reste) };
    let dit = if l.unite == Unite::Forfait && q.entiere() && q.au_pluriel() {
        let tete = if tete.ends_with('s') || tete.ends_with('x') { tete.to_string() } else { format!("{}s", tete) };
        format!("{} {}{}", q.dit(), tete, reste)
    } else if l.unite == Unite::Heure && tete.to_lowercase() == "heure" {
        format!("{} h{}", q.dit(), reste)
    } else {
        l.designation()
    };
    dit.to_lowercase()
}

// ------------------------------------------------------ les allegements

/// La ligne du bulletin qui porte la reduction generale `rg` d'un brut `brut`.
pub(crate) fn ligne_de_reduction(rg: &ReductionGenerale, brut: Montant) -> LigneCotisation {
    LigneCotisation {
        id: droit::forme_bareme::REDUCTION_GENERALE,
        famille: droit::forme_bareme::FAMILLE_ALLEGEMENTS,
        libelle: "Réduction générale dégressive unique".into(),
        base: BaseDeCotisation::Montant(brut),
        taux_pat: None,
        montant_pat: -rg.montant,
        taux_sal: None,
        montant_sal: Montant::ZERO,
        masquer_base_et_taux: true,
    }
}

/// Les bases de l'option de l'employeur (fiche entreprise, `deduction_forfaitaire_option`) : arrete du 4 septembre 2025
/// relatif aux frais professionnels, art. 9, I, al. 2 et 3 (`atelier/reference/sources/dfs/`). « L'employeur peut opter
/// pour la deduction forfaitaire specifique lorsqu'une convention ou un accord collectif l'a explicitement prevu ou
/// lorsque le comite d'entreprise, le comite social et economique ou les delegues du personnel ont donne leur accord. A
/// defaut, il appartient a chaque salarie d'accepter ou non cette option. » (`droit::fiche::OptionDfs`.)
///
/// L'accord du salarie, quand l'option repose sur lui (fiche salarie, `deduction_forfaitaire_accord`) : art. 9, I,
/// al. 3, « Celle-ci peut alors figurer soit dans le contrat de travail ou soit dans un avenant au contrat de travail. A
/// defaut, l'employeur informe et recueille le consentement annuel du salarie par tout moyen. Lorsque le travailleur
/// salarie et assimile ne repond pas dans un delai raisonnable a cette consultation, son silence vaut accord. Le salarie
/// peut toutefois demander a tout moment a son employeur a beneficier ou ne plus beneficier de la deduction, avec
/// application au 1er janvier de l'annee suivante. » Le consentement est annuel : la fiche le date (son historique),
/// `droit::fiche::AccordDfs`.
///
/// LA DFS QUI VAUT POUR CE SALARIE : sa profession, son taux de l'annee, les lignes abattues, le plafond annuel, les
/// remboursements de frais que l'annexe 2 de l'arrete laisse hors de l'assiette et la tolerance du BOSS qui y laisse les
/// autres. Une autre notion que l'option de l'employeur (`droit::fiche::OptionDfs`, la case de sa fiche), qui n'en est
/// qu'une condition.
#[derive(Debug, Clone, PartialEq)]
pub struct DfsDuSalarie {
    pub profession: droit::fiche::ProfessionDfs,
    pub categorie: droit::forme_bareme::CategorieDfs,
    pub taux: Taux,
    pub rubriques: Vec<IdDeCotisation>,
    pub plafond_annuel: Montant,
    /// Les rubriques de frais (`defraiement_forfait`, `repas`, `nuitee`, `defraiement_reel`) que l'annexe 2 de l'arrete (I, 2° et 5°) laisse
    /// hors de l'assiette pour cette profession ; tout autre remboursement de frais se reintegrerait (art. 9, I, al. 4).
    pub frais_hors_assiette: Vec<droit::catalogue::CodeDeRubrique>,
    /// La tolerance du BOSS, Frais professionnels, § 2320 (bareme, `frais_cumulables`) : pour les huit secteurs, dont le
    /// spectacle vivant et le spectacle enregistre, << l'ensemble des remboursements de frais professionnels definis par
    /// l'arrete du 4 septembre 2025 peut faire l'objet d'un cumul avec la deduction forfaitaire specifique >>
    /// (`atelier/reference/sources/boss/boss-frais-professionnels-20261013.txt`, l. 2917-2921).
    pub frais_cumulables: bool,
}

/// UN REFUS DU CALCUL QUI PORTE A PART le champ a dire, quand il y en a un, et le detail, pour le developpeur.
#[track_caller]
fn refus_detaille(phrase: impl Into<String>, cle: Option<&str>, detail: String) -> Echec {
    refus_avec(phrase, Lieu { cle: cle.map(str::to_string), detail: Some(detail), ..Lieu::default() })
}

/// La DFS que la fiche exerce, ou `None`. La decision vit ici (regle 2) : la profession de la fiche salarie
/// (`deduction_forfaitaire`), l'option de l'employeur (fiche entreprise, `deduction_forfaitaire_option`) et, quand
/// l'option repose sur chaque salarie, son accord (`deduction_forfaitaire_accord`). Arrete du 4 septembre 2025, art. 9
/// (`atelier/reference/sources/dfs/arrete-2025-frais-professionnels/`) ; les professions, l'article 5 de l'annexe IV du
/// CGI dans sa redaction en vigueur au 31/12/2000 (`atelier/reference/sources/dfs/cgi-annexe4-art5/`) et l'annexe 1 de
/// l'arrete ; leur taux de l'annee, le bareme (art. 9, II et III). Une section du bareme non marquee verifiee refuse.
pub(crate) fn deduction_forfaitaire(bar: &Pile, sal: &ChampsSalarie, ent: &ChampsEntreprise) -> R<Option<DfsDuSalarie>> {
    let Some(profession) = sal.deduction_forfaitaire.dite().copied() else {
        return Ok(None);
    };
    let qui = crate::qui_de(sal)?;
    let annee = bar.annee;
    let dfs = bar.deduction_forfaitaire.as_ref();
    let professions = dfs.map(|d| &d.professions);
    let connues = || match professions {
        Some(p) => {
            // Dans l'ordre de leurs noms, comme le bareme les ecrit (la cle se trie dans l'ordre de sa declaration).
            let mut noms: Vec<&str> = p.keys().map(|k| k.as_str()).collect();
            noms.sort_unstable();
            noms.join(", ")
        }
        None => "aucune".to_string(),
    };
    let Some(fiche_prof) = professions.and_then(|p| p.get(&profession)) else {
        return Err(socle::refus!(
            "Deduction forfaitaire specifique de {} : la fiche dit << {} >>, que le bareme {} ne connait pas. La deduction \
est attachee a une PROFESSION (arrete du 4 septembre 2025, art. 9, I : << les professions, prevues a l'article 5 de \
l'annexe IV du code general des impots dans sa redaction en vigueur au 31 decembre 2000 >>), et c'est elle qui dit le \
taux et les frais que l'annexe 2 de l'arrete laisse hors de l'assiette : choisir la profession ({}).",
            qui, profession, annee, connues()
        ));
    };
    let dfs = dfs.ok_or_else(|| socle::echec::cle("Le barème de l'année ne porte pas la déduction forfaitaire spécifique : le signaler au développeur.", "deduction_forfaitaire"))?;
    // L'option de l'employeur, et l'accord qu'elle demande.
    match ent.deduction_forfaitaire_option.dite() {
        None => {
            return Err(socle::refus!(
                "Deduction forfaitaire specifique de {} : sa fiche dit qu'il l'exerce ({}), mais la compagnie n'a pas \
opte pour elle (fiche entreprise, << Deduction forfaitaire specifique >>). C'est une option de l'EMPLOYEUR (arrete du \
4 septembre 2025, art. 9, I, al. 2) : par un accord collectif qui l'a explicitement prevue, avec l'accord du comite \
social et economique, ou, a defaut, salarie par salarie. Rien n'est calcule.",
                qui, profession
            ))
        }
        Some(OptionDfs::AccordCollectif | OptionDfs::AccordCse) => {}
        Some(OptionDfs::AccordIndividuel) => match sal.deduction_forfaitaire_accord.dite() {
            Some(AccordDfs::Contrat | AccordDfs::ConsentementAnnuel) => {}
            Some(AccordDfs::Refus) => return Ok(None),
            None => {
                return Err(socle::refus!(
                    "Deduction forfaitaire specifique de {} : la compagnie l'exerce salarie par salarie, et la fiche ne \
dit pas l'accord de celui-ci. L'arrete du 4 septembre 2025 (art. 9, I, al. 3) veut qu'il l'accepte, << soit dans le \
contrat de travail ou soit dans un avenant >>, a defaut par un consentement annuel recueilli << par tout moyen >> (<< son \
silence vaut accord >>). Dire lequel ({}, {}) ou son refus ({}). Rien n'est calcule.",
                    qui, AccordDfs::Contrat, AccordDfs::ConsentementAnnuel, AccordDfs::Refus
                ))
            }
        },
    }
    let categorie = fiche_prof.categorie.clone();
    // Le taux de l'annee : celui de la profession quand l'arrete lui donne un calendrier a part (art. 9, III, pour les
    // professions hors spectacle vivant), sinon celui de sa categorie (art. 9, II).
    let taux = fiche_prof.taux.or_else(|| dfs.taux.get(&categorie).copied()).ok_or_else(|| {
        socle::refus!(
            "Deduction forfaitaire specifique de {} ({}) : le bareme {} n'a pas de taux pour la categorie << {} >>.",
            qui, profession, annee, categorie
        )
    })?;
    Ok(Some(DfsDuSalarie {
        profession,
        categorie,
        taux,
        rubriques: dfs.rubriques_abattues.clone(),
        plafond_annuel: dfs.plafond_annuel,
        frais_hors_assiette: fiche_prof.frais_hors_assiette.clone(),
        frais_cumulables: dfs.frais_cumulables,
    }))
}

/// Ce que le moteur sait calculer sous DFS, et ce qu'il refuse en disant pourquoi.
/// - un contrat paye au mois (permanent, CDD de droit commun) : l'assiette de son chomage (reglement general, art. 49,
///   << entrant dans l'assiette des cotisations de securite sociale >>, sans le << avant application de l'abattement >>
///   des annexes VIII et X), sa prevoyance conventionnelle (3090 : << servant de base aux cotisations de la securite
///   sociale >> ; 1285 : rien), ses tranches de retraite et son assiette minimum mensuelle ne sont pas portees ;
/// - un cadre : l'Apec n'a pas de texte depose qui dise son assiette sous DFS ;
/// - un remboursement de frais que l'annexe 2 de l'arrete ne laisse pas hors de l'assiette, quand le bareme ne porte pas
///   la tolerance du BOSS (`frais_cumulables`) : l'article 9 (I, al. 4) le reintegre << y compris, le cas echeant, les
///   indemnites versees [...] a titre de remboursement des frais professionnels >>, et ce que la reintegration fait a
///   l'assiette chomage et a la DSN (type 35) n'est pas porte. Avec la tolerance (BOSS, Frais professionnels, § 2320,
///   `atelier/reference/sources/boss/boss-frais-professionnels-20261013.txt`, l. 2917-2921 : << l'ensemble des
///   remboursements de frais professionnels definis par l'arrete du 4 septembre 2025 peut faire l'objet d'un cumul avec
///   la deduction forfaitaire specifique >>), tout remboursement de frais reste hors de l'assiette abattue.
/// Une ligne hors assiette se classe par ce qu'elle est, la nature de sa rubrique (`Rubrique::hors_assiette`), pas par
/// son code : un remboursement de frais d'une rubrique de la compagnie est un remboursement de frais.
/// - une ligne hors assiette qui n'est pas un remboursement de frais (une indemnite) : la tolerance du BOSS ne la couvre
///   pas, et le rescrit BOSS-RES-000010 (meme page, § 2325, l. 2923-2934) veut une indemnite de transport conventionnelle
///   << dans l'assiette des cotisations et contributions sociales des le 1er euro prealablement a l'application de
///   l'abattement >> ; l'art. 9, I, al. 4 compose l'assiette du << montant global des remunerations, indemnites, primes
///   [...] >>. Aucun texte depose ne dit si une autre indemnite hors assiette (une indemnite de rupture exoneree par sa
///   propre regle) s'y reintegre : refus (regle 1). Un acompte, un mouvement de tresorerie, n'est rien de tout cela.
fn dfs_applicable(
    eng: &Engagement, nat: &Nature, o: &DfsDuSalarie, lb: &[LigneBrut], sal: &ChampsSalarie, racine: &std::path::Path,
    jour: &Jour,
) -> R<()> {
    let qui = eng.qui();
    if nat.mensuelle() {
        return Err(refus_detaille(format!("{} : la déduction forfaitaire spécifique ne se porte que sur un contrat d'usage, pas sur un contrat payé au mois.", qui), None, format!(
            "{} : une deduction forfaitaire specifique sur un contrat paye au mois ({}). Le logiciel ne la porte que sur \
un contrat d'usage : pour un salarie au mois, l'assiette du chomage (reglement general d'assurance chomage, art. 49), sa \
prevoyance conventionnelle, ses tranches de retraite et son assiette minimum ne sont pas encore calculees sous deduction \
(atelier/reference/sources/dfs/LISEZ-MOI.md, << Ce qui manque >>). Rien n'est calcule.",
            qui, nat.libelle
        )));
    }
    // Un cadre (S21.G00.40.003 = 01) : la notion qui fait ajouter au bareme les lignes de l'Apec (`bareme_cadre`).
    if droit::bareme::est_cadre(sal) {
        return Err(refus_detaille(format!("{} : la déduction forfaitaire spécifique ne se porte pas sur la paie d'un cadre : aucun texte ne dit l'assiette de l'Apec sous déduction.", qui), None, format!(
            "{} : une deduction forfaitaire specifique sur la paie d'un cadre (statut categoriel 01, qui lui fait cotiser a \
l'Apec). L'assiette de l'Apec sous deduction n'est dans aucun texte depose (atelier/reference/sources/dfs/LISEZ-MOI.md, \
<< Ce qui manque >>) : le logiciel ne la devine pas. Rien n'est calcule.",
            qui
        )));
    }
    for l in lb.iter().filter(|l| !l.soumis && !l.montant.est_nul()) {
        let genre = rubrique(racine, &l.code, jour)?.hors_assiette()?;
        if genre == HorsAssiette::Revenu {
            return Err(refus_detaille(format!("{} : la ligne « {} » est hors assiette sans être un remboursement de frais, et il est sous déduction forfaitaire spécifique : la soumettre, ou en faire un remboursement de frais.", qui, l.libelle), None, format!(
                "{} : la ligne << {} >> est hors assiette et n'est pas un remboursement de frais, pour un salarie sous \
deduction forfaitaire specifique ({}). La tolerance du BOSS (Frais professionnels, § 2320) ne laisse hors de l'assiette \
que les remboursements de frais au sens de l'arrete du 4 septembre 2025 ; le rescrit BOSS-RES-000010 (§ 2325) veut une \
indemnite de transport conventionnelle dans l'assiette << des le 1er euro prealablement a l'application de \
l'abattement >>, et aucun texte depose ne dit ce que devient une autre indemnite hors assiette. Le logiciel ne le \
devine pas : si c'est un remboursement de frais, sa rubrique est de nature `frais` ; si elle entre dans l'assiette, \
elle est soumise. Rien n'est calcule.",
                qui, l.libelle, o.profession
            )));
        }
        let frais = genre == HorsAssiette::Frais;
        if frais && !o.frais_cumulables && !o.frais_hors_assiette.contains(&l.code) {
            return Err(refus_detaille(format!("{} : la ligne « {} » rembourse des frais sous déduction forfaitaire spécifique, et le logiciel ne porte pas leur réintégration dans l'assiette.", qui, l.libelle), None, format!(
                "{} : la ligne << {} >> rembourse des frais a un salarie sous deduction forfaitaire specifique ({}). \
L'arrete du 4 septembre 2025 (art. 9, I, al. 4) les reintegre alors dans l'assiette avant l'abattement, sauf ceux de \
la liste de son annexe 2 : pour cette profession, {} ; et le bareme ne porte pas la tolerance du BOSS (Frais \
professionnels, § 2320, `frais_cumulables`) qui les laisserait tous hors de l'assiette. Le logiciel ne porte pas \
cette reintegration (assiette du chomage, bloc 54 de type 35) : rien n'est calcule.",
                qui,
                l.libelle,
                o.profession,
                if o.frais_hors_assiette.is_empty() {
                    "aucun".to_string()
                } else {
                    o.frais_hors_assiette.iter().map(droit::catalogue::CodeDeRubrique::as_str).collect::<Vec<_>>().join(", ")
                }
            )));
        }
    }
    Ok(())
}

/// Ce que l'annee civile a deja deduit pour ce salarie avant ce contrat : les mois anterieurs de la meme annee
/// (`conges::dfs_des_mois_anterieurs` : leurs paies validees, et pour les mois d'un ancien logiciel la reprise des
/// cumuls de la fiche) et les contrats precedents du meme mois (`precedentes`).
/// Arrete du 4 septembre 2025, art. 9, I, al. 1 : << dans la limite de 7 600 euros par annee civile >>. INFERENCE, ecrite comme
/// telle : l'annee est celle de la periode de paie, et les contrats d'un mois se suivent dans l'ordre de la saisie.
fn dfs_deja_deduite(ctx: &Contexte, eng: &Engagement, sal: &FicheSalarie, precedentes: &[Paie]) -> R<Montant> {
    let mut deja = crate::conges::dfs_des_mois_anterieurs(ctx, eng.periode_payee()?.fin().mois_civil(), &sal.id)?;
    for p in precedentes {
        let sienne = p.salarie.id == sal.id || p.engagement.salarie == eng.salarie;
        if sienne {
            if let Some(d) = &p.deduction_forfaitaire {
                deja += d.montant;
            }
        }
    }
    Ok(deja)
}

/// La deduction d'un contrat, et les assiettes abattues de ses rubriques de salaire (dans l'ordre de `montants`) ; ce
/// qu'elle a a en dire (le plafond annuel, l'assiette minimum) s'ajoute a `avis`. Chaque rubrique de salaire abattue du
/// taux de l'annee (arrondie au centime),
/// puis
/// - le plafond annuel : << dans la limite de 7 600 euros par annee civile >> (arrete du 4 septembre 2025, art. 9, I,
///   al. 1) ;
///   `deja` est ce que l'annee a deja deduit ;
/// - l'assiette minimum : << L'application de ces dispositions s'entend sans prejudice du sixieme alinea de l'article
///   R. 242-1 du code de la securite sociale >> (art. 9, I, dernier alinea) ; R. 242-1, al. 6 : la base << ne peut etre inferieure,
///   en aucun cas, au montant cumule [...] du salaire minimum de croissance >> ; BOSS, assiette generale, § 410 : le
///   SMIC, sans les elements conventionnels. `assiette_minimale` est le SMIC du contrat, celui que la DSN retient (SMIC
///   horaire x heures, `calculer` 7.5). INFERENCE pour un cachet : ses heures sont celles que le moteur lui donne pour
///   la DSN (`heures_par_cachet`), aucun texte depose ne dit les heures travaillees d'un cachet.
/// Quand l'une ou l'autre ramene la deduction, l'assiette abattue se repartit entre les rubriques au prorata de leur
/// montant, le dernier centime sur la derniere.
pub(crate) fn abattement_dfs(
    o: &DfsDuSalarie, montants: &[Montant], brut: Montant, deja: Montant, assiette_minimale: Montant, avis: &mut Vec<String>,
) -> (DeductionForfaitaire, Vec<Montant>) {
    let zero = Montant::ZERO;
    // Ce que la rubrique garde : un moins le taux de l'annee.
    let facteur = Coefficient::UN - o.taux.en_coefficient();
    let theoriques: Vec<Montant> = montants.iter().map(|m| (*m * facteur).au_centime()).collect();
    let pleine = brut - theoriques.iter().fold(zero, |a, b| a + *b);
    let reste = zero.max(o.plafond_annuel - deja);
    let mut retenue = pleine.min(reste);
    if retenue < pleine {
        avis.push(format!(
            "Déduction forfaitaire spécifique ramenée de {} à {} : le plafond de {} par année civile (arrêté du \
4 septembre 2025, art. 9) est atteint, {} déjà déduits cette année.",
            euros(pleine),
            euros(retenue),
            euros(o.plafond_annuel),
            euros(deja)
        ));
    }
    if brut - retenue < assiette_minimale {
        let plancher = zero.max(brut - assiette_minimale).min(retenue);
        avis.push(format!(
            "Déduction forfaitaire spécifique ramenée de {} à {} : l'assiette abattue ne descend pas sous le SMIC du \
contrat, {} (CSS R. 242-1, al. 6 ; arrêté du 4 septembre 2025, art. 9, I, dernier alinéa).",
            euros(retenue),
            euros(plancher),
            euros(assiette_minimale)
        ));
        retenue = plancher;
    }
    let assiettes = if retenue == pleine || brut <= zero {
        theoriques
    } else {
        let cible = brut - retenue;
        let mut out = Vec::with_capacity(montants.len());
        let mut cumul = zero;
        for (i, m) in montants.iter().enumerate() {
            let a = if i + 1 == montants.len() { cible - cumul } else { cible.prorata(*m, brut).au_centime() };
            cumul += a;
            out.push(a);
        }
        out
    };
    let deduction = DeductionForfaitaire {
        categorie: o.categorie.clone(),
        profession: o.profession,
        taux: o.taux,
        rubriques: o.rubriques.clone(),
        montant: brut - assiettes.iter().fold(zero, |a, b| a + *b),
        deja,
    };
    (deduction, assiettes)
}

// ---------------------------------------------------------------- calculer

/// Les assiettes d'un montant de salaire (§7.4) : celles qui se calculent par rubrique de salaire
/// (`Assiette::par_rubrique`).
struct Assiettes {
    brut: Montant,
    brut_cs: Montant,
    csg: Montant,
    x115: Montant,
    x110: Montant,
    dfs: Option<Montant>,
    /// Les assiettes majorees apres la deduction forfaitaire specifique : la majoration s'applique au brut abattu
    /// (lettre circulaire Acoss 2013-019, annexe 1, R6 ; fiche Audiens « Les Conges Spectacles », 2023 : 334,50 =
    /// 300 x 1,115 ; atelier/reference/sources/majoration-caisses-conges/).
    x115_dfs: Option<Montant>,
    x110_dfs: Option<Montant>,
    /// La part de la tranche 1 de l'annee d'un intermittent qui revient a ce montant (7.6) : un salarie au mois n'en a pas.
    t1_annuelle: Option<Montant>,
}

impl Assiettes {
    /// L'assiette `a` de ce montant de salaire, s'il la porte : une assiette globale n'est pas celle d'une rubrique.
    fn de(&self, a: Assiette) -> Option<Montant> {
        match a {
            Assiette::Brut => Some(self.brut),
            Assiette::T1Annuelle => self.t1_annuelle,
            Assiette::BrutCs => Some(self.brut_cs),
            Assiette::BrutX115 => Some(self.x115),
            Assiette::BrutX110 => Some(self.x110),
            Assiette::BrutDfs => self.dfs,
            Assiette::BrutX115Dfs => self.x115_dfs,
            Assiette::BrutX110Dfs => self.x110_dfs,
            Assiette::Csg
            | Assiette::T1Journaliere
            | Assiette::T2Journaliere
            | Assiette::T2PrevoyanceJournaliere
            | Assiette::Apec
            | Assiette::ApecT2Journaliere
            | Assiette::Cet
            | Assiette::T2Annuelle
            | Assiette::CetAnnuelle
            | Assiette::T1Mensuelle
            | Assiette::T1MensuelleX115
            | Assiette::T2Mensuelle
            | Assiette::ApecMensuelle
            | Assiette::CetMensuelle
            | Assiette::T1T2Mensuelle
            | Assiette::T2Conventionnelle
            | Assiette::PssPeriode
            | Assiette::BrutChomage
            | Assiette::BrutPlafonne
            | Assiette::BrutX115Plafonne
            | Assiette::BrutPlafonneDfs
            | Assiette::BrutX115PlafonneDfs
            | Assiette::Zero
            | Assiette::RemunerationPercueHorsEna
            | Assiette::RemunerationHabituelleHorsEna
            | Assiette::HeuresEnPlus
            | Assiette::HeuresEnPlusHeures
            | Assiette::HeuresEnPlusExonere
            | Assiette::IjSubrogeesNettes
            | Assiette::IjSubrogeesAssiettePas => None,
        }
    }
}

fn pss_du_contexte(ctx: &Contexte) -> impl Fn(socle::mois::Annee) -> socle::echec::R<droit::plafond::Pss> + '_ {
    move |a| ctx.plafond_securite_sociale(a)
}

/// `bareme_applique(&ctx.bareme(fin), sal, nat, fin, caisse)`, garde : la pile ne
/// depend que des fichiers du bareme, de la fiche entreprise, du salarie, de
/// la nature, de la caisse de conges et du jour. Un mois et ses mois anterieurs la recomposaient a
/// chaque paie, et c'etait l'essentiel du temps de calcul.
pub(crate) fn bareme_du_salarie(ctx: &Contexte, sal: &SalarieDuContrat, nat: &Nature, fin: &Jour, caisse: bool, cpf_cdd: bool, majoration: bool) -> R<Arc<Pile>> {
    pile(ctx, sal, nat, fin, caisse, cpf_cdd, majoration, false)
}

/// LE PLAFOND JOURNALIER DE LA BASE CONGES SPECTACLES d'un contrat (`droit::clauses::plafond_conges_spectacles`) : sur la
/// convention du contrat, celle que la DSN declare (S21.G00.40.017) et que le bulletin imprime
/// (`minima::idcc_du_contrat`), la case de la fiche Entreprise et le metier de la fiche. La paie (`calculer_periode`) et le
/// certificat d'un mois importe (`routes::dossier_salarie`) le lisent ici.
pub fn plafond_conges_spectacles(ctx: &Contexte, spectacle: Option<&droit::fiche::Spectacle>, sal: &SalarieDuContrat) -> R<Option<socle::grandeurs::MontantUnitaire>> {
    let idcc_du_contrat = match ctx.entreprise.idcc.dite() {
        Some(i) => Some(crate::minima::idcc_du_contrat(i, spectacle, sal)?),
        None => None,
    };
    droit::clauses::plafond_conges_spectacles(idcc_du_contrat.as_ref(), ctx.entreprise.plafond_conges_spectacles, sal.code_emploi_ft.dite().filter(|c| !c.est_permanent()))
}

/// La pile d'une paie IMPORTEE, qui ne range que ses bases : ses taux se lisent sur ses pieces
/// (`bareme::EntrepriseLue::taux_lus`).
pub(crate) fn bareme_pour_ranger(ctx: &Contexte, sal: &SalarieDuContrat, nat: &Nature, fin: &Jour, caisse: bool, majoration: bool) -> R<Arc<Pile>> {
    pile(ctx, sal, nat, fin, caisse, false, majoration, true)
}

fn pile(ctx: &Contexte, sal: &SalarieDuContrat, nat: &Nature, fin: &Jour, caisse: bool, cpf_cdd: bool, majoration: bool, taux_lus: bool) -> R<Arc<Pile>> {
    type Cache = Mutex<HashMap<String, Arc<Pile>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let cle = format!("{}|{}|{}|{:?}|{}|{}|{}|{}|{}", ctx.empreinte_bareme(fin)?, fin.iso(), nat.cle, sal, nat.mensuelle(), caisse, cpf_cdd, majoration, taux_lus);
    if let Some(v) = cache.lock().unwrap().get(&cle) {
        return Ok(v.clone());
    }
    let ent = droit::bareme::EntrepriseLue { taux_lus, ..ctx.entreprise_lue(fin)? };
    let bar = Arc::new(bareme_applique(&*ctx.bareme(fin)?, &ent, sal, nat, Some(fin), caisse, cpf_cdd, majoration)?);
    socle::fichiers::ranger(cache, cle, bar.clone());
    Ok(bar)
}

/// La fiche salarie telle qu'elle vaut pour l'engagement `eng`, emploi compris (`emplois::emploi_de`). Un contrat
/// d'usage la lit telle qu'au premier jour du contrat (un nouveau contrat est un nouveau contrat) ; un contrat paye au
/// mois, telle qu'a la fin de la periode payee (`emplois::emploi_du_mois`). La paie (`calculer`) et ce qui lit un contrat
/// sans le payer (`amorcage::embauche` : la DPAE, le signalement d'amorcage, les emplois multiples de la DSN) la
/// lisent ici, et pas autrement : deux lectures d'un meme contrat feraient deux emplois.
pub(crate) fn fiche_de_l_engagement(ctx: &Contexte, eng: &Engagement) -> R<SalarieDuContrat> {
    let nat = eng.nat()?;
    let cle = &eng.salarie;
    crate::fiche_du_salarie(ctx, cle)?;
    let emploi = &eng.emploi;
    let fiche = if let Some(au_mois) = nat.au_mois() {
        // La fiche de la derniere periode elementaire : un mois decoupe declare la derniere valeur connue
        // (norme DSN 2026.1, § 2.1.2.4, p. 22) ; un mois d'un seul tenant, la periode de paie entiere. La dispense de
        // sante se lit sur ces memes bornes (`dispense_de_la_periode`).
        let derniere = *crate::periodes::bornes(ctx, eng)?.last().ok_or_else(|| refus("aucune periode de paie"))?;
        let du_mois = droit::emplois::emploi_du_mois(ctx, cle, &derniere)?;
        let fiche = dispense_de_la_periode(emploi_de(&du_mois, emploi)?, &au_mois, &derniere)?;
        sante_de_la_suspension(ctx, eng, &au_mois, convention_de_l_employeur(ctx, fiche))?
    } else {
        convention_de_l_employeur(ctx, emploi_de(&ctx.salarie_au(cle, &eng.contrat().debut())?, emploi)?)
    };
    let mut fiche = etablissement_du_contrat(fiche, eng);
    // L'embauche que ce contrat est (`DebutDeLAnciennete::Embauche`) date l'anciennete que la fiche ne dit pas encore ;
    // une anciennete ni dite ni lue reste vide, et ce qui la lit refuse en la nommant.
    match DebutDeLAnciennete::de(&ctx.racine, &fiche.fiche.champs, cle, eng.contrat().debut())? {
        DebutDeLAnciennete::Embauche(j) => fiche.fiche.champs.date_anciennete = Some(j),
        DebutDeLAnciennete::Dite(_) | DebutDeLAnciennete::NonDite => {}
    }
    Ok(fiche)
}

/// LA CONVENTION DE L'EMPLOYEUR sur la fiche que la paie lit (`droit::caisse_conges::IDCC_DE_L_EMPLOYEUR`) : l'IDCC de
/// l'entreprise, telle que `ctx` la lit. Le domaine d'activite de l'annexe 8 se lit a cette convention
/// (`droit::caisse_conges::intermittent`).
pub(crate) fn convention_de_l_employeur(ctx: &Contexte, fiche: FicheSalarie) -> SalarieDuContrat {
    let idcc = ctx.entreprise.idcc.dite().filter(|i| !i.is_empty()).cloned();
    SalarieDuContrat { idcc_de_l_employeur: idcc, ..SalarieDuContrat::de(fiche) }
}

/// L'ETABLISSEMENT DE RATTACHEMENT du salarie pour ce contrat (S21.G00.11.001, « l'etablissement de rattachement
/// administratif du salarie », `norme-dsn/texte/bloc-S21.G00.11.txt`) : celui que l'engagement dit, quand la compagnie en a
/// plusieurs ; sinon le principal. La fiche le porte en `etablissement`, que la DSN, les signalements et le bareme lisent
/// (`droit::bareme::etablissement_du_salarie`).
pub fn etablissement_du_contrat(mut fiche: SalarieDuContrat, eng: &Engagement) -> SalarieDuContrat {
    fiche.etablissement = eng.etablissement.clone();
    fiche
}

/// LE JOUR D'OU PART L'ANCIENNETE DANS L'ENTREPRISE d'un salarie pour un contrat (S21.G00.86.003, « l'anciennete dans l'entreprise » :
/// `norme-dsn/texte/bloc-S21.G00.86.txt` ; la 3090, annexe I, art. 3.11, compte des « ans de presence » :
/// `kali-3090-titres-vii-x-xi-annexes-20260924.md`). Seule la compagnie la connait (la case « Dans la compagnie depuis »,
/// `date_anciennete`), sauf un fait que le moteur lit : un salarie qu'aucun engagement anterieur ne nomme chez la
/// compagnie, saisi ou importe, entre chez elle par ce contrat, et son anciennete part de son premier jour. INFERENCE :
/// l'anciennete dans l'entreprise commence a l'entree dans l'entreprise. La validation et le calcul la lisent ici
/// (`routes::fiches::poser_anciennete_a_l_embauche`, `fiche_de_l_engagement`), et nulle part ailleurs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DebutDeLAnciennete {
    /// La date que la fiche dit.
    Dite(Jour),
    /// Aucun engagement anterieur : ce contrat est l'embauche, qui commence ce jour-la.
    Embauche(Jour),
    /// Ni dite, ni lue : un engagement anterieur nomme le salarie, la fiche ne dit pas depuis quand il est la.
    NonDite,
}

impl DebutDeLAnciennete {
    /// D'ou part l'anciennete de `fiche` (le salarie de cle `cle`) pour un contrat qui commence le `debut`.
    pub fn de(racine: &std::path::Path, fiche: &ChampsSalarie, cle: &IdSalarie, debut: Jour) -> R<DebutDeLAnciennete> {
        if let Some(d) = fiche.date_anciennete.dite() {
            return Ok(DebutDeLAnciennete::Dite(*d));
        }
        let anterieur = crate::stock::objets(racine)?
            .iter()
            .any(|o| o.carte().salarie.as_ref() == Some(cle) && o.carte().debut.is_some_and(|d| d < debut));
        Ok(if anterieur { DebutDeLAnciennete::NonDite } else { DebutDeLAnciennete::Embauche(debut) })
    }

    /// Le jour d'ou elle part ; non dite, le calcul qui la lit refuse en nommant la case.
    pub fn jour(self) -> R<Jour> {
        match self {
            DebutDeLAnciennete::Dite(j) | DebutDeLAnciennete::Embauche(j) => Ok(j),
            DebutDeLAnciennete::NonDite => Err(socle::echec::cle(
                "La fiche du salarié ne dit pas depuis quand il est dans la compagnie, et un engagement d'avant le nomme : le dire sur sa fiche (« Dans la compagnie depuis »).",
                "date_anciennete",
            )),
        }
    }
}

/// LE STATUT DU SALARIE, artiste ou technicien, que sa fiche dit : il choisit la section du bareme (l'abattement de
/// l'arrete du 24/01/1975 ne vise que les artistes), le plafond, la regle d'un mois incomplet. Une fiche qui ne le dit
/// pas se refuse : rien ne le devine.
pub(crate) fn statut_de(sal: &ChampsSalarie) -> R<Statut> {
    sal.statut.dite().copied().ok_or_else(|| {
        socle::echec::cle("La fiche du salarié ne dit pas son statut, artiste ou technicien : le dire sur sa fiche.", "statut")
    })
}


/// La paie d'un engagement, les paies deja calculees du meme mois en main (`engagement::charger_donnees`, dans l'ordre de la
/// saisie) : la deduction forfaitaire specifique d'un salarie se plafonne par annee civile, contrats precedents du mois
/// compris (`dfs_deja_deduite`). Un mois d'un seul tenant se calcule en une fois (`calculer_periode`) ; un mois ou
/// le statut, la categorie ou la quotite d'un contrat paye au mois change en cours de periode se decoupe en periodes
/// elementaires (`periodes::bornes`), chacune calculee de meme, les periodes deja calculees comptant parmi les
/// precedentes, et la paie du mois en est la somme (`periodes::assembler`).
///
/// Un maintien de salaire garanti en net (`maintien::au_net`) se reconstitue par ce meme calcul, refait sur les lignes du
/// mois jusqu'a ce que le net garanti soit atteint au centime ; la paie est alors celle de ce montant. Les jours de ses
/// suites reduisent le plafond tant que son complement net n'est pas connu (`maintien::AuNet::jours`) : quand la
/// reconstitution le trouve positif, ils sont maintenus (BOSS, assiette generale, § 820), le plafond les rend, et la
/// reconstitution se refait sur ce plafond. Sa garantie, calculee sur le mois sans les jours d'arret parmi les jours sans
/// remuneration (`maintien::Reconstitution::arrets`), est la meme aux deux passages ; les jours rendus le restent.
pub(crate) fn calculer_apres(ctx: &Contexte, eng: &Engagement, precedentes: &[Paie]) -> R<Paie> {
    let reconstituer = |e: &Engagement| {
        let mut paie_des = |lignes: Vec<crate::LignePaie>, hors: &std::collections::BTreeSet<socle::date::Jour>| {
            let mut x = e.clone();
            x.lignes = lignes;
            if !hors.is_empty() {
                let sans = x.sans_remuneration.as_ref().ok_or_else(|| socle::fichiers::absent("sans_remuneration"))?;
                x.sans_remuneration = Some(sans.hors(hors));
            }
            calculer_du_mois(ctx, &x, precedentes)
        };
        crate::maintien::au_net(&e.lignes, &mut paie_des)
    };
    let Some(mut r) = reconstituer(eng)? else { return calculer_du_mois(ctx, eng, precedentes) };
    let mut e = eng.clone();
    if !r.maintenus.is_empty() {
        let sans = e.sans_remuneration.as_ref().ok_or_else(|| socle::fichiers::absent("sans_remuneration"))?;
        e.sans_remuneration = Some(sans.hors(&r.maintenus));
        r = reconstituer(&e)?.ok_or_else(|| socle::refus!("{} : la ligne du maintien a reconstituer en net a disparu entre deux calculs de la paie.", eng.qui()))?;
    }
    e.lignes = crate::maintien::avec_le_montant(&e.lignes, r.montant);
    let mut p = calculer_du_mois(ctx, &e, precedentes)?;
    p.ecrire().avertissements.push(r.avis);
    Ok(p)
}

/// La paie d'un mois, d'un seul tenant ou en periodes elementaires.
fn calculer_du_mois(ctx: &Contexte, eng: &Engagement, precedentes: &[Paie]) -> R<Paie> {
    let bornes = crate::periodes::bornes(ctx, eng)?;
    if bornes.len() <= 1 {
        let (mut p, annee) = calculer_periode(ctx, eng, precedentes)?;
        let avis = avis_du_mois(eng, &p, annee.as_slice());
        p.ecrire().avertissements.extend(avis);
        return Ok(p);
    }
    let mut parts: Vec<Paie> = Vec::new();
    // L'avis de la bascule de la CET va a la paie du mois, une fois, net de ses periodes (`avis_du_mois`).
    let mut annees: Vec<AnneeDeLaPeriode> = Vec::new();
    for e in crate::periodes::engagements_des_periodes(ctx, eng, &bornes)? {
        let avant: Vec<Paie> = precedentes.iter().chain(parts.iter()).cloned().collect();
        let (p, annee) = calculer_periode(ctx, &e, &avant)?;
        parts.push(p);
        annees.extend(annee);
    }
    let mut p = crate::periodes::assembler(ctx, eng, parts)?;
    let avis = avis_du_mois(eng, &p, &annees);
    p.ecrire().avertissements.extend(avis);
    Ok(p)
}

/// L'assiette d'une rubrique : le calcul ne cotise pas sur une assiette que la rubrique ne dit pas.
fn base_de(r: &Rubrique) -> R<Assiette> {
    r.base.ok_or_else(|| socle::fichiers::absent("base"))
}

/// Le taux patronal d'une rubrique, si elle en dit un (0 % est un taux).
fn part_patronale(r: &Rubrique) -> Option<Taux> {
    r.taux_pat.map(|t| t.0)
}

/// La pile de bareme sur laquelle se calcule la paie `p` (celle de `calculer_periode`, du meme cache).
pub(crate) fn pile_de_la_paie(ctx: &Contexte, p: &Paie) -> R<Arc<Pile>> {
    let eng = &p.engagement;
    let nat = eng.nat()?;
    let caisse = droit::caisse_conges::affilie(&nat, &p.salarie, &eng.contrat())?;
    let cpf_cdd = droit::cpf_cdd::ouvre_cpf_cdd(
        eng.nature,
        eng.motif_recours,
        &crate::numeros::le_contrat(eng.numero.as_ref(), &eng.salarie),
    )?;
    let majoration = droit::majoration_chomage::due(&nat, eng.motif_recours, &eng.contrat())?;
    bareme_du_salarie(ctx, &p.salarie, &nat, &eng.periode_payee()?.fin(), caisse, cpf_cdd, majoration)
}

/// Les garanties de prevoyance de la paie `p` : celles de sa pile de bareme (`Pile::garanties_de_prevoyance`).
pub fn garanties_de_prevoyance(ctx: &Contexte, p: &Paie) -> R<Vec<IdDeCotisation>> {
    Ok(pile_de_la_paie(ctx, p)?.garanties_de_prevoyance())
}

/// LA DISPENSE DE SANTE A DES BORNES. Les accords de branche deposes en
/// donnent : la dispense de l'assure individuel « ne peut jouer que jusqu'a l'echeance du contrat individuel », celle
/// d'une couverture collective se justifie chaque annee, et le salarie « cessant de justifier » de son cas adhere
/// (CCN 3090, art. 9.2 et 9.2.2 b et d, `atelier/reference/sources/kali-3090-accord-sante-permanents-20240719-20261011.md` ;
/// CCNEAC XII.2.1.8.1 b, `kali-1285-accord-frais-de-sante-20260130-20261011.md` l. 78). La fiche porte le motif (`sante_dispense`) et, s'il y a lieu, `sante_dispense_depuis` et
/// `sante_dispense_jusqu_au` ; hors de ces bornes, le salarie est affilie : la fiche que la paie lit perd le motif, et
/// le bareme (`bareme_sante`) comme la DSN (`dsn::affiliations_du_permanent`) le traitent en affilie.
///
/// La periode payee entiere dans la dispense : dispense ; entiere dehors : affilie. Une borne en cours de periode
/// (une dispense qui commence avec le contrat n'en est pas une) refuse : le partage de la cotisation du mois entre
/// dispense et affiliation n'est dit par aucun texte depose (regle 1).
pub(crate) fn dispense_de_la_periode(mut fiche: FicheSalarie, au_mois: &AuMois, payee: &Periode) -> R<FicheSalarie> {
    let sienne = fiche.permanent.ouvrir(au_mois);
    if sienne.sante_dispense.remplie().is_none() {
        return Ok(fiche);
    }
    let depuis = sienne.sante_dispense_depuis.dite().copied();
    let jusqu_au = sienne.sante_dispense_jusqu_au.dite().copied();
    if depuis.is_some_and(|d| d > payee.fin()) || jusqu_au.is_some_and(|j| j < payee.debut()) {
        fiche.champs.permanent.ouvrir_mut(au_mois).sante_dispense = None;
        return Ok(fiche);
    }
    let borne = match (depuis.filter(|d| *d > payee.debut()), jusqu_au.filter(|j| *j < payee.fin())) {
        (Some(d), _) => Some(format!("commence le {}", d.jj_mm_aaaa())),
        (None, Some(j)) => Some(format!("finit le {}", j.jj_mm_aaaa())),
        (None, None) => None,
    };
    if let Some(borne) = borne {
        let qui = [fiche.prenom.portee(), fiche.nom.portee()].into_iter().flatten().filter(|v| !v.is_empty()).collect::<Vec<_>>().join(" ");
        return Err(socle::refus!(
            "{} : sa dispense de complementaire sante {}, en cours de periode payee (du {} au {}). Le moteur ne partage \
pas la cotisation sante d'un mois entre dispense et affiliation : aucun texte depose ne dit comment. Dater la borne de \
la dispense au premier jour d'un mois (fiche salarie, « Dispense depuis ») ou au dernier jour d'un mois (« Dispense \
jusqu'au »), si l'organisme et la convention le permettent ; sinon c'est a trancher. Rien n'est calcule.",
            if qui.is_empty() { "Ce salarie".to_string() } else { qui },
            borne,
            payee.debut().jj_mm_aaaa(),
            payee.fin().jj_mm_aaaa()
        ));
    }
    Ok(fiche)
}

/// LA COMPLEMENTAIRE SANTE D'UN MOIS ENTIEREMENT SUSPENDU SANS INDEMNISATION. Un permanent dont toute la
/// periode payee est couverte par des suspensions non remunerees (conge parental 632, sabbatique 639, sans solde 501... :
/// `imprevus::suspendu_sans_remuneration` ; un arret de travail n'en est pas une) :
///
/// - **3090** : « Le benefice des garanties [...] est suspendu pour les salaries permanents dont le contrat de travail est
///   suspendu et ne donne lieu a aucune indemnisation » ; le salarie qui veut rester affilie paie « integralement » les
///   deux parts, que « l'organisme assureur preleve [...] directement aupres du salarie » (accord du 19/07/2024, etendu,
///   art. 4.2 et 15, `atelier/reference/sources/kali-3090-accord-sante-permanents-20240719-20261011.md`). Ni cotisation ni
///   part patronale sur la paie de ce mois, donc rien a reintegrer au net fiscal : la fiche porte `sante_suspendue`.
/// - **toute autre convention** (1285 : l'accord du 30/01/2026 ne parle que de la suspension « indemnisee » ; un contrat
///   d'entreprise) : AUCUN TEXTE DEPOSE ne dit ce que devient la sante ; le droit commun laisse le maintien facultatif
///   (instruction DSS/3C/5B/2021/127, II.B, `atelier/reference/sources/suspension-non-remuneree/dss-instruction-3c5b-2021-
///   127-suspension-contrat-prevoyance-20210617.txt`) et c'est le contrat d'adhesion de la compagnie qui tranche. La fiche
///   entreprise le dit (`SANTE_SUSPENSION`) : « suspendue », comme la 3090 ; « maintenue », la paie cotise comme un mois
///   ordinaire (la part patronale se reintegre alors au net fiscal, CGI 83, 1° quater, et le montant soumis au PAS reste a
///   0.00 : `pas::du_versement`). Sans reponse, refus (regle 1).
///
/// La prevoyance n'a rien a suspendre ici : 3090 et 1285 l'assoient sur la remuneration (tranche A, tranche 1 ;
/// `legifrance-3090-prevoyance-titre12-20260919.md`, `legifrance-1285-prevoyance-titre12-20260919.md`), nulle ce
/// mois-la -- inference : aucun texte depose ne traite de la suspension non indemnisee pour la prevoyance.
fn sante_de_la_suspension(ctx: &Contexte, eng: &Engagement, au_mois: &AuMois, mut fiche: SalarieDuContrat) -> R<SalarieDuContrat> {
    if fiche.permanent.ouvrir(au_mois).sante_dispense.remplie().is_some() {
        return Ok(fiche);
    }
    let cle = fiche.id.to_string();
    let payee = eng.periode_payee()?;
    let fin = payee.fin();
    let suspensions = crate::imprevus::suspendu_sans_remuneration(&ctx.racine, &fiche.id, eng.numero.as_ref(), &eng.contrat().debut(), &payee)?;
    let Some(premiere) = suspensions.first() else { return Ok(fiche) };
    let motifs: Vec<&str> = suspensions.iter().filter_map(|s| s.suspension()).map(|s| s.motif.code()).collect();
    let ent = ctx.entreprise_au(&fin);
    let idcc = ent.idcc.dite();
    let pourquoi = if idcc == Some(&droit::fiche::IDCC_3090) {
        format!(
            "contrat suspendu sans indemnisation tout le mois (motif {}) : les garanties de sante sont suspendues (CCN 3090, \
accord sante du 19/07/2024, art. 4.2 et 15)",
            motifs.join(", ")
        )
    } else {
        match ent.permanent.ouvrir(au_mois).sante_suspension_non_indemnisee.dite() {
            Some(droit::fiche::SanteSuspension::Suspendue) => format!(
                "contrat suspendu sans indemnisation tout le mois (motif {}) : les garanties de sante sont suspendues (fiche \
entreprise, « {} »)",
                motifs.join(", "),
                SANTE_SUSPENSION_SUSPENDUE
            ),
            Some(droit::fiche::SanteSuspension::Maintenue) => return Ok(fiche),
            None => {
                let qui = [fiche.prenom.portee(), fiche.nom.portee()].into_iter().flatten().filter(|v| !v.is_empty()).collect::<Vec<_>>().join(" ");
                return Err(refus_detaille(format!("{} : son contrat est suspendu sans indemnisation, et la fiche entreprise ne dit pas ce que devient sa complémentaire santé : le dire dans Configuration › Entreprise, « Santé d'un permanent suspendu sans indemnisation ».", if qui.is_empty() { cle.clone() } else { qui.clone() }), Some(SANTE_SUSPENSION.nom()), format!(
                    "{} : son contrat est suspendu sans indemnisation sur toute la periode payee (du {} au {} ; suspension {} \
depuis le {}). Que devient sa complementaire sante ce mois-la ? Sous la convention {}, aucun texte ne le dit : c'est le \
contrat de sante de la compagnie qui tranche (le maintien est facultatif, instruction DSS du 17/06/2021, II.B). A dire\
une fois pour toutes : Configuration › Entreprise, « Santé d'un permanent suspendu sans indemnisation » (`{}`) -- « {} » \
si le contrat suspend les garanties (rien sur la paie), « {} » s'il les maintient avec les cotisations. Rien n'est calcule.",
                    if qui.is_empty() { cle.clone() } else { qui },
                    payee.debut().jj_mm_aaaa(),
                    fin.jj_mm_aaaa(),
                    motifs.join(", "),
                    premiere.debut().jj_mm_aaaa(),
                    match idcc {
                        Some(i) => &**i,
                        None => "de la compagnie",
                    },
                    SANTE_SUSPENSION.nom(),
                    SANTE_SUSPENSION_SUSPENDUE,
                    SANTE_SUSPENSION_MAINTENUE
                )));
            }
        }
    };
    fiche.sante_suspendue = Some(pourquoi);
    Ok(fiche)
}

/// Le contrat de cette paie est-il declare a la caisse des Conges Spectacles (`caisse_conges::affilie`, D. 7121-41) :
/// son contrat et la fiche qu'elle a lue.
pub(crate) fn declaree_a_la_caisse_des_conges(p: &Paie) -> R<bool> {
    let e = &p.engagement;
    droit::caisse_conges::affilie(&*e.nat()?, &p.salarie, &e.contrat())
}

/// La meme question pour la saisie `e` d'un contrat paye au mois, avant que son engagement ne soit construit (le solde de
/// tout compte de `conges::lignes_du_mois`) : sa fiche telle que sa paie la lira au dernier jour du contrat, le metier que
/// la saisie dit l'emportant (`fiche_de_l_engagement`, `emploi_de`).
pub(crate) fn saisie_declaree_a_la_caisse_des_conges(ctx: &Contexte, e: &crate::saisie::Carte, fin: &Jour) -> R<bool> {
    let nat = e.nature_lue()?;
    // Le solde lit le debut que la carte ecrit (`conges::debut_de`) : sans lui, il refuse, meme quand un jour est joue.
    let contrat = match e.debut {
        Some(_) => e.contrat()?,
        None => None,
    }
    .ok_or_else(|| socle::fichiers::absent("debut"))?;
    let cle = e.salarie.as_ref().ok_or_else(|| socle::fichiers::absent("salarie"))?;
    let code_emploi_ft = e.emploi.as_ref().and_then(|x| x.code_emploi_ft.clone()).or_else(|| e.metier.as_ref().map(droit::fiche::CodeEmploiFt::from));
    let emploi = EmploiEngagement { ecrit: EmploiEcrit { code_emploi_ft, ..Default::default() }, emploi_affichage: None };
    let fiche = convention_de_l_employeur(ctx, emploi_de(&ctx.salarie_au(cle, fin)?, &emploi)?);
    droit::caisse_conges::affilie(&nat, &fiche, &contrat)
}

/// LA PERIODE D'ESSAI QUE L'ENGAGEMENT STIPULE ne depasse pas la duree maximale de son contrat (`droit::essai` : C. trav.
/// L. 1242-10 pour un CDD, L. 1221-19 pour un CDI, la convention 3090 art. 7.4 pour un artiste en CDI). Sa categorie se lit
/// sur la fiche du salarie telle que l'engagement la voit (`fiche_de_l_engagement`).
fn essai_dans_son_plafond(ctx: &Contexte, eng: &Engagement, nat: &Nature, sal: &ChampsSalarie) -> R<()> {
    let Some(jours) = eng.ecrit.periode_essai_jours else { return Ok(()) };
    let artiste = statut_de(sal)? == Statut::Artiste;
    let plafond = droit::essai::plafond(nat, &eng.contrat(), sal.statut_conventionnel.dite().copied(), artiste, ctx.entreprise.idcc.dite())?;
    droit::essai::refuser_au_dela(jours, plafond.as_ref(), &format!("{} ({})", crate::qui_de(sal)?, nat.libelle))
}

/// Le SMIC de la reduction generale d'un mois ou une absence retire du salaire.
pub(crate) struct SmicDUneAbsence {
    /// La remuneration percue hors elements non affectes par l'absence (S21.G00.51.011 = 028).
    pub percue: Montant,
    /// La remuneration habituelle du mois complet hors ces elements (S21.G00.51.011 = 029).
    pub habituelle: Montant,
    /// Les heures dont le SMIC horaire fait le SMIC du mois, quand le bareme dit le SMIC horaire de la reduction.
    pub heures: Option<Heures>,
}

/// CSS D. 241-7, IV, dernier alinea (`atelier/reference/sources/code-de-la-securite-sociale/xml/D241-7.xml`) : quand le
/// contrat est suspendu sans paiement de la remuneration ou avec un paiement partiel, « la fraction du montant du salaire
/// minimum a retenir pour le mois ou a lieu l'absence est corrigee selon le rapport entre les revenus d'activite [...] dus
/// et ceux qui auraient ete dus si le salarie avait ete present tout le mois, hors elements de remuneration qui ne sont pas
/// affectes par l'absence », l'affectation s'entendant « d'une proratisation strictement proportionnelle au temps
/// d'absence » ; un paiement integral garde le SMIC entier (alinea precedent). La fiche net-entreprises 2681
/// (`atelier/reference/sources/net-entreprises-base-connaissances/net-entreprises-fiche2681-reduction-generale-absence-
/// proratisation-smic-types-028-029-20260609.txt`, exemples « Prime d'anciennete et maladie » et « garantie sur le net »)
/// met au numerateur le salaire, l'absence, le maintien et les IJSS brutes deduites, hors IJSS subrogees non soumises, et
/// au denominateur le salaire du mois complet ; « SMIC = 1766.92 x 1 104.27 / 1 820.04 ».
///
/// Dans ce calcul, une absence ne proratise que le salaire de base : les lignes `retabli` (l'absence, le maintien, les IJSS
/// brutes) se calculent sur lui (`maintien`, `imprevus`). Le numerateur est donc le salaire de base et ces lignes, le
/// denominateur le salaire de base. Le SMIC du mois du salaire de base (`smic_horaire` x ses heures, au centime) prend le
/// rapport, comme la fiche l'ecrit ; les autres heures payees (en plus du mois) restent entieres. `heures` est ce SMIC
/// rapporte au SMIC horaire : la reduction le retrouve (`droit::reduction`). Sans ligne `retabli` non nulle : aucun rapport.
pub(crate) fn smic_d_une_absence(lb: &[LigneBrut], smic_horaire: Option<MontantUnitaire>) -> Option<SmicDUneAbsence> {
    let rien: Montant = socle::grandeurs::nulle();
    let montants = |f: &dyn Fn(&LigneBrut) -> bool| lb.iter().filter(|l| f(l)).fold(rien, |a, l| a + l.montant);
    let heures = |f: &dyn Fn(&LigneBrut) -> bool| lb.iter().filter(|l| f(l)).fold(Heures::ZERO, |a, l| a + l.heures_annexes);
    if !lb.iter().any(|l| l.retabli && !l.montant.est_nul()) {
        return None;
    }
    let habituelle = montants(&|l| l.salaire_de_base);
    if habituelle <= rien {
        return None;
    }
    let percue = rien.max(habituelle + montants(&|l| l.retabli && l.soumis));
    // Le SMIC du salaire au rapport de la remuneration percue a l'habituelle (non nulle, vu plus haut), en heures au SMIC
    // horaire (positif, `filter`) : jamais `None`.
    let heures = smic_horaire.filter(|s| *s > MontantUnitaire::ZERO).and_then(|s| {
        let smic_du_salaire = (heures(&|l| l.salaire_de_base) * s).au_centime();
        let autres = heures(&|l| !l.salaire_de_base && !l.retabli);
        smic_du_salaire.prorata(percue, habituelle).en_heures(s).map(|h| Heures::ZERO.max(h + autres))
    });
    Some(SmicDUneAbsence { percue, habituelle, heures })
}

/// La paie d'un engagement sur sa periode (la periode de paie, ou une periode elementaire), dans l'ordre du §7. La
/// configuration de la compagnie se lit a la fin de cette periode (`Contexte::au`).
pub(crate) fn calculer_periode(ctx: &Contexte, eng: &Engagement, precedentes: &[Paie]) -> R<(Paie, Option<AnneeDeLaPeriode>)> {
    let nat = eng.nat()?;
    let mut avis_dfs: Vec<String> = Vec::new();
    let fin_periode = eng.periode_payee()?.fin();
    let ctx = &ctx.au(&fin_periode)?;
    let sal = fiche_de_l_engagement(ctx, eng)?;
    essai_dans_son_plafond(ctx, eng, &nat, &sal)?;
    let spec = if nat.mensuelle() {
        None
    } else {
        Some(crate::fiche_du_spectacle(ctx, eng.spectacle.as_ref().ok_or_else(|| socle::fichiers::absent("spectacle"))?)?)
    };
    // Le contrat est-il declare a la caisse des Conges Spectacles ? (`caisse_conges::affilie` : un CDDU de moins de
    // douze mois, un CDD de droit commun de moins de douze mois d'un artiste.)
    let caisse = droit::caisse_conges::affilie(&nat, &sal, &eng.contrat())?;
    // Le contrat ouvre-t-il la contribution CPF-CDD (C. trav. L. 6331-6) ? Un CDD de droit commun, ni saisonnier,
    // ni contrat aide (`droit::cpf_cdd`, la regle du bloc 44).
    let cpf_cdd = droit::cpf_cdd::ouvre_cpf_cdd(
        eng.nature,
        eng.motif_recours,
        &crate::numeros::le_contrat(eng.numero.as_ref(), &eng.salarie),
    )?;
    let majoration = droit::majoration_chomage::due(&nat, eng.motif_recours, &eng.contrat())?;
    let bar = bareme_du_salarie(ctx, &sal, &nat, &fin_periode, caisse, cpf_cdd, majoration)?;
    let rubriques_bulletin = &bar.rubriques_bulletin;
    // Les jours du contrat, que les plafonds journaliers et les heures d'un cachet multiplient.
    let jours = eng.jours;

    // 7.2 : les lignes de remuneration.
    let lb = lignes_brut(eng, ctx)?;
    let soumises: Vec<&LigneBrut> = lb.iter().filter(|l| l.soumis).collect();
    if caisse {
        // La caisse paie ses conges (D. 7121-41) : l'employeur ne les paie pas en plus.
        let conges = |c: &droit::catalogue::CodeDeRubrique| matches!(c.as_str(), "absence_conges_payes" | "indemnite_compensatrice_conges") || droit::catalogue::RubriqueDuMois::IndemniteCongesPayes.nomme(c);
        if let Some(l) = lb.iter().find(|l| conges(&l.code)) {
            return Err(refus_detaille("Cette ligne paie des congés à un salarié déclaré à la caisse des Congés Spectacles : c'est la caisse qui les lui verse ; retirer la ligne.", None, format!(
                "{} : la ligne << {} >> paie des conges a un salarie que l'employeur declare a la caisse des \
Conges Spectacles (code du travail D. 7121-41 ; `caisse_conges::affilie`) : c'est la caisse qui les lui verse. Les payer aussi au bulletin serait les payer deux fois. Refus.",
                eng.qui(),
                l.code
            )));
        }
    }
    let brut = soumises.iter().fold(Montant::ZERO, |a, l| a + l.montant);
    // Le plafond de la paie, le meme partout (`plafond_de_l_engagement`) : corrige a la quotite d'un temps partiel, sauf
    // renonciation de l'etablissement ou artiste (`plafond_temps_partiel`). Les assiettes plafonnees qu'il donne se
    // calculent a un seul endroit (`droit::plafond::assiettes_plafonnees`), regularisees sur l'annee civile pour une paie
    // qui se regularise (`plafonnement`).
    let plafond_urssaf = plafond_de_l_engagement(ctx, eng, &sal)?;
    let plafonnement = plafonnement(ctx, eng, &nat, &sal, &bar.regularisation_du_plafond, precedentes)?;
    let limite_csg = bar.limite_abattement_csg.plafonds;
    let plafonnees = droit::plafond::assiettes_plafonnees(brut, plafond_urssaf, limite_csg, &bar.tranches_du_plafond, &plafonnement);
    let regularisees = plafonnees.regularisees;

    // 7.3 : la prevoyance dans l'assiette CSG.
    let (prev, taux_prev) = match bar.rubrique(&PREVOYANCE) {
        Some(p @ Rubrique { taux_pat: Some(t), .. }) => (p, t.0),
        _ => {
            return Err(refus_detaille("Le barème de l'année n'a pas de prévoyance avec un taux patronal : le signaler au développeur.", None, format!(
                "Bareme {}{} : aucune rubrique `prevoyance` avec un taux \
                 patronal. Elle entre dans l'assiette CSG (98,25 % du brut + \
                 part patronale de prevoyance) : sans elle la CSG serait \
                 sous-declaree sans que rien ne le signale.",
                bar.annee,
                if nat.bareme().is_some() { format!(", nature '{}'", nat.cle) } else { String::new() }
            )))
        }
    };
    let plafond_journalier = bar.plafond_journalier;
    // La tranche 1 journaliere du contrat : le plafond journalier par jour du contrat, dans la limite que le bareme de
    // l'annee dit (`limite_du_plafond_journalier` : en 2026, « dans la limite du plafond mensuel », Audiens, Parametres),
    // pour le mois civil, tous les contrats d'intermittent du salarie confondus : ce que les contrats precedents du meme
    // mois (`precedentes`, dans l'ordre de la saisie) ont REELLEMENT cotise dans chaque tranche se deduit du plafond
    // mensuel de cette tranche (un plafond pour la tranche 1, huit pour les tranches 1 et 2, quatre pour l'Apec et la
    // tranche B), et ce contrat prend le reste, dans la limite de ses propres plafonds journaliers. Ainsi, dans le mois,
    // la tranche 1 ne depasse pas le plafond mensuel et rien n'echappe a la tranche 2 sous huit plafonds. Ce que
    // « mensuel » couvre, le texte ne le dit pas : la lecture et son INFERENCE sont ecrites dans la note de
    // `limite_du_plafond_journalier` du bareme 2026. Une paie d'intermittent est celle d'un contrat dans un mois civil
    // (`Engagement::periode_payee`, ses journees datees dans la periode : `droit::plafond::pour_paie`) : un contrat a
    // cheval sur deux mois a une paie par mois, chacune sous le plafond mensuel de son mois.
    // Un salarie au mois n'a pas de tranche journaliere.
    struct DejaDansLeMois {
        mensuel: Montant,
        t1: Montant,
        t1_et_t2: Montant,
        apec: Montant,
        t1_et_tranche_b: Montant,
    }
    let limite_journaliere = match bar.limite_du_plafond_journalier.as_ref().map(|l| l.dans_la_limite_du).filter(|_| !nat.mensuelle()) {
        Some(droit::forme_bareme::PlafondQuiLimite::PlafondMensuel) => {
            let mensuel = bar.plafond_securite_sociale.as_ref().ok_or_else(|| socle::fichiers::absent("plafond_securite_sociale"))?.mensuel;
            let mut d = DejaDansLeMois { mensuel, t1: Montant::ZERO, t1_et_t2: Montant::ZERO, apec: Montant::ZERO, t1_et_tranche_b: Montant::ZERO };
            for p in precedentes.iter().filter(|p| p.salarie.id == sal.id || p.engagement.salarie == eng.salarie) {
                let base = |a: Assiette| p.bases.get(&a).map(|b| b.en_euros(a)).transpose();
                // Une paie sans tranche journaliere est celle d'un contrat au mois : elle n'en consomme pas.
                let Some(t1) = base(Assiette::T1Journaliere)? else { continue };
                d.t1 += t1;
                // Un non-cadre n'a cotise que dans sa tranche 1 journaliere.
                d.t1_et_t2 += match base(Assiette::T2Journaliere)? {
                    Some(t2) => t1 + t2,
                    None => t1,
                };
                d.apec += match base(Assiette::Apec)? {
                    Some(a) => a,
                    None => t1,
                };
                d.t1_et_tranche_b += match base(Assiette::T2PrevoyanceJournaliere)? {
                    Some(b) => t1 + b,
                    None => t1,
                };
            }
            Some(d)
        }
        None => None,
    };
    // Le montant que ce contrat peut cotiser jusqu'a `n` plafonds : `n` fois ses plafonds journaliers, dans la limite de
    // ce que le mois laisse (`deja` : ce que les contrats precedents y ont cotise).
    let jusqu_a = |p: socle::grandeurs::MontantUnitaire, n: Option<Coefficient>, deja: fn(&DejaDansLeMois) -> Montant| -> Montant {
        let journalier = (jours * p).au_centime();
        let (du_contrat, du_mois) = match n {
            Some(n) => (journalier * n, limite_journaliere.as_ref().map(|d| d.mensuel * n)),
            None => (journalier, limite_journaliere.as_ref().map(|d| d.mensuel)),
        };
        match (limite_journaliere.as_ref(), du_mois) {
            (Some(d), Some(du_mois)) => du_contrat.min(Montant::ZERO.max(du_mois - deja(d))),
            _ => du_contrat,
        }
    };
    let t1_du_contrat = |p: socle::grandeurs::MontantUnitaire| -> Montant { jusqu_a(p, None, |d| d.t1) };
    // La base de la prevoyance de la paie : sa tranche 1 quand le bareme l'y pose (sur le plafond journalier d'un
    // intermittent, que le bareme dit alors ; ou celle d'un salarie au mois, `regularisees`, la tranche que sa cotisation
    // prend), le brut sinon.
    let base_prevoyance = match prev.base {
        Some(Assiette::T1Journaliere) => brut.min(t1_du_contrat(plafond_journalier.ok_or_else(|| socle::fichiers::absent("plafond_journalier"))?)),
        Some(Assiette::T1Mensuelle) if nat.mensuelle() => regularisees.t1,
        _ => brut,
    };
    // Les autres garanties de prevoyance, sur une tranche 2 : leur part patronale entre aussi dans l'assiette (les
    // contributions de l'employeur aux garanties collectives, CSS L136-1-1 : plus bas). Ce sont les lignes que la pile
    // range avec la prevoyance (`Pile::garanties_de_prevoyance`), toutes couches comprises : la tranche B d'un cadre
    // technicien de la CCNEAC aussi, qu'aucune section du S21.G00.54 de type 93 ne nomme.
    let garanties = bar.garanties_de_prevoyance();
    let mut tranches_prev = Montant::ZERO;
    for r in rubriques_bulletin.iter().filter(|r| r.id != PREVOYANCE && garanties.contains(&r.id)) {
        let (id, Some(taux)) = (&r.id, part_patronale(r)) else { continue };
        let base = base_de(r)?;
        let base_t2 = if let (Assiette::T2PrevoyanceJournaliere, Some(pj)) = (base, plafond_journalier) {
            let t1 = t1_du_contrat(pj);
            Some(Montant::ZERO.max(brut.min(jusqu_a(pj, Some(bar.tranches_du_plafond.t2_conventionnelle), |d| d.t1_et_tranche_b)) - t1))
        } else if base == Assiette::T2Conventionnelle && nat.mensuelle() {
            Some(regularisees.t2_conventionnelle)
        } else {
            None
        };
        let Some(base_t2) = base_t2 else {
            return Err(refus_detaille("Le barème de l'année porte une prévoyance que le calcul de la CSG ne sait pas lire : le signaler au développeur.", None, format!(
                "Bareme {} : la ligne de prevoyance `{}` porte l'assiette \
                 `{}`, que le calcul de l'assiette CSG ne sait pas lire. Sa \
                 part patronale doit y entrer : sans elle la CSG serait \
                 sous-declaree.",
                bar.annee, id, base
            )));
        };
        tranches_prev += base_t2.fois_taux(taux);
    }
    // La part patronale de la mutuelle : un forfait sur le plafond MENSUEL de la periode, sans la correction du temps
    // partiel (`plafond_de_la_sante`).
    let mutuelle = bar.rubrique(&MUTUELLE).and_then(part_patronale);
    let pss_sante = if nat.mensuelle() { Some(plafond_de_la_sante(ctx, eng)?) } else { None };
    if let (Some(m), Some(pss)) = (mutuelle, pss_sante) {
        tranches_prev += pss.fois_taux(m).au_centime();
    }

    // 7.4 : les assiettes, par rubrique de salaire. La CSG abat la part du brut sous quatre plafonds, le plafond de la
    // vieillesse, regularisee sur l'annee comme lui (CSS L. 136-2, I ; BOSS, assiette generale, § 290 :
    // `regularisees.abattue_csg`, `Pile::assiette_csg`).
    let abattue_csg = regularisees.abattue_csg;
    let (maj115, maj110) = (bar.majoration_conges_115, bar.majoration_conges_110);
    let assiettes = |montant: Montant, cs: bool| -> Assiettes {
        // La part de la base de la prevoyance qui revient a cette rubrique, au prorata de son montant dans le brut.
        let base_prev = if brut > Montant::ZERO { base_prevoyance.prorata(montant, brut) } else { montant };
        let base_prev_t2 = if !tranches_prev.est_nul() && brut > Montant::ZERO {
            tranches_prev.prorata(montant, brut)
        } else {
            Montant::ZERO
        };
        Assiettes {
            brut: montant,
            brut_cs: if cs { montant } else { Montant::ZERO },
            csg: (bar.assiette_csg(montant, brut, abattue_csg) + base_prev.fois_taux(taux_prev) + base_prev_t2).au_centime(),
            x115: (montant * maj115).au_centime(),
            x110: (montant * maj110).au_centime(),
            dfs: None,
            x115_dfs: None,
            x110_dfs: None,
            t1_annuelle: None,
        }
    };
    // L'assiette des Conges Spectacles : les rubriques que le catalogue y met, d'un contrat que l'employeur declare a la
    // caisse -- et d'aucun autre (un CDI, un contrat de douze mois ou plus : `caisse_conges::affilie`).
    let mut par_rubrique: Vec<Assiettes> = soumises.iter().map(|l| assiettes(l.montant, l.conges_spectacles && caisse)).collect();
    // UN MOIS SANS SALAIRE OU LA PART PATRONALE RESTE DUE (une sante maintenue pendant une suspension non indemnisee :
    // `sante_de_la_suspension`, « maintenue »). Les contributions de l'employeur aux garanties collectives entrent dans
    // l'assiette de la CSG (CSS L136-1-1, I, « toutes les sommes [...] dues [...] a l'occasion d'un travail », dont le III, 2°,
    // n'exclut pas les contributions aux garanties collectives : `atelier/reference/sources/suspension-non-remuneree/legi-extrait-code-de-la-
    // securite-sociale-20261015.md`) ; elles se repartissent au prorata du brut de chaque rubrique, et un brut nul ne les
    // repartit nulle part : elles vont entieres a la premiere rubrique soumise, plutot que de sortir de l'assiette. La part
    // patronale de la tranche 1 de la prevoyance aussi : regularisee, la tranche 1 d'un mois sans brut peut etre positive
    // (la somme des plafonds grandit plus que celle des remunerations), et sa cotisation est due. De meme, l'abattement
    // qu'une paie sans brut regularise va entier a cette rubrique. Sans aucune rubrique soumise, rien ne se pose : la part
    // abattue ecrite est celle qui s'est appliquee, nulle (`abattue_appliquee`), et la paie suivante de l'annee rattrape.
    let mut abattue_appliquee = abattue_csg;
    if brut.est_nul() {
        match par_rubrique.first_mut() {
            Some(premiere) => premiere.csg = (premiere.csg + tranches_prev + base_prevoyance.fois_taux(taux_prev) - bar.abattement_de(abattue_csg)).au_centime(),
            None => abattue_appliquee = Montant::ZERO,
        }
    }
    let zero = Montant::ZERO;
    let base_csg = par_rubrique.iter().fold(zero, |a, b| a + b.csg);

    // 7.5 : le SMIC retenu pour la DSN, en montant unitaire des sa lecture dans le contexte (`Contexte::smic`).
    let smic = ctx.smic(&fin_periode)?;
    let heures_annexes = lb.iter().fold(Heures::ZERO, |a, l| a + l.heures_annexes);
    // Le plafond d'un intermittent : ses heures, dans la limite des heures d'un cachet par jour du contrat.
    let heures = if nat.mensuelle() { Heures::ZERO.max(heures_annexes) } else { heures_annexes.min(jours * bar.heures_par_cachet) };
    let plafond = (heures * smic).au_centime();

    // 7.5 bis : la deduction forfaitaire specifique (arrete du 4 septembre 2025, art. 9 ; `abattement_dfs`). Elle abat
    // l'assiette des lignes que le bareme nomme, rubrique de salaire par rubrique de salaire, apres le plafond annuel
    // et sans descendre sous l'assiette minimum ; les assiettes majorees partent du brut abattu (Acoss 2013-019, R6).
    let mut dfs: Option<DeductionForfaitaire> = None;
    // Le brut abattu (la somme des assiettes abattues), sur lequel se calcule la reduction generale (7.9).
    let mut brut_abattu: Option<Montant> = None;
    if let Some(o) = deduction_forfaitaire(&bar, &sal, &ctx.entreprise)? {
        dfs_applicable(eng, &nat, &o, &lb, &sal, &ctx.racine, &ctx.lue_au)?;
        let deja = dfs_deja_deduite(ctx, eng, &sal, precedentes)?;
        let montants: Vec<Montant> = soumises.iter().map(|l| l.montant).collect();
        let (deduction, assiettes_abattues) = abattement_dfs(&o, &montants, brut, deja, plafond, &mut avis_dfs);
        let mut somme_abattue = Montant::ZERO;
        for (r, abattu) in par_rubrique.iter_mut().zip(assiettes_abattues) {
            somme_abattue += abattu;
            r.dfs = Some(abattu);
            r.x115_dfs = Some((abattu * maj115).au_centime());
            r.x110_dfs = Some((abattu * maj110).au_centime());
        }
        brut_abattu = Some(somme_abattue);
        dfs = Some(deduction);
    }

    // 7.5 ter : la retraite complementaire d'un intermittent, sur le plafond de l'annee chez la compagnie
    // (`droit::plafond::tranches_sous_le_plafond_annuel` : Audiens, « ce sont les plafonds annuels qui doivent etre
    // retenus » pour un non-cadre ; circulaire Agirc-Arrco 2018-09-DC, annexe A, art. 1, pour tout intermittent), sur la
    // remuneration que la retraite compte (`remuneration_de_la_retraite`). Rubrique par rubrique : quand toute la
    // remuneration de la paie est en tranche 1, la tranche de chaque rubrique est sa part, et la retraite d'un non-cadre se
    // calcule comme sur son brut ; sinon, au prorata ; une paie sans remuneration pose sa tranche (regularisee) sur sa
    // premiere rubrique. La tranche appliquee est la somme de celles des rubriques. Seule cotise sur l'annee la paie dont
    // le bareme pose la ligne `retraite_t1` sur `t1_annuelle` (`retraite_sur_l_annee`) : un cadre intermittent cotise sur
    // ses tranches journalieres (`par_categorie.cadre`, Audiens), et un permanent sur ses tranches du mois ; ni l'un ni
    // l'autre n'a de tranche de l'annee.
    let retraite_sur_l_annee = bar.rubrique(&droit::forme_bareme::RETRAITE_T1).is_some_and(|r| r.base == Some(Assiette::T1Annuelle));
    let mut retraite_de_l_annee: Option<(droit::plafond::AnneeAnterieure, droit::plafond::Regularisees)> = None;
    if retraite_sur_l_annee {
        let plafond_annuel = bar.plafond_securite_sociale.as_ref().ok_or_else(|| socle::fichiers::absent("plafond_securite_sociale"))?.annuel;
        let avant = annee_de_la_retraite(ctx, eng, &sal, plafond_annuel, precedentes)?;
        let remuneration = remuneration_de_la_retraite(brut, brut_abattu, dfs.as_ref())?;
        let t = droit::plafond::tranches_sous_le_plafond_annuel(remuneration, &bar.tranches_du_plafond, &avant);
        let abattue = retraite_abattue(dfs.as_ref());
        for (i, r) in par_rubrique.iter_mut().enumerate() {
            let part = if abattue { r.dfs.ok_or_else(|| socle::fichiers::absent(Assiette::BrutDfs.nom()))? } else { r.brut };
            r.t1_annuelle = Some(if t.t1 == remuneration {
                part
            } else if remuneration.est_nul() {
                if i == 0 { t.t1 } else { Montant::ZERO }
            } else {
                t.t1.prorata(part, remuneration)
            });
        }
        let t1 = par_rubrique.iter().filter_map(|r| r.t1_annuelle).fold(Montant::ZERO, |a, x| a + x);
        retraite_de_l_annee = Some((avant, droit::plafond::Regularisees { t1, ..t }));
    }

    // L'assiette des Conges Spectacles dans la limite du plafond journalier que l'accord de la convention ecrit pour
    // l'emploi, quand la compagnie dit qu'il la lie (`clauses::plafond_conges_spectacles` ; C. trav. D. 7121-37),
    // multiplie par les jours travailles de la paie. Chaque rubrique en garde sa part, au prorata, la derniere prenant
    // l'arrondi : la ligne de la caisse, le 066 de la DSN et le certificat lisent cette seule base.
    if let Some(p) = plafond_conges_spectacles(ctx, spec.as_ref(), &sal)? {
        let plafond = (jours * p).au_centime();
        let total = par_rubrique.iter().fold(zero, |a, b| a + b.brut_cs);
        if total > plafond {
            let mut reste = plafond;
            let derniere = par_rubrique.iter().rposition(|b| !b.brut_cs.est_nul());
            for (i, b) in par_rubrique.iter_mut().enumerate() {
                if b.brut_cs.est_nul() {
                    continue;
                }
                b.brut_cs = if Some(i) == derniere { reste } else { plafond.prorata(b.brut_cs, total).au_centime() };
                reste -= b.brut_cs;
            }
        }
    }

    // 7.6 : les bases globales.
    let mut plafond_intermittent = None;
    let mut bases: BTreeMap<Assiette, Montant> = BTreeMap::new();
    let somme_de = |f: &dyn Fn(&Assiettes) -> Montant| par_rubrique.iter().fold(zero, |a, b| a + f(b));
    bases.insert(Assiette::Brut, brut);
    bases.insert(Assiette::BrutCs, somme_de(&|b| b.brut_cs));
    bases.insert(Assiette::Csg, base_csg);
    bases.insert(Assiette::BrutX115, somme_de(&|b| b.x115));
    bases.insert(Assiette::BrutX110, somme_de(&|b| b.x110));
    // Les tranches journalieres d'un intermittent (Audiens, Parametres : « le plafond applicable est le plafond journalier »).
    if let Some(pj) = plafond_journalier.filter(|_| !nat.mensuelle()) {
        let t1 = t1_du_contrat(pj);
        bases.insert(Assiette::T1Journaliere, brut.min(t1));
    }
    // Les tranches d'un cadre intermittent, au-dela de la T1 (`bareme_cadre`).
    if let Some(pj) = plafond_journalier.filter(|_| !nat.mensuelle() && droit::bareme::est_cadre(&sal)) {
        let t1 = t1_du_contrat(pj);
        // Les memes bornes que celles d'un permanent, en nombre de plafonds (`tranches_du_plafond`, leurs textes) : la
        // tranche 2 jusqu'a huit (ANI Agirc-Arrco, art. 32), l'Apec dans la limite de quatre (Audiens, Parametres).
        let huit = jusqu_a(pj, Some(bar.tranches_du_plafond.t2), |d| d.t1_et_t2);
        let quatre_apec = jusqu_a(pj, Some(bar.tranches_du_plafond.apec), |d| d.apec);
        bases.insert(Assiette::T2Journaliere, zero.max(brut.min(huit) - t1));
        // L'Apec, sur le brut dans la limite de quatre plafonds : en tout, et au-dela de la T1.
        bases.insert(Assiette::Apec, brut.min(quatre_apec));
        bases.insert(Assiette::ApecT2Journaliere, zero.max(brut.min(quatre_apec) - t1));
        // La tranche B de la prevoyance d'un cadre technicien de la CCNEAC (XII.2.2.2) : la tranche 2 de son titre XII.2,
        // « l'equivalent de 4 plafonds » (`tranches_du_plafond.t2_conventionnelle`).
        bases.insert(Assiette::T2PrevoyanceJournaliere, zero.max(brut.min(jusqu_a(pj, Some(bar.tranches_du_plafond.t2_conventionnelle), |d| d.t1_et_tranche_b)) - t1));
        // La CET, due quand le brut depasse la T1, sur T1 + T2 : huit plafonds au plus (Agirc-Arrco, « jusqu'a 8 PSS »).
        bases.insert(Assiette::Cet, if brut > t1 { brut.min(huit) } else { zero });
    }
    // La majoration de 11,5 % (CSS L. 243-1-3, D. 243-0-2) s'applique a l'assiette plafonnee : quand le plafond la limite,
    // c'est elle qu'elle majore (lettre circulaire Acoss 2013-019, annexe 1, R2, et sa « Regularisation progressive » :
    // l'assiette plafonnee regularisee, puis x 11,5 % ; atelier/reference/sources/majoration-caisses-conges/LISEZ-MOI.md) ;
    // sinon, le brut majore tel qu'il est.
    let majoree = |t1: Montant, remuneration: Montant, x115: Montant| if t1 == remuneration { x115 } else { (t1 * maj115).au_centime() };
    if nat.mensuelle() {
        // `pss_periode` : l'assiette du forfait de sante (rubrique `mutuelle`, base 18 du bloc 79), le plafond de la
        // periode sans la correction du temps partiel (`plafond_de_la_sante`). Les tranches (`t1_mensuelle`...) suivent,
        // elles, le plafond corrige, regularisees sur l'annee (`regularisees`).
        bases.insert(Assiette::PssPeriode, pss_sante.ok_or_else(|| refus("le plafond de la santé d'un contrat payé au mois manque : le signaler au développeur"))?);
        bases.insert(Assiette::T1Mensuelle, regularisees.t1);
        // Le FNAL plafonne d'un permanent declare a la caisse (CDD de moins de douze mois d'un
        // artiste) : seul le montant plafonne se majore (lettre circulaire Acoss 2013-019, annexe 1, R2), comme
        // `brut_x115_plafonne` d'un intermittent. Sans majoration (1,0), c'est la tranche 1.
        bases.insert(Assiette::T1MensuelleX115, majoree(regularisees.t1, brut, bases[&Assiette::BrutX115]));
        bases.insert(Assiette::T2Mensuelle, regularisees.t2);
        // Quatre plafonds de la periode : le chomage et l'AGS d'un permanent, et l'Apec d'un cadre dans sa propre limite
        // (`AssiettesPlafonnees::apec`, Audiens : « 4 fois le plafond de la Securite sociale »). Les contributions
        // d'assurance chomage « sont assises sur les remunerations brutes dans la limite d'un plafond » (C. trav.
        // L. 5422-9), « fixe a quatre fois le plafond mentionne a l'article L. 241-3 du code de la securite sociale »
        // (reglement general annexe a la convention du 15/11/2024, art. 49, al. 2) ; l'AGS suit la meme assiette
        // (L. 3253-18). Le plafond est celui de la periode de paie, reduit au prorata des jours d'emploi quand le contrat
        // ne couvre pas le mois (CSS R. 242-2, al. 3), sans regularisation (`AssiettesPlafonnees::chomage`, le multiple
        // lu au bareme : `tranches_du_plafond`). Textes : atelier/reference/sources/chomage-permanents/.
        bases.insert(Assiette::ApecMensuelle, plafonnees.apec);
        bases.insert(Assiette::BrutChomage, plafonnees.chomage);
        bases.insert(Assiette::T2Conventionnelle, regularisees.t2_conventionnelle);
        // La CET d'un permanent, cadre ou non : due au-dela de la T1, sur le brut dans la limite de la T2
        // (`tranches_du_plafond.t2` ; ANI Agirc-Arrco, art. 32 et 37 : « Pour les participants dont la remuneration
        // excede le plafond » ; Audiens, « dans la limite de la T1 + T2 »), regularisee comme le dit l'exemple 2 d'Audiens
        // (`droit::plafond::Regularisees`).
        bases.insert(Assiette::CetMensuelle, regularisees.cet);
        bases.insert(Assiette::T1T2Mensuelle, regularisees.t1_t2);
    } else {
        // Le plafond d'un intermittent : celui que le bas du bulletin imprime (`plafond_de_l_engagement`), et la seule
        // source. Textes : atelier/reference/sources/plafond-intermittents/LISEZ-MOI.md. La vieillesse plafonnee et le
        // FNAL s'assoient « dans la limite » du plafond de L. 241-3 CSS ; le chomage des annexes VIII et X « dans la
        // limite d'un plafond fixe a quatre fois le plafond mentionne a l'article L. 241-3 » (annexes VIII et X du
        // 15/11/2024, art. 49). Sous le plafond, rien ne bouge : les bases sont le brut, et le brut majore tel qu'il
        // etait.
        plafond_intermittent = Some(plafond_urssaf);
        bases.insert(Assiette::BrutPlafonne, regularisees.t1);
        bases.insert(Assiette::BrutX115Plafonne, majoree(regularisees.t1, brut, bases[&Assiette::BrutX115]));
        bases.insert(Assiette::BrutChomage, plafonnees.chomage);
        // Les tranches de l'annee de sa retraite complementaire (7.5 ter) : la tranche 1 de la paie, sa tranche 2 et sa CET.
        if let Some((_, t)) = &retraite_de_l_annee {
            bases.insert(Assiette::T1Annuelle, t.t1);
            bases.insert(Assiette::T2Annuelle, t.t2);
            bases.insert(Assiette::CetAnnuelle, t.cet);
        }
    }
    if let Some(s) = brut_abattu {
        bases.insert(Assiette::BrutDfs, s);
        let x115 = par_rubrique.iter().filter_map(|b| b.x115_dfs).fold(zero, |a, x| a + x);
        bases.insert(Assiette::BrutX115Dfs, x115);
        bases.insert(Assiette::BrutX110Dfs, par_rubrique.iter().filter_map(|b| b.x110_dfs).fold(zero, |a, x| a + x));
        if let Some(plafond_ss) = plafond_intermittent {
            // Le plafond limite l'assiette abattue : la vieillesse plafonnee et la base 02 sur le brut abattu dans la
            // limite du plafond du contrat (un intermittent ne se regularise pas : `RegularisationDuPlafond`).
            let t1 = droit::plafond::assiettes_plafonnees(s, plafond_ss, limite_csg, &bar.tranches_du_plafond, &plafonnement).regularisees.t1;
            bases.insert(Assiette::BrutPlafonneDfs, t1);
            bases.insert(Assiette::BrutX115PlafonneDfs, majoree(t1, s, x115));
        }
    }

    // 7.7 : la boucle des cotisations.
    let mut lignes: Vec<LigneCotisation> = Vec::new();
    let (mut tot_pat, mut tot_sal) = (Montant::ZERO, Montant::ZERO);
    for r in rubriques_bulletin {
        let id = r.id.clone();
        let mut cle_base = base_de(r)?;
        if let Some(d) = &dfs {
            // Une rubrique abattue s'assoit sur le brut abattu ; ses assiettes majorees aussi (Acoss 2013-019, R6). Une
            // assiette plafonnee n'a de forme abattue que sous le plafond d'un intermittent.
            if d.rubriques.contains(&id) {
                if let Some(abattue) = cle_base.abattue().filter(|a| bases.contains_key(a)) {
                    cle_base = abattue;
                }
            }
        }
        // Une cotisation du bareme se paie sur des euros : une assiette en heures (`Assiette::en_heures`), d'ou que vienne
        // la ligne (bareme livre, prevoyance conventionnelle, surcouche de la compagnie), ne se cotise pas, et une paie
        // validee relit la base de chacune de ses lignes en euros (`BaseDeCotisation::relue`).
        if cle_base.en_heures() {
            return Err(refus(format!(
                "La cotisation « {} » du barème a pour assiette « {} », qui compte des heures : une cotisation se paie sur des euros. Corriger le barème.",
                id.as_str(),
                cle_base.nom()
            )));
        }
        let base = *bases.get(&cle_base).ok_or_else(|| socle::fichiers::absent(cle_base.nom()))?;
        let assiettes_r: Vec<Montant> = if cle_base.par_rubrique() {
            par_rubrique
                .iter()
                .map(|b| b.de(cle_base).ok_or_else(|| socle::fichiers::absent(cle_base.nom())))
                .collect::<R<_>>()?
        } else {
            vec![base]
        };
        let (mut m_pat, mut m_sal) = (Montant::ZERO, Montant::ZERO);
        let (mut taux_pat, mut taux_sal) = (part_patronale(r), r.taux_sal);
        let non_vide = |c: &Option<Vec<Taux>>| c.clone().filter(|c| !c.is_empty());
        let (mut comp_pat, mut comp_sal) = (non_vide(&r.composants_pat), non_vide(&r.composants_sal));
        let manque = || socle::echec::cle("Le barème ne donne pas le taux de cette ligne : le complément ne s'ajoute à rien.", id.as_str());
        // Le complement au-dela d'un seuil : le taux reduit, plus le complement.
        if let Some(compl) = &r.complement {
            // Sans seuil, le complement s'ajoute toujours.
            let depasse = compl.seuil_smic.map(|n| au_dela_du_seuil(bar.reduction_generale.as_ref(), brut, Heures::ZERO.max(heures_annexes), n)).transpose()?;
            if depasse.is_none_or(|d| d) {
                taux_pat = Some(taux_compose(taux_pat.ok_or_else(manque)? + compl.taux));
            }
        }
        // La CET d'un cadre intermittent, fondue dans T1 et T2 : l'arrondi se fait en deux parts.
        if let Some(cet) = &r.cet_si_depasse {
            if bases.get(&Assiette::Cet).is_some_and(|c| !c.est_nul()) {
                let (cet_pat, cet_sal) = (cet.pat, cet.sal);
                let mut cp = match comp_pat.take() {
                    Some(c) => c,
                    None => vec![taux_pat.ok_or_else(manque)?],
                };
                // Le dernier composant prend la CET, sans arrondi : chaque composant s'arrondit au centime (`somme_pct`).
                if let Some(d) = cp.last_mut() {
                    *d = courte(*d + cet_pat);
                }
                let mut cs = match comp_sal.take() {
                    Some(c) => c,
                    None => vec![taux_sal.ok_or_else(manque)?],
                };
                if let Some(d) = cs.last_mut() {
                    *d = courte(*d + cet_sal);
                }
                comp_pat = Some(cp);
                comp_sal = Some(cs);
                taux_pat = Some(taux_compose(taux_pat.ok_or_else(manque)? + cet_pat));
                taux_sal = Some(taux_compose(taux_sal.ok_or_else(manque)? + cet_sal));
            }
        }
        let composants = non_vide(&r.composants);
        for a in &assiettes_r {
            if let Some(c) = &composants {
                m_pat += somme_pct(*a, c);
            } else if let Some(c) = &comp_pat {
                m_pat += somme_pct(*a, c);
            } else if let Some(t) = taux_pat {
                m_pat += a.fois_taux(t).au_centime();
            }
            if let Some(c) = &comp_sal {
                m_sal += somme_pct(*a, c);
            } else if let Some(t) = taux_sal {
                m_sal += a.fois_taux(t).au_centime();
            }
        }
        lignes.push(LigneCotisation {
            id: id.into(),
            famille: r.famille.clone().ok_or_else(|| socle::fichiers::absent("famille"))?,
            libelle: r.libelle.clone().ok_or_else(|| socle::fichiers::absent("libelle"))?,
            base: BaseDeCotisation::Montant(base),
            taux_pat,
            montant_pat: m_pat,
            taux_sal,
            montant_sal: m_sal,
            masquer_base_et_taux: r.masquer_base_et_taux == Some(true),
        });
        tot_pat += m_pat;
        tot_sal += m_sal;
    }
    // Une tranche au-dela du plafond, et la CET qui n'est pas due (la remuneration sous le plafond), ne sont pas au
    // bulletin quand elles sont nulles : le bareme les porte pour tout permanent, la paie seulement pour qui les doit. De
    // meme la tranche 1 de l'annee d'un intermittent (`t1_annuelle`) quand l'annee a deja passe le plafond annuel : la
    // paie est toute en tranche 2. La tranche 1 d'un mois sans brut d'un permanent (`t1_mensuelle`) reste.
    lignes.retain(|l| {
        let sans_t1 = retraite_sur_l_annee && l.est(&droit::forme_bareme::RETRAITE_T1);
        !(l.base.est_nulle()
            && l.montant_pat.est_nul()
            && l.montant_sal.est_nul()
            && (sans_t1 || [droit::forme_bareme::RETRAITE_T2, droit::forme_bareme::APEC_T2, droit::forme_bareme::CET].iter().any(|id| l.est(id))))
    });

    // 7.9 : la reduction generale, apres les cotisations.
    let statut = sal.statut.dite().copied();
    let caisse_conges = bases[&Assiette::BrutCs] > Montant::ZERO;
    // Sous DFS, la reduction se calcule sur le brut abattu, dans la limite que le bareme dit de celle du brut entier
    // (`deduction_forfaitaire.limite_reduction_generale`, `reduction_generale_sous_dfs`).
    // Le SMIC horaire de la reduction (`smic_horaire_reference`) ; sans lui, pas de reduction.
    let smic_horaire_rg = bar.reduction_generale.as_ref().and_then(|r| r.smic_horaire_reference);
    let absence = smic_d_une_absence(&lb, smic_horaire_rg);
    let heures_rg = match absence.as_ref().and_then(|a| a.heures) {
        Some(h) => h,
        None => heures_annexes,
    };
    if let Some(a) = &absence {
        bases.insert(Assiette::RemunerationPercueHorsEna, a.percue);
        bases.insert(Assiette::RemunerationHabituelleHorsEna, a.habituelle);
    }
    let rg = match brut_abattu {
        Some(abattu) => {
            let limite = bar.deduction_forfaitaire.as_ref().ok_or_else(|| socle::fichiers::absent("deduction_forfaitaire"))?.limite_reduction_generale;
            reduction_generale_sous_dfs(bar.reduction_generale.as_ref(), limite, statut, brut, abattu, heures_rg, caisse_conges)?
        }
        None => reduction_generale(bar.reduction_generale.as_ref(), statut, brut, heures_rg, caisse_conges)?,
    };
    // Le SMIC se calcule sur les heures exactes du rapport ; le bulletin les dit au centieme.
    let rg = rg.map(|mut r| {
        if absence.as_ref().is_some_and(|a| a.heures.is_some()) {
            r.heures = r.heures.a_deux_decimales();
        }
        r
    });
    if let Some(rg) = rg.as_ref().filter(|rg| !rg.montant.est_nul()) {
        lignes.push(ligne_de_reduction(rg, brut));
        tot_pat -= rg.montant;
    }

    // 7.9 bis : les heures en plus d'un salarie paye au mois (`heures_en_plus`) : la reduction de cotisations salariales,
    // la deduction patronale, et le net exonere d'impot, que la remuneration nette fiscale ne prend pas.
    let mois_paye = fin_periode.mois_civil();
    let heures_en_plus = crate::heures_en_plus::apres_les_cotisations(ctx, &bar, &lb, &lignes, brut, abattue_appliquee, tot_pat, || {
        crate::conges::exonere_des_mois_anterieurs(ctx, mois_paye, &sal.id)
    })?;
    let mut exonere = Montant::ZERO;
    // Le nombre des heures en plus : sa base se range a la sortie, dans la grandeur que son assiette compte.
    let mut heures_en_plus_heures = None;
    if let Some(h) = &heures_en_plus {
        lignes.extend(h.lignes.iter().cloned());
        tot_sal -= h.reduction;
        tot_pat -= h.deduction;
        exonere = h.exonere;
        bases.insert(Assiette::HeuresEnPlus, h.remuneration);
        heures_en_plus_heures = Some(h.heures);
        bases.insert(Assiette::HeuresEnPlusExonere, h.exonere);
    }
    // Les indemnites journalieres que la compagnie subrogee reverse (`maintien::lignes`) : le montant net social les
    // compte, la DSN les dit en S21.G00.58 de type 10, et le prelevement a la source les prend pour assiette, a cote du
    // net imposable (`etats::prelevement`).
    for (l, b) in eng.lignes.iter().zip(lb.iter()) {
        if droit::catalogue::RubriqueDuMois::IjssNettes.nomme(&b.code) {
            *bases.entry(Assiette::IjSubrogeesNettes).or_insert(zero) += b.montant;
            *bases.entry(Assiette::IjSubrogeesAssiettePas).or_insert(zero) += l.assiette_pas().ok_or_else(|| socle::fichiers::absent("assiette_pas"))?;
        }
    }

    // 7.10 : la sortie. Ce qui est hors assiette se paie ou se retient quand meme.
    let hors_assiette = lb.iter().filter(|l| !l.soumis).fold(Montant::ZERO, |a, l| a + l.montant);
    // L'avantage en nature est dans le brut (il est soumis), et le salarie l'a recu en nature, pas en argent : le net a
    // payer avant impot, « la somme qui aurait été versée au salarié en l'absence de retenue à la source » (C. trav.
    // R. 3243-1, 9°, `atelier/reference/sources/code-du-travail/xml/R3243-1.xml`), ne le compte pas, ni donc ce net moins
    // l'impot preleve, « la somme effectivement reçue par le salarié » (10°). Qu'une valeur fournie en nature ne soit pas
    // une somme versee est une inference. Le bulletin l'imprime en deduction (`documents::bulletin`, `Jeu::en_nature`).
    // Le net social, le net imposable et le net verse de la DSN le gardent : le montant net
    // social compte « les avantages et accessoires en nature ou en argent » (arrete du 25/02/2016, art. 1er, II, 1°,
    // `atelier/reference/sources/montant-net-social/legi-arrete-20160225-bulletin.md`), et le net verse part de la
    // remuneration nette fiscale (S21.G00.50.004).
    let mut en_nature = Montant::ZERO;
    for l in lb.iter() {
        if rubrique(&ctx.racine, &l.code, &ctx.lue_au)?.nature == droit::catalogue::NatureDeRubrique::AvantageNature {
            en_nature += l.montant;
        }
    }
    let net = brut - tot_sal + hors_assiette - en_nature;
    // La remuneration nette fiscale (S21.G00.50.002) part du brut soumis, deduit les cotisations salariales et
    // reintegre la CSG/CRDS non deductible et la sante patronale (`reintegrations`). Des lignes hors assiette, elle ne
    // prend que celles que le catalogue dit imposables : la norme (2026.1, § 2.3.2, p. 60-61) fait « DEDUIRE les sommes
    // versees a titre de remboursement de frais professionnels » et « AJOUTER les indemnites imposables a l'impot sur
    // le revenu ». Un defraiement, une indemnite exoneree (CGI 80 duodecies) n'y entrent pas ; un acompte non plus : il
    // avance le net du mois, il ne change pas le revenu.
    let hors_assiette_imposable = lb.iter().filter(|l| !l.soumis && l.imposable).fold(Montant::ZERO, |a, l| a + l.montant);
    // Le montant net social (S21.G00.58.004, type 03). C. trav. R. 3243-1, 9° bis, qui renvoie au CSS R. 844-1, II
    // (redaction du decret n° 2023-1378, que le decret n° 2026-535 n'a pas touchee) ; page du BOSS « Montant net
    // social » du 01/10/2026, II.A.1 et Q 20, 27 (textes : atelier/reference/sources/montant-net-social/LISEZ-MOI.md). La
    // totalite des sommes versees en contrepartie ou a l'occasion du travail, exonerees ou non, moins les cotisations et
    // contributions sociales du salarie (`tot_sal`, parts salariales de sante et de prevoyance comprises). Le
    // financement patronal des garanties collectives (sante, prevoyance, Fonds sante des intermittents) n'y entre pas :
    // R. 844-1, II, 1° l'excepte. La page du BOSS l'y range (II.A.1, Q 13, 14, 29) sans texte qui l'appuie ; le decret
    // prime (meme LISEZ-MOI, derniere section). Des lignes hors assiette, seules
    // celles qui sont un revenu (indemnites, imposables ou non) ; ni remboursement de frais dans ses limites, ni
    // mouvement de tresorerie (acompte) : la nature de la rubrique le dit, et une nature qui ne le dit pas refuse
    // (`Rubrique::hors_assiette`). Pas de prelevement a la source : un net SOCIAL.
    let mut hors_assiette_revenu = Montant::ZERO;
    for l in lb.iter().filter(|l| !l.soumis) {
        if rubrique(&ctx.racine, &l.code, &ctx.lue_au)?.hors_assiette()? == HorsAssiette::Revenu {
            hors_assiette_revenu += l.montant;
        }
    }
    // La remuneration nette fiscale reintegre la CSG et la CRDS non deductibles (`reintegrations`) : sans leur ligne, la
    // pile du contrat ne la calcule pas (une couche qui l'a retiree, un bareme qui ne l'a pas).
    if !lignes.iter().any(|l| l.est(&droit::forme_bareme::CSG_CRDS_ND)) {
        return Err(socle::echec::cle(
            format!(
                "{} : le barème {} de son contrat n'a pas la ligne de la CSG et de la CRDS non déductibles, que la rémunération nette fiscale (S21.G00.50.002) réintègre. Rien n'est calculé : le signaler au développeur.",
                eng.qui(),
                bar.annee
            ),
            "rubriques_bulletin.csg_crds_nd",
        ));
    }
    // S21.G00.51.011 = 002, le salaire brut servant aux calculs des droits de l'Assurance chomage, « doit contenir
    // uniquement les éléments de salaire. Il ne doit pas inclure les primes et indemnités relatives à la rupture du
    // contrat de travail déclarées dans le bloc « Prime, gratification et indemnité - S21.G00.52 » » (cahier technique
    // 2026.1, `atelier/reference/norme-dsn/texte/bloc-S21.G00.51.txt`, Type) ; la grille « Indemnités » de net-entreprises
    // le redit pour chacune (« intégration au salaire brut soumis à contributions Assurance Chômage « Non » »,
    // `atelier/reference/norme-dsn/netentreprises-grille-aide-remplissage-indemnites.txt`). Quelle que soit la nature, un
    // CDDU compris (l'indemnite du delai de prevenance d'une fin d'essai, `solde::delai_de_prevenance_a_l_activite`).
    let chomage_51 = brut - lb.iter().filter(|l| l.dsn_bloc52.is_some() && l.soumis).fold(Montant::ZERO, |a, l| a + l.montant);
    let (base_51, retabli) = if nat.remunerations_au_brut() {
        (brut, brut)
    } else {
        let base_51 = lb.iter().filter(|l| l.salaire_de_base).fold(Montant::ZERO, |a, l| a + l.montant);
        // S21.G00.51.011 = 003, « Salaire rétabli – reconstitué », qui « doit toujours être déclaré pour la période de paie
        // courante dans la DSN mensuelle » (cahier technique 2026.1, bloc S21.G00.51, Type :
        // `atelier/reference/norme-dsn/texte/bloc-S21.G00.51.txt`) : le salaire RECONSTITUE, le brut sans les absences qui
        // l'ont reduit (les rubriques `retabli`). Un mois entierement suspendu declare donc 001 a 0.00 et 003 au salaire du
        // mois.
        let retabli = brut - lb.iter().filter(|l| l.retabli).fold(Montant::ZERO, |a, l| a + l.montant);
        (base_51, retabli)
    };
    let tp = plafond_temps_partiel(&ctx.entreprise, eng, &sal)?;
    let mut paie = Paie::nouveau(eng.clone(), sal, spec, brut, base_csg, plafond, plafond_urssaf, abattue_appliquee, smic, heures, tp);
    let p = paie.ecrire();
    p.heures_absence = -lb.iter().filter(|l| l.heures_annexes < Heures::ZERO).fold(Heures::ZERO, |a, l| a + l.heures_annexes);
    p.lignes = lignes;
    p.total_pat = tot_pat;
    p.total_sal = tot_sal;
    p.net_a_payer = net;
    p.net_social = brut - tot_sal + hors_assiette_revenu;
    p.net_imposable = brut - tot_sal + hors_assiette_imposable + reintegrations(&p.lignes) - exonere;
    p.cout_employeur = brut + tot_pat;
    p.bases = bases
        .into_iter()
        .map(|(a, m)| (a, BaseDeCotisation::Montant(m)))
        .chain(heures_en_plus_heures.map(|h| (Assiette::HeuresEnPlusHeures, BaseDeCotisation::Heures(h))))
        .map(|(a, v)| Ok((a, BaseDeCotisation::calculee(a, v)?)))
        .collect::<R<_>>()?;
    p.reduction_generale = rg;
    p.deduction_forfaitaire = dfs;
    p.avertissements.extend(avis_dfs.into_iter().map(|texte| Avis { info: true, texte }));
    if let Some(h) = heures_en_plus {
        p.avertissements.extend(h.avis.into_iter().map(|texte| Avis { info: false, texte }));
    }
    p.salaire_de_base = base_51;
    p.salaire_retabli = retabli;
    p.assiette_chomage = chomage_51;
    p.heures_travaillees = Heures::ZERO.max(heures_annexes);
    p.remuneration = lb;
    // L'annee de part et d'autre de la periode, quand elle se regularise, ou celle de la retraite d'un intermittent : la
    // bascule de la CET s'en juge (`AnneeDeLaPeriode`).
    // La CET d'un intermittent est sur l'annee de sa retraite (7.5 ter), qu'il se regularise ou non pour le plafond de la
    // Securite sociale : c'est elle que son avis juge ; celle d'un salarie au mois, sur sa regularisation.
    let t2 = bar.tranches_du_plafond.t2;
    let annee = match (retraite_de_l_annee, plafonnement) {
        (Some((avant, _)), _) => {
            let mut apres = avant;
            ajouter_a_la_retraite_de_l_annee(&mut apres, &paie)?;
            Some(AnneeDeLaPeriode { avant, apres, t2, plafond: PlafondDeLAnnee::Annuel })
        }
        (None, droit::plafond::Plafonnement::Regularise(avant)) => {
            let mut apres = avant;
            ajouter_a_l_annee(&mut apres, &paie, &bar.regularisation_du_plafond)?;
            Some(AnneeDeLaPeriode { avant, apres, t2, plafond: PlafondDeLAnnee::SommeDesPlafonds })
        }
        (None, droit::plafond::Plafonnement::ParPaie) => None,
    };
    Ok((paie, annee))
}

/// L'ANNEE DE PART ET D'AUTRE D'UNE PERIODE (`calculer_periode`), quand elle se regularise ou que sa retraite se compte
/// sur le plafond de l'annee : ce qui la precede (`avant`), l'annee jusqu'a elle comprise (`apres`), la limite de la
/// tranche 2 de son bareme, en plafonds (`t2`), et le plafond auquel sa remuneration se compare (`plafond`). La paie du
/// mois en tire la bascule de la CET (`droit::plafond::BasculeDeLaCet::entre`) : entre l'annee avant sa premiere periode
/// et l'annee apres sa derniere.
pub(crate) struct AnneeDeLaPeriode {
    avant: droit::plafond::AnneeAnterieure,
    apres: droit::plafond::AnneeAnterieure,
    t2: Coefficient,
    plafond: PlafondDeLAnnee,
}

/// LE PLAFOND AUQUEL LA REMUNERATION DE L'ANNEE SE COMPARE POUR LA CET (`AnneeDeLaPeriode`) : la somme des plafonds des
/// paies d'un salarie au mois, proratises, depuis l'embauche ou le premier mois paye ici (`SommeDesPlafonds`, CSS
/// R. 242-2, II) ; le plafond annuel de la retraite complementaire d'un intermittent, le meme des le 1er janvier
/// (`Annuel` : Audiens, Parametres, « ce sont les plafonds annuels qui doivent etre retenus » ;
/// `droit::plafond::tranches_sous_le_plafond_annuel`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlafondDeLAnnee {
    SommeDesPlafonds,
    Annuel,
}

/// L'avis de la bascule de la CET de la paie du mois `p` (`avis_de_la_bascule`), entre l'annee avant sa premiere
/// periode qui se regularise et l'annee apres sa derniere : aucun quand le critere ne change pas sur le mois (deux
/// bascules d'un mois decoupe qui s'annulent), ni quand la paie ne porte pas de ligne de la CET.
fn avis_du_mois(eng: &Engagement, p: &Paie, annees: &[AnneeDeLaPeriode]) -> Option<Avis> {
    let (premiere, derniere) = (annees.first()?, annees.last()?);
    let bascule = droit::plafond::BasculeDeLaCet::entre(&premiere.avant, &derniere.apres)?;
    let cet = p.lignes.iter().find(|l| l.est(&droit::forme_bareme::CET))?;
    Some(avis_de_la_bascule(eng, bascule, cet, derniere))
}

/// L'AVIS DE LA PAIE OU LA CONTRIBUTION D'EQUILIBRE TECHNIQUE DE L'ANNEE BASCULE (`droit::plafond::BasculeDeLaCet`), avec
/// ce que sa ligne `cet` en tire ce mois : toute celle de l'annee quand elle devient due, sa restitution quand elle se rend.
/// Le plafond compare est celui que dit `annee.plafond` : pour un salarie au mois, la somme des plafonds des paies de
/// l'annee, proratises, depuis l'embauche ou le premier mois paye ici (`droit::plafond::Regularisees`), pas le plafond
/// annuel ; pour un intermittent, le plafond annuel de sa retraite complementaire, quelle que soit la duree de ses
/// contrats. La remuneration due l'est dans la limite de la tranche 2 (`annee.t2` : `tranches_du_plafond.t2` du bareme,
/// en plafonds).
fn avis_de_la_bascule(eng: &Engagement, bascule: droit::plafond::BasculeDeLaCet, cet: &LigneCotisation, annee: &AnneeDeLaPeriode) -> Avis {
    let euros = socle::texte::euros;
    let parts = |sal: Montant, pat: Montant| format!("{} ({} salariale, {} patronale)", euros(sal + pat), euros(sal), euros(pat));
    let plafond = match annee.plafond {
        PlafondDeLAnnee::SommeDesPlafonds => "la somme de ses plafonds de l'année".to_string(),
        PlafondDeLAnnee::Annuel => format!("le plafond annuel de sa retraite complémentaire ({})", euros(annee.apres.plafonds())),
    };
    let texte = match bascule {
        droit::plafond::BasculeDeLaCet::Due { depassement } => format!(
            "{} : la rémunération de l'année dépasse de {} {} ; la contribution d'équilibre technique devient due sur toute la rémunération de l'année, dans la limite de {} plafonds : {}.",
            eng.qui(),
            euros(depassement),
            plafond,
            socle::texte::virgule(annee.t2),
            parts(cet.montant_sal, cet.montant_pat)
        ),
        droit::plafond::BasculeDeLaCet::Rendue { ecart } => format!(
            "{} : la rémunération de l'année repasse de {} sous {} ; la contribution d'équilibre technique de l'année n'est plus due et lui est remboursée sur cette paie : {}.",
            eng.qui(),
            euros(ecart),
            plafond,
            parts(cet.montant_sal.abs(), cet.montant_pat.abs())
        ),
    };
    Avis { info: true, texte }
}

/// Le plafond de securite sociale d'un engagement (`plafond::plafond_permanent`, `plafond::pour_paie`) : celui que le bas du bulletin imprime, et
/// celui sur lequel s'assoient la vieillesse plafonnee, le FNAL et le chomage d'un intermittent (`calculer`, 7.6).
/// `sal` : la fiche du salarie telle que la paie la lit (`emploi_de`), pour son statut : l'arrete du 24/01/1975 ne
/// vise que les artistes ; un technicien, comme un artiste engage cinq jours ou plus, a le plafond proratise sur les
/// jours remuneres de la paie (BOSS, assiette generale, § 1090, `atelier/reference/sources/boss/`). Textes :
/// atelier/reference/sources/plafond-intermittents/LISEZ-MOI.md.
pub(crate) fn plafond_de_l_engagement(ctx: &Contexte, e: &Engagement, sal: &ChampsSalarie) -> R<Montant> {
    let nat = e.nat()?;
    // Le plafond d'un salarie au mois ne depend pas de son statut.
    if nat.mensuelle() {
        let tp = match plafond_temps_partiel(&ctx.entreprise, e, sal)? {
            PlafondTempsPartiel::Proratise => Some(duree_du_temps_partiel(ctx, e)?),
            _ => None,
        };
        // Les jours d'absence que l'employeur ne remunere pas reduisent le plafond (CSS R. 242-2, I, al. 4 ; BOSS § 820-920) :
        // ceux que la deduction du mois a dits (`conges::lignes_du_mois`).
        let sans_remuneration = e.sans_remuneration.as_ref().ok_or_else(|| socle::fichiers::absent("sans_remuneration"))?;
        return plafond_permanent(&e.periode_payee()?, sans_remuneration, tp, &pss_du_contexte(ctx));
    }
    // Le plafond d'un intermittent depend de son statut (artiste : arrete du 24/01/1975, art. 3 ; technicien : R. 242-2
    // CSS).
    let artiste = statut_de(sal)? == Statut::Artiste;
    let ep = EngagementPlafond {
        artiste,
        periode: e.periode_payee()?,
        // Un contrat paye a l'engagement a son terme (`Engagement::contrat` : sans fin seulement pour un permanent ouvert).
        contrat: e.contrat().fermee().ok_or_else(|| socle::fichiers::absent("fin"))?,
        dates_travail: e.dates_travail.clone(),
        jours: e.jours,
    };
    pour_paie(&ep, &pss_du_contexte(ctx))
}

/// COMMENT LE PLAFOND DE LA PAIE DE `eng` TIENT COMPTE DE L'ANNEE (`droit::plafond::Plafonnement`) : une paie que le
/// bareme regularise (`regle`, `RegularisationDuPlafond::regularise`) lit ce qui la precede dans l'annee civile chez la
/// compagnie : les paies de son salarie des mois d'avant (`conges::plafonds_des_mois_anterieurs`), puis celles des
/// contrats precedents du meme mois (`precedentes`), « a chaque echeance de paie » (BOSS, assiette generale, § 1230) ;
/// INFERENCE, ecrite comme telle : les contrats d'un mois se suivent dans l'ordre de la saisie (le total du mois n'en
/// depend pas).
fn plafonnement(
    ctx: &Contexte, eng: &Engagement, nat: &Nature, sal: &SalarieDuContrat, regle: &droit::forme_bareme::RegularisationDuPlafond,
    precedentes: &[Paie],
) -> R<droit::plafond::Plafonnement> {
    if !regle.regularise(nat.mensuelle(), statut_de(sal)? == Statut::Artiste) {
        return Ok(droit::plafond::Plafonnement::ParPaie);
    }
    let mois = eng.periode_payee()?.fin().mois_civil();
    let mut annee = crate::conges::plafonds_des_mois_anterieurs(ctx, mois, &sal.id, regle)?;
    for p in precedentes.iter().filter(|p| p.salarie.id == sal.id || p.engagement.salarie == eng.salarie) {
        ajouter_a_l_annee(&mut annee, p, regle)?;
    }
    Ok(droit::plafond::Plafonnement::Regularise(annee))
}

/// LA PAIE `p`, D'AVANT DANS L'ANNEE, AJOUTEE A CE QUI PRECEDE UNE PAIE (`droit::plafond::AnneeAnterieure`) quand elle se
/// regularise (`regle`) : son brut, son plafond et les assiettes plafonnees qu'elle a ecrites ; une paie decoupee, periode
/// par periode (chacune sa fiche et sa nature). Une paie qui ne se regularise pas n'y entre pas.
pub(crate) fn ajouter_a_l_annee(
    annee: &mut droit::plafond::AnneeAnterieure, p: &Paie, regle: &droit::forme_bareme::RegularisationDuPlafond,
) -> R<()> {
    if !p.periodes.is_empty() {
        for part in &p.periodes {
            ajouter_a_l_annee(annee, part, regle)?;
        }
        return Ok(());
    }
    if !regle.regularise(p.engagement.nat()?.mensuelle(), statut_de(&p.salarie)? == Statut::Artiste) {
        return Ok(());
    }
    let lue = |a: Assiette| assiette_de_l_annee(p, a, "la régularisation du plafond sur l'année lit (CSS R. 242-2, II)");
    let appliquees = droit::plafond::Regularisees {
        t1: lue(Assiette::T1Mensuelle)?,
        t2: lue(Assiette::T2Mensuelle)?,
        t2_conventionnelle: lue(Assiette::T2Conventionnelle)?,
        t1_t2: lue(Assiette::T1T2Mensuelle)?,
        cet: lue(Assiette::CetMensuelle)?,
        abattue_csg: p.part_abattue_csg,
    };
    annee.ajouter(p.brut, p.plafond_urssaf, appliquees);
    Ok(())
}

/// L'assiette `a` qu'une paie d'avant dans l'annee a ecrite, que `lue_par` lit ; une paie qui ne la dit pas refuse.
fn assiette_de_l_annee(p: &Paie, a: Assiette, lue_par: &str) -> R<Montant> {
    let periode = p.engagement.periode_payee()?;
    let b = p.bases.get(&a).ok_or_else(|| {
        refus(format!(
            "{} : sa paie du {} au {} ne dit pas son assiette « {} », que {}. Le signaler au développeur.",
            p.engagement.qui(),
            periode.debut().jj_mm_aaaa(),
            periode.fin().jj_mm_aaaa(),
            a.nom(),
            lue_par
        ))
    })?;
    b.en_euros(a)
}

/// LA DEDUCTION FORFAITAIRE SPECIFIQUE `dfs` ABAT-ELLE LA RETRAITE COMPLEMENTAIRE ? Quand elle abat sa ligne de tranche 1
/// (`rubriques_abattues` du bareme : cahier Agirc-Arrco CT2026, « La solution ainsi retenue pour le regime general de la
/// Securite sociale s'applique en principe au regime AGIRC-ARRCO »).
fn retraite_abattue(dfs: Option<&DeductionForfaitaire>) -> bool {
    dfs.is_some_and(|d| d.rubriques.contains(&droit::forme_bareme::RETRAITE_T1))
}

/// LA REMUNERATION QUE LA RETRAITE COMPLEMENTAIRE D'UN INTERMITTENT COMPTE SUR L'ANNEE : son brut soumis `brut`, ou le
/// brut abattu `abattu` quand la deduction forfaitaire specifique `dfs` abat la retraite (`retraite_abattue`). Le calcul
/// (7.5 ter) et la relecture d'une paie d'avant (`ajouter_a_la_retraite_de_l_annee`) la prennent ici.
fn remuneration_de_la_retraite(brut: Montant, abattu: Option<Montant>, dfs: Option<&DeductionForfaitaire>) -> R<Montant> {
    if retraite_abattue(dfs) {
        return abattu.ok_or_else(|| socle::fichiers::absent(Assiette::BrutDfs.nom()));
    }
    Ok(brut)
}

/// CE QUI PRECEDE LA PAIE D'INTERMITTENT DE `eng` DANS L'ANNEE, POUR SA RETRAITE COMPLEMENTAIRE, sous le plafond annuel
/// `plafond_annuel` : les paies d'intermittent de son salarie des mois d'avant (`conges::retraite_des_mois_anterieurs`),
/// puis celles des contrats precedents du meme mois (`precedentes`), dans l'ordre de la saisie, comme `plafonnement`.
fn annee_de_la_retraite(
    ctx: &Contexte, eng: &Engagement, sal: &SalarieDuContrat, plafond_annuel: Montant, precedentes: &[Paie],
) -> R<droit::plafond::AnneeAnterieure> {
    let mois = eng.periode_payee()?.fin().mois_civil();
    let mut annee = crate::conges::retraite_des_mois_anterieurs(ctx, mois, &sal.id, plafond_annuel)?;
    for p in precedentes.iter().filter(|p| p.salarie.id == sal.id || p.engagement.salarie == eng.salarie) {
        ajouter_a_la_retraite_de_l_annee(&mut annee, p)?;
    }
    Ok(annee)
}

/// LA PAIE `p`, D'AVANT DANS L'ANNEE, AJOUTEE A L'ANNEE DE LA RETRAITE COMPLEMENTAIRE D'UN INTERMITTENT
/// (`droit::plafond::tranches_sous_le_plafond_annuel`) : la remuneration que la retraite compte
/// (`remuneration_de_la_retraite`), sans plafond (celui de l'annee est deja pose), et les tranches qu'elle a appliquees
/// telles qu'elle les a ecrites (7.5 ter : `t1_annuelle`, `t2_annuelle`, `cet_annuelle` ; la tranche 1 + 2 est leur somme,
/// et ni la prevoyance ni la CSG n'y ont de tranche). Une paie decoupee, periode par periode. Une paie qui ne cotise pas
/// sur l'annee (un cadre intermittent, sur ses tranches journalieres ; une paie au mois, sur ses tranches du mois) n'y
/// entre pas.
pub(crate) fn ajouter_a_la_retraite_de_l_annee(annee: &mut droit::plafond::AnneeAnterieure, p: &Paie) -> R<()> {
    if !p.periodes.is_empty() {
        for part in &p.periodes {
            ajouter_a_la_retraite_de_l_annee(annee, part)?;
        }
        return Ok(());
    }
    let Some((t1, t2, cet)) = tranches_de_l_annee_appliquees(p)? else { return Ok(()) };
    let appliquees = droit::plafond::Regularisees {
        t1,
        t2,
        t2_conventionnelle: Montant::ZERO,
        t1_t2: t1 + t2,
        cet,
        abattue_csg: Montant::ZERO,
    };
    let abattu = p.bases.get(&Assiette::BrutDfs).map(|b| b.en_euros(Assiette::BrutDfs)).transpose()?;
    annee.ajouter(remuneration_de_la_retraite(p.brut, abattu, p.deduction_forfaitaire.as_ref())?, Montant::ZERO, appliquees);
    Ok(())
}

/// LES TRANCHES DE L'ANNEE QU'UNE PAIE D'INTERMITTENT `p` A APPLIQUEES a sa retraite complementaire (tranche 1, tranche 2,
/// CET) : celles qu'elle a ecrites (7.5 ter : `t1_annuelle`, `t2_annuelle`, `cet_annuelle`). Seule une paie qui cotise
/// sur l'annee les ecrit (`retraite_sur_l_annee`, 7.5 ter) ; `None` : elle n'y cotise pas.
fn tranches_de_l_annee_appliquees(p: &Paie) -> R<Option<(Montant, Montant, Montant)>> {
    const LUE_PAR: &str = "la retraite complémentaire de l'année d'un intermittent lit (Audiens, Paramètres : les plafonds annuels)";
    if !p.bases.contains_key(&Assiette::T1Annuelle) {
        return Ok(None);
    }
    let lue = |a: Assiette| assiette_de_l_annee(p, a, LUE_PAR);
    Ok(Some((lue(Assiette::T1Annuelle)?, lue(Assiette::T2Annuelle)?, lue(Assiette::CetAnnuelle)?)))
}

/// Le plafond sur lequel se pose le forfait de sante d'un salarie au mois (`pss_periode`, la rubrique `mutuelle` et la
/// base 18 du bloc 79) : celui de la periode de paie, reduit au prorata des jours quand le contrat ne la couvre pas
/// (`plafond::plafond_permanent`), mais SANS la correction du temps partiel.
///
/// LES TEXTES. Les accords de sante fixent une cotisation « au titre de la couverture du seul salarie » en pourcentage
/// du « plafond mensuel de la securite sociale » (IDCC 1285, accord du 30/01/2026, art. 6 ; IDCC 3090, accord du
/// 19/07/2024, art. 13 : `atelier/reference/sources/kali-1285-accord-frais-de-sante-20260130-20261011.md`,
/// `kali-3090-accord-sante-permanents-20240719-20261011.md`) : c'est la valeur mensuelle que l'arrete fixe chaque annee
/// (CSS L. 241-3, D. 242-17, D. 242-18). La correction du temps partiel est autre chose : R. 242-2, I, dernier alinea,
/// corrige « le plafond » retenu pour le calcul des cotisations plafonnees du salarie, a la duree de son contrat ; un
/// forfait n'est pas plafonne. Et les memes accords, apres CSS R. 242-1-6, 2°, c, ouvrent une dispense aux « salaries a
/// temps partiel [...] dont l'adhesion au systeme de garanties les conduirait a s'acquitter d'une cotisation au moins
/// egale a 10 % de leur remuneration brute » : la cotisation d'un temps partiel ne baisse donc pas avec son temps de
/// travail. Le prorata des jours d'un contrat qui ne couvre pas le mois reste une INFERENCE plus ancienne (V3e), qu'aucun
/// de ces textes ne tranche. La prevoyance, elle, porte sur des tranches : « la tranche 1 porte sur la remuneration egale
/// au plafond de la securite sociale vise a l'article L. 241-3 » (IDCC 1285, titre XII, `kali-1285-titres-ii-iii-x-xi-
/// xii-20260923.md`, l. 73), le plafond que R. 242-2 corrige ; elle suit le plafond corrige, comme la retraite
/// complementaire (`plafond_de_l_engagement`). Textes : `atelier/reference/sources/plafond-temps-partiel/LISEZ-MOI.md`.
pub(crate) fn plafond_de_la_sante(ctx: &Contexte, e: &Engagement) -> R<Montant> {
    plafond_permanent(&e.periode_payee()?, &droit::plafond::JoursSansRemuneration::aucun(), None, &pss_du_contexte(ctx))
}

/// Le plafond d'un salarie a temps partiel, et ce que S21.G00.40.084 en declare.
///
/// LA PRORATISATION EST LA REGLE, Y RENONCER EST LE CHOIX.
/// - CSS L. 242-8 : pour un salarie a temps partiel (L. 3123-1 C. trav.), « il est opere un abattement d'assiette »
///   destine a compenser l'ecart avec un temps complet -- un imperatif ; R. 242-2, I, dernier alinea, en donne la
///   mesure (`plafond::plafond_permanent`) ;
/// - fiche net-entreprises 3369 (03/04/2026) : « Par defaut, le salarie a temps partiel cotise uniquement a hauteur de
///   la quotite de travail de son contrat. L'employeur peut choisir de ne pas proratiser le plafond de Securite sociale
///   pour tous les salaries a temps partiel de son etablissement » ; BOSS, assiette generale, § 760 : « Le salarie a
///   temps partiel et l'employeur ont la possibilite de renoncer a cet ajustement » ;
/// - L. 242-10, 1° : L. 242-8 « n'est pas applicable » aux salaries dont l'emploi donne lieu a des taux, assiettes ou
///   montants specifiques ; BOSS § 1090 : ils « ne sont pas eligibles a la reduction du plafond », et « sont notamment
///   vises les artistes du spectacle ».
/// Textes : `atelier/reference/sources/plafond-temps-partiel/LISEZ-MOI.md`.
///
/// Sa forme ecrite dans une paie (`paie_ecrite::FormeDePaie::plafond_temps_partiel`) : `non_concerne`, `proratise`,
/// `entier`, `exclu` ; une autre valeur rend la paie illisible.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlafondTempsPartiel {
    /// Un intermittent (contrat d'usage, sans duree du travail : 40.014 = 99), ou un temps plein : la question ne se
    /// pose pas, le plafond n'est pas corrige a la quotite, et 40.084 ne s'ecrit pas (norme 2027.1.2 : conditionnelle).
    NonConcerne,
    /// Temps partiel : le plafond se corrige a la duree du contrat (la regle).
    Proratise,
    /// Temps partiel, et l'etablissement a renonce a la proratisation (la fiche entreprise le dit) : le plafond entier.
    Entier,
    /// Temps partiel d'un artiste du spectacle : exclu de la proratisation (L. 242-10, 1° ; BOSS § 1090), le plafond
    /// entier quel que soit le choix de l'etablissement.
    Exclu,
}

/// Le plafond du salarie `sal` (sa fiche telle que la paie la lit, pour son statut) a temps partiel de l'engagement `e`,
/// d'apres la fiche entreprise `ent` (`plafond_temps_partiel`). Le temps partiel est celui que la DSN declare en
/// S21.G00.40.014 (20) : une duree du contrat inferieure a la duree de reference (L. 3123-1 du code du travail,
/// `engagement::modalite_temps_travail`). Sans reponse dans la fiche, la regle : proratise (L. 242-8, fiche 3369). Un
/// artiste est exclu (L. 242-10, 1° ; BOSS § 1090) : la reponse de la fiche ne le concerne pas. Le calcul l'ecrit dans
/// la paie (`Contenu::plafond_temps_partiel`), que la DSN lit.
pub(crate) fn plafond_temps_partiel(ent: &ChampsEntreprise, e: &Engagement, sal: &ChampsSalarie) -> R<PlafondTempsPartiel> {
    let Some(au_mois) = e.nat()?.au_mois() else {
        return Ok(PlafondTempsPartiel::NonConcerne);
    };
    if e.modalite_temps_travail()? != droit::natures::ModaliteDuTempsDeTravail::TempsPartiel {
        return Ok(PlafondTempsPartiel::NonConcerne);
    }
    // « Entier » : l'etablissement renonce a la proratisation pour tous ses salaries a temps partiel (fiche 3369 ; BOSS,
    // assiette generale, § 760).
    let renonce = ent.permanent.ouvrir(&au_mois).plafond_temps_partiel.dite() == Some(&droit::fiche::PlafondTempsPartiel::Entier);
    if statut_de(sal)? == Statut::Artiste {
        return Ok(PlafondTempsPartiel::Exclu);
    }
    Ok(if renonce { PlafondTempsPartiel::Entier } else { PlafondTempsPartiel::Proratise })
}

/// (duree du contrat, duree de reference) de la correction du plafond d'un temps partiel (R. 242-2, I, dernier alinea) :
/// la duree inscrite au contrat (S21.G00.40.013), rapportee a « la plus courte des durees mentionnees au 1° de
/// l'article L. 3123-1 du code du travail » -- la duree legale (151,67 h, `contrats-base.json`, L. 3121-27) ou, si
/// elle est plus courte, la duree collective de l'entreprise (S21.G00.40.012). Le texte ajoute a la duree du contrat
/// « le nombre d'heures complementaires » du mois : celles que le moteur compose du fait de la carte (`heures_en_plus`) ;
/// une ligne d'heures tapee a la main ne dit pas si elle en est, et refuse plutot que de le deviner.
pub(crate) fn duree_du_temps_partiel(ctx: &Contexte, e: &Engagement) -> R<(socle::grandeurs::Heures, socle::grandeurs::Heures)> {
    let nat = e.nat()?;
    if let Some(l) = e.lignes.iter().find(|l| *l.rubrique() == droit::catalogue::HEURE) {
        return Err(socle::refus!(
            "{} : le plafond de Securite sociale de ce salarie a temps partiel se corrige a la duree de son \
contrat << majoree du nombre d'heures complementaires >> du mois (CSS R. 242-2, I, dernier alinea), et la paie porte des \
heures en plus du salaire mensuel ({} h) : heures complementaires ou non, le logiciel ne le sait pas. Refus : il ne les \
devine pas.",
            e.qui(),
            heures_de(ctx, l)?.iter().map(|h| socle::grandeurs::ecrit::texte::dit(*h)).collect::<String>()
        ));
    }
    let legale = nat.quotite_reference_defaut().ok_or_else(|| socle::echec::livree(&droit::natures::BASE, &format!("{}.quotite_reference_defaut", nat.libelle)))?;
    let reference = match e.quotite_reference()? {
        Some(r) => legale.min(r.heures("La durée collective du travail")?),
        None => legale,
    };
    // « majoree du nombre d'heures complementaires » : celles que le moteur compose du fait du mois (`heures_en_plus`).
    let hc = crate::heures_en_plus::HEURE_COMPLEMENTAIRE;
    let unite = droit::catalogue::rubrique(&ctx.racine, &hc, &ctx.lue_au)?.unite;
    let complementaires = crate::heures_en_plus::payees(&hc, e.lignes.iter().map(|l| (l.rubrique(), l.quantite(unite))))?;
    Ok((e.quotite()?.heures("La durée d'un temps partiel")? + complementaires, reference))
}

/// Les heures d'une ligne d'heures normales, dans l'unite de sa rubrique (`rubriques-base.json`, `heure` : des heures),
/// que dit le refus du plafond d'un temps partiel ; `None` : la case n'est pas remplie, ou sa rubrique ne compte pas des
/// heures.
fn heures_de(ctx: &Contexte, l: &crate::LignePaie) -> R<Option<socle::grandeurs::Heures>> {
    let unite = droit::catalogue::rubrique(&ctx.racine, l.rubrique(), &ctx.lue_au)?.unite;
    Ok(match l.quantite(unite) {
        Some(QuantiteDeLigne::Heures(h)) => Some(h),
        _ => None,
    })
}

impl Paie {
    /// La forme canonique d'une paie, ecriture stable pour les empreintes et les oracles : chaque avis par son seul texte.
    pub fn canon(&self) -> R<FormeDePaie<String>> {
        self.forme(&|a: &Avis| a.texte.clone(), false)
    }

    /// Sa forme (`paie_ecrite::FormeDePaie`), chaque avis par `avis` ; `ecrite` : celle que la validation ecrit
    /// (`relire::ecrire`), les lignes de l'engagement telles quelles et le plafond d'un temps partiel.
    pub(crate) fn forme<A>(&self, avis: &dyn Fn(&Avis) -> A, ecrite: bool) -> R<FormeDePaie<A>> {
        let mut engagement = self.engagement.canon()?;
        // Le jour du versement que la paie declare et imprime (`Engagement::versement_dit`) : le contrat le dit ; une paie
        // importee dont les pieces ne le disent pas l'ecrit vide.
        engagement.date_paiement = self.engagement.versement_dit()?;
        if ecrite {
            // Telles qu'elles s'ecrivent : sans ce que le moteur pose pour l'ecran (`ligne::Marques`), qui ne s'ecrit pas.
            engagement.lignes = self.engagement.lignes.iter().map(crate::LignePaie::sans_marques).collect();
        }
        Ok(FormeDePaie {
            assiette_chomage: self.assiette_chomage,
            avertissements: self.avertissements.iter().map(avis).collect(),
            base_csg: self.base_csg,
            bases: self.bases.clone(),
            brut: self.brut,
            cout_employeur: self.cout_employeur,
            deduction_forfaitaire: self.deduction_forfaitaire.clone(),
            engagement,
            heures: self.heures,
            heures_absence: self.heures_absence,
            heures_travaillees: self.heures_travaillees,
            lignes: self.lignes.clone(),
            net_a_payer: self.net_a_payer,
            net_imposable: self.net_imposable,
            net_social: self.net_social,
            numero_bulletin: self.numero_bulletin.clone(),
            reduction_generale: self.reduction_generale.clone(),
            remuneration: self.remuneration.clone(),
            salaire_de_base: self.salaire_de_base,
            salaire_retabli: self.salaire_retabli,
            salarie: Cle { cle: self.salarie.id.clone() },
            smic: socle::grandeurs::courte(self.smic),
            spectacle: self.spectacle.as_ref().map(|s| Cle { cle: s.cle.clone() }),
            total_pat: self.total_pat,
            total_sal: self.total_sal,
            plafond: self.plafond,
            plafond_urssaf: self.plafond_urssaf,
            part_abattue_csg: Some(self.part_abattue_csg),
            periodes: self.periodes.iter().map(|p| p.forme(avis, ecrite)).collect::<R<_>>()?,
            plafond_temps_partiel: ecrite.then_some(self.plafond_temps_partiel),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ligne(code: &str, montant: &str, heures: &str, base: bool, retabli: bool, soumis: bool) -> LigneBrut {
        LigneBrut {
            code: droit::catalogue::CodeDeRubrique::lire(code).unwrap(), libelle: code.into(), unite: Unite::Mois, paiement: PaiementDeLigne::PartDuMois(socle::grandeurs::Coefficient::UN, lue(montant)),
            montant: lue(montant), heures_annexes: lue(heures), soumis, imposable: soumis, conges_spectacles: false,
            salaire_de_base: base, retabli, conges_pris: false, dsn_bloc52: None, arret_de_travail: retabli, dsn_autres_bruts: None,
            precision: None,
        }
    }

    /// La grandeur que l'exemple de la fiche ecrit.
    fn lue<G: socle::grandeurs::Grandeur>(t: &str) -> G {
        socle::grandeurs::lu::en_texte(t).unwrap()
    }

    /// CSS D. 241-7, IV, dernier alinea, et la fiche net-entreprises 2681 (`atelier/reference/sources/
    /// net-entreprises-base-connaissances/net-entreprises-fiche2681-reduction-generale-absence-proratisation-smic-types-028-029-
    /// 20260609.txt`), premier exemple : un salaire de 2 000 EUR, une absence qui en retire la moitie, une prime annuelle
    /// non affectee par l'absence ; « 028 » 1 000, « 029 » 2 000, « Smic = 1766.92 x 1000 /2000 = 883.46 » (SMIC horaire
    /// 11,65). Une IJSS subrogee, non soumise, n'entre pas au rapport ; un maintien integral garde le SMIC entier.
    #[test]
    fn le_smic_d_un_mois_d_absence_suit_le_rapport_des_remunerations() {
        let smic = Some(lue::<MontantUnitaire>("11.65"));
        // 35 x 52 / 12 heures, comme la fiche compte le SMIC du mois (1 766,92).
        let mois = ligne("mois", "2000", "151.6667", true, false, true);
        let prime = ligne("prime", "3833.33", "0", false, false, true);
        let absence = ligne("absence_maladie", "-1000", "-75.83", false, true, true);
        let a = smic_d_une_absence(&[mois.clone(), absence.clone(), prime], smic).unwrap();
        assert_eq!((a.percue, a.habituelle), (lue("1000"), lue("2000")));
        assert_eq!((a.heures.unwrap() * smic.unwrap()).au_centime(), lue("883.46"));
        let subrogee = ligne(droit::catalogue::RubriqueDuMois::IjssNettes.code().as_str(),"300", "0", false, false, false);
        assert_eq!(smic_d_une_absence(&[mois.clone(), absence.clone(), subrogee], smic).unwrap().percue, lue("1000"));
        let maintien = ligne("maintien_salaire", "1000", "0", false, true, true);
        let entier = smic_d_une_absence(&[mois.clone(), absence, maintien], smic).unwrap();
        assert_eq!((entier.heures.unwrap() * smic.unwrap()).au_centime(), lue("1766.92"));
        assert!(smic_d_une_absence(&[mois], smic).is_none());
    }
}
