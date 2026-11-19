//! LE BRANCHEMENT : ou part ce qu'un envoi fait partir. Trois branches, et un seul point d'entree que `moteur` choisit
//! pour la compagnie et passe aux envois :
//! - **le monde** : les organismes et les personnes, pour de vrai (net-entreprises, l'Urssaf, la boite courriel) ;
//! - **le simulateur** : la compagnie fictive, dont chaque envoi est joue sans quitter le poste (le depot rend un flux
//!   `FICTIF-…`, la boite est simulee, France Travail rend une AER fictive) ;
//! - **le message** : rien ne part ; chaque envoi rend le refus que porte la branche, avant que rien ne s'ecrive.
//!
//! Les envois ne savent pas pourquoi une compagnie est sur une branche : ils obeissent a celle qu'on leur passe.

use socle::echec::{erreur, Classe, R};

/// Le refus que rend la branche « message » : sa classe (que la page reconnait) et sa phrase, mot pour mot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Refus {
    pub classe: Classe,
    pub phrase: &'static str,
}

/// Ou part ce qu'un envoi fait partir.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Branchement {
    Monde,
    Simulateur,
    Message(Refus),
}

impl Branchement {
    /// Les envois sont-ils joues par le simulateur ?
    pub fn joue(self) -> bool {
        self == Branchement::Simulateur
    }

    /// Le refus de la branche « message », s'il y a lieu.
    pub fn refus(self) -> Option<Refus> {
        match self {
            Branchement::Message(r) => Some(r),
            _ => None,
        }
    }

    /// La porte d'un envoi : la branche « message » refuse, les deux autres laissent passer. Un envoi l'ouvre avant de
    /// rien lire ni ecrire.
    pub fn ouvrir(self) -> R<()> {
        match self.refus() {
            Some(r) => Err(erreur(r.classe, r.phrase)),
            None => Ok(()),
        }
    }

    /// LA PORTE D'UN DEPOT CHEZ UN ORGANISME (une DSN chez net-entreprises, une DPAE chez l'Urssaf, test comme reel) :
    /// celle de la branche (`ouvrir`), puis, pour le monde, le jour (`AuJourDuJour`). Le seul endroit qui le decide :
    /// le transport d'un depot (`transport::depot`) ne part pas sans ce qu'il rend.
    pub fn ouvrir_le_depot(self, quoi: crate::ligne::Acte) -> R<Porte> {
        match self {
            Branchement::Message(r) => Err(erreur(r.classe, r.phrase)),
            Branchement::Simulateur => Ok(Porte::Simulateur),
            Branchement::Monde => Ok(Porte::Monde(AuJourDuJour::constate(quoi)?)),
        }
    }
}

/// Ce que la porte d'un depot laisse passer : le simulateur joue le depot ; le monde l'envoie, le jour constate.
pub enum Porte {
    Monde(AuJourDuJour),
    Simulateur,
}

impl Porte {
    /// Le depot est-il joue par le simulateur ?
    pub fn joue(&self) -> bool {
        matches!(self, Porte::Simulateur)
    }
}

/// UN DEPOT QUI PART CHEZ L'ORGANISME LE JOUR REEL DE LA MACHINE : le jour du moteur (`socle::temps::jour_de_paris`, que
/// `--date` ou `PUPPETPAIE_AUJOURDHUI` deplacent) est celui de sa montre (`socle::temps::jour_de_la_montre`). Un fichier
/// constitue a un autre jour porterait un faux mois de constitution (S20.G00.05.007), que la plate-forme retient pour
/// le dernier ordre accepte : un essai constitue en decembre fait refuser les essais de novembre constitues en novembre
/// (B1-208-11). Le passe se refuse comme le futur : une seule regle, l'egalite. Il ne se fabrique qu'ici.
pub struct AuJourDuJour(());

impl AuJourDuJour {
    fn constate(quoi: crate::ligne::Acte) -> R<AuJourDuJour> {
        let (route, machine) = (socle::temps::jour_de_paris(), socle::temps::jour_de_la_montre());
        if route == machine {
            return Ok(AuJourDuJour(()));
        }
        let acte = match quoi {
            crate::ligne::Acte::Dsn => "Une DSN qui part chez net-entreprises",
            crate::ligne::Acte::Dpae => "Une DPAE qui part chez l'Urssaf",
            autre => return Err(erreur(Classe::Refuse, format!("Un dépôt chez un organisme est une DSN ou une DPAE, pas « {} ». Rien n'est parti.", autre.nom()))),
        };
        Err(erreur(
            Classe::Refuse,
            format!("{} se dépose à la date du jour : cette commande joue le {}, nous sommes le {}. Rien n'est parti.", acte, route.jj_mm_aaaa(), machine.jj_mm_aaaa()),
        ))
    }
}

// ----------------------------------------------------------------- le simulateur

/// Le flux d'un depot joue par le simulateur : `FICTIF-` et douze chiffres hexadecimaux ; `DEMO-` se lit encore (les
/// depots joues des fictives deja installees le portent), et leurs retours se jouent.
pub const PREFIXE_FLUX_FICTIF: &str = "FICTIF-";
pub const ANCIEN_PREFIXE_FLUX_FICTIF: &str = "DEMO-";

/// Ce que le point de depot aurait repondu a un depot joue (une DSN ou une DPAE) : un flux, et la mention.
pub fn reponse_fictive(quoi: crate::ligne::Acte, mode: socle::mode::Mode) -> R<String> {
    let acte = match quoi {
        crate::ligne::Acte::Dsn => "DSN",
        crate::ligne::Acte::Dpae => "DPAE",
        autre => return Err(erreur(Classe::Refuse, format!("Un dépôt joué est une DSN ou une DPAE, pas « {} ».", autre.nom()))),
    };
    let mut o = [0u8; 6];
    socle::temps::aleas(&mut o).map_err(|e| e.reformuler(|m| format!("pas d'alea du systeme : {}", m)).reclasser(Classe::Disque))?;
    let flux = format!("{}{}", PREFIXE_FLUX_FICTIF, o.iter().map(|b| format!("{:02X}", b)).collect::<String>());
    Ok(format!(
        "<idflux>{}</idflux>\nCompagnie fictive : {} {} jouee, rien n'a quitte le poste.",
        flux,
        acte,
        match mode {
            socle::mode::Mode::Reel => "reelle",
            socle::mode::Mode::Test => "de test",
        }
    ))
}
