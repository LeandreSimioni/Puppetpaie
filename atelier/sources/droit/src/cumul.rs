//! Les cumuls des annexes 8 et 10 : ce qui s'accumule d'un bulletin a l'autre. Les heures et cachets declares, par
//! fenetre (mois, annee civile, 12 mois), et ce qui manque aux 507 heures. Le droit compte ce qu'on lui passe (les
//! lignes declarees, `Declare`) ; les lire dans les paies et l'historique est l'affaire de l'engagement.

use std::collections::BTreeMap;
use std::path::Path;

use crate::catalogue::{annexes, rubrique};
use crate::fiche::Lue;
use crate::contexte::{Contexte, IdSalarie};
use socle::date::{Duree, EnJours, Jour, Periode};
use socle::echec::R;
use socle::grandeurs::{Cachets, Heures, HeuresParUnite, Montant};

/// Les constantes du decompte, lues dans les donnees livrees (le catalogue des rubriques et `annexes`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Constantes {
    /// Heures qu'un cachet fait compter (12).
    pub heures_par_cachet: HeuresParUnite,
    /// Cachets convertis en heures, par mois (28).
    pub cachets_par_mois: Cachets,
    /// Seuil d'ouverture des droits (507).
    pub seuil_heures: Heures,
    /// Les heures de formation qui s'assimilent au plus, l'enseignement retenu s'y imputant (338 : annexes VIII et X,
    /// art. 3 §1).
    pub plafond_formation: Heures,
    /// Fenetre des droits, en jours (365).
    pub fenetre_jours: Duree<EnJours>,
}


impl Constantes {
    pub fn lire(racine: &Path, jour: &Jour) -> R<Constantes> {
        let cachet = rubrique(racine, &crate::catalogue::CACHET, jour)?;
        let a = annexes()?;
        Ok(Constantes {
            heures_par_cachet: cachet.heures_forfait.ok_or_else(|| socle::fichiers::absent("cachet.heures_forfait"))?,
            cachets_par_mois: cachet.plafond_mensuel.ok_or_else(|| socle::fichiers::absent("cachet.plafond_mensuel"))?,
            seuil_heures: a.seuil_heures,
            plafond_formation: a.plafond_formation_heures,
            fenetre_jours: a.fenetre_jours,
        })
    }
}

/// `plafond_enseignement` : 70 h, ou 120 h a partir de 50 ans a la date
/// donnee ; sans date de naissance, le plafond bas.
pub fn plafond_enseignement(racine: &Path, date_naissance: Option<Jour>, a_la_date: Jour) -> R<Heures> {
    let r = rubrique(racine, &const { crate::catalogue::CodeDeRubrique::du_moteur("enseignement") }, &a_la_date)?;
    let Some(p) = r.plafond_annuel_heures else {
        return Err(socle::echec::livree(&const { socle::fichiers::CheminRelatif::du_moteur("rubriques-base.json") }, "rubriques.enseignement.plafond_annuel_heures"));
    };
    let (bas, haut) = (p.moins_de_50_ans, p.cinquante_ans_et_plus);
    let Some(n) = date_naissance else { return Ok(bas) };
    let cinquante_ans = socle::date::Anciennete::ans(50);
    Ok(if socle::date::Anciennete::age(&n, &a_la_date) >= cinquante_ans { haut } else { bas })
}

/// Une ligne declaree : un contrat de l'historique ou un mois de nos paies. Ses heures se comptent en heures entieres.
/// Elle se situe par sa fin (la fenetre d'affiliation compte les contrats qui y finissent, les cachets se rattachent au
/// mois de la fin) ; son debut, quand il se dit : un contrat prevu peut ne dire que sa fin
/// (`routes::accueil::droits_des_pieces`).
#[derive(Debug, Clone, PartialEq)]
pub struct Declare {
    pub salarie: IdSalarie,
    pub debut: Option<Jour>,
    pub fin: Jour,
    pub heures: Heures,
    pub cachets: Cachets,
    /// Son brut ; aucun : un contrat prevu qui ne le dit pas (`routes::accueil::droits_des_pieces`).
    pub brut: Option<Montant>,
    /// Part de `heures` qui est de l'enseignement (dedans, pas en plus).
    pub heures_enseignement: Heures,
    /// Les heures de formation suivies (annexes VIII et X, art. 3 §1 : « les actions concourant au développement des
    /// compétences [...] sont assimilées à des heures de travail ») : en plus de `heures`, pas dedans ; elles ne
    /// comptent que pour les 507 heures, sous leur plafond (`Position::heures_formation_retenues`). Une paie d'une
    /// compagnie n'en porte pas : zero.
    pub heures_formation: Heures,
}

impl Declare {
    /// Le mois de la fin, auquel se rattachent les cachets.
    pub fn mois(&self) -> socle::mois::Mois {
        self.fin.mois_civil()
    }

    pub fn heures_equivalentes(&self, c: &Constantes) -> Heures {
        self.heures + self.cachets * c.heures_par_cachet
    }

    /// Tout sauf l'enseignement.
    pub fn heures_montant(&self, c: &Constantes) -> Heures {
        self.heures_equivalentes(c) - self.heures_enseignement
    }
}

/// Etat d'un salarie sur une fenetre.
#[derive(Debug, Clone)]
pub struct Position {
    pub constantes: Constantes,
    pub racine: std::path::PathBuf,
    pub salarie: IdSalarie,
    pub periode: Periode,
    pub libelle_fenetre: String,
    pub engagements: Vec<Declare>,
    /// Pour le plafond d'enseignement (age a la fin de la fenetre).
    pub date_naissance: Option<Jour>,
}

impl Position {
    pub fn cachets(&self) -> Cachets {
        self.engagements.iter().map(|e| e.cachets).sum()
    }

    pub fn heures(&self) -> Heures {
        self.engagements.iter().map(|e| e.heures).sum()
    }

    /// Le total declare, enseignement compris.
    pub fn heures_equivalentes(&self) -> Heures {
        self.engagements.iter().map(|e| e.heures_equivalentes(&self.constantes)).sum()
    }

    /// `{mois: cachets declares}`.
    pub fn cachets_par_mois(&self) -> BTreeMap<socle::mois::Mois, Cachets> {
        let mut out = BTreeMap::new();
        for e in &self.engagements {
            *out.entry(e.mois()).or_insert(Cachets::ZERO) += e.cachets;
        }
        out
    }

    /// Les cachets que les annexes convertissent : 28 par mois au plus.
    pub fn cachets_retenus(&self) -> Cachets {
        self.cachets_par_mois().values().map(|n| (*n).min(self.constantes.cachets_par_mois)).sum()
    }

    pub fn cachets_perdus(&self) -> Cachets {
        self.cachets() - self.cachets_retenus()
    }

    /// Les mois ou le plafond a mordu.
    pub fn mois_plafonnes(&self) -> Vec<socle::mois::Mois> {
        self.cachets_par_mois()
            .into_iter()
            .filter(|(_, n)| *n > self.constantes.cachets_par_mois)
            .map(|(m, _)| m)
            .collect()
    }

    pub fn heures_retenues(&self) -> Heures {
        self.heures() + self.cachets_retenus() * self.constantes.heures_par_cachet
    }

    pub fn heures_enseignement(&self) -> Heures {
        self.engagements.iter().map(|e| e.heures_enseignement).sum()
    }

    pub fn plafond_enseignement(&self) -> R<Heures> {
        plafond_enseignement(&self.racine, self.date_naissance, self.periode.fin())
    }

    pub fn heures_enseignement_retenues(&self) -> R<Heures> {
        Ok(self.heures_enseignement().min(self.plafond_enseignement()?))
    }

    /// Les heures du MONTANT de l'ARE : tout sauf l'enseignement.
    pub fn heures_montant(&self) -> Heures {
        self.heures_retenues() - self.heures_enseignement()
    }

    /// Les heures de formation des lignes.
    pub fn heures_formation(&self) -> Heures {
        self.engagements.iter().map(|e| e.heures_formation).sum()
    }

    /// LES HEURES DE FORMATION RETENUES : dans la limite du plafond (338 h), sur lequel l'enseignement retenu s'impute
    /// (annexes VIII et X, art. 3 §1 : « Les heures d'enseignement ainsi prises en compte sont imputées sur le contingent
    /// des actions de formation »).
    pub fn heures_formation_retenues(&self) -> R<Heures> {
        let reste = (self.constantes.plafond_formation - self.heures_enseignement_retenues()?).max(Heures::ZERO);
        Ok(self.heures_formation().min(reste))
    }

    /// Les heures de formation au-dela de leur plafond.
    pub fn formation_perdue(&self) -> R<Heures> {
        Ok(self.heures_formation() - self.heures_formation_retenues()?)
    }

    /// Les heures qui comptent pour les 507 (l'enseignement et la formation y entrent, plafonnes).
    pub fn heures_ouverture(&self) -> R<Heures> {
        Ok(self.heures_montant() + self.heures_enseignement_retenues()? + self.heures_formation_retenues()?)
    }

    pub fn enseignement_perdu(&self) -> R<Heures> {
        Ok(self.heures_enseignement() - self.heures_enseignement_retenues()?)
    }

    /// Le brut des lignes qui le disent.
    pub fn brut(&self) -> Montant {
        self.engagements.iter().filter_map(|e| e.brut).fold(Montant::ZERO, |a, b| a + b)
    }

    /// Heures restant a acquerir pour atteindre le seuil ; 0 si atteint.
    pub fn manque(&self) -> R<Heures> {
        Ok((self.constantes.seuil_heures - self.heures_ouverture()?).max(Heures::ZERO))
    }

    pub fn atteint(&self) -> R<bool> {
        Ok(self.heures_ouverture()? >= self.constantes.seuil_heures)
    }
}


// ------------------------------------------------------------------ fenetres

/// Une fenetre : sa periode, et son libelle.
pub type Fenetre = (Periode, String);

/// L'annee civile, arretee a la fin du mois `jusqu_au`.
pub fn fenetre_annee_civile(jusqu_au: socle::mois::Mois) -> R<Fenetre> {
    let annee = jusqu_au.annee();
    Ok((Periode::du_au(annee.premier_jour(), jusqu_au.dernier_jour())?, format!("annee civile {}", annee)))
}

/// LES `jours` JOURS QUI FINISSENT A `fin`, bornes comprises : le guide de France Travail
/// (`atelier/reference/sources/chomage-intermittents/francetravail-guide-intermittents-20261020.txt`, sur deux lignes de
/// sa colonne de gauche) : « Exemple : FCT du 10 février 2023. Période de référence : du 11 février 2022 au 10 février
/// 2023. » (365 jours).
fn jours_jusqu_au(fin: Jour, jours: Duree<EnJours>) -> R<Periode> {
    Periode::du_au(fin.avant(jours)?.apres(Duree::jours(1))?, fin)
}

/// Les 12 mois (les 365 jours) qui finissent a une date.
pub fn fenetre_douze_mois(c: &Constantes, fin: Jour) -> R<Fenetre> {
    Ok((jours_jusqu_au(fin, c.fenetre_jours)?, format!("12 mois au {}", fin.jj_mm_aaaa())))
}

/// LA FENETRE ALLONGEE DE LA READMISSION (annexes VIII et X, art. 9 §1 b : « au-delà du 365ème jour précédant la fin du
/// contrat de travail ») : les 365 jours qui finissent a `fin`, plus `en_plus`.
pub fn fenetre_allongee(c: &Constantes, fin: Jour, en_plus: Duree<EnJours>) -> R<Fenetre> {
    let jours = c.fenetre_jours.plus(en_plus);
    Ok((jours_jusqu_au(fin, jours)?, format!("{} jours au {}, réadmission", jours.jours_a_ecrire(), fin.jj_mm_aaaa())))
}

/// Les engagements du salarie dont la FIN est dans la fenetre. `racine` : la compagnie, dont le catalogue dit le plafond
/// de l'enseignement ; sans compagnie (les pieces d'un intermittent), un dossier sans `config/` : le catalogue livre.
pub fn position(
    racine: &Path,
    c: &Constantes,
    engagements: &[Declare],
    salarie: &IdSalarie,
    fenetre: &Fenetre,
    date_naissance: Option<Jour>,
) -> Position {
    let (periode, libelle) = fenetre;
    Position {
        constantes: *c,
        racine: racine.to_path_buf(),
        salarie: salarie.clone(),
        periode: *periode,
        libelle_fenetre: libelle.clone(),
        engagements: engagements.iter().filter(|e| e.salarie == *salarie && periode.contient(&e.fin)).cloned().collect(),
        date_naissance,
    }
}

/// Date de naissance du salarie, ou `None`.
pub fn naissance(ctx: &Contexte, salarie: &IdSalarie) -> R<Option<Jour>> {
    Ok(ctx.salaries.get(salarie).and_then(|f| f.date_naissance.dite().copied()))
}

/// Fin du dernier contrat declare : le depart de la fenetre par defaut.
pub fn derniere_fin(engagements: &[Declare], salarie: &IdSalarie) -> Option<Jour> {
    engagements.iter().filter(|e| e.salarie == *salarie).map(|e| e.fin).max()
}

/// L'AFFILIATION D'UN SALARIE D'UNE COMPAGNIE au regard des annexes 8 et 10 : le meme compte que les droits d'un
/// intermittent depuis ses pieces (`allocation::affiliation_des_droits`), sa date anniversaire (notifiee par France
/// Travail, dans sa fiche) declenchant la readmission ; `jusqu_au` : le jour ou l'on se place (aucun : tout ce qui est
/// declare).
pub fn position_droits(
    ctx: &Contexte,
    salarie: &IdSalarie,
    engagements: &[Declare],
    anniversaire: Option<Jour>,
    jusqu_au: Option<Jour>,
) -> R<crate::allocation::Affiliation> {
    let c = Constantes::lire(&ctx.racine, &ctx.lue_au)?;
    let r = &crate::allocation::allocation()?.readmission;
    crate::allocation::affiliation_des_droits(&ctx.racine, &c, r, engagements, salarie, anniversaire, jusqu_au, naissance(ctx, salarie)?)
}

// ------------------------------------------------------- bloc du bulletin

/// Une colonne du bloc de cumul imprime en bas du bulletin.
#[derive(Debug, Clone, PartialEq)]
pub struct Colonne {
    pub titre: String,
    pub sous_titre: String,
    pub cachets: Cachets,
    pub heures: Heures,
    pub heures_equivalentes: Heures,
    pub brut: Montant,
    pub enseignement: Heures,
    pub heures_ouverture: Option<Heures>,
}

/// Les colonnes : le mois, l'annee civile (arretee a la fin du mois), et
/// la fenetre des droits quand `anniversaire_droits` est dans la fiche.
pub fn tableau_cumul(
    ctx: &Contexte,
    salarie: &IdSalarie,
    mois: socle::mois::Mois,
    engagements: &[Declare],
) -> R<Vec<Colonne>> {
    let c = Constantes::lire(&ctx.racine, &ctx.lue_au)?;
    let annee = mois.annee();
    let ne = naissance(ctx, salarie)?;
    let p_mois = position(&ctx.racine, &c, engagements, salarie, &(Periode::du_mois(mois), "periode".to_string()), ne);
    let p_annee = position(&ctx.racine, &c, engagements, salarie, &fenetre_annee_civile(mois)?, ne);

    let col = |titre: &str, sous: String, p: &Position| -> R<Colonne> {
        Ok(Colonne {
            titre: titre.to_string(),
            sous_titre: sous,
            cachets: p.cachets(),
            heures: p.heures(),
            heures_equivalentes: p.heures_equivalentes(),
            brut: p.brut(),
            enseignement: p.heures_enseignement(),
            heures_ouverture: Some(p.heures_ouverture()?),
        })
    };
    let mut colonnes = vec![
        col("Période", format!("{:02}/{}", mois.numero(), annee), &p_mois)?,
        col("Cumul", format!("année {}", annee), &p_annee)?,
    ];
    // La colonne des droits, quand la fiche porte la date anniversaire : le compte de la route `droits`
    // (`position_droits`), a la derniere fin de contrat jusqu'a la fin du mois ; son sous-titre, la fenetre telle que
    // l'affiliation la nomme (allongee a la readmission).
    if let Some(anniv) = anniversaire_configure(ctx, salarie)? {
        let a = position_droits(ctx, salarie, engagements, Some(anniv), Some(mois.dernier_jour()))?;
        colonnes.push(col("Droits", a.position.libelle_fenetre.clone(), &a.position)?);
    }
    Ok(colonnes)
}

/// Date anniversaire des droits, si elle est dans `config/salaries.json`
/// (`anniversaire_droits`) ; absente tant que France Travail ne l'a pas notifiee.
pub fn anniversaire_configure(ctx: &Contexte, salarie: &IdSalarie) -> R<Option<Jour>> {
    Ok(ctx.salaries.get(salarie).and_then(|f| f.anniversaire_droits.dite().copied()))
}
