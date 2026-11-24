//! La fiche recapitulative du dossier d'un salarie : les deux etats de l'onglet Exports reduits a lui -- les heures et
//! les cachets, le recapitulatif annuel (`etats_paie`) -- en un PDF ; et ce qu'un spectacle ou un salarie a coute dans
//! l'annee.
//!
//! Elle lit ce que les etats lisent, et rien d'autre : les paies ecrites des engagements valides et celles des
//! engagements importes. Le recapitulatif annuel et le cout de l'annee portent les paies importees avec ce que les
//! pieces de l'ancien logiciel en disaient ; les heures et les cachets ne portent que les paies calculees ici, comme
//! l'etat. Un etat qui n'a rien sur la periode (`Refuse`) est LAISSE DE COTE ; tout autre refus remonte.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use droit::fiche::Lue as _;
use serde::Serialize;

use crate::etats_paie::{self, cumul_de, cumuls_par, Demande, LigneLivre, Periode};
use crate::tableaux_export::{pdf_tableau, qui, totaliser, Section, Tableau, R, TOTAL_GENERAL};
use droit::contexte::Contexte;
use socle::echec::{erreur, Classe, Echec};
use socle::mois::Mois;
use socle::grandeurs::Montant;

/// Un refus qui dit « rien sur la periode » : l'etat est laisse de cote.
fn est_erreur_operation(e: &Echec) -> bool {
    matches!(e.classe, None | Some(Classe::Refuse | Classe::SaisieIncomplete | Classe::Manque | Classe::CaseExigee))
}

fn copie(e: &Echec) -> Echec {
    e.clone()
}

/// CE QUE LES SPECTACLES ET LES SALARIES ONT COUTE dans l'annee `annee`, jusqu'au mois `jusqu_a` : chacun sous sa cle,
/// dans l'ordre ou les paies le rencontrent.
#[derive(Debug, Serialize)]
pub struct CoutsDeLAnnee {
    pub annee: socle::mois::Annee,
    pub jusqu_a: Mois,
    #[serde(serialize_with = "socle::ecriture::en_ordre")]
    pub spectacles: Vec<(String, Cout)>,
    #[serde(serialize_with = "socle::ecriture::en_ordre")]
    pub salaries: Vec<(String, Cout)>,
    /// Les paies que le cout ne compte pas (`etats_paie::livre_des_etats`, `engagement::etats::PaieDeCote`), chacune avec
    /// ce qui lui manque.
    #[serde(skip)]
    pub non_comptees: Vec<engagement::etats::PaieDeCote>,
}

/// Le cout d'un spectacle ou d'un salarie : ses bulletins, ses sommes (en texte), ce qui n'est que prevu, et si une paie
/// importee taisait son cout employeur.
#[derive(Debug, Serialize)]
pub struct Cout {
    pub bulletins: usize,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    pub brut: Montant,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    pub charges: Montant,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    pub cout: Montant,
    #[serde(serialize_with = "socle::grandeurs::ecrit::texte::serialize")]
    pub prevu: Montant,
    pub incomplet: bool,
    /// Les paies de ce salarie, ou de ce spectacle, que le cout ne compte pas (`engagement::etats::PaieDeCote`) : sa ligne
    /// les dit.
    #[serde(skip)]
    pub de_cote: Vec<engagement::etats::PaieDeCote>,
}

/// Les deux etats de la periode, lus une fois chacun pour tous les salaries du dossier.
pub struct Recapitulatifs<'a> {
    ctx: &'a Contexte,
    demande: Demande,
    etats: [Option<Result<Rc<Tableau>, Echec>>; 2],
}

impl<'a> Recapitulatifs<'a> {
    pub fn nouveau(ctx: &'a Contexte, periode: socle::mois::PeriodeDemandee, departs: engagement::departs::Departs) -> Self {
        let (mois, annee, toutes) = periode.en_trois();
        let demande = Demande {
            mois,
            annee,
            toutes,
            regroupement: None,
            format: Some(crate::plan_comptable::FormatDExport::Pdf),
            coordonnees: false,
            ecrire: false,
            logiciel: None,
            analytique: None,
            departs,
        };
        Recapitulatifs { ctx, demande, etats: [None, None] }
    }

    /// Ce qu'un spectacle, ou un salarie, a coute dans l'annee demandee, jusqu'au mois de `ctx.aujourdhui` ; ce qui
    /// est seulement saisi pour plus tard se dit a part (`prevu`). Une paie importee dont les pieces ne disaient pas le
    /// cout employeur rend le cumul `incomplet`. Sans annee, ou sans mois cette annee-la : rien.
    pub fn couts_de_l_annee(&mut self) -> R<CoutsDeLAnnee> {
        let Some(annee) = self.demande.annee else {
            return Err(erreur(Classe::Illisible, "Le coût de l'année se demande pour une année : la dire."));
        };
        let courant = self.ctx.aujourdhui.mois_civil();
        let mut out = CoutsDeLAnnee { annee, jusqu_a: courant, spectacles: Vec::new(), salaries: Vec::new(), non_comptees: Vec::new() };
        let p = Periode::nouvelle(self.ctx, None, Some(annee), false, false)?;
        if p.mois.is_empty() {
            return Ok(out);
        }
        let (livre, non_comptees) = etats_paie::livre_des_etats(self.ctx, &p, &self.demande.departs)?;
        out.non_comptees = non_comptees;
        let lignes: Vec<LigneLivre> = livre.groupes.into_iter().flat_map(|g| g.lignes).collect();
        // Les cles, dans l'ordre ou les paies les rencontrent ; leur cumul jusqu'au mois courant, et ce qui n'est que
        // prevu apres lui (son seul cout). Une somme que les pieces importees taisent ne compte pas ; un cout qui manque
        // rend le cumul `incomplet`.
        let couts = |cle: &dyn Fn(&LigneLivre) -> Option<String>| -> Vec<(String, Cout)> {
            let faits = cumuls_par(lignes.iter().filter(|l| l.mois <= courant), cle);
            let prevus = cumuls_par(lignes.iter().filter(|l| l.mois > courant), cle);
            cumuls_par(&lignes, cle)
                .into_iter()
                .map(|(k, _)| {
                    let (f, prevu) = (cumul_de(&faits, &k), cumul_de(&prevus, &k).cout_employeur.montant);
                    let incomplet = f.cout_employeur.dite < f.bulletins;
                    (k, Cout { bulletins: f.bulletins, brut: f.brut.montant, charges: f.cotisations_patronales.montant, cout: f.cout_employeur.montant, prevu, incomplet, de_cote: Vec::new() })
                })
                .collect()
        };
        out.spectacles = couts(&|l| Some(l.spectacle.clone()).filter(|s| !s.is_empty()));
        out.salaries = couts(&|l| Some(l.cle_salarie.to_string()));
        // Chaque paie mise de cote, sur la ligne de son salarie et sur celle de son spectacle (sous son titre, la cle des
        // lignes du livre) ; celui dont aucune paie ne se compte a la sienne.
        fn poser(lignes: &mut Vec<(String, Cout)>, cle: String, d: &engagement::etats::PaieDeCote) {
            match lignes.iter_mut().find(|(k, _)| *k == cle) {
                Some((_, c)) => c.de_cote.push(d.clone()),
                None => lignes.push((
                    cle,
                    Cout { bulletins: 0, brut: Montant::ZERO, charges: Montant::ZERO, cout: Montant::ZERO, prevu: Montant::ZERO, incomplet: false, de_cote: vec![d.clone()] },
                )),
            }
        }
        for d in &out.non_comptees {
            if let Some(cle) = &d.salarie {
                poser(&mut out.salaries, cle.to_string(), d);
            }
            if let Some(titre) = d.spectacle.as_ref().and_then(|s| self.ctx.spectacles.get(s)).and_then(|f| f.titre.dite()).filter(|t| !t.is_empty()) {
                poser(&mut out.spectacles, titre.clone(), d);
            }
        }
        Ok(out)
    }

    /// L'etat `i` de la periode : 0, les heures et les cachets ; 1, le recapitulatif annuel.
    fn etat(&mut self, i: usize) -> R<Rc<Tableau>> {
        if self.etats[i].is_none() {
            let r = if i == 0 {
                etats_paie::heures_et_cachets(self.ctx, &self.demande)
            } else {
                etats_paie::recapitulatif_annuel(self.ctx, &self.demande)
            };
            self.etats[i] = Some(r.map(Rc::new));
        }
        match self.etats[i].as_ref().expect("lu") {
            Ok(t) => Ok(Rc::clone(t)),
            Err(e) => Err(copie(e)),
        }
    }

    /// Les deux etats reduits au salarie `cle` (les lignes qui portent sa cle : deux salaries de meme nom et de meme
    /// prenom ont chacun la leur), en un PDF dans `tmp`. `None` quand aucun des deux n'a rien a dire de
    /// lui. Les avertissements des etats nomment d'autres salaries : la fiche ne les porte pas.
    pub fn pdf(&mut self, cle: &droit::contexte::IdSalarie, tmp: &Path) -> R<Option<PathBuf>> {
        let fiche = self.ctx.salaries.get(cle).ok_or_else(|| socle::fichiers::manque(cle.as_str()))?;
        let lui = qui(fiche)?;
        let mut morceaux: Vec<Vec<u8>> = Vec::new();
        for i in 0..2 {
            let t = match self.etat(i) {
                Ok(t) => t,
                Err(e) if est_erreur_operation(&e) => continue,
                Err(e) => return Err(e),
            };
            let sections: Vec<Section> = t
                .sections
                .iter()
                .map(|s| Section::nouvelle(s.libelle.clone(), s.lignes.iter().filter(|l| l.texte(crate::tableaux_export::ColonneDEtat::CleSalarie) == cle.as_str()).cloned().collect()))
                .filter(|s| !s.lignes.is_empty())
                .collect();
            if sections.is_empty() {
                continue;
            }
            let mut reduit = Tableau::nouveau(t.etat,format!("{} — {}", t.titre, lui), t.periode.clone());
            reduit.colonnes = t.colonnes.clone();
            reduit.sections = sections;
            reduit.sources = t.sources.clone();
            reduit.mois = t.mois.clone();
            totaliser(&mut reduit, TOTAL_GENERAL);
            morceaux.push(pdf_tableau(&reduit, Some(self.ctx))?);
        }
        if morceaux.is_empty() {
            return Ok(None);
        }
        let chemin = tmp.join(format!("R_{}.pdf", cle));
        let octets = crate::fabrique::fusion_octets(&morceaux, tmp)?;
        std::fs::write(&chemin, octets).map_err(|e| socle::echec::du_disque(&chemin, &e))?;
        Ok(Some(chemin))
    }
}
