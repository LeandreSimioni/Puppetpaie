/* L'AFFICHAGE TIRÉ DES FICHES : sous « Avancé » de chaque place, et la boîte d'un geste qu'une route rend.
 *
 * Une route sans écran dessiné porte, dans sa fiche, la place où elle se montre (`ecran`, `affichage: "auto"`) et son
 * bouton (`bouton`) : la page l'affiche ici depuis sa fiche. Chaque champ de la charge dit ce que l'écran en montre
 * (`libelle`) et sa forme (`forme` : une liste se tape élément par élément, un oui/non se coche, un nombre, un mois ou une
 * date se choisissent ; le reste se tape). Puis la phrase rendue (ou ses lignes, `phrases`), ou le refus. La page ne
 * compose ni ne décide rien : chaque mot et chaque forme viennent de la fiche ou de la réponse. Une place sans route
 * automatique ne montre rien.
 *
 * Un geste qu'une route rend (`gestes` : sa route, sa charge, son libellé, les cases que sa boîte fait saisir, ce qu'elles
 * montrent d'abord, leurs choix et ce que la boîte dit) se dessine de même : son bouton, puis, s'il fait saisir, sa boîte,
 * faite des champs de la fiche de sa route. La page ne nomme aucune route : elle lit le geste et la fiche. */
"use strict";

/** La valeur de départ d'une case, pour un attribut. */
const valeurDeDepart = (v) => (v == null ? "" : ` value="${echapper(String(v))}"`);

const SAISIES = {
  oui_non: (j, v) => `<input type="checkbox" data-j="${j}"${v ? " checked" : ""}>`,
  mois: (j, v) => `<input type="month" data-j="${j}"${valeurDeDepart(v)}>`,
  date: (j, v) => `<input type="date" data-j="${j}"${valeurDeDepart(v)}>`,
  nombre: (j, v) => `<input type="number" data-j="${j}"${valeurDeDepart(v)}>`,
  liste: (j) => `<span data-liste="${j}"><input type="text" data-element></span> <button type="button" class="lien" data-plus="${j}">+</button>`,
};

/** La saisie d'un champ selon sa forme ; `valeur` : ce qu'elle montre d'abord ; `choix` : la liste où il se choisit. */
function saisieDuChamp(forme, j, valeur, choix) {
  if (choix) {
    return `<select data-j="${j}">${choix.map((o) => `<option value="${echapper(o.code)}"${
      o.code === valeur ? " selected" : ""}>${echapper(o.libelle)}</option>`).join("")}</select>`;
  }
  return (SAISIES[forme] || ((k, v) => `<input type="text" data-j="${k}"${valeurDeDepart(v)}>`))(j, valeur);
}

/** Les cases de `champs` (des champs d'une fiche) : chacune son libellé et sa saisie, ce que `valeurs` en dit d'abord, ses
 *  `choix` (une case qu'ils ne nomment pas : rien d'abord, et elle se tape). */
function casesDesChamps(champs, valeurs, choix) {
  return champs.map((c, j) => `<label class="champ-boite">${echapper(c.libelle)}
      ${saisieDuChamp(c.forme, j, valeurs[c.nom], choix[c.nom])}</label>`).join("");
}

/** Le « + » d'une liste : un élément de plus à taper. */
function brancherLesListes(bloc) {
  bloc.querySelectorAll("[data-plus]").forEach((b) => {
    b.onclick = () => {
      const x = document.createElement("input");
      x.type = "text";
      bloc.querySelector(`[data-liste="${b.dataset.plus}"]`).appendChild(x);
      x.focus();
    };
  });
}

/** La valeur d'un champ, selon sa forme ; vide, le champ ne part pas. */
function valeurDuChamp(bloc, forme, j) {
  if (forme === "liste") {
    const v = [...bloc.querySelectorAll(`[data-liste="${j}"] input`)].map((x) => x.value.trim()).filter(Boolean);
    return v.length ? v : undefined;
  }
  const x = bloc.querySelector(`[data-j="${j}"]`);
  if (forme === "oui_non") return x.checked ? true : undefined;
  const v = x.value.trim();
  if (!v) return undefined;
  return forme === "nombre" ? nombreTape(v) : v;
}

/** Ce que les cases de `champs` disent, dans `bloc` : un champ vide ne part pas. */
function lireLesChamps(bloc, champs) {
  const lu = {};
  champs.forEach((c, j) => {
    const v = valeurDuChamp(bloc, c.forme, j);
    if (v !== undefined) lu[c.nom] = v;
  });
  return lu;
}

/** Ce que la réponse dit à lire : sa phrase, puis ses lignes (chaque route automatique rend sa phrase). */
function phrasesDe(r) {
  const p = [];
  if (r && r.phrase) p.push(r.phrase);
  if (r && Array.isArray(r.phrases)) p.push(...r.phrases);
  return p;
}

/** Dessine dans `cible` (porteuse de `data-auto`, le nom d'une place) les routes automatiques de cette place. */
function dessinerAvance(cible) {
  if (!cible) return;
  const place = cible.dataset.auto;
  const routes = Object.values(CATALOGUE).filter((f) => f.affichage === "auto" && f.ecran === place);
  const enveloppe = cible.closest(".avance-auto") || cible;
  enveloppe.hidden = !routes.length;
  cible.innerHTML = routes.map((f, i) => `
    <div class="carte avance-route" data-i="${i}">
      ${casesDesChamps(f.charge, {}, {})}
      <p><button class="secondaire" data-faire>${echapper(f.bouton)}</button></p>
      <div data-rendu></div>
    </div>`).join("");
  cible.querySelectorAll(".avance-route").forEach((bloc) => {
    const f = routes[+bloc.dataset.i];
    brancherLesListes(bloc);
    bloc.querySelector("[data-faire]").onclick = async () => {
      const rendu = bloc.querySelector("[data-rendu]");
      try {
        const r = await api(f.nom, lireLesChamps(bloc, f.charge));
        rendu.innerHTML = phrasesDe(r).map((p) => `<p>${echapper(p)}</p>`).join("");
      } catch (e) {
        rendu.innerHTML = `<div class="encart erreur">${echapper(e.message)}</div>`;
      }
    };
  });
}

/** Les places dessinées une fois pour toutes (`data-auto` dans la page) : à l'ouverture, le catalogue lu. */
function dessinerLesAvances() {
  document.querySelectorAll("[data-auto]").forEach(dessinerAvance);
}

/* ------------------------------------------------ les gestes qu'une route rend */

/** Les boutons des gestes, chacun son libellé. */
function boutonsDesGestes(gestes) {
  return gestes.map((g, k) => `<button class="lien" data-geste="${k}">${echapper(g.libelle)}</button>`).join("");
}

/** Les champs de la fiche de la route du geste que sa boîte fait saisir (`cases`), dans l'ordre de la fiche. */
function champsDuGeste(g) {
  return CATALOGUE[g.route].charge.filter((c) => g.cases.includes(c.nom));
}

/** Ce que la boîte d'un geste dit (`phrases`), puis ses cases. */
function casesDuGeste(g) {
  return `${g.phrases.map((p) => `<p class="note">${echapper(p)}</p>`).join("")}
    ${casesDesChamps(champsDuGeste(g), g.valeurs, g.choix)}`;
}

/** Ce que les cases du geste disent, dans `bloc`, ajouté à sa charge ; `null` si aucune ne dit rien. */
function chargeDuGeste(bloc, g) {
  const lu = lireLesChamps(bloc, champsDuGeste(g));
  return Object.keys(lu).length ? { ...g.charge, ...lu } : null;
}

/** Branche les boutons des gestes de `cible` : un geste sans case part avec sa charge ; un autre ouvre sa boîte.
 *  `entete` : ce que la boîte dit de quoi il s'agit (`titre`, à la suite du libellé ; `dit`, dessous). `apres` : après
 *  la réponse. */
function brancherLesGestes(cible, gestes, entete, apres) {
  cible.querySelectorAll("[data-geste]").forEach((b) => {
    const g = gestes[+b.dataset.geste];
    b.onclick = () => faireLeGeste(g, entete, apres);
  });
}

async function faireLeGeste(g, entete, apres) {
  const partir = async (charge, boite) => {
    let r;
    try { r = await api(g.route, charge); } catch (e) { dire(e.message, "erreur"); return; }
    if (boite) fermerBoite();
    dire(r.phrase);
    await apres();
  };
  if (!g.cases.length) { await partir(g.charge, false); return; }
  ouvrirBoite(`
    <h3>${echapper(g.libelle)}${entete.titre ? ` — ${echapper(entete.titre)}` : ""}</h3>
    ${entete.dit ? `<p class="discret">${echapper(entete.dit)}</p>` : ""}
    <div id="geste-cases">${casesDuGeste(g)}</div>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="geste-ok">Enregistrer</button>
    </div>`);
  const bloc = document.getElementById("geste-cases");
  brancherLesListes(bloc);
  // Une case vidée ne part pas : la route dit ce qu'une case absente fait (une reprise absente s'efface).
  document.getElementById("geste-ok").onclick = () => partir(chargeDuGeste(bloc, g) || g.charge, true);
}
