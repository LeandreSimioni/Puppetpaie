//! L'identite declaree d'une personne : ce qu'une piece (une DSN, une DPAE, une AEM, un certificat d'emploi, un
//! bulletin, un compte rendu NEOReS) ou une DSN partie dit de qui elle est ; et l'employeur qu'une piece imprime.

use droit::fiche::{Nir, Siret};
use serde::Serialize;
use socle::date::Jour;

use crate::ntt::{IdentifiantDIndividu, Ntt};

/// L'IDENTITE DECLAREE D'UNE PERSONNE : son nom de famille, ses prenoms, sa date de naissance, son NIR ou son NTT, chaque
/// case vide quand la piece ne la porte pas (ou la porte hors de sa forme, pour le NIR et la naissance). Ce sont les
/// donnees identifiantes du bloc 30 de la DSN (30.001, .002, .004, .006, .020 : `atelier/reference/norme-dsn/texte/
/// bloc-S21.G00.30.txt`) et de la DPAE. Le nom et les prenoms sont du texte : la norme ne leur donne qu'une longueur (la
/// DSN 80 caracteres, S21.G00.30.002 et .004 ; la DPAE 32, `droit::fiche::NomDeclare`), et la piece d'autrui ne se
/// refuse pas pour elle. Le NIR et le NTT peuvent aller ensemble : « Le NTT doit être renseigné sur la première DSN où
/// le NIR est attribué » (meme texte, l. 353-354).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct IdentiteDeclaree {
    pub nom: Option<String>,
    pub prenoms: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub naissance: Option<NaissanceDeclaree>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nir: Option<Nir>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ntt: Option<Ntt>,
}

impl IdentiteDeclaree {
    /// Ce qui l'identifie dans une DSN : son NIR, sinon son NTT ; aucun s'il n'a ni l'un ni l'autre.
    pub fn identifiant(&self) -> Option<IdentifiantDIndividu> {
        match (&self.nir, &self.ntt) {
            (Some(n), _) => Some(IdentifiantDIndividu::Nir(n.clone())),
            (None, Some(t)) => Some(IdentifiantDIndividu::Ntt(t.clone())),
            (None, None) => None,
        }
    }
}

/// L'EMPLOYEUR LU D'UNE PIECE d'autrui (un bulletin, une AEM, un certificat d'emploi Congés Spectacles) : la raison
/// sociale et le SIRET qu'elle imprime, chacun vide quand elle ne l'imprime pas (ou l'imprime hors de sa forme, pour le
/// SIRET : `Siret::d_une_piece`). La raison sociale est du texte, telle qu'imprimee ; la fiche Entreprise la range a sa
/// porte (`droit::fiche::RaisonSociale`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct EmployeurLu {
    pub raison_sociale: Option<String>,
    pub siret: Option<Siret>,
}

/// UNE DATE DE NAISSANCE DECLAREE (S21.G00.30.006) : `JJMMAAAA`, dont le jour ou le mois peuvent etre inconnus, ecrits
/// `00` ou `99` (cahier technique 2026.1, p. 172, `atelier/reference/norme-dsn/texte/bloc-S21.G00.30.txt`, l. 175-178 :
/// « Jour de naissance inconnu : 00MMAAAA ou 99MMAAAA, Mois de naissance inconnu : JJ00AAAA ou JJ99AAAA, Si jour et
/// mois inconnus : 0000AAAA ou 9999AAAA » ; CSL 00 et CSL-11, l. 185-189). Un compte rendu NEOReS l'ecrit `AAAA-MM-JJ`,
/// avec les memes inconnus (RO.B009.R008, R009 : `atelier/reference/sources/neores/
/// gip-mds-neores-cahier-technique-2027.1.0-20260303.txt`, l. 2872-2879). Elle se lit dans ces formes et elles seules :
/// un jour connu (`Connue` : une fiche, une piece qui l'imprime en entier, `du_jour`), ou des chiffres dont un est
/// inconnu, gardes tels qu'ils sont declares (`Incomplete`). Elle s'ecrit `JJMMAAAA` (`jjmmaaaa`) ou `AAAA-MM-JJ`
/// (`iso`, sa serde).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NaissanceDeclaree {
    Connue(Jour),
    Incomplete { jour: u32, mois: u32, an: socle::mois::Annee },
}

impl NaissanceDeclaree {
    /// Le motif de CSL 00 (`(0[0-9]|[1-2][0-9]|3[0-1]|99)(0[0-9]|1[0-2]|99)(18|19|20)[0-9]{2}`), et un jour du
    /// calendrier quand le jour et le mois sont connus (CSL-11) ; `None` sinon.
    pub fn lire(v: &str) -> Option<NaissanceDeclaree> {
        if v.len() != 8 || !v.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        NaissanceDeclaree::des_chiffres(&v[0..2], &v[2..4], &v[4..8])
    }

    /// `AAAA-MM-JJ`, les memes inconnus (un compte rendu NEOReS) ; `None` hors de cette forme.
    pub fn lire_iso(v: &str) -> Option<NaissanceDeclaree> {
        let b = v.as_bytes();
        let chiffres = |r: std::ops::Range<usize>| b[r].iter().all(u8::is_ascii_digit);
        if b.len() != 10 || b[4] != b'-' || b[7] != b'-' || !chiffres(0..4) || !chiffres(5..7) || !chiffres(8..10) {
            return None;
        }
        NaissanceDeclaree::des_chiffres(&v[8..10], &v[5..7], &v[0..4])
    }

    /// Le jour, le mois, l'annee, en chiffres.
    fn des_chiffres(jj: &str, mm: &str, aaaa: &str) -> Option<NaissanceDeclaree> {
        use socle::mois::Annee;
        use socle::piece::{format, Zone};
        let n = |t: &str| t.bytes().fold(0u32, |x, c| x * 10 + u32::from(c - b'0'));
        let (jour, mois) = (n(jj), n(mm));
        let an = Zone::lue(aaaa, format::AnneeEnChiffres).valeur()?.ok()?;
        let inconnu = |x: u32| x == 0 || x == 99;
        let forme = (jour <= 31 || jour == 99) && (mois <= 12 || mois == 99) && (Annee::constante(1800)..=Annee::constante(2099)).contains(&an);
        if !forme {
            return None;
        }
        if inconnu(jour) || inconnu(mois) {
            return Some(NaissanceDeclaree::Incomplete { jour, mois, an });
        }
        let m = Zone::lue(mm, format::MoisEnChiffres).valeur()?;
        Zone::lue(jj, format::JourDuMois(an.mois(m))).valeur().map(NaissanceDeclaree::Connue)
    }

    /// Celle d'un jour connu.
    pub fn du_jour(j: Jour) -> NaissanceDeclaree {
        NaissanceDeclaree::Connue(j)
    }

    /// Le jour qu'elle dit, quand le jour et le mois sont connus.
    pub fn jour(&self) -> Option<Jour> {
        match self {
            NaissanceDeclaree::Connue(j) => Some(*j),
            NaissanceDeclaree::Incomplete { .. } => None,
        }
    }

    /// `JJMMAAAA`, tel que declare.
    pub fn jjmmaaaa(&self) -> String {
        match self {
            NaissanceDeclaree::Connue(j) => j.jjmmaaaa(),
            NaissanceDeclaree::Incomplete { jour, mois, an } => format!("{:02}{:02}{}", jour, mois, an),
        }
    }

    /// `JJ/MM/AAAA`, le jour et le mois tels que declares.
    pub fn jj_mm_aaaa(&self) -> String {
        match self {
            NaissanceDeclaree::Connue(j) => j.jj_mm_aaaa(),
            NaissanceDeclaree::Incomplete { jour, mois, an } => format!("{:02}/{:02}/{}", jour, mois, an),
        }
    }

    /// `AAAA-MM-JJ`, le jour et le mois tels que declares.
    pub fn iso(&self) -> String {
        match self {
            NaissanceDeclaree::Connue(j) => j.iso(),
            NaissanceDeclaree::Incomplete { jour, mois, an } => format!("{}-{:02}-{:02}", an, mois, jour),
        }
    }
}

impl Serialize for NaissanceDeclaree {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.iso())
    }
}
