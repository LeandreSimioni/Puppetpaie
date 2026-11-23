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
 *  mais fermée vidée (elle n'a pas de sens pour ce qu'on saisit) ; une clé que la route ne décrit pas se garde. */
function lireLesParties(x) {
  const lu = JSON.parse(x.dataset.valeur || "null") || {};
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
  // UNE VALEUR FAITE DE PARTIES (`partiesDe`, `lireLesParties`). Vide de tout ce qu'elle dit, la valeur est vide.
  parties: {
    html: (v, o) => `<span class="case-parties" data-case${attributsDesParties(v, o)}>${partiesDe(v || {}, o)}</span>`,
    lire: (x) => { const lu = lireLesParties(x); return rienDit(lu) ? null : lu; },
  },
  // DES RANGÉES (les autres établissements) : chacune ses parties (`o.parties`), une croix qui la retire, et sous elle
  // ses temps (`o.temps.lignes`, par la clé de la rangée) ; « + » en ajoute une (`o.ajouter`). Une rangée qui ne dit rien
  // ne part pas ; plus aucune rangée, la valeur est vide.
  rangees: {
    html: (v, o) => {
      const tempsDe = (l) => {
        const t = o.temps && l && l.nic ? o.temps.lignes.find((x) => x.cle === l.nic) : null;
        return t ? t.cases.flatMap((c) => [c.avant, c.a_venir]).filter(Boolean)
          .map((x) => `<small class="case-dit">${echapper(x.valeur)} · ${echapper(x.dit)}</small>`).join("") : "";
      };
      const rangee = (l) => `<div class="rangee"><span class="case-parties" data-rangee${attributsDesParties(l, o)}>${partiesDe(l || {}, o)}</span>
        <button type="button" class="lien retirer-rangee" title="retirer" aria-label="retirer">&times;</button>${tempsDe(l)}</div>`;
      return `<div class="case-rangees" data-case>${(Array.isArray(v) ? v : []).map(rangee).join("")}
        <template>${rangee(null)}</template><button type="button" class="secondaire" data-rangee-plus>${echapper(o.ajouter || "+")}</button></div>`;
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
        .map((c) => `<label class="coche"><input type="radio" name="${nom}" value="${echapper(c.code)}"${coche === c.code ? " checked" : ""}> ${
          echapper(c.libelle)}</label>${exemple(c)}`).join("")}</span>`;
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
  // Un fichier choisi se lit en base64 (sans l'en-tête de l'adresse data:) ; le moteur reconnaît son format à ses
  // premiers octets, pas à son extension.
  bloc.querySelectorAll('input[type="file"][data-case]').forEach((x) => {
    x.onchange = () => {
      delete x.dataset.contenu;
      const f = x.files && x.files[0];
      if (!f) return;
      const l = new FileReader();
      l.onload = () => { x.dataset.contenu = String(l.result).split(",").pop(); };
      l.onerror = () => dire(String(l.error), "erreur");
      l.readAsDataURL(f);
    };
  });
  // Les rangées : « + » en ajoute une (le modèle vide), la croix retire la sienne ; la case le dit comme une saisie.
  const dire = (x) => x.dispatchEvent(new Event("change", { bubbles: true }));
  bloc.querySelectorAll("[data-rangee-plus]").forEach((b) => {
    b.onclick = () => {
      b.before(b.previousElementSibling.content.cloneNode(true));
      brancherLesCases(b.parentElement, o);
      dire(b.parentElement);
    };
  });
  bloc.querySelectorAll(".retirer-rangee").forEach((b) => {
    b.onclick = () => { const liste = b.closest(".case-rangees"); b.closest(".rangee").remove(); dire(liste); };
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
  const balise = ["parties", "rangees", "html", "boutons"].includes(forme) ? "div" : "label";
  return `<${balise} class="case${c.largeur === "pleine" ? " pleine" : ""}" data-cle="${echapper(c.cle)}"${c.aide ? ` title="${echapper(c.aide)}"` : ""}><span class="case-libelle">${echapper(c.libelle)}</span>
    ${caseDeForme(forme, c.valeur, { choix: c.choix, prerempli: c.prerempli, exemple: c.exemple, parties: c.parties, ajouter: c.ajouter, temps: c.temps, admis: c.admis })}${
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
  const lu = {};
  bloc.querySelectorAll(".case[data-cle]").forEach((c) => {
    const x = c.querySelector(":scope > [data-case]");
    const v = lireLaCase(x);
    if (v === undefined || (v === null && x.dataset.prerempli != null)) return;
    lu[c.dataset.cle] = v;
  });
  return lu;
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
 *  `choix` (ceux que le geste rend, sinon ceux de la fiche), et pour une case vide l'exemple que la fiche de la route
 *  donne de ce champ (`o.exemple`, l'exemple de la fiche : une valeur simple). `o.modifie` : la boîte modifie une chose
 *  qui existe (`modifiable`), et les champs que la fiche dit avancés (`avance`) se rangent sous « Avancé ». */
function casesDesChamps(champs, valeurs, choix, o = {}) {
  const exemple = (nom) => {
    const x = o.exemple ? o.exemple[nom] : undefined;
    return typeof x === "string" || typeof x === "number" ? String(x) : undefined;
  };
  const une = (c) => {
    const d = { cle: c.nom, libelle: c.libelle, forme: c.forme, valeur: valeurs[c.nom], choix: choix[c.nom] || c.choix,
      exemple: exemple(c.nom), admis: c.admis };
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
      try {
        const r = await api(f.nom, lireLesChamps(bloc, f.charge));
        rendu.innerHTML = phrasesDe(r).map((p) => `<p>${echapper(p)}</p>`).join("");
      } catch (e) {
        rendu.innerHTML = `<div class="encart erreur">${echapper(e.message)}</div>`;
      }
    };
  });
}

/* ------------------------------------------------ l'écran qu'une route rend */

/** La route qui rend l'écran de la place `place` (sa fiche : `affichage: "montre"`), s'il en est une. */
function routeQuiMontre(place) {
  return Object.values(CATALOGUE).find((f) => f.affichage === "montre" && f.ecran === place);
}

/** MONTRE DANS `cible` L'ÉCRAN QUE REND LA ROUTE DE FICHE `f` (`routes::ecran::EcranRendu`) : ce qu'il dit en tête
 *  (`intro`), puis ses rangs, chacun ce qui s'y regarde (`montre`, le document que la route nomme : à gauche, la place
 *  gardée sur tous les rangs dès qu'un rang en a un), son titre, ce qu'il dit, sa pastille et son état, et ses gestes
 *  (`brancherLesGestes`, la boîte titrée du titre du rang). Après un geste, l'écran se relit ; le refus de la lecture
 *  se dit à sa place. */
async function montrerLEcran(cible, f) {
  let r;
  try { r = await api(f.nom); } catch (e) { cible.innerHTML = `<div class="encart erreur">${echapper(e.message)}</div>`; return; }
  const images = r.rangs.some((g) => g.montre);
  const apercu = (g) => (!images ? "" : `<div class="rang-apercu">${g.montre
    ? `<img src="${echapper(adresseDuDocument(g.montre))}" alt="${echapper(g.montre.libelle)}">` : ""}</div>`);
  cible.innerHTML = `${r.intro ? `<p class="intro">${echapper(r.intro)}</p>` : ""}
    <div class="rangs">${r.rangs.map((g, i) => `
      <div class="rang${images ? " rang-image" : ""}" data-rang="${i}">
        ${apercu(g)}
        <div class="rang-infos">
          <b class="rang-nom">${echapper(g.titre)}</b>
          ${g.dit.map((l) => `<span class="discret">${echapper(l)}</span>`).join("")}
          <div>${pastilleDeLEtat(g.pastille)}${g.etat ? ` <span class="discret">${echapper(g.etat)}</span>` : ""}</div>
        </div>
        <div class="rang-actions">${boutonsDesGestes(g.gestes)}</div>
      </div>`).join("")}</div>`;
  cible.querySelectorAll("[data-rang]").forEach((b) => {
    const g = r.rangs[+b.dataset.rang];
    brancherLesGestes(b, g.gestes, { titre: g.titre }, () => montrerLEcran(cible, f));
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

/** Ce que la boîte d'un geste dit (`phrases`), puis ses cases ; `o.modifie` : voir `casesDesChamps`. */
function casesDuGeste(g, o = {}) {
  return `${g.phrases.map((p) => `<p class="note">${echapper(p)}</p>`).join("")}
    ${casesDesChamps(champsDuGeste(g), g.valeurs, g.choix, { ...o, exemple: CATALOGUE[g.route].exemple })}`;
}

/** Ce que les cases du geste disent, dans `bloc`, ajouté à sa charge ; `null` si aucune ne dit rien. */
function chargeDuGeste(bloc, g) {
  const lu = lireLesChamps(bloc, champsDuGeste(g));
  return Object.keys(lu).length ? { ...g.charge, ...lu } : null;
}

/** Branche les boutons des gestes de `cible` : un geste sans case part avec sa charge ; un autre ouvre sa boîte.
 *  `entete` : ce que la boîte dit de quoi il s'agit (`titre`, à la suite du libellé ; `dit`, dessous) ; `pardessus` :
 *  la boîte s'ouvre par-dessus celle qui est ouverte, qui revient quand elle se ferme ; `modifie` : la boîte modifie une
 *  chose qui existe (`casesDesChamps`). `apres` : après la réponse. */
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
    <div id="geste-cases">${casesDuGeste(g, { modifie: entete.modifie })}</div>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="geste-ok">Enregistrer</button>
    </div>`, rendre);
  const bloc = document.getElementById("geste-cases");
  brancherLesCases(bloc);
  // Une case vidée ne part pas : la route dit ce qu'une case absente fait (une reprise absente s'efface).
  document.getElementById("geste-ok").onclick = () => partir(chargeDuGeste(bloc, g) || g.charge, true);
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
      else if (x.tagName === "INPUT" || x.tagName === "SELECT") x.value = t.value;
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
