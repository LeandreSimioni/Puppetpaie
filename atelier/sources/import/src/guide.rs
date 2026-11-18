//! LE GUIDE DE L'IMPORT : ce qu'une compagnie qui vient d'un logiciel precedent met dans sa boite aux lettres `import/`,
//! ce que le moteur sait lire de chaque piece, et ce qui restera a dire. La page le montre tel quel
//! (`importation-etat`, `guide`) ; chaque phrase dit ce que le code de l'import lit, et rien d'autre :
//! - les bulletins : `bulletin_lu` (`Entreprise`, `Salarie`, `BulletinLu`), et la fiche que `bulletins` en tire
//!   (`fiche_entreprise`, `statut_et_metier`) ;
//! - les DSN : `declarations::dsn_lue::lire` (`DsnLue` : `EntrepriseDeclaree`, `IndividuDeclare`, `ContratDeclare`, `VersementLu`, le bloc 20, la licence 40.049,
//!   le numero d'adherent Conges Spectacles 40.022, le contact S20.G00.07) et `parametrage` (le code risque 40.040, les
//!   adhesions S21.G00.15) ;
//! - les certificats Congés Spectacles : `certificat_lu` (`Certificat`), et ce que `bulletins` en tire
//!   (`fiche_de_certificats`, `entreprise_du_certificat`, `contrat_du_certificat`) ;
//! - les AEM : `dsn_aem::lire_aem` (`ContratAtteste`) ; en PDF, `aem_lu` (`Aem`), et `bulletins` (`contrat_de_l_aem`) ;
//! - les AER Cinéma Spectacle : `aem_lu::lire_aer` (par les champs du formulaire, une `Aem` par periode) ;
//! - les archives : `tri` (un zip s'ouvre, son contenu se trie comme le reste ; un PDF sans texte ne se lit pas).
//!
//! L'ESSENTIEL D'ABORD (`essentiel`, `essentielles`) : les bulletins refont l'historique, une DSN deposee pre-remplit la
//! configuration (`routes::importation::parametrage`), tout dans un dossier qu'on choisit ; puis, a part, ce qu'on peut
//! mettre si certains documents manquent (`si_manquent`, `autres`).
//!
//! Ce qui restera a dire : ce qu'aucune de ces lectures ne donne -- la nature d'un contrat de permanent
//! (`nature`), et ce que « Pour demarrer » demande encore (les conges d'un permanent repris compris)
//! (`declarations::demarrage`), que la page montre apres l'import ; ce qu'une DSN pourrait donner s'y dit a part
//! (`declarations::demarrage::ParUneDsn`). Un document ajoute ensuite se reprend en relancant l'import (`rejouer`).

use std::path::Path;

/// Le guide, tel que la page le montre.
#[derive(Debug, serde::Serialize)]
pub struct Guide {
    /// L'essentiel, en quelques phrases.
    pub essentiel: &'static [&'static str],
    /// Les deux pieces de l'essentiel, les bulletins et une DSN deposee : ce qui s'en lit.
    pub essentielles: Vec<Piece>,
    /// La phrase qui ouvre les autres pieces.
    pub si_manquent: &'static str,
    /// Ce qu'on peut mettre aussi, si certains documents manquent.
    pub autres: Vec<Piece>,
    pub accepte: &'static str,
    pub reste: &'static str,
    /// Le meme chemin apres une premiere reprise : ajouter au dossier, relancer l'import.
    pub rejouer: &'static str,
    pub rien_n_est_perdu: &'static str,
    /// Le titre du guide replie, une fois un historique repris.
    pub replie: &'static str,
    /// Le titre de ce qui se lit d'une piece (`Piece::lu`).
    pub ce_qui_s_en_lit: &'static str,
    /// Le titre de ce qui est hors du chemin simple (`avance`), ou est l'apercu.
    pub avance: &'static str,
    /// Le geste de l'apercu (`importation-apercu`), sous `avance`.
    pub apercu: &'static str,
}

/// Une sorte de piece : ce que c'est, ou la trouver, ce qui s'en lit.
#[derive(Debug, serde::Serialize)]
pub struct Piece {
    pub quoi: &'static str,
    pub ou: &'static str,
    pub lu: &'static [&'static str],
}

/// Le guide, tel que la page le montre.
pub fn guide() -> Guide {
    Guide {
        essentiel: &[
            "Pour un import de bonne qualité, reprenez toutes vos fiches de paie : elles refont votre historique.",
            "Ajoutez une DSN déposée : elle pré-remplit la configuration de la compagnie.",
            "Mettez le tout dans un dossier sur l'ordinateur, puis choisissez ce dossier et importez.",
        ],
        essentielles: vec![
            Piece {
                quoi: "Les bulletins de paie, en PDF",
                ou: "Ceux que votre ancien logiciel imprime ou exporte : un bulletin par fichier, ou plusieurs dans un même fichier. Pas une photo ni un scan : le logiciel lit le texte du PDF.",
                lu: &[
                    "la compagnie : son nom, son adresse, son Siret, son code APE, sa convention collective ; son compte Urssaf et sa licence quand le bulletin les imprime",
                    "chaque salarié : nom, prénom, adresse, numéro de sécurité sociale, emploi, artiste ou non, cadre ou non, numéro Congés Spectacles ; sa date de naissance quand le bulletin l'imprime",
                    "chaque spectacle : son numéro d'objet et son titre",
                    "chaque paie : les jours travaillés, les cachets ou les heures, le brut, les cotisations, le net et l'impôt prélevé ; le bulletin lui-même se garde, et se rouvre depuis l'Historique",
                ],
            },
            Piece {
                quoi: "Les DSN déposées",
                ou: "Les fichiers que votre ancien logiciel a déposés chaque mois sur net-entreprises, s'il vous les a laissés. Mettez-les avec les bulletins : ils portent ce que les bulletins n'impriment pas toujours (le numéro Congés Spectacles, la licence, le contact des organismes, le code risque des accidents du travail, les contrats de prévoyance).",
                lu: &[
                    "la compagnie : son Siret, son adresse, son code APE, sa convention, sa licence, son numéro d'adhérent Congés Spectacles, la personne que les organismes contactent ; le compte qui paie les cotisations quand elles sont prélevées d'un seul compte",
                    "ses accidents du travail : le code risque et le taux de chaque année déclarée",
                    "ses organismes et ses contrats de prévoyance et de santé",
                    "chaque salarié : son identité entière, naissance comprise",
                    "chaque contrat : son début, sa fin, son numéro, sa durée de travail, son emploi",
                    "chaque mois : ce qui a été versé et déclaré",
                ],
            },
        ],
        si_manquent: "Si certains documents manquent, vous pourriez aussi mettre :",
        autres: vec![
            Piece {
                quoi: "Les certificats d'emploi Congés Spectacles, en PDF",
                ou: "Ceux que votre ancien logiciel a édités pour chaque intermittent, à la mise en page d'Audiens. Avec les bulletins, ils complètent sans rien reprendre deux fois ; seuls, ils suffisent à créer les intermittents et leurs contrats.",
                lu: &[
                    "la compagnie : son Siret, son nom, son numéro d'adhérent Congés Spectacles",
                    "chaque salarié : nom, prénom, adresse, numéro de sécurité sociale, emploi, cadre ou non",
                    "chaque contrat : ses dates, ses cachets (un artiste) ou ses jours (un technicien), son salaire brut arrondi à l'euro",
                ],
            },
            Piece {
                quoi: "Les attestations employeur (AEM)",
                ou: "Les fichiers d'attestations envoyés à France Travail pour les intermittents.",
                lu: &["chaque contrat d'intermittent : ses dates, ses cachets ou ses heures, son salaire, son numéro d'objet"],
            },
            Piece {
                quoi: "Les attestations employeur (AEM), en PDF",
                ou: "Celles que votre ancien logiciel a éditées et remises à chaque intermittent, une par mois. Avec les bulletins, elles complètent sans rien reprendre deux fois ; seules, elles suffisent à créer les intermittents et leurs contrats. Un formulaire rempli à la main dont le texte ne se lit pas est dit tel quel : déposez alors le bulletin ou le certificat du même mois.",
                lu: &[
                    "la compagnie : son Siret, son nom, son code APE, sa convention, sa commune, son numéro d'adhérent Congés Spectacles",
                    "chaque salarié : nom, prénom, date de naissance, adresse, numéro de sécurité sociale, emploi, artiste ou technicien, cadre ou non",
                    "chaque contrat : ses dates dans le mois, ses heures, ses cachets, ses jours, son salaire brut, son numéro d'objet",
                ],
            },
            Piece {
                quoi: "Les AER Cinéma Spectacle, en PDF",
                ou: "Les attestations que France Travail a éditées depuis vos DSN, telles que vous les avez reçues. Comme une AEM, chacune remplace un bulletin qui manque, sans rien reprendre deux fois.",
                lu: &[
                    "la compagnie : son Siret, son nom, son code APE, sa convention, sa commune",
                    "chaque salarié : nom, prénom, date de naissance, adresse, numéro de sécurité sociale, emploi",
                    "chaque période d'emploi : ses dates, ses heures, ses cachets, son salaire brut, le numéro d'objet de son contrat",
                ],
            },
            Piece {
                quoi: "Une archive zip",
                ou: "Telle que vous l'avez reçue : elle s'ouvre et son contenu se lit comme le reste.",
                lu: &[],
            },
        ],
        accepte: crate::tri::ACCEPTE,
        reste: "Après l'import, ce qu'une DSN pourrait donner se dit à part, pour l'ajouter ; ce qu'aucune pièce ne porte se demande, une fois, avec l'endroit où le trouver : par exemple qui signe les contrats, ou la date de la première embauche de la compagnie.",
        rejouer: "Un document oublié, une DSN, une année qui manquait : mettez-le dans le dossier et relancez l'import. Seul ce qui est neuf est repris, et les fiches ne reçoivent que ce qui leur manque. Un salarié repris sans numéro de sécurité sociale n'est pas reconnu par une DSN ajoutée ensuite : elle lui crée une seconde fiche.",
        rien_n_est_perdu: "Un document que le logiciel ne sait pas lire est dit, avec sa raison ; il ne change rien au reste. Les noms des fichiers ne comptent pas : le logiciel lit leur contenu.",
        replie: "Ce qu'on met dans le dossier",
        ce_qui_s_en_lit: "Ce qui s'en lit",
        avance: "Avancé",
        apercu: "Aperçu, sans rien écrire",
    }
}

/// LA COMPAGNIE COMMENCE PAR L'IMPORT : elle est venue par « d'un logiciel precedent » (`compagnies::Venue`) et rien
/// n'en est encore repris (aucun contrat ni aucune paie importes, `engagement::importe`). La page s'ouvre alors sur le
/// guide (`routes::ouverture::etat`).
pub fn commencer_par_l_import(racine: &Path) -> socle::echec::R<bool> {
    Ok(compagnie::installation::venue(racine)? == Some(compagnie::installation::Venue::LogicielPrecedent)
        && engagement::importe::contrats_lus(racine)?.is_empty()
        && engagement::importe::paies_lues(racine)?.is_empty())
}
