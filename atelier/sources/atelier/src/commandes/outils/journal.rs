//! `moteur-atelier journal --dpae [--ecrire]` : remettre au journal des envois les DPAE que les certificats de l'Urssaf
//! (nature 41, `retours/dpae/`) attestent (`envois::reprise_journal::reprise_dpae`) -- des DPAE faites par un autre
//! logiciel ou un autre poste. Sans `--ecrire`, l'outil DIT ce qu'il ferait. Rien ne part, aucun reseau. La reprise des
//! DSN est la route `journal-reprise`.

use moteur::commandes::analyse::{Args, Opt, Spec};
use moteur::commandes::Cadre;
use socle::echec::Echec;

pub static SPEC: Spec = Spec { commande: "journal", positionnels: &[], options: &[Opt::drapeau("dpae"), Opt::drapeau("ecrire")] };

pub fn executer(cadre: &Cadre, args: &Args) -> i32 {
    if !args.drapeau("dpae") {
        dire!("journal --dpae [--ecrire] : la reprise des DPAE attestees. Les DSN : la route journal-reprise.");
        return 2;
    }
    super::code(faire(cadre, args.drapeau("ecrire")))
}

fn faire(cadre: &Cadre, ecrire: bool) -> Result<i32, Echec> {
    let maintenant = socle::paris::maintenant_naif().iso();
    let d = envois::reprise_journal::reprise_dpae(&cadre.ctx, ecrire, "atelier", &maintenant)?;
    for x in &d.inconnus {
        dire!("Non rattache : {}", x);
    }
    if d.trouves.is_empty() {
        dire!("Aucun certificat de DPAE (nature 41) dans retours/dpae/ : rien a reprendre.");
        return Ok(0);
    }
    dire!("DPAE attestees par les certificats de l'Urssaf : {}\n", d.trouves.len());
    for v in &d.trouves {
        let a = &v.attestee;
        dire!(
            "  {}  {:<4}  {:<8}  {}{}",
            super::dit(a.quand.as_deref()),
            v.mode.nom(),
            super::dit(a.mois.map(|m| m.to_string()).as_deref()),
            a.flux,
            if v.deja { "   [deja au journal]" } else { "" }
        );
        for e in &a.embauches {
            dire!("        {} a partir du {}", e.salarie, e.debut.iso());
        }
    }
    if d.ajoutes.is_empty() {
        dire!("\nRien de nouveau : le journal connait deja tout cela.");
    } else if ecrire {
        dire!("\n{} ligne(s) ajoutee(s) au journal.", d.ajoutes.len());
    } else {
        dire!("\n{} ligne(s) SERAIENT ajoutees. Rien n'a ete ecrit : --ecrire les pose.", d.ajoutes.len());
    }
    Ok(0)
}
