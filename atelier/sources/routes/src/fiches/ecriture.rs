//! UNE FICHE SE SAISIT, SE VERIFIE ET S'ECRIT PAR UN SEUL CHEMIN. La fiche est sa forme a un jour (`droit::fiche` :
//! `ChampsEntreprise`, `ChampsSalarie`, `ChampsSpectacle`) ; ce que l'ecran change, ce sont des cases
//! (`ChangementEntreprise`…), posees sur la fiche telle qu'il l'a montree. Chaque case tapee s'est typee a la porte
//! (`droit::fiche::FicheSaisie::lire_cases`, `Changements::lire`), ses fautes a cote ; la fiche typee se derive (la forme
//! declaree se fabrique de la forme lisible : `SorteDeFiche::deriver`), se verifie (`champs::verifier_fiche`) et se
//! compare case par case (ce qui change, ce qui se date : la valeur ecrite de chaque case, `ValeurDeCase`). Puis chaque
//! case qui change s'ecrit datee dans la fiche telle qu'ecrite (`ecrire_datee` choisit ce qui se date ;
//! `droit::fiche::dater`, par `FicheDatee::changer`, est le seul qui ecrit une valeur datee).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use declarations::champs::{self, Anomalie, Apercu, FicheAJuger, Gravite};
use import::proposee::FicheProposee;
use droit::contexte::{Contexte, IdSalarie, IdSpectacle};
use droit::fiche::{
    CaseDeFiche, CaseEntreprise, ChampsEntreprise, ChampsSalarie, ChampsSpectacle, FauteDeCase, FicheDatee, FicheEntrepriseDatee, FicheNommee, FicheSaisie, FicheSalarieDatee,
    FicheSpectacleDatee, Lue as _, MotEcrit, NomDeFiche, ValeurDeCase,
};
use socle::date::Jour;
use socle::echec::{erreur, Classe, Echec, R};

use super::{date_proposee, ecrire, plain, refus, DejaDeclare};
use import::proposee::A_COMPLETER;
use compagnie::avant::sauver_avant;

// ------------------------------------------------------------------ les trois fiches

/// UNE SORTE DE FICHE : sa forme datee, son nom (celui de `champs-base.json`), son fichier, et ce qui lui est propre.
pub(crate) trait SorteDeFiche: FicheDatee<Plate: FicheSaisie> {
    const NOM: NomDeFiche;
    /// La fiche, a juger par la table (`champs::verifier_fiche`).
    fn a_juger(p: &Self::Plate) -> FicheAJuger<'_>;
    /// La fiche que l'import propose, de cette sorte.
    fn proposee(p: Self::Plate) -> FicheProposee;
    /// La fiche de cette sorte qu'est la fiche proposee `f` ; d'une autre sorte, aucune.
    fn de_la_proposee(f: &mut FicheProposee) -> Option<&mut Self::Plate>;
    /// Aucune case n'y est ecrite (ses notes ne comptent pas).
    fn est_vide(p: &Self::Plate) -> bool;
    /// FABRIQUE LES FORMES DECLAREES DEPUIS LES FORMES LISIBLES (`deriver_par_la_table`, et ce qui est propre a la
    /// fiche), de ce que les cases `source` disent (a l'ecriture : celles que l'ecran change ; a la verification :
    /// toutes). Rend ce que le type d'une forme declaree refuse, sur la forme lisible dont elle se deduit.
    fn deriver(p: &mut Self::Plate, forcer: bool, source: &dyn Fn(CaseDeFiche) -> bool) -> R<Vec<FauteDeCase>> {
        deriver_par_la_table::<Self>(p, forcer, source)
    }
    /// La forme lisible `texte` de la case `cle` du bloc `bloc` (une personne de l'entreprise), comblee (`comblee`).
    fn combler_dans_un_bloc(_p: &mut Self::Plate, _bloc: droit::fiche::NomDeCase, _cle: CaseDeFiche, _texte: String) {}
    /// Les noms d'ou se fait la cle d'une fiche neuve, du plus court au plus long (`cle_neuve`).
    fn candidats_de_cle(_p: &Self::Plate) -> Vec<String> {
        Vec::new()
    }
    const FICHIER: &'static str;
    /// Les cases qui s'ecrivent a part (`ecrire_a_part`).
    fn a_part() -> Vec<CaseDeFiche> {
        Vec::new()
    }
    /// Le fichier qui la porte.
    type Fichier: Clone + Default + PartialEq + Serialize + serde::de::DeserializeOwned;
    /// La cle d'une fiche dans son fichier : `IdSalarie`, `IdSpectacle` ; l'entreprise, le fichier entier, n'en a pas
    /// (`()`).
    type Cle: Clone + PartialEq + std::fmt::Debug + Serialize + serde::de::DeserializeOwned;

    /// La cle telle qu'une phrase la dit.
    fn cle_dite(cle: &Self::Cle) -> String;
    /// La fiche que la cle nomme.
    fn nommee(cle: Self::Cle) -> FicheNommee;
    /// Le salarie que la cle nomme, pour une fiche de salarie.
    fn salarie(_cle: &Self::Cle) -> Option<&IdSalarie> {
        None
    }
    /// La cle d'une fiche neuve (`cle_neuve`) ; l'entreprise n'en a pas.
    fn nommer_neuve(d: &Self::Plate, existants: &Self::Fichier) -> R<Option<Self::Cle>>;
    /// La fiche `cle` du fichier, si elle existe.
    fn existante<'a>(f: &'a mut Self::Fichier, cle: Option<&Self::Cle>) -> Option<&'a mut Self>;
    /// La fiche `cle`, neuve si le fichier ne la porte pas encore.
    fn ou_neuve<'a>(f: &'a mut Self::Fichier, cle: Option<&Self::Cle>) -> R<&'a mut Self>;
    /// Le fichier porte-t-il la fiche `cle` ?
    fn contient(f: &Self::Fichier, cle: &Self::Cle) -> bool;
    /// La fiche telle que l'ecran l'a montree (`ctx.lue_au`), si elle existe.
    fn montree(ctx: &Contexte, cle: Option<&Self::Cle>) -> Option<Self::Plate>;
    /// Les cases qui different d'une fiche a l'autre.
    fn differentes(a: &Self::Plate, b: &Self::Plate) -> Vec<CaseDeFiche>;
    /// Les cases de la fiche, sans ses notes (ce que l'ecran montre).
    fn sans_notes(p: Self::Plate) -> Self::Plate;
    /// La fiche telle que le moteur la retient, pour ce qu'il lit d'une case qu'elle ne dit pas : « actif » d'un salarie
    /// ou d'un spectacle (`archivee`). L'ecran la montre ainsi, et l'ecriture part de ce qu'il a montre.
    fn retenue(p: Self::Plate) -> Self::Plate {
        p
    }

    /// Ce que le moteur pose dans la fiche changee `apres`, d'apres celle que l'ecran a montree (`avant`), avant tout
    /// controle (la forme d'un code risque, la notification d'un taux AT/MP que la saisie change).
    fn ajuster(_avant: &Self::Plate, _apres: &mut Self::Plate) {}
    /// Ce qu'une ecriture refuse, `forcer` ou non (un NIR, un matricule, un numero d'objet deja pris).
    fn refuser(_ctx: &Contexte, _cle: Option<&Self::Cle>, _avant: &Self::Plate, _apres: &Self::Plate) -> R<()> {
        Ok(())
    }
    /// Ce qu'une fiche neuve recoit du moteur (le matricule suivant).
    fn completer_neuve(_ctx: &Contexte, _plate: &mut Self::Plate) -> R<()> {
        Ok(())
    }
    /// La phrase a confirmer avant d'ecrire, sans date (le jour fixe du versement des intermittents).
    fn usage(_ctx: &Contexte, _avant: &Self::Plate, _apres: &Self::Plate) -> R<Option<String>> {
        Ok(None)
    }
    /// Ce qui se date a part (`a_part`), pour la boite « Ce qui change ».
    fn a_dater_a_part(_avant: &Self::Plate, _apres: &Self::Plate) -> Vec<ChangementDit> {
        Vec::new()
    }
    /// Les cases `a_part`, ecrites dans `brute` ; rend vrai si elles ont change.
    fn ecrire_a_part(_ctx: &Contexte, _brute: &mut Self, _apres: &Self::Plate, _depuis: Option<Jour>, _note: Option<&str>) -> R<bool> {
        Ok(false)
    }
    /// Ce que le moteur inscrit apres l'ecriture des cases `ecrites`, par la regle qui l'inscrit aussi ailleurs ; en
    /// phrases, que la reponse dit.
    fn apres_ecriture(_ctx: &Contexte, _ecrites: &[CaseDeFiche]) -> R<Vec<String>> {
        Ok(Vec::new())
    }
}

impl SorteDeFiche for FicheEntrepriseDatee {
    const NOM: NomDeFiche = NomDeFiche::Entreprise;
    const FICHIER: &'static str = droit::contexte::ENTREPRISE;
    type Fichier = FicheEntrepriseDatee;
    type Cle = ();

    fn a_part() -> Vec<CaseDeFiche> {
        vec![CaseEntreprise::Etablissements.into(), CaseEntreprise::TauxAtmp.into()]
    }

    fn cle_dite(_cle: &()) -> String {
        String::new()
    }
    fn nommee(_cle: ()) -> FicheNommee {
        FicheNommee::Entreprise
    }
    fn a_juger(p: &ChampsEntreprise) -> FicheAJuger<'_> {
        FicheAJuger::Entreprise(p)
    }
    fn proposee(p: ChampsEntreprise) -> FicheProposee {
        FicheProposee::Entreprise(p)
    }
    fn de_la_proposee(f: &mut FicheProposee) -> Option<&mut ChampsEntreprise> {
        match f {
            FicheProposee::Entreprise(p) => Some(p),
            FicheProposee::Salarie(_) | FicheProposee::Spectacle(_) => None,
        }
    }
    fn est_vide(p: &ChampsEntreprise) -> bool {
        p.est_vide()
    }
    fn combler_dans_un_bloc(p: &mut ChampsEntreprise, bloc: droit::fiche::NomDeCase, cle: CaseDeFiche, texte: String) {
        use droit::fiche::{CaseContactSanteTravail as C, CaseDeclarantDsn as D, CaseSignataire as S};
        if bloc == CaseEntreprise::Signataire.into() {
            if let Some(s) = p.signataire.as_mut() {
                if cle == S::NomAffichage.into() {
                    s.nom_affichage = Some(texte);
                } else if cle == S::PrenomAffichage.into() {
                    s.prenom_affichage = Some(texte);
                }
            }
        } else if bloc == CaseEntreprise::DeclarantDsn.into() {
            if let Some(d) = p.declarant_dsn.as_mut().filter(|_| cle == D::NomPrenomAffichage.into()) {
                d.nom_prenom_affichage = Some(texte);
            }
        } else if bloc == CaseEntreprise::ContactSanteTravail.into() {
            if let Some(c) = p.contact_sante_travail.as_mut().filter(|_| cle == C::NomPrenomAffichage.into()) {
                c.nom_prenom_affichage = Some(texte);
            }
        }
    }
    /// La table, puis le numero d'adherent Conges Spectacles, les personnes, le SIREN et le NIC, les autres
    /// etablissements.
    fn deriver(p: &mut ChampsEntreprise, forcer: bool, source: &dyn Fn(CaseDeFiche) -> bool) -> R<Vec<FauteDeCase>> {
        let mut fautes = deriver_par_la_table::<Self>(p, forcer, source)?;
        // Le numero d'adherent Conges Spectacles s'ecrit dans sa forme (`conges_spectacles::forme_adherent`) : « 0123
        // 450001-f » est la meme valeur que « 0123450001F ».
        if source(documents::conges_spectacles::CLE_ADHERENT.into()) {
            if let Some(brut) = p.congesspectacles_adherent.as_mut() {
                let forme = documents::conges_spectacles::forme_adherent(brut);
                if forme != *brut {
                    *brut = forme;
                }
            }
        }
        deriver_personnes(p, forcer, source);
        deriver_siren_nic(p, source);
        if source(CaseEntreprise::Etablissements.into()) {
            fautes.extend(deriver_etablissements(p)?);
        }
        Ok(fautes)
    }
    fn nommer_neuve(_d: &ChampsEntreprise, _existants: &Self) -> R<Option<()>> {
        Ok(None)
    }
    fn existante<'a>(f: &'a mut Self, _cle: Option<&()>) -> Option<&'a mut Self> {
        Some(f)
    }
    fn ou_neuve<'a>(f: &'a mut Self, _cle: Option<&()>) -> R<&'a mut Self> {
        Ok(f)
    }
    fn contient(_f: &Self, _cle: &()) -> bool {
        false
    }
    fn montree(ctx: &Contexte, _cle: Option<&()>) -> Option<ChampsEntreprise> {
        Some(crate::etablissements::entreprise_montree(ctx))
    }
    fn differentes(a: &ChampsEntreprise, b: &ChampsEntreprise) -> Vec<CaseDeFiche> {
        a.cases_differentes(b)
    }
    fn sans_notes(p: ChampsEntreprise) -> ChampsEntreprise {
        p.sans_notes()
    }
    /// LE TAUX AT/MP SAISI (`taux_atmp`, une ligne : `routes::atmp`) : le code risque s'ecrit sans point ni blanc
    /// (S21.G00.40.040, « 923AD » : sa saisie se range a sa lecture, `CodeRisqueAt::rangee`) ; la ligne est celle de la
    /// personne (`provenance`), sans notification.
    fn ajuster(avant: &ChampsEntreprise, apres: &mut ChampsEntreprise) {
        if apres.taux_atmp == avant.taux_atmp {
            return;
        }
        let Some(t) = apres.taux_atmp.as_mut() else { return };
        t.provenance = Some(droit::fiche::ProvenanceAtmp::Saisie);
        t.notification = None;
    }
    /// La ligne du taux AT/MP qui ne se lit pas comme la DSN le declare (`droit::bareme::taux_atmp_refuse`) ; une ligne
    /// ne se retire pas (une autre la remplace, a sa date).
    fn refuser(_ctx: &Contexte, _cle: Option<&()>, avant: &ChampsEntreprise, apres: &ChampsEntreprise) -> R<()> {
        if apres.taux_atmp == avant.taux_atmp {
            return Ok(());
        }
        let Some(t) = apres.taux_atmp.as_ref() else {
            return Err(refus("Accidents du travail (AT/MP) : une ligne ne se retire pas ; inscrivez celle qui la remplace, avec sa date. Rien n'a été enregistré."));
        };
        match droit::bareme::taux_atmp_refuse(t).map_err(|e| e.reclasser(Classe::Refuse))? {
            Some(m) => Err(refus(format!("Accidents du travail (AT/MP) : {} Rien n'a été enregistré.", m))),
            None => Ok(()),
        }
    }
    /// Le jour fixe du versement des intermittents ; le taux AT/MP recu par la DSN que la saisie change, a la date de la
    /// saisie (`droit::bareme::taux_atmp_a_confirmer`).
    fn usage(ctx: &Contexte, avant: &ChampsEntreprise, apres: &ChampsEntreprise) -> R<Option<String>> {
        let versement = apres
            .date_de_versement_intermittents
            .as_ref()
            .filter(|neuf| Some(*neuf) != avant.date_de_versement_intermittents.as_ref())
            .and_then(|neuf| neuf.a_confirmer_pour_les_intermittents());
        let atmp = match apres.taux_atmp.as_ref().filter(|_| apres.taux_atmp != avant.taux_atmp) {
            Some(t) => {
                let recue = t.depuis.and_then(|d| ctx.entreprise_au(&d).taux_atmp);
                droit::bareme::taux_atmp_a_confirmer(recue.as_ref(), t).map_err(|e| e.reclasser(Classe::Refuse))?
            }
            None => None,
        };
        let phrases: Vec<String> = versement.into_iter().chain(atmp).collect();
        Ok((!phrases.is_empty()).then(|| phrases.join(" ")))
    }
    fn a_dater_a_part(avant: &ChampsEntreprise, apres: &ChampsEntreprise) -> Vec<ChangementDit> {
        let liste = |e: &ChampsEntreprise| e.etablissements.iter().flatten().cloned().collect::<Vec<_>>();
        crate::etablissements::a_dater(&liste(avant), &liste(apres)).iter().map(crate::etablissements::Changement::dit).collect()
    }
    /// Les autres etablissements (`routes::etablissements`), et la ligne du taux AT/MP, a sa propre date
    /// (`ecrire_ligne_atmp`).
    fn ecrire_a_part(ctx: &Contexte, brute: &mut Self, apres: &ChampsEntreprise, depuis: Option<Jour>, note: Option<&str>) -> R<bool> {
        let montree = brute.au(&ctx.lue_au);
        let mut ecrit = false;
        if montree.etablissements != apres.etablissements {
            let avant = brute.etablissements.clone();
            let liste: Vec<droit::fiche::Etablissement> = apres.etablissements.iter().flatten().cloned().collect();
            crate::etablissements::ecrire(ctx, brute, apres, &liste, depuis, note)?;
            ecrit |= brute.etablissements != avant;
        }
        if montree.taux_atmp != apres.taux_atmp {
            if let Some(t) = apres.taux_atmp.clone() {
                ecrit |= ecrire_ligne_atmp(brute, t, note)?;
            }
        }
        Ok(ecrit)
    }
    /// UN CODE RISQUE « TC » INSCRIT SANS TAUX (siege ou autre etablissement) prend aussitot le taux collectif de
    /// l'arrete de l'annee, comme a l'ouverture (`routes::atmp::inscrire_l_arrete`, la meme regle et la meme ecriture) :
    /// la paie n'attend pas la prochaine ouverture.
    fn apres_ecriture(ctx: &Contexte, ecrites: &[CaseDeFiche]) -> R<Vec<String>> {
        if !ecrites.iter().any(|c| Self::a_part().contains(c)) {
            return Ok(Vec::new());
        }
        let mut dit = Vec::new();
        for an in [ctx.aujourdhui.an, ctx.aujourdhui.an + 1] {
            dit.extend(crate::atmp::inscrire_l_arrete(ctx, an)?);
        }
        Ok(dit)
    }
}

/// UNE LIGNE DU TAUX AT/MP DU SIEGE S'ECRIT A SA DATE (`depuis`, celle de la notification), dans la case datee
/// (`droit::fiche::dater`) : avant elle, la case garde ce qu'elle disait (vide, si elle ne disait rien) ; une ligne au
/// meme jour est remplacee (`FicheDatee::changer`, `premiere_datee`). La meme ligne n'ecrit rien. Le seul endroit qui
/// ecrit la case, quelle que soit la source
/// (`routes::atmp`). Rend vrai si elle a change.
pub(crate) fn ecrire_ligne_atmp(brute: &mut FicheEntrepriseDatee, t: droit::fiche::TauxAtmp, note: Option<&str>) -> R<bool> {
    let depuis = t.depuis.ok_or_else(|| refus("Accidents du travail (AT/MP) : la ligne dit à partir de quand elle vaut. Rien n'a été enregistré."))?;
    // La meme ligne ; ou une saisie qui redit la ligne que la notification ou l'arrete a ecrite : celle-ci garde sa
    // provenance.
    let deja = brute.taux_atmp.as_deref().and_then(|l| droit::fiche::valeur_au(l, &depuis));
    if deja.as_ref() == Some(&t) || (t.provenance == Some(droit::fiche::ProvenanceAtmp::Saisie) && deja.as_ref().is_some_and(|d| droit::bareme::meme_taux_atmp(d, &t))) {
        return Ok(false);
    }
    droit::fiche::FicheDatee::changer(brute, droit::fiche::ChangementEntreprise::TauxAtmp(Some(t)), Some(&depuis), true, note);
    Ok(true)
}

/// UNE LIGNE DU TAUX AT/MP QU'UNE SOURCE DONNE (l'import, la notification recue, l'arrete : `routes::atmp`), ecrite
/// dans la fiche entreprise (`ecrire_ligne_atmp`), l'ancien fichier sauvegarde. Rend vrai si elle a change.
pub(crate) fn inscrire_taux_atmp(ctx: &Contexte, t: droit::fiche::TauxAtmp, note: &str) -> R<bool> {
    let mut contenu = lire_fichier::<FicheEntrepriseDatee>(ctx)?;
    let ecrit = ecrire_ligne_atmp(&mut contenu, t, Some(note))?;
    if ecrit {
        ecrire_fichier::<FicheEntrepriseDatee>(ctx, &contenu)?;
    }
    Ok(ecrit)
}

/// LES LIGNES DES AUTRES ETABLISSEMENTS QUE LE MOTEUR CHANGE (le taux collectif de l'arrete, `routes::atmp`), ecrites
/// comme celles de l'ecran (`routes::etablissements::ecrire`), a partir de `depuis`, l'ancien fichier sauvegarde.
pub(crate) fn inscrire_etablissements(ctx: &Contexte, liste: &[droit::fiche::Etablissement], depuis: Jour, note: &str) -> R<()> {
    let mut contenu = lire_fichier::<FicheEntrepriseDatee>(ctx)?;
    let siege = contenu.au(&depuis);
    crate::etablissements::ecrire(ctx, &mut contenu, &siege, liste, Some(depuis), Some(note))?;
    ecrire_fichier::<FicheEntrepriseDatee>(ctx, &contenu)
}

impl SorteDeFiche for FicheSalarieDatee {
    const NOM: NomDeFiche = NomDeFiche::Salarie;
    const FICHIER: &'static str = droit::contexte::SALARIES;
    type Fichier = droit::fiche::Salaries;
    type Cle = IdSalarie;

    fn cle_dite(cle: &IdSalarie) -> String {
        cle.to_string()
    }
    fn nommee(cle: IdSalarie) -> FicheNommee {
        FicheNommee::Salarie(cle)
    }
    fn salarie(cle: &IdSalarie) -> Option<&IdSalarie> {
        Some(cle)
    }
    fn a_juger(p: &ChampsSalarie) -> FicheAJuger<'_> {
        FicheAJuger::Salarie(p)
    }
    fn proposee(p: ChampsSalarie) -> FicheProposee {
        FicheProposee::Salarie(p)
    }
    fn de_la_proposee(f: &mut FicheProposee) -> Option<&mut ChampsSalarie> {
        match f {
            FicheProposee::Salarie(p) => Some(p),
            FicheProposee::Entreprise(_) | FicheProposee::Spectacle(_) => None,
        }
    }
    fn est_vide(p: &ChampsSalarie) -> bool {
        p.est_vide()
    }
    /// La table, puis le departement de naissance, les initiales du contrat et la nationalite DSN : le departement et les
    /// initiales ne s'ecrivent que dans une case vide, quelles que soient les cases `source`.
    fn deriver(p: &mut ChampsSalarie, forcer: bool, source: &dyn Fn(CaseDeFiche) -> bool) -> R<Vec<FauteDeCase>> {
        use droit::fiche::CaseSalarie as S;
        let mut fautes = deriver_par_la_table::<Self>(p, forcer, source)?;
        // LE DEPARTEMENT DE NAISSANCE SE LIT DANS LE NIR (regle 2 : la page le montre, le moteur l'ecrit) : ses 6e et 7e
        // caracteres sont le departement de naissance, 99 pour une personne nee a l'etranger (S21.G00.30.001 et .014,
        // `atelier/reference/norme-dsn/texte-2027/bloc-S21.G00.30.txt`). Il ne s'ecrit que dans une case vide : une
        // valeur saisie qui le contredit se dit (`champs::verifier_fiche`), elle ne se remplace pas.
        if p.departement_naissance.is_none() {
            if let Some(dep) = p.nir.dite().and_then(|n| champs::departement_du_nir(n.as_ref())) {
                match FauteDeCase::pose::<droit::fiche::ChangementSalarie>(S::DepartementNaissance.into(), &dep) {
                    Ok(ch) => FicheSalarieDatee::appliquer(p, ch),
                    Err(f) => fautes.push(f),
                }
            }
        }
        // Les initiales du contrat se lisent dans le nom declare, lui-meme derive du nom saisi.
        if p.initiales_contrat.remplie().is_none() {
            let lettres: String = p.nom.portee().into_iter().flat_map(str::chars).filter(|c| c.is_ascii_uppercase()).take(2).collect();
            if !lettres.is_empty() {
                p.initiales_contrat = Some(lettres);
            }
        }
        // LA NATIONALITE DSN SE DEDUIT DU PAYS : avec un pays de nationalite de la liste, la case 30.013 ne se choisit
        // plus, le moteur l'ecrit (`droit::nationalite::codification`). Sans pays, ou avec un pays inconnu (qui se dit sur
        // sa case), la case reste ce qu'elle est.
        if source(S::PaysNationalite.into()) {
            if let Some(code) = p.pays_nationalite.dite() {
                if let Some(pays) = socle::pays::du_code(code.as_ref())? {
                    let attendu = droit::nationalite::codification(&pays);
                    if p.code_nationalite_dsn != Some(attendu) {
                        p.code_nationalite_dsn = Some(attendu);
                    }
                }
            }
        }
        Ok(fautes)
    }
    /// Le nom (declare, sinon lisible), puis le nom et le prenom.
    fn candidats_de_cle(p: &ChampsSalarie) -> Vec<String> {
        let dit = |t: Option<&str>| t.map(str::trim).filter(|t| !t.is_empty()).map(|t| propre(t, false));
        let base = dit(p.nom.portee()).or_else(|| dit(p.nom_affichage.portee()));
        let prenom = dit(p.prenom.portee()).or_else(|| dit(p.prenom_affichage.portee()));
        let ensemble = base.as_ref().map(|b| format!("{}-{}", b, prenom.iter().cloned().collect::<String>()));
        base.into_iter().chain(ensemble).collect()
    }
    fn nommer_neuve(d: &ChampsSalarie, existants: &droit::fiche::Salaries) -> R<Option<IdSalarie>> {
        cle_neuve::<Self>(d, existants, IdSalarie::lire).map(Some)
    }
    fn existante<'a>(f: &'a mut droit::fiche::Salaries, cle: Option<&IdSalarie>) -> Option<&'a mut Self> {
        f.fiches.get_mut(cle?)
    }
    fn ou_neuve<'a>(f: &'a mut droit::fiche::Salaries, cle: Option<&IdSalarie>) -> R<&'a mut Self> {
        let cle = cle.ok_or_else(|| refus("Une fiche de salarié se nomme par sa clé."))?;
        Ok(f.fiches.entry(cle.clone()).or_default())
    }
    fn contient(f: &droit::fiche::Salaries, cle: &IdSalarie) -> bool {
        f.fiches.contains_key(cle)
    }
    fn montree(ctx: &Contexte, cle: Option<&IdSalarie>) -> Option<ChampsSalarie> {
        ctx.salaries.get(cle?).map(|f| Self::retenue(f.champs.clone()))
    }
    fn differentes(a: &ChampsSalarie, b: &ChampsSalarie) -> Vec<CaseDeFiche> {
        a.cases_differentes(b)
    }
    fn sans_notes(p: ChampsSalarie) -> ChampsSalarie {
        p.sans_notes()
    }
    fn retenue(p: ChampsSalarie) -> ChampsSalarie {
        ChampsSalarie { actif: Some(!p.archivee()), ..p }
    }
    fn refuser(ctx: &Contexte, cle: Option<&IdSalarie>, avant: &ChampsSalarie, apres: &ChampsSalarie) -> R<()> {
        // LE MATRICULE D'UN SALARIE SANS NIR NE SE CHANGE PAS : il finit son numero technique temporaire.
        if let (Some(_), Some(a), Some(b)) = (cle, avant.code_salarie.remplie(), apres.code_salarie.remplie()) {
            if a != b && apres.nir.portee().is_none() && avant.nir.portee().is_none() {
                return Err(refus(format!(
                    "Le matricule de cette fiche passerait de {} a {}, et le salarie n'a pas de NIR : il est declare sous un numero \
                     technique temporaire qui FINIT par son matricule. Le changer ferait apparaitre un deuxieme individu dans la DSN, \
                     que rien ne rattacherait au premier. Saisir d'abord le NIR notifie par la caisse.",
                    a, b
                )));
            }
        }
        // UN NIR, UNE PERSONNE : `forcer` ne leve pas ce refus.
        if let Some(nir) = apres.nir.dite() {
            let autres: Vec<&str> =
                ctx.salaries.iter().filter(|(k, s)| Some(*k) != cle && s.nir.dite() == Some(nir)).map(|(k, _)| k.as_str()).collect();
            if !autres.is_empty() {
                return Err(refus(format!(
                    "Le NIR {} est déjà celui de la fiche {}. Un NIR désigne une personne et une seule : complétez ou réactivez cette \
                     fiche-là plutôt que d'en créer une seconde.",
                    nir,
                    autres.join(", ")
                )));
            }
        }
        // UN MATRICULE, UN SALARIE : la DSN rapproche chaque salarie de ses declarations par son matricule (S21.G00.30.019,
        // dont « le choix de la valeur revient a l'employeur », `norme-dsn/texte/bloc-S21.G00.30.txt`, l. 327-329) ; deux
        // fiches au meme matricule s'y confondraient. `forcer` ne leve pas ce refus.
        if let Some(matricule) = apres.code_salarie.remplie().map(str::trim).filter(|m| !m.is_empty()) {
            let porteur = ctx.salaries.iter().find(|(k, s)| Some(*k) != cle && s.code_salarie.portee().is_some_and(|m| m.trim() == matricule));
            if let Some((k, s)) = porteur {
                let qui = match s.nomme() {
                    Some(n) => n,
                    None => k.to_string(),
                };
                return Err(refus(format!(
                    "Le matricule {} est déjà celui de {} (fiche {}). Un matricule désigne un salarié et un seul : la DSN \
rapproche chacun de ses déclarations par lui. En choisir un autre, ou laisser la case vide pour prendre le suivant.",
                    matricule, qui, k
                )));
            }
        }
        Ok(())
    }
    fn completer_neuve(ctx: &Contexte, plate: &mut ChampsSalarie) -> R<()> {
        // Le matricule d'une fiche neuve : le suivant. Un matricule saisi est garde tel quel. L'ANCIENNETE PART DE
        // L'EMBAUCHE, pas du jour de la fiche : la validation du premier engagement la pose (`poser_anciennete_a_l_embauche`).
        if plate.code_salarie.remplie().map(str::trim).is_none_or(str::is_empty) {
            plate.code_salarie = Some(droit::fiche::Matricule::lire(&matricule_suivant(&lire_fichier::<FicheSalarieDatee>(ctx)?)?.to_string())?);
        }
        Ok(())
    }
}

impl SorteDeFiche for FicheSpectacleDatee {
    const NOM: NomDeFiche = NomDeFiche::Spectacle;
    const FICHIER: &'static str = droit::contexte::SPECTACLES;
    type Fichier = droit::fiche::Spectacles;
    type Cle = IdSpectacle;

    fn cle_dite(cle: &IdSpectacle) -> String {
        cle.to_string()
    }
    fn nommee(cle: IdSpectacle) -> FicheNommee {
        FicheNommee::Spectacle(cle)
    }
    fn a_juger(p: &ChampsSpectacle) -> FicheAJuger<'_> {
        FicheAJuger::Spectacle(p)
    }
    fn proposee(p: ChampsSpectacle) -> FicheProposee {
        FicheProposee::Spectacle(p)
    }
    fn de_la_proposee(f: &mut FicheProposee) -> Option<&mut ChampsSpectacle> {
        match f {
            FicheProposee::Spectacle(p) => Some(p),
            FicheProposee::Entreprise(_) | FicheProposee::Salarie(_) => None,
        }
    }
    fn est_vide(p: &ChampsSpectacle) -> bool {
        p.est_vide()
    }
    /// Le titre (pas « A COMPLETER »), sinon le numero d'objet.
    fn candidats_de_cle(p: &ChampsSpectacle) -> Vec<String> {
        let titre = p.titre.portee().map(str::trim).filter(|t| !t.is_empty() && !t.eq_ignore_ascii_case(A_COMPLETER));
        match (titre, p.numero_objet.dite()) {
            (Some(t), _) => vec![propre(t, true)],
            (None, Some(n)) => vec![format!("objet-{}", n)],
            (None, None) => Vec::new(),
        }
    }
    fn nommer_neuve(d: &ChampsSpectacle, existants: &droit::fiche::Spectacles) -> R<Option<IdSpectacle>> {
        cle_neuve::<Self>(d, existants, IdSpectacle::lire).map(Some)
    }
    fn existante<'a>(f: &'a mut droit::fiche::Spectacles, cle: Option<&IdSpectacle>) -> Option<&'a mut Self> {
        f.fiches.get_mut(cle?)
    }
    fn ou_neuve<'a>(f: &'a mut droit::fiche::Spectacles, cle: Option<&IdSpectacle>) -> R<&'a mut Self> {
        let cle = cle.ok_or_else(|| refus("Une fiche de spectacle se nomme par sa clé."))?;
        Ok(f.fiches.entry(cle.clone()).or_default())
    }
    fn contient(f: &droit::fiche::Spectacles, cle: &IdSpectacle) -> bool {
        f.fiches.contains_key(cle)
    }
    fn montree(ctx: &Contexte, cle: Option<&IdSpectacle>) -> Option<ChampsSpectacle> {
        ctx.spectacles.get(cle?).cloned().map(Self::retenue)
    }
    fn differentes(a: &ChampsSpectacle, b: &ChampsSpectacle) -> Vec<CaseDeFiche> {
        a.cases_differentes(b)
    }
    fn sans_notes(p: ChampsSpectacle) -> ChampsSpectacle {
        p.sans_notes()
    }
    fn retenue(p: ChampsSpectacle) -> ChampsSpectacle {
        ChampsSpectacle { actif: Some(!p.archivee()), ..p }
    }
    fn refuser(ctx: &Contexte, cle: Option<&IdSpectacle>, _avant: &ChampsSpectacle, apres: &ChampsSpectacle) -> R<()> {
        // UN NUMERO D'OBJET, UN SPECTACLE : France Travail l'attribue « pour chaque spectacle ou chaque production » et il
        // « permet d'identifier toutes les personnes ayant travaille » dessus (S21.G00.40.050,
        // `norme-dsn/texte/bloc-S21.G00.40.txt`, l. 1379-1385) ; deux fiches au meme numero se confondraient dans la DSN.
        // `forcer` ne leve pas ce refus.
        let Some(numero) = apres.numero_objet.dite() else { return Ok(()) };
        let porteur = ctx.spectacles.iter().find(|(k, s)| Some(*k) != cle && s.numero_objet.dite().is_some_and(|n| n.meme(numero)));
        if let Some((k, s)) = porteur {
            let qui = match s.titre.remplie().filter(|t| !t.trim().is_empty() && !t.trim().eq_ignore_ascii_case(A_COMPLETER)) {
                Some(t) => format!("du spectacle « {} » (fiche {})", t, k),
                None => format!("de la fiche {}", k),
            };
            return Err(refus(format!(
                "Le numéro d'objet {} est déjà celui {}. Un numéro d'objet désigne un spectacle et un seul : complétez \
cette fiche-là, ou corrigez le numéro.",
                numero, qui
            )));
        }
        Ok(())
    }
}

/// Le fichier de la fiche `F` de la compagnie, lu par sa forme ; absent, il est vide (une compagnie sans fiche encore).
pub(crate) fn lire_fichier<F: SorteDeFiche>(ctx: &Contexte) -> R<F::Fichier> {
    let chemin = ctx.racine.join(F::FICHIER);
    if !chemin.exists() {
        return Ok(F::Fichier::default());
    }
    droit::fiche::lire_fichier(&chemin, F::FICHIER)
}

/// Le fichier ecrit, l'ancien sauvegarde.
fn ecrire_fichier<F: SorteDeFiche>(ctx: &Contexte, fichier: &F::Fichier) -> R<()> {
    let chemin = ctx.racine.join(F::FICHIER);
    sauver_avant(&chemin)?;
    ecrire(&chemin, fichier)
}

/// Le prochain matricule (S21.G00.30.019, que l'employeur choisit : `norme-dsn/texte-2027/bloc-S21.G00.30.txt`) : le
/// plus grand code salarie numerique que le fichier ait jamais porte, plus un. Chaque valeur datee compte.
fn matricule_suivant(salaries: &droit::fiche::Salaries) -> R<i64> {
    let codes = salaries.fiches.values().flat_map(|s| s.code_salarie.iter().flatten()).filter_map(|e| e.valeur.portee().map(str::trim));
    let numeriques: Vec<i64> = codes
        .filter(|c| !c.is_empty() && c.chars().all(|x| x.is_ascii_digit()))
        .map(|c| c.parse::<i64>().map_err(|e| refus(format!("Matricule illisible : {} ({}).", c, e))))
        .collect::<R<_>>()?;
    Ok(numeriques.into_iter().fold(0, i64::max) + 1)
}

// ------------------------------------------------------------------ la saisie

/// LA FICHE `plate` TELLE QUE L'ECRAN LA MONTRE, sa forme lisible comblee (`champs::a_combler`) : une fiche d'avant ne
/// porte que la forme declaree, et l'ecran propose une forme lisible de depart. Rien n'est ecrit pour autant.
pub(crate) fn comblee<F: SorteDeFiche>(plate: &F::Plate) -> R<F::Plate> {
    let mut montree = plate.clone();
    for c in champs::a_combler(plate)? {
        match c.bloc {
            None => match FauteDeCase::pose::<F::Changement>(c.cle, &c.texte) {
                Ok(ch) => F::appliquer(&mut montree, ch),
                Err(f) => return Err(erreur(Classe::Refuse, format!("La forme lisible « {} » ne se lit pas : {}", c.cle, f.phrase))),
            },
            Some(b) => F::combler_dans_un_bloc(&mut montree, b, c.cle, c.texte),
        }
    }
    Ok(montree)
}

/// UNE FICHE TAPEE, lue par sa porte (`FicheSaisie::lire_cases`) : la fiche typee, et les fautes de ses cases.
pub(crate) struct Tapee<P> {
    pub(crate) fiche: P,
    pub(crate) fautes: Vec<FauteDeCase>,
}

impl<'de, P: FicheSaisie> Deserialize<'de> for Tapee<P> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Tapee<P>, D::Error> {
        let o = socle::forme::Objet::deserialize(d)?;
        match P::lire_cases(&o) {
            Ok((fiche, fautes)) => Ok(Tapee { fiche, fautes }),
            Err(e) => Err(serde::de::Error::custom(e.phrase)),
        }
    }
}

/// UNE FICHE EN COURS DE SAISIE (`verifier-fiche`) :`{"nom": …, "donnees": {…}}`, chaque case typee a la porte.
#[derive(Deserialize)]
#[serde(tag = "nom", content = "donnees", deny_unknown_fields)]
pub(crate) enum FicheTapee {
    #[serde(rename = "entreprise")]
    Entreprise(Tapee<ChampsEntreprise>),
    #[serde(rename = "salarie")]
    Salarie(Tapee<ChampsSalarie>),
    #[serde(rename = "spectacle")]
    Spectacle(Tapee<ChampsSpectacle>),
}

/// LA FICHE QUE L'ECRAN MONTRE EN CE MOMENT (`permanents`) : `{"nom": …, "donnees": {…}}`, et pour un salarie deja
/// enregistre sa cle (`cle`) : les cases d'un permanent qui lui est du (`engagement::permanents`), et celles que le
/// metier d'un de ses engagements demande, se montrent (`fiches::montres`). Une case fautive y est vide.
#[derive(Deserialize)]
#[serde(tag = "nom", deny_unknown_fields)]
pub(crate) enum FicheEnCours {
    #[serde(rename = "entreprise")]
    Entreprise { donnees: Tapee<ChampsEntreprise> },
    #[serde(rename = "salarie")]
    Salarie { cle: Option<IdSalarie>, donnees: Tapee<ChampsSalarie> },
    #[serde(rename = "spectacle")]
    Spectacle { donnees: Tapee<ChampsSpectacle> },
}

// ------------------------------------------------------------------ la derivation

/// LA FORME DECLAREE DE CHAQUE LIGNE DE `champs-base.json` QUI SE DEDUIT D'UNE FORME LISIBLE (`derive_de`) dont la case
/// est une `source` : le texte lisible normalise (`champs::normaliser`), lu par le type de la case declaree (la porte
/// d'un changement, `FauteDeCase::pose`). Sans `forcer`, seul ce qui est vide ou ne respecte pas sa norme se refait. Ce que
/// le type refuse est la faute de la case declaree, dite sur sa forme lisible (`champs::des_fautes`).
fn deriver_par_la_table<F: SorteDeFiche>(p: &mut F::Plate, forcer: bool, source: &dyn Fn(CaseDeFiche) -> bool) -> R<Vec<FauteDeCase>> {
    let mut fautes = Vec::new();
    for c in champs::fiche(F::NOM)? {
        let Some(src) = c.derive_de.filter(|s| source(*s)) else { continue };
        let MotEcrit::Mot(lisible) = p.ecrite(src) else { continue };
        if lisible.is_empty() {
            continue;
        }
        let attendu = champs::normaliser(&c, &lisible)?;
        match p.ecrite(c.cle) {
            MotEcrit::Mot(t) if t == attendu => continue,
            MotEcrit::Mot(t) if !t.is_empty() && !forcer && champs::faute_de_norme(&c, &t)?.is_none() => continue,
            _ => {}
        }
        match FauteDeCase::pose::<F::Changement>(c.cle, &attendu) {
            Ok(ch) => F::appliquer(p, ch),
            Err(f) => fautes.push(f),
        }
    }
    Ok(fautes)
}

/// Sans accents, en capitales, espaces resserrees.
fn capitales(t: &str) -> String {
    let t: String = champs::sans_accents(t).chars().flat_map(char::to_uppercase).collect();
    t.split(socle::texte::blanc).filter(|m| !m.is_empty()).collect::<Vec<_>>().join(" ")
}

/// Le texte dit d'une case texte, sans ses blancs autour.
fn texte(t: &Option<String>) -> Option<&str> {
    t.portee().map(str::trim).filter(|t| !t.is_empty())
}

/// Les formes declarees du signataire et des contacts. LE CONTACT DSN ET CELUI DE LA MEDECINE DU TRAVAIL INTERNE : la
/// norme les ecrit en « nom et prenom » en une seule rubrique (S20.G00.07.001, `norme-dsn/texte/bloc-S20.G00.07.txt`
/// l. 63-71) ; la fiche les saisit en une case (`nom_prenom_affichage`), dont la forme declaree est la meme en capitales
/// sans accents.
fn deriver_personnes(p: &mut ChampsEntreprise, forcer: bool, source: &dyn Fn(CaseDeFiche) -> bool) {
    let poser = |declaree: &mut Option<String>, lisible: &Option<String>| {
        if let Some(l) = texte(lisible).map(capitales) {
            if forcer || texte(declaree).is_none() {
                *declaree = Some(l);
            }
        }
    };
    if source(CaseEntreprise::Signataire.into()) {
        if let Some(s) = p.signataire.as_mut() {
            poser(&mut s.nom, &s.nom_affichage.clone());
            poser(&mut s.prenom, &s.prenom_affichage.clone());
        }
    }
    if source(CaseEntreprise::DeclarantDsn.into()) {
        if let Some(d) = p.declarant_dsn.as_mut() {
            poser(&mut d.nom_prenom, &d.nom_prenom_affichage.clone());
        }
    }
    if source(CaseEntreprise::ContactSanteTravail.into()) {
        if let Some(c) = p.contact_sante_travail.as_mut() {
            poser(&mut c.nom_prenom, &c.nom_prenom_affichage.clone());
        }
    }
}

/// LE SIREN ET LE NIC SE DEDUISENT DU SIRET (regle 2 : la page ne les pose pas) : le SIRET est le SIREN (9 chiffres) suivi
/// du NIC (5 chiffres) -- c'est sa definition (INSEE), aucune valeur ne se devine. Un SIRET hors de sa forme s'est refuse
/// a la porte.
fn deriver_siren_nic(p: &mut ChampsEntreprise, source: &dyn Fn(CaseDeFiche) -> bool) {
    if !source(CaseEntreprise::Siret.into()) {
        return;
    }
    let Some(siret) = p.siret.clone() else { return };
    let (siren, nic) = (siret.siren(), siret.nic());
    if p.siren.as_ref() != Some(&siren) {
        p.siren = Some(siren);
    }
    if p.nic.as_ref() != Some(&nic) {
        p.nic = Some(nic);
    }
}

/// LES AUTRES ETABLISSEMENTS S'ECRIVENT COMME LE SIEGE : le NIC se lit dans le SIRET (ses cinq derniers chiffres),
/// l'adresse et la commune declarees se fabriquent depuis leur forme lisible comme celles du siege (`champs::normaliser`
/// de sa ligne), puis se lisent par leur type (ce qu'il refuse se dit, l'etablissement nomme) ; un versement mobilite
/// sans aucune case dite n'en est pas un. Le code risque et le taux se sont types a la porte.
fn deriver_etablissements(p: &mut ChampsEntreprise) -> R<Vec<FauteDeCase>> {
    use droit::fiche::{CaseEntreprise as E, CommuneEmployeur, LigneAdresse};
    let mut fautes = Vec::new();
    let Some(lignes) = p.etablissements.as_mut() else { return Ok(fautes) };
    let fiche = champs::fiche(NomDeFiche::Entreprise)?;
    let regle = |cle: E| fiche.iter().find(|c| c.cle == cle.into()).cloned();
    let (adresse, commune) = (regle(E::Adresse), regle(E::Commune));
    for o in lignes.iter_mut() {
        if let Some(siret) = o.siret.as_ref() {
            o.nic = Some(siret.nic());
        }
        let qui = match texte(&o.libelle) {
            Some(q) => q.to_string(),
            None => "un établissement".to_string(),
        };
        let mut faute = |f: Option<String>| {
            if let Some(f) = f {
                fautes.push(FauteDeCase { case: E::Etablissements.into(), chemin: None, phrase: format!("{} : {}", qui, f) });
            }
        };
        if let (Some(c), Some(t)) = (adresse.as_ref(), texte(&o.adresse_affichage)) {
            let ecrit = champs::normaliser(c, t)?;
            match LigneAdresse::lire(&ecrit) {
                Ok(l) => o.adresse = Some(l),
                Err(e) => faute(Some(e.phrase)),
            }
        }
        if let (Some(c), Some(t)) = (commune.as_ref(), texte(&o.commune_affichage)) {
            let ecrit = champs::normaliser(c, t)?;
            match CommuneEmployeur::lire(&ecrit) {
                Ok(l) => o.commune = Some(l),
                Err(e) => faute(Some(e.phrase)),
            }
        }
        // Les taux d'une ligne arrivent de la page en texte (« 0.5 », « 1,5 % ») : ils s'ecrivent en nombre, comme la
        // fiche les porte deja, sinon une ligne renvoyee telle quelle se dirait changee et s'ecrirait en texte.
        if let Some(t) = o.taux_atmp.as_mut() {
            faute(montant_saisi(&mut t.valeur, "Taux AT/MP"));
        }
        if let Some(vm) = o.versement_mobilite.as_mut() {
            faute(montant_saisi(&mut vm.taux, "Taux du versement mobilité"));
        }
        if o.versement_mobilite.as_ref().is_some_and(droit::fiche::VersementMobilite::est_vide) {
            o.versement_mobilite = None;
        }
    }
    Ok(fautes)
}

/// UN MONTANT SAISI EN TEXTE (`12,5`, `1.5 %`) S'ECRIT EN NOMBRE, dans sa forme la plus courte, comme un nombre lu du
/// fichier ; un texte blanc ne dit rien. Rend la faute d'un texte qui n'est pas un nombre.
fn montant_saisi(m: &mut Option<droit::fiche::Montant>, quoi: &str) -> Option<String> {
    let Some(droit::fiche::Montant::Texte(brut)) = m.as_ref() else { return None };
    let brut = brut.replace('%', "");
    let brut = brut.trim();
    if brut.is_empty() {
        *m = None;
        return None;
    }
    let Some(d) = socle::num::decimal_a_virgule(brut) else { return Some(format!("{} illisible : {}.", quoi, brut)) };
    match socle::forme::lire_texte::<droit::fiche::Montant>(&socle::num::court(d).to_string(), "Le nombre ne se lit pas.") {
        Ok(lu) => {
            *m = Some(lu);
            None
        }
        Err(e) => Some(format!("{} illisible : {} ({}).", quoi, brut, e.phrase)),
    }
}

// ------------------------------------------------------------------ ce qui change

/// UNE CASE VIDE : sans valeur, un texte blanc ou « A COMPLETER » (le titre d'un spectacle importe sans nom), une liste
/// ou une structure vides. La remplir n'est pas un changement a dater.
pub(crate) fn case_vide(v: Option<&ValeurDeCase>) -> bool {
    v.is_none_or(|v| match v.ecrite() {
        MotEcrit::Mot(t) => t.trim().is_empty() || t.trim().eq_ignore_ascii_case(A_COMPLETER),
        MotEcrit::Nombre(_) => false,
        MotEcrit::Vide | MotEcrit::StructureVide => true,
        MotEcrit::OuiNon(_) => false,
        // Une liste, ou une structure dont une case est ecrite.
        MotEcrit::Structure => false,
    })
}

/// Sans valeur : une case vide (`case_vide`), une case a cocher non cochee, une structure dont aucune case n'a de
/// valeur. La fiche les montre vides ; un enregistrement qui les laisse ainsi ne change rien.
fn sans_valeur(v: Option<&ValeurDeCase>) -> bool {
    case_vide(v)
        || match v.map(|x| (x.ecrite(), x.cases())) {
            Some((MotEcrit::OuiNon(b), _)) => !b,
            Some((_, Some(cases))) => cases.iter().all(|(_, x)| sans_valeur(x.as_ref())),
            // Une valeur simple non vide, une liste non vide (`case_vide` a dit le reste) : elle a une valeur.
            Some((_, None)) | None => false,
        }
}

/// LA MEME VALEUR, a la forme pres : un nombre ecrit en texte (100 et « 100 »), une structure sans ses formes lisibles
/// (`lisibles` : les cases que l'ecran comble ne se declarent pas, `champs::formes_lisibles`) ni ses cases laissees
/// vides.
fn meme(a: &ValeurDeCase, b: &ValeurDeCase, lisibles: &[CaseDeFiche]) -> bool {
    if a == b {
        return true;
    }
    if let (Some(x), Some(y)) = (a.cases(), b.cases()) {
        let dites = |s: Vec<(String, Option<ValeurDeCase>)>| -> Vec<(String, ValeurDeCase)> {
            s.into_iter().filter(|(k, v)| !lisibles.iter().any(|l| l.nom() == k) && !sans_valeur(v.as_ref())).filter_map(|(k, v)| v.map(|v| (k, v))).collect()
        };
        let (x, y) = (dites(x), dites(y));
        return x.len() == y.len() && x.iter().all(|(k, v)| y.iter().any(|(l, w)| l == k && meme(v, w, lisibles)));
    }
    // Deux valeurs simples se comparent par leur nombre ou leur mot ; une structure ou une liste d'un cote seulement
    // n'est pas la meme valeur.
    let simple = |v: &ValeurDeCase| matches!(v.ecrite(), MotEcrit::Mot(_) | MotEcrit::Nombre(_) | MotEcrit::OuiNon(_));
    simple(a)
        && simple(b)
        && match (a.nombre(), b.nombre()) {
            (Some(x), Some(y)) => x == y,
            _ => a.ecrite().mot().is_some() && a.ecrite() == b.ecrite(),
        }
}

fn meme_case(a: Option<&ValeurDeCase>, b: Option<&ValeurDeCase>, lisibles: &[CaseDeFiche]) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => meme(a, b, lisibles),
        (a, b) => a.is_none() && b.is_none(),
    }
}

/// La structure `apres` garde chaque case de `avant` qui a une valeur (elle n'en remplit que des vides).
fn seulement_completee(avant: &ValeurDeCase, apres: &ValeurDeCase, lisibles: &[CaseDeFiche]) -> bool {
    // Autre chose qu'une structure, d'un cote ou de l'autre : rien ne s'y complete.
    let (Some(a), Some(b)) = (avant.cases(), apres.cases()) else { return false };
    a.iter().filter(|(_, x)| !sans_valeur(x.as_ref())).all(|(k, x)| match (x, b.iter().find(|(l, _)| l == k).and_then(|(_, y)| y.as_ref())) {
        (Some(x), Some(y)) => meme(x, y, lisibles),
        _ => false,
    })
}

/// La case avait une valeur : la changer est un changement, a dater. Une case a cocher que la fiche dit a la sienne,
/// cochee ou non.
fn avait_une_valeur(avant: Option<&ValeurDeCase>) -> bool {
    avant.is_some_and(|v| matches!(v.ecrite(), MotEcrit::OuiNon(_))) || !sans_valeur(avant)
}

/// LES FAITS QUI NE SE DATENT PAS : une case qui dit un fait du passe, vrai de tout temps, et non une valeur qui vaut a
/// partir d'un jour. La date de la premiere embauche de la compagnie (CSS L. 130-1, II, l'effectif d'une entreprise de
/// moins de cinq ans) : la changer la corrige, elle ne change pas a une date. De meme l'anciennete d'un salarie, le jour
/// ou il est entre dans l'entreprise (« Dans la compagnie depuis », d'ou se compte S21.G00.86.003 a la fin de chaque mois
/// declare : `norme-dsn/texte-2027/bloc-S21.G00.86.txt`, l. 91-95). Elles s'ecrivent sans date d'effet.
fn faits_sans_date() -> [(NomDeFiche, CaseDeFiche); 2] {
    [(NomDeFiche::Entreprise, CaseEntreprise::PremiereEmbauche.into()), (NomDeFiche::Salarie, droit::fiche::CaseSalarie::DateAnciennete.into())]
}

/// LES CASES `cles` QUI CHANGENT LA FICHE `avant` (telle que l'ecran l'a montree) en `apres` ET DEMANDENT LEUR DATE
/// D'EFFET (modele, « Le temps » : toute la configuration est datee, et une valeur sans date rend le passe faux des
/// qu'elle change) : une valeur deja dite qui change, toujours ; une case que la fiche ne dit pas est vide. UNE CASE VIDE
/// QU'ON REMPLIT NE SE DATE PAS : elle comble un manque, sa premiere valeur vaut de tout temps ; ce qui est parti ne se
/// refait pas (regle 4), seul ce qui reste a faire la lit. Sauf `premiere_datee` : une valeur qui, par nature, ne vaut
/// qu'a partir d'un jour (le taux de PAS recu, « le taux personnalise ne s'applique que pour l'avenir »,
/// BOI-IR-PAS-30-10-50 § 20, `bofip/BOI-IR-PAS-30-10-50-20190227.html`). Une fiche neuve n'a rien a dater.
fn changements_a_dater<F: SorteDeFiche>(avant: &F::Plate, apres: &F::Plate, cles: &[CaseDeFiche], premiere_datee: bool) -> R<Vec<CaseDeFiche>> {
    let nom = F::NOM;
    if F::est_vide(avant) {
        return Ok(Vec::new());
    }
    let sans_date = faits_sans_date();
    let lisibles = champs::formes_lisibles(nom)?;
    Ok(cles
        .iter()
        .copied()
        .filter(|k| {
            let dite = avant.valeur_ecrite(*k).filter(|a| !case_vide(Some(a)));
            let a = dite.as_ref();
            let apres = apres.valeur_ecrite(*k);
            let b = apres.as_ref();
            !sans_date.contains(&(nom, *k))
                && !(a.is_some() && b.is_some() && meme_case(a, b, &lisibles))
                && !(sans_valeur(a) && sans_valeur(b))
                && (premiere_datee || avait_une_valeur(a))
                // Une structure dont on remplit des cases vides sans changer celles qui ont une valeur : c'est leur
                // premiere valeur, comme une case vide.
                && (premiere_datee || !matches!((a, b), (Some(a), Some(b)) if seulement_completee(a, b, &lisibles)))
        })
        .collect())
}

/// ECRIRE UNE FICHE DATEE, la seule fonction qui choisit ce qui se date dans une fiche : chaque case qui change de
/// `avant` (la fiche a `lue_au`, celle que l'ecran a montree) a `apres` s'ecrit dans la fiche datee `brute` (`FicheDatee::changer`,
/// `droit::fiche::dater`). Ce qui change une valeur existante vaut a partir de `depuis`, et sans lui se refuse : ce qui
/// precede garde l'ancienne valeur (`changements_a_dater`). Une case vide qu'on remplit prend sa valeur de tout temps,
/// `depuis` ou non ; une valeur `premiere_datee` (le taux de PAS recu) vaut a partir de `depuis` meme dans une case vide :
/// avant lui, la fiche ne disait rien. Les notes s'ecrivent telles quelles. Les cases `a_part` s'ecrivent a part. Rend
/// les cases ecrites.
pub(crate) fn ecrire_datee<F: SorteDeFiche>(
    brute: &mut F,
    avant: &F::Plate,
    apres: &F::Plate,
    premiere_datee: bool,
    depuis: Option<Jour>,
    note: Option<&str>,
    lue_au: &Jour,
) -> R<Vec<CaseDeFiche>> {
    let cles: Vec<CaseDeFiche> = F::differentes(avant, apres).into_iter().filter(|k| !F::a_part().contains(k)).collect();
    let a_dater = changements_a_dater::<F>(avant, apres, &cles, premiere_datee)?;
    if depuis.is_none() && !a_dater.is_empty() {
        return Err(refus(format!(
            "Changer {} demande sa date d'effet (« a partir du ») : ce qui precede garde l'ancienne valeur, et la date ne \
             se devine pas. Rien n'a ete enregistre.",
            libelles_a_dater(F::NOM, &a_dater)?.join(", ")
        )));
    }
    let mut ecritures: Vec<(F::Changement, Option<Jour>, bool)> = Vec::new();
    let mut ecrites = Vec::new();
    let lisibles = champs::formes_lisibles(F::NOM)?;
    for k in cles {
        let (a, b) = (avant.valeur_ecrite(k), apres.valeur_ecrite(k));
        let (a, b) = (a.as_ref(), b.as_ref());
        let rien = b.is_none_or(|v| v.ecrite() == MotEcrit::Mot(String::new()) || (v.cases().is_some() && sans_valeur(Some(v))));
        if (case_vide(a) && rien) || (a.is_some() && b.is_some() && meme_case(a, b, &lisibles)) {
            continue;
        }
        let jours = brute.depuis(k);
        let datee = depuis.is_some() && a_dater.contains(&k);
        let jour = if datee { depuis } else { jours.iter().copied().filter(|j| j.is_none_or(|j| j <= *lue_au)).last().flatten() };
        let ch = F::changement(apres, k).ok_or_else(|| plain(format!("La case {} n'est pas de la fiche {}.", k, F::NOM)))?;
        // Datee, la valeur ne vaut qu'a partir de `depuis`, meme dans une case vide : avant lui, la fiche ne disait rien.
        ecritures.push((ch, jour, datee));
        ecrites.push(k);
    }
    for (ch, jour, datee) in ecritures {
        droit::fiche::FicheDatee::changer(brute, ch, jour.as_ref(), datee, note);
    }
    brute.noter(apres);
    Ok(ecrites)
}

/// Les libelles des cases a dater, sans un champ calcule qui change avec la case d'ou il derive (le meme fait).
fn libelles_a_dater(nom: NomDeFiche, a_dater: &[CaseDeFiche]) -> R<Vec<String>> {
    let fiche = champs::fiche(nom)?;
    let derive_d_un_change = |k: &CaseDeFiche| fiche.iter().find(|c| c.cle == *k).and_then(|c| c.derive_de).is_some_and(|s| a_dater.contains(&s));
    Ok(a_dater
        .iter()
        .filter(|k| !derive_d_un_change(k))
        .map(|k| match fiche.iter().find(|c| c.cle == *k) {
            Some(c) => format!("« {} »", c.libelle),
            None => format!("« {} »", k),
        })
        .collect())
}

// ------------------------------------------------------------------ poser des cases

/// POSER DES CASES D'UNE FICHE QUI EXISTE (un geste qui ne touche que quelques cases : le modele de bulletin, la case des
/// permanents, le compte des DPAE, l'anciennete d'un salarie, une fiche archivee, ce que l'import complete) : face a la
/// fiche telle qu'elle est sur le disque a `lue_au` (`ecrire_datee`), l'ancien fichier sauvegarde. Ni derivation ni
/// verification : chaque case est deja dans sa forme. `cle` : celle d'un salarie ou d'un spectacle, `None` pour
/// l'entreprise.
pub(crate) fn poser<F: SorteDeFiche>(ctx: &Contexte, cle: Option<&F::Cle>, changements: Vec<F::Changement>, depuis: Option<Jour>, note: Option<&str>) -> R<()> {
    poser_au::<F>(ctx, cle, changements, false, depuis, note, &ctx.lue_au)
}

/// `poser`, face a la fiche lue a `lue_au` ; `premiere_datee` comme `ecrire_datee`.
pub(crate) fn poser_au<F: SorteDeFiche>(ctx: &Contexte, cle: Option<&F::Cle>, changements: Vec<F::Changement>, premiere_datee: bool, depuis: Option<Jour>, note: Option<&str>, lue_au: &Jour) -> R<()> {
    let mut contenu = lire_fichier::<F>(ctx)?;
    let Some(brute) = F::existante(&mut contenu, cle) else {
        return Err(refus(format!("La fiche {} {} n'existe pas : rien n'est écrit.", F::NOM, cle.map(F::cle_dite).into_iter().collect::<String>())));
    };
    let avant = brute.au(lue_au);
    let mut apres = avant.clone();
    for ch in changements {
        F::appliquer(&mut apres, ch);
    }
    ecrire_datee::<F>(brute, &avant, &apres, premiere_datee, depuis, note, lue_au)?;
    ecrire_fichier::<F>(ctx, &contenu)
}

// ------------------------------------------------------------------ enregistrer une fiche

/// CE QUE L'ENREGISTREMENT D'UNE FICHE PASSE OUTRE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Forcer {
    /// Rien : une erreur refuse la fiche.
    Rien,
    /// Ce que l'ecran peut forcer (« Enregistrer quand meme ») : une erreur de forme (`forcable`), jamais ce que le type
    /// d'une case refuse.
    Forme,
    /// Tout : l'import et le questionnaire du salarie posent une fiche a completer, qui le dit (« A COMPLETER »).
    Tout,
}

impl Forcer {
    /// Ces anomalies refusent-elles la fiche ?
    fn refuse(self, anomalies: &[Anomalie]) -> bool {
        let mut erreurs = anomalies.iter().filter(|a| a.gravite == Gravite::Erreur);
        match self {
            Forcer::Rien => erreurs.next().is_some(),
            Forcer::Forme => erreurs.any(|a| !a.forcable),
            Forcer::Tout => false,
        }
    }
}

/// Ce qu'un refus de fiche porte a part : ses anomalies, et leur nombre en mots.
#[derive(Serialize)]
struct FicheRefusee {
    anomalies: Vec<Anomalie>,
    erreurs_dit: String,
}

/// La fiche porte au moins une erreur : elle n'est pas ecrite.
fn fiche_refusee(nom: NomDeFiche, anomalies: &[Anomalie]) -> R<Echec> {
    let mut phrases = Vec::new();
    for a in anomalies.iter().filter(|a| a.gravite == Gravite::Erreur) {
        phrases.push(match champs::champ(nom, a.champ)?.map(|c| c.libelle).filter(|l| !l.is_empty()) {
            Some(l) => format!("« {} » : {}", l, a.message),
            None => a.message.clone(),
        });
    }
    let erreurs_dit = if phrases.len() > 1 { format!("{} erreurs", phrases.len()) } else { "Une erreur".to_string() };
    let message = format!("La fiche n'est pas écrite : {}", phrases.join(" ; "));
    Ok(Echec { a_part: Some(socle::echec::APart::forme(FicheRefusee { anomalies: anomalies.to_vec(), erreurs_dit })), ..erreur(Classe::FicheRefusee, message) })
}

/// Pose sur chaque anomalie le libelle de sa case, celui que la fiche montre.
pub(crate) fn avec_libelles(nom: NomDeFiche, anomalies: &mut [Anomalie]) -> R<()> {
    let champs = champs::fiche(nom)?;
    for a in anomalies.iter_mut() {
        // Le code PCS-ESE ne se tape pas : la fiche le remplit du metier choisi (`emplois.rs`).
        if a.champ == droit::fiche::CaseSalarie::PcsEse.into() {
            a.libelle = Some("Métier (il donne le code PCS-ESE)".to_string());
        } else if let Some(c) = champs.iter().find(|c| c.cle == a.champ) {
            a.libelle = Some(c.libelle.clone());
        }
    }
    Ok(())
}

/// UNE FICHE PRETE A S'ECRIRE : telle que l'ecran l'a montree (`avant`), et telle qu'elle sera, ses changements poses
/// et derivee (`apres`), avec les cases que l'ecran a nommees.
pub(crate) struct Preparee<F: SorteDeFiche> {
    cle: Option<F::Cle>,
    avant: F::Plate,
    apres: F::Plate,
}

/// PREPARER L'ECRITURE de la fiche `cle` (`None` : une fiche neuve, ou l'entreprise) : la fiche telle que l'ecran l'a
/// montree, ce que `changer` y pose (il rend les cases qu'il nomme), ce que la fiche refuse quoi qu'on force, puis la
/// derivation des formes declarees de ce que ces cases disent.
pub(crate) fn preparer<F: SorteDeFiche>(ctx: &Contexte, cle: Option<&F::Cle>, changer: impl FnOnce(&mut F::Plate) -> Vec<CaseDeFiche>) -> R<Preparee<F>> {
    let mut avant = F::Plate::default();
    if let Some(p) = F::montree(ctx, cle) {
        avant = p;
    }
    let mut apres = avant.clone();
    let nommees = changer(&mut apres);
    F::ajuster(&avant, &mut apres);
    F::refuser(ctx, cle, &avant, &apres)?;
    if cle.is_none() {
        F::completer_neuve(ctx, &mut apres)?;
    }
    // Ce que le type d'une forme declaree refuse refuse la fiche, quoi qu'on force : elle ne se relirait pas.
    let fautes = F::deriver(&mut apres, true, &|c| nommees.contains(&c))?;
    if !fautes.is_empty() {
        return Err(fiche_refusee(F::NOM, &champs::des_fautes(F::NOM, &fautes)?)?);
    }
    Ok(Preparee { cle: cle.cloned(), avant, apres })
}

/// Ce que l'ecriture d'une fiche a fait : sa cle (celle d'une fiche neuve est fabriquee ici), les anomalies, si le
/// fichier a change (une fiche enregistree telle qu'elle est n'ecrit rien), et les cases ecrites.
pub(crate) struct FicheEcrite<K> {
    pub(crate) cle: Option<K>,
    pub(crate) anomalies: Vec<Anomalie>,
    pub(crate) ecrit: bool,
    pub(crate) champs: Vec<CaseDeFiche>,
}

/// ECRIRE UNE FICHE PREPAREE : verifiee entiere, telle qu'elle sera (une erreur la refuse, sauf ce que `forcer` passe
/// outre), puis chaque case qui change datee (`ecrire_datee`), et l'ancien fichier sauvegarde.
pub(crate) fn ecrire_fiche<F: SorteDeFiche>(ctx: &Contexte, p: Preparee<F>, forcer: Forcer, depuis: Option<Jour>, note: Option<&str>) -> R<FicheEcrite<F::Cle>> {
    let anomalies = champs::verifier_fiche(F::a_juger(&p.apres))?;
    if forcer.refuse(&anomalies) {
        return Err(fiche_refusee(F::NOM, &anomalies)?);
    }
    let mut contenu = lire_fichier::<F>(ctx)?;
    let avant_fichier = contenu.clone();
    let cle = match p.cle {
        // UNE FICHE NEUVE : la cle se fabrique ici et nulle part ailleurs.
        None => F::nommer_neuve(&p.apres, &contenu)?,
        cle => cle,
    };
    let brute = F::ou_neuve(&mut contenu, cle.as_ref())?;
    let mut champs = ecrire_datee::<F>(brute, &p.avant, &p.apres, false, depuis, note, &ctx.lue_au)?;
    if F::a_part().iter().any(|k| F::differentes(&p.avant, &p.apres).contains(k)) && F::ecrire_a_part(ctx, brute, &p.apres, depuis, note)? {
        champs.extend(F::a_part());
    }
    let ecrit = contenu != avant_fichier;
    if ecrit {
        ecrire_fichier::<F>(ctx, &contenu)?;
    }
    Ok(FicheEcrite { cle, anomalies, ecrit, champs })
}

/// La cle d'une fiche neuve, lisible et unique dans son fichier : le nom du salarie (puis son prenom), le titre du
/// spectacle (ou son numero d'objet, quand il n'en a pas encore).
/// Un nom en cle : sans accents ni apostrophes, chaque autre signe un tiret, en capitales (ou en minuscules : `bas`).
fn propre(texte: &str, bas: bool) -> String {
    let t = champs::sans_accents(texte).replace(['\'', '\u{2019}'], "");
    let t: String = t.chars().map(|c| if envois::etats::lettre_ou_chiffre(c) { c } else { '-' }).collect();
    let t = t.split('-').filter(|m| !m.is_empty()).collect::<Vec<_>>().join("-");
    if bas {
        t.to_lowercase()
    } else {
        t.to_uppercase()
    }
}

fn cle_neuve<F: SorteDeFiche>(d: &F::Plate, existants: &F::Fichier, lire: fn(&str) -> R<F::Cle>) -> R<F::Cle> {
    let candidats: Vec<String> = F::candidats_de_cle(d).into_iter().filter(|c| !c.is_empty() && !c.starts_with('-') && !c.ends_with('-')).collect();
    let Some(dernier) = candidats.last() else {
        return Err(refus(format!("Impossible de nommer cette fiche {} : il lui faut au moins un {}.", F::NOM, if F::NOM == NomDeFiche::Salarie { "nom" } else { "titre" })));
    };
    for c in &candidats {
        let cle = lire(c)?;
        if !F::contient(existants, &cle) {
            return Ok(cle);
        }
    }
    let mut n = 2;
    loop {
        let cle = lire(&format!("{}-{}", dernier, n))?;
        if !F::contient(existants, &cle) {
            return Ok(cle);
        }
        n += 1;
    }
}

// ------------------------------------------------------------------ l'avertissement

/// UNE VALEUR QUI CHANGE, telle que la boite « Ce qui change » la montre : la case, son libelle, avant et apres ; ce
/// que la valeur sortante a deja servi a declarer ; la forme lisible d'une forme declaree.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ChangementDit {
    champ: String,
    libelle: String,
    avant: Option<ValeurDeCase>,
    apres: Option<ValeurDeCase>,
    #[serde(skip_serializing_if = "Option::is_none")]
    deja_declare: Option<DejaDeclare>,
    #[serde(skip_serializing_if = "Option::is_none")]
    avant_lisible: Option<ValeurDeCase>,
    #[serde(skip_serializing_if = "Option::is_none")]
    apres_lisible: Option<ValeurDeCase>,
}

impl ChangementDit {
    /// Un changement dit en mots (une ligne d'un autre etablissement).
    pub(crate) fn en_mots(champ: &str, libelle: String, avant: String, apres: String) -> ChangementDit {
        ChangementDit { champ: champ.to_string(), libelle, avant: Some(ValeurDeCase::texte(avant)), apres: Some(ValeurDeCase::texte(apres)), deja_declare: None, avant_lisible: None, apres_lisible: None }
    }
}

/// L'AVERTISSEMENT D'UN ENREGISTREMENT DE FICHE : ce qu'il ferait changer (chaque valeur existante qui change demande sa
/// date d'effet), et la date que le moteur propose (`date_proposee`, `depuis` et `pourquoi`) ; et ce qui se confirme sans
/// se dater (`usage`).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Avertissement {
    changements: Vec<ChangementDit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    depuis: Option<Jour>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pourquoi: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    usage: Option<String>,
}

/// L'avertissement d'une fiche preparee ; `None` : rien n'est a dater ni a confirmer. N'ecrit rien.
fn avertissement<F: SorteDeFiche>(ctx: &Contexte, p: &Preparee<F>) -> R<Option<Avertissement>> {
    let usage = F::usage(ctx, &p.avant, &p.apres)?;
    let cles: Vec<CaseDeFiche> = F::differentes(&p.avant, &p.apres).into_iter().filter(|k| !F::a_part().contains(k)).collect();
    let a_dater = changements_a_dater::<F>(&p.avant, &p.apres, &cles, false)?;
    let mut changements = changements_dits::<F>(ctx, &p.avant, &p.apres, &a_dater)?;
    if !F::est_vide(&p.avant) {
        changements.extend(F::a_dater_a_part(&p.avant, &p.apres));
    }
    if changements.is_empty() {
        return Ok(usage.map(|u| Avertissement { changements, depuis: None, pourquoi: None, usage: Some(u) }));
    }
    let (depuis, pourquoi) = date_proposee(ctx, F::NOM, p.cle.as_ref().and_then(F::salarie))?;
    Ok(Some(Avertissement { changements, depuis: Some(depuis), pourquoi: Some(pourquoi), usage }))
}

/// Chaque valeur a dater, telle que la boite la montre ; un champ declare dit aussi ce que sa valeur sortante a deja
/// servi a declarer.
fn changements_dits<F: SorteDeFiche>(ctx: &Contexte, avant: &F::Plate, apres: &F::Plate, a_dater: &[CaseDeFiche]) -> R<Vec<ChangementDit>> {
    let fiche = champs::fiche(F::NOM)?;
    // La forme lisible de depart d'une fiche qui n'en a pas (`champs::a_combler`), que l'ecran lui a montree.
    let combles = champs::a_combler(avant)?;
    let lisible_avant = |source: CaseDeFiche| match combles.iter().find(|c| c.bloc.is_none() && c.cle == source) {
        Some(c) => Some(ValeurDeCase::texte(c.texte.clone())),
        None => avant.valeur_ecrite(source),
    };
    let mut sortie = Vec::new();
    for k in a_dater {
        let c = fiche.iter().find(|c| c.cle == *k);
        // Un champ calcule qui change avec la case d'ou il derive est le meme fait : la case le dit seule.
        if c.filter(|c| c.calcule()).and_then(|c| c.derive_de).is_some_and(|s| a_dater.contains(&s)) {
            continue;
        }
        let libelle = match c {
            Some(c) => c.libelle.clone(),
            None => k.to_string(),
        };
        let mut dit = ChangementDit { champ: k.to_string(), libelle, avant: avant.valeur_ecrite(*k), apres: apres.valeur_ecrite(*k), deja_declare: None, avant_lisible: None, apres_lisible: None };
        if let Some(c) = c.filter(|c| c.declare) {
            let valeur = avant.ecrite(*k);
            dit.deja_declare = Some(super::usages_declares(ctx, c.rubrique, valeur.mot())?);
        }
        // La forme lisible d'une forme declaree : c'est elle que la boite montre.
        if let Some(source) = c.filter(|c| c.calcule()).and_then(|c| c.derive_de) {
            if let Some(l) = fiche.iter().find(|x| x.cle == source) {
                dit.libelle = l.libelle.clone();
            }
            dit.avant_lisible = lisible_avant(source);
            dit.apres_lisible = apres.valeur_ecrite(source);
        }
        sortie.push(dit);
    }
    Ok(sortie)
}

/// Une valeur en mots, pour une phrase : vide, un texte, oui ou non, des mots, les cases d'une structure.
fn en_mots(v: Option<&ValeurDeCase>) -> String {
    match v {
        None => "vide".to_string(),
        Some(v) => v.en_mots(),
    }
}

/// L'avertissement en phrases : ce qui se confirme, chaque valeur qui change, puis la date proposee et ce qui l'ecrit.
fn avertissement_en_phrases(a: &Avertissement) -> Vec<String> {
    let mut lignes: Vec<String> = a.usage.iter().cloned().collect();
    lignes.extend(a.changements.iter().map(|c| {
        let (avant, apres) = match (&c.avant_lisible, &c.apres_lisible) {
            (None, None) => (&c.avant, &c.apres),
            _ => (&c.avant_lisible, &c.apres_lisible),
        };
        format!("{} : {} devient {}.", c.libelle, en_mots(avant.as_ref()), en_mots(apres.as_ref()))
    }));
    match (&a.depuis, &a.pourquoi) {
        (Some(depuis), Some(pourquoi)) => {
            lignes.push(format!("À partir du {}. {} La même charge avec « confirme » l'écrit (« depuis » pour une autre date).", depuis.jj_mm_aaaa(), pourquoi))
        }
        _ => lignes.push("La même charge avec « confirme » l'écrit.".to_string()),
    }
    lignes
}

// ------------------------------------------------------------------ la route enregistrer-fiche

/// La charge de `enregistrer-fiche`, pour une fiche : sa cle (absente : une fiche neuve, ou l'entreprise), ses
/// changements, la date d'effet de ce qui change et pourquoi, et ce qui est lu et passe outre.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "K: serde::de::DeserializeOwned"))]
pub(crate) struct Enregistrement<K> {
    cle: Option<K>,
    /// Chacun se lit seul dans le type de sa case (`droit::fiche::Changements::lire`) : toutes les fautes ensemble.
    changements: socle::forme::Liste,
    depuis: Option<Jour>,
    note: Option<String>,
    forcer: Option<bool>,
    confirme: Option<bool>,
}

/// La charge de `enregistrer-fiche` : la fiche nommee (`nom`) et son enregistrement.
#[derive(Deserialize)]
#[serde(tag = "nom", deny_unknown_fields)]
pub(crate) enum ChargeEnregistrer {
    #[serde(rename = "entreprise")]
    Entreprise(Enregistrement<()>),
    #[serde(rename = "salarie")]
    Salarie(Enregistrement<IdSalarie>),
    #[serde(rename = "spectacle")]
    Spectacle(Enregistrement<IdSpectacle>),
}

/// Ce que rend `enregistrer-fiche` : l'avertissement (rien n'est ecrit), ou ce qui l'est.
#[derive(Serialize)]
#[serde(untagged)]
pub(crate) enum FicheEnregistree<K> {
    Avertie { avertissement: Avertissement, phrase: String, phrases: Vec<String> },
    Ecrite { cle: Option<K>, ecrits: Vec<CaseDeFiche>, #[serde(with = "socle::paris::ecrit::seconde::option")] enregistre_a: Option<socle::paris::Moment>, enregistre_dit: Option<String>, phrase: String },
}

impl<K: Serialize> crate::enveloppe::Dite for FicheEnregistree<K> {
    fn phrase(&self) -> &str {
        match self {
            FicheEnregistree::Avertie { phrase, .. } | FicheEnregistree::Ecrite { phrase, .. } => phrase,
        }
    }
}

/// `enregistrer-fiche` : une fiche de l'ecran, ecrite (`ecrire_fiche`). Une valeur existante qui change demande sa date
/// d'effet : sans `confirme`, la route rend l'avertissement (`avertissement`) et n'ecrit rien ; la meme charge avec
/// `confirme` ecrit, a partir de `depuis` (la date proposee, s'il manque).
pub(crate) fn enregistrer_fiche(ctx: &Contexte, charge: ChargeEnregistrer) -> R<crate::enveloppe::Rendu> {
    use crate::enveloppe::en_json;
    match charge {
        ChargeEnregistrer::Entreprise(e) => en_json(enregistrer::<FicheEntrepriseDatee>(ctx, e)?),
        ChargeEnregistrer::Salarie(e) => en_json(enregistrer::<FicheSalarieDatee>(ctx, e)?),
        ChargeEnregistrer::Spectacle(e) => en_json(enregistrer::<FicheSpectacleDatee>(ctx, e)?),
    }
}

fn enregistrer<F: SorteDeFiche>(ctx: &Contexte, e: Enregistrement<F::Cle>) -> R<FicheEnregistree<F::Cle>> {
    let Enregistrement { cle, changements, depuis, note, forcer, confirme } = e;
    // UNE CASE QUE SON TYPE REFUSE refuse la fiche, avec toutes les autres : rien n'est ecrit.
    let (changements, fautes) = droit::fiche::Changements::<F::Changement>::lire(&changements).map_err(|e| e.reclasser(Classe::MalFormee))?;
    if !fautes.is_empty() {
        return Err(fiche_refusee(F::NOM, &champs::des_fautes(F::NOM, &fautes)?)?);
    }
    let p = preparer::<F>(ctx, cle.as_ref(), |plate| {
        changements
            .0
            .into_iter()
            .map(|ch| {
                let k = F::case_changee(&ch);
                F::appliquer(plate, ch);
                k
            })
            .collect()
    })?;
    let forcer = forcer == Some(true);
    let mut depuis = depuis;
    if let Some(a) = avertissement(ctx, &p)? {
        if confirme != Some(true) {
            let phrase = if a.depuis.is_some() { "Ce qui change se date : rien n'est écrit." } else { "À confirmer : rien n'est écrit." };
            return Ok(FicheEnregistree::Avertie { phrases: avertissement_en_phrases(&a), avertissement: a, phrase: phrase.to_string() });
        }
        depuis = depuis.or(a.depuis);
    }
    let neuve = F::NOM != NomDeFiche::Entreprise && cle.is_none();
    let note = note.as_deref().map(str::trim).filter(|n| !n.is_empty());
    let e = ecrire_fiche::<F>(ctx, p, if forcer { Forcer::Forme } else { Forcer::Rien }, depuis, note)?;
    let en_erreur = e.anomalies.iter().any(|a| a.gravite == Gravite::Erreur);
    // Une fiche creee se nomme ensuite par sa cle (la saisie d'une carte, `cle` d'une autre ecriture) : la reponse la rend.
    let phrase = match (e.ecrit, neuve, forcer && en_erreur) {
        (false, _, _) => "Rien n'a changé : rien n'est écrit.",
        (true, _, true) => "Enregistré malgré les erreurs.",
        (true, true, false) => "Fiche créée.",
        (true, false, false) => "Enregistré.",
    };
    // « enregistré à HH:MM » : l'heure du moteur (`temps.rs`), pas celle du navigateur (au banc, le temps est simule).
    let enregistre_a = e.ecrit.then(|| socle::temps::secondes().a_la_seconde());
    let enregistre_dit = enregistre_a.map(|a| {
        let s = a.seconde_du_jour_de_paris();
        format!("enregistré à {:02}:{:02}", s / 3_600, s / 60 % 60)
    });
    let inscrit = if e.ecrit { F::apres_ecriture(ctx, &e.champs)? } else { Vec::new() };
    let phrase = std::iter::once(phrase.to_string()).chain(inscrit).collect::<Vec<_>>().join(" ");
    Ok(FicheEnregistree::Ecrite { cle: e.cle, ecrits: e.champs, enregistre_a, enregistre_dit, phrase })
}

// ------------------------------------------------------------------ la route verifier-fiche

/// Ce que le moteur PROPOSERAIT pour une case encore vide, et pourquoi.
#[derive(Debug, Clone, Serialize)]
struct Suggestion {
    valeur: String,
    pourquoi: &'static str,
}

/// Ce que rend `verifier-fiche` : les anomalies de la fiche telle qu'elle SERAIT derivee, ce qui serait declare, ce que
/// le moteur proposerait pour les cases vides.
#[derive(Serialize)]
pub(crate) struct FicheVerifiee {
    phrase: String,
    phrases: Vec<String>,
    anomalies: Vec<Anomalie>,
    declare: Apercu,
    suggestions: BTreeMap<CaseDeFiche, Suggestion>,
}

impl crate::enveloppe::Dite for FicheVerifiee {
    fn phrase(&self) -> &str {
        &self.phrase
    }
}

/// `verifier-fiche` : la fiche tapee, chaque case typee a sa porte (`FicheTapee`), derivee (`SorteDeFiche::deriver`, sans
/// forcer : seul ce qui est vide ou faux se refait), verifiee comme a l'ecriture : les anomalies de la table, puis les
/// fautes de ses cases (ce que leur type a refuse, a la porte ou a la derivation), toutes ensemble, chacune sur sa case.
/// Une case que la fiche ne decrit pas a refuse la charge a la porte.
pub(crate) fn verifier_fiche(t: FicheTapee) -> R<FicheVerifiee> {
    match t {
        FicheTapee::Entreprise(t) => verifier::<FicheEntrepriseDatee>(t),
        FicheTapee::Salarie(t) => verifier::<FicheSalarieDatee>(t),
        FicheTapee::Spectacle(t) => verifier::<FicheSpectacleDatee>(t),
    }
}

fn verifier<F: SorteDeFiche>(t: Tapee<F::Plate>) -> R<FicheVerifiee> {
    let mut derivee = t.fiche.clone();
    let mut fautes = t.fautes;
    fautes.extend(F::deriver(&mut derivee, false, &|_| true)?);
    let mut anomalies = champs::verifier_fiche(F::a_juger(&derivee))?;
    anomalies.extend(champs::des_fautes(F::NOM, &fautes)?);
    let phrases: Vec<String> = anomalies.iter().map(|a| format!("{} : {}", a.champ, a.message)).collect();
    let phrase = if phrases.is_empty() { "La fiche est juste.".to_string() } else { format!("Anomalies de la fiche : {}.", phrases.len()) };
    Ok(FicheVerifiee { phrase, phrases, anomalies, declare: champs::apercu_derive(&t.fiche)?, suggestions: suggestions(F::a_juger(&t.fiche))? })
}

/// Ce que le moteur PROPOSERAIT pour une case encore vide.
fn suggestions(fiche: FicheAJuger<'_>) -> R<BTreeMap<CaseDeFiche, Suggestion>> {
    use droit::fiche::CaseSalarie as S;
    let mut propose = BTreeMap::new();
    if let FicheAJuger::Salarie(d) = fiche {
        if let Some(dep) = d.nir.dite().and_then(|n| champs::departement_du_nir(n.as_ref())) {
            let mut trouvees = vec![(S::DepartementNaissance, dep.clone(), d.departement_naissance.is_some())];
            if dep != "99" {
                trouvees.push((S::PaysNaissance, "FR".to_string(), d.pays_naissance.is_some()));
            }
            for (cle, valeur, dite) in trouvees {
                if !dite {
                    propose.insert(cle.into(), Suggestion { valeur, pourquoi: "lu dans le numéro de sécurité sociale" });
                }
            }
        }
    } else if let FicheAJuger::Entreprise(d) = fiche {
        let code = match (texte(&d.code_urssaf), d.code_postal.dite()) {
            (None, Some(cp)) => declarations::urssaf::base()?.du_code_postal(cp.as_ref()),
            _ => None,
        };
        if let Some(code) = code {
            propose.insert(
                CaseEntreprise::CodeUrssaf.into(),
                Suggestion {
                    valeur: code.to_string(),
                    pourquoi: "l'organisme du département, d'après votre code postal — à corriger si le vôtre diffère (versement en lieu unique, pôle grandes entreprises, GUSO)",
                },
            );
        }
    }
    Ok(propose)
}
