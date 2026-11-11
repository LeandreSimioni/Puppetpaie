//! L'analyse des arguments d'une commande : options longues, abreviations non ambigues (`--fen` pour
//! `--fenetre`), `--option=valeur`, positionnels facultatifs, choix fermes. Un refus d'analyse a le code 2 :
//! `usage : ...` puis `moteur COMMANDE : <le refus>` sur la sortie d'erreur.
//!
//! Rien ici ne sait ce que fait une commande : une commande declare ses options (`Spec`), l'analyse rend ce qui a
//! ete tape (`Args`).

use std::collections::HashMap;

use socle::ligne::Argument;

/// Ce qu'une option attend.
#[derive(Clone, Copy, PartialEq)]
pub enum Genre {
    /// `--fenetre` : un drapeau.
    Drapeau,
    /// `--depuis JJ/MM/AAAA` : une valeur.
    Valeur,
    /// `--environnement production` : une valeur parmi des choix fermes.
    Choix(&'static [&'static str]),
}

pub struct Opt {
    pub nom: &'static str,
    pub genre: Genre,
    pub defaut: Option<&'static str>,
}

impl Opt {
    pub const fn drapeau(nom: &'static str) -> Opt {
        Opt { nom, genre: Genre::Drapeau, defaut: None }
    }

    pub const fn valeur(nom: &'static str) -> Opt {
        Opt { nom, genre: Genre::Valeur, defaut: None }
    }

    pub const fn avec_defaut(nom: &'static str, defaut: &'static str) -> Opt {
        Opt { nom, genre: Genre::Valeur, defaut: Some(defaut) }
    }

    pub const fn choix(nom: &'static str, choix: &'static [&'static str], defaut: Option<&'static str>) -> Opt {
        Opt { nom, genre: Genre::Choix(choix), defaut }
    }
}

/// Un argument sans tiret (`moteur coffre etat`), facultatif.
pub struct Pos {
    pub nom: &'static str,
    pub choix: Option<&'static [&'static str]>,
    pub defaut: Option<&'static str>,
}

impl Pos {
    pub const fn facultatif(nom: &'static str, defaut: Option<&'static str>) -> Pos {
        Pos { nom, choix: None, defaut }
    }

    pub const fn parmi(nom: &'static str, choix: &'static [&'static str], defaut: Option<&'static str>) -> Pos {
        Pos { nom, choix: Some(choix), defaut }
    }
}

/// Les options et les positionnels d'une commande.
pub struct Spec {
    pub commande: &'static str,
    pub positionnels: &'static [Pos],
    pub options: &'static [Opt],
}

/// La valeur d'une option ou d'un positionnel : tapee, le defaut de sa `Spec`, ou un drapeau leve (sans valeur).
#[derive(Debug)]
enum Valeur {
    Tapee(Argument),
    Defaut(&'static str),
    Drapeau,
}

/// Ce qui a ete tape.
#[derive(Default, Debug)]
pub struct Args {
    valeurs: HashMap<String, Valeur>,
}

impl Args {
    pub fn drapeau(&self, nom: &str) -> bool {
        self.valeurs.contains_key(nom)
    }

    pub fn texte(&self, nom: &str) -> Option<&str> {
        self.valeurs.get(nom).map(|v| match v {
            Valeur::Tapee(a) => a.texte(),
            Valeur::Defaut(d) => d,
            Valeur::Drapeau => "",
        })
    }

    /// L'argument tape pour `nom` (un jour, un mois s'en lisent : `socle::ligne::Argument`) ; `None` s'il n'a pas ete tape.
    pub fn tape(&self, nom: &str) -> Option<&Argument> {
        match self.valeurs.get(nom) {
            Some(Valeur::Tapee(a)) => Some(a),
            Some(Valeur::Defaut(_) | Valeur::Drapeau) | None => None,
        }
    }

    /// La valeur d'une option que sa `Spec` pourvoit d'un defaut (`Opt::avec_defaut`, `Opt::choix`, `Pos::parmi`) : elle
    /// est toujours la ; une commande qui la demande sans defaut se refuse en le disant.
    pub fn exige(&self, nom: &str) -> Result<&str, socle::echec::Echec> {
        self.texte(nom).ok_or_else(|| socle::echec::Echec { classe: Some(socle::echec::Classe::Manque), phrase: format!("L'option « {} » n'a pas de valeur.", nom), a_part: None })
    }
}

/// Un refus d'analyse : sa phrase.
pub struct Refus(pub String);

pub fn usage(spec: &Spec) -> String {
    let mut u = format!("usage : moteur {}", spec.commande);
    for o in spec.options {
        u.push_str(&match o.genre {
            Genre::Drapeau => format!(" [--{}]", o.nom),
            _ => format!(" [--{} {}]", o.nom, o.nom.to_uppercase().replace('-', "_")),
        });
    }
    for p in spec.positionnels {
        u.push_str(&format!(" [{}]", p.nom));
    }
    u
}

/// Les choix d'un refus : `'a', 'b'`.
fn liste(c: &[&str]) -> String {
    c.iter().map(|x| format!("'{}'", x)).collect::<Vec<_>>().join(", ")
}

/// Analyse `args` (ce qui suit le nom de la commande). Rend ce qui a ete tape, ou le refus.
pub fn analyser(spec: &Spec, args: &[Argument]) -> Result<Args, Refus> {
    let mut r = Args::default();
    for o in spec.options {
        if let Some(d) = o.defaut {
            r.valeurs.insert(o.nom.to_string(), Valeur::Defaut(d));
        }
    }
    let mut pos: Vec<&Argument> = Vec::new();
    let mut i = 0;
    let mut fin_des_options = false;
    while i < args.len() {
        let arg = &args[i];
        let a = arg.texte();
        i += 1;
        if fin_des_options || !a.starts_with("--") || a.len() == 2 {
            if !fin_des_options && a == "--" {
                fin_des_options = true;
                continue;
            }
            pos.push(arg);
            continue;
        }
        let (brut, valeur_collee) = match arg.option_collee() {
            Some((n, v)) => (n, Some(v)),
            None => (&a[2..], None),
        };
        // Le nom exact d'abord, puis l'abreviation si une seule option y repond.
        let opt = match spec.options.iter().find(|o| o.nom == brut) {
            Some(o) => o,
            None => {
                let candidates: Vec<&Opt> = spec.options.iter().filter(|o| o.nom.starts_with(brut)).collect();
                match candidates.len() {
                    1 => candidates[0],
                    0 => return Err(Refus(format!("argument inconnu : {}", a))),
                    _ => {
                        let noms: Vec<String> = candidates.iter().map(|o| format!("--{}", o.nom)).collect();
                        return Err(Refus(format!("option ambiguë : --{} peut être {}", brut, noms.join(", "))));
                    }
                }
            }
        };
        if opt.genre == Genre::Drapeau {
            if valeur_collee.is_some() {
                return Err(Refus(format!("l'option --{} ne prend pas de valeur", opt.nom)));
            }
            r.valeurs.insert(opt.nom.to_string(), Valeur::Drapeau);
            continue;
        }
        let v = match valeur_collee {
            Some(v) => v,
            None => match args.get(i) {
                Some(v) if !(v.texte().starts_with("--") && v.texte().len() > 2) => {
                    i += 1;
                    v.clone()
                }
                _ => return Err(Refus(format!("l'option --{} attend une valeur", opt.nom))),
            },
        };
        if let Genre::Choix(c) = opt.genre {
            if !c.contains(&v.texte()) {
                return Err(Refus(format!("l'option --{} ne prend pas « {} » (choix possibles : {})", opt.nom, v.texte(), liste(c))));
            }
        }
        r.valeurs.insert(opt.nom.to_string(), Valeur::Tapee(v));
    }
    if pos.len() > spec.positionnels.len() {
        let en_trop: Vec<&str> = pos[spec.positionnels.len()..].iter().map(|s| s.texte()).collect();
        return Err(Refus(format!("arguments inconnus : {}", en_trop.join(" "))));
    }
    for (k, p) in spec.positionnels.iter().enumerate() {
        match pos.get(k) {
            Some(v) => {
                if let Some(c) = p.choix.filter(|c| !c.contains(&v.texte())) {
                    return Err(Refus(format!("{} ne prend pas « {} » (choix possibles : {})", p.nom, v.texte(), liste(c))));
                }
                r.valeurs.insert(p.nom.to_string(), Valeur::Tapee((*v).clone()));
            }
            None => {
                if let Some(d) = p.defaut {
                    r.valeurs.insert(p.nom.to_string(), Valeur::Defaut(d));
                }
            }
        }
    }
    Ok(r)
}

/// Analyse `args` ; sur un refus, ecrit `usage` puis la phrase sur la sortie d'erreur et rend le code 2.
pub fn analyser_ou_sortir(spec: &Spec, args: &[Argument]) -> Result<Args, i32> {
    analyser(spec, args).map_err(|Refus(m)| {
        eprintln!("{}\nmoteur {} : {}", usage(spec), spec.commande, m);
        2
    })
}
