//! La mise en page : de la description validée aux pages du dessin (`dessin.rs`), en mesures exactes.
//!
//! Les blocs se posent l'un sous l'autre dans la zone utile de la page ; un bloc qui ne tient pas continue (un
//! paragraphe, un tableau, entre deux lignes) ou passe entier (des colonnes, une image) à la page suivante. Chaque
//! page reçoit d'abord le logo, l'en-tête et le pied (FORMAT.md).

use std::collections::VecDeque;
use std::path::Path;

use crate::description::{
    defaut, illisible, refus, resoudre, Alignement, Bloc, BlocEtiquete, CelluleTableau, Filet, LigneTableau, Morceau, Police,
    PoliceFixe, Refus, Repere, Rvb, CORPS_PLANCHER,
};
use crate::dessin::{Dessin, Famille, Image, Polices, Style};
use crate::mesure::Mesure;

/// La marge des comparaisons de hauteurs (un centième de millimètre) : un bloc qui dépasse la place de moins que
/// cela y tient.
const EPS: Mesure = Mesure::millioniemes(28_346);

/// La marge de la mise en lignes (un millième de millimètre) : un mot qui dépasse la ligne de moins que cela y tient.
const EPS_LIGNE: Mesure = Mesure::millioniemes(2_835);

/// Ce que la zone d'une cellule de tableau a de plus que sa ligne, sous le texte (un vingtième de millimètre).
const MARGE_SOUS_CELLULE: Mesure = Mesure::millioniemes(141_732);

/// Une taille posée : la largeur (la plus grande) et la hauteur (la somme) de ce qui a été posé.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Taille {
    l: Mesure,
    h: Mesure,
}

impl Taille {
    /// Rien de posé.
    const RIEN: Taille = Taille { l: Mesure::ZERO, h: Mesure::ZERO };

    fn new(l: Mesure, h: Mesure) -> Taille {
        Taille { l, h }
    }

    /// `autre` posée sous celle-ci.
    fn empiler(self, autre: Taille) -> Taille {
        Taille { l: self.l.max(autre.l), h: self.h + autre.h }
    }
}

/// Ce qu'un bloc a posé dans la zone qu'on lui donne, et s'il lui en reste pour la page suivante.
#[derive(Clone, Copy)]
struct Rendu {
    taille: Taille,
    encore: bool,
}

impl Rendu {
    /// Rien n'a été posé, et rien ne reste pour la page suivante.
    const VIDE: Rendu = Rendu { taille: Taille::RIEN, encore: false };

    /// Rien n'a tenu : tout à la page suivante.
    fn rien() -> Rendu {
        Rendu { taille: Taille::RIEN, encore: true }
    }
}

/// Une zone de la page : son coin haut gauche et sa taille, en points depuis le coin haut gauche de la page.
#[derive(Clone, Copy)]
struct Zone {
    x: Mesure,
    y: Mesure,
    l: Mesure,
    h: Mesure,
}

impl Zone {
    /// La zone dont le coin avance de (`dx`, `dy`), d'autant plus petite.
    fn decaler(&mut self, dx: Mesure, dy: Mesure) {
        self.x += dx;
        self.y += dy;
        self.l -= dx;
        self.h -= dy;
    }

    fn decalee(mut self, dx: Mesure, dy: Mesure) -> Zone {
        self.decaler(dx, dy);
        self
    }
}

/// Ce que la mise en page consulte : les polices, et le style du document (celui d'une ligne vide).
struct Ctx<'a> {
    polices: &'a Polices,
    base: Style,
}

// ---------------------------------------------------------------------------
// Les paragraphes.
// ---------------------------------------------------------------------------

/// Un morceau de texte d'un même style.
#[derive(Clone)]
struct Run {
    style: Style,
    texte: String,
}

/// Une ligne mise en page.
struct Ligne {
    /// Les morceaux de la ligne, sans espace final.
    runs: Vec<Run>,
    /// Sa largeur naturelle, sans espace final.
    largeur: Mesure,
    /// Ses espaces ordinaires (U+0020), sans l'espace final.
    espaces: usize,
    /// La dernière ligne du paragraphe, ou une ligne finie par `\n` : elle n'est pas justifiée.
    fin: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sorte {
    Mot,
    Espace,
}

struct Jeton {
    sorte: Sorte,
    pieces: Vec<Run>,
    largeur: Mesure,
}

enum Element {
    Jeton(Jeton),
    RetourALaLigne,
}

fn ajouter_caractere(pieces: &mut Vec<Run>, c: char, modele: &Run) {
    if let Some(dernier) = pieces.last_mut() {
        if dernier.style == modele.style {
            dernier.texte.push(c);
            return;
        }
    }
    pieces.push(Run { style: modele.style, texte: c.to_string() });
}

/// Les morceaux découpés en mots, espaces et retours à la ligne. Un mot peut s'étendre sur plusieurs morceaux de
/// styles différents (« FIL DE FER » en gras suivi de « , » ne se coupe pas entre les deux).
fn decouper(pol: &Polices, morceaux: &[Run]) -> Vec<Element> {
    let mut elements = Vec::new();
    let mut courant: Option<(Sorte, Vec<Run>)> = None;
    let fermer = |elements: &mut Vec<Element>, courant: &mut Option<(Sorte, Vec<Run>)>| {
        if let Some((sorte, pieces)) = courant.take() {
            let largeur = pieces.iter().map(|p| pol.largeur(&p.style, &p.texte)).sum();
            elements.push(Element::Jeton(Jeton { sorte, pieces, largeur }));
        }
    };
    for m in morceaux {
        for c in m.texte.chars() {
            if c == '\n' {
                fermer(&mut elements, &mut courant);
                elements.push(Element::RetourALaLigne);
                continue;
            }
            let sorte = if c == ' ' { Sorte::Espace } else { Sorte::Mot };
            if courant.as_ref().map(|(s, _)| *s) != Some(sorte) {
                fermer(&mut elements, &mut courant);
                courant = Some((sorte, Vec::new()));
            }
            ajouter_caractere(&mut courant.as_mut().expect("un jeton").1, c, m);
        }
    }
    fermer(&mut elements, &mut courant);
    elements
}

/// Un mot trop large coupé lettre à lettre, en morceaux qui tiennent dans `largeur`.
fn couper_le_mot(pol: &Polices, jeton: Jeton, largeur: Mesure) -> Vec<Jeton> {
    let mut sortie = Vec::new();
    let mut courant = Jeton { sorte: Sorte::Mot, pieces: Vec::new(), largeur: Mesure::ZERO };
    for piece in &jeton.pieces {
        for c in piece.texte.chars() {
            let lc = pol.largeur(&piece.style, &c.to_string());
            if courant.largeur + lc > largeur && !courant.pieces.is_empty() {
                sortie.push(std::mem::replace(
                    &mut courant,
                    Jeton { sorte: Sorte::Mot, pieces: Vec::new(), largeur: Mesure::ZERO },
                ));
            }
            ajouter_caractere(&mut courant.pieces, c, piece);
            courant.largeur += lc;
        }
    }
    if !courant.pieces.is_empty() {
        sortie.push(courant);
    }
    sortie
}

/// La ligne en cours de construction.
struct Constructeur {
    jetons: Vec<Jeton>,
    x: Mesure,
}

impl Constructeur {
    fn a_un_mot(&self) -> bool {
        self.jetons.iter().any(|t| t.sorte == Sorte::Mot)
    }

    fn pousser(&mut self, t: Jeton) {
        self.x += t.largeur;
        self.jetons.push(t);
    }

    /// La ligne finie : ses espaces de fin ne comptent pas.
    fn finir(&mut self, fin: bool) -> Ligne {
        while matches!(self.jetons.last(), Some(t) if t.sorte == Sorte::Espace) {
            self.jetons.pop();
        }
        let mut runs: Vec<Run> = Vec::new();
        let mut largeur = Mesure::ZERO;
        let mut espaces = 0;
        for t in self.jetons.drain(..) {
            largeur += t.largeur;
            if t.sorte == Sorte::Espace {
                espaces += t.pieces.iter().map(|p| p.texte.chars().count()).sum::<usize>();
            }
            for p in t.pieces {
                match runs.last_mut() {
                    Some(dernier) if dernier.style == p.style => dernier.texte.push_str(&p.texte),
                    _ => runs.push(p),
                }
            }
        }
        self.x = Mesure::ZERO;
        Ligne { runs, largeur, espaces, fin }
    }
}

/// Les morceaux mis en lignes de `largeur` au plus. `\n` force un retour ; un mot plus large que la ligne est coupé
/// lettre à lettre ; les espaces de début d'une ligne venue d'un retour automatique sont ignorés.
fn mettre_en_lignes(pol: &Polices, morceaux: &[Run], largeur: Mesure) -> Vec<Ligne> {
    let mut lignes = Vec::new();
    let mut b = Constructeur { jetons: Vec::new(), x: Mesure::ZERO };
    let mut retour_automatique = false;
    for e in decouper(pol, morceaux) {
        match e {
            Element::RetourALaLigne => {
                lignes.push(b.finir(true));
                retour_automatique = false;
            }
            Element::Jeton(t) if t.sorte == Sorte::Espace => {
                if !(retour_automatique && b.jetons.is_empty()) {
                    b.pousser(t);
                }
            }
            Element::Jeton(t) => {
                if b.x + t.largeur <= largeur + EPS_LIGNE {
                    b.pousser(t);
                    continue;
                }
                if b.a_un_mot() {
                    lignes.push(b.finir(false));
                    retour_automatique = true;
                } else if t.largeur > largeur + EPS_LIGNE {
                    // Seulement des espaces de début : on les abandonne si le mot doit être coupé.
                    b.jetons.clear();
                    b.x = Mesure::ZERO;
                }
                if t.largeur <= largeur + EPS_LIGNE {
                    b.pousser(t);
                } else {
                    let parts = couper_le_mot(pol, t, largeur);
                    let n = parts.len();
                    for (i, part) in parts.into_iter().enumerate() {
                        b.pousser(part);
                        if i + 1 < n {
                            lignes.push(b.finir(false));
                            retour_automatique = true;
                        }
                    }
                }
            }
        }
    }
    // Une dernière ligne vide après un `\n` final n'est pas émise ; un paragraphe vide non plus.
    if !b.jetons.is_empty() {
        lignes.push(b.finir(true));
    }
    lignes
}

/// Un paragraphe : ses morceaux, mis en lignes à la première pose puis posés ligne à ligne, page après page.
struct Paragraphe {
    morceaux: Vec<Run>,
    alignement: Alignement,
    /// Le pas des lignes, en facteur du corps (1,25 : comme `line-height: 1.25` en CSS).
    interligne: Mesure,
    lignes: Option<VecDeque<Ligne>>,
}

impl Paragraphe {
    fn new(alignement: Alignement, interligne: Mesure) -> Paragraphe {
        Paragraphe { morceaux: Vec::new(), alignement, interligne, lignes: None }
    }

    fn pousser(&mut self, style: Style, texte: String) {
        self.morceaux.push(Run { style, texte });
    }

    /// Le pas de la ligne et la distance de son haut à sa ligne de base : le plus grand de ses morceaux, la ligne de
    /// base au milieu de l'interligne (la moitié de l'écart de chaque côté).
    fn mesures(&self, ctx: &Ctx<'_>, ligne: &Ligne) -> (Mesure, Mesure) {
        let styles: Vec<Style> =
            if ligne.runs.is_empty() { vec![ctx.base] } else { ligne.runs.iter().map(|r| r.style).collect() };
        let mut pas = Mesure::ZERO;
        let mut base = Mesure::ZERO;
        for st in styles {
            let p = st.corps * self.interligne;
            let (asc, desc) = (ctx.polices.ascendante(&st), ctx.polices.descendante(&st));
            let demi = (p - (asc + desc)) / 2;
            pas = pas.max(p);
            base = base.max(demi + asc);
        }
        (pas, base)
    }

    /// La hauteur du paragraphe entier dans la largeur `largeur`, sans rien poser.
    fn hauteur(&self, ctx: &Ctx<'_>, largeur: Mesure) -> Mesure {
        mettre_en_lignes(ctx.polices, &self.morceaux, largeur).iter().map(|l| self.mesures(ctx, l).0).sum()
    }

    fn rendre(&mut self, ctx: &Ctx<'_>, dessin: &mut Dessin, mut zone: Zone) -> Rendu {
        let mut r = Rendu::VIDE;
        let max = zone.l;
        if self.lignes.is_none() {
            self.lignes = Some(mettre_en_lignes(ctx.polices, &self.morceaux, max).into());
            self.morceaux.clear();
        }
        while let Some(ligne) = self.lignes.as_mut().and_then(|l| l.pop_front()) {
            let (pas, base) = self.mesures(ctx, &ligne);
            if pas > zone.h {
                self.lignes.as_mut().expect("des lignes").push_front(ligne);
                r.encore = true;
                break;
            }
            if !ligne.runs.is_empty() {
                let decalage = match self.alignement {
                    Alignement::Gauche | Alignement::Justifie => Mesure::ZERO,
                    Alignement::Centre => (max - ligne.largeur) / 2,
                    Alignement::Droite => max - ligne.largeur,
                };
                let mut espace = Mesure::ZERO;
                if self.alignement == Alignement::Justifie && !ligne.fin && ligne.espaces > 0 {
                    let reste = max - ligne.largeur;
                    if reste > Mesure::ZERO {
                        espace = reste / ligne.espaces as i64;
                    }
                }
                let morceaux: Vec<(Style, &str)> = ligne.runs.iter().map(|m| (m.style, m.texte.as_str())).collect();
                dessin.texte(zone.x + decalage, zone.y + base, &morceaux, espace);
            }
            r.taille = r.taille.empiler(Taille::new(ligne.largeur, pas));
            zone.decaler(Mesure::ZERO, pas);
        }
        r
    }
}

// ---------------------------------------------------------------------------
// Les blocs.
// ---------------------------------------------------------------------------

/// Un bloc prêt à être posé.
enum Noeud {
    Paragraphe {
        avant: Mesure,
        apres: Mesure,
        para: Paragraphe,
        /// La marge à gauche : le texte se pose dans une largeur d'autant réduite.
        retrait: Mesure,
        commence: bool,
    },
    Espace {
        hauteur: Mesure,
        fait: bool,
    },
    Image {
        image: Image,
        fait: bool,
    },
    /// Une image posée là où l'on est, qui n'occupe aucune hauteur : le texte qui suit passe dessous, et elle peut
    /// descendre dans la marge du bas (FORMAT.md, `flottante`).
    ImageFlottante(Image),
    Colonnes(Colonnes),
    Tableau(Tableau),
    /// Un bloc à position absolue sur la page (FORMAT.md) : il ne consomme aucune hauteur.
    Fixe(Fixe),
    /// Le passage à la page suivante (chaque document commence sur une nouvelle page).
    Saut {
        fait: bool,
    },
}

/// Un texte, un trait ou une image posés à des coordonnées de la page, et non à l'endroit courant. Les coordonnées
/// partent du coin haut gauche de la PAGE : `origine` est la marge (gauche, haut) à retrancher du coin de la zone
/// qu'on nous donne.
struct Fixe {
    origine: (Mesure, Mesure),
    page: (Mesure, Mesure),
    objet: ObjetFixe,
}

enum ObjetFixe {
    Texte {
        texte: String,
        x: Mesure,
        /// La ligne de base.
        y: Mesure,
        style: Style,
        alignement: Alignement,
    },
    Trait {
        de: (Mesure, Mesure),
        a: (Mesure, Mesure),
        epaisseur: Mesure,
        couleur: Rvb,
    },
    Image {
        image: Image,
        x: Mesure,
        y: Mesure,
    },
}

/// Un texte d'une ligne posé dans `zone`, son coin haut gauche en (`x`, `y`) : sa ligne de base est à l'ascendante
/// sous ce coin. Un texte dont la hauteur ne tient pas sous ce coin, dans la zone, ne se pose pas.
/// Un texte a position fixe : `tient_sur_la_page` a refuse, a la construction, celui qui sortirait de sa page.
fn poser_texte(ctx: &Ctx<'_>, dessin: &mut Dessin, zone: Zone, x: Mesure, y: Mesure, style: Style, texte: &str) {
    dessin.texte(zone.x + x, zone.y + y + ctx.polices.ascendante(&style), &[(style, texte)], Mesure::ZERO);
}

impl Fixe {
    fn poser(&self, ctx: &Ctx<'_>, dessin: &mut Dessin, zone: Zone) {
        // La zone de la page entière, à l'origine de la page.
        let page = Zone { x: zone.x - self.origine.0, y: zone.y - self.origine.1, l: self.page.0, h: self.page.1 };
        match &self.objet {
            ObjetFixe::Texte { texte, x, y, style, alignement } => {
                if texte.is_empty() {
                    return;
                }
                let largeur = ctx.polices.largeur(style, texte);
                let x = match alignement {
                    Alignement::Droite => *x - largeur,
                    Alignement::Centre => *x - largeur / 2,
                    _ => *x,
                };
                poser_texte(ctx, dessin, page, x, *y - ctx.polices.ascendante(style), *style, texte);
            }
            ObjetFixe::Trait { de, a, epaisseur, couleur } => {
                dessin.trait_(&[(page.x + de.0, page.y + de.1), (page.x + a.0, page.y + a.1)], *epaisseur, *couleur);
            }
            ObjetFixe::Image { image, x, y } => dessin.image(image, page.x + *x, page.y + *y),
        }
    }
}

/// Un tableau prêt à être posé : les hauteurs de toutes ses lignes sont calculées une fois, dans la largeur où il
/// sera posé. Il se coupe entre les lignes (jamais dans une ligne), et son en-tête se redessine en haut de chaque
/// page où il continue.
struct Tableau {
    avant: Mesure,
    apres: Mesure,
    largeur: Mesure,
    /// Abscisse et largeur de chaque colonne.
    colonnes: Vec<(Mesure, Mesure)>,
    interligne: Mesure,
    /// haut, droite, bas, gauche.
    marge: [Mesure; 4],
    filet_lignes: Option<Filet>,
    filets_cellules: Option<Filet>,
    entete: Vec<LigneTableau>,
    lignes: Vec<LigneTableau>,
    h_entete: Vec<Mesure>,
    h_lignes: Vec<Mesure>,
    /// La prochaine ligne du corps à poser.
    suivante: usize,
    commence: bool,
}

struct Colonnes {
    filet: Option<Filet>,
    avant: Mesure,
    marge_haut: Mesure,
    colonnes: Vec<(Mesure, Vec<Noeud>)>,
    fait: bool,
}

impl Noeud {
    /// La hauteur que le bloc occupe dans une largeur donnée (sans rien dessiner).
    fn hauteur(&self, ctx: &Ctx<'_>, largeur: Mesure) -> Mesure {
        match self {
            Noeud::Paragraphe { avant, apres, para, retrait, .. } => *avant + para.hauteur(ctx, largeur - *retrait) + *apres,
            Noeud::Espace { hauteur, .. } => *hauteur,
            Noeud::Image { image, .. } => image.hauteur(),
            Noeud::ImageFlottante(_) | Noeud::Fixe(_) | Noeud::Saut { .. } => Mesure::ZERO,
            Noeud::Colonnes(c) => c.hauteur(ctx, largeur),
            // Le tableau entier, comme s'il tenait sur une page : pour qui l'additionne.
            Noeud::Tableau(t) => {
                t.avant + t.h_entete.iter().copied().sum::<Mesure>() + t.h_lignes.iter().copied().sum::<Mesure>() + t.apres
            }
        }
    }

    fn rendre(&mut self, ctx: &Ctx<'_>, dessin: &mut Dessin, zone: Zone) -> Rendu {
        match self {
            Noeud::Image { image, fait } => {
                if *fait {
                    return Rendu::VIDE;
                }
                if image.hauteur() > zone.h {
                    // Ne tient pas ici : à la page suivante.
                    return Rendu::rien();
                }
                dessin.image(image, zone.x, zone.y);
                *fait = true;
                Rendu { taille: Taille::new(image.largeur(), image.hauteur()), encore: false }
            }
            Noeud::Fixe(f) => {
                f.poser(ctx, dessin, zone);
                Rendu::VIDE
            }
            Noeud::ImageFlottante(image) => {
                // Elle se pose même au ras du bas.
                dessin.image(image, zone.x, zone.y);
                Rendu::VIDE
            }
            Noeud::Saut { fait } => {
                if *fait {
                    return Rendu::VIDE;
                }
                // Pas une taille nulle : elle arrêterait la mise en page si le saut ouvrait une page.
                *fait = true;
                Rendu { taille: Taille::new(Mesure::entier(1), Mesure::ZERO), encore: true }
            }
            Noeud::Espace { hauteur, fait } => {
                if *fait {
                    return Rendu::VIDE;
                }
                *fait = true;
                // Un espace qui déborde de la page est absorbé par le saut de page.
                Rendu { taille: Taille::new(Mesure::ZERO, if *hauteur > zone.h { zone.h } else { *hauteur }), encore: false }
            }
            Noeud::Paragraphe { avant, apres, para, retrait, commence } => {
                let decalage = if *commence { Mesure::ZERO } else { *avant };
                if decalage > zone.h {
                    return Rendu::rien();
                }
                let mut sous = zone.decalee(*retrait, decalage);
                sous.l = zone.l - *retrait;
                let r = para.rendre(ctx, dessin, sous);
                if r.encore && r.taille.h.est_nulle() {
                    // Pas même une ligne : tout à la page suivante, espace avant compris.
                    return Rendu::rien();
                }
                *commence = true;
                let mut hauteur = decalage + r.taille.h;
                if !r.encore {
                    let reste = zone.h - hauteur;
                    hauteur += if *apres > reste { reste.max(Mesure::ZERO) } else { *apres };
                }
                Rendu { taille: Taille::new(r.taille.l, hauteur), encore: r.encore }
            }
            Noeud::Colonnes(c) => c.rendre(ctx, dessin, zone),
            Noeud::Tableau(t) => t.rendre(ctx, dessin, zone),
        }
    }
}

impl Colonnes {
    fn epaisseur_filet(&self) -> Mesure {
        // Sans filet, aucune épaisseur.
        self.filet.iter().map(|&(_, e)| e).sum()
    }

    fn hauteur_contenu(&self, ctx: &Ctx<'_>, largeur: Mesure) -> Mesure {
        self.colonnes
            .iter()
            .map(|(frac, blocs)| {
                let l = largeur * *frac;
                blocs.iter().map(|b| b.hauteur(ctx, l)).sum::<Mesure>()
            })
            .fold(Mesure::ZERO, |a, b| a.max(b))
    }

    fn hauteur(&self, ctx: &Ctx<'_>, largeur: Mesure) -> Mesure {
        self.avant + self.epaisseur_filet() + self.marge_haut + self.hauteur_contenu(ctx, largeur)
    }

    fn rendre(&mut self, ctx: &Ctx<'_>, dessin: &mut Dessin, zone: Zone) -> Rendu {
        if self.fait {
            return Rendu::VIDE;
        }
        let largeur = zone.l;
        let besoin = self.hauteur(ctx, largeur);
        if besoin > zone.h + EPS {
            // Les colonnes ne se coupent pas : tout à la page suivante.
            return Rendu::rien();
        }
        let epaisseur = self.epaisseur_filet();
        let contenu = self.hauteur_contenu(ctx, largeur);
        let mut x = Mesure::ZERO;
        for (frac, blocs) in self.colonnes.iter_mut() {
            let lc = largeur * *frac;
            let mut colonne = zone.decalee(x, self.avant);
            colonne.l = lc;
            if let Some((couleur, e)) = self.filet {
                let y = colonne.y + e / 2;
                dessin.trait_(&[(colonne.x, y), (colonne.x + lc, y)], e, couleur);
            }
            let mut dedans = colonne.decalee(Mesure::ZERO, epaisseur + self.marge_haut);
            dedans.h = contenu + EPS;
            for b in blocs.iter_mut() {
                let r = b.rendre(ctx, dessin, dedans);
                dedans.decaler(Mesure::ZERO, r.taille.h);
            }
            x += lc;
        }
        self.fait = true;
        Rendu { taille: Taille::new(largeur, besoin), encore: false }
    }
}

impl Tableau {
    /// Le texte d'une cellule, à refaire à chaque pose (un paragraphe se consomme en se posant, et l'en-tête se pose
    /// sur chaque page).
    fn paragraphe(&self, c: &CelluleTableau, taille: Mesure) -> Paragraphe {
        let mut para = Paragraphe::new(c.alignement, self.interligne);
        for m in &c.morceaux {
            let st = Style::new(taille).couleur(m.couleur).gras(m.gras).italique(m.italique);
            para.pousser(st, m.texte.replace("\r\n", "\n").replace('\r', "\n"));
        }
        para
    }

    fn vide(c: &CelluleTableau) -> bool {
        c.morceaux.iter().all(|m| m.texte.is_empty())
    }

    /// Abscisse et largeur d'une cellule qui commence à la colonne `debut`.
    fn emprise(&self, debut: usize, etendue: usize) -> (Mesure, Mesure) {
        let x = self.colonnes[debut].0;
        let l: Mesure = self.colonnes[debut..debut + etendue].iter().map(|c| c.1).sum();
        (x, l)
    }

    fn largeur_texte(&self, l: Mesure) -> Mesure {
        l - self.marge[1] - self.marge[3]
    }

    /// La hauteur d'une ligne : celle de sa cellule la plus haute, marges comprises.
    fn hauteur_ligne(&self, ctx: &Ctx<'_>, l: &LigneTableau) -> Mesure {
        let mut debut = 0;
        let mut h = Mesure::ZERO;
        for c in &l.cellules {
            let (_, lc) = self.emprise(debut, c.etendue);
            debut += c.etendue;
            if Self::vide(c) {
                continue;
            }
            h = h.max(self.paragraphe(c, l.taille).hauteur(ctx, self.largeur_texte(lc)));
        }
        h + self.marge[0] + self.marge[2]
    }

    /// Fond, texte puis traits des lignes (haut de chacune en `y` dans `zone`) : dans cet ordre, pour que le fond de
    /// la ligne suivante ne recouvre pas le filet de celle-ci.
    fn poser_lignes(&self, ctx: &Ctx<'_>, dessin: &mut Dessin, zone: Zone, lignes: &[(&LigneTableau, Mesure, Mesure, bool)]) {
        for (l, y, h, _) in lignes {
            if let Some(f) = l.fond {
                dessin.rectangle(zone.x, zone.y + *y, self.largeur, *h, f);
            }
        }
        for (l, y, h, _) in lignes {
            let mut debut = 0;
            for c in &l.cellules {
                let (x, lc) = self.emprise(debut, c.etendue);
                debut += c.etendue;
                if Self::vide(c) {
                    continue;
                }
                let mut cellule = zone.decalee(x + self.marge[3], *y + self.marge[0]);
                cellule.l = self.largeur_texte(lc);
                cellule.h = *h - self.marge[0] + MARGE_SOUS_CELLULE;
                self.paragraphe(c, l.taille).rendre(ctx, dessin, cellule);
            }
        }
        for (l, y, h, corps) in lignes {
            let (x0, y0) = (zone.x, zone.y + *y);
            if let Some((couleur, e)) = self.filets_cellules {
                let mut debut = 0;
                for c in &l.cellules {
                    let (x, lc) = self.emprise(debut, c.etendue);
                    debut += c.etendue;
                    let (g, d) = (x0 + x, x0 + x + lc);
                    dessin.trait_(&[(g, y0), (d, y0), (d, y0 + *h), (g, y0 + *h), (g, y0)], e, couleur);
                }
            }
            let haut: Option<Filet> = l.filet_haut;
            let bas: Option<Filet> = l.filet_bas.or(if *corps { self.filet_lignes } else { None });
            if let Some((c, e)) = haut {
                dessin.trait_(&[(x0, y0 + e / 2), (x0 + self.largeur, y0 + e / 2)], e, c);
            }
            if let Some((c, e)) = bas {
                let yb = y0 + *h - e / 2;
                dessin.trait_(&[(x0, yb), (x0 + self.largeur, yb)], e, c);
            }
        }
    }

    fn rendre(&mut self, ctx: &Ctx<'_>, dessin: &mut Dessin, zone: Zone) -> Rendu {
        let decalage = if self.commence { Mesure::ZERO } else { self.avant };
        let dispo = zone.h - decalage + EPS;
        let h_entete: Mesure = self.h_entete.iter().copied().sum();
        // Les lignes du corps qui tiennent sous l'en-tête ; sans une seule (et sans corps du tout, l'en-tête seul),
        // tout passe à la page suivante.
        let mut fin = self.suivante;
        let mut bas = h_entete;
        while fin < self.lignes.len() && bas + self.h_lignes[fin] <= dispo {
            bas += self.h_lignes[fin];
            fin += 1;
        }
        let rien_ne_tient = if self.lignes.is_empty() { h_entete > dispo } else { fin == self.suivante };
        if rien_ne_tient {
            return Rendu::rien();
        }
        let mut a_poser: Vec<(&LigneTableau, Mesure, Mesure, bool)> = Vec::new();
        let mut y = decalage;
        for (l, h) in self.entete.iter().zip(&self.h_entete) {
            a_poser.push((l, y, *h, false));
            y += *h;
        }
        for k in self.suivante..fin {
            a_poser.push((&self.lignes[k], y, self.h_lignes[k], true));
            y += self.h_lignes[k];
        }
        self.poser_lignes(ctx, dessin, zone, &a_poser);
        self.suivante = fin;
        self.commence = true;
        let a_suivre = fin < self.lignes.len();
        let mut hauteur = y;
        if !a_suivre {
            let reste = zone.h - hauteur;
            hauteur += if self.apres > reste { reste.max(Mesure::ZERO) } else { self.apres };
        }
        Rendu { taille: Taille::new(self.largeur, hauteur), encore: a_suivre }
    }
}

// ---------------------------------------------------------------------------
// Les pages : le logo, l'en-tête et le pied.
// ---------------------------------------------------------------------------

/// Un en-tête ou un pied de page prêt à être posé.
struct ReperePose {
    repere: Repere,
    style: Style,
    /// Le pied se mesure depuis le bas de la page, l'en-tête depuis le haut.
    en_bas: bool,
    /// L'image du bout de la ligne, chargée une fois, et sa descente sous la ligne de base.
    image: Option<(Image, Mesure)>,
}

/// Ce qui se pose sur chaque page avant son contenu (FORMAT.md) : le logo en haut à droite, l'en-tête et le pied.
struct Decor {
    page: (Mesure, Mesure),
    /// Les marges : haut, droite, bas, gauche.
    marges: [Mesure; 4],
    logo: Option<(Image, Mesure, Mesure)>,
    reperes: Vec<ReperePose>,
    /// Le total de pages, quand on le connaît (deuxième passe) ; 0 sinon.
    total: usize,
}

impl Decor {
    /// Pose le décor de la page numéro `n` et rend la zone utile.
    fn poser(&self, ctx: &Ctx<'_>, dessin: &mut Dessin, n: usize) -> Zone {
        let (largeur, hauteur) = self.page;
        let page = Zone { x: Mesure::ZERO, y: Mesure::ZERO, l: largeur, h: hauteur };
        let [haut, droite, bas, gauche] = self.marges;
        if let Some((logo, marge_haut, marge_droite)) = &self.logo {
            dessin.image(logo, largeur - *marge_droite - logo.largeur(), *marge_haut);
        }
        for r in &self.reperes {
            if n == 1 && !r.repere.premiere_page {
                continue;
            }
            let ligne = if r.en_bas { hauteur - r.repere.distance } else { r.repere.distance };
            let y = ligne - ctx.polices.ascendante(&r.style);
            let mut fin = largeur - droite;
            // L'image tient le bout de la ligne ; le texte de droite s'arrête une espace avant elle.
            if let Some((image, descente)) = &r.image {
                let x = fin - image.largeur();
                dessin.image(image, x, ligne + *descente - image.hauteur());
                fin = x - ctx.polices.largeur(&r.style, " ");
            }
            let remplacer = |t: &str| t.replace("{page}", &n.to_string()).replace("{pages}", &self.total.to_string());
            for (texte, ou) in [(&r.repere.gauche, 0), (&r.repere.centre, 1), (&r.repere.droite, 2)] {
                let Some(texte) = texte else { continue };
                let texte = remplacer(texte);
                if texte.is_empty() {
                    continue;
                }
                let l = ctx.polices.largeur(&r.style, &texte);
                let x = match ou {
                    0 => gauche,
                    1 => (gauche + fin) / 2 - l / 2,
                    _ => fin - l,
                };
                poser_texte(ctx, dessin, page, x, y, r.style, &texte);
            }
        }
        Zone { x: gauche, y: haut, l: largeur - gauche - droite, h: hauteur - haut - bas }
    }
}

// ---------------------------------------------------------------------------
// La construction.
// ---------------------------------------------------------------------------

fn charger_image(chemin: &str, largeur: Mesure, ou: &str, quoi: &str) -> Result<Image, Refus> {
    let donnees = match std::fs::read(chemin) {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return refus(format!("{} : {} introuvable : {}", ou, quoi, chemin))
        }
        Err(e) => return illisible(format!("{} : {} illisible ({}) : {}", ou, quoi, e, chemin)),
    };
    let svg = Path::new(chemin).extension().is_some_and(|e| e.eq_ignore_ascii_case("svg"));
    Image::lire(&donnees, largeur, svg).map_err(|e| e.reformuler(|p| format!("{} : {} {} : {}", ou, quoi, chemin, p)))
}

/// Refuse un texte dont un caractère manque à la police elle-même (un idéogramme, un emoji) : tout ce que
/// Liberation possède — latin étendu, grec, cyrillique — s'imprime.
fn verifier_texte(ctx: &Ctx<'_>, style: Style, texte: &str, ou: &str) -> Result<(), Refus> {
    for c in texte.chars() {
        if c != '\n' && !ctx.polices.possede(&style, c) {
            return refus(format!(
                "{} : le caractère « {} » (U+{:04X}) n'existe pas dans la police Liberation",
                ou, c, c as u32
            ));
        }
    }
    Ok(())
}

/// Le style d'un morceau de texte, pour la vérification de ses caractères.
fn style_du_morceau(m: &Morceau) -> Style {
    Style::new(Mesure::UN).gras(m.gras).italique(m.italique)
}

/// D'où partent les coordonnées des blocs à position fixe (la marge gauche et haute), et la taille de la page.
struct Cadre {
    origine: (Mesure, Mesure),
    page: (Mesure, Mesure),
}

/// UN TEXTE A POSITION FIXE TIENT SUR SA PAGE (un texte fixe, un en-tete, un pied) : son haut a `y` depuis le haut de la
/// page, il ne descend pas sous son bas (`hauteur`). Un texte qui en sortirait ne se dessine pas : il se refuse, en
/// nommant le document et le texte. C'est un defaut du modele, pas de la saisie : ni police reduite, ni texte coupe.
fn tient_sur_la_page(ctx: &Ctx<'_>, style: Style, y: Mesure, hauteur: Mesure, document: Option<&str>, ou: &str, texte: &str) -> Result<(), Refus> {
    if ctx.polices.hauteur(&style) > hauteur - y {
        let document = match document {
            Some(t) => format!("Le document « {} »", t),
            None => "Le document".to_string(),
        };
        return defaut(format!(
            "{} pose « {} » ({}) sous le bas de sa page : le texte ne se dessinerait pas. C'est un défaut du logiciel, à signaler au développeur.",
            document, texte, ou
        ));
    }
    Ok(())
}

fn construire(
    b: &BlocEtiquete,
    ctx: &Ctx<'_>,
    largeur: Mesure,
    police: &Police,
    hauteur_page: Mesure,
    cadre: &Cadre,
    document: Option<&str>,
) -> Result<Noeud, Refus> {
    let ou = b.etiquette.as_str();
    let noeud = match &b.bloc {
        Bloc::Paragraphe { morceaux, taille, alignement, espace_avant, espace_apres, retrait } => {
            let mut para = Paragraphe::new(*alignement, police.interligne);
            for m in morceaux {
                let texte = m.texte.replace("\r\n", "\n").replace('\r', "\n");
                verifier_texte(ctx, style_du_morceau(m), &texte, ou)?;
                let st = Style::new(*taille).couleur(m.couleur).gras(m.gras).italique(m.italique);
                para.pousser(st, texte);
            }
            Noeud::Paragraphe { avant: *espace_avant, apres: *espace_apres, para, retrait: *retrait, commence: false }
        }
        Bloc::Espace(h) => Noeud::Espace { hauteur: *h, fait: false },
        Bloc::Image { chemin, largeur: l, flottante } => {
            let image = charger_image(chemin, *l, ou, "image")?;
            if image.largeur() > largeur + EPS {
                return refus(format!(
                    "{} : l'image ({} pt de large) est plus large que la place disponible ({:.1} pt)",
                    ou, l, largeur
                ));
            }
            if *flottante {
                Noeud::ImageFlottante(image)
            } else {
                Noeud::Image { image, fait: false }
            }
        }
        Bloc::TexteFixe { texte, x, y, taille, police: nom, gras, couleur, alignement, largeur_max } => {
            let texte = texte.replace("\r\n", "\n").replace('\r', "\n");
            if texte.contains('\n') {
                return refus(format!("{} : un texte à position fixe tient sur une ligne (pas de \\n)", ou));
            }
            let famille = match nom {
                PoliceFixe::Helvetica => Famille::Sans,
                PoliceFixe::Times => Famille::Times,
                PoliceFixe::Courier => Famille::Courier,
            };
            let style = Style::new(*taille).couleur(*couleur).famille(famille).gras(*gras);
            verifier_texte(ctx, style, &texte, ou)?;
            // `largeur_max` : le corps demandé tant que le texte y tient, sinon réduit au prorata (corps × place ÷
            // largeur), sans descendre sous le plancher, mesuré sur les métriques de la police embarquée.
            let style = match largeur_max {
                Some(place) => {
                    let largeur = ctx.polices.largeur(&style, &texte);
                    match taille.fois_rapport(place.en_millioniemes(), largeur.en_millioniemes()) {
                        Some(reduit) if largeur > *place => style.corps(reduit.max(CORPS_PLANCHER)),
                        _ => style,
                    }
                }
                None => style,
            };
            tient_sur_la_page(ctx, style, *y - ctx.polices.ascendante(&style), cadre.page.1, document, ou, &texte)?;
            Noeud::Fixe(Fixe {
                origine: cadre.origine,
                page: cadre.page,
                objet: ObjetFixe::Texte { texte, x: *x, y: *y, style, alignement: *alignement },
            })
        }
        Bloc::Trait { x1, y1, x2, y2, epaisseur, couleur } => Noeud::Fixe(Fixe {
            origine: cadre.origine,
            page: cadre.page,
            objet: ObjetFixe::Trait { de: (*x1, *y1), a: (*x2, *y2), epaisseur: *epaisseur, couleur: *couleur },
        }),
        Bloc::ImageFixe { chemin, x, y, largeur: l, hauteur: h, centree } => {
            // L'image tient dans la boîte (l × h), proportions gardées : d'abord à la pleine largeur, puis, si elle
            // déborde en hauteur, à la largeur qui remplit la hauteur.
            let mut image = charger_image(chemin, *l, ou, "image")?;
            let mut largeur_image = *l;
            if image.hauteur() > *h + EPS {
                if let Some(reduite) = l.fois_rapport(h.en_millioniemes(), image.hauteur().en_millioniemes()) {
                    largeur_image = reduite;
                    image = charger_image(chemin, largeur_image, ou, "image")?;
                }
            }
            let (dx, dy) =
                if *centree { ((*l - largeur_image) / 2, (*h - image.hauteur()) / 2) } else { (Mesure::ZERO, Mesure::ZERO) };
            Noeud::Fixe(Fixe {
                origine: cadre.origine,
                page: cadre.page,
                objet: ObjetFixe::Image { image, x: *x + dx, y: *y + dy },
            })
        }
        Bloc::Tableau {
            colonnes,
            interligne,
            marge_cellule,
            filet_lignes,
            filets_cellules,
            entete,
            lignes,
            espace_avant,
            espace_apres,
        } => {
            for l in entete.iter().chain(lignes.iter()) {
                for c in &l.cellules {
                    for m in &c.morceaux {
                        verifier_texte(ctx, style_du_morceau(m), &m.texte, ou)?;
                    }
                }
            }
            let mut x = Mesure::ZERO;
            let mut cols = Vec::new();
            for (frac, _) in colonnes {
                let lc = largeur * *frac;
                cols.push((x, lc));
                x += lc;
            }
            let [mh, md, mb, mg] = *marge_cellule;
            if let Some((k, (_, lc))) = cols.iter().enumerate().find(|(_, (_, lc))| *lc <= md + mg + EPS) {
                return refus(format!(
                    "{} : la colonne {} ({:.1} pt) est plus étroite que les marges de ses cellules",
                    ou,
                    k + 1,
                    lc
                ));
            }
            let mut t = Tableau {
                avant: *espace_avant,
                apres: *espace_apres,
                largeur,
                colonnes: cols,
                interligne: *interligne,
                marge: [mh, md, mb, mg],
                filet_lignes: *filet_lignes,
                filets_cellules: *filets_cellules,
                entete: entete.clone(),
                lignes: lignes.clone(),
                h_entete: Vec::new(),
                h_lignes: Vec::new(),
                suivante: 0,
                commence: false,
            };
            t.h_entete = t.entete.iter().map(|l| t.hauteur_ligne(ctx, l)).collect();
            t.h_lignes = t.lignes.iter().map(|l| t.hauteur_ligne(ctx, l)).collect();
            // L'en-tête et la plus haute ligne doivent tenir ensemble sur une page : sinon le tableau ne finirait
            // jamais de passer à la page suivante.
            let plus_haute = t.h_lignes.iter().copied().fold(Mesure::ZERO, |a, b| a.max(b));
            let besoin = t.avant + t.h_entete.iter().copied().sum::<Mesure>() + plus_haute;
            if besoin > hauteur_page + EPS {
                return refus(format!(
                    "{} : l'en-tête et la plus haute ligne du tableau occupent {:.1} pt, plus que la hauteur utile d'une page ({:.1} pt) ; une ligne ne se coupe pas entre deux pages",
                    ou, besoin, hauteur_page
                ));
            }
            Noeud::Tableau(t)
        }
        Bloc::Colonnes { filet, espace_avant, marge_interne_haut, colonnes } => {
            let mut cols = Vec::new();
            for (frac, blocs) in colonnes {
                let lc = largeur * *frac;
                let mut noeuds = Vec::new();
                for sb in blocs {
                    noeuds.push(construire(sb, ctx, lc, police, hauteur_page, cadre, document)?);
                }
                fondre_marges(&mut noeuds);
                cols.push((*frac, noeuds));
            }
            Noeud::Colonnes(Colonnes {
                filet: *filet,
                avant: *espace_avant,
                marge_haut: *marge_interne_haut,
                colonnes: cols,
                fait: false,
            })
        }
    };
    // Ce qui ne se coupe pas doit tenir sur une page.
    if matches!(noeud, Noeud::Colonnes(_) | Noeud::Image { .. }) {
        let h = noeud.hauteur(ctx, largeur);
        if h > hauteur_page + EPS {
            let quoi = if matches!(noeud, Noeud::Colonnes(_)) {
                "les colonnes ne se coupent pas entre deux pages, et elles"
            } else {
                "l'image"
            };
            return refus(format!(
                "{} : {} occupent {:.1} pt, plus que la hauteur utile d'une page ({:.1} pt)",
                ou, quoi, h, hauteur_page
            ));
        }
    }
    Ok(noeud)
}

/// Les marges verticales voisines se fondent, comme en CSS : entre un bloc qui laisse `a` après lui et un bloc qui
/// demande `b` avant lui, l'écart vaut max(a, b), pas a + b. Sans cela, un contrat de douze articles prenait deux
/// pages là où le rendu HTML d'avant en prenait une. Un bloc `espace` interrompt la fonte (il s'ajoute toujours).
fn fondre_marges(noeuds: &mut [Noeud]) {
    let mut apres_prec: Option<Mesure> = None;
    for n in noeuds.iter_mut() {
        let avant = match n {
            Noeud::Paragraphe { avant, .. } => Some(avant),
            Noeud::Colonnes(c) => Some(&mut c.avant),
            Noeud::Tableau(t) => Some(&mut t.avant),
            _ => None,
        };
        if let (Some(avant), Some(a)) = (avant, apres_prec) {
            let b = *avant;
            *avant = if b > a { b - a } else { Mesure::ZERO };
        }
        apres_prec = match n {
            Noeud::Paragraphe { apres, .. } => Some(*apres),
            Noeud::Colonnes(_) => Some(Mesure::ZERO),
            Noeud::Tableau(t) => Some(t.apres),
            _ => None,
        };
    }
}

/// Fabrique le PDF (ses octets) d'après une description.
///
/// Un `{pages}` dans l'en-tête ou le pied de page demande le nombre total de pages, qu'on ne connaît qu'une fois le
/// document mis en page : on le met donc en page DEUX fois, la première pour compter (le total y reste à 0), la
/// seconde avec le vrai nombre. La largeur d'un chiffre ne change rien aux sauts de page (l'en-tête et le pied sont
/// dans les marges), et le texte est aligné sur le bord, pas sur la largeur du total : le résultat est stable. Sans
/// `{pages}`, une seule passe.
pub fn fabriquer(description: &crate::forme::Description) -> Result<Vec<u8>, Refus> {
    let polices = Polices::charger()?;
    let (dessin, titre, veut_total) = fabriquer_passe(&polices, description, 0)?;
    if !veut_total {
        return dessin.finir(titre.as_deref());
    }
    let pages = dessin.nombre_de_pages();
    let (dessin, titre, _) = fabriquer_passe(&polices, description, pages)?;
    dessin.finir(titre.as_deref())
}

/// Une passe de mise en page : le dessin, le titre, et si l'en-tête ou le pied emploie `{pages}`.
fn fabriquer_passe(
    polices: &Polices,
    description: &crate::forme::Description,
    total: usize,
) -> Result<(Dessin, Option<String>, bool), Refus> {
    let d = resoudre(description)?;
    let ctx = Ctx { polices, base: Style::new(d.police.taille) };
    let cadre = Cadre { origine: (d.page.gauche, d.page.haut), page: (d.page.largeur, d.page.hauteur) };

    let logo = match &d.logo {
        Some(l) => Some((charger_image(&l.chemin, l.largeur, "logo", "fichier")?, l.marge_haut, l.marge_droite)),
        None => None,
    };
    let mut reperes = Vec::new();
    for (r, en_bas, nom) in [(&d.entete_page, false, "entete_page"), (&d.pied_page, true, "pied_page")] {
        if let Some(r) = r {
            let style = Style::new(r.taille).couleur(r.couleur).gras(r.gras);
            // Sa ligne de base, depuis le haut de la page : le pied se mesure depuis le bas (`Decor::poser`).
            let ligne = if en_bas { d.page.hauteur - r.distance } else { r.distance };
            for t in [&r.gauche, &r.centre, &r.droite].into_iter().flatten() {
                verifier_texte(&ctx, style, t, nom)?;
                if !t.is_empty() {
                    tient_sur_la_page(&ctx, style, ligne - ctx.polices.ascendante(&style), d.page.hauteur, d.titre.as_deref(), nom, t)?;
                }
            }
            let image = match &r.image {
                Some(i) => Some((charger_image(&i.chemin, i.largeur, nom, "image")?, i.descente)),
                None => None,
            };
            reperes.push(ReperePose { repere: r.clone(), style, en_bas, image });
        }
    }
    let veut_total = reperes.iter().any(|r| r.repere.emploie_total());
    let decor = Decor {
        page: (d.page.largeur, d.page.hauteur),
        marges: [d.page.haut, d.page.droite, d.page.bas, d.page.gauche],
        logo,
        reperes,
        total,
    };

    let largeur = d.page.largeur_utile();
    let hauteur_page = d.page.hauteur_utile();
    // Les blocs de tous les documents, l'un après l'autre ; chaque document commence sur une nouvelle page, sauf le
    // premier.
    let mut noeuds: Vec<Noeud> = Vec::new();
    for (i, blocs) in d.documents.iter().enumerate() {
        if i > 0 {
            noeuds.push(Noeud::Saut { fait: false });
        }
        let debut = noeuds.len();
        for b in blocs {
            noeuds.push(construire(b, &ctx, largeur, &d.police, hauteur_page, &cadre, d.titre.as_deref())?);
        }
        fondre_marges(&mut noeuds[debut..]);
    }

    let mut dessin = Dessin::new(d.page.largeur, d.page.hauteur);
    let mut suivant = 0;
    loop {
        dessin.nouvelle_page();
        let n = dessin.nombre_de_pages();
        let mut zone = decor.poser(&ctx, &mut dessin, n);
        // Les blocs se posent l'un sous l'autre tant que la page a de la place.
        let mut pose = Rendu::VIDE;
        while zone.h > Mesure::ZERO && suivant < noeuds.len() {
            let r = noeuds[suivant].rendre(&ctx, &mut dessin, zone);
            zone.decaler(Mesure::ZERO, r.taille.h);
            pose.taille = pose.taille.empiler(r.taille);
            if r.encore {
                pose.encore = true;
                break;
            }
            suivant += 1;
        }
        pose.encore = pose.encore || suivant < noeuds.len();
        if !pose.encore {
            break;
        }
        if pose.taille == Taille::RIEN {
            return refus("écriture du PDF impossible : un bloc ne tient pas sur une page neuve");
        }
    }
    Ok((dessin, d.titre.clone(), veut_total))
}
