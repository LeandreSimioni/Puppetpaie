//! Le bloc S21.G00.31 « Changements individu » (corrections DSN).
//!
//! Le NIR, le nom de famille, les prenoms et la date de naissance sont des donnees IDENTIFIANTES : elles se reportent
//! a l'identique d'une DSN mensuelle a la suivante, et tout changement se signale par le bloc changement « des la
//! premiere DSN portant le changement » (cahier technique 2026.1, § 2.1.1, p. 18 ; § 2.1.2.3.1, p. 20 :
//! « obligatoirement des la premiere DSN mensuelle dans laquelle est utilisee la nouvelle identite »). Le bloc 30 porte
//! la nouvelle valeur, le bloc 31 l'ANCIENNE (« Les rubriques doivent etre renseignees avec la valeur avant
//! changement », p. 177), datee en 31.001 ; plusieurs changements a la meme date font un seul bloc, des dates
//! differentes autant de blocs (p. 177) ; un bloc qui ne porte que sa date est interdit (CCH-12, p. 177). Textes :
//! `atelier/reference/norme-dsn/texte/bloc-S21.G00.31.txt`, `bloc-S21.G00.34.txt` (la fin de 31.011, p. 179),
//! `chapitre-2-principes-de-constitution.txt` (p. 18-21), `chapitre-4-controles.txt` (§ 4.4.12, p. 99 : 31.001 ne
//! depasse pas le dernier jour du mois principal declare).
//!
//! CE QUI A ETE DECLARE SE RELIT, IL NE SE RECALCULE PAS (regles 1 et 4) : l'identite de reference est celle de la fiche
//! que lit la paie declaree la plus recente du salarie, d'un mois anterieur au mois declare (`dsn::paies_declarees_du_mois`,
//! la version de l'engagement sur laquelle sa DSN est partie). La date du changement est celle que la fiche datee porte :
//! le `depuis` de la valeur qui suit la derniere qui porte l'ancienne valeur declaree (`droit::fiche::dater`). Une fiche qui
//! ne date pas le changement refuse : la date ne se devine pas.

use std::collections::BTreeMap;

use droit::contexte::Contexte;
use droit::fiche::{ChampsSalarie, Lue, Datee, FicheSalarie, FicheSalarieDatee, Matricule, Nir};
use socle::date::Jour;
use socle::mois::Mois;
use crate::dsn::Fichier;
use socle::echec::R;
use crate::usages::strip;

/// UN IDENTIFIANT DE L'INDIVIDU QUE LE BLOC 31 REPORTE : le NIR, le nom de famille, les prenoms, la date de naissance
/// (S21.G00.31.008 a .011, p. 177), chacun avec sa rubrique 31, son libelle, et la case de la fiche qui le porte (le
/// bloc 30 l'ecrit en 30.001, .002, .004, .006).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseDIdentite {
    Nir,
    Nom,
    Prenoms,
    Naissance,
}

/// La valeur d'un identifiant telle que le bloc 30 l'ecrit : le NIR, un texte sans son blanc autour (le nom, les
/// prenoms), un jour (la date de naissance, ecrite JJMMAAAA comme 30.006).
#[derive(Debug, Clone, PartialEq, Eq)]
enum ValeurDIdentite {
    Nir(Nir),
    Texte(String),
    Jour(Jour),
}

impl std::fmt::Display for ValeurDIdentite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValeurDIdentite::Nir(n) => f.write_str(n),
            ValeurDIdentite::Texte(t) => f.write_str(t),
            ValeurDIdentite::Jour(j) => f.write_str(&j.jjmmaaaa()),
        }
    }
}

/// Un texte de la fiche tel que le bloc 30 l'ecrit.
fn texte_ecrit<T: AsRef<str>>(c: &Option<T>) -> Option<String> {
    c.portee().map(|t| strip(t).to_string())
}

impl CaseDIdentite {
    /// Les identifiants, dans l'ordre du bloc 31 (p. 177).
    pub const TOUTES: [CaseDIdentite; 4] = [CaseDIdentite::Nir, CaseDIdentite::Nom, CaseDIdentite::Prenoms, CaseDIdentite::Naissance];

    /// Sa rubrique du bloc 31.
    pub fn r31(self) -> droit::natures::RubriqueDeLaNorme {
        match self {
            CaseDIdentite::Nir => droit::rubrique!("S21.G00.31.008"),
            CaseDIdentite::Nom => droit::rubrique!("S21.G00.31.009"),
            CaseDIdentite::Prenoms => droit::rubrique!("S21.G00.31.010"),
            CaseDIdentite::Naissance => droit::rubrique!("S21.G00.31.011"),
        }
    }

    /// Ce qu'il est, pour une phrase.
    pub fn libelle(self) -> &'static str {
        match self {
            CaseDIdentite::Nir => "NIR",
            CaseDIdentite::Nom => "nom de famille",
            CaseDIdentite::Prenoms => "prenoms",
            CaseDIdentite::Naissance => "date de naissance",
        }
    }

    /// Sa valeur dans la fiche, telle que le bloc 30 l'ecrit.
    fn ecrite(self, s: &ChampsSalarie) -> Option<ValeurDIdentite> {
        match self {
            CaseDIdentite::Nir => s.nir.dite().cloned().map(ValeurDIdentite::Nir),
            CaseDIdentite::Nom => texte_ecrit(&s.nom).map(ValeurDIdentite::Texte),
            CaseDIdentite::Prenoms => texte_ecrit(&s.prenom).map(ValeurDIdentite::Texte),
            CaseDIdentite::Naissance => s.date_naissance.dite().copied().map(ValeurDIdentite::Jour),
        }
    }

    /// Pose dans `s` sa case telle que `au` la porte.
    fn poser(self, s: &mut ChampsSalarie, au: &ChampsSalarie) {
        match self {
            CaseDIdentite::Nir => s.nir = au.nir.clone(),
            CaseDIdentite::Nom => s.nom = au.nom.clone(),
            CaseDIdentite::Prenoms => s.prenom = au.prenom.clone(),
            CaseDIdentite::Naissance => s.date_naissance = au.date_naissance.clone(),
        }
    }

    /// Ses valeurs dans la fiche datee, de la plus ancienne a la plus recente, telles que le bloc 30 les ecrit.
    fn datees(self, b: &FicheSalarieDatee) -> Vec<(Option<Jour>, Option<ValeurDIdentite>)> {
        match self {
            CaseDIdentite::Nir => datees(&b.nir, |c| c.dite().cloned().map(ValeurDIdentite::Nir)),
            CaseDIdentite::Nom => datees(&b.nom, |c| texte_ecrit(c).map(ValeurDIdentite::Texte)),
            CaseDIdentite::Prenoms => datees(&b.prenom, |c| texte_ecrit(c).map(ValeurDIdentite::Texte)),
            CaseDIdentite::Naissance => datees(&b.date_naissance, |c| c.dite().copied().map(ValeurDIdentite::Jour)),
        }
    }
}

/// L'identite d'un individu telle qu'une DSN l'a declaree : chaque case vide quand la DSN ne l'a pas portee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaree {
    /// Le mois principal declare et le nom du fichier qui l'a portee.
    pub mois: Mois,
    pub fichier: String,
    pub nir: Option<Nir>,
    pub nom: Option<String>,
    pub prenoms: Option<String>,
    pub naissance: Option<Jour>,
    /// Le numero technique temporaire (30.020), s'il y en avait un.
    pub ntt: Option<crate::ntt::Ntt>,
    pub matricule: Option<Matricule>,
}

/// Le matricule de la fiche tel que le bloc 30 l'ecrit, sans son blanc autour.
fn matricule_ecrit(c: &Option<Matricule>) -> R<Option<Matricule>> {
    c.portee().map(|t| Matricule::lire(strip(t))).transpose()
}

impl Declaree {
    /// La valeur d'un identifiant telle que le bloc 30 l'ecrit (la date de naissance en JJMMAAAA, comme 30.006).
    fn valeur(&self, case: CaseDIdentite) -> Option<ValeurDIdentite> {
        match case {
            CaseDIdentite::Nir => self.nir.clone().map(ValeurDIdentite::Nir),
            CaseDIdentite::Nom => self.nom.clone().map(ValeurDIdentite::Texte),
            CaseDIdentite::Prenoms => self.prenoms.clone().map(ValeurDIdentite::Texte),
            CaseDIdentite::Naissance => self.naissance.map(ValeurDIdentite::Jour),
        }
    }

    /// L'identite seule (sans le mois ni le fichier) : deux etablissements d'un meme mois doivent dire la meme.
    fn identite(&self) -> (&Option<Nir>, &Option<String>, &Option<String>, Option<Jour>, &Option<crate::ntt::Ntt>) {
        (&self.nir, &self.nom, &self.prenoms, self.naissance, &self.ntt)
    }
}

/// Les valeurs d'une liste datee, de la plus ancienne a la plus recente, telles que le bloc 30 les ecrit (`ecrite` ;
/// `None` : une valeur qui ne ressemble a aucune valeur declaree).
fn datees<T>(liste: &Option<Vec<Datee<T>>>, ecrite: fn(&Option<T>) -> Option<ValeurDIdentite>) -> Vec<(Option<Jour>, Option<ValeurDIdentite>)> {
    liste.iter().flat_map(|l| droit::fiche::valeurs_datees(l)).map(|e| (e.depuis, ecrite(&e.valeur))).collect()
}

/// La fiche telle que le bloc 30 doit la lire pour le mois declare : ses identifiants tels qu'ils valaient le dernier
/// jour du mois (`fin`), lus dans la fiche datee (`brute`), le reste tel quel. Un changement d'identite date APRES le
/// mois declare n'entre pas dans sa DSN, sans quoi 31.001 depasserait le dernier jour du mois principal declare
/// (§ 4.4.12, p. 99) -- il ira dans la DSN du mois ou il prend effet.
pub fn identite_du_mois(salarie: &FicheSalarie, brute: Option<&FicheSalarieDatee>, fin: &Jour) -> R<FicheSalarie> {
    let Some(brute) = brute else {
        return Err(socle::refus!(
            "{} : sa fiche n'est pas dans les fiches salariés de la compagnie, ses identifiants du mois ne se lisent pas. Rien n'est écrit.",
            qui(&salarie.champs)
        ));
    };
    let au: ChampsSalarie = brute.au(fin);
    let mut sortie = salarie.clone();
    for case in CaseDIdentite::TOUTES {
        case.poser(&mut sortie.champs, &au);
    }
    Ok(sortie)
}

/// Les identites declarees des mois anterieurs au mois declare, lues a la demande, du plus recent au plus ancien.
pub struct Declarations<'a> {
    ctx: &'a Contexte,
    departs: &'a engagement::departs::Departs,
    mois: Vec<Mois>,
    lus: BTreeMap<usize, Vec<Declaree>>,
}

impl<'a> Declarations<'a> {
    /// Les mois saisis avant `mois`.
    pub fn avant(ctx: &'a Contexte, mois: Mois, departs: &'a engagement::departs::Departs) -> R<Declarations<'a>> {
        let mut avant: Vec<Mois> = engagement::stock::mois_saisis(&ctx.racine)?.into_iter().filter(|m| *m < mois).collect();
        avant.sort();
        avant.reverse();
        Ok(Declarations { ctx, departs, mois: avant, lus: BTreeMap::new() })
    }

    /// Les individus que les paies du `i`-ieme mois qu'une DSN partie a declarees portent (`engagement::etats::paie_declaree`),
    /// avec la fiche que leur calcul a lue : l'identite que leur DSN a ecrite. Une paie qu'aucune DSN n'a encore declaree
    /// n'a rien dit de lui.
    fn lire(&mut self, i: usize) -> R<&Vec<Declaree>> {
        if !self.lus.contains_key(&i) {
            let mois = self.mois[i];
            let ctx_m = self.ctx.du_mois(mois)?;
            let mut paies = Vec::new();
            for e in engagement::etats::du_mois(self.ctx, mois, self.departs)? {
                if let Some(p) = e.valide().map(|v| engagement::etats::paie_declaree(&ctx_m, v.objet(), self.departs, mois)).transpose()?.flatten() {
                    paies.push(p);
                }
            }
            let mut tous = Vec::new();
            for p in paies {
                let s = &p.salarie;
                let nir = s.nir.dite().cloned();
                tous.push(Declaree {
                    mois,
                    fichier: "la paie declaree".to_string(),
                    ntt: match nir {
                        None => Some(crate::ntt::numero(self.ctx.entreprise.siren.ecrite("siren")?, s)?),
                        Some(_) => None,
                    },
                    nir,
                    nom: texte_ecrit(&s.nom),
                    prenoms: texte_ecrit(&s.prenom),
                    naissance: s.date_naissance.dite().copied(),
                    matricule: matricule_ecrit(&s.code_salarie)?,
                });
            }
            self.lus.insert(i, tous);
        }
        Ok(&self.lus[&i])
    }

    /// L'identite que la derniere DSN qui le porte a declaree pour ce salarie, ou `None` s'il n'a jamais ete declare.
    /// Il se reconnait a un NIR que sa fiche (`brute`, datee) a porte, sinon a son matricule
    /// (30.019 ; il finit le NTT et ne change pas sans NIR, `routes::fiches::ecrire_fiche`).
    pub fn derniere(&mut self, salarie: &ChampsSalarie, brute: Option<&FicheSalarieDatee>) -> R<Option<Declaree>> {
        let mut nirs: Vec<Nir> = salarie.nir.dite().cloned().into_iter().collect();
        for d in brute.into_iter().flat_map(|b| b.nir.iter().flat_map(|l| droit::fiche::valeurs_datees(l))) {
            nirs.extend(d.valeur.clone());
        }
        let matricule = matricule_ecrit(&salarie.code_salarie)?;
        for i in 0..self.mois.len() {
            let tous = self.lire(i)?;
            let mut siens: Vec<&Declaree> = tous.iter().filter(|d| d.nir.as_ref().is_some_and(|n| nirs.contains(n))).collect();
            if siens.is_empty() && matricule.is_some() {
                siens = tous.iter().filter(|d| d.matricule == matricule).collect();
            }
            let Some(premier) = siens.first() else { continue };
            if let Some(autre) = siens.iter().find(|d| d.identite() != premier.identite()) {
                return Err(socle::refus!(
                    "{} : la DSN de {} le declare deux fois, sous deux identites ({} et {}) : laquelle a ete declaree ne se \
devine pas. Rien n'est ecrit.",
                    qui(salarie),
                    premier.mois,
                    decrire(premier),
                    decrire(autre)
                ));
            }
            return Ok(Some((*premier).clone()));
        }
        Ok(None)
    }
}

/// L'identite que la derniere DSN mensuelle a declaree pour `salarie` (sa fiche lue), des mois saisis jusqu'a celui de
/// `jour` compris : ce que lit une DSN hors du mois (un signalement, une demande de taux) pour poser son NTT
/// (`ntt::poser_ntt`).
pub fn derniere_declaree(ctx: &Contexte, salarie: &droit::fiche::SalarieDuContrat, jour: &Jour, departs: &engagement::departs::Departs) -> R<Option<Declaree>> {
    Declarations::avant(ctx, jour.mois_civil().suivant(), departs)?.derniere(salarie, ctx.salarie_brut(&salarie.id))
}

fn qui(salarie: &ChampsSalarie) -> String {
    let nom = match salarie.nomme() {
        Some(n) => n,
        None => "Un salarié sans nom".to_string(),
    };
    match salarie.code_salarie.portee() {
        Some(matricule) => format!("{} (matricule {})", nom, strip(matricule)),
        None => format!("{} (sans matricule)", nom),
    }
}

fn decrire(d: &Declaree) -> String {
    // Ce que la declaration ne porte pas se dit tel.
    let dite = |c: &Option<String>| match c {
        Some(v) => v.clone(),
        None => "(non declare)".to_string(),
    };
    format!(
        "{} {}, ne(e) le {}, {}",
        dite(&d.prenoms),
        dite(&d.nom),
        dite(&d.naissance.as_ref().map(Jour::jjmmaaaa)),
        match &d.nir {
            Some(nir) => format!("NIR {}", nir),
            None => format!("NTT {}", dite(&d.ntt.as_ref().map(|n| n.to_string()))),
        }
    )
}

/// Le jour d'effet que la fiche datee `brute` donne au changement de `case` depuis la valeur `declaree` : le `depuis`
/// de la valeur qui suit la derniere qui la porte, au plus tard la fin du mois declare (`fin`).
fn date_d_effet(brute: Option<&FicheSalarieDatee>, case: CaseDIdentite, declaree: &ValeurDIdentite, fin: &Jour) -> Option<Jour> {
    let brute = brute?;
    let valeurs = case.datees(brute);
    let mut meilleur: Option<Jour> = None;
    for paire in valeurs.windows(2) {
        let ((_, v), (suivant, _)) = (&paire[0], &paire[1]);
        if let Some(suivant) = suivant.filter(|s| s <= fin) {
            if v.as_ref() == Some(declaree) {
                meilleur = Some(suivant);
            }
        }
    }
    meilleur
}

/// Ecrit, apres le bloc 30 de `salarie` (sa fiche telle que le bloc 30 l'a lue, `identite_du_mois`), les blocs 31 de
/// ce qui differe de l'identite `declaree` ; rien s'il n'a jamais ete declare. `mois` et `fin` : le mois declare et
/// son dernier jour.
///
/// Le NIR qui ARRIVE (declare sans NIR, sous un NTT) n'a pas d'« ancien NIR » : la norme fait le lien par le NTT porte
/// a cote du NIR (30.020, p. 175 ; `ntt.rs`), pas par le bloc 31.
pub fn ecrire(f: &mut Fichier, salarie: &ChampsSalarie, brute: Option<&FicheSalarieDatee>, declaree: Option<&Declaree>, mois: Mois, fin: &Jour) -> R<()> {
    let Some(d) = declaree else { return Ok(()) };
    let mut par_date: BTreeMap<Jour, Vec<(droit::natures::RubriqueDeLaNorme, ValeurDIdentite)>> = BTreeMap::new();
    let mut sans_date: Vec<String> = Vec::new();
    for case in CaseDIdentite::TOUTES {
        let maintenant = case.ecrite(salarie);
        if d.valeur(case) == maintenant {
            continue;
        }
        // Une case que la DSN n'a pas portee n'a pas d'ancienne valeur : le NIR qui arrive se lie par le NTT.
        let Some(avant) = d.valeur(case) else { continue };
        if case == CaseDIdentite::Nir && maintenant.is_none() {
            return Err(socle::refus!(
                "{} a ete declare(e) sous le NIR {} (DSN de {}, {}), et sa fiche n'a plus de NIR : un NIR declare ne se \
retire pas, il se reporte a l'identique d'une DSN a la suivante (cahier technique, § 2.1.1, p. 18). Remettre le NIR \
dans la fiche, ou le remplacer par le NIR corrige, date. Rien n'est ecrit.",
                qui(salarie),
                avant,
                d.mois,
                d.fichier
            ));
        }
        match date_d_effet(brute, case, &avant, fin) {
            Some(j) => par_date.entry(j).or_default().push((case.r31(), avant.clone())),
            None => sans_date.push(format!(
                "  - {} : declare(e) \u{ab} {} \u{bb} dans la DSN de {} ({}), {} dans la fiche",
                case.libelle(),
                avant,
                d.mois,
                d.fichier,
                match &maintenant {
                    Some(m) => format!("\u{ab} {} \u{bb}", m),
                    None => "rien".to_string(),
                }
            )),
        }
    }
    if !sans_date.is_empty() {
        return Err(socle::refus!(
            "La DSN de {} ne se fait pas : l'identite de {} a change depuis sa derniere declaration, et la fiche ne dit pas \
depuis quand.\n{}\nLa DSN declare l'ancienne valeur et la date du changement (bloc \u{ab} Changements individu \u{bb} \
S21.G00.31, cahier technique p. 177 ; \u{ab} obligatoirement des la premiere DSN mensuelle dans laquelle est utilisee \
la nouvelle identite \u{bb}, p. 20) : dater ce changement dans la fiche du salarie (y remettre l'ancienne valeur, \
enregistrer, puis saisir la nouvelle : l'ecran demande alors la date d'effet). Rien n'est ecrit.",
            mois,
            qui(salarie),
            sans_date.join("\n")
        ));
    }
    for (jour, rubriques) in par_date {
        f.add(droit::rubrique!("S21.G00.31.001"), jour.jjmmaaaa());
        for (r, v) in rubriques {
            f.add(r, v);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declaree() -> Declaree {
        Declaree {
            mois: Mois::nouveau(2026, 7).unwrap(),
            fichier: "la paie declaree".into(),
            nir: Some(Nir::lire("2901075999009").unwrap()),
            nom: Some("CLAIRON".into()),
            prenoms: Some("LUCILE".into()),
            naissance: Some(Jour::nouveau(1990, 10, 2).unwrap()),
            ntt: None,
            matricule: Some(Matricule::lire("6").unwrap()),
        }
    }

    fn fin() -> Jour {
        Jour::nouveau(2026, 9, 30).unwrap()
    }

    fn lue(v: &str) -> ChampsSalarie {
        socle::forme::lire_texte(v, "la fiche").unwrap()
    }

    /// Une fiche datee : chaque valeur de tout temps, puis `apres` (champ, depuis, valeur).
    fn fiche(valeurs: &str, apres: &[(&str, Jour, &str)]) -> FicheSalarieDatee {
        use droit::fiche::FicheDatee;
        let mut f = FicheSalarieDatee::neuve(&lue(valeurs));
        for (champ, depuis, v) in apres {
            let changement = format!(r#"{{"case": {}, "valeur": {}}}"#, socle::ecriture::json_en_ligne(champ), socle::ecriture::json_en_ligne(v));
            let ch = socle::forme::lire_texte(&changement, "le changement").unwrap();
            f.changer(ch, Some(depuis), false, None);
        }
        f
    }

    #[test]
    fn rien_ne_change_rien_ne_s_ecrit() {
        let s = r#"{"nir": "2901075999009", "nom": "CLAIRON", "prenom": "LUCILE", "date_naissance": "1990-10-02", "code_salarie": "6"}"#;
        let b = fiche(s, &[]);
        let mut f = Fichier::nouveau();
        ecrire(&mut f, &lue(s), Some(&b), Some(&declaree()), fin().mois_civil(), &fin()).unwrap();
        assert!(f.lignes.is_empty());
        ecrire(&mut f, &lue(s), Some(&b), None, fin().mois_civil(), &fin()).unwrap();
        assert!(f.lignes.is_empty());
    }

    #[test]
    fn un_changement_date_porte_l_ancienne_valeur_et_un_non_date_refuse() {
        let s = lue(r#"{"nir": "2901075999009", "nom": "DUVAL", "prenom": "LUCILE", "date_naissance": "1990-10-02", "code_salarie": "6"}"#);
        let b = fiche(r#"{"nom": "CLAIRON"}"#, &[("nom", Jour::nouveau(2026, 9, 15).unwrap(), "DUVAL")]);
        let mut f = Fichier::nouveau();
        ecrire(&mut f, &s, Some(&b), Some(&declaree()), fin().mois_civil(), &fin()).unwrap();
        assert_eq!(f.lignes, vec!["S21.G00.31.001,'15092026'", "S21.G00.31.009,'CLAIRON'"]);
        // Une fiche qui n'a jamais porte la valeur declaree ne date rien.
        let b = fiche(r#"{"nom": "AUTRE"}"#, &[("nom", Jour::nouveau(2026, 9, 15).unwrap(), "DUVAL")]);
        let e = ecrire(&mut Fichier::nouveau(), &s, Some(&b), Some(&declaree()), fin().mois_civil(), &fin()).unwrap_err();
        assert!(e.phrase.contains("nom de famille") && e.phrase.contains("CLAIRON") && e.phrase.contains("DUVAL"), "{}", e.phrase);
    }

    #[test]
    fn un_nir_declare_ne_disparait_pas() {
        let s = r#"{"nom": "CLAIRON", "prenom": "LUCILE", "date_naissance": "1990-10-02", "code_salarie": "6"}"#;
        let e = ecrire(&mut Fichier::nouveau(), &lue(s), Some(&fiche(s, &[])), Some(&declaree()), fin().mois_civil(), &fin()).unwrap_err();
        assert!(e.phrase.contains("n'a plus de NIR"), "{}", e.phrase);
    }

    #[test]
    fn l_identite_du_mois_ignore_un_changement_posterieur() {
        let b = fiche(r#"{"nom": "CLAIRON", "adresse": "ANCIENNE"}"#, &[("nom", Jour::nouveau(2026, 10, 2).unwrap(), "DUVAL"), ("adresse", Jour::nouveau(2026, 10, 2).unwrap(), "NOUVELLE")]);
        let s = FicheSalarie { id: droit::contexte::IdSalarie::lire("DUVAL").unwrap(), champs: lue(r#"{"nom": "DUVAL", "adresse": "NOUVELLE"}"#) };
        let m = identite_du_mois(&s, Some(&b), &fin()).unwrap();
        assert_eq!(m.nom.portee(), Some("CLAIRON"));
        // Ce qui n'identifie pas se lit tel quel (la derniere valeur connue, § 2.1.2.4.2 par analogie).
        assert_eq!(m.adresse.portee(), Some("NOUVELLE"));
    }
}
