//! UN CONTRAT NE S'ATTESTE QU'UNE FOIS. Une AEM deposee, une AEM en PDF, un bulletin, un certificat Congés Spectacles
//! disent chacun d'un contrat : ce salarie, ces dates, ces heures, cachets et jours, ce brut. Le registre tient ce qui
//! est deja atteste (l'historique, puis ce que l'import en cours retient) ; chaque piece lui demande si son contrat
//! l'est (`pareils`), et s'il ne l'est pas, le lui ajoute (`ajouter`). Ce qu'il retient, et ce qu'il change a
//! l'historique, est ce que l'ecriture prend (`import::ecrite`).
//!
//! LA REGLE : un contrat du meme salarie (par son NIR) dont les dates chevauchent celles de la piece l'atteste deja ;
//! la piece le confirme, n'ajoute rien, et ses ecarts se notent. Deux exceptions, de la piece la moins complete a la
//! plus complete :
//! - le contrat qu'un certificat atteste ne porte ni les heures, ni les cachets et les jours a part, et son brut est
//!   arrondi a l'euro (`import::certificat_lu`) : un bulletin ou une AEM le remplace, MOIS PAR MOIS (`reste_du_
//!   certificat`) ; les mois du certificat qu'aucun d'eux n'atteste restent attestes par lui ;
//! - le bulletin est la piece de paie : quand il dit d'autres compteurs que le contrat d'une AEM aux memes dates, son
//!   contrat remplace celui de l'AEM, et l'ecart se dit avec les deux valeurs.
//!
//! Un compteur qu'une piece ne dit pas reste vide : il n'est pas zero, et deux contrats ne comptent pareil que s'ils
//! disent la meme chose.

use std::collections::{BTreeSet, HashMap};

use droit::contexte::Contexte;
use engagement::importe::{Attestation, ContratLu};
use socle::date::Jour;
use socle::echec::R;
use socle::num::Decimal;

/// Un contrat atteste : de qui (NIR), quand, par quoi, et ce qu'il compte.
#[derive(Clone, Debug)]
pub struct Atteste {
    pub(crate) nir: String,
    pub(crate) debut: Option<Jour>,
    /// Sa fin ; aucune pour un contrat qui court toujours.
    pub(crate) fin: Option<Jour>,
    /// « AEM », « bulletin », « certificat » (la piece de cet import), ou « historique ».
    pub(crate) source: &'static str,
    /// Ce qui l'atteste.
    pub par: Attestation,
    pub(crate) heures: Option<i64>,
    pub cachets: Option<i64>,
    pub jours: Option<i64>,
    pub brut: Option<Decimal>,
    /// Sa ligne, telle qu'elle s'ecrit.
    ligne: ContratLu,
    /// Pour un contrat de l'historique : sa ligne telle que l'historique la porte (l'ecriture la retrouve par elle).
    historique: Option<ContratLu>,
}

impl Atteste {
    /// Son nom dans le compte rendu.
    pub fn d_ou(&self) -> String {
        match (self.source, self.par) {
            ("historique", Attestation::Certificat) => "historique, d'un certificat".into(),
            ("historique", Attestation::Aem) => "historique, d'une AEM".into(),
            (s, _) => s.into(),
        }
    }

    pub fn de_certificat(&self) -> bool {
        self.par == Attestation::Certificat
    }

    /// Ce qu'il compte : ses heures, ses cachets, ses jours, son brut.
    pub fn compte(&self) -> (Option<i64>, Option<i64>, Option<i64>, Option<Decimal>) {
        (self.heures, self.cachets, self.jours, self.brut)
    }

    /// Ce qu'il compte, en mots, pour dire un ecart avec ses deux valeurs.
    pub fn en_mots(&self) -> String {
        en_mots(self.heures, self.cachets, self.jours, self.brut)
    }

    /// Commence-t-il au plus tot le `debut` et finit-il au plus tard la `fin` d'une piece ?
    pub fn tient_dans(&self, debut: Jour, fin: Jour) -> bool {
        self.debut.is_some_and(|d| d >= debut) && self.fin.is_some_and(|f| f <= fin)
    }
}

/// Un compteur en mots : ce qu'il dit, ou qu'il ne dit rien.
fn compteur(x: Option<i64>, mot: &str) -> String {
    match x {
        Some(n) => format!("{} {}(s)", n, mot),
        None => format!("{}s non dit(e)s", mot),
    }
}

/// Des compteurs et un brut, en mots.
pub fn en_mots(heures: Option<i64>, cachets: Option<i64>, jours: Option<i64>, brut: Option<Decimal>) -> String {
    let brut = match brut {
        Some(b) => format!("{} de brut", socle::texte::euros(b)),
        None => "brut illisible".to_string(),
    };
    format!("{}, {}, {}, {}", compteur(heures, "heure"), compteur(cachets, "cachet"), compteur(jours, "jour"), brut)
}

/// Ce qu'un contrat atteste dit, pour le registre.
fn lu(nir: &str, ligne: &ContratLu, source: &'static str) -> Atteste {
    let c = &ligne.contrat;
    Atteste {
        nir: nir.to_string(),
        debut: ligne.debut,
        fin: ligne.fin,
        source,
        par: c.atteste_par,
        heures: c.heures,
        cachets: c.cachets,
        jours: c.jours,
        brut: c.brut,
        ligne: ligne.clone(),
        historique: (source == "historique").then(|| ligne.clone()),
    }
}

/// Les dates `[d1, f1]` et `[d, f]` se chevauchent ; une fin absente court toujours ; un contrat sans debut ne
/// chevauche rien.
pub fn chevauche(d1: Option<Jour>, f1: Option<Jour>, d: Jour, f: Jour) -> bool {
    d1.is_some_and(|d1| d1 <= f) && f1.is_none_or(|f1| d <= f1)
}

/// Les mois de `debut` a `fin` ; `None` : une date manque.
fn mois_de(debut: Option<Jour>, fin: Option<Jour>) -> Option<Vec<socle::mois::Mois>> {
    Some(socle::mois::Mois::entre(&debut?, &fin?))
}

/// Ce qu'un certificat atteste seul (`reste_du_certificat`) : rien, son reste, ou la phrase qui dit pourquoi le reste ne
/// se compte pas.
pub enum Reste {
    Rien,
    Seul(ContratLu),
    NeSeComptePas(String),
}

/// UN CERTIFICAT, MOINS LES MOIS QUE D'AUTRES PIECES ATTESTENT. Le certificat dit un contrat entier (ses dates, son
/// nombre de jours ou de cachets, son brut) ; un bulletin ou une AEM dit un mois. Les mois que `par` attestent lui
/// sont retires ; ce qui reste est ce que le certificat atteste seul : les dates des mois restants, et ses compteurs
/// et son brut moins ceux des pieces qui le coupent (une soustraction de valeurs lues, pas un partage). `Ok(None)` :
/// il ne reste rien. Ce qui ne se soustrait pas se refuse (regle 1) : des mois restants de part et d'autre d'un mois
/// coupe (les compteurs ne se partagent pas entre eux), des pieces qui comptent dans une autre unite que lui ou ne
/// disent pas leur compte, un reste nul ou negatif. La base conge du certificat ne dit que le contrat entier : le reste
/// ne la porte pas.
fn reste_du_certificat(c: &Atteste, par: &[&Atteste]) -> Reste {
    let sans_dates = || Reste::NeSeComptePas("un contrat ne dit pas ses dates : ses mois ne se comptent pas".to_string());
    let (Some(debut_c), Some(fin_c)) = (c.debut, c.fin) else { return sans_dates() };
    let siens = socle::mois::Mois::entre(&debut_c, &fin_c);
    let coupants: Vec<&&Atteste> = par.iter().filter(|p| chevauche(p.debut, p.fin, debut_c, fin_c)).collect();
    let mut coupes: BTreeSet<socle::mois::Mois> = BTreeSet::new();
    for p in &coupants {
        let Some(siens) = mois_de(p.debut, p.fin) else { return sans_dates() };
        coupes.extend(siens);
    }
    let restent: Vec<usize> = (0..siens.len()).filter(|i| !coupes.contains(&siens[*i])).collect();
    let (Some(&premier), Some(&dernier)) = (restent.first(), restent.last()) else { return Reste::Rien };
    let mois_restants = restent.iter().map(|i| siens[*i].libelle(socle::mois::Graphie::Accentuee)).collect::<Vec<_>>().join(", ");
    if dernier - premier + 1 != restent.len() {
        return Reste::NeSeComptePas(format!(
            "il reste au certificat ({}) des mois de part et d'autre d'un mois attesté ailleurs : ses jours ou cachets et son brut ne se partagent pas entre eux ; déposez les bulletins de ces mois",
            mois_restants
        ));
    }
    let somme = |f: fn(&Atteste) -> Option<i64>| -> Option<i64> { coupants.iter().map(|p| f(p)).sum() };
    let (heures, cachets, jours) = (somme(|p| p.heures), somme(|p| p.cachets), somme(|p| p.jours));
    let brut: Option<Decimal> = coupants.iter().map(|p| p.brut).sum();
    // Le certificat compte dans une seule unite (des cachets ou des jours) : le reste se compte dans la sienne ; l'autre
    // reste ce que le certificat en dit.
    let reste = |le_sien: Option<i64>, ailleurs: Option<i64>| match le_sien {
        Some(n) if n > 0 => ailleurs.map(|a| n - a),
        autre => autre,
    };
    let (reste_cachets, reste_jours) = (reste(c.cachets, cachets), reste(c.jours, jours));
    let reste_brut = c.brut.zip(brut).map(|(a, b)| a - b).filter(|b| *b > Decimal::ZERO);
    let unite_lue = c.cachets.is_none_or(|n| n == 0 || cachets.is_some_and(|x| x > 0)) && c.jours.is_none_or(|n| n == 0 || jours.is_some_and(|x| x > 0));
    let positifs = matches!((reste_cachets, reste_jours), (Some(a), Some(b)) if a >= 0 && b >= 0 && a + b > 0);
    let (true, true, Some(reste_brut)) = (unite_lue, positifs, reste_brut) else {
        return Reste::NeSeComptePas(format!(
            "le reste du certificat ({}) ne se compte pas : le certificat dit {}, ce qui l'atteste ailleurs {} ; déposez les bulletins de ces mois",
            mois_restants,
            c.en_mots(),
            en_mots(heures, cachets, jours, brut)
        ));
    };
    let debut = siens[premier].premier_jour().max(debut_c);
    let fin = siens[dernier].dernier_jour().min(fin_c);
    let mut ligne = c.ligne.clone();
    let mut b = reste_brut.round_dp(2);
    b.rescale(2);
    (ligne.debut, ligne.fin) = (Some(debut), Some(fin));
    ligne.contrat.cachets = reste_cachets;
    ligne.contrat.jours = reste_jours;
    ligne.contrat.brut = Some(b);
    ligne.contrat.base_conge = None;
    Reste::Seul(ligne)
}

/// Ce que l'import retient : (NIR, la ligne du contrat, le bloc du compte rendu qui le compte).
pub type Retenu = (String, ContratLu, &'static str);

/// Ce qu'un contrat de l'historique devient : (sa ligne, sa ligne nouvelle ; `None` : il est retire).
pub type Change = (ContratLu, Option<ContratLu>);

pub struct Registre {
    tous: Vec<Atteste>,
    pub retenus: Vec<Retenu>,
    pub changes: Vec<Change>,
    /// Les contrats de l'historique qui n'attestent rien, faute d'un NIR ou de dates : le compte rendu
    /// le dit.
    pub muets: Vec<String>,
}

/// Ce que l'ajout d'un contrat a fait des contrats qu'il remplace : une phrase par contrat.
pub type Remplaces = Vec<String>;

impl Registre {
    /// Le registre de l'historique : les contrats des engagements importes, rattaches a leur NIR par la fiche
    /// (`par_cle` : cle de fiche -> NIR). LE CONTRAT D'UN PERMANENT (`Importe::permanent`, celui que l'ecriture fond en un
    /// engagement : `import::ecrite`) s'atteste mois par mois, comme ses bulletins l'ont atteste : chaque mois verse, aux
    /// dates du mois dans le contrat, compte ce que sa paie porte ; un contrat sans terme (un CDI : « Date de fin
    /// previsionnelle : un CDI n'en a pas », `contrats-base.json`, S21.G00.40.010) n'est donc pas muet. Un autre contrat
    /// sans debut ou sans fin, ou dont la fiche n'a pas de NIR, ne se compare a rien : il n'atteste rien,
    /// et le registre le dit (`muets`).
    pub fn de_l_historique(ctx: &Contexte, par_cle: &HashMap<droit::contexte::IdSalarie, String>) -> R<Registre> {
        let mut tous = Vec::new();
        let mut muets = Vec::new();
        for i in engagement::importe::tous(&ctx.racine)? {
            let Some(c) = i.contrat_lu()? else { continue };
            let (cle, debut, fin) = (c.salarie.clone(), c.debut, c.fin);
            let permanent = i.permanent();
            let manque: Vec<&str> = [(debut.is_none(), "début"), (fin.is_none() && !permanent, "fin")].iter().filter(|m| m.0).map(|m| m.1).collect();
            let bornes = match (debut, fin) {
                (Some(d), Some(f)) => format!("du {} au {}", d.jj_mm_aaaa(), f.jj_mm_aaaa()),
                (Some(d), None) => format!("du {}, sans fin dite", d.jj_mm_aaaa()),
                (None, Some(f)) => format!("sans début dit, jusqu'au {}", f.jj_mm_aaaa()),
                (None, None) => "sans début ni fin dits".to_string(),
            };
            let quel = format!("le contrat de l'historique de {} {}", cle, bornes);
            if !manque.is_empty() {
                muets.push(format!("{} n'a pas de {} : il ne se compare à aucune pièce, une pièce du même contrat l'ajouterait une seconde fois", quel, manque.join(", ni de ")));
                continue;
            }
            let c = c.a_l_apercu();
            let Some(nir) = par_cle.get(&cle) else {
                muets.push(format!("{} : la fiche de ce salarié n'a pas de NIR, il ne se compare à aucune pièce, une pièce du même contrat l'ajouterait une seconde fois", quel));
                continue;
            };
            if !permanent {
                tous.push(lu(nir, &c, "historique"));
                continue;
            }
            for (mois, p) in i.paies() {
                let m = *mois;
                let (premier, dernier) = (m.premier_jour(), m.dernier_jour());
                let mut a = lu(nir, &c, "historique");
                a.debut = Some(match debut {
                    Some(d) => premier.max(d),
                    None => premier,
                });
                a.fin = Some(match fin {
                    Some(f) => dernier.min(f),
                    None => dernier,
                });
                (a.heures, a.cachets, a.jours, a.brut) = (p.heures, p.cachets, p.jours, p.brut);
                tous.push(a);
            }
        }
        Ok(Registre { tous, retenus: Vec::new(), changes: Vec::new(), muets })
    }

    /// Les contrats deja attestes qui chevauchent celui que la piece `piece` (« certificat », « AEM », « bulletin »)
    /// atteste. Un certificat ne compte que pour un certificat : un bulletin ou une AEM le remplace (`ajouter`). Vide :
    /// le contrat ne l'est pas.
    pub fn pareils(&self, nir: &str, debut: Jour, fin: Jour, piece: &str) -> Vec<Atteste> {
        self.tous
            .iter()
            .filter(|a| a.nir == nir && chevauche(a.debut, a.fin, debut, fin) && (piece == "certificat" || !a.de_certificat()))
            .cloned()
            .collect()
    }

    /// Ce qu'un certificat atteste seul, une fois retires les mois que les contrats `pareils` (ni des certificats, ni
    /// vides) attestent : `reste_du_certificat`.
    pub fn reste_du_certificat(&self, nir: &str, ligne: &ContratLu, pareils: &[Atteste]) -> Reste {
        reste_du_certificat(&lu(nir, ligne, "certificat"), &pareils.iter().collect::<Vec<_>>())
    }

    /// Retient le contrat `ligne` de la piece `source`, compte au bloc `bloc`. Il remplace, du meme salarie et aux dates
    /// qui chevauchent les siennes, les mois des certificats qu'il atteste, et, pour un bulletin, le contrat d'une AEM
    /// (l'appelant l'a compare : d'autres compteurs). Rend une phrase par contrat remplace.
    pub fn ajouter(&mut self, nir: &str, ligne: ContratLu, source: &'static str, bloc: &'static str) -> Remplaces {
        let a = lu(nir, &ligne, source);
        let mut phrases = Vec::new();
        if let (false, Some(debut), Some(fin)) = (a.de_certificat(), a.debut, a.fin) {
            let remplaces: Vec<usize> = (0..self.tous.len())
                .filter(|i| {
                    let x = &self.tous[*i];
                    x.nir == nir && chevauche(x.debut, x.fin, debut, fin) &&(x.de_certificat() || (source == "bulletin" && x.par == Attestation::Aem))
                })
                .collect();
            for i in remplaces.into_iter().rev() {
                let x = self.tous.remove(i);
                let nouvelle = if x.de_certificat() {
                    match reste_du_certificat(&x, &[&a]) {
                        Reste::Rien => {
                            phrases.push(format!("il remplace le contrat repris d'un certificat Congés Spectacles ({}) aux mêmes dates", x.d_ou()));
                            None
                        }
                        Reste::Seul(l) => {
                            let r = lu(nir, &l, x.source);
                            let date = |d: &Option<Jour>| d.iter().map(Jour::jj_mm_aaaa).collect::<String>();
                            phrases.push(format!(
                                "il remplace le contrat repris d'un certificat Congés Spectacles ({}) pour les mois que cette pièce atteste ; le reste du certificat, du {} au {}, reste attesté par lui : {} (ce que dit le certificat moins ce que dit cette pièce ; le brut du certificat est arrondi à l'euro)",
                                x.d_ou(),
                                date(&r.debut),
                                date(&r.fin),
                                r.en_mots()
                            ));
                            Some(l)
                        }
                        Reste::NeSeComptePas(m) => {
                            phrases.push(format!("il remplace le contrat repris d'un certificat Congés Spectacles ({}) ; {}", x.d_ou(), m));
                            None
                        }
                    }
                } else {
                    phrases.push(format!(
                        "il remplace le contrat de l'AEM ({}) aux mêmes dates, qui compte d'autres choses : l'AEM disait {}, le bulletin dit {} ; la pièce de paie l'emporte",
                        x.d_ou(),
                        x.en_mots(),
                        a.en_mots()
                    ));
                    None
                };
                self.changer(x, nouvelle);
            }
        }
        self.tous.push(a);
        self.retenus.push((nir.to_string(), ligne, bloc));
        phrases
    }

    /// `x`, retire du registre, devient `nouvelle` (`None` : il disparait), dans ce que l'import retient ou dans ce qu'il
    /// change a l'historique.
    fn changer(&mut self, x: Atteste, nouvelle: Option<ContratLu>) {
        match &x.historique {
            Some(h) => {
                self.changes.retain(|c| &c.0 != h);
                self.changes.push((h.clone(), nouvelle.clone()));
            }
            None => {
                let i = self.retenus.iter().position(|r| r.0 == x.nir && r.1 == x.ligne);
                match (i, &nouvelle) {
                    (Some(i), Some(l)) => self.retenus[i].1 = l.clone(),
                    (Some(i), None) => {
                        self.retenus.remove(i);
                    }
                    _ => {}
                }
            }
        }
        if let Some(l) = nouvelle {
            let mut r = lu(&x.nir, &l, x.source);
            r.historique = x.historique;
            self.tous.push(r);
        }
    }

    /// Combien ce bloc retient.
    pub fn nombre(&self, bloc: &str) -> usize {
        self.retenus.iter().filter(|r| r.2 == bloc).count()
    }

    /// Les lignes que ce bloc retient.
    pub fn lignes(&self, bloc: &str) -> Vec<ContratLu> {
        self.retenus.iter().filter(|r| r.2 == bloc).map(|r| r.1.clone()).collect()
    }
}
