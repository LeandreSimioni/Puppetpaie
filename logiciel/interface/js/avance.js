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
const FORMES_SIMPLES = new Set(["texte", "texte_long", "nombre", "montant", "oui_non", "mois", "date", "choix", "choix_rendu"]);

/** Ce que le moteur prend pour une case vide (`o.prerempli`), montré dans la case sans y être écrit ; sans lui, l'exemple
 *  que la route donne d'une case vide (`o.exemple`), grisé. */
const prerempliDe = (o, enClair = String) => (o.prerempli != null ? ` placeholder="${echapper(enClair(o.prerempli))}"`
  : o.exemple ? ` placeholder="ex. ${echapper(o.exemple)}"` : "");

/** Une valeur dans un objet, par son chemin (« taux.t1_cadre »). */
const auChemin = (o, chemin) => chemin.split(".").reduce((x, k) => (x && typeof x === "object" ? x[k] : undefined), o);

/** Pose `v` dans l'objet `o` au bout de son chemin. */
function poserAuChemin(o, chemin, v) {
  const ks = chemin.split(".");
  ks.slice(0, -1).forEach((k) => { if (!o[k] || typeof o[k] !== "object") o[k] = {}; o = o[k]; });
  o[ks[ks.length - 1]] = v;
}

/** Une valeur qui ne dit rien, à quelque profondeur que ce soit. */
const rienDit = (v) => v == null || v === "" || (typeof v === "object" && Object.values(v).every(rienDit));

let NUMERO_DES_BOUTONS = 0;   // chaque groupe de boutons à cocher a son nom

/** LES PARTIES D'UNE VALEUR (`o.parties`), chacune sa case dans un `[data-partie]` : celles qui sont ouvertes (de toujours,
 *  ouvertes par la route, ou par la valeur choisie d'une autre partie : `ouvre`). Une partie dit ses choix (`choix`,
 *  sinon ceux de la valeur, `o.choix`), son pré-rempli, son exemple et son aide (la bulle) ; ses choix en boutons à
 *  cocher (`boutons`) ; un oui-non qui se choisit dans une liste (ses `choix` « true », « false »). */
function partiesDe(val, o) {
  const parties = o.parties || [];
  const choisie = parties.filter((p) => (p.forme === "choix_rendu" || p.forme === "choix") && !p.choix)
    .map((p) => (o.choix || []).find((c) => c.code !== "" && c.code === String(auChemin(val, p.cle)))).find(Boolean);
  const ouvertes = (choisie && choisie.ouvre) || [];
  return parties.filter((p) => p.toujours || p.ouverte || ouvertes.includes(p.cle)).map((p) => {
    const v = auChemin(val, p.cle);
    const choix = p.choix || ((p.forme === "choix_rendu" || p.forme === "choix") ? o.choix : undefined);
    const forme = p.boutons && choix ? "boutons" : p.forme === "oui_non" && choix ? "choix_oui_non" : p.forme;
    return `<label class="partie" data-partie="${echapper(p.cle)}"${p.aide ? ` title="${echapper(p.aide)}"` : ""}>${echapper(p.libelle)} ${
      caseDeForme(forme, v, { choix, prerempli: p.prerempli, exemple: p.exemple, booleen: p.forme === "oui_non" })}</label>`;
  }).join("");
}

/** CE QUE DISENT LES PARTIES de `x` (un élément qui porte `data-valeur`, la valeur telle qu'elle était, et `data-parties`,
 *  les chemins des parties que la route décrit) : la valeur d'avant, chaque partie montrée par-dessus, une partie décrite
 *  mais fermée vidée (elle n'a pas de sens pour ce qu'on saisit) ; une clé que la route ne décrit pas se garde. `sur` :
 *  la valeur que d'autres cases de la même clé ont déjà dite dans l'écran (« Pour démarrer » pose une ligne par partie
 *  qui manque), sur laquelle ses parties se posent à la place de la valeur d'avant. */
function lireLesParties(x, sur) {
  const lu = sur != null ? JSON.parse(JSON.stringify(sur)) : JSON.parse(x.dataset.valeur || "null") || {};
  const montrees = new Map([...x.querySelectorAll(":scope > [data-partie]")].map((p) => [p.dataset.partie, p]));
  (x.dataset.parties ? x.dataset.parties.split(" ") : []).forEach((k) => {
    const p = montrees.get(k);
    poserAuChemin(lu, k, p ? lireLaCase(p.querySelector("[data-case]")) : null);
  });
  return lu;
}

/** Les attributs d'un élément de parties : sa valeur d'avant et les chemins de ses parties. */
const attributsDesParties = (v, o) => ` data-valeur="${echapper(JSON.stringify(v == null ? null : v))}" data-parties="${
  echapper((o.parties || []).map((p) => p.cle).join(" "))}"`;

/** UN MONTANT DANS SA CASE s'écrit comme la page écrit un montant (`euros`, « 1 053,08 »), sans l'euro (la ligne le dit
 *  après lui) ; il se tape avec une virgule ou un point, et le moteur le lit. */
const montantEcrit = (v) => (v == null || v === "" ? null : enEuros(v));

/** Des cases à cocher, une par choix (`choix` : code, libellé, aide), cochées pour les codes de `v`. */
function casesACocher(v, choix) {
  return `<span data-case data-liste="coches">${choix.map((c) => `<label class="coche"${c.aide ? ` title="${echapper(c.aide)}"` : ""}>
          <input type="checkbox" value="${echapper(c.code)}"${(v || []).includes(c.code) ? " checked" : ""}> ${echapper(c.libelle)}</label>`).join("")}</span>`;
}

/** LA TABLE : à chaque forme, sa case (`html`, ce qu'elle montre de `v` ; `o.choix` : ce qu'un choix offre, `o.unite` :
 *  l'unité d'une quantité, `o.prerempli` : ce que le moteur prend pour elle vide, `o.parties` : les parties d'une valeur
 *  faite de parties) et sa lecture (`lire`, la valeur que la route lit ; `null` : une case vide). Chaque case porte
 *  `data-case`. Un jour et des jours s'ouvrent dans le calendrier (`js/calendrier.js`, `poserLesCalendriers`). */
const CASES = {
  texte: { html: (v, o) => `<input type="text" data-case${valeurDeDepart(v)}${prerempliDe(o)}>`, lire: tape },
  // UN TEXTE LONG, sur plusieurs lignes : une zone de texte, lue comme un texte.
  texte_long: { html: (v, o) => `<textarea rows="3" data-case${prerempliDe(o)}>${v == null ? "" : echapper(String(v))}</textarea>`, lire: tape },
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
  // s'écrit oui, décoché n'écrit rien. Un oui-non que la route exige (`o.obligatoire`) s'écrit toujours, oui ou non.
  oui_non: {
    html: (v, o) => `<input type="checkbox" data-case${(v == null ? o.prerempli : v) ? " checked" : ""}${
      o.prerempli == null ? "" : ` data-prerempli="${o.prerempli ? "oui" : "non"}"`}${o.obligatoire ? " data-obligatoire" : ""}>`,
    lire: (x) => (x.dataset.obligatoire != null ? x.checked
      : x.dataset.prerempli == null ? (x.checked ? true : null)
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
      ? casesACocher(v, o.choix)
      : `<span data-case data-liste="textes">${[...(v || []), ""].map((t) => `<input type="text"${valeurDeDepart(t)}>`).join("")}
          <button type="button" class="lien" data-plus>+</button></span>`),
    lire: (x) => [...x.querySelectorAll(x.dataset.liste === "coches" ? "input:checked" : "input")]
      .map((c) => c.value.trim()).filter(Boolean),
  },
  // Plusieurs valeurs d'une liste fermée : une case à cocher par choix, son libellé à côté ; les codes cochés, dans
  // l'ordre de la liste.
  plusieurs_choix: {
    html: (v, o) => casesACocher(v, o.choix || []).replace(/<\/span>$/,
      `<button type="button" class="lien" data-tout-cocher>${aideDe("tout_cocher")}</button></span>`),
    lire: (x) => [...x.querySelectorAll("input:checked")].map((c) => c.value),
  },
  // Plusieurs valeurs d'une liste que la route rend à côté (`choix` du geste, sous la clé de la case) : la même case.
  get plusieurs_choix_rendus() { return CASES.plusieurs_choix; },
  // Une période (un mois, une année, toutes les années) : une liste déroulante des périodes que la route offre, les
  // mois sous leur année.
  periode: { html: (v, o) => `<select data-case>${optionsDuChoix(v, o.choix)}</select>`, lire: (x) => x.value || null },
  // Un choix vide que le moteur pré-remplit (`o.prerempli` : le métier que la paie applique) montre ce qu'il prend dans
  // sa case vide ; choisir une valeur de la liste l'écrit, celle qui est montrée comprise ; le vide n'écrit aucune valeur : la carte n'en dit pas, et le pré-rempli se montre.
  choix: { html: (v, o) => `<select data-case>${optionsDuChoix(v, o.choix, o.prerempli)}</select>`, lire: (x) => x.value || null },
  choix_rendu: { html: (v, o) => `<select data-case>${optionsDuChoix(v, o.choix, o.prerempli)}</select>`, lire: (x) => x.value || null },
  // Une valeur composée que le moteur décrit par son sens : elle se lit, elle ne se saisit pas d'une case.
  objet: { html: (v) => `<span class="case-objet" data-case>${echapper(objetEnClair(v))}</span>`, lire: () => undefined },
  // UNE VALEUR FAITE DE PARTIES (`partiesDe`, `lireLesParties`). Vide de tout ce qu'elle dit, la valeur est vide.
  parties: {
    html: (v, o) => `<span class="case-parties" data-case${attributsDesParties(v, o)}>${partiesDe(v || {}, o)}</span>`,
    lire: (x) => { const lu = lireLesParties(x); return rienDit(lu) ? null : lu; },
  },
  // DES RANGÉES (les autres établissements) : chacune ses parties (`o.parties`), une croix qui la retire, et sous elle
  // ses temps (`o.temps.lignes`, par la clé de la rangée) ; le bouton que la route nomme en ajoute une (`o.ajouter`). Des
  // rangées que la route ne fait pas ajouter (aucun `ajouter` : l'avenant qu'un envoi fait partir) ne se retirent pas
  // non plus : seules leurs parties se saisissent. Une rangée qui ne dit rien ne part pas ; plus aucune rangée, la valeur
  // est vide.
  rangees: {
    html: (v, o) => {
      const tempsDe = (l) => {
        const t = o.temps && l && l.nic ? o.temps.lignes.find((x) => x.cle === l.nic) : null;
        return t ? t.cases.flatMap((c) => [c.avant, c.a_venir]).filter(Boolean)
          .map((x) => `<small class="case-dit">${echapper(x.valeur)} · ${echapper(x.dit)}</small>`).join("") : "";
      };
      const retirer = o.ajouter ? `<button type="button" class="lien retirer-rangee" title="retirer" aria-label="retirer">&times;</button>` : "";
      const rangee = (l) => `<div class="rangee"><span class="case-parties" data-rangee${attributsDesParties(l, o)}>${partiesDe(l || {}, o)}</span>
        ${retirer}${tempsDe(l)}</div>`;
      return `<div class="case-rangees" data-case>${(Array.isArray(v) ? v : []).map(rangee).join("")}${o.ajouter
        ? `<template>${rangee(null)}</template><button type="button" class="secondaire" data-rangee-plus>${echapper(o.ajouter)}</button>` : ""}</div>`;
    },
    lire: (x) => {
      const l = [...x.querySelectorAll(":scope > .rangee > [data-rangee]")].map(lireLesParties).filter((r) => !rienDit(r));
      return l.length ? l : null;
    },
  },
  // DES BOUTONS À COCHER (les choix d'une partie ou d'une case, chacun une phrase entière) ; un oui-non y écrit vrai ou
  // faux. Une case vide que le moteur pré-remplit (`o.prerempli`) montre coché ce qu'il prend : le garder n'écrit rien,
  // en cocher un autre l'écrit. Un choix qui a son exemple (`exemple` : le document que la route nomme) l'ouvre à côté.
  boutons: {
    html: (v, o) => {
      const nom = `boutons-${++NUMERO_DES_BOUTONS}`;
      const coche = v != null ? String(v) : o.prerempli != null ? String(o.prerempli) : null;
      const exemple = (c) => (c.exemple ? ` <a class="lien" href="${echapper(adresseDuDocument(c.exemple))}" target="_blank" rel="noopener">${
        echapper(c.exemple.libelle)}</a>` : "");
      return `<span class="boutons-choix" role="radiogroup" data-case${o.booleen ? " data-booleen" : ""}${
        v == null && o.prerempli != null ? ` data-prerempli="${echapper(String(o.prerempli))}"` : ""}>${(o.choix || []).filter((c) => c.code !== "")
        .map((c) => `<span class="choix-et-exemple"><label class="coche"><input type="radio" name="${nom}" value="${echapper(c.code)}"${coche === c.code ? " checked" : ""}> ${
          echapper(c.libelle)}</label>${exemple(c)}</span>`).join("")}</span>`;
    },
    lire: (x) => {
      const c = x.querySelector("input:checked");
      if (!c || c.value === x.dataset.prerempli) return null;
      return x.dataset.booleen != null ? c.value === "true" : c.value;
    },
  },
  // UN OUI-NON QUI SE CHOISIT DANS UNE LISTE (« true », « false ») : il écrit vrai ou faux.
  choix_oui_non: {
    html: (v, o) => `<select data-case>${optionsDuChoix(v == null ? null : String(v), o.choix, null)}</select>`,
    lire: (x) => (x.value === "" ? null : x.value === "true"),
  },
  // DU HTML COLLÉ (la signature des courriels), tel que le moteur le rend, nettoyé ; vide, son exemple. Le navigateur
  // récrit le HTML qu'il montre : non touché (`data-rendu`, posé par `brancherLesCases`), il repart tel que la route l'a
  // rendu.
  html: {
    html: (v, o) => `<div class="case-html" contenteditable="true" data-case${o.exemple ? ` data-exemple="${echapper(o.exemple)}"` : ""}${
      v == null ? "" : ` data-valeur="${echapper(String(v))}"`}>${v == null ? "" : String(v)}</div>`,
    lire: (x) => {
      if (x.dataset.rendu != null && x.innerHTML === x.dataset.rendu) return x.dataset.valeur == null ? null : x.dataset.valeur;
      return x.textContent.trim() || x.querySelector("img") ? x.innerHTML.trim() : null;
    },
  },
  // UN FICHIER QUE LA PERSONNE CHOISIT SUR SON POSTE, des types que la fiche admet (`o.admis`) : ses octets partent en
  // base64, lus dès qu'il est choisi (`brancherLesCases`) ; rien de choisi, la case est vide.
  fichier: {
    html: (v, o) => `<input type="file" data-case${o.admis ? ` accept="${echapper(o.admis.join(","))}"` : ""}>`,
    lire: (x) => (x.dataset.contenu == null ? null : x.dataset.contenu),
  },
  // UN MOT DE PASSE QU'UN HUMAIN TAPE : masqué, jamais pré-rempli, que le navigateur ne propose ni ne génère (la forme ne
  // dit pas si c'est l'actuel ou un neuf : `off`, comme la boîte du mot de passe) ; le journal des gestes de la page ne
  // dit que qu'il a été tapé (`js/tuyau.js`). Il part tel qu'il est tapé, blancs compris.
  mot_de_passe: { html: () => `<input type="password" autocomplete="off" data-case>`, lire: (x) => (x.value === "" ? null : x.value) },
};

/** L'ADRESSE OÙ LA FENÊTRE SERT UN DOCUMENT QUE LA ROUTE NOMME (`adresse`, `routes::ouverture::DocumentMontre`), refaite à
 *  chaque fois : le document a pu changer depuis. */
const adresseDuDocument = (d) => `${d.adresse}${d.adresse.includes("?") ? "&" : "?"}jeton=${encodeURIComponent(JETON)}&t=${Date.now()}`;

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

/** UN FICHIER DÉPOSÉ SUR `zone` (glissé depuis le poste) : `preneur()`, au moment du glisser, rend ce qui le prend (une
 *  fonction des fichiers déposés), ou rien quand personne ne le prend ici. Un dépôt qui ne porte pas de fichier ne fait
 *  rien. La zone ne se branche qu'une fois (la boîte sert à toutes les boîtes). */
function recevoirLeFichier(zone, preneur) {
  if (zone.dataset.depot) return;
  zone.dataset.depot = "1";
  const porteUnFichier = (ev) => ev.dataTransfer && [...ev.dataTransfer.types].includes("Files");
  zone.addEventListener("dragover", (ev) => {
    if (!porteUnFichier(ev) || !preneur()) return;
    ev.preventDefault();
    zone.classList.add("depot-survole");
  });
  zone.addEventListener("dragleave", (ev) => { if (!zone.contains(ev.relatedTarget)) zone.classList.remove("depot-survole"); });
  zone.addEventListener("drop", (ev) => {
    zone.classList.remove("depot-survole");
    if (!porteUnFichier(ev) || !ev.dataTransfer.files.length) return;
    ev.preventDefault();
    const prendre = preneur();
    if (!prendre) return;
    ev.stopPropagation();
    prendre(ev.dataTransfer.files);
  });
}

/** Ce qui pose un dépôt dans la case fichier `x` (`deposerLeFichier`), si elle est à l'écran. */
const dansLaCase = (x) => (x && x.isConnected ? (fichiers) => deposerLeFichier(x, fichiers) : null);

/** La case fichier de `bloc` quand il n'en a qu'une, sinon rien. */
function seuleCaseFichier(bloc) {
  const l = bloc.querySelectorAll('input[type="file"][data-case]');
  return l.length === 1 ? l[0] : null;
}

/** Pose `fichiers` (ceux d'un dépôt) dans la case fichier `x`, et la fait lire. */
function deposerLeFichier(x, fichiers) {
  x.files = fichiers;
  x.dispatchEvent(new Event("change", { bubbles: true }));
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
  bloc.querySelectorAll(".case-html").forEach((x) => { if (x.dataset.rendu == null) x.dataset.rendu = x.innerHTML; });
  // Tout cocher d'une case à plusieurs choix, ou tout décocher quand tout l'est déjà.
  bloc.querySelectorAll("[data-tout-cocher]").forEach((b) => {
    b.onclick = () => {
      const cases = [...b.parentElement.querySelectorAll('input[type="checkbox"]')];
      const cocher = cases.some((c) => !c.checked);
      cases.forEach((c) => { c.checked = cocher; });
      b.parentElement.dispatchEvent(new Event("change", { bubbles: true }));
    };
  });
  // Un fichier choisi se lit en base64 (sans l'en-tête de l'adresse data:) ; le moteur reconnaît son format à ses
  // premiers octets, pas à son extension.
  bloc.querySelectorAll('input[type="file"][data-case]').forEach((x) => {
    x.onchange = () => {
      delete x.dataset.contenu;
      const f = x.files && x.files[0];
      if (!f) return;
      const l = new FileReader();
      // Lu, il le dit (`case-lue`) : l'aperçu d'un geste attend ses octets (`montrerLApercu`).
      l.onload = () => {
        x.dataset.contenu = String(l.result).split(",").pop();
        x.dispatchEvent(new Event("case-lue", { bubbles: true }));
      };
      l.onerror = () => dire(String(l.error), "erreur");
      l.readAsDataURL(f);
    };
    // Un fichier se dépose aussi sur sa case, ou sur la boîte quand elle n'en a qu'une : c'est le même choix.
    recevoirLeFichier(x.closest(".case") || x, () => dansLaCase(x));
    const boite = x.closest("#boite");
    if (boite) recevoirLeFichier(boite, () => dansLaCase(seuleCaseFichier(boite)));
  });
  // Les rangées : « + » en ajoute une (le modèle vide), la croix retire la sienne ; la case le dit comme une saisie.
  const changer = (x) => x.dispatchEvent(new Event("change", { bubbles: true }));
  bloc.querySelectorAll("[data-rangee-plus]").forEach((b) => {
    b.onclick = () => {
      b.before(b.previousElementSibling.content.cloneNode(true));
      brancherLesCases(b.parentElement, o);
      changer(b.parentElement);
    };
  });
  bloc.querySelectorAll(".retirer-rangee").forEach((b) => {
    b.onclick = () => { const liste = b.closest(".case-rangees"); b.closest(".rangee").remove(); changer(liste); };
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
 *  jours (la case des jours). `o.bulles` : ce qu'une route dit d'une rubrique (`bulleDeLaLigne`), sous son code ; une
 *  ligne dont la rubrique en a une porte un « i » au bout, qui ouvre sa bulle sous elle (`o.ouvertes` : les rangs des
 *  lignes dont la bulle est ouverte). */
function lignesDePaie(lignes, choixDe, o) {
  const colonnes = colonnesDesLignes(lignes, o.choix);
  const bulleDe = (l) => (o.bulles && l.rubrique ? o.bulles[l.rubrique] : null);
  const avecBulles = lignes.some(bulleDe);
  const gabarit = colonnes.map((c) => LARGEUR_DE_COLONNE[c.forme] || "minmax(6rem, 1fr)").concat("2rem", avecBulles ? ["1.5rem"] : []).join(" ");
  const tete = `<div class="ligne-paie tete">${colonnes.map((c) => `<span>${echapper(c.libelle)}</span>`).join("")}<span></span>${avecBulles ? "<span></span>" : ""}</div>`;
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
    const bulle = bulleDe(l);
    const ouverte = !!(bulle && o.ouvertes && o.ouvertes.has(i));
    const porte = !avecBulles ? "" : bulle
      ? `<button type="button" class="lien bulle-i" data-bulle="${i}" aria-expanded="${ouverte}" title="${echapper(bulle.tete)}" aria-label="${echapper(bulle.etiquette)}">i</button>`
      : "<span></span>";
    return `<div class="ligne-paie" data-ligne="${i}" data-valeur="${echapper(JSON.stringify(l))}">
      ${colonnes.map(cellule).join("")}
      <button type="button" class="lien retirer-ligne" title="retirer cette ligne" aria-label="retirer cette ligne">&times;</button>
      ${porte}
      ${sous.length ? `<div class="ligne-sous">${sous.join("")}</div>` : ""}
      ${bulle ? bulleDeLaLigne(bulle, ouverte) : ""}
    </div>`;
  }).join("")}</div>`;
}

/** LA BULLE D'UNE LIGNE : ce que la route dit de sa rubrique (`tete`, `montants` : chacun sa `condition` et son
 *  `montant`, `texte`), telle qu'elle le rend. Elle n'est pas une case : elle n'écrit rien. Fermée, elle est cachée. */
function bulleDeLaLigne(b, ouverte) {
  return `<div class="ligne-bulle"${ouverte ? "" : " hidden"}>
    <div><b>${echapper(b.tete)}</b></div>
    <table>${b.montants.map((m) => `<tr><td>${echapper(m.condition)}</td><td class="montant">${echapper(euros(m.montant))}</td></tr>`).join("")}</table>
    <span class="bulle-texte">${echapper(b.texte)}</span>
    ${b.ne_se_lit_pas.map((p) => `<span class="discret">${echapper(p)}</span>`).join("")}
  </div>`;
}

/** Le « i » d'une ligne ouvre sa bulle, un second clic la ferme ; `ouvertes` garde les rangs des bulles ouvertes, pour
 *  qu'elles le restent quand les lignes se redessinent. */
function brancherLesBulles(bloc, ouvertes) {
  bloc.querySelectorAll(".bulle-i").forEach((b) => {
    b.onclick = () => {
      const i = +b.dataset.bulle;
      const bulle = b.closest(".ligne-paie").querySelector(".ligne-bulle");
      bulle.hidden = !bulle.hidden;
      b.setAttribute("aria-expanded", String(!bulle.hidden));
      if (bulle.hidden) ouvertes.delete(i); else ouvertes.add(i);
    };
  });
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

/* ------------------------------------------------ une case qu'une route décrit */

/** UNE CASE QU'UNE ROUTE DÉCRIT, et la seule façon d'en dessiner une (la carte d'un engagement, une fiche, la boîte d'un
 *  geste, les champs d'une route) : `c.cle`, `c.libelle`, `c.forme`, `c.aide` (la bulle au survol), `c.valeur`,
 *  `c.prerempli` (ce que le moteur prend pour elle vide), `c.choix` (en boutons à cocher quand `c.boutons`), `c.parties` (`caseDeForme`), `c.temps` (la valeur
 *  d'avant et celle à venir, en mots). Sous la case : ce que la valeur choisie dit d'elle-même (`dit` d'un choix : un
 *  motif de rupture), puis ses temps ; sa faute se pose à part (`fautesDesCases`), à chaque réponse. */
function caseDeRoute(c) {
  const choisi = (c.choix || []).find((o) => o.code !== "" && o.code === String(c.valeur));
  const temps = c.temps ? [c.temps.avant, c.temps.a_venir].filter(Boolean).map((l) => `<small class="case-dit">${
    echapper(l.valeur)} · ${echapper(l.dit)}${l.note ? ` · ${echapper(l.note)}` : ""}</small>`).join("") : "";
  // Des choix que la route dit en boutons à cocher (`boutons`) se cochent.
  const forme = c.boutons && c.choix ? "boutons" : c.forme;
  // Une case faite de plusieurs contrôles n'est pas un <label> : un clic n'y vise pas le premier.
  const balise = ["parties", "rangees", "html", "boutons", "plusieurs_choix", "plusieurs_choix_rendus"].includes(forme) ? "div" : "label";
  return `<${balise} class="case${c.largeur === "pleine" ? " pleine" : ""}" data-cle="${echapper(c.cle)}"${c.aide ? ` title="${echapper(c.aide)}"` : ""}><span class="case-libelle">${echapper(c.libelle)}</span>
    ${caseDeForme(forme, c.valeur, { choix: c.choix, prerempli: c.prerempli, exemple: c.exemple, parties: c.parties, ajouter: c.ajouter, temps: c.temps, admis: c.admis, obligatoire: c.obligatoire })}${
    choisi && choisi.dit ? `<small class="case-dit">${echapper(choisi.dit)}</small>` : ""}${temps}<span class="case-dits" data-dits>${ditsDe(c)}</span>${
    c.renvoi ? `<button type="button" class="lien case-renvoi" data-renvoi="${echapper(JSON.stringify(c.renvoi))}">${echapper(c.renvoi.libelle)}</button>` : ""}</${balise}>`;
}

/** Ce que le moteur dit sous une case (`dit`), une ligne chacune. */
const ditsDe = (c) => (c.dit || []).map((l) => `<small class="case-dit">${echapper(l)}</small>`).join("");

/** Une case d'une chose qui existe (une fiche, la boîte qui modifie) : un oui-non y montre comme un pré-rempli sa valeur
 *  d'avant la saisie (`depart`, par défaut celle qu'elle montre) : le changer l'écrit, y revenir n'écrit rien
 *  (`CASES.oui_non`, `lireLesCases`). */
const modifiable = (c, depart = c.valeur) => (c.forme === "oui_non" && depart != null
  ? { ...c, valeur: c.valeur === depart ? null : c.valeur, prerempli: depart } : c);

/** Les cases en grille, comme sur la carte (`.e-cases` ; `classe` : celle d'une fiche, deux par ligne) : celles de base,
 *  puis celles que la route place sous « Avancé », repliées, ouvertes d'elles-mêmes quand `ouvert`. */
function grilleDesCases(base, avancees, ouvert, classe = "") {
  return `<div class="e-cases ${classe}">${base}</div>${avancees ? `<details class="e-avance"${ouvert ? " open" : ""}><summary>Avancé</summary>
    <div class="e-cases ${classe}">${avancees}</div></details>` : ""}`;
}

/** CE QUE LES CASES DE `bloc` DISENT, sous la clé de chacune (une case vidée : `null`). Comme sur la carte, une case qui
 *  ne dit rien de plus que ce qu'elle montrait ne se nomme pas, et la route garde sa valeur : un oui-non qu'on n'a pas
 *  changé (`modifiable`), une valeur composée qui ne se saisit pas. */
function lireLesCases(bloc) {
  return Object.fromEntries(lesCasesLues(bloc));
}

/** Chaque case de `bloc` qui dit quelque chose, dans l'ordre de l'écran : `[clé, valeur]`, une paire par case, même
 *  quand deux cases ont la même clé (« Pour démarrer » en pose une par fiche). */
function lesCasesLues(bloc) {
  return [...bloc.querySelectorAll(".case[data-cle]")].flatMap((c) => {
    const x = c.querySelector(":scope > [data-case]");
    const v = lireLaCase(x);
    if (v === undefined || (v === null && x.dataset.prerempli != null)) return [];
    return [[c.dataset.cle, v]];
  });
}

/** LES FAUTES DES CASES de `cible`, telles que la route les rend (`anomalie` de chaque case de `cases`) : la case marquée,
 *  le message dessous ; une case sans faute n'en dit rien. Un tiroir « Avancé » qui tient une erreur s'ouvre de lui-même. */
function fautesDesCases(cible, cases) {
  cases.forEach((c) => {
    const bloc = cible.querySelector(`.case[data-cle="${CSS.escape(c.cle)}"]`);
    if (!bloc) return;
    const a = c.anomalie;
    const x = bloc.querySelector(":scope > [data-case]");
    x.classList.toggle("faute", !!a && a.gravite === "erreur");
    x.classList.toggle("suspect", !!a && a.gravite === "avertissement");
    const avant = bloc.querySelector(":scope > .anomalie");
    if (avant) avant.remove();
    if (a) bloc.insertAdjacentHTML("beforeend", `<small class="anomalie ${echapper(a.gravite)}">${echapper(a.message)}</small>`);
    const tiroir = a && a.gravite === "erreur" ? bloc.closest("details") : null;
    if (tiroir) tiroir.open = true;
  });
}

/* ------------------------------------------------ les champs d'une fiche de route */

/** Les cases de `champs` (des champs d'une fiche) : chacune sa case (`caseDeRoute`), ce que `valeurs` en dit d'abord, ses
 *  `choix` (ceux que le geste rend, sinon ceux de la fiche), ses parties quand le geste les rend (`o.parties`, sous la clé
 *  de la case), et pour une case vide l'exemple que la fiche de la route
 *  donne de ce champ (`o.exemple`, l'exemple de la fiche : une valeur simple). `o.modifie` : la boîte modifie une chose
 *  qui existe (`modifiable`), et les champs que la fiche dit avancés (`avance`) se rangent sous « Avancé ». */
function casesDesChamps(champs, valeurs, choix, o = {}) {
  const exemple = (nom) => {
    const x = o.exemple ? o.exemple[nom] : undefined;
    return typeof x === "string" || typeof x === "number" ? String(x) : undefined;
  };
  const une = (c) => {
    const d = { cle: c.nom, libelle: c.libelle, forme: c.forme, valeur: valeurs[c.nom], choix: choix[c.nom] || c.choix,
      exemple: exemple(c.nom), admis: c.admis, obligatoire: c.obligatoire, parties: o.parties ? o.parties[c.nom] : undefined };
    return caseDeRoute(o.modifie ? modifiable(d) : d);
  };
  const avance = (c) => !!o.modifie && !!c.avance;
  return grilleDesCases(champs.filter((c) => !avance(c)).map(une).join(""), champs.filter(avance).map(une).join(""), false);
}

/** Ce que les cases de `champs` disent, dans `bloc` : un champ vide ne part pas. */
function lireLesChamps(bloc, champs) {
  const lu = lireLesCases(bloc);
  return Object.fromEntries(champs.filter((c) => lu[c.nom] != null && !(Array.isArray(lu[c.nom]) && !lu[c.nom].length))
    .map((c) => [c.nom, lu[c.nom]]));
}

/** LES CASES QUE LA ROUTE EXIGE ET QUE `charge` NE DIT PAS (`obligatoire` de chaque champ de sa fiche) : chacune marquée
 *  dans `bloc`, son tiroir « Avancé » ouvert ; les autres, démarquées. Vrai s'il en manque : la charge ne part pas. */
function marquerLesManquantes(bloc, champs, charge) {
  const manque = (c) => c.obligatoire && charge[c.nom] == null;
  fautesDesCases(bloc, champs.map((c) => ({ cle: c.nom, anomalie: manque(c) ? { gravite: "erreur", message: "À remplir." } : null })));
  return champs.some(manque);
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
      ${casesDesChamps(f.charge, {}, {}, { exemple: f.exemple })}
      <p><button class="secondaire" data-faire>${echapper(f.bouton)}</button></p>
      <div data-rendu></div>
    </div>`).join("");
  cible.querySelectorAll(".avance-route").forEach((bloc) => {
    const f = routes[+bloc.dataset.i];
    brancherLesCases(bloc);
    bloc.querySelector("[data-faire]").onclick = async () => {
      const rendu = bloc.querySelector("[data-rendu]");
      const charge = lireLesChamps(bloc, f.charge);
      if (marquerLesManquantes(bloc, f.charge, charge)) return;
      try {
        const r = await api(f.nom, charge);
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

/** LA FORME DU BOUTON D'UN GESTE, depuis ce que la route en dit, et rien d'autre : le geste principal de son écran (son
 *  `rang`) est un bouton principal, un geste second un lien ; celui qui exige le mot de passe (`demande`) est un envoi
 *  (une fictive le dit simulé). */
function classeDuGeste(g) {
  return `${g.rang === "principal" ? "primaire" : "lien"}${g.demande ? " envoi" : ""}`;
}

/** Les boutons des gestes, chacun son libellé et sa forme (`classeDuGeste`) ; `garde` : ceux-là seulement (chacun garde
 *  son rang dans `gestes`). */
function boutonsDesGestes(gestes, garde = () => true) {
  return gestes.map((g, k) => (garde(g)
    ? `<button class="${classeDuGeste(g)}" data-geste="${k}">${echapper(g.libelle)}</button>` : "")).join("");
}

/** Un geste dont la route est avancée (sa fiche : `avance`) se range sous « Avancé ». */
const gesteAvance = (g) => !!CATALOGUE[g.route].avance;

/** Les boutons des gestes, ceux dont la route est avancée rangés sous « Avancé ». */
function boutonsRanges(gestes) {
  return `${boutonsDesGestes(gestes, (x) => !gesteAvance(x))}${gestes.some(gesteAvance)
    ? `<details class="e-avance"><summary>Avancé</summary>${boutonsDesGestes(gestes, gesteAvance)}</details>` : ""}`;
}

/** Les champs de la fiche de la route du geste que sa boîte fait saisir (`cases`), dans l'ordre de la fiche. */
function champsDuGeste(g) {
  return CATALOGUE[g.route].charge.filter((c) => g.cases.includes(c.nom));
}

/** Les choix des cases du geste qui se choisissent (`choix`, sous la clé de chaque case), en paires `[clé, choix]`. */
const choixDuGeste = (g) => Object.entries(g.choix || {}).filter(([, l]) => Array.isArray(l));

/** Les clés des cases du geste dont un choix ouvre d'autres cases (`ouvre` d'un choix). */
const clesQuiOuvrent = (g) => choixDuGeste(g).filter(([, l]) => l.some((c) => c.ouvre)).map(([cle]) => cle);

/** LES CHAMPS DU GESTE QUI SONT OUVERTS quand ses cases disent `valeurs`, avec la règle des parties (`partiesDe`) : une
 *  case qu'aucun choix n'ouvre est toujours là ; une case qu'un choix ouvre (`ouvre`) n'est là que quand l'un des choix
 *  qui l'ouvrent est choisi. */
function champsOuverts(g, valeurs) {
  const tous = choixDuGeste(g);
  const choisis = tous.map(([cle, l]) => l.find((c) => c.code !== "" && valeurs[cle] != null && c.code === String(valeurs[cle])))
    .filter(Boolean);
  return champsDuGeste(g).filter((c) => {
    const ouvrent = tous.flatMap(([, l]) => l).filter((x) => (x.ouvre || []).includes(c.nom));
    return !ouvrent.length || choisis.some((x) => ouvrent.includes(x));
  });
}

/** Les gestes dont un choix ouvre des cases, sous le numéro de leur bloc (`data-ouvre-geste`), pour le redessiner. */
const GESTES_QUI_OUVRENT = new Map();
let NUMERO_DES_GESTES = 0;

/** Les cases ouvertes du geste `g` quand elles disent `valeurs` (`champsOuverts`). */
const casesOuvertes = (g, valeurs, o) =>
  casesDesChamps(champsOuverts(g, valeurs), valeurs, g.choix, { ...o, exemple: CATALOGUE[g.route].exemple, parties: g.parties });

/** Ce que la boîte d'un geste dit (`phrases`), puis ses cases ouvertes (`champsOuverts`) ; `o.modifie` : voir
 *  `casesDesChamps`. Quand un choix du geste ouvre des cases, elles sont dans un bloc qui se redessine quand ce choix
 *  change (`data-ouvre-geste`). */
function casesDuGeste(g, o = {}) {
  const phrases = g.phrases.map((p) => `<p class="note">${echapper(p)}</p>`).join("");
  const cases = casesOuvertes(g, g.valeurs, o);
  if (!clesQuiOuvrent(g).length) return `${phrases}
    ${cases}`;
  // Les blocs d'une boîte fermée ne sont plus dans la page : leur geste s'oublie.
  GESTES_QUI_OUVRENT.forEach((_, k) => { if (!document.querySelector(`[data-ouvre-geste="${k}"]`)) GESTES_QUI_OUVRENT.delete(k); });
  const n = String(++NUMERO_DES_GESTES);
  GESTES_QUI_OUVRENT.set(n, { g, o, tape: {} });
  return `${phrases}
    <div data-ouvre-geste="${n}">${cases}</div>`;
}

// Un choix qui ouvre des cases change : son bloc se redessine avec les cases que le choix fait ouvre, chaque case
// gardant ce qu'on y a tapé, même fermée puis rouverte (`tape`) ; une case fermée n'est pas dans la page, et ne part
// donc pas dans la charge. Une case fichier ne se redessine pas avec son fichier : elle reste telle quelle, choisie.
document.addEventListener("change", (ev) => {
  const bloc = ev.target.closest && ev.target.closest("[data-ouvre-geste]");
  const geste = bloc && GESTES_QUI_OUVRENT.get(bloc.dataset.ouvreGeste);
  const c = geste && ev.target.closest(".case[data-cle]");
  if (!c || !clesQuiOuvrent(geste.g).includes(c.dataset.cle)) return;
  geste.tape = { ...geste.tape, ...lireLesCases(bloc) };
  const fichiers = [...bloc.querySelectorAll(".case[data-cle]")].filter((x) => x.querySelector(':scope > input[type="file"][data-case]'));
  bloc.innerHTML = casesOuvertes(geste.g, { ...geste.g.valeurs, ...geste.tape }, geste.o);
  fichiers.forEach((x) => {
    const neuve = bloc.querySelector(`.case[data-cle="${CSS.escape(x.dataset.cle)}"]`);
    if (neuve) neuve.replaceWith(x);
  });
  brancherLesCases(bloc);
});

/** L'APERÇU D'UN GESTE (`apercu` : la lecture que la route nomme avec lui) : à chaque case changée de `bloc` (un
 *  fichier, une fois lu), la lecture reçoit la charge en cours, celle que le geste enverrait, et ce qu'elle rend se
 *  pose dans `cible`, sous les cases : ses phrases, puis l'image qu'elle montre (`media`, `image` en base64), qui
 *  s'ouvre en grand d'un clic ; un refus s'y dit. Tant qu'une case que la lecture exige (sa fiche) est vide, rien ne se
 *  montre ; seule la dernière réponse se pose. */
function montrerLApercu(bloc, g, cible) {
  let numero = 0;
  const montrer = async () => {
    const n = ++numero;
    // Aucune case lue : rien ne s'appelle.
    const charge = chargeDuGeste(bloc, g);
    if (!charge || CATALOGUE[g.apercu].charge.some((c) => c.obligatoire && charge[c.nom] == null)) { cible.innerHTML = ""; return; }
    let r;
    try { r = await api(g.apercu, charge); } catch (e) {
      if (n === numero) cible.innerHTML = `<div class="encart erreur">${echapper(e.message)}</div>`;
      return;
    }
    if (n !== numero) return;
    cible.innerHTML = `${phrasesDe(r).map((p) => `<p class="note">${echapper(p)}</p>`).join("")}${r.image && r.media
      ? `<img src="data:${echapper(r.media)};base64,${echapper(r.image)}" alt="">` : ""}`;
    const img = cible.querySelector("img");
    if (img) img.onclick = () => montrerEnGrand(img.src, img.alt);
  };
  // Après le changement entier : le bloc des cases qu'un choix ouvre se redessine d'abord (plus haut, sur `document`).
  const ensuite = () => setTimeout(montrer);
  bloc.addEventListener("change", ensuite);
  bloc.addEventListener("case-lue", ensuite);
  montrer();
}

/** Ce que les cases du geste disent, dans `bloc`, ajouté à sa charge ; `null` si aucune ne dit rien. */
function chargeDuGeste(bloc, g) {
  const lu = lireLesChamps(bloc, champsDuGeste(g));
  return Object.keys(lu).length ? { ...g.charge, ...lu } : null;
}

/** Branche les boutons des gestes de `cible` : un geste qui n'a ni case ni phrase part avec sa charge ; un autre ouvre sa
 *  boîte (sans case, elle dit ses phrases et part par le bouton du geste : une confirmation). Un geste qui exige le mot
 *  de passe du logiciel (`demande`, rendue par la route) le fait taper dans la boîte du mot de passe avant de partir ; il
 *  part dans la charge, et la page ne le garde pas.
 *  `entete` : ce que la boîte dit de quoi il s'agit (`titre`, à la suite du libellé ; `dit`, dessous) ; `pardessus` :
 *  la boîte s'ouvre par-dessus celle qui est ouverte, qui revient quand elle se ferme ; `modifie` : la boîte modifie une
 *  chose qui existe (`casesDesChamps`) ; `quitter` : le bloc dont les écrans retenus se quittent avant le geste, qui
 *  les redessinera (`quitterLEcran` : enregistrer, jeter ou rester). `apres` : après la réponse. */
function brancherLesGestes(cible, gestes, entete, apres) {
  cible.querySelectorAll("[data-geste]").forEach((b) => {
    const g = gestes[+b.dataset.geste];
    b.onclick = async () => {
      if (entete.quitter && !(await quitterLEcran(entete.quitter))) return;
      await faireLeGeste(g, entete, apres, b);
    };
  });
}

/** LA RÉPONSE D'UN ENVOI PARTI EN FOND (`en_fond`, routes/src/envois_en_fond.rs) : la boîte se ferme, la phrase se
 *  dit, et le bandeau suit l'envoi (`suivreVeille`, js/veille.js : seule une compagnie ouverte en reçoit). Vrai si c'en
 *  est une : ce qui suivrait un envoi attendu (ce qui est parti, l'avis) se dit dans le bandeau. */
function partiEnFond(r) {
  if (!r || !r.en_fond) return false;
  fermerBoite();
  dire(r.phrase);
  // Vu en cours dès sa réponse : s'il est au bout avant que le bandeau le lise, sa place se repeint quand même.
  ENVOIS_VUS_EN_COURS.add(r.en_fond);
  suivreVeille();
  // Le cadre sous le mois (`etat-du-mois`) dit l'envoi en cours.
  rafraichirActions();
  return true;
}

/** `bouton` : celui du geste, éteint le temps de la réponse quand le geste part sans boîte. `apres` reçoit la réponse.
 *  Un geste dont la route engage (sa fiche : `acte`) est un envoi pour la boîte du mot de passe (`envoi`) ; le bouton de
 *  sa boîte a la forme que lui donne la route (`classeDuGeste`), en bouton principal. */
async function faireLeGeste(g, entete, apres, bouton) {
  const envoi = CATALOGUE[g.route].acte === "engage";
  const partir = async (charge, boite) => {
    const motdepasse = g.demande ? await demanderMotDePasse(g.demande, { envoi }) : null;
    if (g.demande && !motdepasse) return false;
    // Le bouton qui fait partir le geste s'éteint le temps de la réponse (lier le Drive attend qu'on accepte chez
    // Google) : on ne clique pas deux fois.
    // Un acte en cours se voit : le bouton dit son geste suivi de « … » le temps de la réponse.
    const rallumer = occupe(boite ? document.getElementById("geste-ok") : bouton, `${g.libelle.replace(/…$/, "")}…`);
    let r;
    try { r = await api(g.route, motdepasse ? { ...charge, motdepasse } : charge); } catch (e) {
      rallumer();
      // Un refus que l'écran sait reprendre (`entete.refus` rend vrai) ne se dit pas deux fois.
      if (!(entete.refus && entete.refus(e, g))) dire(e.message, "erreur");
      return false;
    }
    // Parti en fond : le bandeau dit la suite (`partiEnFond`).
    if (partiEnFond(r)) { rallumer(); return ENREGISTRE_PUIS_SUITE; }
    if (boite) fermerBoite();
    dire(r.phrase, r.couleur);
    await apres(r);
    rallumer();
    // Ce que la route dit qui suit (`suite` : le dépôt d'un signalement écrit) s'ouvre ensuite.
    if (r.suite) ouvrirGeste(r.suite);
    // Écrit, et la boîte déjà fermée par lui : quitter ne la referme pas (elle peut être la suite).
    return ENREGISTRE_PUIS_SUITE;
  };
  if (!g.cases.length && !g.phrases.length) { await partir(g.charge, false); return; }
  const rendre = entete.pardessus ? mettreDeCote() : null;
  ouvrirBoite(`
    <h3>${echapper(g.libelle)}${entete.titre ? ` — ${echapper(entete.titre)}` : ""}</h3>
    ${entete.dit ? `<p class="discret">${echapper(entete.dit)}</p>` : ""}
    <div id="geste-cases">${casesDuGeste(g, { modifie: entete.modifie })}</div>
    ${g.apercu ? `<div id="geste-apercu" class="geste-apercu"></div>` : ""}
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="${classeDuGeste({ ...g, rang: "principal" })}" id="geste-ok">${echapper(g.libelle.replace(/…$/, ""))}</button>
    </div>`, rendre);
  const bloc = document.getElementById("geste-cases");
  brancherLesCases(bloc);
  if (g.apercu) montrerLApercu(bloc, g, document.getElementById("geste-apercu"));
  // Une case vidée ne part pas : la route dit ce qu'une case absente fait (une reprise absente s'efface) ; une case que
  // la route exige, vide, retient le geste et le dit sur elle.
  const enregistrer = async () => {
    const charge = chargeDuGeste(bloc, g) || g.charge;
    // Seules les cases ouvertes se marquent : une case qu'un autre choix ouvrirait ne manque pas.
    if (marquerLesManquantes(bloc, champsOuverts(g, { ...g.valeurs, ...lireLesCases(bloc) }), charge)) return false;
    return partir(charge, true);
  };
  document.getElementById("geste-ok").onclick = enregistrer;
  // Quitter la boîte d'un clic à côté avec une saisie non enregistrée se demande (`retenirLEcran`) ; « Annuler » la
  // ferme sans rien garder.
  if (g.cases.length) {
    retenirLEcran(bloc, { enregistrer });
    CLIC_DEHORS = async () => { if (await quitterLEcran(document.getElementById("boite"))) fermerBoite(); };
  }
}

/* ------------------------------------------------ quitter un écran posé */

/** LES ÉCRANS QUI SE SAISISSENT, tels qu'ils sont posés (une fiche, « Pour démarrer », un écran rendu) : chacun le bloc
 *  de ses cases, ce qu'elles disaient à l'ouverture (`lesCasesLues`), ce qui l'enregistre (`enregistrer` : vrai quand
 *  c'est écrit), ce qui le reprend tel qu'il est enregistré quand on jette (`jeter`, s'il en a), et ce qui montre, à
 *  chaque frappe, s'il a changé (`quandChange`, s'il en a). Plusieurs peuvent être à l'écran ensemble : la fiche
 *  Entreprise et « Pour démarrer » au-dessus d'elle. */
let ECRANS_RETENUS = [];

/** Ce que disent les cases d'un bloc, tel qu'on le compare. */
const casesDuBloc = (bloc) => JSON.stringify(lesCasesLues(bloc));

/** Un écran retenu a changé : il est à l'écran et ses cases ne disent plus ce qu'elles disaient à l'ouverture. */
const aChange = (e) => e.bloc.isConnected && casesDuBloc(e.bloc) !== e.depart;

/** Retient l'écran dont les cases sont dans `bloc`, tel qu'il s'ouvre (`o` : `enregistrer`, `jeter`, `quandChange`). */
function retenirLEcran(bloc, o) {
  ECRANS_RETENUS = ECRANS_RETENUS.filter((e) => e.bloc !== bloc && e.bloc.isConnected);
  ECRANS_RETENUS.push({ bloc, depart: casesDuBloc(bloc), enregistrer: o.enregistrer, jeter: o.jeter || null, quandChange: o.quandChange || null });
}

/** Oublie l'écran de `bloc` : ce qu'il porte vient d'être écrit, ou abandonné d'un geste (« Annuler »). */
function oublierLEcran(bloc) {
  ECRANS_RETENUS = ECRANS_RETENUS.filter((e) => e.bloc !== bloc);
}

/** L'écran de `bloc` porte une saisie qui n'est pas enregistrée. */
function ecranModifie(bloc) {
  const e = ECRANS_RETENUS.find((x) => x.bloc === bloc);
  return !!e && aChange(e);
}

// Chaque frappe dans un écran retenu lui dit s'il a changé (la mention « modifié » d'une fiche, son « Annuler »).
["input", "change"].forEach((type) => document.addEventListener(type, (ev) => {
  ECRANS_RETENUS.filter((e) => e.quandChange && e.bloc.contains(ev.target)).forEach((e) => e.quandChange(aChange(e)));
}));

/** LA BOÎTE DES CHANGEMENTS NON ENREGISTRÉS, la même pour tout écran retenu : « enregistrer », « jeter », ou « rester »
 *  quand on la ferme d'un clic à côté. La boîte ouverte dessous (celle de l'ouverture) est mise de côté, et revient sauf
 *  si l'on jette. */
async function demanderSiEnregistrer() {
  const rendre = mettreDeCote();
  const choix = await new Promise((resoudre) => {
    let repondu = false;
    const repondre = (v) => { if (!repondu) { repondu = true; resoudre(v); } };
    ouvrirBoite(`<h3>${aideDe("non_enregistres_titre")}</h3>
      <p>${aideDe("non_enregistres")}</p>
      <div class="actions">
        <button class="primaire" id="q-enregistrer">${aideDe("garder_changements")}</button>
        <button class="secondaire" id="q-jeter">${aideDe("jeter_changements")}</button>
      </div>`, () => repondre("rester"));
    // Répondre d'abord, fermer ensuite : dans l'autre ordre, `fermerBoite` appellerait le rappel de fermeture
    // (« rester ») avant le bouton.
    document.getElementById("q-enregistrer").onclick = () => { repondre("enregistrer"); fermerBoite(); };
    document.getElementById("q-jeter").onclick = () => { repondre("jeter"); fermerBoite(); };
  });
  if (rendre && choix !== "jeter") rendre();
  return choix;
}

/** QUITTER LES ÉCRANS RETENUS (ceux de `dans` : la page entière, ou une boîte qui se ferme) : rien à demander pour un
 *  écran qui n'est plus à l'écran, où l'on n'a rien changé ou où l'on est revenu à ce qui était ; pour chacun des autres,
 *  la boîte des changements non enregistrés (`demanderSiEnregistrer`) : enregistrer, jeter (l'écran se reprend tel
 *  qu'il est enregistré), ou rester. Rend `true` si l'on peut partir. */
async function quitterLEcran(dans = document) {
  ECRANS_RETENUS = ECRANS_RETENUS.filter((e) => e.bloc.isConnected);
  for (const e of ECRANS_RETENUS.filter((x) => dans.contains(x.bloc))) {
    if (!aChange(e)) continue;
    const choix = await demanderSiEnregistrer();
    if (choix === "rester") return false;
    if (choix === "jeter") {
      oublierLEcran(e.bloc);
      if (e.jeter) await e.jeter();
      continue;
    }
    const fait = await e.enregistrer();
    if (!fait) return false;
    oublierLEcran(e.bloc);
    // L'enregistrement a ouvert la page qui le suit : on reste sur elle.
    if (fait === ENREGISTRE_PUIS_SUITE) return false;
  }
  return true;
}
/** Ce que rend l'`enregistrer` d'un écran retenu quand il a écrit et ouvert la page qui suit (les indemnités d'un arrêt,
 *  les jours payés) : la sortie ne la referme pas. Sinon il rend vrai (écrit) ou faux (refusé). */
const ENREGISTRE_PUIS_SUITE = "suite";

/* ------------------------------------------------ un écran que la route rend */

/** La route qui rend l'écran de la place `place` (sa fiche : `affichage: "rendu"`), s'il en est une. */
function routeQuiRend(place) {
  return Object.values(CATALOGUE).find((f) => f.affichage === "rendu" && f.ecran === place);
}

/** L'IMAGE `src` EN GRAND, par-dessus la page ; un clic ou Échap la referme. */
function montrerEnGrand(src, alt) {
  const voile = document.createElement("div");
  voile.className = "image-en-grand";
  voile.innerHTML = `<img src="${echapper(src)}" alt="${echapper(alt || "")}">`;
  const fermer = () => { voile.remove(); document.removeEventListener("keydown", echap, true); };
  const echap = (ev) => { if (ev.key === "Escape") { ev.stopPropagation(); fermer(); } };
  voile.onclick = fermer;
  document.addEventListener("keydown", echap, true);
  document.body.appendChild(voile);
}

/** DESSINE DANS `cible` L'ÉCRAN QUE LA ROUTE `f` REND (`routeQuiRend`). Sa charge : le mois de l'écran quand sa fiche le
 *  prend (`mois`), celui de l'onglet Paie ; sans lui, la route prend le sien. Un refus se dit à la place de l'écran. */
async function dessinerLEcranRendu(cible, f) {
  const mois = f.charge.some((c) => c.nom === "mois" && c.forme === "mois") && typeof MOIS_AFFICHE === "string" && MOIS_AFFICHE
    ? { mois: MOIS_AFFICHE } : {};
  let r;
  try { r = await api(f.nom, mois); }
  catch (e) { cible.innerHTML = `<div class="encart erreur">${echapper(e.message)}</div>`; return; }
  poserLEcranRendu(cible, r, () => dessinerLEcranRendu(cible, f));
}

/** POSE DANS `cible` L'ÉCRAN RENDU `r` (`routes::ecran::EcranRendu`) ; ce qu'il n'a pas, la route ne l'écrit pas.
 *  - Ce qu'il dit en tête : ses encarts.
 *  - Ses rangs : chacun ce qui s'y regarde (`montre`, le document que la route nomme : à gauche, la place gardée sur
 *    tous les rangs dès qu'un rang en a un), son titre, ce qu'il dit, sa pastille et son état, et ses gestes
 *    (`brancherLesGestes`, la boîte titrée du titre du rang ; ceux dont la route est avancée, sous « Avancé ») ; après
 *    un geste, `relire`.
 *  - Ses cases, dessinées comme celles d'une fiche (`casesDeLaFiche`, l'historique de ses cases datées compris), et le
 *    bouton de son geste (`enregistrer` : sa route, sa charge, le champ de la charge où vont les cases, `dans`, et son
 *    libellé). Enregistrer envoie ce que les cases disent et pose l'écran que la route rend en réponse ; un refus se
 *    dit, et les saisies restent. Quitter avec une saisie non enregistrée se demande (`retenirLEcran`). */
function poserLEcranRendu(cible, r, relire) {
  const rangs = r.rangs || [];
  const images = rangs.some((g) => g.montre);
  const apercu = (g) => (!images ? "" : `<div class="rang-apercu">${g.montre
    ? `<img src="${echapper(adresseDuDocument(g.montre))}" alt="${echapper(g.montre.libelle)}">` : ""}</div>`);
  const dessinRangs = !rangs.length ? "" : `<div class="rangs">${rangs.map((g, i) => `
      <div class="rang${images ? " rang-image" : ""}" data-rang="${i}">
        ${apercu(g)}
        <div class="rang-infos">
          <b class="rang-nom">${echapper(g.titre)}</b>
          ${g.dit.map((l) => `<span class="discret">${echapper(l)}</span>`).join("")}
          <div>${pastilleDeLEtat(g.pastille)}${g.etat ? ` <span class="discret">${echapper(g.etat)}</span>` : ""}</div>
        </div>
        <div class="rang-actions">${boutonsRanges(g.gestes)}</div>
      </div>`).join("")}</div>`;
  const g = r.enregistrer;
  const cases = !r.cases ? "" : `<div data-ecran-cases>${casesDeLaFiche({ cases: r.cases, historique: r.historique || [] })}</div>
    ${g ? `<div class="actions"><button class="primaire" data-enregistrer>${echapper(g.libelle)}</button></div>` : ""}`;
  const dedans = `${encarts(r.encarts || [])}${dessinRangs}${cases}`;
  cible.innerHTML = r.cases ? `<div class="carte">${dedans}</div>` : dedans;
  cible.querySelectorAll("[data-rang]").forEach((b) => {
    const x = rangs[+b.dataset.rang];
    brancherLesGestes(b, x.gestes, { titre: x.titre }, relire);
    // Un fichier déposé sur le rang ouvre le geste du rang qui prend un fichier (quand il n'y en a qu'un), le fichier
    // déjà choisi dans sa case.
    const prenants = x.gestes.filter((g) => champsDuGeste(g).some((c) => c.forme === "fichier"));
    if (prenants.length === 1) {
      recevoirLeFichier(b, () => (b.isConnected ? (fichiers) => {
        faireLeGeste(prenants[0], { titre: x.titre }, relire);
        const c = seuleCaseFichier(document.getElementById("geste-cases"));
        if (c) deposerLeFichier(c, fichiers);
      } : null));
    }
    // L'image du rang s'ouvre en grand d'un clic.
    const img = b.querySelector(".rang-apercu img");
    if (img) img.onclick = () => montrerEnGrand(img.src, img.alt);
  });
  if (!r.cases) return;
  const bloc = cible.querySelector("[data-ecran-cases]");
  brancherLesCases(bloc);
  if (!g) return;
  const enregistrer = async () => {
    let x;
    try { x = await api(g.route, { ...g.charge, [g.dans]: lireLesCases(bloc) }); }
    catch (e) { dire(e.message, "erreur"); return false; }
    dire(x.phrase);
    poserLEcranRendu(cible, x, relire);
    return true;
  };
  cible.querySelector("[data-enregistrer]").onclick = enregistrer;
  retenirLEcran(bloc, { enregistrer });
}

/* ------------------------------------------------ une fiche que la route rend */

/** L'HISTORIQUE D'UNE FICHE (`historique` de la route `fiche`) : replié, une rangée par valeur de chaque case datée, en
 *  mots, celle d'aujourd'hui en gras. Rien quand aucune case n'a changé. */
function historiqueDeLaFiche(h) {
  if (!h.length) return "";
  return `<details class="champ historique-fiche">
    <summary>${aideDe("historique_titre")}</summary>
    <div class="aide">${aideDe("historique_fiche")}</div>
    <div class="histoire">${h.map((t) => t.valeurs.map((e) => `
      <div class="histoire-ligne${e.en_vigueur ? " en-vigueur" : ""}">
        <span class="histoire-champ">${echapper(t.libelle)}</span>
        <span class="histoire-valeur">${echapper(e.valeur)}</span>
        <span class="histoire-date">${echapper(e.dit)}</span>
        ${e.note ? `<div class="histoire-note">${echapper(e.note)}</div>` : ""}
      </div>`).join("")).join("")}</div>
  </details>`;
}

/** LES CASES D'UNE FICHE RENDUE (`f`), dans l'ordre où la route les rend, chacune une case de route d'une chose qui existe
 *  (`caseDeRoute`, `modifiable`) : la grille de base, puis celles qu'elle place sous « Avancé », ouvert quand la route le
 *  dit (`avance_ouvert` : une case s'écarte du cas ordinaire), puis l'historique. */
function casesDeLaFiche(f, depart = {}) {
  // Un titre quand le bloc change (`bloc`, rendu par la route ; un bloc sans titre n'en pose pas).
  const de = (place) => {
    let bloc = null;
    return f.cases.filter((c) => c.place === place).map((c) => {
      const titre = c.bloc !== bloc && c.bloc ? `<h3 class="bloc-titre">${echapper(c.bloc)}</h3>` : "";
      bloc = c.bloc;
      return titre + caseDeRoute(modifiable(c, depart[c.cle]));
    }).join("");
  };
  return grilleDesCases(de("base"), de("avance"), f.avance_ouvert, "fiche-cases") + historiqueDeLaFiche(f.historique);
}

/** CE QUI DESSINE UNE FICHE RENDUE, sans ce qui change en tapant : ses cases, leurs parties ouvertes et leurs choix. */
const dessinDeLaFiche = (f) => JSON.stringify(f.cases.map((c) => [c.cle, c.choix,
  (c.parties || []).filter((p) => p.ouverte).map((p) => [p.cle, p.choix])]));

/** POSE DANS `cible` LA FICHE RENDUE `f` : la première fois, quand la route rend un autre dessin (d'autres cases, d'autres
 *  parties ouvertes, d'autres choix), ou quand elle a récrit des cases (`reecrites` : elles montrent ce qu'elle rend),
 *  elle se dessine (ce qui est déjà tapé reste dans sa case, et le curseur aussi ; un oui-non garde pour pré-rempli sa
 *  valeur d'avant la saisie : `modifiable`) ; sinon, seuls changent ses fautes (`fautesDesCases`) et ce que le moteur dit
 *  sous chaque case. Un tiroir « Avancé » ouvert à la main le reste. */
function poserLaFiche(cible, f) {
  const reecrites = new Set(f.reecrites || []);
  // Chaque contrôle par sa case, sa rangée et sa partie.
  const controles = () => [...cible.querySelectorAll("[data-case]")].map((x) => {
    const c = x.closest(".case[data-cle]");
    const r = x.closest(".rangee");
    const p = x.closest("[data-partie]");
    return [`${c ? c.dataset.cle : ""}|${r ? [...r.parentElement.children].indexOf(r) : ""}|${p ? p.dataset.partie : ""}`, x];
  });
  if (!cible.dataset.posee || cible.dataset.dessin !== dessinDeLaFiche(f) || reecrites.size) {
    if (!cible.dataset.posee) cible.depart = Object.fromEntries(f.cases.map((c) => [c.cle, c.valeur]));
    cible.dataset.posee = "1";
    cible.dataset.dessin = dessinDeLaFiche(f);
    const tapees = new Map(controles());
    const actif = controles().find(([, x]) => x === document.activeElement || x.contains(document.activeElement));
    const avanceOuvert = cible.querySelector(":scope > details.e-avance[open]");
    cible.innerHTML = casesDeLaFiche(f, cible.depart);
    if (avanceOuvert) cible.querySelector(":scope > details.e-avance").open = true;
    controles().forEach(([k, x]) => {
      const t = tapees.get(k);
      if (!t || reecrites.has(k.split("|")[0])) return;
      if (x.isContentEditable) x.innerHTML = t.innerHTML;
      else if (x.type === "checkbox") x.checked = t.checked;
      else if (x.tagName === "INPUT" || x.tagName === "SELECT" || x.tagName === "TEXTAREA") x.value = t.value;
    });
    brancherLesCases(cible);
    const garde = actif ? controles().find(([k]) => k === actif[0]) : null;
    if (garde) garde[1].focus();
  } else {
    f.cases.forEach((c) => {
      const d = cible.querySelector(`.case[data-cle="${CSS.escape(c.cle)}"] > [data-dits]`);
      if (d) d.innerHTML = ditsDe(c);
    });
  }
  fautesDesCases(cible, f.cases);
}
