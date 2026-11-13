//! La lecture des retours DSN : ce que les organismes
//! repondent apres un depot.
//!
//! Des octets ou du XML en entree, des valeurs JSON en sortie : les bilans de
//! depot (natures 10 et 11), leurs anomalies, la liste des retours et des
//! depots, le CRM 94 (taux du PAS) par ses quatre strategies, les AER de
//! nature 46 (une archive .zip, lue ici a la main), les fiches de
//! parametrage des organismes complementaires.
//!
//! Le XML se lit en arbre d'elements dont le texte est
//! celui d'avant le premier enfant (commentaires et instructions ignores), les
//! balises et les attributs a espace de noms ecrits `{uri}nom`. Un XML qui ne se
//! lit pas est un refus `XmlIllisible` (la phrase est celle de `roxmltree`).

use serde::{Deserialize, Serialize};

use crate::tables_retours::{CODECS_ASCII, CODECS_AUTRES, CODECS_CP1252, CODECS_LATIN1, CODECS_REFUSES, CP437, DECIMAUX, NOM_DEBUT, NOM_SUITE, WINDOWS_1252};

use droit::contexte::TypeDeParametreNeores;
use droit::fiche::{Nic, Nir, Siren, Siret};
use droit::nature_declaration::{EssaiOuReel, NatureDeLaDeclaration};
use droit::natures::{Fraction, ListeDeLaNorme, NumeroDOrdre};
use droit::organismes::IdentifiantDOrganisme;
use droit::type_declaration::TypeDeLaDeclaration;
use socle::echec::{erreur as echec, Classe, Echec, R};
use socle::date::Jour;
use socle::garde_reseau::Url;
use socle::paris::{FuseauXsd, InstantOuJour, Moment};
use socle::piece::format;
use socle::piece::Zone;
use socle::texte::blanc;

// ---------------------------------------------------------------- les tables

droit::codes_fermes! {
    /// LA NATURE D'UN RETOUR, des deux canaux : la liste « Liste des retours, format, et disponibilité par environnement
    /// et régime » du guide d'implementation de l'API DSN v6.8, § 1.3, p. 11-13
    /// (`atelier/reference/sources/gip-mds-api-dsn/gip-mds-guide-implementation-api-dsn-v6.8-20260706.txt`, l. 573-788 :
    /// chaque code a sa ligne ; deux lignes « n/a », les fiches de parametrage OC et les BPIJ, n'ont pas de code), et le
    /// retour 41 du canal DPAE (FAQ de l'API DPAE, p. 5, `atelier/reference/norme-dpae/5493-API-DPAE-FAQ.pdf` : « Les
    /// retours en mode API (codes retour 10, 11, 20, 41) sont-ils au format DSN ? Réponse : Les formats des retours 10
    /// et 11 sont identiques à ceux délivrés en DSN. Les retours 20 et 41 sont spécifiques à la DPAE » ; « le retour 41
    /// qui correspond à l'accusé réception nommé aussi certificat de conformité des DPAE »). Les codes des deux canaux
    /// ne se recouvrent pas : 10, 11 et 20 sont ceux de la DSN. Son libelle, son organisme et son format : `NatureDuRetour::fiche`.
    NatureDuRetour, "Nature du retour", texte "guide de l'API DSN v6.8, l. 573-788, et FAQ de l'API DPAE, p. 5" {
        /// l. 583-585
        AccuseDEnregistrement = "10",
        /// l. 595-597
        CertificatDeConformite = "11",
        /// l. 598-600
        BilanDIdentification = "20",
        /// l. 601-604
        ControlesInterDeclarations = "21",
        /// l. 605-607
        ControlesArretSixMois = "22",
        /// l. 608-610
        ControlesTempsPartielTherapeutique = "23",
        /// l. 614
        AccuseAtRt = "30",
        /// l. 615-617
        CompteRenduAtRt = "31",
        /// l. 618-621
        AccuseArretSixMois = "32",
        /// l. 622-624
        CompteRenduArretSixMois = "33",
        /// l. 625
        TauxAtMp = "34",
        /// l. 626-628
        AccuseTptCnam = "35",
        /// l. 629-631
        CompteRenduTptCnam = "36",
        /// l. 632
        AccuseTptMsa = "37",
        /// l. 633
        CompteRenduTptMsa = "38",
        /// l. 634-636
        AccuseFranceTravail = "40",
        /// FAQ de l'API DPAE, p. 5
        CertificatDpae = "41",
        /// l. 637
        CompteRenduCdduD = "42",
        /// l. 638-640
        AerCdduD = "43",
        /// l. 641-645
        Fctu = "44",
        /// l. 646-648
        CompteRenduSpectacle = "45",
        /// l. 649-651
        AerSpectacle = "46",
        /// l. 611-612
        AccuseDeDistributionOc = "50",
        /// l. 613
        CompteRenduOc = "51",
        /// l. 655
        AccuseUrssaf = "60",
        /// l. 656-658
        CompteRenduUrssafAgregees = "61",
        /// l. 659-661
        AccuseAgircArrco = "70",
        /// l. 662-666
        CompteRenduAgircArrco = "71",
        /// l. 667-668
        AccuseCamieg = "72",
        /// l. 668-669
        CompteRenduCamieg = "73",
        /// l. 670-671
        AccuseCnieg = "74",
        /// l. 671-672
        CompteRenduCnieg = "75",
        /// l. 673-674
        AccuseCrpcen = "76",
        /// l. 674-675
        CompteRenduCrpcen = "77",
        /// l. 676-677
        AccuseCpf = "78",
        /// l. 677-678
        CompteRenduCpf = "79",
        /// l. 705-707
        CoherenceMsa = "80",
        /// l. 688-691
        AccuseIrcantec = "82",
        /// l. 691-694
        CompteRenduIrcantec = "83",
        /// l. 695-697
        AccuseCprpsncf = "84",
        /// l. 698-701
        AccuseCrpnpac = "86",
        /// l. 701-704
        CompteRenduCrpnpac = "87",
        /// l. 708-710
        MandatAAcquitter = "90",
        /// l. 711-713
        AccuseDgfip = "92",
        /// l. 714-716
        CompteRenduDgfipAgregees = "93",
        /// l. 717-719
        TauxPas = "94",
        /// l. 720-722
        TauxSurTaux = "95",
        /// l. 723-724
        AccuseRafp = "100",
        /// l. 724-725
        CompteRenduRafp = "101",
        /// l. 726
        AccuseFspoeie = "102",
        /// l. 727-728
        AccuseCnracl = "104",
        /// l. 728-729
        CompteRenduCnracl = "105",
        /// l. 730-731
        AccuseEnim = "106",
        /// l. 731-732
        CompteRenduEnim = "107",
        /// l. 733-734
        AccuseCnbf = "110",
        /// l. 734-735
        CompteRenduCnbf = "111",
        /// l. 736
        CompteRenduSadv = "114",
        /// l. 737-738
        AccuseCibtp = "115",
        /// l. 738-739
        CompteRenduCibtp = "116",
        /// l. 740-742
        BonusMalus = "117",
        /// l. 743-745
        Effectifs = "118",
        /// l. 746-748
        UrssafImmediat = "119",
        /// l. 749-751
        UrssafOpposable = "120",
        /// l. 752-754
        Identite = "121",
        /// l. 755-756
        AccuseFctuCibtp = "122",
        /// l. 756-757
        CompteRenduFctuCibtp = "123",
        /// l. 758-760
        UrssafRappel = "124",
        /// l. 761-763
        RappelSubstitutionMsa = "125",
        /// l. 764-766
        FluxFinancierCavec = "127",
        /// l. 767-769
        ResolutionAnomaliesMsa = "130",
        /// l. 770-772
        PostSubstitution = "132",
        /// l. 652-654
        CompteRenduFctuNeores = "135",
        /// l. 786-788
        PostExigibiliteMsa = "136",
    }
}

impl NatureDuRetour {
    /// SA LIGNE de la liste des retours (guide v6.8, l. 573-788), pour les natures qu'une compagnie du spectacle recoit :
    /// son libelle (celui que le journal des envois ecrit, `RetourOrganisme.libelle`), l'organisme qui le rend (la
    /// colonne « OPS ») et son format (« Format phase 3 ») : « harmonise », le schema des bilans (`lire_rapport`) ;
    /// « aucun », un accuse sans document, son statut dit tout ; « pdf », une archive d'AER ; « neores », le format
    /// NEOReS (`lire_neores`, cahier technique depose dans `atelier/reference/sources/neores/`) ; « oc », le compte
    /// rendu des organismes complementaires (`lire_crm_oc`, specification et schema V1.1.4 deposes dans
    /// `gip-mds-api-dsn/`) ; « agirc-arrco », le format temporaire de l'AGIRC-ARRCO, que personne ne publie (le guide le
    /// cite, [7], sans le joindre), et qui ne se traduit donc pas. Le format de la nature 93 n'est pas rempli dans le
    /// tableau : « harmonise » est une INFERENCE (la nature 95, de la meme DGFiP, a son exemple sous « Format DSN
    /// harmonise », § 6.3). Une autre nature de la liste n'a pas de fiche ici : elle se lit, se range et se dit par son
    /// code.
    fn fiche(self) -> Option<(&'static str, &'static str, &'static str)> {
        use NatureDuRetour as N;
        Some(match self {
            N::AccuseDEnregistrement => ("Accuse d'enregistrement electronique / avis de rejet", "le point de dépôt (SI DSN)", "harmonise"),
            N::CertificatDeConformite => ("Certificat de conformite / bilan d'anomalies", "le point de dépôt (SI DSN)", "harmonise"),
            N::BilanDIdentification => ("Bilan d'identification des salaries", "le SI DSN", "harmonise"),
            N::ControlesInterDeclarations => ("Controles inter-declarations / signalement AT-RT", "le SI DSN", "harmonise"),
            N::ControlesArretSixMois => ("Controles arret de plus de 6 mois", "le SI DSN", "harmonise"),
            N::ControlesTempsPartielTherapeutique => ("Controles temps partiel therapeutique", "le SI DSN", "harmonise"),
            N::TauxAtMp => ("Compte-rendu metier taux AT/MP (notification de la Carsat)", "la CNAM (taux AT/MP)", "harmonise"),
            N::AccuseFranceTravail => ("Accuse de reception France Travail", "France Travail", "aucun"),
            N::CompteRenduCdduD => ("Compte-rendu metier CDDU-D", "France Travail", "harmonise"),
            N::AerCdduD => ("AER CDDU-D (archive PDF)", "France Travail", "pdf"),
            N::Fctu => ("AER ou compte-rendu metier FCTU", "France Travail", "harmonise"),
            N::CompteRenduSpectacle => ("Compte-rendu metier « cinema, spectacle, expatries »", "France Travail", "harmonise"),
            N::AerSpectacle => ("AER « cinema, spectacle » (archive PDF)", "France Travail", "pdf"),
            N::AccuseDeDistributionOc => ("Accuse de distribution a l'organisme complementaire", "l'organisme complémentaire", "aucun"),
            N::CompteRenduOc => ("Compte-rendu metier de l'organisme complementaire", "l'organisme complémentaire", "oc"),
            N::AccuseUrssaf => ("Accuse de reception URSSAF", "l'Urssaf", "aucun"),
            N::CompteRenduUrssafAgregees => ("Compte-rendu metier URSSAF, donnees agregees", "l'Urssaf", "harmonise"),
            N::AccuseAgircArrco => ("Accuse de reception AGIRC-ARRCO", "l'AGIRC-ARRCO", "aucun"),
            N::CompteRenduAgircArrco => ("Compte-rendu metier AGIRC-ARRCO", "l'AGIRC-ARRCO", "agirc-arrco"),
            N::AccuseDgfip => ("Accuse de reception DGFiP", "la DGFiP", "aucun"),
            N::CompteRenduDgfipAgregees => ("Compte-rendu metier DGFiP, donnees agregees", "la DGFiP", "harmonise"),
            N::TauxPas => ("Compte-rendu metier DGFiP, donnees nominatives (taux du PAS)", "la DGFiP", "harmonise"),
            N::TauxSurTaux => ("Compte-rendu metier DGFiP, controles taux sur taux", "la DGFiP", "harmonise"),
            N::BonusMalus => ("Compte-rendu metier bonus-malus contribution chomage", "l'Urssaf ou la MSA", "harmonise"),
            N::Effectifs => ("Compte-rendu metier effectifs", "l'Urssaf ou la MSA", "neores"),
            N::UrssafImmediat => ("Compte-rendu metier URSSAF H+4 (immediat)", "l'Urssaf", "neores"),
            N::UrssafOpposable => ("Compte-rendu metier URSSAF J+5 (opposable)", "l'Urssaf", "neores"),
            N::Identite => ("Compte-rendu metier identite", "le SI DSN", "neores"),
            N::UrssafRappel => ("Compte-rendu metier URSSAF de rappel", "l'Urssaf", "neores"),
            _ => return None,
        })
    }

    /// Son libelle, s'il a une fiche.
    pub fn libelle(self) -> Option<&'static str> {
        self.fiche().map(|f| f.0)
    }

    /// Son libelle dit : celui de sa fiche, sinon son code.
    pub fn libelle_dit(self) -> String {
        match self.libelle() {
            Some(l) => l.to_string(),
            None => format!("nature {} (inconnue de ce moteur)", self),
        }
    }

    /// L'organisme qui le rend, s'il a une fiche.
    pub fn organisme(self) -> Option<&'static str> {
        self.fiche().map(|f| f.1)
    }

    /// Son format, s'il a une fiche.
    pub fn format(self) -> Option<&'static str> {
        self.fiche().map(|f| f.2)
    }

    /// Son libelle sur le canal d'un envoi `acte`, celui que le journal des envois ecrit : le canal DPAE a les siens,
    /// pour les quatre retours que la FAQ de l'API DPAE nomme (10, 11, 20, 41, p. 5).
    pub fn libelle_du_canal(self, acte: crate::ligne::Acte) -> String {
        use NatureDuRetour as N;
        if acte != crate::ligne::Acte::Dpae {
            return self.libelle_dit();
        }
        match self {
            N::AccuseDEnregistrement => "avis de rejet du depot".into(),
            N::CertificatDeConformite => "certificat de conformite / bilan d'anomalies".into(),
            N::BilanDIdentification => "bilan d'identification des salaries".into(),
            N::CertificatDpae => "certificat de conformite des DPAE".into(),
            autre => format!("nature {} (inconnue du canal DPAE)", autre),
        }
    }
}

/// Le dossier des retours des organismes, sous la racine d'une compagnie.
pub const RETOURS: &str = "retours";

/// L'IDENTIFIANT D'UN FLUX : ce que la plate-forme donne a un depot, et par quoi elle range ses retours (« Identifiant du
/// flux DSN », `depot.idflux` et `flux.id`, « [0-9A-Za-z.-_]* », [1,50] : guide d'implementation de l'API DSN v6.8,
/// l. 1448-1453, 1568-1573, 1676-1681,
/// `atelier/reference/sources/gip-mds-api-dsn/gip-mds-guide-implementation-api-dsn-v6.8-20260706.txt`). INFERENCE : la
/// classe se lit « chiffres, lettres, `.`, `-`, `_` » (lue comme une expression reguliere, `.-_` serait un intervalle).
/// Le canal DPAE n'a pas de texte depose pour la forme du sien (son guide se dit compatible avec celui de la DSN) ; le
/// simulateur ecrit `FICTIF-` et douze chiffres hexadecimaux (`branchement::reponse_fictive`). Il s'ecrit et se lit
/// par son texte ; une autre forme se refuse.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct IdDeFlux(String);

impl IdDeFlux {
    /// Le flux que ce texte ecrit, s'il en a la forme.
    pub fn lu(texte: &str) -> Option<IdDeFlux> {
        let forme = (1..=50).contains(&texte.len()) && texte.bytes().all(|o| o.is_ascii_alphanumeric() || matches!(o, b'.' | b'-' | b'_'));
        forme.then(|| IdDeFlux(texte.to_string()))
    }

    /// Le flux ecrit `texte` ; refus s'il n'en a pas la forme.
    pub fn lire(texte: &str) -> R<IdDeFlux> {
        IdDeFlux::lu(texte).ok_or_else(|| {
            socle::refus!(
                "Identifiant de flux {} : il s'écrit en chiffres, lettres, « . », « - » ou « _ », de 1 à 50 caractères (guide de l'API DSN, depot.idflux).",
                socle::fichiers::repr(texte)
            )
        })
    }

    /// Le flux tel qu'ecrit.
    pub fn texte(&self) -> &str {
        &self.0
    }

    /// Le flux d'un depot que le simulateur a joue (`branchement::PREFIXE_FLUX_FICTIF`, et l'ancien `DEMO-`).
    pub fn est_fictif(&self) -> bool {
        self.0.starts_with(crate::branchement::PREFIXE_FLUX_FICTIF) || self.0.starts_with(crate::branchement::ANCIEN_PREFIXE_FLUX_FICTIF)
    }
}

impl std::fmt::Display for IdDeFlux {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for IdDeFlux {
    type Error = Echec;
    fn try_from(s: String) -> R<IdDeFlux> {
        IdDeFlux::lire(&s)
    }
}

impl From<IdDeFlux> for String {
    fn from(f: IdDeFlux) -> String {
        f.0
    }
}

/// LE VERDICT D'UNE PIECE : ce qu'un bilan dit d'un envoi ou d'une declaration, ou un compte rendu de ce qu'il a
/// analyse. OK, KO, ANO : `envoi_etat_type` et `etat_type` du schema des bilans (l. 722-770), `ok_ko` pour
/// `etat_conformite` (l. 716-721, OK et KO ;
/// `atelier/reference/sources/gip-mds-api-dsn/net-entreprises-dsn-bilans-v02r01-20230703.xsd`) ; RO.B003.R002 d'un
/// NEOReS, OK et ANO (cahier technique NEOReS 2026.1.2, l. 1826-1829,
/// `atelier/reference/sources/neores/gip-mds-neores-cahier-technique-2026.1.2-20251024.txt`) ; le certificat d'une
/// DPAE n'en dit que OK ou KO (`etats::verdict_du_certificat`). Il se lit sans casse, a sa seule porte (`Verdict::lu`) :
/// la documentation du schema l'ecrit « ok: déclaration conforme […], ko: déclaration rejetée » (l. 201). Il s'ecrit en
/// capitales.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Verdict {
    Ok,
    Ko,
    Ano,
}

impl Verdict {
    pub const TOUS: &'static [Verdict] = &[Verdict::Ok, Verdict::Ko, Verdict::Ano];

    /// Son code, en capitales : le seul endroit ou il s'ecrit.
    pub fn code(self) -> &'static str {
        match self {
            Verdict::Ok => "OK",
            Verdict::Ko => "KO",
            Verdict::Ano => "ANO",
        }
    }

    /// Le verdict que ce texte ecrit, sans casse ; un autre texte n'en est pas un.
    pub fn lu(texte: &str) -> Option<Verdict> {
        Verdict::TOUS.iter().copied().find(|v| v.code().eq_ignore_ascii_case(texte))
    }

    /// Le verdict ecrit `texte` ; refus s'il n'en est pas un.
    pub fn lire(texte: &str) -> R<Verdict> {
        Verdict::lu(texte).ok_or_else(|| socle::refus!("Verdict {} : il s'écrit OK, KO ou ANO (schéma des bilans, etat_type).", socle::fichiers::repr(texte)))
    }

    /// Ce qu'il dit d'une declaration.
    pub fn en_mots(self) -> &'static str {
        match self {
            Verdict::Ok => "conforme",
            Verdict::Ko => "refusée",
            Verdict::Ano => "acceptée avec des anomalies",
        }
    }

    /// Ce qu'il dit d'un envoi.
    pub fn de_l_envoi(self) -> &'static str {
        match self {
            Verdict::Ok => "accepté",
            Verdict::Ko => "refusé",
            Verdict::Ano => "accepté avec des anomalies",
        }
    }

    /// Sa pastille : OK fait, KO refuse, ANO a corriger (la compagnie le peut).
    pub fn a_l_ecran(self) -> socle::ecran::EtatDEcran {
        match self {
            Verdict::Ok => socle::ecran::EtatDEcran::Fait,
            Verdict::Ko => socle::ecran::EtatDEcran::Refuse,
            Verdict::Ano => socle::ecran::EtatDEcran::AFaire,
        }
    }
}

impl std::fmt::Display for Verdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code())
    }
}

impl Serialize for Verdict {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.code())
    }
}

impl<'de> Deserialize<'de> for Verdict {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let texte = <String as Deserialize>::deserialize(d)?;
        Verdict::lire(&texte).map_err(|e| serde::de::Error::custom(e.phrase))
    }
}

/// LE COMPTE RENDU D'UN TEST DE DEPOT, ecrit en minuscules (`ok`, `ko`) : ce que le journal des envois garde d'un test qui
/// a ouvert un depot reel (`ligne::Ouverture`), et ce que dit ou en est le test d'un fichier (`etats::Repetition`). Il se
/// relit dans cette forme seule. `attendu` : un test conforme dont le bilan n'est pas encore revenu (aucun verdict), et
/// `null` aucun test conforme.
pub mod compte_rendu_ecrit {
    use super::Verdict;
    use serde::{Deserialize, Serialize};

    fn en_minuscules(v: Verdict) -> String {
        v.code().to_lowercase()
    }

    fn lu<E: serde::de::Error>(texte: &str) -> Result<Verdict, E> {
        match Verdict::TOUS.iter().copied().find(|v| en_minuscules(*v) == texte) {
            Some(v) => Ok(v),
            None => Err(E::custom(format!("compte rendu {} : il s'écrit ok, ko ou ano.", socle::fichiers::repr(texte)))),
        }
    }

    /// Un verdict qui peut manquer : `None` ne s'ecrit pas (`skip_serializing_if`).
    pub mod option {
        use super::*;

        pub fn serialize<S: serde::Serializer>(v: &Option<Verdict>, s: S) -> Result<S::Ok, S::Error> {
            v.map(en_minuscules).serialize(s)
        }

        pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Verdict>, D::Error> {
            match Option::<String>::deserialize(d)? {
                Some(t) => lu(&t).map(Some),
                None => Ok(None),
            }
        }
    }

    /// Le compte rendu du test conforme d'un fichier : aucun test conforme (`None`, `null`), son bilan attendu
    /// (`Some(None)`, `attendu`), ou son verdict.
    pub mod du_test {
        use super::*;

        const ATTENDU: &str = "attendu";

        pub fn serialize<S: serde::Serializer>(v: &Option<Option<Verdict>>, s: S) -> Result<S::Ok, S::Error> {
            match v {
                None => s.serialize_none(),
                Some(None) => s.serialize_str(ATTENDU),
                Some(Some(v)) => s.serialize_str(&en_minuscules(*v)),
            }
        }

        pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Option<Verdict>>, D::Error> {
            match Option::<String>::deserialize(d)? {
                None => Ok(None),
                Some(t) if t == ATTENDU => Ok(Some(None)),
                Some(t) => lu(&t).map(|v| Some(Some(v))),
            }
        }
    }
}

droit::codes_fermes! {
    /// LE STATUT QU'UNE LISTE DES RETOURS DONNE A UN RETOUR (`flux.retour.statut`) : « OK », « KO », « ANO », « EN COURS »
    /// (avec une espace : guide de l'API DSN v6.8, l. 87 et 1733-1745,
    /// `atelier/reference/sources/gip-mds-api-dsn/gip-mds-guide-implementation-api-dsn-v6.8-20260706.txt`) ; et
    /// « ANOBLOQUANTE », que France Travail donne a ses comptes rendus 42, 44 et 45 (« Pôle emploi est contraint de
    /// transmettre un code statut à « ANOBLOQUANTE » même si […] les balises « envoi_etat » et « etat » de la déclaration
    /// sont OK » : « De la DSN à l'attestation employeur » v3.2, l. 235-243 et 390-404,
    /// `atelier/reference/sources/net-entreprises-base-connaissances/net-entreprises-de-la-dsn-a-l-attestation-employeur-v3-2-20230328.txt`).
    StatutDuRetour, "Statut du retour", texte "guide de l'API DSN v6.8, l. 1733-1745, et « De la DSN à l'attestation employeur » v3.2, l. 235-243" {
        Ok = "OK",
        Ko = "KO",
        Ano = "ANO",
        EnCours = "EN COURS",
        AnoBloquante = "ANOBLOQUANTE",
    }
}

impl StatutDuRetour {
    /// Ce que le guide en dit (l. 1739-1745) ; « ANOBLOQUANTE » n'y est pas.
    pub fn phrase(self) -> Option<&'static str> {
        match self {
            StatutDuRetour::Ok => Some("le traitement s'est terminé normalement"),
            StatutDuRetour::Ko => Some("au moins une anomalie bloquante a empêché la poursuite du traitement"),
            StatutDuRetour::Ano => Some("des anomalies non bloquantes sont survenues, le traitement est allé à son terme"),
            StatutDuRetour::EnCours => Some("la déclaration est en cours de distribution vers l'organisme"),
            StatutDuRetour::AnoBloquante => None,
        }
    }

    /// Le statut qui ecrit ce verdict (meme code).
    pub fn du_verdict(v: Verdict) -> StatutDuRetour {
        match v {
            Verdict::Ok => StatutDuRetour::Ok,
            Verdict::Ko => StatutDuRetour::Ko,
            Verdict::Ano => StatutDuRetour::Ano,
        }
    }

    /// Le verdict qu'il ecrit, quand il en ecrit un (OK, KO, ANO) ; « EN COURS » et « ANOBLOQUANTE » n'en sont pas.
    pub fn verdict(self) -> Option<Verdict> {
        match self {
            StatutDuRetour::Ok => Some(Verdict::Ok),
            StatutDuRetour::Ko => Some(Verdict::Ko),
            StatutDuRetour::Ano => Some(Verdict::Ano),
            StatutDuRetour::EnCours | StatutDuRetour::AnoBloquante => None,
        }
    }
}

droit::codes_fermes! {
    /// LE TYPE D'UN RAPPORT DE DEPOT (`rapport/@type`), celui que la plate-forme ecrit dans les pieces recues, et que le
    /// guide de l'API DSN v6.8 donne a ses exemples : « AEE : exemple_retour_10_AEE.xml », « Certificat de conformité :
    /// exemple_retour_11_CCO.xml », « Bilan d’anomalies : exemple_retour_11_BAN.xml » (l. 4825-4828 ; « CCO / BAN », nature
    /// 11, l. 5691 ; `atelier/reference/sources/gip-mds-api-dsn/gip-mds-guide-implementation-api-dsn-v6.8-20260706.txt`).
    /// Un rapport d'un autre type n'est pas le bilan d'un depot (`bilan_d_octets`).
    TypeDeRapport, "Type de rapport", texte "guide de l'API DSN v6.8, l. 4825-4828" {
        Aee = "AEE",
        Cco = "CCO",
        Ban = "BAN",
    }
}

impl TypeDeRapport {
    /// Son libelle (le « Modele conceptuel » de la specification du schema des bilans VT2.2,
    /// `gip-mds-specifications-schema-bilans-vt2.2-20201118.txt`).
    pub fn libelle(self) -> &'static str {
        match self {
            TypeDeRapport::Aee => "Accusé d'enregistrement électronique",
            TypeDeRapport::Cco => "Certificat de conformité",
            TypeDeRapport::Ban => "Bilan d'anomalies",
        }
    }
}

droit::codes_fermes! {
    /// LA CATEGORIE D'UNE ANOMALIE D'UN BILAN (`categorie`, `categorie_anomalie_type` du schema des bilans, l. 839-855,
    /// `atelier/reference/sources/gip-mds-api-dsn/net-entreprises-dsn-bilans-v02r01-20230703.xsd`) : bloquante ou non.
    CategorieDAnomalie, "Catégorie d'anomalie", texte "schéma des bilans v02r01, categorie_anomalie_type, l. 839-855" {
        Bloquant = "bloquant",
        NonBloquant = "non-bloquant",
    }
}


/// Les rubriques DSN du taux de PAS, et ce qu'elles portent.
const RUBRIQUES_PAS: [(&str, ChampPas); 3] = [
    ("S21.G00.50.006", ChampPas::Taux),
    ("S21.G00.50.007", ChampPas::Type),
    ("S21.G00.50.008", ChampPas::Identifiant),
];

/// Noms metier NEODeS des memes rubriques (cahier technique 2026.1, p. 244).
const NOMS_METIER: [(&str, ChampPas); 3] = [("tauxpas", ChampPas::Taux), ("typetauxpas", ChampPas::Type), ("idtauxpas", ChampPas::Identifiant)];

/// Ce qu'une rubrique du bloc 50 donne d'un taux de PAS (S21.G00.50.006 a .008).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChampPas {
    Taux,
    Type,
    Identifiant,
}

/// L'archive `nom` (`AAAAMMJJ-hhmmss_natureN_flux.ext`, `releve_retours::archiver_retour`) est-elle celle d'un retour de nature
/// `nature` ? Son deuxieme segment le dit.
pub fn archive_de_nature(nom: &str, nature: NatureDuRetour) -> bool {
    nom.split('_').nth(1).and_then(|s| s.strip_prefix("nature")) == Some(nature.code())
}

// ------------------------------------------------------ le texte

/// Le texte sans ses blancs de tête et de queue.
fn nettoye(s: &str) -> &str {
    s.trim_matches(blanc)
}

/// Un chiffre decimal Unicode (le `\d` d'une expression reguliere sur du texte).
fn est_chiffre(c: char) -> bool {
    dans(DECIMAUX, c)
}

/// `$` accepte un saut de
/// ligne final.
fn est_nir(s: &str) -> bool {
    let t = sans_queue(s, '\n');
    let n = t.chars().count();
    (n == 13 || n == 15) && t.starts_with(['1', '2']) && t.chars().all(|c| c.is_ascii_digit())
}

/// Le NIR qu'une valeur de la piece porte (`est_nir`), sans sa cle : ses 13 premiers chiffres (S21.G00.30.001) ; une
/// valeur qui n'est pas un NIR n'en porte aucun.
fn nir_porte(s: &str) -> R<Option<Nir>> {
    est_nir(s).then(|| Nir::lire(&s.chars().take(13).collect::<String>())).transpose()
}


// ------------------------------------------------------------------ le XML

/// Un element, comme ElementTree le range.
#[derive(Debug)]
struct Noeud {
    /// `{uri}nom`, ou `nom` sans espace de noms.
    tag: String,
    attributs: Vec<(String, String)>,
    /// `.text` : le texte d'avant le premier enfant.
    texte: String,
    enfants: Vec<usize>,
}

/// L'arbre ; `n` est dans l'ordre du document, la racine d'abord (`.iter()`).
struct Arbre {
    n: Vec<Noeud>,
}

fn local(tag: &str) -> &str {
    match tag.rsplit_once('}') {
        Some((_, nom)) => nom,
        None => tag,
    }
}

fn nom_etendu(ns: Option<&str>, nom: &str) -> String {
    match ns {
        Some(u) if !u.is_empty() => format!("{{{}}}{}", u, nom),
        _ => nom.to_string(),
    }
}

impl Arbre {
    fn racine(&self) -> &Noeud {
        &self.n[0]
    }

    fn enfants<'a>(&'a self, e: &'a Noeud) -> impl Iterator<Item = &'a Noeud> {
        e.enfants.iter().map(move |&i| &self.n[i])
    }

    /// `Element.iter()` : l'element, puis ses descendants dans l'ordre du document.
    fn sous<'a>(&'a self, e: &'a Noeud) -> impl Iterator<Item = &'a Noeud> {
        let mut pile = vec![e];
        let mut sortie = Vec::new();
        while let Some(x) = pile.pop() {
            sortie.push(x);
            for &i in x.enfants.iter().rev() {
                pile.push(&self.n[i]);
            }
        }
        sortie.into_iter()
    }

    fn tous(&self) -> impl Iterator<Item = &Noeud> {
        self.n.iter()
    }
}

impl Noeud {
    /// `Element.get(nom)`.
    fn attribut(&self, nom: &str) -> Option<&str> {
        self.attributs.iter().find(|(k, _)| k == nom).map(|(_, v)| v.as_str())
    }

    /// `(e.text or "").strip()`.
    fn texte_net(&self) -> &str {
        nettoye(&self.texte)
    }
}

fn ajouter(a: &mut Arbre, n: roxmltree::Node) -> usize {
    let idx = a.n.len();
    let mut texte = String::new();
    for c in n.children() {
        if c.is_element() {
            break;
        }
        if let Some(t) = c.text().filter(|_| c.is_text()) {
            texte.push_str(t);
        }
    }
    a.n.push(Noeud {
        tag: nom_etendu(n.tag_name().namespace(), n.tag_name().name()),
        attributs: n
            .attributes()
            .map(|at| (nom_etendu(at.namespace(), at.name()), at.value().to_string()))
            .collect(),
        texte,
        enfants: Vec::new(),
    });
    let mut enfants = Vec::new();
    for c in n.children().filter(|c| c.is_element()) {
        enfants.push(ajouter(a, c));
    }
    a.n[idx].enfants = enfants;
    idx
}

fn dans(table: &[(u32, u32)], c: char) -> bool {
    socle::texte::dans_les_plages(table, c)
}

/// Un nom d'element, d'attribut ou d'instruction tel qu'expat le lit (XML 1.0,
/// quatrieme edition ; au plus un deux-points, ni en tete ni en queue).
fn nom_valide(n: &[char]) -> bool {
    // Un nom vide n'en est pas un.
    n.first().zip(n.last()).is_some_and(|(&premier, &dernier)| {
        premier != ':' && dernier != ':' && n.iter().filter(|&&c| c == ':').count() <= 1 && dans(NOM_DEBUT, premier) && n[1..].iter().all(|&c| c == ':' || dans(NOM_SUITE, c))
    })
}

fn commence_par(c: &[char], i: usize, motif: &str) -> bool {
    motif.chars().enumerate().all(|(k, m)| c.get(i + k) == Some(&m))
}

fn chercher(c: &[char], depuis: usize, motif: &str) -> Option<usize> {
    (depuis..c.len()).find(|&i| commence_par(c, i, motif))
}

/// L'indice qui suit `motif` (de `saut` caracteres) cherche depuis `depuis` ; la fin du texte s'il n'y est pas (ce qui
/// ne se ferme pas court jusqu'au bout, comme expat le lit).
fn apres(c: &[char], depuis: usize, motif: &str, saut: usize) -> usize {
    match chercher(c, depuis, motif) {
        Some(k) => k + saut,
        None => c.len(),
    }
}

/// Le texte `s` sans `fin` en queue, s'il l'a.
fn sans_queue(s: &str, fin: char) -> &str {
    match s.strip_suffix(fin) {
        Some(t) => t,
        None => s,
    }
}

fn blanc_xml(c: Option<&char>) -> bool {
    matches!(c, Some(' ' | '\t' | '\r' | '\n'))
}

fn mal_forme(m: &str) -> Echec {
    echec(Classe::XmlIllisible, m)
}

/// La declaration `<?xml ...?>` comme expat la verifie : `version` d'abord, puis
/// `encoding` (qui commence par une lettre), puis `standalone` (yes ou no) ; chaque
/// valeur n'a que des lettres, chiffres, point, tiret et soulignement.
fn declaration_valide(d: &[char]) -> bool {
    // (nom, valeur, suite) ; None : plus rien a lire.
    fn pseudo(d: &[char], mut i: usize) -> Option<Option<(String, String, usize)>> {
        if i == d.len() {
            return Some(None);
        }
        if !blanc_xml(d.get(i)) {
            return None;
        }
        while blanc_xml(d.get(i)) {
            i += 1;
        }
        if i == d.len() {
            return Some(None);
        }
        let debut = i;
        loop {
            match d.get(i) {
                None => return None,
                Some('=') => break,
                Some(c) if blanc_xml(Some(c)) => {
                    while blanc_xml(d.get(i)) {
                        i += 1;
                    }
                    if d.get(i) != Some(&'=') {
                        return None;
                    }
                    break;
                }
                _ => i += 1,
            }
        }
        let nom_fin = match d[debut..i].iter().position(|&c| c == '=' || blanc_xml(Some(&c))) {
            Some(p) => debut + p,
            None => i,
        };
        if i == debut {
            return None;
        }
        let nom: String = d[debut..nom_fin].iter().collect();
        i += 1;
        while blanc_xml(d.get(i)) {
            i += 1;
        }
        let guill = match d.get(i) {
            Some(&q @ ('"' | '\'')) => q,
            _ => return None,
        };
        i += 1;
        let v = i;
        loop {
            match d.get(i) {
                Some(&c) if c == guill => break,
                Some(c) if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') => i += 1,
                _ => return None,
            }
        }
        Some(Some((nom, d[v..i].iter().collect(), i + 1)))
    }
    // La declaration lue jusqu'au bout ; `None` : elle ne suit pas la grammaire.
    fn lue(d: &[char]) -> Option<()> {
        let (nom, _, mut i) = pseudo(d, 0)??;
        (nom == "version").then_some(())?;
        let Some((mut nom, mut val, mut j)) = pseudo(d, i)? else {
            return Some(());
        };
        if nom == "encoding" {
            val.chars().next().filter(|c| c.is_ascii_alphabetic())?;
            let Some(x) = pseudo(d, j)? else {
                return Some(());
            };
            (nom, val, j) = x;
        }
        i = j;
        (nom == "standalone" && (val == "yes" || val == "no")).then_some(())?;
        while blanc_xml(d.get(i)) {
            i += 1;
        }
        (i == d.len()).then_some(())
    }
    lue(d).is_some()
}

/// La declaration `<?xml ...?>` de tete, si le texte en commence par une.
fn controle_declaration(c: &[char]) -> R<()> {
    if commence_par(c, 0, "<?xml") && blanc_xml(c.get(5)) {
        if let Some(fin) = chercher(c, 5, "?>") {
            if !declaration_valide(&c[5..fin]) {
                return Err(mal_forme("XML mal forme : la declaration <?xml ?>"));
            }
        }
    }
    Ok(())
}

/// Ce qu'expat refuse et que `roxmltree` laisse passer : la declaration mal
/// ecrite, un nom hors de XML 1.0 quatrieme edition, une reference a un
/// caractere impossible (`&#xD800;`, `&#x110000;`).
fn controle_expat(s: &str) -> R<()> {
    let c: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut pile: Vec<String> = Vec::new();
    controle_declaration(&c)?;
    // Les prefixes declares quelque part : `p:xmlns` n'est lisible que si `p` l'est.
    let declares: std::collections::HashSet<String> = {
        let t: String = c.iter().collect();
        t.match_indices("xmlns:")
            .map(|(k, _)| t[k + 6..].chars().take_while(|x| !blanc_xml(Some(x)) && !matches!(x, '=' | '>' | '/')).collect())
            .collect()
    };
    let vide_apres = |c: &[char], fin: usize| -> bool {
        let mut k = fin;
        while blanc_xml(c.get(k)) {
            k += 1;
        }
        if c.get(k) != Some(&'=') {
            return false;
        }
        k += 1;
        while blanc_xml(c.get(k)) {
            k += 1;
        }
        matches!((c.get(k), c.get(k + 1)), (Some('"'), Some('"')) | (Some('\''), Some('\'')))
    };
    let nom_a = |i: usize, c: &[char], fin: &dyn Fn(char) -> bool| -> usize {
        match (i..c.len()).find(|&k| fin(c[k])) {
            Some(k) => k,
            None => c.len(),
        }
    };
    let reference = |c: &[char], i: usize| -> R<()> {
        // `&#` en c[i..] : chiffres decimaux, ou `x` et chiffres hexadecimaux, puis `;`.
        let (base, mut k) = if c.get(i + 2) == Some(&'x') { (16, i + 3) } else { (10, i + 2) };
        let debut = k;
        let mut v: u64 = 0;
        while let Some(d) = c.get(k).and_then(|x| x.to_digit(base)).filter(|_| c[k].is_ascii_hexdigit()) {
            v = (v * base as u64 + d as u64).min(0x1FFFFF);
            k += 1;
        }
        if k > debut && c.get(k) == Some(&';') && ((0xD800..0xE000).contains(&v) || v > 0x10FFFF) {
            return Err(mal_forme("XML mal forme : un numero de caractere invalide"));
        }
        Ok(())
    };
    while i < c.len() {
        if c[i] == '&' && c.get(i + 1) == Some(&'#') {
            reference(&c, i)?;
            i += 2;
            continue;
        }
        if c[i] != '<' {
            i += 1;
            continue;
        }
        if commence_par(&c, i, "<!--") {
            i = apres(&c, i + 4, "-->", 3);
        } else if commence_par(&c, i, "<![CDATA[") {
            i = apres(&c, i + 9, "]]>", 3);
        } else if commence_par(&c, i, "<?") {
            let fin = nom_a(i + 2, &c, &|x| blanc_xml(Some(&x)) || x == '?');
            let cible = &c[i + 2..fin];
            let est_declaration = i == 0 && cible == ['x', 'm', 'l'] && blanc_xml(c.get(fin));
            let xml = cible.len() == 3 && cible.iter().collect::<String>().eq_ignore_ascii_case("xml");
            if (!nom_valide(cible) || cible.contains(&':') || (xml && !est_declaration)) || !(blanc_xml(c.get(fin)) || commence_par(&c, fin, "?>")) {
                return Err(mal_forme("XML mal forme : un caractere inattendu"));
            }
            i = apres(&c, fin, "?>", 2);
        } else if commence_par(&c, i, "<!") {
            // DOCTYPE : son nom se verifie, on saute le reste, crochets et guillemets compris.
            let (mut profondeur, mut guill) = (0i32, None);
            i += 2;
            if commence_par(&c, i, "DOCTYPE") {
                let mut k = i + 7;
                while blanc_xml(c.get(k)) {
                    k += 1;
                }
                let fin = nom_a(k, &c, &|x| blanc_xml(Some(&x)) || matches!(x, '[' | '>'));
                if !nom_valide(&c[k..fin]) {
                    return Err(mal_forme("XML mal forme : un caractere inattendu"));
                }
            }
            while i < c.len() {
                if c[i] == '&' && c.get(i + 1) == Some(&'#') {
                    reference(&c, i)?;
                }
                match (c[i], guill) {
                    (q, Some(g)) if q == g => guill = None,
                    (_, Some(_)) => {}
                    ('"' | '\'', None) => guill = Some(c[i]),
                    ('[', None) => profondeur += 1,
                    (']', None) => profondeur -= 1,
                    ('<', None) if commence_par(&c, i, "<!--") => {
                        i = apres(&c, i + 4, "-->", 2);
                    }
                    ('>', None) if profondeur <= 0 => {
                        i += 1;
                        break;
                    }
                    _ => {}
                }
                i += 1;
            }
        } else if commence_par(&c, i, "</") {
            let fin = nom_a(i + 2, &c, &|x| blanc_xml(Some(&x)) || x == '>');
            let nom: String = c[i + 2..fin].iter().collect();
            if pile.pop().is_some_and(|ouvert| ouvert != nom) {
                return Err(mal_forme("XML mal forme : une balise fermee sans etre ouverte"));
            }
            i = apres(&c, i + 2, ">", 1);
        } else {
            let fin = nom_a(i + 1, &c, &|x| blanc_xml(Some(&x)) || x == '/' || x == '>');
            if fin > i + 1 && !nom_valide(&c[i + 1..fin]) {
                return Err(mal_forme("XML mal forme : un caractere inattendu"));
            }
            let nom_balise: String = c[i + 1..fin].iter().collect();
            i = fin;
            // les attributs
            loop {
                while blanc_xml(c.get(i)) {
                    i += 1;
                }
                match c.get(i) {
                    None => break,
                    Some('>') => {
                        i += 1;
                        break;
                    }
                    Some('/') => i += 1,
                    Some(_) => {
                        let fin = nom_a(i, &c, &|x| blanc_xml(Some(&x)) || matches!(x, '=' | '>' | '/'));
                        if fin == i {
                            break;
                        }
                        if !nom_valide(&c[i..fin]) {
                            return Err(mal_forme("XML mal forme : un caractere inattendu"));
                        }
                        let nom_attr: String = c[i..fin].iter().collect();
                        if let Some((prefixe, local)) = nom_attr.split_once(':') {
                            if (prefixe == "xmlns" && local == "xmlns") || (local == "xmlns" && prefixe != "xmlns" && !declares.contains(prefixe) && prefixe != "xml") {
                                return Err(mal_forme("XML mal forme : un prefixe sans espace de noms"));
                            }
                            if prefixe == "xmlns" && vide_apres(&c, fin) {
                                return Err(mal_forme("XML mal forme : un prefixe retire"));
                            }
                        }
                        i = fin;
                        while blanc_xml(c.get(i)) {
                            i += 1;
                        }
                        if c.get(i) != Some(&'=') {
                            break;
                        }
                        i += 1;
                        while blanc_xml(c.get(i)) {
                            i += 1;
                        }
                        let Some(&q @ ('"' | '\'')) = c.get(i) else {
                            break;
                        };
                        i += 1;
                        while i < c.len() && c[i] != q {
                            if c[i] == '&' && c.get(i + 1) == Some(&'#') {
                                reference(&c, i)?;
                            }
                            i += 1;
                        }
                        i += 1;
                    }
                }
            }
            if i >= 2 && c.get(i - 1) == Some(&'>') && c[i - 2] != '/' {
                pile.push(nom_balise);
            }
        }
    }
    Ok(())
}

/// Un XML déjà en texte : sa déclaration d'encodage est sans effet.
fn lire_texte(s: &str) -> R<Arbre> {
    match s.strip_prefix('\u{feff}') {
        Some(t) => lire_texte_nu(t),
        None => lire_texte_nu(s),
    }
}

/// Le texte sans son BOM : un second est un caractere de tete que XML refuse.
fn lire_texte_nu(s: &str) -> R<Arbre> {
    if s.starts_with('\u{feff}') {
        return Err(mal_forme("XML mal forme : un caractere inattendu"));
    }
    controle_expat(s)?;
    let options = roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() };
    let doc = roxmltree::Document::parse_with_options(s, options).map_err(|e| echec(Classe::XmlIllisible, e.to_string()))?;
    let mut a = Arbre { n: Vec::new() };
    ajouter(&mut a, doc.root_element());
    Ok(a)
}

/// L'encodage declare par `<?xml ... encoding="..."?>` en tete du texte.
fn encodage_declare(t: &str) -> Option<String> {
    if !t.starts_with("<?xml") {
        return None;
    }
    let fin = t.find("?>")?;
    let decl = &t[..fin];
    let i = decl.find("encoding")?;
    let reste = decl[i + 8..].trim_start();
    let reste = reste.strip_prefix('=')?.trim_start();
    let guill = reste.chars().next().filter(|c| *c == '"' || *c == '\'')?;
    let reste = &reste[1..];
    let j = reste.find(guill)?;
    Some(reste[..j].to_string())
}

/// `encodings.normalize_encoding`, en minuscules.
fn normaliser(nom: &str) -> String {
    let (mut out, mut ponct) = (String::new(), false);
    for c in nom.to_lowercase().chars() {
        if c.is_alphanumeric() || c == '.' {
            if ponct && !out.is_empty() {
                out.push('_');
            }
            if c.is_ascii() {
                out.push(c);
            }
            ponct = false;
        } else {
            ponct = true;
        }
    }
    out
}

/// Ce que la lecture XML fait d'un encodage déclaré (les classes de `tables_retours`).
#[derive(PartialEq)]
enum Codage {
    Utf8,
    Latin1,
    Ascii,
    Cp1252,
    /// `UTF-16` (`None`), `UTF-16LE` (`Some(true)`), `UTF-16BE` (`Some(false)`).
    Utf16(Option<bool>),
    /// Un nom d'encodage connu mais qui n'est pas sur 256 caracteres (multi-octets) : refuse
    /// (`Illisible`).
    Refuse,
    /// Une autre table sur 256 caracteres, que ce moteur ne porte pas : `XmlIllisible`.
    Autre,
    /// Un nom d'encodage inconnu : `LookupError`.
    Introuvable,
}

fn codage(nom: &str) -> Codage {
    match nom.to_ascii_lowercase().as_str() {
        "utf-8" => return Codage::Utf8,
        "iso-8859-1" => return Codage::Latin1,
        "us-ascii" => return Codage::Ascii,
        "utf-16" => return Codage::Utf16(None),
        "utf-16le" => return Codage::Utf16(Some(true)),
        "utf-16be" => return Codage::Utf16(Some(false)),
        _ => {}
    }
    let n = normaliser(nom);
    let a = |t: &[&str]| t.contains(&n.as_str());
    if a(CODECS_LATIN1) {
        Codage::Latin1
    } else if a(CODECS_ASCII) {
        Codage::Ascii
    } else if a(CODECS_CP1252) {
        Codage::Cp1252
    } else if a(CODECS_REFUSES) {
        Codage::Refuse
    } else if a(CODECS_AUTRES) {
        Codage::Autre
    } else {
        Codage::Introuvable
    }
}

fn erreur_codage(c: &Codage, nom: &str) -> Echec {
    match c {
        Codage::Introuvable => echec(Classe::Illisible, format!("encodage inconnu : {}", nom)),
        Codage::Refuse => echec(Classe::Illisible, "multi-byte encodings are not supported"),
        _ => echec(Classe::XmlIllisible, "encodage inconnu"),
    }
}

/// `ET.fromstring(octets)` : l'encodage declare est celui qui decode.
fn lire_octets(o: &[u8]) -> R<Arbre> {
    let invalide = || echec(Classe::XmlIllisible, "XML mal forme : un caractere inattendu");
    let incorrect = || echec(Classe::XmlIllisible, "l'encodage declare par le XML n'est pas le sien");
    let (o, bom8) = match o.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        Some(r) => (r, true),
        None => (o, false),
    };
    // UTF-16, comme expat le devine : un BOM, sinon un premier octet nul (gros
    // boutiste) ou un deuxieme (petit boutiste).
    let utf16 = if bom8 || o.len() < 2 {
        None
    } else if o.starts_with(&[0xFF, 0xFE]) {
        Some((true, 2))
    } else if o.starts_with(&[0xFE, 0xFF]) {
        Some((false, 2))
    } else if o[0] == 0 {
        Some((false, 0))
    } else if o[1] == 0 {
        Some((true, 0))
    } else {
        None
    };
    if let Some((le, saut)) = utf16 {
        let unites: Vec<u16> = o[saut..]
            .chunks_exact(2)
            .map(|p| if le { u16::from_le_bytes([p[0], p[1]]) } else { u16::from_be_bytes([p[0], p[1]]) })
            .collect();
        let t = String::from_utf16_lossy(&unites);
        controle_declaration(&t.chars().take(512).collect::<Vec<_>>())?;
        if let Some(n) = encodage_declare(&t) {
            match codage(&n) {
                Codage::Utf16(None) => {}
                Codage::Utf16(Some(l)) if l == le => {}
                c @ (Codage::Introuvable | Codage::Refuse) => return Err(erreur_codage(&c, &n)),
                _ => return Err(incorrect()),
            }
        }
        if (o.len() - saut) % 2 != 0 {
            return Err(invalide());
        }
        let t = String::from_utf16(&unites).map_err(|_| invalide())?;
        return lire_texte_nu(&t);
    }
    // Les octets d'une declaration valide sont de l'ASCII : on la lit telle quelle.
    let debut = String::from_utf8_lossy(&o[..o.len().min(512)]).into_owned();
    controle_declaration(&debut.chars().collect::<Vec<_>>())?;
    let nom = encodage_declare(&debut);
    let c = match &nom {
        Some(n) => codage(n),
        None => Codage::Utf8,
    };
    let texte = match c {
        Codage::Utf8 => std::str::from_utf8(o).map_err(|_| invalide())?.to_string(),
        Codage::Latin1 => o.iter().map(|&b| b as char).collect(),
        Codage::Ascii => {
            if !o.is_ascii() {
                return Err(invalide());
            }
            String::from_utf8_lossy(o).into_owned()
        }
        Codage::Cp1252 => {
            let mut t = String::with_capacity(o.len());
            for &b in o {
                match if b < 0x80 { b as u32 } else { WINDOWS_1252[b as usize - 0x80] } {
                    0xFFFD => return Err(invalide()),
                    u => t.push(char::from_u32(u).ok_or_else(invalide)?),
                }
            }
            t
        }
        Codage::Utf16(_) => return Err(incorrect()),
        Codage::Refuse | Codage::Autre | Codage::Introuvable => {
            return Err(match &nom {
                Some(n) => erreur_codage(&c, n),
                None => echec(Classe::XmlIllisible, "encodage inconnu"),
            })
        }
    };
    lire_texte_nu(&texte)
}

// ------------------------------------------------------- les bilans de depot

/// UN BILAN DE DEPOT (un rapport AEE, CCO ou BAN), lu a plat : chaque champ est le texte du premier element de son nom
/// qui en porte un ; un champ que le rapport ne porte pas est vide (`None`). Le flux, lui, est exige : un rapport sans
/// `idflux` n'est le bilan d'aucun depot.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Bilan {
    pub flux: IdDeFlux,
    pub recu_le: Option<Reception>,
    pub essai_reel: Option<EssaiOuReel>,
    pub etat_envoi: Option<Verdict>,
    pub etat_declaration: Option<Verdict>,
    pub mois: Option<socle::mois::Mois>,
    pub logiciel: Option<String>,
    pub type_rapport: TypeDeRapport,
    pub type_declaration: Option<TypeDeLaDeclaration>,
    pub fraction: Option<Fraction>,
    pub ordre: Option<NumeroDOrdre>,
    pub salaries: Option<usize>,
    pub contrats: Option<usize>,
    pub conformite: Option<String>,
    pub certificat: Option<String>,
    pub siret_declarant: Option<Siret>,
}

impl Bilan {
    /// Ce que le bilan dit de la declaration (son etat), sinon de l'envoi.
    pub fn verdict(&self) -> Option<Verdict> {
        self.etat_declaration.or(self.etat_envoi)
    }

    /// Ce que `autre` dit et que celui-ci ne dit pas.
    fn completer(&mut self, autre: &Bilan) {
        fn prendre<T: Clone>(x: &mut Option<T>, y: &Option<T>) {
            if x.is_none() {
                x.clone_from(y);
            }
        }
        prendre(&mut self.recu_le, &autre.recu_le);
        prendre(&mut self.essai_reel, &autre.essai_reel);
        prendre(&mut self.etat_envoi, &autre.etat_envoi);
        prendre(&mut self.etat_declaration, &autre.etat_declaration);
        prendre(&mut self.mois, &autre.mois);
        prendre(&mut self.logiciel, &autre.logiciel);
        prendre(&mut self.type_declaration, &autre.type_declaration);
        prendre(&mut self.fraction, &autre.fraction);
        prendre(&mut self.ordre, &autre.ordre);
        prendre(&mut self.salaries, &autre.salaries);
        prendre(&mut self.contrats, &autre.contrats);
        prendre(&mut self.conformite, &autre.conformite);
        prendre(&mut self.certificat, &autre.certificat);
        prendre(&mut self.siret_declarant, &autre.siret_declarant);
    }
}

/// LA RECEPTION D'UN DEPOT, telle que son bilan la dit : `date_reception` (un `date` de XML Schema) et, s'il la porte,
/// `heure_reception` (un `time`) ; schema des bilans harmonises, l. 318-327
/// (`atelier/reference/sources/gip-mds-api-dsn/net-entreprises-dsn-bilans-v02r01-20230703.xsd`). Une heure sans date ne
/// date rien : le bilan n'a pas de reception. Elle s'ecrit comme elle est lue : `date` ou `dateTheure`, octet pour octet.
/// L'ordre : par jour ecrit, puis par texte ecrit.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Reception {
    /// Le jour de reception, tel qu'ecrit.
    jour: Jour,
    /// `date` ou `dateTheure`, tels qu'ecrits.
    ecrite: String,
    /// « 09:36 » : l'heure et la minute telles qu'ecrites, quand le bilan dit l'heure.
    hh_mm: Option<String>,
    /// L'instant ou le jour que le journal en garde (`au_journal`), calcule a la lecture.
    au_journal: Option<InstantOuJour>,
}

impl Reception {
    /// Le jour de reception, tel qu'ecrit.
    pub fn jour(&self) -> Jour {
        self.jour
    }

    /// L'INSTANT OU LE JOUR QUE LE JOURNAL EN GARDE (`InstantOuJour`) : le jour seul quand le bilan ne dit pas l'heure,
    /// l'instant a la seconde quand il la dit. Le schema ne dit pas le fuseau d'une heure qui n'en ecrit pas : c'est
    /// l'heure telle qu'elle est ecrite, tenue pour celle de Paris (une heure qui n'existe pas a Paris ne s'y ramene pas).
    /// Une heure qui ecrit son fuseau est un instant. `None` (la reprise ne sait pas dire QUAND, et ne la reprend pas)
    /// quand elle ne se ramene pas a la seconde de Paris sans rien couper ni supposer : une fraction de seconde non nulle ;
    /// une date qui porte un fuseau sans heure (le jour de Paris n'y est pas dit), ou un autre fuseau que celui de son
    /// heure.
    pub fn au_journal(&self) -> Option<InstantOuJour> {
        self.au_journal
    }

    /// Tel que l'ecran le dit : l'heure de Paris de ce qui est lu (« 03/09/2026 à 09:36 », ou « 03/09/2026 » sans heure) ;
    /// quand elle ne s'y ramene pas (`au_journal`), la date et l'heure telles qu'ecrites.
    pub fn dit(&self) -> String {
        match (self.au_journal, &self.hh_mm) {
            (Some(q), _) => q.dit(),
            (None, Some(h)) => format!("{} à {}", self.jour.jj_mm_aaaa(), h),
            (None, None) => self.jour.jj_mm_aaaa(),
        }
    }
}

impl Serialize for Reception {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.ecrite)
    }
}

/// L'instant ou le jour d'une date `(jour, fuseau)` et d'une heure `(secondes depuis minuit, fraction nulle, fuseau)` de
/// XML Schema (`Reception::au_journal`).
fn au_journal(date: (Jour, Option<FuseauXsd>), heure: Option<(i64, bool, Option<FuseauXsd>)>) -> Option<InstantOuJour> {
    let (jour, fuseau_date) = date;
    let Some((seconde, a_la_seconde, fuseau)) = heure else {
        return fuseau_date.is_none().then_some(InstantOuJour::Jour(jour));
    };
    if !a_la_seconde || fuseau_date.is_some_and(|z| fuseau.is_none_or(|y| y.secondes() != z.secondes())) {
        return None;
    }
    let local = (jour.ordinal() - 719_163) * 86_400 + seconde;
    let instant = match fuseau {
        None => {
            // `24:00:00` est le debut du jour suivant.
            let (jour, seconde) = (Jour::depuis_jours_unix(local.div_euclid(86_400))?, local.rem_euclid(86_400));
            Moment::a_paris(jour, seconde / 3_600, (seconde % 3_600) / 60, seconde % 60).filter(|m| m.jour_de_paris() == jour && m.seconde_du_jour_de_paris() == seconde)?
        }
        Some(z) => Moment::de_secondes(local - z.secondes())?,
    };
    Some(InstantOuJour::Instant(instant))
}

/// La reception que disent les champs `date_reception` et `heure_reception` du rapport (`get`) ; une date ou une heure
/// qui n'est pas de la forme du schema refuse le rapport.
fn reception_du_rapport(get: &dyn Fn(&str) -> Option<String>) -> R<Option<Reception>> {
    let Some(d) = get("date_reception") else { return Ok(None) };
    let illisible = |element: &str, quoi: &str, t: &str| {
        socle::echec::cle(format!("La {} de réception d'un bilan ne se lit pas : {} n'est pas une {} de XML Schema.", quoi, socle::date::repr_texte(t), quoi), element).reclasser(Classe::Illisible)
    };
    let date = Zone::lue(&d, format::DateXsd).valeur().ok_or_else(|| illisible("date_reception", "date", &d))?;
    let (heure, ecrite) = match get("heure_reception") {
        Some(h) => (Some(Zone::lue(&h, format::HeureXsd).valeur().ok_or_else(|| illisible("heure_reception", "heure", &h))?), format!("{}T{}", d, h)),
        None => (None, d.clone()),
    };
    let hh_mm = heure.map(|(s, _, _)| format!("{:02}:{:02}", s / 3_600, (s % 3_600) / 60));
    Ok(Some(Reception { jour: date.0, ecrite, hh_mm, au_journal: au_journal(date, heure) }))
}

/// Le mois d'une DSN (S20.G00.05.005, `01MMAAAA`) ; un mois qui ne se lit pas refuse le rapport.
fn mois_du_rapport(brut: &str) -> R<socle::mois::Mois> {
    Zone::lue(nettoye(brut), format::MoisDsn).valeur().map_err(|m| m.reclasser(Classe::Illisible))
}

fn bilan_depuis(champs: &[(String, String)], type_rapport: TypeDeRapport) -> R<Option<Bilan>> {
    let get = |k: &str| champs.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
    let Some(flux) = forme_du_retour(get("idflux").as_deref(), IdDeFlux::lire)? else { return Ok(None) };
    let recu_le = reception_du_rapport(&get)?;
    Ok(Some(Bilan {
        flux,
        recu_le,
        essai_reel: essai_du_retour(get("essai_reel").as_deref())?,
        etat_envoi: forme_du_retour(get("envoi_etat").as_deref(), Verdict::lire)?,
        etat_declaration: forme_du_retour(get("etat").or(get("etat_conformite")).as_deref(), Verdict::lire)?,
        mois: get("mois").map(|m| mois_du_rapport(&m)).transpose()?,
        logiciel: get("nom_logiciel"),
        type_rapport,
        type_declaration: type_du_retour(get("type").as_deref())?,
        fraction: forme_du_retour(get("fraction").as_deref(), Fraction::lire)?,
        ordre: forme_du_retour(get("ordre").as_deref(), NumeroDOrdre::lire)?,
        salaries: forme_du_retour(get("salaries").as_deref(), compte_du_retour)?,
        contrats: forme_du_retour(get("contrats").as_deref(), compte_du_retour)?,
        conformite: get("etat_conformite"),
        certificat: get("certificat_conformite"),
        siret_declarant: forme_du_retour(get("siret").as_deref(), Siret::lire)?,
    }))
}

/// LE BILAN QUE PORTENT LES OCTETS `o` : `None` quand ce n'est pas un rapport de depot (un XML d'une autre forme, ou
/// sans flux) ; un XML qui ne se lit pas est un refus (sa classe) : la reprise du journal le nomme (`illisibles`), le
/// rattachement des retours le tient pour une piece sans bilan (`bilan_si_xml`).
pub fn bilan_d_octets(o: &[u8]) -> R<Option<Bilan>> {
    let texte = String::from_utf8_lossy(o);
    let arbre = lire_texte(nettoye(&texte))?;
    let racine = arbre.racine();
    if local(&racine.tag) != "rapport" {
        return Ok(None);
    }
    let Some(type_rapport) = racine.attribut("type").and_then(TypeDeRapport::du_code) else {
        return Ok(None);
    };
    // Un balayage a plat, premiere occurrence gagnante.
    let mut champs: Vec<(String, String)> = Vec::new();
    for el in arbre.tous() {
        let nom = local(&el.tag);
        let t = el.texte_net();
        if !t.is_empty() && !champs.iter().any(|(n, _)| n == nom) {
            champs.push((nom.to_string(), t.to_string()));
        }
    }
    bilan_depuis(&champs, type_rapport)
}

/// `bilan_d_octets`, une piece qui n'est pas du XML lisible n'ayant pas de bilan (une archive, un PDF) ; tout autre
/// refus (un encodage inconnu, un mois illisible) remonte.
pub fn bilan_si_xml(o: &[u8]) -> R<Option<Bilan>> {
    match bilan_d_octets(o) {
        Err(Echec { classe: Some(Classe::XmlIllisible), .. }) => Ok(None),
        autre => autre,
    }
}

/// Les declarations qu'un certificat de conformite ou un bilan d'anomalies (nature 11, rapport `CCO` ou `BAN`) identifie,
/// une par `declaration` : `nature`, `type`, `ordre` (S20.G00.05.001, .002, .004), `siren`, `nic` (d'affectation),
/// `etat`, `essai_reel` (de l'envoi : 01 essai, 02 reel) et les `codes` de ses anomalies. C'est TOUT ce que la plate-forme
/// dit d'une declaration : ni salarie, ni contrat, ni date d'evenement (schema des bilans `v01r08`, ceux qu'elle rend ;
/// specification VT2.2, `atelier/reference/sources/gip-mds-api-dsn/`). Un XML illisible, ou qui
/// n'est pas un rapport de depot, n'en identifie aucune.
pub fn declarations_du_bilan(o: &[u8]) -> R<Vec<DeclarationIdentifiee>> {
    let mut out = Vec::new();
    // Un fichier qui n'est pas un XML, ou un rapport qui n'est pas celui d'un depot, n'identifie aucune declaration.
    let arbre = octets_arbre(o)?.filter(|a| local(&a.racine().tag) == "rapport" && a.racine().attribut("type").and_then(TypeDeRapport::du_code).is_some());
    if let Some(arbre) = &arbre {
        out.extend(declarations_de_l_arbre(arbre)?);
    }
    Ok(out)
}

fn declarations_de_l_arbre(arbre: &Arbre) -> R<Vec<DeclarationIdentifiee>> {
    let essai_reel = essai_du_retour(arbre.tous().find(|e| local(&e.tag) == "essai_reel").map(|e| e.texte_net()).filter(|t| !t.is_empty()))?;
    let mut out = Vec::new();
    for d in arbre.tous().filter(|e| local(&e.tag) == "declaration") {
        let (mut champs, mut codes): (Vec<(&str, &str)>, Vec<String>) = (Vec::new(), Vec::new());
        for x in arbre.sous(d) {
            let (nom, t) = (local(&x.tag), x.texte_net());
            if nom == "code" {
                codes.push(t.to_string());
            } else if !t.is_empty() && !champs.iter().any(|(n, _)| *n == nom) {
                champs.push((nom, t));
            }
        }
        let get = |k: &str| champs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string());
        out.push(DeclarationIdentifiee {
            nature_declaration: nature_du_retour(get("nature").as_deref())?,
            type_declaration: type_du_retour(get("type").as_deref())?,
            ordre: forme_du_retour(get("ordre").as_deref(), NumeroDOrdre::lire)?,
            siren: forme_du_retour(get("SIREN").as_deref(), Siren::lire)?,
            nic: forme_du_retour(get("nic_affectation").as_deref(), Nic::lire)?,
            etat: forme_du_retour(get("etat").as_deref(), Verdict::lire)?,
            essai_reel,
            codes,
        });
    }
    Ok(out)
}

/// UNE DECLARATION QU'UN BILAN IDENTIFIE (`declarations_du_bilan`) : ce que le bilan en dit, chaque champ vide quand il
/// ne le porte pas, et les codes de ses anomalies.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeclarationIdentifiee {
    #[serde(rename = "nature")]
    pub nature_declaration: Option<NatureDeLaDeclaration>,
    #[serde(rename = "type")]
    pub type_declaration: Option<TypeDeLaDeclaration>,
    pub ordre: Option<NumeroDOrdre>,
    pub siren: Option<Siren>,
    pub nic: Option<Nic>,
    pub etat: Option<Verdict>,
    pub essai_reel: Option<EssaiOuReel>,
    pub codes: Vec<String>,
}

/// La nature d'une declaration qu'un retour de la norme nomme (S20.G00.05.001), `None` s'il ne la dit pas : un code
/// hors de la liste de la norme refuse le retour, qui ne se lit pas.
fn nature_du_retour(code: Option<&str>) -> R<Option<NatureDeLaDeclaration>> {
    code_du_retour(code, "nature")
}

/// L'envoi d'essai ou reel qu'un retour dit (S10.G00.00.005, que reprennent `essai_reel` d'un bilan, RO.B000.R013 d'un
/// NEOReS et `CodeEnvoi` d'un compte rendu d'organisme complementaire), `None` s'il ne le dit pas : un code hors de la
/// liste de la norme refuse le retour, qui ne se lit pas.
fn essai_du_retour(code: Option<&str>) -> R<Option<EssaiOuReel>> {
    code_du_retour(code, "code d'envoi")
}

/// Le type d'une declaration qu'un retour de la norme nomme (S20.G00.05.002), `None` s'il ne le dit pas : un code hors
/// de la liste de la norme refuse le retour, qui ne se lit pas.
fn type_du_retour(code: Option<&str>) -> R<Option<TypeDeLaDeclaration>> {
    code_du_retour(code, "type")
}

/// Un compte qu'un retour porte : un `integer` du schema des bilans (`salaries`, `contrats`, l. 170-188,
/// `atelier/reference/sources/gip-mds-api-dsn/net-entreprises-dsn-bilans-v02r01-20230703.xsd`), un attribut entier du
/// compte rendu d'un organisme complementaire (`NbErreursSimilaires`, `NbreSal`, `NbreErreur`... :
/// `ctip-crm-dsn-oc-p3-v1.1.4-schema-20230915.xsd`, l. 190-199, 910-961) ; une autre forme se refuse.
fn compte_du_retour(texte: &str) -> R<usize> {
    texte.parse::<usize>().map_err(|_| socle::refus!("Un compte que le retour porte ne se lit pas : {} n'est pas un nombre entier positif.", socle::fichiers::repr(texte)))
}

/// Ce qu'un retour de la norme porte dans une forme de la norme (`lire` : une fraction, un numero d'ordre), `None` s'il
/// ne le porte pas : hors de sa forme, le retour ne se lit pas.
fn forme_du_retour<T>(texte: Option<&str>, lire: fn(&str) -> R<T>) -> R<Option<T>> {
    texte.map(|t| lire(t).map_err(|e| e.reclasser(Classe::Illisible))).transpose()
}

fn code_du_retour<T: ListeDeLaNorme>(code: Option<&str>, quoi: &str) -> R<Option<T>> {
    code.map(|c| {
        T::du_code_de_la_norme(c).ok_or_else(|| {
            socle::refus!("Le retour porte, en {} de la déclaration ({}), « {} », qui n'est pas un code de sa liste dans la norme : il ne se lit pas.", quoi, T::RUBRIQUE, c)
                .reclasser(Classe::Illisible)
        })
    })
    .transpose()
}

/// L'arbre d'octets ; un XML mal forme n'en est pas un (`None`), tout autre refus (un
/// encodage inconnu) remonte.
fn octets_arbre(o: &[u8]) -> R<Option<Arbre>> {
    match lire_octets(o) {
        Ok(a) => Ok(Some(a)),
        Err(Echec { classe: Some(Classe::XmlIllisible), .. }) => Ok(None),
        Err(autre) => Err(autre),
    }
}

/// `anomalies_du_bilan` : les motifs d'un BAN, en phrases lisibles.
pub fn anomalies_lisibles_d_octets(o: &[u8]) -> R<Vec<String>> {
    let mut out = Vec::new();
    // Un fichier qui n'est pas un XML n'en porte aucune.
    let arbre = octets_arbre(o)?;
    for (arbre, a) in arbre.iter().flat_map(|arbre| arbre.tous().map(move |a| (arbre, a))) {
        if local(&a.tag) != "declaration_anomalie" {
            continue;
        }
        let (mut message, mut categorie, mut nom) = ("", "", "");
        for x in arbre.sous(a) {
            match local(&x.tag) {
                "message" => message = x.texte_net(),
                "categorie" => categorie = x.texte_net(),
                "nom_famille" => nom = x.texte_net(),
                _ => {}
            }
        }
        if !message.is_empty() {
            let cat = match forme_du_retour(Some(categorie).filter(|c| !c.is_empty()), code_ferme::<CategorieDAnomalie>)? {
                Some(c) => format!(" ({})", c),
                None => String::new(),
            };
            let qui = if nom.is_empty() { String::new() } else { format!(" -- {}", nom) };
            out.push(format!("{}{}{}", message, cat, qui));
        }
    }
    Ok(out)
}

/// UNE ANOMALIE D'UN BILAN, en codes (`anomalies_codees_d_octets`) : chaque champ est le texte du premier element de son
/// nom sous l'anomalie (vide s'il n'en porte pas ; la categorie, absente), tel que le journal des incidents l'ecrit.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct AnomalieCodee {
    pub categorie: Option<CategorieDAnomalie>,
    pub code: String,
    pub message: String,
    pub rubrique: String,
}

/// Les anomalies d'un bilan, en codes.
pub fn anomalies_codees_d_octets(o: &[u8]) -> R<Vec<AnomalieCodee>> {
    let mut out = Vec::new();
    // Un fichier qui n'est pas un XML n'en porte aucune.
    let arbre = octets_arbre(o)?;
    for (arbre, a) in arbre.iter().flat_map(|arbre| arbre.tous().map(move |a| (arbre, a))) {
        if !matches!(local(&a.tag), "declaration_anomalie" | "envoi_anomalie") {
            continue;
        }
        let (mut code, mut categorie, mut message, mut rubrique) = (String::new(), None, String::new(), String::new());
        for x in arbre.sous(a) {
            let t = x.texte_net();
            match local(&x.tag) {
                "code" if code.is_empty() => code = t.to_string(),
                "categorie" if categorie.is_none() => categorie = forme_du_retour(Some(t).filter(|c| !c.is_empty()), code_ferme::<CategorieDAnomalie>)?,
                "message" if message.is_empty() => message = t.to_string(),
                "identifiant" if rubrique.is_empty() => rubrique = t.to_string(),
                _ => {}
            }
        }
        if !code.is_empty() || !message.is_empty() {
            out.push(AnomalieCodee { categorie, code, message, rubrique });
        }
    }
    Ok(out)
}

/// `fusionner_bilans` : un seul bilan par flux, le plus complet. Le bilan de
/// declaration (CCO, BAN) l'emporte sur l'accuse de transport (AEE).
pub fn fusionner_bilans(bilans: &[Bilan]) -> Option<Bilan> {
    // Le bilan de declaration d'abord, puis l'accuse de transport.
    let rang = |b: &Bilan| b.type_rapport == TypeDeRapport::Aee;
    let mut tries: Vec<&Bilan> = bilans.iter().collect();
    tries.sort_by_key(|b| rang(b));
    let (premier, autres) = tries.split_first()?;
    let mut base = (*premier).clone();
    for autre in autres {
        base.completer(autre);
    }
    Some(base)
}

// ------------------------------------------------ reponses de recherche (liste)

/// UN RETOUR QUE LA PLATE-FORME LISTE (guide de l'API DSN v6.8, §3.4.2, « Pour chaque flux et chaque retour » :
/// `atelier/reference/sources/gip-mds-api-dsn/gip-mds-guide-implementation-api-dsn-v6.8-20260706.txt`) : sa
/// publication, sa production, l'OPS et le delegataire, sa nature, son statut, son identifiant et l'adresse de sa piece ;
/// `libelle`, celui qu'un organisme joue de la compagnie fictive lui donne ; `flux`, l'identifiant du flux qui le porte.
/// Un champ que la liste ne donne pas est vide. La publication est `yyyyMMddhhmmss` [14,14] (guide v6.8, « Date de
/// publication du retour ») ; la DPAE liste ses retours par la meme interface (guide d'implementation de l'API DPAE :
/// « entièrement compatible avec le mode API disponible pour la DSN », `atelier/reference/norme-dpae/API-DPAE-Guide-Implementation-NAT5492.pdf`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Annonce {
    #[serde(default, skip_serializing_if = "Option::is_none", with = "socle::paris::ecrit::compacte::option")]
    pub publication: Option<Moment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub production: Option<String>,
    /// L'organisme du bilan (S21.G00.70.002 ou 15.002 de la declaration, « F [5,10] », guide v6.8 : `OPSINCONNU` pour un
    /// organisme non reference).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ops: Option<IdentifiantDOrganisme>,
    /// Le delegataire de gestion (S21.G00.70.003 ou 15.003, « F [5,9] »).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delegataire: Option<IdentifiantDOrganisme>,
    #[serde(rename = "nature")]
    pub nature_retour: NatureDuRetour,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statut: Option<StatutDuRetour>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Une adresse vide se lit absente, comme `<url/>` dans la liste XML.
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "socle::forme::vide_si_aucun::deserialize")]
    pub url: Option<Url>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub libelle: Option<String>,
    pub flux: Option<IdDeFlux>,
}

impl Annonce {
    /// Ce qui distingue un retour : son flux, sa nature, sa publication.
    pub fn cle(&self) -> (Option<&IdDeFlux>, NatureDuRetour, Option<Moment>) {
        (self.flux.as_ref(), self.nature_retour, self.publication)
    }
}

/// UN DEPOT QUE LA PLATE-FORME LISTE (guide v6.8, §3.3.2) : l'identifiant de son flux, sa date, son statut (OK, KO).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Depot {
    pub idflux: IdDeFlux,
    #[serde(with = "socle::paris::ecrit::compacte")]
    pub date: Moment,
    pub statut: String,
}

/// Un element seul (un objet) ou une liste d'elements : la reponse JSON de la plate-forme ecrit un objet quand il n'y
/// en a qu'un.
#[derive(Deserialize)]
#[serde(untagged)]
enum UnOuPlusieurs<T> {
    Un(T),
    Plusieurs(Vec<T>),
}

impl<T> UnOuPlusieurs<T> {
    /// Les elements ; rien (une cle absente ou `null`) n'en fait aucun.
    fn en_liste(u: Option<UnOuPlusieurs<T>>) -> Vec<T> {
        u.into_iter()
            .flat_map(|x| match x {
                UnOuPlusieurs::Un(x) => vec![x],
                UnOuPlusieurs::Plusieurs(l) => l,
            })
            .collect()
    }
}

/// La reponse JSON (2.0) de la liste des retours : `{"retours": {"flux": [{"id", "retour": [...]}]}}`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ListeJson {
    retours: Option<RetoursJson>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RetoursJson {
    flux: Option<UnOuPlusieurs<FluxJson>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FluxJson {
    /// Un id vide se lit absent, comme `<id/>` dans la liste XML.
    #[serde(default, deserialize_with = "socle::forme::vide_si_aucun::deserialize")]
    id: Option<IdDeFlux>,
    retour: Option<UnOuPlusieurs<RetourJson>>,
}

/// Un retour de la reponse JSON : l'annonce, sans son flux (celui du flux qui le porte).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RetourJson {
    #[serde(default, with = "socle::paris::ecrit::compacte::option")]
    publication: Option<Moment>,
    production: Option<String>,
    #[serde(default, deserialize_with = "organisme_ou_non_renseigne")]
    ops: Option<IdentifiantDOrganisme>,
    #[serde(default, deserialize_with = "organisme_ou_non_renseigne")]
    delegataire: Option<IdentifiantDOrganisme>,
    #[serde(rename = "nature")]
    nature_retour: NatureDuRetour,
    statut: Option<StatutDuRetour>,
    id: Option<String>,
    /// Une adresse vide se lit absente, comme `<url/>` dans la liste XML.
    #[serde(default, deserialize_with = "socle::forme::vide_si_aucun::deserialize")]
    url: Option<Url>,
}

/// L'OPS ou le delegataire d'un retour JSON : « Non renseigné s'il s'agit d'un retour par le SI DSN ou par un organisme
/// de base » (`atelier/reference/sources/gip-mds-api-dsn/gip-mds-guide-implementation-api-dsn-v6.8-20260706.txt`,
/// l. 1699-1705) ; non renseigne, c'est absent ou vide, comme dans la
/// reponse XML (un enfant vide n'y est pas lu).
fn organisme_ou_non_renseigne<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<IdentifiantDOrganisme>, D::Error> {
    match Option::<String>::deserialize(d)? {
        None => Ok(None),
        Some(t) if t.is_empty() => Ok(None),
        Some(t) => IdentifiantDOrganisme::lire(&t).map(Some).map_err(|e| serde::de::Error::custom(e.phrase)),
    }
}

/// La reponse JSON (2.0) de la liste des depots : `{"depots": {"declarant": {"siret", "nom", "prenom", "depot": [...]}}}`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DepotsJson {
    depots: Option<DeclarantJson>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeclarantJson {
    declarant: Option<DepotsDuDeclarant>,
}

/// Le declarant que la liste nomme : la releve ne lit que ses depots.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DepotsDuDeclarant {
    #[serde(default, deserialize_with = "socle::forme::non_lue")]
    siret: socle::forme::NonLue,
    #[serde(default, deserialize_with = "socle::forme::non_lue")]
    nom: socle::forme::NonLue,
    #[serde(default, deserialize_with = "socle::forme::non_lue")]
    prenom: socle::forme::NonLue,
    depot: Option<UnOuPlusieurs<Depot>>,
}

const LISTE_ILLISIBLE: &str = "La liste que la plate-forme rend ne se lit pas.";

/// La liste illisible, la cle qui ne va pas a part.
#[track_caller]
fn illisible(cle: &str) -> Echec {
    socle::echec::cle(LISTE_ILLISIBLE, cle).reclasser(Classe::Illisible)
}

/// L'identifiant d'organisme que la liste porte a `cle` ; hors de sa forme, la liste ne se lit pas (la phrase du type,
/// la cle a part).
fn organisme_liste(texte: &str, cle: &str) -> R<IdentifiantDOrganisme> {
    IdentifiantDOrganisme::lire(texte).map_err(|e| socle::echec::cle(e.phrase, cle).reclasser(Classe::Illisible))
}

/// Un code d'une liste fermee des retours (la nature, le statut, la categorie d'une anomalie...), lu par la forme de son
/// type : hors de sa liste, la phrase du type, celle de la liste JSON.
fn code_ferme<T: serde::de::DeserializeOwned>(texte: &str) -> R<T> {
    let lecteur: serde::de::value::StrDeserializer<serde::de::value::Error> = serde::de::IntoDeserializer::into_deserializer(texte);
    T::deserialize(lecteur).map_err(|e| socle::echec::refus(e.to_string()))
}

/// Un code d'une liste fermee que la liste XML porte a `cle` ; hors de sa liste, la liste ne se lit pas (la phrase du
/// type, celle de la liste JSON, la cle a part).
fn code_liste<T: serde::de::DeserializeOwned>(texte: &str, cle: &str) -> R<T> {
    code_ferme(texte).map_err(|e| socle::echec::cle(e.phrase, cle).reclasser(Classe::Illisible))
}

/// Le texte des enfants de `e`, par leur nom local (un enfant vide n'est pas dit).
fn enfants_textes<'a>(a: &'a Arbre, e: &'a Noeud) -> Vec<(&'a str, &'a str)> {
    a.enfants(e).map(|c| (local(&c.tag), c.texte_net())).filter(|(_, t)| !t.is_empty()).collect()
}

/// Les retours que la plate-forme liste : XML (1.0) ou JSON (2.0). Un champ que la plate-forme ne decrit pas refuse
/// la liste.
pub fn liste_retours_d_octets(o: &[u8]) -> R<Vec<Annonce>> {
    let texte = String::from_utf8_lossy(o);
    let texte = nettoye(&texte);
    if texte.starts_with('{') {
        let l: ListeJson = socle::forme::lire_texte(texte, LISTE_ILLISIBLE).map_err(|m| m.reclasser(Classe::Illisible))?;
        let mut out = Vec::new();
        for f in UnOuPlusieurs::en_liste(l.retours.and_then(|r| r.flux)) {
            for r in UnOuPlusieurs::en_liste(f.retour) {
                out.push(Annonce {
                    publication: r.publication,
                    production: r.production,
                    ops: r.ops,
                    delegataire: r.delegataire,
                    nature_retour: r.nature_retour,
                    statut: r.statut,
                    id: r.id,
                    url: r.url,
                    libelle: None,
                    flux: f.id.clone(),
                });
            }
        }
        return Ok(out);
    }
    let arbre = lire_texte(texte)?;
    let mut out = Vec::new();
    for flux in arbre.tous() {
        if local(&flux.tag) != "flux" {
            continue;
        }
        // Un identifiant vide n'est pas dit, comme tout enfant vide ; un autre qui n'a pas la forme d'un flux refuse la liste.
        let idf = match flux.attribut("id").filter(|s| !s.is_empty()) {
            Some(i) => Some(i),
            None => arbre.enfants(flux).find(|c| local(&c.tag) == "id").map(|c| c.texte_net()).filter(|t| !t.is_empty()),
        };
        let idf = idf.map(|i| IdDeFlux::lire(i).map_err(|e| socle::echec::cle(e.phrase, "flux.id").reclasser(Classe::Illisible))).transpose()?;
        for r in arbre.enfants(flux) {
            if local(&r.tag) != "retour" {
                continue;
            }
            let (mut publication, mut production, mut ops, mut delegataire, mut nature, mut statut, mut id, mut url) = (None, None, None, None, None, None, None, None);
            for (nom, t) in enfants_textes(&arbre, r) {
                let texte = Some(t.to_string());
                match nom {
                    "publication" => publication = Some(Zone::lue(t, format::InstantCompact).valeur().ok_or_else(|| illisible("flux.retour.publication"))?),
                    "production" => production = texte,
                    "ops" => ops = Some(organisme_liste(t, "flux.retour.ops")?),
                    "delegataire" => delegataire = Some(organisme_liste(t, "flux.retour.delegataire")?),
                    "nature" => nature = Some(code_liste(t, "flux.retour.nature")?),
                    "statut" => statut = Some(code_liste(t, "flux.retour.statut")?),
                    "id" => id = texte,
                    "url" => url = Some(Url::lire(t).map_err(|e| socle::echec::cle(e.phrase, "flux.retour.url").reclasser(Classe::Illisible))?),
                    autre => return Err(illisible(&format!("flux.retour.{}", autre))),
                }
            }
            let nature_retour = nature.ok_or_else(|| illisible("flux.retour.nature"))?;
            out.push(Annonce { publication, production, ops, delegataire, nature_retour, statut, id, url, libelle: None, flux: idf.clone() });
        }
    }
    Ok(out)
}

/// Les depots que la plate-forme liste (depots > declarant > depot{idflux, date, statut}) : XML (1.0) ou JSON (2.0).
pub fn liste_depots_d_octets(o: &[u8]) -> R<Vec<Depot>> {
    let texte = String::from_utf8_lossy(o);
    let texte = nettoye(&texte);
    if texte.starts_with('{') {
        let d: DepotsJson = socle::forme::lire_texte(texte, LISTE_ILLISIBLE).map_err(|m| m.reclasser(Classe::Illisible))?;
        return Ok(UnOuPlusieurs::en_liste(d.depots.and_then(|x| x.declarant).and_then(|x| x.depot)));
    }
    let arbre = lire_texte(texte)?;
    let mut out = Vec::new();
    for el in arbre.tous().filter(|el| local(&el.tag) == "depot") {
        let champs = enfants_textes(&arbre, el);
        let champ = |k: &str| {
            champs.iter().find(|(n, _)| *n == k).map(|(_, t)| t.to_string()).ok_or_else(|| illisible(&format!("depot.{}", k)))
        };
        if let Some((autre, _)) = champs.iter().find(|(n, _)| !["idflux", "date", "statut"].contains(n)) {
            return Err(illisible(&format!("depot.{}", autre)));
        }
        let date = Zone::lue(&champ("date")?, format::InstantCompact).valeur().ok_or_else(|| illisible("depot.date"))?;
        let idflux = IdDeFlux::lire(&champ("idflux")?).map_err(|e| socle::echec::cle(e.phrase, "depot.idflux").reclasser(Classe::Illisible))?;
        out.push(Depot { idflux, date, statut: champ("statut")? });
    }
    Ok(out)
}

// ------------------------------------------------------- CRM 94 : taux du PAS

#[derive(Clone, Default)]
struct Entree94 {
    nir: Option<Nir>,
    taux: Option<String>,
    type_: Option<String>,
    identifiant: Option<String>,
}

impl Entree94 {
    fn complete(&self) -> bool {
        self.identifiant.is_some() && self.nir.is_some()
    }

    fn champ(&mut self, c: ChampPas) -> &mut Option<String> {
        match c {
            ChampPas::Taux => &mut self.taux,
            ChampPas::Type => &mut self.type_,
            ChampPas::Identifiant => &mut self.identifiant,
        }
    }

    /// Le releve, quand l'entree dit son NIR et son identifiant ; un taux ou un type qui n'a pas la forme de la norme
    /// refuse la piece.
    fn releve(&self) -> R<Option<ReleveTauxPas>> {
        let (Some(nir), Some(identifiant)) = (self.nir.clone(), self.identifiant.clone()) else { return Ok(None) };
        let taux = self.taux.as_deref().map(taux_de_pas).transpose()?;
        let type_taux = self.type_.as_deref().map(type_de_taux).transpose()?;
        Ok(Some(ReleveTauxPas { nir, taux, type_taux, identifiant: identifiant_de_pas(&identifiant)? }))
    }
}

/// Un identifiant de taux de PAS lu dans un compte rendu (S21.G00.50.008) ; une autre forme refuse la piece.
fn identifiant_de_pas(t: &str) -> R<droit::pas::IdentifiantDuTauxPas> {
    droit::pas::IdentifiantDuTauxPas::lire(t).map_err(|e| e.reclasser(Classe::Illisible))
}

/// Un taux de PAS lu dans un compte rendu (S21.G00.50.006) ; une autre forme refuse la piece.
fn taux_de_pas(t: &str) -> R<droit::pas::TauxDePas> {
    droit::pas::TauxDePas::lire(t).ok_or_else(|| echec(Classe::Illisible, format!("Un compte rendu de taux de PAS porte le taux « {} », qui n'a pas la forme de la norme (S21.G00.50.006).", t)))
}

/// Un type de taux de PAS lu dans un compte rendu (S21.G00.50.007) ; un code hors de la liste refuse la piece.
fn type_de_taux(t: &str) -> R<droit::type_taux_pas::TypeDeTauxPas> {
    droit::type_taux_pas::TypeDeTauxPas::du_code(t).ok_or_else(|| echec(Classe::Illisible, format!("Un compte rendu de taux de PAS porte le type « {} », qui n'est pas dans la liste de la norme (S21.G00.50.007).", t)))
}

/// UN TAUX DE PAS QU'UN COMPTE RENDU DE NATURE 94 COMMUNIQUE : le NIR (13 caracteres), le taux, son type et l'identifiant
/// (S21.G00.50.006 a .008) ; un taux ou un type que la piece ne dit pas est vide : aucun taux transmis.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReleveTauxPas {
    pub nir: Nir,
    pub taux: Option<droit::pas::TauxDePas>,
    #[serde(rename = "type")]
    pub type_taux: Option<droit::type_taux_pas::TypeDeTauxPas>,
    pub identifiant: droit::pas::IdentifiantDuTauxPas,
}

/// LES TAUX DE PAS D'UN COMPTE RENDU, et la strategie qui les a lus (`None` : aucune n'a rien trouve).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TauxPasLus {
    pub strategie: Option<&'static str>,
    pub releves: Vec<ReleveTauxPas>,
}

/// UN TAUX DE PAS RATTACHE A LA FICHE DE SON SALARIE (`releves_vers_salaries`) : le releve, et la cle de la fiche.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TauxPasRattache {
    #[serde(flatten)]
    pub releve: ReleveTauxPas,
    pub salarie: droit::contexte::IdSalarie,
}

/// Ce que `releves_vers_salaries` rend : les taux rattaches a une fiche, et ceux dont aucune fiche ne porte le NIR.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TauxPasRepartis {
    pub rattaches: Vec<TauxPasRattache>,
    pub orphelins: Vec<ReleveTauxPas>,
}

/// (nom local, valeurs des attributs, texte net) de chaque element.
struct Plat {
    nom: String,
    attribs: Vec<(String, String)>,
    texte: String,
}

fn a_plat(a: &Arbre) -> Vec<Plat> {
    a.tous()
        .map(|e| Plat { nom: local(&e.tag).to_string(), attribs: e.attributs.clone(), texte: e.texte_net().to_string() })
        .collect()
}

fn valeurs_de(p: &Plat) -> Vec<&str> {
    p.attribs.iter().map(|(_, v)| v.as_str()).chain(std::iter::once(p.texte.as_str())).collect()
}

fn rubrique_pas(v: &str) -> Option<ChampPas> {
    RUBRIQUES_PAS.iter().find(|(k, _)| *k == v).map(|(_, c)| *c)
}

fn pas_rapport_crm(a: &Arbre) -> R<Vec<ReleveTauxPas>> {
    let racine = a.racine();
    if local(&racine.tag) != "rapport" {
        return Ok(Vec::new());
    }
    let mut releves = Vec::new();
    // Un rapport sans identifiant ne transmet aucun taux.
    let identifiant = racine.attribut("identifiant").filter(|s| !s.is_empty());
    for (identifiant, el) in identifiant.iter().flat_map(|i| a.tous().map(move |el| (i, el))) {
        if local(&el.tag) != "salarie" {
            continue;
        }
        let champs = enfants_textes(a, el);
        let lu = |k: &str| champs.iter().rev().find(|(n, _)| *n == k).map(|(_, t)| t.to_string());
        let Some(nir) = forme_du_retour(lu("NIR").as_deref(), Nir::lire)? else { continue };
        // Le schema des bilans (net-entreprises-dsn-bilans-v02r01-20230703.xsd) dit `taux_imposition_PAS` facultatif,
        // sans dire ce que son absence vaut : aucun texte deposé n'en fait un taux, un type ni un identifiant. Le salarie
        // que le compte rendu nomme sans taux n'a aucun taux transmis (ni taux, ni type) ; ce que sa paie en fait, c'est
        // le calcul du PAS qui le dit (`droit::pas::situation` : aucun taux de type 01, la grille).
        let (taux, type_taux) = match lu("taux_imposition_PAS") {
            Some(t) => (Some(taux_de_pas(&t)?), Some(droit::type_taux_pas::TypeDeTauxPas::TransmisParLaDgfip)),
            None => (None, None),
        };
        releves.push(ReleveTauxPas { nir, taux, type_taux, identifiant: identifiant_de_pas(identifiant)? });
    }
    Ok(releves)
}

fn pas_par_code(a: &Arbre) -> R<Vec<ReleveTauxPas>> {
    let elements = a_plat(a);
    const RUB: &str = "S21.G00.50.008";
    // Les attributs (clés et valeurs) puis le texte de chaque élément.
    let present = elements
        .iter()
        .any(|p| p.texte.contains(RUB) || p.attribs.iter().any(|(k, v)| k.contains(RUB) || v.contains(RUB)));
    if !present {
        return Ok(Vec::new());
    }
    let (mut releves, mut cur) = (Vec::new(), Entree94::default());
    let mut attente: Option<ChampPas> = None;
    for p in &elements {
        let valeurs = valeurs_de(p);
        for v in &valeurs {
            if let Some(nir) = nir_porte(v)? {
                if cur.complete() {
                    releves.push(std::mem::take(&mut cur));
                }
                cur.nir = Some(nir);
            }
        }
        let mut trouve = false;
        for v in &valeurs {
            if let Some(champ) = rubrique_pas(v) {
                attente = Some(champ);
                if !p.texte.is_empty() && rubrique_pas(&p.texte).is_none() {
                    *cur.champ(champ) = Some(p.texte.clone());
                    attente = None;
                }
                trouve = true;
                break;
            }
        }
        if !trouve {
            if let (Some(champ), false) = (attente, p.texte.is_empty()) {
                *cur.champ(champ) = Some(p.texte.clone());
                attente = None;
            }
        }
        if cur.complete() && cur.taux.is_some() && cur.type_.is_some() {
            releves.push(std::mem::take(&mut cur));
        }
    }
    if cur.complete() {
        releves.push(cur);
    }
    releves.iter().filter_map(|e| e.releve().transpose()).collect()
}

fn pas_par_nom_metier(a: &Arbre) -> R<Vec<ReleveTauxPas>> {
    let elements = a_plat(a);
    let metier = |nom: &str| NOMS_METIER.iter().find(|(k, _)| *k == nom).map(|(_, c)| *c);
    if !elements.iter().any(|p| metier(&p.nom.to_lowercase()).is_some()) {
        return Ok(Vec::new());
    }
    let (mut releves, mut cur) = (Vec::new(), Entree94::default());
    for p in &elements {
        for v in valeurs_de(p) {
            if let Some(nir) = nir_porte(v)? {
                if cur.complete() {
                    releves.push(std::mem::take(&mut cur));
                }
                cur.nir = Some(nir);
            }
        }
        if let (Some(champ), false) = (metier(&p.nom.to_lowercase()), p.texte.is_empty()) {
            if cur.champ(champ).is_some() {
                let nir = cur.nir.clone();
                releves.push(std::mem::replace(&mut cur, Entree94 { nir, ..Default::default() }));
            }
            *cur.champ(champ) = Some(p.texte.clone());
        }
    }
    if cur.complete() {
        releves.push(cur);
    }
    releves.iter().filter_map(|e| e.releve().transpose()).collect()
}

fn pas_par_balayage(a: &Arbre) -> R<Vec<ReleveTauxPas>> {
    let valeurs: Vec<String> = a_plat(a)
        .iter()
        .flat_map(|p| valeurs_de(p).into_iter().filter(|v| !v.is_empty()).map(str::to_string).collect::<Vec<_>>())
        .collect();
    let (mut releves, mut cur) = (Vec::new(), Entree94::default());
    for v in &valeurs {
        if let Some(nir) = nir_porte(v)? {
            if cur.complete() {
                releves.push(cur);
            }
            cur = Entree94 { nir: Some(nir), ..Default::default() };
        } else if cur.nir.is_some() {
            if cur.taux.is_none() && droit::pas::TauxDePas::lire(v).is_some() {
                cur.taux = Some(v.clone());
            } else if cur.type_.is_none() && droit::type_taux_pas::TypeDeTauxPas::du_code(v).is_some() {
                cur.type_ = Some(v.clone());
            } else if cur.identifiant.is_none() && droit::pas::IdentifiantDuTauxPas::lire(v).is_ok() {
                cur.identifiant = Some(v.clone());
            }
        }
    }
    if cur.complete() {
        releves.push(cur);
    }
    releves.iter().filter_map(|e| e.releve().transpose()).collect()
}

/// `extraire_taux_pas` : les quatre strategies, de la plus sure a la plus large.
pub fn taux_pas_d_octets(o: &[u8]) -> R<TauxPasLus> {
    let texte = String::from_utf8_lossy(o);
    let arbre = lire_texte(&texte)?;
    let strategies: [(&'static str, fn(&Arbre) -> R<Vec<ReleveTauxPas>>); 4] = [
        ("rapport CRM nominatif", pas_rapport_crm),
        ("rubriques codees", pas_par_code),
        ("noms metier NEODeS", pas_par_nom_metier),
        ("balayage large", pas_par_balayage),
    ];
    for (nom, f) in strategies {
        let releves = f(&arbre)?;
        if !releves.is_empty() {
            return Ok(TauxPasLus { strategie: Some(nom), releves });
        }
    }
    Ok(TauxPasLus { strategie: None, releves: Vec::new() })
}

/// Chaque releve a une fiche par le NIR ; `nirs` est la liste `(cle, nir)` des fiches qui en ont un, dans l'ordre du
/// fichier.
pub fn releves_vers_salaries(releves: &[ReleveTauxPas], nirs: &[(droit::contexte::IdSalarie, Nir)]) -> TauxPasRepartis {
    let mut par_nir: std::collections::HashMap<&str, &droit::contexte::IdSalarie> = std::collections::HashMap::new();
    for (c, n) in nirs {
        par_nir.insert(n.as_ref(), c);
    }
    let (mut rattaches, mut orphelins) = (Vec::new(), Vec::new());
    for r in releves {
        match par_nir.get(r.nir.as_ref()) {
            Some(cle) => rattaches.push(TauxPasRattache { releve: r.clone(), salarie: (*cle).clone() }),
            None => orphelins.push(r.clone()),
        }
    }
    TauxPasRepartis { rattaches, orphelins }
}

// ------------------------------------------------------------ l'AER, le .zip

fn u16_le(b: &[u8], i: usize) -> usize {
    u16::from_le_bytes([b[i], b[i + 1]]) as usize
}

fn u32_le(b: &[u8], i: usize) -> u64 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]) as u64
}

fn u64_le(b: &[u8], i: usize) -> u64 {
    let mut o = [0u8; 8];
    o.copy_from_slice(&b[i..i + 8]);
    u64::from_le_bytes(o)
}

fn mauvais(m: impl Into<String>) -> Echec {
    echec(Classe::ArchiveIllisible, m)
}

fn cp437(o: &[u8]) -> R<String> {
    o.iter()
        .map(|&b| if b < 0x80 { Some(b as char) } else { char::from_u32(CP437[b as usize - 0x80]) })
        .collect::<Option<String>>()
        .ok_or_else(|| mauvais("un nom de membre illisible en cp437"))
}

/// Le nom d'un membre : UTF-8 si le drapeau 11 est leve, cp437 sinon.
fn nom_zip(o: &[u8], drapeaux: usize) -> R<String> {
    if drapeaux & 0x800 != 0 {
        std::str::from_utf8(o)
            .map(str::to_string)
            .map_err(|e| echec(Classe::Illisible, format!("texte illisible en UTF-8 a l'octet {}", e.valid_up_to())))
    } else {
        cp437(o)
    }
}

struct Membre {
    nom: String,
    original: String,
    drapeaux: usize,
    methode: usize,
    crc: u64,
    taille_comp: u64,
    taille: u64,
    decalage: i128,
    /// L'octet ou le membre ne peut pas aller plus loin.
    fin: i128,
}

fn description_compression(t: usize) -> &'static str {
    match t {
        0 => "store",
        1 => "shrink",
        2..=5 => "reduce",
        6 | 10 => "implode",
        7 => "tokenize",
        8 => "deflate",
        9 => "deflate64",
        12 => "bzip2",
        14 => "lzma",
        18 => "terse",
        19 => "lz77",
        97 => "wavpack",
        98 => "ppmd",
        _ => "unknown",
    }
}

/// `ZipInfo._decodeExtra` : la zone zip64 des tailles et du decalage.
fn extra_zip64(extra: &[u8], m: &mut Membre) -> R<()> {
    let mut e = extra;
    while e.len() >= 4 {
        let (tp, ln) = (u16_le(e, 0), u16_le(e, 2));
        if ln + 4 > e.len() {
            return Err(mauvais(format!("archive zip abimee : champ supplementaire {:04x} (taille {})", tp, ln)));
        }
        if tp == 0x0001 {
            let mut d = &e[4..ln + 4];
            let mut lire = |champ: &str| -> R<u64> {
                if d.len() < 8 {
                    return Err(mauvais(format!("archive zip64 abimee : {} absent du champ supplementaire", champ)));
                }
                let v = u64_le(d, 0);
                d = &d[8..];
                Ok(v)
            };
            if m.taille == 0xFFFF_FFFF {
                m.taille = lire("File size")?;
            }
            if m.taille_comp == 0xFFFF_FFFF {
                m.taille_comp = lire("Compress size")?;
            }
            if m.decalage == 0xFFFF_FFFF {
                m.decalage = lire("Header offset")? as i128;
            }
        }
        e = &e[ln + 4..];
    }
    Ok(())
}

/// Le repertoire central, dans l'ordre de l'archive.
fn repertoire_zip(b: &[u8]) -> R<Vec<Membre>> {
    let pas_un_zip = || mauvais("ce n'est pas une archive zip");
    let taille_fichier = b.len();
    // `_EndRecData`
    let pos = if taille_fichier >= 22 && b[taille_fichier - 22..].starts_with(b"PK\x05\x06") && b[taille_fichier - 2..] == [0, 0] {
        taille_fichier - 22
    } else {
        let debut = taille_fichier.saturating_sub((1 << 16) + 22);
        match b[debut..].windows(4).rposition(|w| w == b"PK\x05\x06").map(|p| debut + p) {
            Some(p) if p + 22 <= taille_fichier => p,
            _ => return Err(pas_un_zip()),
        }
    };
    let rec = &b[pos..pos + 22];
    let (mut taille_cd, mut decalage_cd) = (u32_le(rec, 12) as i128, u32_le(rec, 16) as i128);
    let mut location = pos as i128;
    // `_EndRecData64`
    if pos >= 20 && &b[pos - 20..pos - 16] == b"PK\x06\x07" {
        let loc = pos - 20;
        let (disque, reloff, disques) = (u32_le(b, loc + 4), u64_le(b, loc + 8) as i128, u32_le(b, loc + 16));
        if disque != 0 || disques > 1 {
            return Err(mauvais("archive zip sur plusieurs disques : non lue"));
        }
        let offset = loc as i128 - 56;
        if reloff > offset {
            return Err(mauvais("archive zip64 abimee : le reperage de la fin du repertoire"));
        }
        let lire56 = |de: i128| -> Option<&[u8]> {
            let de = de as usize;
            (de.checked_add(56)? <= b.len()).then(|| &b[de..de + 56])
        };
        let mut extrasz = offset - reloff;
        let mut data = lire56(reloff).ok_or_else(pas_un_zip)?;
        if !data.starts_with(b"PK\x06\x06") && reloff != offset {
            extrasz = 0;
            data = lire56(offset).ok_or_else(pas_un_zip)?;
        }
        if !data.starts_with(b"PK\x06\x06") {
            return Err(mauvais("archive zip64 sans fin de repertoire"));
        }
        let (sz, dirsize, diroffset) = (u64_le(data, 4) as i128, u64_le(data, 40) as i128, u64_le(data, 48) as i128);
        if diroffset + dirsize != reloff || sz + 12 != 56 + extrasz {
            return Err(mauvais("archive zip64 abimee : la fin du repertoire"));
        }
        taille_cd = dirsize;
        decalage_cd = diroffset;
        location = offset - extrasz;
    }
    let concat = location - taille_cd - decalage_cd;
    let debut_cd = decalage_cd + concat;
    if debut_cd < 0 {
        return Err(mauvais("archive zip abimee : la position du repertoire"));
    }
    let debut_cd = (debut_cd as usize).min(b.len());
    let cd = &b[debut_cd..(debut_cd as i128 + taille_cd).min(b.len() as i128) as usize];
    let mut membres = Vec::new();
    let (mut total, mut p) = (0i128, 0usize);
    while total < taille_cd {
        if cd.len() < p + 46 {
            return Err(mauvais("archive zip tronquee : le repertoire"));
        }
        let c = &cd[p..p + 46];
        if &c[0..4] != b"PK\x01\x02" {
            return Err(mauvais("archive zip abimee : la signature du repertoire"));
        }
        p += 46;
        let (ln_nom, ln_extra, ln_comm) = (u16_le(c, 28), u16_le(c, 30), u16_le(c, 32));
        let prendre = |p: &mut usize, n: usize| -> &[u8] {
            let a = (*p).min(cd.len());
            let z = (*p + n).min(cd.len());
            *p += n;
            &cd[a..z]
        };
        let nom_brut = prendre(&mut p, ln_nom);
        let extra = prendre(&mut p, ln_extra);
        // Le commentaire de l'entree ne se lit pas : on passe ses octets.
        p += ln_comm;
        let drapeaux = u16_le(c, 8);
        let original = nom_zip(nom_brut, drapeaux)?;
        // Le nom s'arrete au premier octet nul (`split` rend toujours un premier morceau).
        let nom: String = original.chars().take_while(|&c| c != '\0').collect();
        let mut m = Membre {
            nom,
            original,
            drapeaux,
            methode: u16_le(c, 10),
            crc: u32_le(c, 16),
            taille_comp: u32_le(c, 20),
            taille: u32_le(c, 24),
            decalage: u32_le(c, 42) as i128,
            fin: 0,
        };
        if c[6] > 63 {
            return Err(echec(Classe::ArchiveNonLue, format!("archive zip de version {}.{} : non lue", c[6] / 10, c[6] % 10)));
        }
        extra_zip64(extra, &mut m)?;
        m.decalage += concat;
        membres.push(m);
        total += (46 + ln_nom + ln_extra + ln_comm) as i128;
    }
    // Chaque membre finit ou le suivant commence (le repertoire pour le dernier).
    let mut ordre: Vec<usize> = (0..membres.len()).collect();
    ordre.sort_by_key(|&i| std::cmp::Reverse(membres[i].decalage));
    let mut fin = debut_cd as i128;
    for i in ordre {
        membres[i].fin = fin;
        fin = membres[i].decalage;
    }
    Ok(membres)
}

fn crc32(o: &[u8]) -> u64 {
    let mut c = flate2::Crc::new();
    c.update(o);
    c.sum() as u64
}

/// `ZipFile.read(nom)`.
fn lire_membre(b: &[u8], m: &Membre) -> R<Vec<u8>> {
    if m.decalage < 0 {
        return Err(echec(Classe::Illisible, format!("negative seek value {}", m.decalage)));
    }
    let dec = m.decalage as usize;
    if dec.saturating_add(30) > b.len() {
        return Err(mauvais("archive zip tronquee : l'en-tete d'un fichier"));
    }
    let h = &b[dec..dec + 30];
    if &h[0..4] != b"PK\x03\x04" {
        return Err(mauvais("archive zip abimee : la signature de l'en-tete d'un fichier"));
    }
    let (ln_nom, ln_extra) = (u16_le(h, 26), u16_le(h, 28));
    let debut_nom = dec + 30;
    let nom_brut = &b[debut_nom.min(b.len())..(debut_nom + ln_nom).min(b.len())];
    let debut_donnees = debut_nom + ln_nom + ln_extra;
    if m.drapeaux & 0x20 != 0 {
        return Err(echec(Classe::ArchiveNonLue, "archive zip a donnees corrigees (bit 5) : non lue"));
    }
    if m.drapeaux & 0x40 != 0 {
        return Err(echec(Classe::ArchiveNonLue, "archive zip chiffree (bit 6) : non lue"));
    }
    let nom_en_tete = nom_zip(nom_brut, u16_le(h, 6))?;
    if nom_en_tete != m.original {
        return Err(mauvais(format!("archive zip abimee : le repertoire nomme « {} », l'en-tete « {} »", m.original, nom_en_tete)));
    }
    if (debut_donnees as i128).saturating_add(m.taille_comp as i128) > m.fin {
        return Err(mauvais(format!("archive zip abimee : « {} » chevauche un autre fichier", m.original)));
    }
    if m.drapeaux & 1 != 0 {
        return Err(echec(Classe::ArchiveNonLue, format!("File « {} » is encrypted, password required for extraction", m.nom)));
    }
    let comprime = &b[debut_donnees.min(b.len())..(debut_donnees.saturating_add(m.taille_comp as usize)).min(b.len())];
    let donnees = match m.methode {
        0 => {
            let n = (m.taille as usize).min(m.taille_comp as usize);
            if comprime.len() < n {
                return Err(echec(Classe::ArchiveNonLue, ""));
            }
            comprime[..n].to_vec()
        }
        8 => {
            // Du deflate brut. Un flux tronque rend ce qu'il a (le CRC dira le reste, comme
            // zlib) ; un flux faux est une `zlib.error`.
            use flate2::{Decompress, FlushDecompress, Status};
            let mut d = Decompress::new(false);
            let mut out: Vec<u8> = Vec::with_capacity((comprime.len() * 3).max(4096));
            loop {
                let lu = d.total_in() as usize;
                if out.capacity() == out.len() {
                    out.reserve(out.len().max(4096));
                }
                match d.decompress_vec(&comprime[lu..], &mut out, FlushDecompress::None) {
                    Ok(Status::StreamEnd) => break,
                    Ok(Status::Ok) => {}
                    Ok(Status::BufError) => {
                        if d.total_in() as usize >= comprime.len() {
                            break;
                        }
                        out.reserve(out.len().max(4096));
                    }
                    Err(e) => return Err(echec(Classe::ArchiveNonLue, format!("decompression impossible : {}", e))),
                }
            }
            out
        }
        t => {
            return Err(echec(Classe::ArchiveNonLue, format!("compression {} ({}) : non lue", t, description_compression(t))));
        }
    };
    if crc32(&donnees) != m.crc {
        return Err(mauvais(format!("archive zip abimee : le CRC-32 de « {} » ne correspond pas", m.nom)));
    }
    Ok(donnees)
}

/// Le dernier nom d'un chemin POSIX ; aucun pour un chemin qui n'en porte pas (`/`, `.`).
fn nom_de_fichier(nom: &str) -> Option<&str> {
    nom.split('/').rfind(|p| !p.is_empty() && *p != ".")
}

/// Le nom d'un fichier (`n`, sans son chemin) sans sa dernière extension.
fn radical(n: &str) -> &str {
    match n.rfind('.') {
        Some(i) if i > 0 && i < n.len() - 1 => &n[..i],
        _ => n,
    }
}

/// Le motif `_NOM_AER` : `AER_<siret>__<MMAAAA>_<code>_<JJMMAAAA>_<NOM>_<rang>_<id>.pdf`,
/// majuscules ou minuscules. Rend `(MMAAAA, NOM)`.
fn nom_aer(nom: &str) -> Option<(String, String)> {
    let s = sans_queue(nom, '\n');
    let c: Vec<char> = s.chars().collect();
    if c.len() < 4 || !"AER_".chars().zip(&c).all(|(a, b)| a.eq_ignore_ascii_case(b)) {
        return None;
    }
    let mut i = 4;
    let debut = i;
    while i < c.len() && est_chiffre(c[i]) {
        i += 1;
    }
    if i == debut {
        return None;
    }
    let debut = i;
    while i < c.len() && c[i] == '_' {
        i += 1;
    }
    if i == debut {
        return None;
    }
    let chiffres = |i: usize, n: usize| i + n <= c.len() && c[i..i + n].iter().all(|&x| est_chiffre(x));
    if !chiffres(i, 6) {
        return None;
    }
    let periode: String = c[i..i + 6].iter().collect();
    i += 6;
    if c.get(i) != Some(&'_') {
        return None;
    }
    i += 1;
    while i < c.len() && c[i] != '_' {
        i += 1;
    }
    if c.get(i) != Some(&'_') {
        return None;
    }
    i += 1;
    if !chiffres(i, 8) || c.get(i + 8) != Some(&'_') {
        return None;
    }
    i += 9;
    // `(?P<qui>.+?)` : le plus court dont la suite est `_<rang>_<id>.pdf`.
    let queue = |r: &[char]| -> bool {
        if r.first() != Some(&'_') {
            return false;
        }
        let mut j = 1;
        let d = j;
        while j < r.len() && est_chiffre(r[j]) {
            j += 1;
        }
        if j == d || r.get(j) != Some(&'_') {
            return false;
        }
        j += 1;
        let d = j;
        while j < r.len() && est_chiffre(r[j]) {
            j += 1;
        }
        j > d && r.len() == j + 4 && ".pdf".chars().zip(&r[j..]).all(|(a, b)| a.eq_ignore_ascii_case(b))
    };
    for p in i + 1..=c.len() {
        if c[p - 1] == '\n' {
            return None;
        }
        if queue(&c[p..]) {
            return Some((periode, c[i..p].iter().collect()));
        }
    }
    None
}

/// Une attestation d'une archive de nature 46 : son nom dans l'archive, sa periode (quand le nom la dit, `MMAAAA`), le salarie que le nom designe, et le PDF.
#[derive(Debug, Clone)]
pub struct AttestationAer {
    pub nom_archive: String,
    pub periode: Option<socle::mois::Mois>,
    pub qui: String,
    pub donnees: Vec<u8>,
}

/// `aer_spectacle` : les attestations d'une archive de nature 46 ; liste vide si
/// le contenu n'est pas une archive lisible.
pub fn aer_spectacle(o: &[u8]) -> R<Vec<AttestationAer>> {
    let membres = match repertoire_zip(o) {
        Ok(m) => m,
        Err(e) if e.classe == Some(Classe::ArchiveIllisible) => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    // `NameToInfo` garde le dernier membre de chaque nom.
    let mut out = Vec::new();
    for m in &membres {
        // Un dossier, ou un chemin qui ne nomme pas de fichier, n'est pas une attestation.
        let Some(fichier) = nom_de_fichier(&m.nom).filter(|_| !m.nom.ends_with('/')) else { continue };
        let (periode, qui) = match nom_aer(fichier) {
            Some((mmaaaa, qui)) => (Zone::lue(&mmaaaa, format::Mmaaaa).valeur(), qui.replace('_', " ")),
            None => (None, radical(fichier).to_string()),
        };
        let dernier = match membres.iter().rev().find(|x| x.nom == m.nom) {
            Some(d) => d,
            None => m,
        };
        let donnees = lire_membre(o, dernier)?;
        out.push(AttestationAer { nom_archive: m.nom.clone(), periode, qui, donnees });
    }
    Ok(out)
}

/// `rattacher` : pour chaque nom d'attestation, la cle du salarie, ou rien. `index` : `(cle, "NOM PRENOM")` des fiches,
/// dans l'ordre du fichier.
pub fn rattacher(quis: &[&str], index: &[(droit::contexte::IdSalarie, String)]) -> Vec<Option<droit::contexte::IdSalarie>> {
    let mut idx: Vec<(String, &droit::contexte::IdSalarie)> = index.iter().map(|(cle, nom)| (socle::texte::cle_recherche(nom), cle)).collect();
    idx.sort_by_key(|(n, _)| std::cmp::Reverse(n.chars().count()));
    quis.iter()
        .map(|q| {
            let cible = socle::texte::cle_recherche(q);
            idx.iter().find(|(nom, _)| !nom.is_empty() && (cible.contains(nom.as_str()) || nom.contains(cible.as_str()))).map(|(_, cle)| (*cle).clone())
        })
        .collect()
}

// ------------------------------------------------- la notification du taux AT/MP

/// UN TAUX AT/MP NOTIFIE, tel qu'un compte rendu de nature 34 le porte : l'etablissement de la declaration a laquelle il
/// est rattache (`SIREN`, `nic_affectation`), puis un element `taux_AT` (`code_ctn`, `section`, `code_risque`,
/// `temoin_bureau`, `taux`, `date_effet`, `date_calcul`). Le code CTN et la section ne se lisent pas : rien ne s'en sert.
#[derive(Debug, Clone, PartialEq)]
pub struct TauxAtmpNotifie {
    pub siren: droit::fiche::Siren,
    pub nic: droit::fiche::Nic,
    /// Le code risque tel que la DSN le declare (S21.G00.40.040) : celui de la notification, suivi du « B » quand le taux
    /// est celui des fonctions support.
    pub code_risque: droit::bareme::CodeRisqueAt,
    /// En pourcentage (1.50 pour 1,50 %).
    pub taux: socle::num::Decimal,
    pub date_effet: socle::date::Jour,
    pub date_calcul: Option<socle::date::Jour>,
}

/// Le jour d'une zone d'une notification : `AAAAMMJJ` (le format du schema des bilans) ou `AAAA-MM-JJ`, et ces deux formes
/// seules.
fn jour_notifie(t: &str) -> Option<socle::date::Jour> {
    match Zone::lue(t, format::Aaaammjj).valeur().filter(|j| j.aaaammjj() == t) {
        Some(j) => Some(j),
        None => Zone::lue(t, format::JourIso).valeur(),
    }
}

/// LES TAUX AT/MP QU'UNE NOTIFICATION DE NATURE 34 COMMUNIQUE. Le guide de l'API DSN (§5.26.1 : « CR métier taux AT »,
/// format DSN harmonise, rattache a la derniere declaration mensuelle) et la specification du schema des bilans (VT2.2,
/// groupe « Bilan » : « taux_AT — Code CTN, Section, Code risque, Témoin bureau, Taux, Date effet, Date calcul » ; les noms
/// des elements, `…-declaration-resultat-type-20201118.png`), deposes dans `atelier/reference/sources/gip-mds-api-dsn/`
/// (LISEZ-MOI, « La notification du taux AT/MP »). Le taux est lu en pourcentage et le temoin bureau « B » s'ajoute au
/// code (inferences dites au LISEZ-MOI). Refus quand la piece ne se lit pas, ou qu'elle ne porte aucun `taux_AT` lisible
/// (un bloc `communication` seul : les noms de ses rubriques ne sont ecrits nulle part) : rien ne se devine.
pub fn taux_atmp_d_octets(o: &[u8]) -> Result<Vec<TauxAtmpNotifie>, Echec> {
    let arbre = lire_octets(o)?;
    let mut out = Vec::new();
    for d in arbre.tous().filter(|e| local(&e.tag) == "declaration") {
        let id = arbre.sous(d).find(|x| local(&x.tag) == "declaration_identification");
        let (siren, nic) = match id {
            Some(i) => (premier_texte(&arbre, i, &["SIREN"]).to_string(), premier_texte(&arbre, i, &["nic_affectation"]).to_string()),
            None => (String::new(), String::new()),
        };
        for t in arbre.sous(d).filter(|x| local(&x.tag) == "taux_AT") {
            let champ = |n: &str| premier_texte(&arbre, t, &[n]).to_string();
            let illisible = |quoi: &str, v: &str| echec(Classe::Illisible, format!("Notification du taux AT/MP : {} illisible ({:?}).", quoi, v));
            if siren.is_empty() || nic.is_empty() {
                return Err(echec(Classe::Illisible, "Notification du taux AT/MP : la déclaration ne dit pas son établissement (SIREN, nic_affectation)."));
            }
            let siren_lu = droit::fiche::Siren::lire(&siren).map_err(|_| illisible("SIREN", &siren))?;
            let nic_lu = droit::fiche::Nic::lire(&nic).map_err(|_| illisible("NIC", &nic))?;
            // Le code de la notification a cinq signes (le « B » est son temoin bureau) : sa forme est celle de la DSN.
            let code = champ("code_risque").to_uppercase();
            let Some(lu) = droit::bareme::CodeRisqueAt::lire(&code).ok().filter(|c| c.as_str().len() == 5) else {
                return Err(illisible("code risque", &code));
            };
            let bureau = champ("temoin_bureau");
            let code_risque = match bureau.as_str() {
                "" => lu,
                "B" | "b" => lu.avec_bureau()?,
                autre => return Err(illisible("témoin bureau", autre)),
            };
            let brut = champ("taux");
            let taux = socle::num::decimal_a_virgule(&brut).filter(|v| *v > socle::num::Decimal::ZERO && *v < socle::num::Decimal::from(100));
            let Some(taux) = taux else { return Err(illisible("taux", &brut)) };
            let effet = champ("date_effet");
            let Some(date_effet) = jour_notifie(&effet) else { return Err(illisible("date d'effet", &effet)) };
            out.push(TauxAtmpNotifie {
                siren: siren_lu,
                nic: nic_lu,
                code_risque,
                taux,
                date_effet,
                date_calcul: match champ("date_calcul") {
                    c if c.is_empty() => None,
                    c => Some(jour_notifie(&c).ok_or_else(|| illisible("date de calcul", &c))?),
                },
            });
        }
    }
    if out.is_empty() {
        return Err(echec(Classe::Illisible, "Notification du taux AT/MP : aucun élément taux_AT lisible (code risque, taux, date d'effet)."));
    }
    Ok(out)
}

// --------------------------------------------- un rapport au format harmonise, lu pour l'Historique

fn premier_texte<'a>(a: &'a Arbre, e: &'a Noeud, noms: &[&str]) -> &'a str {
    for nom in noms {
        if let Some(x) = a.sous(e).find(|x| local(&x.tag) == *nom && !x.texte_net().is_empty()) {
            return x.texte_net();
        }
    }
    ""
}

/// Un rapport au FORMAT DSN HARMONISE (la racine `rapport` du schema des bilans) lu pour qu'on le lise : son type, le
/// message d'en-tete (`rapport/@message`), l'etat de l'envoi (`envoi_etat` : OK, KO ou ANO), chaque declaration
/// (nature, type, ordre, mois, etat), chaque anomalie (code, categorie, message, rubrique declenchante, numero de
/// ligne, nombre, et le salarie de son environnement), et les informations collectives (`communication`, §2.6 : le
/// type et les donnees, nom et valeur). Tout vient de la specification VT2.2 deposee
/// (`atelier/reference/sources/gip-mds-api-dsn/gip-mds-specifications-schema-bilans-vt2.2-20201118.txt`) ; les noms
/// d'elements sont ceux des pieces recues (`v01r08`) et des exemples du guide. `None` : ce n'est pas un rapport
/// harmonise (un XML d'un autre format, un PDF, une archive), ou il ne se lit pas.
pub fn lire_rapport(o: &[u8]) -> R<Option<Rapport>> {
    let Some(arbre) = octets_arbre(o)? else { return Ok(None) };
    let racine = arbre.racine();
    if local(&racine.tag) != "rapport" {
        return Ok(None);
    }
    let type_rapport = racine.attribut("type").map(str::to_string);
    // Le libelle d'un rapport de depot (`TypeDeRapport`) ; un autre type se dit tel qu'ecrit.
    let libelle = type_rapport.as_deref().and_then(TypeDeRapport::du_code).map(|t| t.libelle().to_string());
    let envoi = arbre.tous().find(|e| local(&e.tag) == "envoi");
    let de_l_envoi = |nom: &str| envoi.map(|e| premier_texte(&arbre, e, &[nom]).to_string()).filter(|t| !t.is_empty());
    let (etat_envoi, essai_reel) = (forme_du_retour(de_l_envoi("envoi_etat").as_deref(), Verdict::lire)?, essai_du_retour(de_l_envoi("essai_reel").as_deref())?);
    let mut declarations = Vec::new();
    for d in arbre.tous().filter(|e| local(&e.tag) == "declaration") {
        let sous_bloc = |nom: &str| match arbre.sous(d).find(|x| local(&x.tag) == nom) {
            Some(x) => x,
            None => d,
        };
        let (id, bilan) = (sous_bloc("declaration_identification"), sous_bloc("declaration_bilan"));
        let mois = premier_texte(&arbre, id, &["mois"]);
        declarations.push(DeclarationDuRapport {
            nature_declaration: nature_du_retour(Some(premier_texte(&arbre, id, &["nature"])).filter(|t| !t.is_empty()))?,
            type_declaration: type_du_retour(Some(premier_texte(&arbre, id, &["type"])).filter(|t| !t.is_empty()))?,
            ordre: forme_du_retour(Some(premier_texte(&arbre, id, &["ordre"])).filter(|t| !t.is_empty()), NumeroDOrdre::lire)?,
            mois: if mois.is_empty() { None } else { Some(mois_du_rapport(mois)?) },
            etat: forme_du_retour(Some(premier_texte(&arbre, bilan, &["etat", "etat_conformite"])).filter(|t| !t.is_empty()), Verdict::lire)?,
        });
    }
    let mut anomalies = Vec::new();
    for a in arbre.tous().filter(|e| {
        let n = local(&e.tag);
        // `anomalie`, ou un nom qui finit en `…_anomalie` (`declaration_anomalie`, `envoi_anomalie`).
        n.strip_suffix("anomalie").is_some_and(|debut| debut.is_empty() || debut.ends_with('_'))
    }) {
        let rubrique = match arbre.sous(a).find(|x| local(&x.tag) == "rubrique_declenchante") {
            Some(r) => premier_texte(&arbre, r, &["identifiant", "rubrique"]).to_string(),
            None => premier_texte(&arbre, a, &["identifiant"]).to_string(),
        };
        let salarie = [premier_texte(&arbre, a, &["nom_famille"]), premier_texte(&arbre, a, &["prenoms"])]
            .iter()
            .filter(|s| !s.is_empty())
            .cloned()
            .collect::<Vec<&str>>()
            .join(" ");
        let (code, message) = (premier_texte(&arbre, a, &["code"]), premier_texte(&arbre, a, &["message"]));
        if code.is_empty() && message.is_empty() {
            continue;
        }
        anomalies.push(AnomalieDuRapport {
            code: code.to_string(),
            categorie: Some(premier_texte(&arbre, a, &["categorie"])).filter(|c| !c.is_empty()).map(str::to_string),
            message: message.to_string(),
            rubrique,
            ligne: premier_texte(&arbre, a, &["numero_ligne"]).to_string(),
            nombre: premier_texte(&arbre, a, &["nombre"]).to_string(),
            salarie,
        });
    }
    let mut informations = Vec::new();
    for c in arbre.tous().filter(|e| local(&e.tag) == "information_collective") {
        let donnees = arbre
            .sous(c)
            .skip(1)
            .filter(|x| x.enfants.is_empty() && !x.texte_net().is_empty() && local(&x.tag) != "type")
            .map(|x| DonneeCollective { nom: local(&x.tag).to_string(), valeur: x.texte_net().to_string() })
            .collect();
        informations.push(InformationCollective { type_information: premier_texte(&arbre, c, &["type"]).to_string(), donnees });
    }
    Ok(Some(Rapport {
        type_rapport,
        libelle,
        message: racine.attribut("message").map(str::to_string).filter(|m| !m.is_empty()),
        etat_envoi,
        essai_reel,
        declarations,
        anomalies,
        informations,
    }))
}

/// UN RAPPORT AU FORMAT HARMONISE, lu pour qu'on le lise (`lire_rapport`). Le type, son libelle, le message, l'etat et
/// l'essai de l'envoi sont vides quand le rapport ne les dit pas ; le texte d'un element (une declaration, une anomalie,
/// une information) est celui que le rapport lui donne.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Rapport {
    #[serde(rename = "type")]
    pub type_rapport: Option<String>,
    pub libelle: Option<String>,
    pub message: Option<String>,
    pub etat_envoi: Option<Verdict>,
    pub essai_reel: Option<EssaiOuReel>,
    pub declarations: Vec<DeclarationDuRapport>,
    pub anomalies: Vec<AnomalieDuRapport>,
    pub informations: Vec<InformationCollective>,
}

/// Une declaration d'un rapport : nature, type, ordre, mois et etat, quand il les dit.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeclarationDuRapport {
    #[serde(rename = "nature")]
    pub nature_declaration: Option<NatureDeLaDeclaration>,
    #[serde(rename = "type")]
    pub type_declaration: Option<TypeDeLaDeclaration>,
    pub ordre: Option<NumeroDOrdre>,
    pub mois: Option<socle::mois::Mois>,
    pub etat: Option<Verdict>,
}

/// Une anomalie d'un rapport, telle que la page l'affiche.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AnomalieDuRapport {
    pub code: String,
    /// Telle qu'ecrite : la specification des comptes rendus harmonises n'en fixe pas la liste (« Contenu a definir
    /// ulterieurement, a priori un niveau de gravite », VT2.2 l. 248) ; celle du bilan est `CategorieDAnomalie`.
    pub categorie: Option<String>,
    pub message: String,
    pub rubrique: String,
    pub ligne: String,
    pub nombre: String,
    pub salarie: String,
}

/// Une information collective d'un rapport (§2.6) : son type, et ses donnees.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InformationCollective {
    #[serde(rename = "type")]
    pub type_information: String,
    pub donnees: Vec<DonneeCollective>,
}

/// Une donnee d'une information collective : son nom et sa valeur.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DonneeCollective {
    pub nom: String,
    pub valeur: String,
}

/// Les noms des membres d'une archive ZIP (une archive d'AER, nature 43 ou 46) ; vide si ce n'en est pas une.
pub fn membres_du_zip(o: &[u8]) -> R<Vec<String>> {
    match repertoire_zip(o) {
        Ok(m) => Ok(m.into_iter().map(|x| x.nom).filter(|n| !n.ends_with('/')).collect()),
        Err(e) if e.classe == Some(Classe::ArchiveIllisible) => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}

// ------------------------------------------------ un compte rendu NEOReS, lu

droit::codes_fermes! {
    /// `RO.B001.R001` « Declencheur du retour organisme » : 01 a 04 au cahier technique NEOReS 2026.1.2 (l. 1619-1622,
    /// `atelier/reference/sources/neores/gip-mds-neores-cahier-technique-2026.1.2-20251024.txt`) ; 05 et 06 viennent du
    /// cahier 2027.1.0 (l. 1749-1754, `gip-mds-neores-cahier-technique-2027.1.0-20260303.txt`, et la note
    /// differentielle, § II).
    DeclencheurNeores, "Déclencheur du retour organisme (RO.B001.R001)", texte "cahier technique NEOReS 2027.1.0, l. 1749-1754" {
        DsnOuSignalement = "01",
        FinDExercice = "02",
        Rappel = "03",
        Parametre = "04",
        Substitutions = "05",
        Informatif = "06",
    }
}

impl DeclencheurNeores {
    /// Ce qu'il dit, pour la phrase du compte rendu.
    pub fn libelle(self) -> &'static str {
        match self {
            DeclencheurNeores::DsnOuSignalement => "retour sur une DSN mensuelle ou un signalement d'événement",
            DeclencheurNeores::FinDExercice => "retour en fin d'exercice",
            DeclencheurNeores::Rappel => "rappel d'actions non réalisées",
            DeclencheurNeores::Parametre => "paramètre à prendre en compte en paie",
            DeclencheurNeores::Substitutions => "récapitulatif des substitutions de données opérées par l'organisme",
            DeclencheurNeores::Informatif => "retour informatif",
        }
    }
}

droit::codes_fermes! {
    /// `RO.B002.R004` « Type de retour organisme » : « 01 - Retour organisme normal », « 02 - Retour organisme annule et
    /// remplace » (cahier technique NEOReS 2026.1.2, l. 1690-1691 ; 2027.1.0, l. 1821-1822).
    TypeDeRetourNeores, "Type de retour organisme (RO.B002.R004)", texte "cahier technique NEOReS 2026.1.2, l. 1690-1691" {
        Normal = "01",
        AnnuleEtRemplace = "02",
    }
}

droit::codes_fermes! {
    /// `RO.B004.R003` « Type d'action » (cahier technique NEOReS 2026.1.2, l. 1892-1901 ; 2027.1.0, l. 2026-2030).
    TypeDActionNeores, "Type d'action (RO.B004.R003)", texte "cahier technique NEOReS 2026.1.2, l. 1892-1901" {
        CorrectionDeDonnees = "01",
        CorrectionDIdentification = "02",
        EmissionDesDonneesAttendues = "03",
        PriseEnCompteDUnParametre = "04",
        RegularisationDUnParametre = "05",
    }
}

impl TypeDActionNeores {
    /// Ce qu'il dit, pour la phrase du compte rendu.
    pub fn libelle(self) -> &'static str {
        match self {
            TypeDActionNeores::CorrectionDeDonnees => "correction de données DSN précédemment déclarées",
            TypeDActionNeores::CorrectionDIdentification => "correction de données d'identification d'individus",
            TypeDActionNeores::EmissionDesDonneesAttendues => "émission de l'ensemble des données attendues par l'organisme pour un mois principal déclaré",
            TypeDActionNeores::PriseEnCompteDUnParametre => "prise en compte d'un paramètre",
            TypeDActionNeores::RegularisationDUnParametre => "régularisation d'un paramètre",
        }
    }
}

droit::codes_fermes! {
    /// `RO.B004.R006` et `RO.B007.R009` « Resultat du controle » (cahier technique NEOReS 2026.1.2, l. 1962-1968 ; le bloc
    /// RO.B007 n'en a que 01 a 06, l. 2477-2482).
    ResultatDuControleNeores, "Résultat du contrôle (RO.B004.R006, RO.B007.R009)", texte "cahier technique NEOReS 2026.1.2, l. 1962-1968" {
        ValeurIncoherente = "01",
        RubriqueAbsente = "02",
        RubriquePresenteATort = "03",
        BlocAbsent = "04",
        BlocPresentATort = "05",
        CumulIncoherent = "06",
        DonneesDuMoisAbsentes = "07",
    }
}

impl ResultatDuControleNeores {
    /// Ce qu'il dit, pour la phrase du compte rendu.
    pub fn libelle(self) -> &'static str {
        match self {
            ResultatDuControleNeores::ValeurIncoherente => "valeur incohérente",
            ResultatDuControleNeores::RubriqueAbsente => "rubrique absente",
            ResultatDuControleNeores::RubriquePresenteATort => "rubrique présente à tort",
            ResultatDuControleNeores::BlocAbsent => "bloc absent",
            ResultatDuControleNeores::BlocPresentATort => "bloc présent à tort",
            ResultatDuControleNeores::CumulIncoherent => "cumul incohérent",
            ResultatDuControleNeores::DonneesDuMoisAbsentes => "ensemble des données attendues pour le mois absentes",
        }
    }
}

droit::codes_fermes! {
    /// `RO.B004.R016` (cahier technique NEOReS 2026.1.2, l. 2087-2088) puis `RO.B005.R005` (cahier 2027.1.0, l. 2340-2341)
    /// « Caractere substituable de la donnee ».
    SubstituableNeores, "Caractère substituable de la donnée (RO.B004.R016, RO.B005.R005)", texte "cahier technique NEOReS 2026.1.2, l. 2087-2088" {
        Substituable = "01",
        NonSubstituable = "02",
    }
}

impl SubstituableNeores {
    /// Ce qu'il dit, pour la phrase du compte rendu.
    pub fn libelle(self) -> &'static str {
        match self {
            SubstituableNeores::Substituable => "donnée substituable : sans correction ni opposition de votre part, l'Urssaf ou la MSA la corrigera elle-même",
            SubstituableNeores::NonSubstituable => "donnée non substituable",
        }
    }
}

/// Le texte de la rubrique `nom` (`RO_B004_R001`), enfant direct du bloc `e` ; aucun si le bloc ne la porte pas, ou
/// vide.
fn rubrique_de<'a>(a: &'a Arbre, e: &'a Noeud, nom: &str) -> Option<&'a str> {
    a.enfants(e).find(|x| local(&x.tag) == nom).map(Noeud::texte_net).filter(|t| !t.is_empty())
}

/// `rubrique_de`, en texte.
fn rubrique_texte(a: &Arbre, e: &Noeud, nom: &str) -> Option<String> {
    rubrique_de(a, e, nom).map(str::to_string)
}

/// `rubrique_de`, un montant signe (RO.B005.R004, CSL 00 : `0\.00|[+-](0|[1-9]\d*)\.\d{2}`, cahier technique NEOReS
/// 2027.1.0, `atelier/reference/sources/neores/gip-mds-neores-cahier-technique-2027.1.0-20260303.txt`) : « + » un
/// credit, « - » un debit ; une autre forme se refuse.
fn rubrique_montant(a: &Arbre, e: &Noeud, nom: &str) -> R<Option<socle::num::Decimal>> {
    rubrique_de(a, e, nom)
        .map(|t| {
            let forme = t == "0.00" || (t.starts_with(['+', '-']) && t.len() >= 5 && t.as_bytes()[t.len() - 3] == b'.');
            forme.then(|| socle::num::decimal_lu(t)).flatten().ok_or_else(|| echec(Classe::Illisible, format!("{} : {} n'est pas un montant signé", nom, socle::date::repr_texte(t))))
        })
        .transpose()
}

/// `rubrique_de`, une date : `AAAA-MM-JJ`, la forme d'une rubrique de nature date du cahier technique NEOReS (CSL 00,
/// `atelier/reference/sources/neores/gip-mds-neores-cahier-technique-2026.1.2-20251024.txt`, l. 482) ; une autre forme
/// se refuse.
fn rubrique_jour(a: &Arbre, e: &Noeud, nom: &str) -> R<Option<socle::date::Jour>> {
    rubrique_de(a, e, nom)
        .map(|t| Zone::lue(t, format::JourIso).valeur().ok_or_else(|| echec(Classe::Illisible, format!("{} : {} n'est pas une date AAAA-MM-JJ", nom, socle::date::repr_texte(t)))))
        .transpose()
}

/// `rubrique_de`, un instant : `AAAA-MM-JJTHH:MM:SS`, la forme des dates de production d'un retour et de calcul d'un
/// parametre ([19,19], meme cahier, RO.B002.R011 et RO.B008.R006) ; une autre forme se refuse.
fn rubrique_instant(a: &Arbre, e: &Noeud, nom: &str) -> R<Option<socle::paris::Moment>> {
    rubrique_de(a, e, nom)
        .map(|t| Zone::lue(t, format::InstantIso).valeur().ok_or_else(|| echec(Classe::Illisible, format!("{} : {} n'est pas un instant AAAA-MM-JJTHH:MM:SS", nom, socle::date::repr_texte(t)))))
        .transpose()
}

/// UN INDIVIDU D'UN COMPTE RENDU NEOReS (`RO.B006`, `RO.B012`) : nom, prenom, matricule, debut et numero du contrat.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IndividuNeores {
    pub nom: Option<String>,
    pub prenom: Option<String>,
    pub matricule: Option<String>,
    pub debut_contrat: Option<socle::date::Jour>,
    pub contrat: Option<String>,
}

/// Une donnee DSN en jeu (`RO.B007`) : la rubrique, la valeur declaree et l'attendue, le cumul, la periode, le
/// resultat, et les valeurs acceptees (`RO.B010`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DonneeNeores {
    pub rubrique: Option<String>,
    pub declaree: Option<String>,
    pub attendue: Option<String>,
    pub cumul: bool,
    pub debut: Option<socle::date::Jour>,
    pub fin: Option<socle::date::Jour>,
    pub resultat: Option<ResultatDuControleNeores>,
    pub acceptees: Vec<String>,
}

/// Le detail d'une action (`RO.B005`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DetailNeores {
    pub atypie: Option<String>,
    pub debut: Option<socle::date::Jour>,
    pub fin: Option<socle::date::Jour>,
    pub montant: Option<socle::num::Decimal>,
    pub substituable: Option<SubstituableNeores>,
    pub rappel: Option<String>,
    pub interlocuteur: Option<String>,
    pub individus: Vec<IndividuNeores>,
    pub donnees: Vec<DonneeNeores>,
}

/// Une cible d'action (`RO.B004`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ActionNeores {
    pub code: Option<String>,
    pub libelle: Option<String>,
    #[serde(rename = "type")]
    pub type_action: Option<TypeDActionNeores>,
    pub resultat: Option<ResultatDuControleNeores>,
    pub individus: Option<String>,
    pub contrats: Option<String>,
    pub controle: Option<String>,
    pub reference: Option<String>,
    pub action: Option<String>,
    pub aide: Option<String>,
    pub consequences: Option<String>,
    pub substituable: Option<SubstituableNeores>,
    pub rappel: Option<String>,
    pub exception: Option<String>,
    pub details: Vec<DetailNeores>,
}

/// Un parametre (`RO.B008`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ParametreNeores {
    #[serde(rename = "type")]
    pub type_parametre: Option<TypeDeParametreNeores>,
    pub valeur: Option<String>,
    pub effet: Option<socle::date::Jour>,
    pub fin: Option<socle::date::Jour>,
    pub rubrique: Option<String>,
    #[serde(with = "socle::paris::ecrit::seconde::option")]
    pub calcule_le: Option<socle::paris::Moment>,
}

/// Une identite (`RO.B009`) : ce qui a ete declare, et la reference. L'individu declare l'est par son NIR (R001) ou, sans
/// NIR, par son numero technique temporaire (R002) (cahier technique NEOReS 2027.1.0,
/// `atelier/reference/sources/neores/gip-mds-neores-cahier-technique-2027.1.0-20260303.txt` l. 2752-2798).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IdentiteNeores {
    pub nir_declare: Option<Nir>,
    pub ntt_declare: Option<declarations::ntt::Ntt>,
    pub nir_reference: Option<Nir>,
    pub nom_declare: Option<String>,
    pub nom_reference: Option<String>,
    pub prenom_declare: Option<String>,
    pub prenom_reference: Option<String>,
    pub naissance_declaree: Option<String>,
    pub naissance_reference: Option<String>,
    pub matricule: Option<String>,
}

/// Une information (`RO.B011`) et ses individus (`RO.B012`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct InformationNeores {
    pub code: Option<String>,
    pub libelle: Option<String>,
    pub numero: Option<String>,
    pub information: Option<String>,
    pub reference: Option<String>,
    pub aide: Option<String>,
    pub individus: Vec<IndividuNeores>,
}

/// UN COMPTE RENDU NEOReS LU (`lire_neores`) ; une rubrique que le compte rendu ne porte pas est vide.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Neores {
    pub version: Option<String>,
    pub declencheur: Option<DeclencheurNeores>,
    pub code: Option<String>,
    pub libelle: Option<String>,
    /// `RO.B002.R004` vaut « 02 », un retour qui annule et remplace (`TypeDeRetourNeores`).
    pub remplace: bool,
    pub organisme: Option<String>,
    pub debut: Option<socle::date::Jour>,
    pub fin: Option<socle::date::Jour>,
    #[serde(with = "socle::paris::ecrit::seconde::option")]
    pub produit_le: Option<socle::paris::Moment>,
    pub mois: Option<socle::mois::Mois>,
    pub essai_reel: Option<EssaiOuReel>,
    pub ordre: Option<NumeroDOrdre>,
    pub metier: Option<String>,
    pub conformite: Option<Verdict>,
    pub actions: Vec<ActionNeores>,
    pub parametres: Vec<ParametreNeores>,
    pub identites: Vec<IdentiteNeores>,
    pub informations: Vec<InformationNeores>,
}

/// Les blocs `nom` sous `e` (lui exclu), dans l'ordre du document.
fn blocs_sous<'a>(a: &'a Arbre, e: &'a Noeud, nom: &'a str) -> Vec<&'a Noeud> {
    a.sous(e).skip(1).filter(|x| local(&x.tag) == nom).collect()
}

/// Un compte rendu au FORMAT NEOReS (CRM 118 a 121, 124, 125, 130, 132, 135, 136 : guide de l'API DSN v6.8, §6.8 et
/// tableau des retours), lu d'apres son cahier technique : CT 2026.1.2 et CT 2027.1.0
/// (`atelier/reference/sources/neores/`). Les blocs et rubriques s'y nomment `RO.Bnnn.Rnnn`, ecrits `RO_Bnnn_Rnnn` dans le
/// fichier transmis (CT 2026.1.2, §2.2). Un bloc est un element qui porte ses rubriques et ses blocs enfants (les
/// arborescences, §5) : une INFERENCE sur la forme XML (le schema XSD n'est pas public : net-entreprises sert le cahier
/// technique, pas le schema), celle que le lecteur des effectifs eprouve sur les 118 recus depuis 2022
/// (`releve_retours::reconstruire_effectifs`). Rend :
/// - `retour` : le code, le libelle, l'organisme emetteur (`RO.B002.R002`, `.R003`, `.R005`), le declencheur
///   (`RO.B001.R001`), l'annule-et-remplace (`RO.B002.R004` = 02), la periode (`.R008`, `.R009`) ;
/// - `conformite` et `metier` (`RO.B003.R002`, `.R001`) ;
/// - `actions` : chaque cible d'action (`RO.B004`), ses details (`RO.B005`), leurs individus (`RO.B006`), leurs
///   donnees DSN (`RO.B007`, valeurs acceptees `RO.B010`) ;
/// - `parametres` (`RO.B008`) et `identites` (`RO.B009`) ; `informations` (`RO.B011`, `RO.B012`, CT 2027.1.0).
///
/// `None` : aucun bloc NEOReS (`RO_B000` a `RO_B012`), ou le fichier ne se lit pas.
pub fn lire_neores(o: &[u8]) -> R<Option<Neores>> {
    let Some(a) = octets_arbre(o)? else { return Ok(None) };
    let est_bloc = |n: &Noeud| {
        let t = local(&n.tag);
        t.len() == 7 && t.starts_with("RO_B") && t[4..].chars().all(|c| c.is_ascii_digit())
    };
    if !a.tous().any(est_bloc) {
        return Ok(None);
    }
    let premier = |nom: &str| a.tous().find(|x| local(&x.tag) == nom);
    let r = |bloc: Option<&Noeud>, nom: &str| bloc.and_then(|b| rubrique_texte(&a, b, nom));
    let (b000, b001, b002, b003) = (premier("RO_B000"), premier("RO_B001"), premier("RO_B002"), premier("RO_B003"));
    let version = a.tous().find(|x| local(&x.tag) == "RO_R001").map(|x| x.texte_net().to_string()).filter(|t| !t.is_empty());
    let declencheur = forme_du_retour(r(b001, "RO_B001_R001").as_deref(), code_ferme::<DeclencheurNeores>)?;
    let type_de_retour = forme_du_retour(r(b002, "RO_B002_R004").as_deref(), code_ferme::<TypeDeRetourNeores>)?;
    let individu = |b: &Noeud, p: &str| -> R<IndividuNeores> {
        Ok(IndividuNeores {
            nom: rubrique_texte(&a, b, &format!("{}_R003", p)),
            prenom: rubrique_texte(&a, b, &format!("{}_R004", p)),
            matricule: rubrique_texte(&a, b, &format!("{}_R005", p)),
            debut_contrat: rubrique_jour(&a, b, &format!("{}_R006", p))?,
            contrat: rubrique_texte(&a, b, &format!("{}_R007", p)),
        })
    };
    let mut actions = Vec::new();
    for c in a.tous().filter(|x| local(&x.tag) == "RO_B004") {
        let rc = |nom: &str| rubrique_texte(&a, c, nom);
        let mut details = Vec::new();
        for d in blocs_sous(&a, c, "RO_B005") {
            let rd = |nom: &str| rubrique_texte(&a, d, nom);
            let individus = blocs_sous(&a, d, "RO_B006").into_iter().map(|b| individu(b, "RO_B006")).collect::<R<_>>()?;
            let donnees = blocs_sous(&a, d, "RO_B007")
                .into_iter()
                .map(|b| {
                    Ok(DonneeNeores {
                        rubrique: rubrique_texte(&a, b, "RO_B007_R001"),
                        declaree: rubrique_texte(&a, b, "RO_B007_R002"),
                        attendue: rubrique_texte(&a, b, "RO_B007_R003"),
                        cumul: rubrique_de(&a, b, "RO_B007_R004") == Some("01"),
                        debut: rubrique_jour(&a, b, "RO_B007_R005")?,
                        fin: rubrique_jour(&a, b, "RO_B007_R006")?,
                        resultat: forme_du_retour(rubrique_de(&a, b, "RO_B007_R009"), code_ferme::<ResultatDuControleNeores>)?,
                        acceptees: blocs_sous(&a, b, "RO_B010").into_iter().filter_map(|v| rubrique_texte(&a, v, "RO_B010_R001")).collect(),
                    })
                })
                .collect::<R<_>>()?;
            details.push(DetailNeores {
                atypie: rd("RO_B005_R001"),
                debut: rubrique_jour(&a, d, "RO_B005_R002")?,
                fin: rubrique_jour(&a, d, "RO_B005_R003")?,
                montant: rubrique_montant(&a, d, "RO_B005_R004")?,
                // CT 2027.1.0 : le caractere substituable et le rappel descendent au detail de l'action (note differentielle, § III).
                substituable: forme_du_retour(rubrique_de(&a, d, "RO_B005_R005"), code_ferme::<SubstituableNeores>)?,
                rappel: rd("RO_B005_R006"),
                interlocuteur: rd("RO_B005_R007"),
                individus,
                donnees,
            });
        }
        actions.push(ActionNeores {
            code: rc("RO_B004_R001"),
            libelle: rc("RO_B004_R002"),
            type_action: forme_du_retour(rubrique_de(&a, c, "RO_B004_R003"), code_ferme::<TypeDActionNeores>)?,
            resultat: forme_du_retour(rubrique_de(&a, c, "RO_B004_R006"), code_ferme::<ResultatDuControleNeores>)?,
            individus: rc("RO_B004_R009"),
            contrats: rc("RO_B004_R010"),
            controle: rc("RO_B004_R011"),
            reference: rc("RO_B004_R012"),
            action: rc("RO_B004_R013"),
            aide: rc("RO_B004_R014"),
            consequences: rc("RO_B004_R015"),
            substituable: forme_du_retour(rubrique_de(&a, c, "RO_B004_R016"), code_ferme::<SubstituableNeores>)?,
            rappel: rc("RO_B004_R017"),
            exception: rc("RO_B004_R018"),
            details,
        });
    }
    let parametres = a
        .tous()
        .filter(|x| local(&x.tag) == "RO_B008")
        .map(|p| {
            Ok(ParametreNeores {
                type_parametre: forme_du_retour(rubrique_de(&a, p, "RO_B008_R001"), code_ferme::<TypeDeParametreNeores>)?,
                valeur: rubrique_texte(&a, p, "RO_B008_R002"),
                effet: rubrique_jour(&a, p, "RO_B008_R003")?,
                fin: rubrique_jour(&a, p, "RO_B008_R004")?,
                rubrique: rubrique_texte(&a, p, "RO_B008_R005"),
                calcule_le: rubrique_instant(&a, p, "RO_B008_R006")?,
            })
        })
        .collect::<R<_>>()?;
    let identites = a
        .tous()
        .filter(|x| local(&x.tag) == "RO_B009")
        .map(|b| {
            let x = |n: &str| rubrique_texte(&a, b, n);
            Ok(IdentiteNeores {
                nir_declare: forme_du_retour(rubrique_de(&a, b, "RO_B009_R001"), Nir::lire)?,
                ntt_declare: forme_du_retour(rubrique_de(&a, b, "RO_B009_R002"), declarations::ntt::Ntt::lire)?,
                nir_reference: forme_du_retour(rubrique_de(&a, b, "RO_B009_R003"), Nir::lire)?,
                nom_declare: x("RO_B009_R004"),
                nom_reference: x("RO_B009_R005"),
                prenom_declare: x("RO_B009_R006"),
                prenom_reference: x("RO_B009_R007"),
                naissance_declaree: x("RO_B009_R008"),
                naissance_reference: x("RO_B009_R009"),
                matricule: x("RO_B009_R010"),
            })
        })
        .collect::<R<_>>()?;
    let informations = a
        .tous()
        .filter(|x| local(&x.tag) == "RO_B011")
        .map(|b| {
            let x = |n: &str| rubrique_texte(&a, b, n);
            Ok(InformationNeores {
                code: x("RO_B011_R001"),
                libelle: x("RO_B011_R002"),
                numero: x("RO_B011_R003"),
                information: x("RO_B011_R004"),
                reference: x("RO_B011_R005"),
                aide: x("RO_B011_R006"),
                individus: blocs_sous(&a, b, "RO_B012").into_iter().map(|i| individu(i, "RO_B012")).collect::<R<_>>()?,
            })
        })
        .collect::<R<_>>()?;
    Ok(Some(Neores {
        version,
        declencheur,
        code: r(b002, "RO_B002_R002"),
        libelle: r(b002, "RO_B002_R003"),
        remplace: type_de_retour == Some(TypeDeRetourNeores::AnnuleEtRemplace),
        organisme: r(b002, "RO_B002_R005"),
        debut: b002.map(|b| rubrique_jour(&a, b, "RO_B002_R008")).transpose()?.flatten(),
        fin: b002.map(|b| rubrique_jour(&a, b, "RO_B002_R009")).transpose()?.flatten(),
        produit_le: b002.map(|b| rubrique_instant(&a, b, "RO_B002_R011")).transpose()?.flatten(),
        mois: r(b000, "RO_B000_R006").map(|m| mois_du_jour(&m)).transpose()?,
        essai_reel: essai_du_retour(r(b000, "RO_B000_R013").as_deref())?,
        ordre: forme_du_retour(r(b000, "RO_B000_R012").as_deref(), NumeroDOrdre::lire)?,
        metier: r(b003, "RO_B003_R001"),
        conformite: forme_du_retour(r(b003, "RO_B003_R002").as_deref(), Verdict::lire)?,
        actions,
        parametres,
        identites,
        informations,
    }))
}

// ------------------------------- un compte rendu d'organisme complementaire, lu

droit::codes_fermes! {
    /// `Fichier/@TypCRM`, le type du compte rendu : « 2 (CRT compte-rendu basique) », « 3 (CRC compte-rendu complet) »
    /// (schema CRM DSN OC P3 V1.1.4, l. 1071-1081,
    /// `atelier/reference/sources/gip-mds-api-dsn/ctip-crm-dsn-oc-p3-v1.1.4-schema-20230915.xsd`).
    TypeDeCrmOc, "Type de CRM (TypCRM)", texte "schéma CRM DSN OC P3 V1.1.4, l. 1071-1081" {
        Traitement = "2",
        Complet = "3",
    }
}

droit::codes_fermes! {
    /// `Declaration/@CodeRetourDecla`, le code retour global : schema CRM DSN OC P3 V1.1.4 (l. 1001-1015) et
    /// specification V1.1.4 du 17/06/2025 (`ctip-crm-dsn-oc-p3-v1.1.4-20250617.txt`, niveau 2, « Code retour global »).
    RetourGlobalOc, "Code retour global (CodeRetourDecla)", texte "schéma CRM DSN OC P3 V1.1.4, l. 1001-1015" {
        Acceptee = "0",
        AccepteeAvecCorrections = "1",
        PartiellementPriseEnCompte = "2",
        NonPriseEnCompte = "3",
    }
}

impl RetourGlobalOc {
    /// Ce qu'il dit, pour la phrase du compte rendu.
    pub fn libelle(self) -> &'static str {
        match self {
            RetourGlobalOc::Acceptee => "DSN acceptée",
            RetourGlobalOc::AccepteeAvecCorrections => "DSN acceptée, avec des corrections à apporter (anomalies non bloquantes)",
            RetourGlobalOc::PartiellementPriseEnCompte => "DSN partiellement prise en compte (des erreurs empêchent la prise en compte de certains salariés ou de certaines données)",
            RetourGlobalOc::NonPriseEnCompte => "DSN non prise en compte par l'organisme (erreurs au niveau de la déclaration)",
        }
    }
}

droit::codes_fermes! {
    /// `RetourMetier/@CriticiteRetour` : « C = "Critique" », « P = "Passable" » (schema CRM DSN OC P3 V1.1.4, l. 514-525).
    CriticiteOc, "Criticité du retour (CriticiteRetour)", texte "schéma CRM DSN OC P3 V1.1.4, l. 514-525" {
        Critique = "C",
        Passable = "P",
    }
}

impl CriticiteOc {
    /// Ce qu'elle dit, pour la phrase du compte rendu.
    pub fn libelle(self) -> &'static str {
        match self {
            CriticiteOc::Critique => "critique : à corriger impérativement dans la DSN suivante",
            CriticiteOc::Passable => "passable : à corriger dès que possible",
        }
    }
}

/// Un compte rendu metier d'organisme complementaire (nature 51 : guide de l'API DSN v6.8, tableau des retours,
/// « CRM-DSN-OC-P3-V1.1.4-17062025 »), lu d'apres son schema et sa specification
/// (`atelier/reference/sources/gip-mds-api-dsn/ctip-crm-dsn-oc-p3-v1.1.4-schema-20230915.xsd`,
/// `ctip-crm-dsn-oc-p3-v1.1.4-20250617.txt`) : l'element `Fichier`, ses `Declaration` (code retour global, complement,
/// producteur, contact, compteurs), leurs mandats SEPA a creer (`MandatNetE`, `MandatProp`) et leurs `RetourMetier`
/// (code « SXXXnn », libelle, description, criticite, rubrique ou bloc a l'origine, valeur declaree, valeur corrigee ou
/// attendue, date d'evenement, regroupement) avec leur `ContexteRetourMetier` (rubrique, libelle, valeur) -- d'ou le
/// salarie (S21.G00.30.002 et .004). `None` : ce n'en est pas un, ou il ne se lit pas (il est en ISO-8859-1, qu'il
/// declare : guide v6.8, §7.1).
/// Un jour `JJMMAAAA`, et rien d'autre.
fn jour_jjmmaaaa(t: &str) -> Option<socle::date::Jour> {
    Zone::lue(t, format::Jjmmaaaa).valeur()
}

/// Un attribut d'un CRM OC qui n'a pas la forme du schema : le CRM ne se lit pas.
#[track_caller]
fn illisible_crm_oc(attribut: &str, valeur: &str) -> Echec {
    socle::echec::cle(format!("Le compte rendu d'un organisme complémentaire ne se lit pas : {} vaut {}, pas JJMMAAAA.", attribut, socle::date::repr_texte(valeur)), attribut).reclasser(Classe::Illisible)
}

pub fn lire_crm_oc(o: &[u8]) -> R<Option<CrmOc>> {
    let Some(a) = octets_arbre(o)? else { return Ok(None) };
    let racine = a.racine();
    if local(&racine.tag) != "Fichier" || !a.enfants(racine).any(|x| local(&x.tag) == "Declaration") {
        return Ok(None);
    }
    let at = |n: &Noeud, k: &str| n.attribut(k).map(nettoye).filter(|t| !t.is_empty()).map(str::to_string);
    // Un code d'organisme ou de delegataire du CRM ; hors de sa forme, le CRM ne se lit pas.
    let organisme_crm_oc = |n: &Noeud, k: &str| -> R<Option<IdentifiantDOrganisme>> {
        at(n, k).map(|t| IdentifiantDOrganisme::lire(&t).map_err(|e| socle::echec::cle(format!("Le compte rendu d'un organisme complémentaire ne se lit pas : {}", e.phrase), k).reclasser(Classe::Illisible))).transpose()
    };
    let mut declarations = Vec::new();
    for d in a.enfants(racine).filter(|x| local(&x.tag) == "Declaration") {
        let mandats = a
            .enfants(d)
            .filter(|x| matches!(local(&x.tag), "MandatNetE" | "MandatProp"))
            .map(|m| -> R<MandatOc> {
                // L'adresse du mandat ; hors de la forme d'une adresse, le CRM ne se lit pas.
                let url = at(m, "URLMandatProp").map(|t| Url::lire(&t).map_err(|e| socle::echec::cle(format!("Le compte rendu d'un organisme complémentaire ne se lit pas : {}", e.phrase), "URLMandatProp").reclasser(Classe::Illisible))).transpose()?;
                Ok(MandatOc { net_entreprises: local(&m.tag) == "MandatNetE", iban: at(m, "IBAN"), bic: at(m, "BIC"), ics: at(m, "ICS"), url })
            })
            .collect::<R<Vec<MandatOc>>>()?;
        let mut retours = Vec::new();
        for rm in a.enfants(d).filter(|x| local(&x.tag) == "RetourMetier") {
            let contexte: Vec<ContexteOc> = a
                .enfants(rm)
                .filter(|x| local(&x.tag) == "ContexteRetourMetier")
                .map(|c| ContexteOc { rubrique: at(c, "Rubrique"), libelle: at(c, "Libelle"), valeur: at(c, "Valeur"), libelle_valeur: at(c, "LibelVal") })
                .collect();
            let du_contexte = |r: &str| contexte.iter().find(|c| c.rubrique.as_deref() == Some(r)).and_then(|c| c.valeur.clone());
            let salarie = [du_contexte("S21.G00.30.002"), du_contexte("S21.G00.30.004")].into_iter().flatten().collect::<Vec<_>>().join(" ");
            retours.push(RetourMetierOc {
                code: at(rm, "CodeRetourMetier"),
                libelle: at(rm, "LibelCodeRetourMetier"),
                description: at(rm, "DescriptionRetour"),
                criticite: forme_du_retour(at(rm, "CriticiteRetour").as_deref(), code_ferme::<CriticiteOc>)?,
                nombre: forme_du_retour(at(rm, "NbErreursSimilaires").as_deref(), compte_du_retour)?,
                rubrique: at(rm, "RubriqueBlocRetour"),
                libelle_rubrique: at(rm, "LibelRubBlocRetour"),
                valeur: at(rm, "ValeurRubriqueErreur"),
                libelle_valeur: at(rm, "LibelValRubriqueErreur"),
                corrigee: at(rm, "ValeurCorrigeeAjoutee"),
                libelle_corrigee: at(rm, "LibelValCorrigeeAjoutee"),
                date_evenement: at(rm, "DateEvt").map(|t| jour_jjmmaaaa(&t).ok_or_else(|| illisible_crm_oc("DateEvt", &t))).transpose()?,
                salarie: Some(salarie).filter(|s| !s.is_empty()),
                contexte,
            });
        }
        declarations.push(DeclarationOc {
            nature_declaration: nature_du_retour(at(d, "NatureDecla").as_deref())?,
            type_declaration: type_du_retour(at(d, "TypeDecla").as_deref())?,
            ordre: forme_du_retour(at(d, "NoOrdreDecla").as_deref(), NumeroDOrdre::lire)?,
            mois: at(d, "DateMoisDec").map(|m| mois_du_jour(&m)).transpose()?,
            code_oc: organisme_crm_oc(d, "CodeOC")?,
            code_delegataire: organisme_crm_oc(d, "CodeDELEG")?,
            producteur: at(d, "NomProducteurCRM"),
            code_producteur: organisme_crm_oc(d, "CodeProducteurCRM")?,
            contact: at(d, "NomContact"),
            courriel: at(d, "MailContact"),
            telephone: at(d, "TelContact"),
            code_retour: forme_du_retour(at(d, "CodeRetourDecla").as_deref(), code_ferme::<RetourGlobalOc>)?,
            complement: at(d, "ComplementRetourDecla"),
            erreurs: forme_du_retour(at(d, "NbreErreur").as_deref(), compte_du_retour)?,
            critiques: forme_du_retour(at(d, "NbreCritique").as_deref(), compte_du_retour)?,
            affiliations: forme_du_retour(at(d, "NbreAff").as_deref(), compte_du_retour)?,
            radiations: forme_du_retour(at(d, "NbreRad").as_deref(), compte_du_retour)?,
            salaries: forme_du_retour(at(d, "NbreSal").as_deref(), compte_du_retour)?,
            mandats,
            retours,
        });
    }
    Ok(Some(CrmOc { type_crm: forme_du_retour(at(racine, "TypCRM").as_deref(), code_ferme::<TypeDeCrmOc>)?, essai_reel: essai_du_retour(at(racine, "CodeEnvoi").as_deref())?, version: at(racine, "VersionCRM"), declarations }))
}

/// UN COMPTE RENDU D'ORGANISME COMPLEMENTAIRE LU (`lire_crm_oc`) ; un attribut que le fichier ne porte pas est vide.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CrmOc {
    pub type_crm: Option<TypeDeCrmOc>,
    pub essai_reel: Option<EssaiOuReel>,
    pub version: Option<String>,
    pub declarations: Vec<DeclarationOc>,
}

/// Une `Declaration` du compte rendu.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeclarationOc {
    #[serde(rename = "nature")]
    pub nature_declaration: Option<NatureDeLaDeclaration>,
    #[serde(rename = "type")]
    pub type_declaration: Option<TypeDeLaDeclaration>,
    pub ordre: Option<NumeroDOrdre>,
    pub mois: Option<socle::mois::Mois>,
    /// S21.G00.15.002, l'organisme (schema du CRM OC, `CodeOC`).
    pub code_oc: Option<IdentifiantDOrganisme>,
    /// S21.G00.15.003, le delegataire de gestion (`CodeDELEG`).
    pub code_delegataire: Option<IdentifiantDOrganisme>,
    pub producteur: Option<String>,
    /// L'organisme ou le delegataire qui produit le compte rendu (`CodeProducteurCRM`).
    pub code_producteur: Option<IdentifiantDOrganisme>,
    pub contact: Option<String>,
    pub courriel: Option<String>,
    pub telephone: Option<String>,
    pub code_retour: Option<RetourGlobalOc>,
    pub complement: Option<String>,
    pub erreurs: Option<usize>,
    pub critiques: Option<usize>,
    pub affiliations: Option<usize>,
    pub radiations: Option<usize>,
    pub salaries: Option<usize>,
    pub mandats: Vec<MandatOc>,
    pub retours: Vec<RetourMetierOc>,
}

/// Un mandat SEPA a creer (`MandatNetE`, `MandatProp`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MandatOc {
    pub net_entreprises: bool,
    pub iban: Option<String>,
    pub bic: Option<String>,
    pub ics: Option<String>,
    pub url: Option<Url>,
}

/// Un `RetourMetier`, et le salarie que son contexte nomme.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RetourMetierOc {
    pub code: Option<String>,
    pub libelle: Option<String>,
    pub description: Option<String>,
    pub criticite: Option<CriticiteOc>,
    pub nombre: Option<usize>,
    pub rubrique: Option<String>,
    pub libelle_rubrique: Option<String>,
    pub valeur: Option<String>,
    pub libelle_valeur: Option<String>,
    pub corrigee: Option<String>,
    pub libelle_corrigee: Option<String>,
    /// `DateEvt`, `JJMMAAAA` (schema du CRM OC v1.1.4, son motif ;
    /// `atelier/reference/sources/gip-mds-api-dsn/ctip-crm-dsn-oc-p3-v1.1.4-schema-20230915.xsd`).
    pub date_evenement: Option<socle::date::Jour>,
    pub salarie: Option<String>,
    pub contexte: Vec<ContexteOc>,
}

/// Un `ContexteRetourMetier` : la rubrique, son libelle, la valeur et son libelle.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ContexteOc {
    pub rubrique: Option<String>,
    pub libelle: Option<String>,
    pub valeur: Option<String>,
    pub libelle_valeur: Option<String>,
}

/// Le mois d'un jour que la norme ecrit : `DateMoisDec` d'un compte rendu d'organisme complementaire, au format
/// JJMMAAAA (schema CRM DSN OC v1.1.4, `atelier/reference/sources/gip-mds-api-dsn/`), et `RO.B000.R006` d'un NEOReS,
/// `AAAA-MM-01` (cahier technique 2027.1.0, `atelier/reference/sources/neores/`) ; un jour qui ne se lit pas refuse le
/// compte rendu.
fn mois_du_jour(brut: &str) -> R<socle::mois::Mois> {
    match Zone::lue(brut, format::JourEcrit).valeur() {
        Some(j) => Ok(j.mois_civil()),
        None => Err(echec(Classe::Illisible, format!("date illisible : {}", socle::date::repr_texte(brut.trim())))),
    }
}
