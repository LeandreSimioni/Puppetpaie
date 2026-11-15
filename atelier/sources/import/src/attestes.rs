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
use droit::fiche::Nir;
use engagement::importe::{Attestation, ContratLu};
use crate::reponse::{Attestant, Source};
use socle::date::{Periode, PeriodeOuverte};
use socle::echec::R;
use socle::grandeurs::{Cachets, Heures, Jours, Montant};

/// Un contrat atteste : de qui (NIR), quand, par quoi, et ce qu'il compte.
#[derive(Clone, Debug)]
pub struct Atteste {
    pub(crate) nir: Nir,
    /// Ses dates : celles de la piece, ou, pour le contrat d'un permanent de l'historique, celles d'un mois verse.
    pub(crate) periode: Periode,
    /// L'AEM, le bulletin, le certificat (la piece de cet import), ou l'historique.
    pub(crate) source: Attestant,
    /// Ce qui l'atteste.
    pub par: Attestation,
    pub(crate) heures: Option<Heures>,
    pub cachets: Option<Cachets>,
    pub jours: Option<Jours>,
    pub brut: Option<Montant>,
    /// Sa ligne, telle qu'elle s'ecrit.
    ligne: ContratLu,
    /// Pour un contrat de l'historique : sa ligne telle que l'historique la porte (l'ecriture la retrouve par elle).
    historique: Option<ContratLu>,
}

impl Atteste {
    /// Son nom dans le compte rendu.
    pub fn d_ou(&self) -> String {
        match (self.source, self.par) {
            (Attestant::Historique, Attestation::Certificat) => "historique, d'un certificat".into(),
            (Attestant::Historique, Attestation::Aem) => "historique, d'une AEM".into(),
            (s, _) => s.nom().into(),
        }
    }

    pub fn de_certificat(&self) -> bool {
        self.par == Attestation::Certificat
    }

    /// Ce qu'il compte : ses heures, ses cachets, ses jours, son brut.
    pub fn compte(&self) -> (Option<Heures>, Option<Cachets>, Option<Jours>, Option<Montant>) {
        (self.heures, self.cachets, self.jours, self.brut)
    }

    /// Ce qu'il compte, en mots, pour dire un ecart avec ses deux valeurs.
    pub fn en_mots(&self) -> String {
        en_mots(self.heures, self.cachets, self.jours, self.brut)
    }

    /// Tient-il dans la periode d'une piece ?
    pub fn tient_dans(&self, piece: &Periode) -> bool {
        piece.croise(&self.periode) == Some(self.periode)
    }
}

/// Un compteur en mots : ce qu'il dit, ou qu'il ne dit rien.
fn compteur<G: socle::grandeurs::Grandeur>(x: Option<G>, mot: &str) -> String {
    match x {
        Some(n) => format!("{} {}(s)", socle::grandeurs::ecrit::sans_zero::dit(n), mot),
        None => format!("{}s non dit(e)s", mot),
    }
}

/// Des compteurs et un brut, en mots.
pub fn en_mots(heures: Option<Heures>, cachets: Option<Cachets>, jours: Option<Jours>, brut: Option<Montant>) -> String {
    let brut = match brut {
        Some(b) => format!("{} de brut", socle::texte::euros(b.decimal())),
        None => "brut illisible".to_string(),
    };
    format!("{}, {}, {}, {}", compteur(heures, "heure"), compteur(cachets, "cachet"), compteur(jours, "jour"), brut)
}

/// Ce qu'un contrat atteste aux dates `periode` dit, pour le registre.
fn lu(nir: &Nir, ligne: &ContratLu, periode: Periode, source: Attestant) -> Atteste {
    let c = &ligne.contrat;
    Atteste {
        nir: nir.clone(),
        periode,
        source,
        par: c.atteste_par,
        heures: c.heures,
        cachets: c.cachets,
        jours: c.jours,
        // Le brut que la ligne lue ecrit (un nombre de sa forme) : sa porte.
        brut: c.brut.map(Montant::du_decimal),
        ligne: ligne.clone(),
        historique: (source == Attestant::Historique).then(|| ligne.clone()),
    }
}

/// Ce qu'un certificat atteste seul (`reste_du_certificat`) : rien, son reste et ses dates, ou la phrase qui dit
/// pourquoi le reste ne se compte pas.
pub enum Reste {
    Rien,
    Seul(ContratLu, Periode),
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
fn reste_du_certificat(c: &Atteste, par: &[&Atteste]) -> R<Reste> {
    let siens = c.periode.mois();
    let coupants: Vec<&&Atteste> = par.iter().filter(|p| p.periode.croise(&c.periode).is_some()).collect();
    let coupes: BTreeSet<socle::mois::Mois> = coupants.iter().flat_map(|p| p.periode.mois()).collect();
    let restent: Vec<usize> = (0..siens.len()).filter(|i| !coupes.contains(&siens[*i])).collect();
    let (Some(&premier), Some(&dernier)) = (restent.first(), restent.last()) else { return Ok(Reste::Rien) };
    let mois_restants = restent.iter().map(|i| siens[*i].libelle(socle::mois::Graphie::Accentuee)).collect::<Vec<_>>().join(", ");
    if dernier - premier + 1 != restent.len() {
        return Ok(Reste::NeSeComptePas(format!(
            "il reste au certificat ({}) des mois de part et d'autre d'un mois attesté ailleurs : ses jours ou cachets et son brut ne se partagent pas entre eux ; déposez les bulletins de ces mois",
            mois_restants
        )));
    }
    fn somme<G: std::iter::Sum<G>>(coupants: &[&&Atteste], f: fn(&Atteste) -> Option<G>) -> Option<G> {
        coupants.iter().map(|p| f(p)).sum()
    }
    let (heures, cachets, jours) = (somme(&coupants, |p| p.heures), somme(&coupants, |p| p.cachets), somme(&coupants, |p| p.jours));
    let brut: Option<Montant> = coupants.iter().map(|p| p.brut).try_fold(Montant::ZERO, |a, b| b.map(|b| a + b));
    // Le certificat compte dans une seule unite (des cachets ou des jours) : le reste se compte dans la sienne ; l'autre
    // reste ce que le certificat en dit.
    fn reste<G: Copy + PartialOrd + std::ops::Sub<Output = G>>(le_sien: Option<G>, ailleurs: Option<G>, zero: G) -> Option<G> {
        match le_sien {
            Some(n) if n > zero => ailleurs.map(|a| n - a),
            autre => autre,
        }
    }
    let (reste_cachets, reste_jours) = (reste(c.cachets, cachets, Cachets::ZERO), reste(c.jours, jours, Jours::ZERO));
    let reste_brut = c.brut.zip(brut).map(|(a, b)| a - b).filter(|b| *b > Montant::ZERO);
    let unite_lue = c.cachets.is_none_or(|n| n.est_nul() || cachets.is_some_and(|x| x > Cachets::ZERO)) && c.jours.is_none_or(|n| n.est_nul() || jours.is_some_and(|x| x > Jours::ZERO));
    // Un reste se compte dans une seule unite : des cachets restants, ou des jours restants.
    let positifs = matches!((reste_cachets, reste_jours), (Some(a), Some(b)) if a >= Cachets::ZERO && b >= Jours::ZERO && (a > Cachets::ZERO || b > Jours::ZERO));
    let (true, true, Some(reste_brut)) = (unite_lue, positifs, reste_brut) else {
        return Ok(Reste::NeSeComptePas(format!(
            "le reste du certificat ({}) ne se compte pas : le certificat dit {}, ce qui l'atteste ailleurs {} ; déposez les bulletins de ces mois",
            mois_restants,
            c.en_mots(),
            en_mots(heures, cachets, jours, brut)
        )));
    };
    let mois_du_reste = Periode::du_au(siens[premier].premier_jour(), siens[dernier].dernier_jour())?;
    let periode = c
        .periode
        .croise(&mois_du_reste)
        .ok_or_else(|| socle::refus!("le reste du certificat ({}) tombe hors de son contrat ({})", mois_du_reste, c.periode))?;
    let mut ligne = c.ligne.clone();
    // La ligne lue ecrit son brut en nombre (`ContratLu`) : sa porte.
    let b = reste_brut.au_centime().decimal();
    (ligne.debut, ligne.fin) = (Some(periode.debut()), Some(periode.fin()));
    ligne.contrat.cachets = reste_cachets;
    ligne.contrat.jours = reste_jours;
    ligne.contrat.brut = Some(b);
    ligne.contrat.base_conge = None;
    Ok(Reste::Seul(ligne, periode))
}

/// Ce que l'import retient : (NIR, la ligne du contrat, le bloc du compte rendu qui le compte).
pub type Retenu = (Nir, ContratLu, BlocDImport);

/// LE BLOC DU COMPTE RENDU D'UN IMPORT qui compte un contrat retenu : les declarations (les contrats d'une DSN), les
/// bulletins, les certificats Congés Spectacles, les AEM en PDF.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlocDImport {
    Declarations,
    Bulletins,
    Certificats,
    AemPdf,
}

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
    pub fn de_l_historique(ctx: &Contexte, par_cle: &HashMap<droit::contexte::IdSalarie, Nir>) -> R<Registre> {
        let mut tous = Vec::new();
        let mut muets = Vec::new();
        for i in engagement::importe::tous(&ctx.racine)? {
            let Some(c) = i.contrat_lu()? else { continue };
            let (cle, debut, fin) = (c.salarie.clone(), c.debut, c.fin);
            let permanent = i.permanent();
            let bornes = match (debut, fin) {
                (Some(d), Some(f)) => format!("du {} au {}", d.jj_mm_aaaa(), f.jj_mm_aaaa()),
                (Some(d), None) => format!("du {}, sans fin dite", d.jj_mm_aaaa()),
                (None, Some(f)) => format!("sans début dit, jusqu'au {}", f.jj_mm_aaaa()),
                (None, None) => "sans début ni fin dits".to_string(),
            };
            let quel = format!("le contrat de l'historique de {} {}", cle, bornes);
            let muet = |manque: &str| format!("{} n'a pas de {} : il ne se compare à aucune pièce, une pièce du même contrat l'ajouterait une seconde fois", quel, manque);
            let contrat = match debut {
                Some(d) => PeriodeOuverte::du(d, fin)?,
                None if fin.is_none() && !permanent => {
                    muets.push(muet("début, ni de fin"));
                    continue;
                }
                None => {
                    muets.push(muet("début"));
                    continue;
                }
            };
            let fermee = contrat.fermee();
            if !permanent && fermee.is_none() {
                muets.push(muet("fin"));
                continue;
            }
            let c = c.a_l_apercu();
            let Some(nir) = par_cle.get(&cle) else {
                muets.push(format!("{} : la fiche de ce salarié n'a pas de NIR, il ne se compare à aucune pièce, une pièce du même contrat l'ajouterait une seconde fois", quel));
                continue;
            };
            if let (false, Some(periode)) = (permanent, fermee) {
                tous.push(lu(nir, &c, periode, Attestant::Historique));
                continue;
            }
            // Le contrat d'un permanent s'atteste mois par mois, aux dates du mois dans le contrat.
            for (mois, p) in i.paies() {
                let periode = contrat.croise(&Periode::du_mois(*mois)).ok_or_else(|| socle::refus!("{} : sa paie de {} est hors du contrat", quel, mois))?;
                let mut a = lu(nir, &c, periode, Attestant::Historique);
                (a.heures, a.cachets, a.jours, a.brut) = (p.heures, p.cachets, p.jours, p.brut);
                tous.push(a);
            }
        }
        Ok(Registre { tous, retenus: Vec::new(), changes: Vec::new(), muets })
    }

    /// Les contrats deja attestes qui chevauchent celui que la piece `piece` (un certificat, une AEM, un bulletin)
    /// atteste. Un certificat ne compte que pour un certificat : un bulletin ou une AEM le remplace (`ajouter`). Vide :
    /// le contrat ne l'est pas.
    pub fn pareils(&self, nir: &Nir, periode: &Periode, piece: Source) -> Vec<Atteste> {
        self.tous
            .iter()
            .filter(|a| a.nir == *nir && a.periode.croise(periode).is_some() && (piece == Source::Certificat || !a.de_certificat()))
            .cloned()
            .collect()
    }

    /// Ce qu'un certificat atteste seul aux dates `periode`, une fois retires les mois que les contrats `pareils` (ni des
    /// certificats, ni vides) attestent : `reste_du_certificat`.
    pub fn reste_du_certificat(&self, nir: &Nir, ligne: &ContratLu, periode: Periode, pareils: &[Atteste]) -> R<Reste> {
        reste_du_certificat(&lu(nir, ligne, periode, Attestant::Piece(Source::Certificat)), &pareils.iter().collect::<Vec<_>>())
    }

    /// Retient le contrat `ligne` de la piece `source`, aux dates `periode`, compte au bloc `bloc`. Il remplace, du meme
    /// salarie et aux dates qui chevauchent les siennes, les mois des certificats qu'il atteste, et, pour un bulletin, le
    /// contrat d'une AEM (l'appelant l'a compare : d'autres compteurs). Rend une phrase par contrat remplace.
    pub fn ajouter(&mut self, nir: &Nir, ligne: ContratLu, periode: Periode, source: Source, bloc: BlocDImport) -> R<Remplaces> {
        let a = lu(nir, &ligne, periode, Attestant::Piece(source));
        let mut phrases = Vec::new();
        if !a.de_certificat() {
            let remplaces: Vec<usize> = (0..self.tous.len())
                .filter(|i| {
                    let x = &self.tous[*i];
                    x.nir == *nir && x.periode.croise(&periode).is_some() && (x.de_certificat() || (source == Source::Bulletin && x.par == Attestation::Aem))
                })
                .collect();
            for i in remplaces.into_iter().rev() {
                let x = self.tous.remove(i);
                let nouvelle = if x.de_certificat() {
                    match reste_du_certificat(&x, &[&a])? {
                        Reste::Rien => {
                            phrases.push(format!("il remplace le contrat repris d'un certificat Congés Spectacles ({}) aux mêmes dates", x.d_ou()));
                            None
                        }
                        Reste::Seul(l, reste) => {
                            let r = lu(nir, &l, reste, x.source);
                            phrases.push(format!(
                                "il remplace le contrat repris d'un certificat Congés Spectacles ({}) pour les mois que cette pièce atteste ; le reste du certificat, du {} au {}, reste attesté par lui : {} (ce que dit le certificat moins ce que dit cette pièce ; le brut du certificat est arrondi à l'euro)",
                                x.d_ou(),
                                reste.debut().jj_mm_aaaa(),
                                reste.fin().jj_mm_aaaa(),
                                r.en_mots()
                            ));
                            Some((l, reste))
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
        self.retenus.push((nir.clone(), ligne, bloc));
        Ok(phrases)
    }

    /// `x`, retire du registre, devient `nouvelle` et ses dates (`None` : il disparait), dans ce que l'import retient ou
    /// dans ce qu'il change a l'historique.
    fn changer(&mut self, x: Atteste, nouvelle: Option<(ContratLu, Periode)>) {
        let ligne = nouvelle.as_ref().map(|n| n.0.clone());
        match &x.historique {
            Some(h) => {
                self.changes.retain(|c| &c.0 != h);
                self.changes.push((h.clone(), ligne));
            }
            None => {
                let i = self.retenus.iter().position(|r| r.0 == x.nir && r.1 == x.ligne);
                match (i, ligne) {
                    (Some(i), Some(l)) => self.retenus[i].1 = l,
                    (Some(i), None) => {
                        self.retenus.remove(i);
                    }
                    _ => {}
                }
            }
        }
        if let Some((l, periode)) = nouvelle {
            let mut r = lu(&x.nir, &l, periode, x.source);
            r.historique = x.historique;
            self.tous.push(r);
        }
    }

    /// Combien ce bloc retient.
    pub fn nombre(&self, bloc: BlocDImport) -> usize {
        self.retenus.iter().filter(|r| r.2 == bloc).count()
    }

    /// Les lignes que ce bloc retient.
    pub fn lignes(&self, bloc: BlocDImport) -> Vec<ContratLu> {
        self.retenus.iter().filter(|r| r.2 == bloc).map(|r| r.1.clone()).collect()
    }
}
