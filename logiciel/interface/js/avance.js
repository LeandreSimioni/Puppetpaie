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

/** Les options d'un choix : une case vide d'abord (rien de choisi ne part ; son libellé est celui de la valeur offerte
 *  de code vide, quand la route en offre une : « à son terme », « modèle de base »), puis chaque valeur offerte, sous
 *  son groupe quand elle en dit un (`groupe`, dans l'ordre où ils viennent) ; une valeur posée que la liste n'offre pas
 *  reste offerte telle quelle (une fiche mise de côté). Une case vide que le moteur pré-remplit (`prerempli`) porte
 *  le libellé de ce qu'il prend : rien n'est choisi, et choisir cette valeur dans la liste l'écrit. */
function optionsDuChoix(v, choix, prerempli) {
  const offerts = choix || [];
  const vide = offerts.find((o) => o.code === "");
  const pleins = offerts.filter((o) => o.code !== "");
  const garde = v != null && v !== "" && !pleins.some((o) => o.code === String(v)) ? [{ code: String(v), libelle: String(v) }] : [];
  const montre = v == null && prerempli != null
    ? (pleins.find((o) => o.code === String(prerempli)) || { libelle: String(prerempli) }).libelle : null;
  const option = (o) => `<option value="${echapper(o.code)}"${o.code === String(v) ? " selected" : ""}>${echapper(o.libelle)}</option>`;
  const groupes = [...new Set(pleins.map((o) => o.groupe))];
  const corps = groupes.map((g) => (g == null ? pleins.filter((o) => o.groupe == null).map(option).join("")
    : `<optgroup label="${echapper(g)}">${pleins.filter((o) => o.groupe === g).map(option).join("")}</optgroup>`)).join("");
  const libelleDuVide = montre != null ? montre : vide ? vide.libelle : "";
  return `<option value="">${echapper(libelleDuVide)}</option>${corps}${garde.map(option).join("")}`;
}

/** Une valeur composée, en lecture : ses parties dites, « clé : valeur », ou une ligne par élément d'une liste. */
function objetEnClair(v) {
  if (v == null) return "—";
  if (Array.isArray(v)) return v.length ? v.map(objetEnClair).join(" ; ") : "—";
  if (typeof v === "object") {
    return Object.entries(v).filter(([, x]) => x != null && x !== "").map(([k, x]) => `${k} : ${objetEnClair(x)}`).join(" · ") || "—";
  }
  return String(v);
}

/** Les formes qui se tapent ou se choisissent d'une valeur simple : une valeur composée ne s'y écrit pas. */
const FORMES_SIMPLES = new Set(["texte", "nombre", "montant", "oui_non", "mois", "date", "choix", "choix_rendu"]);

/** Ce que le moteur prend pour une case vide (`o.prerempli`), montré dans la case sans y être écrit. */
const prerempliDe = (o, enClair = String) => (o.prerempli == null ? "" : ` placeholder="${echapper(enClair(o.prerempli))}"`);

/** UN MONTANT DANS SA CASE s'écrit comme la page écrit un montant (`euros`, « 1 053,08 »), sans l'euro (la ligne le dit
 *  après lui) ; il se tape avec une virgule ou un point, et le moteur le lit. */
const montantEcrit = (v) => (v == null || v === "" ? null : enEuros(v));

/** LA TABLE : à chaque forme, sa case (`html`, ce qu'elle montre de `v` ; `o.choix` : ce qu'un choix offre, `o.unite` :
 *  l'unité d'une quantité, `o.prerempli` : ce que le moteur prend pour elle vide, `o.parties` : les parties d'une valeur
 *  faite de parties) et sa lecture (`lire`, la valeur que la route lit ; `null` : une case vide). Chaque case porte
 *  `data-case`. Un jour et des jours s'ouvrent dans le calendrier (`js/calendrier.js`, `poserLesCalendriers`). */
const CASES = {
  texte: { html: (v, o) => `<input type="text" data-case${valeurDeDepart(v)}${prerempliDe(o)}>`, lire: tape },
  nombre: { html: (v, o) => `<input type="text" inputmode="decimal" data-case${valeurDeDepart(v)}${prerempliDe(o)}>`, lire: tape },
  // Un montant montré tel qu'il s'écrit (`montantEcrit`) ; laissé tel quel, il repart tel que le moteur l'a écrit.
  montant: {
    html: (v, o) => `<input type="text" inputmode="decimal" data-case${valeurDeDepart(montantEcrit(v))}${
      v == null ? "" : ` data-ecrit="${echapper(String(v))}"`}${prerempliDe(o, montantEcrit)}>`,
    lire: (x) => (x.dataset.ecrit != null && tape(x) === montantEcrit(x.dataset.ecrit) ? x.dataset.ecrit : tape(x)),
  },
  quantite: {
    html: (v, o) => `<input type="text" inputmode="numeric" data-case data-unite="${echapper(o.unite)}"${
      valeurDeDepart(v == null ? null : v[o.unite])}>`,
    lire: (x) => (tape(x) == null ? null : { [x.dataset.unite]: tape(x) }),
  },
  // UN OUI-NON PRÉ-REMPLI (`o.prerempli` : ce que le moteur prend sans lui) se montre tel que le moteur le prend, et se
  // change dans les deux sens : ce qui diffère du pré-rempli s'écrit, y revenir n'écrit rien. Sans pré-rempli, coché
  // s'écrit oui, décoché n'écrit rien.
  oui_non: {
    html: (v, o) => `<input type="checkbox" data-case${(v == null ? o.prerempli : v) ? " checked" : ""}${
      o.prerempli == null ? "" : ` data-prerempli="${o.prerempli ? "oui" : "non"}"`}>`,
    lire: (x) => (x.dataset.prerempli == null ? (x.checked ? true : null)
      : x.checked === (x.dataset.prerempli === "oui") ? null : x.checked),
  },
  mois: { html: (v) => `<input type="month" data-case${valeurDeDepart(v)}>`, lire: tape },
  date: {
    html: (v, o) => `<input type="text" data-case data-calendrier="jour" autocomplete="off"${
      o.prerempli == null ? ` placeholder="jj/mm/aaaa"` : prerempliDe(o, jjmmaaaa)}${valeurDeDepart(v == null ? null : jjmmaaaa(v))}>`,
    lire: lireUnJour,
  },
  // Des jours : le bouton qui les dit ouvre le calendrier où l'on coche ; un jour qui dit son propre lieu (`{jour, lieu}`)
  // a sa case à côté, tant qu'il reste coché. `o.mois` : le mois dont les jours se cochent ici (ceux d'avant se lisent,
  // ceux d'après sont ceux des mois suivants) ; `o.salarie` : de qui le calendrier montre les contrats passés.
  jours: {
    html: (v, o) => `<span class="jours-case" data-case data-valeur="${echapper(JSON.stringify(v || []))}"${
      o.mois ? ` data-mois="${echapper(o.mois)}"` : ""}${o.salarie ? ` data-salarie="${echapper(o.salarie)}"` : ""}>
      <button type="button" class="jours-coches" data-calendrier="jours">${echapper(joursDits(v || []))}</button>${
      lieuxDesJours(v || [])}</span>`,
    lire: (x) => JSON.parse(x.dataset.valeur).map((d) => {
      const iso = jourDe(d);
      const c = x.querySelector(`[data-lieu-jour="${iso}"]`);
      if (!c) return d;
      return c.value.trim() ? { jour: iso, lieu: c.value.trim() } : iso;
    }),
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
  // Un choix vide que le moteur pré-remplit (`o.prerempli` : le métier que la paie applique) montre ce qu'il prend dans
  // sa case vide ; choisir une valeur de la liste l'écrit, celle qui est montrée comprise ; le vide n'écrit aucune valeur : la carte n'en dit pas, et le pré-rempli se montre.
  choix: { html: (v, o) => `<select data-case>${optionsDuChoix(v, o.choix, o.prerempli)}</select>`, lire: (x) => x.value || null },
  choix_rendu: { html: (v, o) => `<select data-case>${optionsDuChoix(v, o.choix, o.prerempli)}</select>`, lire: (x) => x.value || null },
  // Une valeur composée que le moteur décrit par son sens : elle se lit, elle ne se saisit pas d'une case.
  objet: { html: (v) => `<span class="case-objet" data-case>${echapper(objetEnClair(v))}</span>`, lire: () => undefined },
  // UNE VALEUR FAITE DE PARTIES (`o.parties` : chacune sa clé, son libellé, sa forme) : chaque partie sa case ; celle
  // qui se choisit offre `o.choix`, et la valeur choisie dit les parties qu'elle ouvre (`ouvre`) : une partie qui n'est
  // pas de toujours ne se montre que si elle est ouverte. Vide de toutes ses parties, la valeur est vide.
  parties: {
    html: (v, o) => {
      const val = v || {};
      const parties = o.parties || [];
      const choisie = parties.filter((p) => p.forme === "choix_rendu" || p.forme === "choix")
        .map((p) => (o.choix || []).find((c) => c.code !== "" && c.code === String(val[p.cle]))).find(Boolean);
      const ouvertes = (choisie && choisie.ouvre) || [];
      return `<span class="case-parties" data-case>${parties.filter((p) => p.toujours || ouvertes.includes(p.cle)).map((p) =>
        `<label class="partie" data-partie="${echapper(p.cle)}">${echapper(p.libelle)} ${caseDeForme(p.forme, val[p.cle],
          { choix: p.forme === "choix_rendu" || p.forme === "choix" ? o.choix : undefined })}</label>`).join("")}</span>`;
    },
    lire: (x) => {
      const lu = {};
      x.querySelectorAll(":scope > [data-partie]").forEach((p) => { lu[p.dataset.partie] = lireLaCase(p.querySelector("[data-case]")); });
      return Object.values(lu).some((y) => y != null) ? lu : null;
    },
  },
};

/** Les jours qui disent leur propre lieu (`{jour, lieu}`) : une case chacun, le lieu du jour. */
function lieuxDesJours(jours) {
  const propres = jours.filter((d) => d && typeof d === "object" && d.lieu);
  return propres.map((d) => `<label class="lieu-du-jour">${echapper(jjmmaaaa(d.jour).slice(0, 5))}
    <input type="text" data-lieu-jour="${echapper(d.jour)}"${valeurDeDepart(d.lieu)}></label>`).join("");
}

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

/** LA CASE d'une forme, sans libellé ; une forme que la table n'a pas est une faute de la page : elle se dit. Une valeur
 *  composée que la route rend sous une forme simple (l'emploi d'une carte, un objet décrit « texte ») se montre en
 *  lecture, comme un objet : elle ne s'écrit pas dans une case de texte. */
function caseDeForme(forme, v, o = {}) {
  const f = v != null && typeof v === "object" && FORMES_SIMPLES.has(forme) ? "objet" : forme;
  const c = CASES[f];
  if (!c) throw new Error(`La forme « ${f} » n'a pas de case dans la page : le signaler au développeur.`);
  return c.html(v, o).replace("data-case", `data-case data-forme="${f}"`);
}

/** La valeur d'une case (l'élément qui porte `data-case`), lue par sa forme. */
function lireLaCase(x) {
  return CASES[x.dataset.forme].lire(x);
}

/** Branche ce qu'une case fait seule dans `bloc` : le « + » d'une liste, les calendriers (`o` : ce que le calendrier lit
 *  des mois passés, `o.passes(mois, salarie)`, quand l'écran qui l'appelle le sait). */
function brancherLesCases(bloc, o = {}) {
  bloc.querySelectorAll("[data-plus]").forEach((b) => {
    b.onclick = () => {
      const x = document.createElement("input");
      x.type = "text";
      b.before(x);
      x.focus();
    };
  });
  poserLesCalendriers(bloc, o);
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

/** Les clés d'une ligne : celles que sa rubrique dit (`cles`) ; une ligne sans rubrique ne dit que les clés dont les
 *  valeurs sont offertes à toutes les lignes (`communs` : la rubrique, parmi celles que la carte propose). */
function clesDeLaLigne(r, communs) {
  return r ? r.cles : ETAT.rubriques[0].cles.filter((c) => communs[c.cle]);
}

/** LES COLONNES DES LIGNES : chaque clé de base que leurs rubriques disent, dans l'ordre où elle vient, une colonne
 *  chacune ; les clés de jours d'une rubrique (`jours` : ceux du mois, puis ceux des mois suivants) en une seule colonne,
 *  qui se coche dans un seul calendrier. Une liste à cocher (les cases d'une ligne de frais) et une clé que la route place
 *  sous « Avancé » (`place`) ne font pas de colonne : elles se posent sous leur ligne. */
function colonnesDesLignes(lignes, communs) {
  const colonnes = [];
  const jours = { cle: null, cles: [], forme: "jours", libelle: "" };
  (lignes.length ? lignes : [{}]).forEach((l) => clesDeLaLigne(rubriqueDe(l.rubrique), communs).forEach((c) => {
    if (c.forme === "liste" || c.place === "avance") return;
    if (c.forme === "jours") {
      if (!jours.cle) { jours.cle = c.cle; jours.libelle = c.libelle; colonnes.push(jours); }
      if (!jours.cles.includes(c.cle)) jours.cles.push(c.cle);
      return;
    }
    if (!colonnes.some((x) => x.cle === c.cle)) colonnes.push({ cle: c.cle, cles: [c.cle], forme: c.forme, libelle: c.libelle });
  }));
  return colonnes;
}

/** La largeur d'une colonne, par sa forme : une liste où l'on choisit et un texte s'élargissent, un nombre reste court. */
const LARGEUR_DE_COLONNE = {
  choix_rendu: "minmax(8rem, 1.4fr)", choix: "minmax(8rem, 1.4fr)", texte: "minmax(8rem, 2fr)",
  quantite: "minmax(4rem, .5fr)", nombre: "minmax(4.5rem, .6fr)", montant: "minmax(4.5rem, .6fr)", jours: "minmax(7rem, 1fr)",
};

/** DES LIGNES DE PAIE, une par rangée, aux colonnes fixes et titrées une fois : chaque clé que sa rubrique dit, chacune
 *  sa case, puis la croix qui retire la ligne, au bout. Sous la ligne : ses cases à cocher (une liste, quand des valeurs
 *  sont offertes : sans elles, rien ne se coche et la valeur reste), et ce que sa rubrique dit d'elle-même (`dit`). Les
 *  mots de la rubrique (`mots` : « à », « € ») suivent ses nombres. `o.choix` : ce que les choix offrent à toutes les
 *  lignes (la rubrique) ; `choixDe(i)` : ce qu'ils offrent en plus à la ligne `i` ; `o.mois`, `o.salarie` : ceux de ses
 *  jours (la case des jours). */
function lignesDePaie(lignes, choixDe, o) {
  const colonnes = colonnesDesLignes(lignes, o.choix);
  const gabarit = colonnes.map((c) => LARGEUR_DE_COLONNE[c.forme] || "minmax(6rem, 1fr)").concat("2rem").join(" ");
  const tete = `<div class="ligne-paie tete">${colonnes.map((c) => `<span>${echapper(c.libelle)}</span>`).join("")}<span></span></div>`;
  return `<div class="lignes-paie" style="--colonnes: ${gabarit}">${lignes.length ? tete : ""}${lignes.map((l, i) => {
    const r = rubriqueDe(l.rubrique);
    const cles = clesDeLaLigne(r, o.choix);
    const choix = { ...o.choix, ...(r ? r.choix : {}), ...choixDe(i) };
    // Les nombres de la ligne, que les mots de sa rubrique suivent : le premier et le second, ou le seul.
    const nombres = cles.filter((c) => c.forme === "quantite" || c.forme === "nombre" || c.forme === "montant").map((c) => c.cle);
    const mot = (cle) => {
      if (!r || !nombres.includes(cle)) return "";
      const k = nombres.length > 1 ? nombres.indexOf(cle) : 1;
      return k < 2 ? `<span class="mot">${echapper(r.mots[k])}</span>` : "";
    };
    const cellule = (col) => {
      const siennes = cles.filter((c) => col.cles.includes(c.cle));
      if (!siennes.length) return `<span class="colonne vide"></span>`;
      const c = siennes[0];
      const marque = (h) => h.replace("data-case", `data-case data-cle="${c.cle}" data-cles="${siennes.map((x) => x.cle).join(" ")}"`);
      const v = c.forme === "jours" ? siennes.flatMap((x) => l[x.cle] || []) : l[c.cle];
      // Les choix de la rubrique s'expliquent au survol, quand elle dit leur aide (`lieu_aide`).
      const aide = r && r.lieu_aide && r.choix[c.cle] ? ` title="${echapper(r.lieu_aide)}"` : "";
      return `<label class="colonne"${aide}><span class="colonne-libelle">${echapper(c.libelle)}</span>${marque(caseDeForme(c.forme, v, {
        unite: r ? r.quantite : null, choix: choix[c.cle], mois: siennes.length > 1 ? o.mois : null, salarie: o.salarie }))}${mot(c.cle)}</label>`;
    };
    const coches = cles.filter((c) => c.forme === "liste" && c.place !== "avance" && choix[c.cle] && choix[c.cle].length);
    // Les clés que la route place sous « Avancé » (le territoire d'un repas ou d'une nuitée) : repliées sous la ligne,
    // ouvertes d'elles-mêmes quand l'une dit quelque chose.
    const avancees = cles.filter((c) => c.place === "avance");
    const dit = (x) => x != null && x !== "" && !(Array.isArray(x) && !x.length);
    const avance = avancees.length ? `<details class="ligne-avance"${avancees.some((c) => dit(l[c.cle])) ? " open" : ""}><summary>Avancé</summary>${
      avancees.map((c) => `<label class="colonne"${r && r.lieu_aide && r.choix[c.cle] ? ` title="${echapper(r.lieu_aide)}"` : ""}><span class="colonne-libelle">${
        echapper(c.libelle)}</span>${caseDeForme(c.forme, l[c.cle], { choix: choix[c.cle] }).replace("data-case", `data-case data-cle="${c.cle}"`)}</label>`).join("")}</details>` : "";
    const sous = [
      ...coches.map((c) => `<span class="colonne">${caseDeForme(c.forme, l[c.cle], { choix: choix[c.cle] })
        .replace("data-case", `data-case data-cle="${c.cle}"`)}</span>`),
      avance,
      r && r.dit ? `<span class="discret">${echapper(r.dit)}</span>` : "",
    ].filter(Boolean);
    return `<div class="ligne-paie" data-ligne="${i}" data-valeur="${echapper(JSON.stringify(l))}">
      ${colonnes.map(cellule).join("")}
      <button type="button" class="lien retirer-ligne" title="retirer cette ligne" aria-label="retirer cette ligne">&times;</button>
      ${sous.length ? `<div class="ligne-sous">${sous.join("")}</div>` : ""}
    </div>`;
  }).join("")}</div>`;
}

/** Les lignes que montrent les rangées de `bloc`, telles que la route les lit : ce que chaque case dit, par-dessus la
 *  ligne telle qu'elle était écrite (une clé qui ne se dessine pas garde sa valeur). Les jours d'une colonne partent tous
 *  dans sa première clé, les autres vides : la route les range par mois (`ligne::ranger`). Une ligne dont on vient de
 *  changer la rubrique garde les clés que la nouvelle dit aussi. */
function lireLesLignes(bloc) {
  return [...bloc.querySelectorAll(".ligne-paie[data-ligne]")].map((rangee) => {
    const lu = JSON.parse(rangee.dataset.valeur);
    rangee.querySelectorAll("[data-case][data-cle]").forEach((x) => {
      const v = lireLaCase(x);
      if (v === undefined) return;
      const [premiere, ...autres] = (x.dataset.cles || x.dataset.cle).split(" ");
      lu[premiere] = v;
      autres.forEach((k) => { lu[k] = []; });
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
