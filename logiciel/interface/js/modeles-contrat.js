/* Configuration → Modèles de contrat. */
"use strict";

/* LES MODÈLES DE CONTRAT DE LA COMPAGNIE.

   Chaque nature de contrat a son modèle de base, livré, qui porte déjà ce que
   la loi exige ; la compagnie le copie en modèles spéciaux, en choisit un par
   défaut, et un engagement peut en choisir un autre (Avancé). Un modèle s'ouvre
   en paragraphes : la base se lit, un spécial s'écrit paragraphe par paragraphe.

   La page ne décide rien (règle 2) et n'écrit aucune phrase : le moteur dit les
   modèles (`modeles-contrat`), lit un modèle avec ses champs, ce qui a changé
   dans sa base et les engagements de l'aperçu (`modele-contrat`), fait l'aperçu
   (`apercu-modele-contrat`) ; chaque geste est une route, qui vérifie le modèle
   et dit son refus : `copier-modele-contrat`, `renommer-modele-contrat`,
   `ecrire-paragraphe-contrat`, `ajouter-paragraphe-contrat`,
   `deplacer-paragraphe-contrat`, `supprimer-paragraphe-contrat`,
   `supprimer-modele-contrat`, `modele-contrat-par-defaut`. L'avenant garde son
   modèle propre d'une nature (`poser-modele-avenant`, `retirer-modele-avenant`) ;
   les clauses particulières s'écrivent une fois (`clauses-particulieres`).

   `ouvert` : le modèle dont l'éditeur est ouvert ({nature, id}), redessiné
   après chaque geste. */
async function dessinerModelesContrat(cible, ouvert = null) {
  cible.innerHTML = `<p class="note">…</p>`;
  let m;
  try {
    m = await appelerMoteur("modeles-contrat", {});
  } catch (e) {
    cible.innerHTML = `<div class="encart erreur">${echapper(e.message)}</div>`;
    return;
  }
  const estOuvert = (n, x) => ouvert && ouvert.nature === n.nature && (ouvert.id || null) === (x.id || null);
  const ligne = (n, x) => `
    <div class="modele-contrat" data-nature="${echapper(n.nature)}" data-id="${echapper(x.id || "")}">
      <div class="modele-entete">
        <b>${echapper(x.nom)}</b>
        ${pastilleDeLEtat(x.pastille)}
      </div>
      ${x.base_changee ? `<div class="encart">${echapper(x.base_changee)}</div>` : ""}
      ${x.manques.length ? `<div class="encart erreur">${x.manques.map(echapper).join("<br>")}</div>` : ""}
      <div class="modele-gestes">
        <button type="button" class="lien" data-ouvrir>${estOuvert(n, x) ? "Fermer" : "Ouvrir"}</button>
        ${x.defaut ? "" : `<button type="button" class="lien" data-defaut>Choisir par défaut</button>`}
        ${x.base ? "" : `<button type="button" class="lien" data-supprimer>Supprimer</button>`}
      </div>
      <div class="modele-editeur"></div>
    </div>`;
  const avenant = (n) => `
    <div class="modele-avenant" data-nature="${echapper(n.nature)}">
      <div class="modele-entete">
        <b>${echapper(n.avenant.nom)}</b>
        ${pastilleDeLEtat(n.avenant.pastille)}
      </div>
      ${n.avenant.refus ? `<div class="encart erreur">${echapper(n.avenant.refus)}</div>` : ""}
      <div class="modele-gestes">
        <button type="button" class="lien" data-ecrire>${echapper(n.avenant.bouton)}</button>
        ${n.avenant.propre ? `<button type="button" class="lien" data-retirer>Modèle du logiciel</button>` : ""}
      </div>
      <div class="modele-editeur" hidden>
        <textarea rows="18" spellcheck="false" class="modele-texte"></textarea>
        <p class="discret">${echapper(m.ecran.avenant)}</p>
        <div class="modele-gestes">
          <button type="button" data-poser>Poser ce modèle</button>
          <button type="button" class="lien" data-fermer>Fermer</button>
        </div>
      </div>
    </div>`;
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
      <div class="modele-copier" data-nature="${echapper(n.nature)}">
        <input type="text" placeholder="Nom du modèle">
        <button type="button" class="lien">Copier la base</button>
      </div>
      ${avenant(n)}`).join("")}`;

  // Un geste : la route, puis la page redessinée (l'éditeur ouvert le reste) et sa phrase.
  const geste = async (route, charge, garder = ouvert) => {
    try {
      const r = await api(route, charge);
      await dessinerModelesContrat(cible, garder);
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
    const ici = { nature, id };
    const estIci = ouvert && ouvert.nature === nature && (ouvert.id || null) === id;
    bloc.querySelector("[data-ouvrir]").onclick = () => dessinerModelesContrat(cible, estIci ? null : ici);
    const defaut = bloc.querySelector("[data-defaut]");
    if (defaut) defaut.onclick = () => geste("modele-contrat-par-defaut", id ? { nature, id } : { nature });
    const supprimer = bloc.querySelector("[data-supprimer]");
    if (supprimer) supprimer.onclick = () => geste("supprimer-modele-contrat", { id }, estIci ? null : ouvert);
    if (estIci) dessinerEditeurDeModele(bloc.querySelector(".modele-editeur"), ici, geste);
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

/* L'ÉDITEUR D'UN MODÈLE, en paragraphes : la base se lit, un spécial s'écrit.
   Une donnée s'insère par la liste (`champs` de la route), dans l'alinéa où l'on
   écrit ; la condition d'un alinéa se choisit dans la même liste. */
async function dessinerEditeurDeModele(cible, ici, geste) {
  let lu;
  try {
    lu = await appelerMoteur("modele-contrat", ici.id ? ici : { nature: ici.nature });
  } catch (e) {
    cible.innerHTML = `<div class="encart erreur">${echapper(e.message)}</div>`;
    return;
  }
  const ecrit = !lu.base;
  const fige = ecrit ? "" : "disabled";
  const condition = (si) => `
    <select class="modele-si" ${fige}>
      <option value="">Toujours</option>
      ${lu.champs.map((c) => `<option value="${echapper(c.marqueur)}" ${c.marqueur === si ? "selected" : ""}>${echapper(c.quoi)}</option>`).join("")}
    </select>`;
  const alinea = (a) => `
    <div class="modele-alinea">
      ${condition(a.si)}
      <textarea rows="3" ${ecrit ? "" : "readonly"}>${echapper(a.texte)}</textarea>
    </div>`;
  const paragraphe = (p) => `
    <div class="modele-paragraphe" data-numero="${p.numero}">
      <div class="modele-entete">
        <b>${p.numero}</b>
        <input type="text" class="modele-titre" value="${echapper(p.titre)}" ${ecrit ? "" : "readonly"}>
      </div>
      <div class="modele-alineas">${p.alineas.map(alinea).join("")}</div>
      ${ecrit ? `<div class="modele-gestes">
        <button type="button" data-enregistrer>Enregistrer</button>
        <button type="button" class="lien" data-alinea>Ajouter un alinéa</button>
        <button type="button" class="lien" data-monter>Monter</button>
        <button type="button" class="lien" data-descendre>Descendre</button>
        <button type="button" class="lien" data-retirer>Supprimer</button>
      </div>` : ""}
    </div>`;
  cible.innerHTML = `
    ${lu.manques.length ? `<div class="encart erreur">${lu.manques.map(echapper).join("<br>")}</div>` : ""}
    ${lu.base_changee.length ? `<div class="encart">${lu.base_changee.map(echapper).join("<br>")}</div>` : ""}
    <p class="discret">${echapper(lu.en_francais)}</p>
    ${ecrit ? `<div class="modele-gestes">
      <input type="text" class="modele-nom" value="${echapper(lu.nom)}">
      <button type="button" class="lien" data-renommer>Renommer</button>
    </div>` : ""}
    <div class="modele-gestes">
      <select class="modele-engagement">
        ${lu.engagements.map((e, i) => `<option value="${i}">${echapper(e.nom)}</option>`).join("")}
      </select>
      <button type="button" class="lien" data-apercu>Aperçu PDF</button>
    </div>
    ${ecrit ? `<select class="modele-inserer">
      <option value="">Insérer une donnée</option>
      ${lu.champs.map((c) => `<option value="${echapper(c.marqueur)}">${echapper(c.quoi)}</option>`).join("")}
    </select>` : ""}
    ${lu.articles.map(paragraphe).join("")}
    ${ecrit ? `<div class="modele-gestes">
      <input type="text" class="modele-nouveau" placeholder="Titre">
      <button type="button" class="lien" data-ajouter>Ajouter un paragraphe</button>
    </div>` : ""}`;

  cible.querySelector("[data-apercu]").onclick = async (ev) => {
    const choisi = lu.engagements[cible.querySelector(".modele-engagement").value];
    const rallumer = occupe(ev.target);
    try {
      const r = await api("apercu-modele-contrat", { nature: lu.nature, id: lu.id, engagement: choisi ? choisi.engagement : null });
      await ouvrirEtDire(r.chemin, r.phrase);
    } catch (e) {
      dire(e.message, "erreur");
    } finally {
      rallumer();
    }
  };
  if (!ecrit) return;
  const id = lu.id;
  // L'alinéa où l'on écrit en dernier reçoit la donnée choisie.
  let dernier = null;
  cible.addEventListener("focusin", (ev) => { if (ev.target.matches(".modele-alinea textarea")) dernier = ev.target; });
  const inserer = cible.querySelector(".modele-inserer");
  inserer.onchange = () => {
    if (dernier && inserer.value) {
      const marque = `{{${inserer.value}}}`;
      const [a, b] = [dernier.selectionStart, dernier.selectionEnd];
      dernier.value = dernier.value.slice(0, a) + marque + dernier.value.slice(b);
      dernier.focus();
      dernier.selectionStart = dernier.selectionEnd = a + marque.length;
    }
    inserer.value = "";
  };
  cible.querySelector("[data-renommer]").onclick = () => geste("renommer-modele-contrat", { id, nom: cible.querySelector(".modele-nom").value });
  cible.querySelector("[data-ajouter]").onclick = () => geste("ajouter-paragraphe-contrat", { id, numero: lu.articles.length + 1, titre: cible.querySelector(".modele-nouveau").value });
  cible.querySelectorAll(".modele-paragraphe").forEach((bloc) => {
    const numero = Number(bloc.dataset.numero);
    bloc.querySelector("[data-enregistrer]").onclick = () => geste("ecrire-paragraphe-contrat", {
      id,
      numero,
      titre: bloc.querySelector(".modele-titre").value,
      alineas: [...bloc.querySelectorAll(".modele-alinea")].map((a) => ({ si: a.querySelector("select").value || null, texte: a.querySelector("textarea").value })),
    });
    bloc.querySelector("[data-alinea]").onclick = () => bloc.querySelector(".modele-alineas").insertAdjacentHTML("beforeend", alinea({ si: null, texte: "" }));
    bloc.querySelector("[data-monter]").onclick = () => geste("deplacer-paragraphe-contrat", { id, numero, sens: "monter" });
    bloc.querySelector("[data-descendre]").onclick = () => geste("deplacer-paragraphe-contrat", { id, numero, sens: "descendre" });
    bloc.querySelector("[data-retirer]").onclick = () => geste("supprimer-paragraphe-contrat", { id, numero });
  });
}
