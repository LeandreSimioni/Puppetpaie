//! Les cumuls des annexes 8 et 10 : ce qui s'accumule d'un bulletin a l'autre. Les heures et cachets declares, par
//! fenetre (mois, annee civile, 12 mois), et ce qui manque aux 507 heures. Le droit compte ce qu'on lui passe (les
//! lignes declarees, `Declare`) ; les lire dans les paies et l'historique est l'affaire de l'engagement.

use std::collections::BTreeMap;
use std::path::Path;

use crate::catalogue::{annexes, rubrique};
use crate::fiche::Lue;
use crate::contexte::{aujourdhui, Contexte, IdSalarie};
use socle::date::{Duree, EnJours, Jour, Periode};
use socle::echec::R;
use socle::grandeurs::{Cachets, Heures, HeuresParUnite, Jours, Montant};

/// Les constantes du decompte, lues dans les donnees livrees (le catalogue des rubriques et `annexes`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Constantes {
    /// Heures qu'un cachet fait compter (12).
    pub heures_par_cachet: HeuresParUnite,
    /// Cachets convertis en heures, par mois (28).
    pub cachets_par_mois: Cachets,
    /// Seuil d'ouverture des droits (507).
    pub seuil_heures: Heures,
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
#[derive(Debug, Clone, PartialEq)]
pub struct Declare {
    pub salarie: IdSalarie,
    pub periode: Periode,
    pub heures: Heures,
    pub cachets: Cachets,
    pub jours: Jours,
    pub brut: Montant,
    /// Part de `heures` qui est de l'enseignement (dedans, pas en plus).
    pub heures_enseignement: Heures,
}

impl Declare {
    /// Le mois de la fin, auquel se rattachent les cachets.
    pub fn mois(&self) -> socle::mois::Mois {
        self.periode.fin().mois_civil()
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

    /// Les heures qui comptent pour les 507 (l'enseignement y entre, plafonne).
    pub fn heures_ouverture(&self) -> R<Heures> {
        Ok(self.heures_montant() + self.heures_enseignement_retenues()?)
    }

    pub fn enseignement_perdu(&self) -> R<Heures> {
        Ok(self.heures_enseignement() - self.heures_enseignement_retenues()?)
    }

    pub fn jours(&self) -> Jours {
        self.engagements.iter().map(|e| e.jours).sum()
    }

    pub fn brut(&self) -> Montant {
        self.engagements.iter().fold(Montant::ZERO, |a, e| a + e.brut)
    }

    /// Heures restant a acquerir pour atteindre le seuil ; 0 si atteint.
    pub fn manque(&self) -> R<Heures> {
        Ok((self.constantes.seuil_heures - self.heures_ouverture()?).max(Heures::ZERO))
    }

    pub fn atteint(&self) -> R<bool> {
        Ok(self.heures_ouverture()? >= self.constantes.seuil_heures)
    }

    /// Les jours qui restent avant la fin de la fenetre, du lendemain d'`aujourdhui` a sa fin ; aucun quand elle est
    /// passee ou finit aujourd'hui.
    pub fn jours_restants(&self, aujourdhui: Jour) -> Duree<EnJours> {
        Duree::ecoulee(&aujourdhui, &self.periode.fin())
    }

    /// Les cachets qu'il manque pour le seuil, le dernier entame compte ; un cachet de zero heure refuse.
    pub fn cachets_manquants(&self) -> R<Cachets> {
        self.manque()?.cachets_pour(self.constantes.heures_par_cachet).ok_or_else(|| socle::fichiers::absent("cachet.heures_forfait"))
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

/// Les 12 mois precedant une date.
pub fn fenetre_douze_mois(c: &Constantes, fin: Jour) -> R<Fenetre> {
    Ok((Periode::du_au(fin.avant(c.fenetre_jours)?, fin)?, format!("12 mois au {}", fin.jj_mm_aaaa())))
}

/// Les 12 mois PRECEDANT la date anniversaire des droits : l'anniversaire est
/// la fin de la fenetre, jamais son debut.
pub fn fenetre_anniversaire(c: &Constantes, anniversaire: Jour) -> R<Fenetre> {
    Ok((
        Periode::du_au(anniversaire.avant(c.fenetre_jours)?, anniversaire)?,
        format!("12 mois au {} (anniversaire des droits)", anniversaire.jj_mm_aaaa()),
    ))
}

/// Les engagements du salarie dont la FIN est dans la fenetre.
pub fn position(
    ctx: &Contexte,
    c: &Constantes,
    engagements: &[Declare],
    salarie: &IdSalarie,
    fenetre: &Fenetre,
    date_naissance: Option<Jour>,
) -> Position {
    let (periode, libelle) = fenetre;
    Position {
        constantes: *c,
        racine: ctx.racine.clone(),
        salarie: salarie.clone(),
        periode: *periode,
        libelle_fenetre: libelle.clone(),
        engagements: engagements.iter().filter(|e| e.salarie == *salarie && periode.contient(&e.periode.fin())).cloned().collect(),
        date_naissance,
    }
}

/// Date de naissance du salarie, ou `None`.
pub fn naissance(ctx: &Contexte, salarie: &IdSalarie) -> R<Option<Jour>> {
    Ok(ctx.salarie(salarie)?.and_then(|f| f.date_naissance.dite().copied()))
}

/// Fin du dernier contrat declare : le depart de la fenetre par defaut.
pub fn derniere_fin(engagements: &[Declare], salarie: &IdSalarie) -> Option<Jour> {
    engagements.iter().filter(|e| e.salarie == *salarie).map(|e| e.periode.fin()).max()
}

/// Position au regard des annexes 8 et 10. `anniversaire` (notifie par France
/// Travail) prime ; sinon la fenetre est les 12 mois precedant `a_la_date` ou
/// la fin du dernier contrat declare.
pub fn position_droits(
    ctx: &Contexte,
    salarie: &IdSalarie,
    engagements: &[Declare],
    anniversaire: Option<Jour>,
    a_la_date: Option<Jour>,
) -> R<Position> {
    let c = Constantes::lire(&ctx.racine, &ctx.lue_au)?;
    let f = match anniversaire {
        Some(a) => fenetre_anniversaire(&c, a)?,
        None => {
            let fin = match a_la_date.or_else(|| derniere_fin(engagements, salarie)) {
                Some(j) => j,
                None => aujourdhui()?,
            };
            fenetre_douze_mois(&c, fin)?
        }
    };
    Ok(position(ctx, &c, engagements, salarie, &f, naissance(ctx, salarie)?))
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
    /// Heures restantes avant le seuil (colonne droits).
    pub reste: Option<Heures>,
    /// Date d'anniversaire (colonne droits).
    pub echeance: Option<Jour>,
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
    let p_mois = position(ctx, &c, engagements, salarie, &(Periode::du_mois(mois), "periode".to_string()), ne);
    let p_annee = position(ctx, &c, engagements, salarie, &fenetre_annee_civile(mois)?, ne);

    let col = |titre: &str, sous: String, p: &Position, reste: Option<Heures>, echeance: Option<Jour>| -> R<Colonne> {
        Ok(Colonne {
            titre: titre.to_string(),
            sous_titre: sous,
            cachets: p.cachets(),
            heures: p.heures(),
            heures_equivalentes: p.heures_equivalentes(),
            brut: p.brut(),
            reste,
            echeance,
            enseignement: p.heures_enseignement(),
            heures_ouverture: Some(p.heures_ouverture()?),
        })
    };
    let mut colonnes = vec![
        col("Période", format!("{:02}/{}", mois.numero(), annee), &p_mois, None, None)?,
        col("Cumul", format!("année {}", annee), &p_annee, None, None)?,
    ];
    if let Some(anniv) = anniversaire_configure(ctx, salarie)? {
        let p_droits = position(ctx, &c, engagements, salarie, &fenetre_anniversaire(&c, anniv)?, ne);
        let reste = p_droits.manque()?;
        colonnes.push(col("Droits", format!("au {}", anniv.jj_mm_aaaa()), &p_droits, Some(reste), Some(anniv))?);
    }
    Ok(colonnes)
}

/// Date anniversaire des droits, si elle est dans `config/salaries.json`
/// (`anniversaire_droits`) ; absente tant que France Travail ne l'a pas notifiee.
pub fn anniversaire_configure(ctx: &Contexte, salarie: &IdSalarie) -> R<Option<Jour>> {
    Ok(ctx.salarie(salarie)?.and_then(|f| f.anniversaire_droits.dite().copied()))
}
