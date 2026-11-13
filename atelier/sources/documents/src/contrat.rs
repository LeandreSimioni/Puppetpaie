//! Le contrat ecrit, tel qu'il s'ecrit : le contrat d'engagement d'un spectacle, le CDD et le CDI d'un permanent, et
//! leurs avenants.
//!
//! Le texte du contrat n'est PAS dans ce module : il vit dans son modele (`contrat_ecrit::ModeleDuContrat`), la base de
//! sa nature ou un modele special de la compagnie, que les routes choisissent et figent au depart
//! (`routes::modeles_contrat`). Ce module envoie a la fabrique le TEXTE du modele et les VALEURS de la paie
//! (`avenant::valeurs_de_la_paie`) ; c'est elle qui le met en page. Un modele qui tairait une mention que la loi exige
//! se refuse (`contrat_ecrit::refuser_un_modele_incomplet`).

use std::path::Path;

use fabrique_pdf::forme::{self, Description, Document, Style};
use serde::Serialize;
use socle::date::Jour;

use crate::avenant::valeurs_de_la_paie;
use crate::bulletin::{chemin_net, identite_de, FORMAT, LOGO_LARGEUR, LOGO_MARGE_DROITE, LOGO_MARGE_HAUT};
use droit::contexte::EntrepriseDuDocument;
use engagement::calcul::Paie;
use droit::contexte::Contexte;
use socle::echec::{erreur, refus, APart, Classe, Echec, R};
use crate::contrat_ecrit::ModeleDuContrat;
use crate::marqueurs::{pour_la_fabrique, ModeleLu, Valeurs};
use droit::marqueurs::Marqueur;


/// Une description de documents modeles : le format, le style, le logo de la compagnie, et ses documents.
pub fn enveloppe(style: Style, ent: &EntrepriseDuDocument, documents: Vec<Document>) -> Description {
    Description {
        format: FORMAT,
        page: None,
        police: None,
        style: Some(style),
        logo: identite_de(&ent.gabarits, "logo").map(|logo| forme::Logo {
            chemin: chemin_net(&logo),
            largeur: LOGO_LARGEUR,
            marge_haut: LOGO_MARGE_HAUT,
            marge_droite: LOGO_MARGE_DROITE,
        }),
        titre: None,
        documents,
        entete_page: None,
        pied_page: None,
    }
}

/// Un document modele : le texte du modele `lu` et ses valeurs, rien d'autre.
pub fn document_modele(lu: &ModeleLu, valeurs: &Valeurs) -> forme::Modele {
    forme::Modele {
        chemin: None,
        texte: Some(lu.texte.clone()),
        valeurs: pour_la_fabrique(valeurs),
        remplace: None,
        signature: None,
        signature_employeur: None,
        modifications: None,
    }
}

/// Les valeurs que le modele lit dans cette paie (sa forme canonique, `Paie::canon`).
pub fn valeurs_de_paie(ctx: &Contexte, ent: &EntrepriseDuDocument, p: &Paie) -> R<Valeurs> {
    Ok(valeurs_de_la_paie(ctx, ent, &p.canon(ctx)?, None)?)
}

/// Ce qu'un refus de modele porte a part : le modele, le marqueur en cause, et ce qu'il faut ecrire a sa place.
#[derive(Serialize)]
struct MarqueurAPart {
    modele: String,
    marqueur: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    a_ecrire: Option<String>,
}

/// Les contrats qui en annulent un autre : `{(salarie, premier jour, numero du contrat): (numero remplace, date)}`.
pub type Remplacements = [((droit::contexte::IdSalarie, Jour, engagement::numeros::NumeroContrat), (String, Jour))];

/// `contrat.description` puis `fabrique.figer` : la description de ces contrats, un document par
/// paie, son modele (`modeles`, un par paie, dans leur ordre) en texte. `remplacements` : ces contrats-la annulent et
/// remplacent celui qui est parti avant eux, et le disent en tete.
pub fn description(ctx: &Contexte, paies: &[Paie], modeles: &[ModeleDuContrat], ent: &EntrepriseDuDocument, remplacements: &Remplacements) -> R<Description> {
    if paies.len() != modeles.len() {
        return Err(refus("description illisible : chaque contrat a son modèle"));
    }
    let signature = identite_de(&ent.gabarits, "signature");
    let mut documents = Vec::new();
    for (p, modele) in paies.iter().zip(modeles) {
        let nat = p.engagement.nat()?;
        let lu = &modele.lu;
        let v = valeurs_de_paie(ctx, ent, p)?;
        let mut un = document_modele(lu, &v);
        let (salarie, debut) = (&p.salarie.id, p.engagement.debut);
        let remplace = p.engagement.numero.as_ref().and_then(|n| remplacements.iter().find(|((s, d, k), _)| s == salarie && *d == debut && k == n));
        if let Some((_, (numero, date))) = remplace {
            un.remplace = Some(forme::Remplace { numero: numero.clone(), date: date.jj_mm_aaaa() });
        }
        un.signature = signature.as_deref().map(chemin_net);
        refuser_ce_que_le_modele_tairait(modele, &v)?;
        crate::contrat_ecrit::refuser_un_modele_incomplet(modele, &nat, &v)?;
        documents.push(Document::Modele(un));
    }
    Ok(enveloppe(Style::Contrat, ent, documents))
}

/// Les valeurs qu'un modele de contrat DOIT citer des qu'elles ne sont pas vides. `remuneration.detail` ne cite que
/// ce qui fait le brut ; ce qui est verse en plus (les defraiements) n'est que dans `remuneration.hors_brut`.
const A_CITER: [Marqueur; 1] = [Marqueur::RemunerationHorsBrut];

/// REGLE 1 : un gabarit de compagnie qui n'a pas le marqueur d'une valeur a citer ne fait pas un contrat qui la tait
/// (un marqueur INCONNU, la lecture du modele le refuse deja ; c'est l'inverse : une valeur sans sa place). Une valeur
/// vide passe : le modele qui l'omet dit alors tout ce qu'il y a a dire.
///
/// L'inverse aussi, pour le taux du cachet (REGLE 1) : un contrat qui ne paie aucun cachet, et dont ni
/// l'engagement ni le mois n'ecrivent de taux, n'en a pas ; le moteur prenait 130 EUR, un chiffre qu'aucun texte ne fixe.
/// Un modele qui cite `{{remuneration.taux_cachet}}` ne fait pas ce contrat-la (la fabrique dirait « marqueur
/// inconnu », sans dire pourquoi).
fn refuser_ce_que_le_modele_tairait(modele: &ModeleDuContrat, valeurs: &Valeurs) -> R<()> {
    const TAUX: Marqueur = Marqueur::RemunerationTauxCachet;
    let qui = valeurs.get(&Marqueur::SalarieNomComplet).ok_or_else(|| refus("Le contrat ne nomme pas son salarié."))?;
    let lu = &modele.lu;
    if valeurs.get(&TAUX).is_none() && lu.cite(TAUX) {
        return Err(Echec {
            a_part: Some(APart::forme(MarqueurAPart { modele: modele.nom.clone(), marqueur: TAUX.to_string(), a_ecrire: Some(Marqueur::RemunerationDetail.to_string()) })),
            ..erreur(
                Classe::ModeleIncomplet,
                format!(
                    "{} cite le taux du cachet, et le contrat de {} n'a aucun taux de cachet : il ne paie aucun cachet, et ni l'engagement ni le mois n'en écrivent. Le moteur n'en invente pas. Écrire la rémunération de ce modèle par le détail de la rémunération, qui la dit quelle que soit sa forme (le modèle de base le fait), ou porter le taux sur une ligne de cachet de l'engagement. Rien n'est écrit.",
                    modele.nom,
                    qui,
                ),
            )
        });
    }
    for cle in A_CITER {
        let Some(valeur) = valeurs.get(&cle).filter(|v| !v.trim().is_empty()) else { continue };
        if !lu.cite(cle) {
            return Err(Echec {
                a_part: Some(APart::forme(MarqueurAPart { modele: modele.nom.clone(), marqueur: cle.to_string(), a_ecrire: None })),
                ..erreur(
                    Classe::ModeleIncomplet,
                    format!(
                        "{} ne cite pas ce que le contrat de {} lui verse hors brut ({}) : le contrat le tairait. Ajouter à l'article « Rémunération » de ce modèle un alinéa [si {}] qui le cite (le modèle de base le fait). Rien n'est écrit.",
                        modele.nom,
                        qui,
                        valeur.trim(),
                        cle.nom()
                    ),
                )
            });
        }
    }
    Ok(())
}

/// Ce que le logiciel depose dans la case de l'artiste, quatre lignes.
pub fn mention_signature(nom: &str, quand_lisible: &str, adresse: &str, code: &str) -> String {
    format!("Signé électroniquement le {}\npar {}\n{}\ncode de signature {}", quand_lisible, nom, adresse, code)
}

/// Le contrat de `paies` (un par page), ecrit a `chemin`. Le contrat parti se refait ainsi, depuis la version de
/// l'engagement qui est partie ; son exemplaire contresigne est ce PDF-la, plus la mention de l'artiste
/// (`fabrique_pdf::marquage::contresigner`).
pub fn ecrire_contrat(ctx: &Contexte, paies: &[Paie], modeles: &[ModeleDuContrat], ent: &EntrepriseDuDocument, chemin: &Path, remplacements: &Remplacements) -> R<()> {
    crate::fabrique::ecrire_pdf(&description(ctx, paies, modeles, ent, remplacements)?, chemin)
}


/// `avenant.description` puis `fabrique.figer` : le modele de l'avenant (`lu`) en texte, les valeurs du contrat et les
/// siennes (`extra`), et les articles modifies avec l'ancien texte et le nouveau. `du_contrat` : le modele du contrat
/// qu'il modifie, celui avec lequel ce contrat est parti ; `None`, un contrat signe ailleurs, dont l'avenant ne cite
/// aucun article : les refus qui gardent ces articles ne le regardent pas.
fn description_avenant(ctx: &Contexte, p: &Paie, ent: &EntrepriseDuDocument, lu: &ModeleLu, modifs: &[forme::Modification], extra: Valeurs, du_contrat: Option<&ModeleDuContrat>) -> R<Description> {
    let nat = p.engagement.nat()?;
    let mut v = valeurs_de_paie(ctx, ent, p)?;
    // Les memes refus que le contrat : ses articles modifies sont ceux du modele de CONTRAT de la compagnie,
    // et un modele qui n'a pas le marqueur d'une valeur a citer ferait un avenant qui la tait.
    v.extend(extra);
    if let Some(contrat) = du_contrat {
        refuser_ce_que_le_modele_tairait(contrat, &v)?;
        crate::contrat_ecrit::refuser_un_modele_incomplet(contrat, &nat, &v)?;
    }
    let mut un = document_modele(lu, &v);
    un.modifications = Some(modifs.to_vec());
    un.signature = identite_de(&ent.gabarits, "signature").as_deref().map(chemin_net);
    Ok(enveloppe(Style::Avenant, ent, vec![Document::Modele(un)]))
}

/// Le contrat que l'avenant modifie, comme il le nomme (`{{avenant.reference}}`) : « n° FA1.26.13 du 12/10/2026 », ou,
/// signe hors du logiciel (`ailleurs`), « commencé le 01/03/2022 » -- le numero que le logiciel lui a donne n'est pas sur
/// le papier signe.
fn reference_du_contrat(p: &Paie, date_contrat: Jour, ailleurs: bool) -> String {
    let date_contrat = date_contrat.jj_mm_aaaa();
    match p.engagement.numero.as_ref() {
        Some(n) if !ailleurs => format!("n° {} du {}", n, date_contrat),
        _ if ailleurs => format!("commencé le {}", date_contrat),
        _ => format!("du {}", date_contrat),
    }
}

/// L'avenant en PDF, a l'identite de la compagnie, la signature de l'employeur posee comme sur
/// le contrat. Les dates en JJ/MM/AAAA. `modifs` : ce que `avenant-apercu` rend ; `modele` : le modele du contrat, dont
/// l'avenant suit le sien -- celui avec lequel le contrat est parti, ou, signe hors du logiciel (`ailleurs`), celui que
/// son engagement suit : ses modifications se disent alors en clair (`avenant::changements_en_clair`), par le paragraphe
/// `{{modification.phrase}}` du bloc MODIFICATION:.
pub fn ecrire_avenant(
    ctx: &Contexte,
    p: &Paie,
    ent: &EntrepriseDuDocument,
    chemin: &Path,
    modifs: &[forme::Modification],
    numero: i64,
    date_contrat: Jour,
    effet: Jour,
    date: Jour,
    modele: &ModeleDuContrat,
    ailleurs: bool,
) -> R<()> {
    if modifs.is_empty() {
        return Err(erreur(Classe::Illisible, "Rien n'a changé : pas d'avenant à écrire."));
    }
    let dits = crate::contrat_ecrit::manques_de_l_avenant(&modele.avenant.texte);
    if !dits.is_empty() {
        let quoi_faire = if modele.base { "C'est un défaut du logiciel : le signaler." } else { "Corriger l'avenant de ce modèle (Configuration › Modèles de contrat)." };
        return Err(erreur(Classe::ModeleIncomplet, format!("L'avenant de {} ne se fait pas : {} {} Rien n'est parti.", modele.nom, dits.join(" "), quoi_faire)));
    }
    let extra = Valeurs::from([
        (Marqueur::AvenantNumero, numero.to_string()),
        (Marqueur::AvenantDateContrat, date_contrat.jj_mm_aaaa()),
        (Marqueur::AvenantReference, reference_du_contrat(p, date_contrat, ailleurs)),
        (Marqueur::AvenantEffet, effet.jj_mm_aaaa()),
        (Marqueur::AvenantDate, date.jj_mm_aaaa()),
    ]);
    let d = description_avenant(ctx, p, ent, &modele.avenant, modifs, extra, (!ailleurs).then_some(modele))?;
    crate::fabrique::ecrire_pdf(&d, chemin)
}
