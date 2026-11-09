/* Configuration → Modèles de contrat. */
"use strict";

/* LES MODÈLES DE CONTRAT DE LA COMPAGNIE.

   Chaque nature de contrat a son modèle de base, livré, qui porte déjà ce que
   la loi exige ; la compagnie le copie en modèles spéciaux, en choisit un par
   défaut, et un engagement peut en choisir un autre (Avancé). L'avenant garde
   son modèle propre d'une nature ; les clauses particulières s'écrivent une fois.

   La page ne décide rien (règle 2) : le moteur dit les modèles et ce qui leur
   manque (`modeles-contrat`), lit un modèle (`modele-contrat`), copie la base
   (`copier-modele-contrat`), écrit un spécial (`ecrire-modele-contrat`), le
   supprime (`supprimer-modele-contrat`), choisit le défaut
   (`modele-contrat-par-defaut`), pose et retire le modèle d'avenant
   (`poser-modele-avenant`, `retirer-modele-avenant`), écrit les clauses
   (`clauses-particulieres`) ; un refus se montre tel qu'il vient. */
async function dessinerModelesContrat(cible) {
  cible.innerHTML = `<p class="note">…</p>`;
  let m;
  try {
    m = await appelerMoteur("modeles-contrat", {});
  } catch (e) {
    cible.innerHTML = `<div class="encart erreur">${echapper(e.message)}</div>`;
    return;
  }
  const ligne = (n, x) => `
    <div class="modele-contrat" data-nature="${echapper(n.nature)}" data-id="${echapper(x.id || "")}">
      <div style="display:flex;align-items:baseline;gap:8px;flex-wrap:wrap">
        <b>${echapper(x.nom)}</b>
        ${pastilleDeLEtat(x.pastille)}
      </div>
      ${x.base_changee ? `<div class="encart">${echapper(x.base_changee)}</div>` : ""}
      ${x.manques.length ? `<div class="encart erreur">${x.manques.map(echapper).join("<br>")}</div>` : ""}
      <div style="display:flex;gap:12px;align-items:center;flex-wrap:wrap;margin-top:6px">
        <button type="button" class="lien" data-lire>${x.base ? "Lire" : "Modifier"}</button>
        ${x.defaut ? "" : `<button type="button" class="lien" data-defaut>Choisir par défaut</button>`}
        ${x.base ? "" : `<button type="button" class="lien" data-supprimer>Supprimer</button>`}
      </div>
      <div class="modele-editeur" hidden>
        <textarea rows="18" spellcheck="false" style="width:100%;font-family:monospace;font-size:12px" ${x.base ? "readonly" : ""}></textarea>
        <div style="display:flex;gap:12px;margin-top:6px">
          ${x.base ? "" : `<button type="button" data-ecrire>Enregistrer</button>`}
          <button type="button" class="lien" data-fermer>Fermer</button>
        </div>
      </div>
    </div>`;
  const avenant = (n) => `
    <div class="modele-avenant" data-nature="${echapper(n.nature)}">
      <div style="display:flex;align-items:baseline;gap:8px;flex-wrap:wrap">
        <b>${echapper(n.avenant.nom)}</b>
        ${pastilleDeLEtat(n.avenant.pastille)}
      </div>
      ${n.avenant.refus ? `<div class="encart erreur">${echapper(n.avenant.refus)}</div>` : ""}
      <div style="display:flex;gap:12px;align-items:center;flex-wrap:wrap;margin-top:6px">
        <button type="button" class="lien" data-ecrire>${echapper(n.avenant.bouton)}</button>
        ${n.avenant.propre ? `<button type="button" class="lien" data-retirer>Modèle du logiciel</button>` : ""}
      </div>
      <div class="modele-editeur" hidden>
        <textarea rows="18" spellcheck="false" style="width:100%;font-family:monospace;font-size:12px"></textarea>
        <div style="display:flex;gap:12px;margin-top:6px">
          <button type="button" data-poser>Poser ce modèle</button>
          <button type="button" class="lien" data-fermer>Fermer</button>
        </div>
      </div>
    </div>`;
  const champs = (n) => `
    <details class="modele-champs">
      <summary>${echapper(n.champs_titre)}</summary>
      <table class="tableau">
        <tr><th>Champ</th><th>Ce qui s'y écrit</th><th>La loi l'exige</th></tr>
        ${n.champs.map((c) => `<tr>
          <td><code>${echapper(c.marqueur)}</code></td>
          <td>${echapper(c.quoi)}</td>
          <td>${c.obligatoire ? echapper(c.texte || "oui") : ""}</td></tr>`).join("")}
      </table>
      <p class="discret">${echapper(m.ecran.avenant)}</p>
    </details>`;
  cible.innerHTML = `
    <p class="intro">${echapper(m.ecran.intro)}</p>
    <p class="discret">${echapper(m.ecran.en_francais)}</p>
    <h3>Clauses particulières</h3>
    <p class="discret">${echapper(m.ecran.clauses)}</p>
    <textarea id="clauses-compagnie" rows="3" style="width:100%">${echapper(m.clauses_particulieres || "")}</textarea>
    <div><button type="button" id="clauses-enregistrer">Enregistrer les clauses</button></div>
    <ol class="discret">${m.ecran.comment.map((x) => `<li>${echapper(x)}</li>`).join("")}</ol>
    ${m.natures.map((n) => `
      <h3>${echapper(n.nom.charAt(0).toUpperCase() + n.nom.slice(1))} <span class="discret">— ${echapper(n.libelle)}</span></h3>
      ${n.modeles.map((x) => ligne(n, x)).join("")}
      <div class="modele-copier" data-nature="${echapper(n.nature)}" style="display:flex;gap:8px;margin:6px 0">
        <input type="text" placeholder="Nom du modèle">
        <button type="button" class="lien">Copier la base</button>
      </div>
      ${avenant(n)}
      ${champs(n)}`).join("")}`;

  const geste = async (route, charge) => {
    try {
      const r = await api(route, charge);
      await dessinerModelesContrat(cible);
      dire(r.phrase);
    } catch (e) {
      dire(e.message, "erreur");
    }
  };
  document.getElementById("clauses-enregistrer").onclick = async (ev) => {
    ev.target.disabled = true;
    try {
      const r = await api("clauses-particulieres", { texte: document.getElementById("clauses-compagnie").value });
      ETAT.configuration = await api("configuration", {}); peindreEtatGlobal();
      dire(r.phrase);
    } catch (e) {
      dire(e.message, "erreur");
    } finally {
      ev.target.disabled = false;
    }
  };
  cible.querySelectorAll(".modele-contrat").forEach((bloc) => {
    const nature = bloc.dataset.nature;
    const id = bloc.dataset.id || null;
    const editeur = bloc.querySelector(".modele-editeur");
    bloc.querySelector("[data-lire]").onclick = async () => {
      try {
        const lu = await appelerMoteur("modele-contrat", id ? { nature, id } : { nature });
        editeur.querySelector("textarea").value = lu.texte;
        editeur.hidden = false;
      } catch (e) {
        dire(e.message, "erreur");
      }
    };
    bloc.querySelector("[data-fermer]").onclick = () => { editeur.hidden = true; };
    const ecrire = bloc.querySelector("[data-ecrire]");
    if (ecrire) ecrire.onclick = () => geste("ecrire-modele-contrat", { id, texte: editeur.querySelector("textarea").value });
    const defaut = bloc.querySelector("[data-defaut]");
    if (defaut) defaut.onclick = () => geste("modele-contrat-par-defaut", id ? { nature, id } : { nature });
    const supprimer = bloc.querySelector("[data-supprimer]");
    if (supprimer) supprimer.onclick = () => geste("supprimer-modele-contrat", { id });
  });
  cible.querySelectorAll(".modele-copier").forEach((bloc) => {
    bloc.querySelector("button").onclick = () => geste("copier-modele-contrat", { nature: bloc.dataset.nature, nom: bloc.querySelector("input").value });
  });
  cible.querySelectorAll(".modele-avenant").forEach((bloc) => {
    const n = m.natures.find((x) => x.nature === bloc.dataset.nature);
    const editeur = bloc.querySelector(".modele-editeur");
    bloc.querySelector("[data-ecrire]").onclick = () => {
      editeur.querySelector("textarea").value = n.avenant.texte || "";
      editeur.hidden = false;
    };
    bloc.querySelector("[data-fermer]").onclick = () => { editeur.hidden = true; };
    bloc.querySelector("[data-poser]").onclick = () => geste("poser-modele-avenant", { nature: n.nature, texte: editeur.querySelector("textarea").value });
    const retirer = bloc.querySelector("[data-retirer]");
    if (retirer) retirer.onclick = () => geste("retirer-modele-avenant", { nature: n.nature });
  });
}
