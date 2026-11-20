/* LE DESSINATEUR DE LA PAGE : il dessine ce qu'une route rend et fait saisir ce qu'une route attend, depuis la forme que
 * le moteur dit de chaque case (`forme`), et rien d'autre. Une table, une case par forme (`CASES`) ; les lignes de paie
 * (`lignes`), chacune les clés de sa rubrique ; les gestes, un bouton chacun.
 *
 * Il sert partout de même : les champs d'une fiche de route sous « Avancé » de chaque place, la boîte d'un geste qu'une
 * route rend, la carte d'un engagement (`js/carte.js`). La page ne compose ni ne décide rien : chaque mot, chaque forme,
 * chaque liste où l'on choisit viennent de la fiche ou de la réponse. */
"use strict";

/** La valeur de départ d'une case, pour un attribut. */
const valeurDeDepart = (v) => (v == null ? "" : ` value="${echapper(String(v))}"`);

/** Un texte tapé, ou `null` pour une case vide. */
const tape = (x) => (x.value.trim() === "" ? null : x.value.trim());

/** Les options d'un choix : une case vide d'abord (rien de choisi ne part), puis chaque valeur offerte ; une valeur
 *  posée que la liste n'offre pas reste offerte telle quelle (une fiche mise de côté). */
function optionsDuChoix(v, choix) {
  const offerts = choix || [];
  const garde = v != null && v !== "" && !offerts.some((o) => o.code === String(v)) ? [{ code: String(v), libelle: String(v) }] : [];
  return `<option value=""></option>${[...offerts, ...garde].map((o) => `<option value="${echapper(o.code)}"${
    o.code === String(v) ? " selected" : ""}>${echapper(o.libelle)}</option>`).join("")}`;
}

/** LA TABLE : à chaque forme, sa case (`html`, ce qu'elle montre de `v` ; `o.choix` : ce qu'un choix offre, `o.unite` :
 *  l'unité d'une quantité) et sa lecture (`lire`, la valeur que la route lit ; `null` : une case vide). Chaque case porte
 *  `data-case`. Un jour et des jours s'ouvrent dans le calendrier (`js/calendrier.js`, `poserLesCalendriers`). */
const CASES = {
  texte: { html: (v) => `<input type="text" data-case${valeurDeDepart(v)}>`, lire: tape },
  nombre: { html: (v) => `<input type="text" inputmode="decimal" data-case${valeurDeDepart(v)}>`, lire: tape },
  quantite: {
    html: (v, o) => `<input type="text" inputmode="numeric" data-case data-unite="${echapper(o.unite)}"${
      valeurDeDepart(v == null ? null : v[o.unite])}>`,
    lire: (x) => (tape(x) == null ? null : { [x.dataset.unite]: tape(x) }),
  },
  oui_non: { html: (v) => `<input type="checkbox" data-case${v ? " checked" : ""}>`, lire: (x) => (x.checked ? true : null) },
  mois: { html: (v) => `<input type="month" data-case${valeurDeDepart(v)}>`, lire: tape },
  date: {
    html: (v) => `<input type="text" data-case data-calendrier="jour" placeholder="jj/mm/aaaa" autocomplete="off"${
      valeurDeDepart(v == null ? null : jjmmaaaa(v))}>`,
    lire: lireUnJour,
  },
  jours: {
    html: (v) => `<button type="button" class="jours-coches" data-case data-calendrier="jours"
      data-valeur="${echapper(JSON.stringify(v || []))}">${echapper(joursDits(v || []))}</button>`,
    lire: (x) => JSON.parse(x.dataset.valeur),
  },
  liste: {
    html: (v, o) => (o.choix
      ? `<span data-case data-liste="coches">${o.choix.map((c) => `<label class="coche"${c.aide ? ` title="${echapper(c.aide)}"` : ""}>
          <input type="checkbox" value="${echapper(c.code)}"${(v || []).includes(c.code) ? " checked" : ""}> ${echapper(c.libelle)}</label>`).join("")}</span>`
      : `<span data-case data-liste="textes">${[...(v || []), ""].map((t) => `<input type="text"${valeurDeDepart(t)}>`).join("")}
          <button type="button" class="lien" data-plus>+</button></span>`),
    lire: (x) => [...x.querySelectorAll(x.dataset.liste === "coches" ? "input:checked" : "input")]
      .map((c) => c.value.trim()).filter(Boolean),
  },
  choix: { html: (v, o) => `<select data-case>${optionsDuChoix(v, o.choix)}</select>`, lire: (x) => x.value || null },
  choix_rendu: { html: (v, o) => `<select data-case>${optionsDuChoix(v, o.choix)}</select>`, lire: (x) => x.value || null },
  // Une valeur composée que le moteur décrit par son sens : elle se lit, elle ne se saisit pas d'une case.
  objet: { html: (v) => `<span class="case-objet" data-case>${echapper(v == null ? "—" : JSON.stringify(v))}</span>`, lire: () => undefined },
};

/** Un jour tapé : JJ/MM/AAAA (un ou deux chiffres pour le jour et le mois) devient AAAA-MM-JJ ; ce qui ne se lit pas
 *  comme un jour part tel quel, et le moteur le refuse en nommant sa case. */
function lireUnJour(x) {
  const t = x.value.trim();
  if (!t) return null;
  const m = /^(\d{1,2})\/(\d{1,2})\/(\d{4})$/.exec(t);
  return m ? `${m[3]}-${m[2].padStart(2, "0")}-${m[1].padStart(2, "0")}` : t;
}

/** Ce que dit le bouton des jours cochés : « 12, 13 nov. », ou l'invitation. */
function joursDits(jours) {
  if (!jours.length) return "— jours —";
  const isos = jours.map(jourDe).sort();
  return joursEnClair(isos) + " " + MOIS_ABR[+isos[0].slice(5, 7) - 1];
}

/** LA CASE d'une forme, sans libellé ; une forme que la table n'a pas est une faute de la page : elle se dit. */
function caseDeForme(forme, v, o = {}) {
  const c = CASES[forme];
  if (!c) throw new Error(`La forme « ${forme} » n'a pas de case dans la page : le signaler au développeur.`);
  return c.html(v, o).replace("data-case", `data-case data-forme="${forme}"`);
}

/** La valeur d'une case (l'élément qui porte `data-case`), lue par sa forme. */
function lireLaCase(x) {
  return CASES[x.dataset.forme].lire(x);
}

/** Branche ce qu'une case fait seule dans `bloc` : le « + » d'une liste, les calendriers. */
function brancherLesCases(bloc) {
  bloc.querySelectorAll("[data-plus]").forEach((b) => {
    b.onclick = () => {
      const x = document.createElement("input");
      x.type = "text";
      b.before(x);
      x.focus();
    };
  });
  poserLesCalendriers(bloc);
}

/* ------------------------------------------------ les lignes de paie */

/** La rubrique de code `code`, telle que la route etat la décrit (`rubriques`). */
function rubriqueDe(code) {
  return ETAT.rubriques.find((r) => r.code === code);
}

/** LA LIGNE QUE LA ROUTE LIT, depuis la ligne `l` telle qu'elle est écrite : sa rubrique et chacune des clés que sa rubrique
 *  dit (`cles`), rien d'autre ; une clé qu'elle ne porte pas encore est vide (`null`, `[]` pour des jours ou une liste). */
function ligneDonnee(l, rubrique) {
  const vide = (c) => (c.forme === "jours" || c.forme === "liste" ? [] : null);
  return Object.fromEntries(rubrique.cles.map((c) => [c.cle, l[c.cle] === undefined ? vide(c) : l[c.cle]]));
}

/** DES LIGNES DE PAIE, une par rangée, aux colonnes fixes : la rubrique (un choix parmi `offertes`, des codes), puis
 *  chaque clé que sa rubrique dit, chacune sa case et son libellé ; le bouton qui retire la ligne au bout. `choixDe(i)` :
 *  ce que les choix de la ligne `i` offrent en plus de ceux de sa rubrique. */
function lignesDePaie(lignes, offertes, choixDe) {
  const rubriques = offertes.map((code) => ({ code, libelle: rubriqueDe(code) ? rubriqueDe(code).libelle : code }));
  return `<div class="lignes-paie">${lignes.map((l, i) => {
    const r = rubriqueDe(l.rubrique);
    const autres = r ? r.cles.filter((c) => c.cle !== "rubrique") : [];
    const choix = { ...(r ? r.choix : {}), ...choixDe(i) };
    return `<div class="ligne-paie" data-ligne="${i}">
      <label class="colonne">${r ? echapper(r.cles.find((c) => c.cle === "rubrique").libelle) : ""} ${caseDeForme("choix_rendu", l.rubrique, { choix: rubriques }).replace("data-case", 'data-case data-cle="rubrique"')}</label>
      ${autres.map((c) => `<label class="colonne">${echapper(c.libelle)} ${caseDeForme(c.forme, l[c.cle], {
        unite: r.quantite, choix: choix[c.cle] }).replace("data-case", `data-case data-cle="${c.cle}"`)}</label>`).join("")}
      <button type="button" class="lien retirer-ligne" title="retirer cette ligne">&times;</button>
    </div>`;
  }).join("")}</div>`;
}

/** Les lignes que montrent les rangées de `bloc`, telles que la route les lit. Une ligne dont on vient de changer la
 *  rubrique garde les clés que la nouvelle dit aussi. */
function lireLesLignes(bloc) {
  return [...bloc.querySelectorAll(".ligne-paie")].map((rangee) => {
    const lu = {};
    rangee.querySelectorAll("[data-case][data-cle]").forEach((x) => {
      const v = lireLaCase(x);
      if (v !== undefined) lu[x.dataset.cle] = v;
    });
    const r = rubriqueDe(lu.rubrique);
    return r ? ligneDonnee(lu, r) : lu;
  });
}

/* ------------------------------------------------ les champs d'une fiche de route */

/** Les cases de `champs` (des champs d'une fiche) : chacune son libellé et sa case, ce que `valeurs` en dit d'abord, ses
 *  `choix` (ceux que le geste rend, sinon ceux de la fiche). */
function casesDesChamps(champs, valeurs, choix) {
  return champs.map((c, j) => `<label class="champ-boite" data-j="${j}">${echapper(c.libelle)}
      ${caseDeForme(c.forme, valeurs[c.nom], { choix: choix[c.nom] || c.choix })}</label>`).join("");
}

/** Ce que les cases de `champs` disent, dans `bloc` : un champ vide ne part pas. */
function lireLesChamps(bloc, champs) {
  const lu = {};
  champs.forEach((c, j) => {
    const v = lireLaCase(bloc.querySelector(`[data-j="${j}"] [data-case]`));
    if (v != null && !(Array.isArray(v) && !v.length)) lu[c.nom] = v;
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

/** Dessine dans `cible` (porteuse de `data-auto`, le nom d'une place) les routes automatiques de cette place, chacune
 *  depuis sa fiche (`ecran`, `affichage: "auto"`, `bouton`, ses champs). Une place sans route automatique ne montre rien. */
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
    brancherLesCases(bloc);
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
 *  `entete` : ce que la boîte dit de quoi il s'agit (`titre`, à la suite du libellé ; `dit`, dessous) ; `pardessus` :
 *  la boîte s'ouvre par-dessus celle qui est ouverte, qui revient quand elle se ferme. `apres` : après la réponse. */
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
  const rendre = entete.pardessus ? mettreDeCote() : null;
  ouvrirBoite(`
    <h3>${echapper(g.libelle)}${entete.titre ? ` — ${echapper(entete.titre)}` : ""}</h3>
    ${entete.dit ? `<p class="discret">${echapper(entete.dit)}</p>` : ""}
    <div id="geste-cases">${casesDuGeste(g)}</div>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="geste-ok">Enregistrer</button>
    </div>`, rendre);
  const bloc = document.getElementById("geste-cases");
  brancherLesCases(bloc);
  // Une case vidée ne part pas : la route dit ce qu'une case absente fait (une reprise absente s'efface).
  document.getElementById("geste-ok").onclick = () => partir(chargeDuGeste(bloc, g) || g.charge, true);
}
