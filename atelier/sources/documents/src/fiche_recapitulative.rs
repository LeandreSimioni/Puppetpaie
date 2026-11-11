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

use serde::Serialize;

use crate::etats_paie::{self, Demande, Periode};
use crate::tableaux_export::{pdf_tableau, qui, totaliser, Section, Tableau, R, TOTAL_GENERAL};
use droit::contexte::Contexte;
use socle::echec::{erreur, Classe, Echec};
use socle::mois::Mois;
use socle::num::Decimal;

/// Un refus qui dit « rien sur la periode » : l'etat est laisse de cote.
fn est_erreur_operation(e: &Echec) -> bool {
    matches!(e.classe, None | Some(Classe::Refuse) | Some(Classe::SaisieIncomplete))
}

fn copie(e: &Echec) -> Echec {
    e.clone()
}

/// CE QUE LES SPECTACLES ET LES SALARIES ONT COUTE dans l'annee `annee`, jusqu'au mois `jusqu_a` : chacun sous sa cle,
/// dans l'ordre ou les paies le rencontrent.
#[derive(Debug, Serialize)]
pub struct CoutsDeLAnnee {
    pub annee: String,
    pub jusqu_a: Mois,
    #[serde(serialize_with = "en_objet")]
    pub spectacles: Vec<(String, Cout)>,
    #[serde(serialize_with = "en_objet")]
    pub salaries: Vec<(String, Cout)>,
}

/// Le cout d'un spectacle ou d'un salarie : ses bulletins, ses sommes (en texte), ce qui n'est que prevu, et si une paie
/// importee taisait son cout employeur.
#[derive(Debug, Serialize)]
pub struct Cout {
    pub bulletins: i64,
    pub brut: Decimal,
    pub charges: Decimal,
    pub cout: Decimal,
    pub prevu: Decimal,
    pub incomplet: bool,
}

/// Des paires (cle, cout) ecrites en objet, dans leur ordre.
fn en_objet<S: serde::Serializer>(v: &[(String, Cout)], s: S) -> Result<S::Ok, S::Error> {
    s.collect_map(v.iter().map(|(k, c)| (k, c)))
}

/// Les deux etats de la periode, lus une fois chacun pour tous les salaries du dossier.
pub struct Recapitulatifs<'a> {
    ctx: &'a Contexte,
    demande: Demande,
    etats: [Option<Result<Rc<Tableau>, Echec>>; 2],
}

impl<'a> Recapitulatifs<'a> {
    pub fn nouveau(ctx: &'a Contexte, mois: Option<Mois>, annee: Option<String>, toutes: bool, departs: engagement::departs::Departs) -> Self {
        let demande = Demande { mois, annee, toutes, format: "pdf".to_string(), departs, ..Demande::default() };
        Recapitulatifs { ctx, demande, etats: [None, None] }
    }

    /// Ce qu'un spectacle, ou un salarie, a coute dans l'annee demandee, jusqu'au mois de `ctx.aujourdhui` ; ce qui
    /// est seulement saisi pour plus tard se dit a part (`prevu`). Une paie importee dont les pieces ne disaient pas le
    /// cout employeur rend le cumul `incomplet`. Sans annee, ou sans mois cette annee-la : rien.
    pub fn couts_de_l_annee(&mut self) -> R<CoutsDeLAnnee> {
        let Some(annee) = self.demande.annee.clone() else {
            return Err(erreur(Classe::Illisible, "Le coût de l'année se demande pour une année : la dire."));
        };
        let courant = self.ctx.aujourdhui.mois_civil();
        let mut out = CoutsDeLAnnee { annee: annee.clone(), jusqu_a: courant, spectacles: Vec::new(), salaries: Vec::new() };
        if annee.is_empty() {
            return Ok(out);
        }
        let p = Periode::nouvelle(self.ctx, None, Some(&annee), false, false)?;
        if p.mois.is_empty() {
            return Ok(out);
        }
        struct Cumul {
            bulletins: i64,
            brut: Decimal,
            charges: Decimal,
            cout: Decimal,
            prevu: Decimal,
            incomplet: bool,
        }
        let zero = socle::num::zero2();
        let mut cotes: [Vec<(String, Cumul)>; 2] = [Vec::new(), Vec::new()];
        for l in etats_paie::lignes_livre(self.ctx, &p, &mut Vec::new(), &self.demande.departs)? {
            for (i, cle) in [&l.spectacle, &l.cle_salarie].into_iter().enumerate() {
                if cle.is_empty() {
                    continue;
                }
                let pos = match cotes[i].iter().position(|(k, _)| k == cle) {
                    Some(n) => n,
                    None => {
                        cotes[i].push((
                            cle.clone(),
                            Cumul { bulletins: 0, brut: zero, charges: zero, cout: zero, prevu: zero, incomplet: false },
                        ));
                        cotes[i].len() - 1
                    }
                };
                // Une somme que la piece importee ne disait pas ne s'ajoute pas ; un cout qui manque rend le cumul
                // `incomplet`.
                let d = &mut cotes[i][pos].1;
                if l.mois > courant {
                    d.prevu += l.cout_employeur.into_iter().sum::<Decimal>();
                    continue;
                }
                d.bulletins += 1;
                d.brut += l.brut.into_iter().sum::<Decimal>();
                d.charges += l.cotisations_patronales.into_iter().sum::<Decimal>();
                if l.cout_employeur.is_none() {
                    d.incomplet = true;
                }
                d.cout += l.cout_employeur.into_iter().sum::<Decimal>();
            }
        }
        let [spectacles, salaries] = cotes.map(|v| {
            v.into_iter()
                .map(|(cle, d)| {
                    (cle, Cout { bulletins: d.bulletins, brut: d.brut, charges: d.charges, cout: d.cout, prevu: d.prevu, incomplet: d.incomplet })
                })
                .collect()
        });
        out.spectacles = spectacles;
        out.salaries = salaries;
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
    pub fn pdf(&mut self, cle: &str, tmp: &Path) -> R<Option<PathBuf>> {
        let fiche = self.ctx.salaries.get(cle).ok_or_else(|| socle::fichiers::absent(cle).reclasser(Classe::Manque))?;
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
                .map(|s| Section::nouvelle(s.libelle.clone(), s.lignes.iter().filter(|l| l.texte("cle_salarie") == cle).cloned().collect()))
                .filter(|s| !s.lignes.is_empty())
                .collect();
            if sections.is_empty() {
                continue;
            }
            let mut reduit = Tableau::nouveau(&t.etat, format!("{} — {}", t.titre, lui), t.periode.clone());
            reduit.colonnes = t.colonnes.clone();
            reduit.sections = sections;
            reduit.sources = t.sources.clone();
            reduit.mois = t.mois.clone();
            totaliser(&mut reduit, TOTAL_GENERAL)?;
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
