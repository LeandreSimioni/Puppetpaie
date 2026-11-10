/* Configuration → Modèles de contrat. */
"use strict";

/* LES MODÈLES DE CONTRAT DE LA COMPAGNIE.

   Chaque nature de contrat a son modèle de base, livré, contrat et avenant ; la
   compagnie le copie en modèles spéciaux, en choisit un par défaut, et un
   engagement peut en choisir un autre (Avancé). Un modèle s'ouvre en
   paragraphes, son contrat ou son avenant : la base se lit, un spécial s'écrit
   paragraphe par paragraphe.

   La page ne décide rien (règle 2) et n'écrit aucune phrase : le moteur dit les
   modèles (`modeles-contrat`), lit une partie d'un modèle avec ses champs, ce qui
   a changé dans sa base et les engagements de l'aperçu (`modele-contrat`), fait
   l'aperçu (`apercu-modele-contrat`) ; chaque geste est une route, qui vérifie le
   modèle et dit son refus : `copier-modele-contrat`, `renommer-modele-contrat`,
   `ecrire-paragraphe-contrat`, `ajouter-paragraphe-contrat`,
   `deplacer-paragraphe-contrat`, `supprimer-paragraphe-contrat`,
   `supprimer-modele-contrat`, `modele-contrat-par-defaut`.

   `ouvert` : la partie du modèle dont l'éditeur est ouvert ({nature, id,
   partie}), redessinée après chaque geste. */
async function dessinerModelesContrat(cible, ouvert = null) {
  cible.innerHTML = `<p class="note">…</p>`;
  let m;
  try {
    m = await appelerMoteur("modeles-contrat", {});
  } catch (e) {
    cible.innerHTML = `<div class="encart erreur">${echapper(e.message)}</div>`;
    return;
  }
  const estOuvert = (nature, id) => ouvert && ouvert.nature === nature && (ouvert.id || null) === id;
  const ligne = (n, x) => `
    <div class="modele-contrat" data-nature="${echapper(n.nature)}" data-id="${echapper(x.id || "")}">
      <div class="modele-entete">
        <b>${echapper(x.nom)}</b>
        ${pastilleDeLEtat(x.pastille)}
      </div>
      ${x.base_changee ? `<div class="encart">${echapper(x.base_changee)}</div>` : ""}
      ${x.manques.length ? `<div class="encart erreur">${x.manques.map(echapper).join("<br>")}</div>` : ""}
      <div class="modele-gestes">
        ${estOuvert(n.nature, x.id || null)
          ? `<button type="button" class="lien" data-ouvrir>Fermer</button>`
          : `<button type="button" class="lien" data-ouvrir>Ouvrir</button>`}
        ${x.defaut ? "" : `<button type="button" class="lien" data-defaut>Choisir par défaut</button>`}
        ${x.base ? "" : `<button type="button" class="lien" data-supprimer>Supprimer</button>`}
      </div>
      <div class="modele-editeur"></div>
    </div>`;
  cible.innerHTML = `
    <p class="intro">${echapper(m.ecran.intro)}</p>
    <p class="discret">${echapper(m.ecran.en_francais)}</p>
    <ol class="discret">${m.ecran.comment.map((x) => `<li>${echapper(x)}</li>`).join("")}</ol>
    ${m.natures.map((n) => `
      <h3>${echapper(n.nom.charAt(0).toUpperCase() + n.nom.slice(1))} <span class="discret">— ${echapper(n.libelle)}</span></h3>
      ${n.modeles.map((x) => ligne(n, x)).join("")}
      <div class="modele-copier" data-nature="${echapper(n.nature)}">
        <input type="text" placeholder="Nom du modèle">
        <button type="button" class="lien">Copier la base</button>
      </div>`).join("")}`;

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
  cible.querySelectorAll(".modele-contrat").forEach((bloc) => {
    const nature = bloc.dataset.nature;
    const id = bloc.dataset.id || null;
    const ici = estOuvert(nature, id);
    bloc.querySelector("[data-ouvrir]").onclick = () => dessinerModelesContrat(cible, ici ? null : { nature, id, partie: "contrat" });
    const defaut = bloc.querySelector("[data-defaut]");
    if (defaut) defaut.onclick = () => geste("modele-contrat-par-defaut", { nature, id });
    const supprimer = bloc.querySelector("[data-supprimer]");
    if (supprimer) supprimer.onclick = () => geste("supprimer-modele-contrat", { id }, ici ? null : ouvert);
    if (ici) dessinerEditeurDeModele(bloc.querySelector(".modele-editeur"), ouvert, geste, (o) => dessinerModelesContrat(cible, o));
  });
  cible.querySelectorAll(".modele-copier").forEach((bloc) => {
    bloc.querySelector("button").onclick = () => geste("copier-modele-contrat", { nature: bloc.dataset.nature, nom: bloc.querySelector("input").value });
  });
}

/* L'ÉDITEUR D'UNE PARTIE D'UN MODÈLE (son contrat ou son avenant), en
   paragraphes : la base se lit, un spécial s'écrit. Une donnée s'insère par la
   liste (`champs` de la route), dans l'alinéa où l'on écrit ; la condition d'un
   alinéa se choisit dans la même liste. `ouvrir` : rouvre la liste sur une autre
   partie. */
async function dessinerEditeurDeModele(cible, ici, geste, ouvrir) {
  let lu;
  try {
    lu = await appelerMoteur("modele-contrat", ici);
  } catch (e) {
    cible.innerHTML = `<div class="encart erreur">${echapper(e.message)}</div>`;
    return;
  }
  const ecrit = !lu.base;
  const contrat = lu.partie === "contrat";
  const choix = (classe, premier, si) => `
    <select class="${classe}" ${ecrit ? "" : "disabled"}>
      <option value="">${premier}</option>
      ${lu.champs.map((c) => `<option value="${echapper(c.marqueur)}" ${c.marqueur === si ? "selected" : ""}>${echapper(c.quoi)}</option>`).join("")}
    </select>`;
  const alinea = (a) => `
    <div class="modele-alinea">
      ${choix("modele-si", "Toujours", a.si)}
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
    <div class="modele-gestes">
      <button type="button" class="lien" data-partie="contrat" ${contrat ? "disabled" : ""}>Contrat</button>
      <button type="button" class="lien" data-partie="avenant" ${contrat ? "" : "disabled"}>Avenant</button>
    </div>
    ${lu.manques.length ? `<div class="encart erreur">${lu.manques.map(echapper).join("<br>")}</div>` : ""}
    ${lu.base_changee && lu.base_changee.length ?`<div class="encart">${lu.base_changee.map(echapper).join("<br>")}</div>` : ""}
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
    ${ecrit ? choix("modele-inserer", "Insérer une donnée", null) : ""}
    ${lu.articles.map(paragraphe).join("")}
    ${ecrit ? `<div class="modele-gestes">
      <input type="text" class="modele-nouveau" placeholder="Titre">
      <button type="button" class="lien" data-ajouter>Ajouter un paragraphe</button>
    </div>` : ""}`;

  cible.querySelectorAll("[data-partie]").forEach((b) => { b.onclick = () => ouvrir({ ...ici, partie: b.dataset.partie }); });
  cible.querySelector("[data-apercu]").onclick = async (ev) => {
    const choisi = lu.engagements[cible.querySelector(".modele-engagement").value];
    const rallumer = occupe(ev.target);
    try {
      const r = await api("apercu-modele-contrat", { nature: lu.nature, id: lu.id, partie: lu.partie, engagement: choisi ? choisi.engagement : null });
      await ouvrirEtDire(r.chemin, r.phrase);
    } catch (e) {
      dire(e.message, "erreur");
    } finally {
      rallumer();
    }
  };
  if (!ecrit) return;
  const sur = { id: lu.id, partie: lu.partie };
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
  cible.querySelector("[data-renommer]").onclick = () => geste("renommer-modele-contrat", { id: lu.id, nom: cible.querySelector(".modele-nom").value });
  cible.querySelector("[data-ajouter]").onclick = () => geste("ajouter-paragraphe-contrat", { ...sur, numero: lu.articles.length + 1, titre: cible.querySelector(".modele-nouveau").value });
  cible.querySelectorAll(".modele-paragraphe").forEach((bloc) => {
    const numero = Number(bloc.dataset.numero);
    bloc.querySelector("[data-enregistrer]").onclick = () => geste("ecrire-paragraphe-contrat", {
      ...sur,
      numero,
      titre: bloc.querySelector(".modele-titre").value,
      alineas: [...bloc.querySelectorAll(".modele-alinea")].map((a) => ({ si: a.querySelector("select").value || null, texte: a.querySelector("textarea").value })),
    });
    bloc.querySelector("[data-alinea]").onclick = () => bloc.querySelector(".modele-alineas").insertAdjacentHTML("beforeend", alinea({ si: null, texte: "" }));
    bloc.querySelector("[data-monter]").onclick = () => geste("deplacer-paragraphe-contrat", { ...sur, numero, sens: "monter" });
    bloc.querySelector("[data-descendre]").onclick = () => geste("deplacer-paragraphe-contrat", { ...sur, numero, sens: "descendre" });
    bloc.querySelector("[data-retirer]").onclick = () => geste("supprimer-paragraphe-contrat", { ...sur, numero });
  });
}
